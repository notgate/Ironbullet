use super::*;

impl ExecutionContext {
    pub(super) fn execute_parse_lr(
        &mut self,
        settings: &ParseLRSettings,
    ) -> crate::error::Result<()> {
        let source = self.variables.resolve_input(&settings.input_var);
        let left = self.variables.interpolate(&settings.left);
        let right = self.variables.interpolate(&settings.right);

        // Empty delimiters make a recursive parser non-advancing: `find("")`
        // always succeeds at the current cursor. Reject both modes rather than
        // permitting an accidental infinite loop or ambiguous extraction.
        if left.is_empty() || right.is_empty() {
            return Err(crate::error::AppError::Pipeline(
                "Parse LR requires non-empty left and right delimiters".into(),
            ));
        }

        if settings.recursive {
            let mut results = Vec::new();
            let mut search_from = 0;
            while let Some((_, left_end)) =
                find_literal(&source[search_from..], &left, settings.case_insensitive)
            {
                let abs_start = search_from + left_end;
                if let Some((right_start, right_end)) =
                    find_literal(&source[abs_start..], &right, settings.case_insensitive)
                {
                    results.push(source[abs_start..abs_start + right_start].to_string());
                    search_from = abs_start + right_end;
                } else {
                    break;
                }
            }
            let value = results.join(", ");
            self.variables
                .set_user(&settings.output_var, value, settings.capture);
        } else {
            let value = if let Some((_, left_end)) =
                find_literal(&source, &left, settings.case_insensitive)
            {
                if let Some((right_start, _)) =
                    find_literal(&source[left_end..], &right, settings.case_insensitive)
                {
                    source[left_end..left_end + right_start].to_string()
                } else {
                    String::new()
                }
            } else {
                String::new()
            };
            self.variables
                .set_user(&settings.output_var, value, settings.capture);
        }

        Ok(())
    }

    pub(super) fn execute_parse_regex(
        &mut self,
        settings: &ParseRegexSettings,
    ) -> crate::error::Result<()> {
        let source = self.variables.resolve_input(&settings.input_var);
        let pattern = self.variables.interpolate(&settings.pattern);
        let re = regex::RegexBuilder::new(&pattern)
            .multi_line(settings.multi_line)
            .build()?;

        if let Some(caps) = re.captures(&source) {
            let mut output = String::new();
            caps.expand(&settings.output_format, &mut output);
            self.variables
                .set_user(&settings.output_var, output, settings.capture);
        }

        Ok(())
    }

    pub(super) fn execute_parse_json(
        &mut self,
        settings: &ParseJSONSettings,
    ) -> crate::error::Result<()> {
        let source = self.variables.resolve_input(&settings.input_var);
        let path = self.variables.interpolate(&settings.json_path);

        // Empty source is not an error — just produce an empty output (same behaviour as OB2).
        // Failing the whole check on an empty body makes configs brittle for endpoints that
        // conditionally return JSON (e.g. success = JSON, failure = empty / redirect).
        if source.trim().is_empty() {
            self.variables
                .set_user(&settings.output_var, String::new(), settings.capture);
            return Ok(());
        }

        let json: serde_json::Value = serde_json::from_str(&source)
            .map_err(|e| crate::error::AppError::Pipeline(format!("Invalid JSON: {}", e)))?;

        // JSON Pointer (RFC 6901) pass-through: paths starting with '/'
        let value = if path.starts_with('/') {
            json.pointer(&path)
                .map(json_value_to_string)
                .unwrap_or_default()
        } else {
            // JSONPath-lite: supports dot-notation, [n] indexing, and [] wildcard
            // e.g.  "data.servers[].servers[].is_trial"
            //       "user.profile.name"
            //       "items[0].id"
            evaluate_json_path(&json, &path)
        };

        self.variables
            .set_user(&settings.output_var, value, settings.capture);
        Ok(())
    }

    pub(super) fn execute_parse_css(
        &mut self,
        settings: &ParseCSSSettings,
    ) -> crate::error::Result<()> {
        let source = self.variables.resolve_input(&settings.input_var);
        let selector_str = self.variables.interpolate(&settings.selector);
        let attribute = self.variables.interpolate(&settings.attribute);

        let document = scraper::Html::parse_document(&source);
        let selector = scraper::Selector::parse(&selector_str).map_err(|e| {
            crate::error::AppError::Pipeline(format!(
                "Invalid CSS selector '{}': {:?}",
                selector_str, e
            ))
        })?;

        let elements: Vec<_> = document.select(&selector).collect();
        let extract = |el: &scraper::ElementRef<'_>| {
            if attribute.is_empty() || attribute == "text" || attribute == "innerText" {
                el.text().collect::<Vec<_>>().join("")
            } else if attribute == "innerHTML" || attribute == "html" {
                el.inner_html()
            } else if attribute == "outerHTML" {
                el.html()
            } else {
                el.value().attr(&attribute).unwrap_or("").to_string()
            }
        };
        let value = match settings.index {
            -1 => elements
                .iter()
                .map(|element| extract(element).trim().to_string())
                .collect::<Vec<_>>()
                .join(", "),
            index if index >= 0 => elements
                .get(index as usize)
                .map(extract)
                .unwrap_or_default(),
            _ => String::new(),
        };

        self.variables.set_user(
            &settings.output_var,
            value.trim().to_string(),
            settings.capture,
        );
        Ok(())
    }

    pub(super) fn execute_parse_xpath(
        &mut self,
        settings: &ParseXPathSettings,
    ) -> crate::error::Result<()> {
        let source = self.variables.resolve_input(&settings.input_var);
        let xpath_str = self.variables.interpolate(&settings.xpath);

        let package = sxd_document::parser::parse(&source);
        let value = match package {
            Ok(package) => {
                let doc = package.as_document();
                let factory = sxd_xpath::Factory::new();
                match factory.build(&xpath_str) {
                    Ok(Some(xpath)) => {
                        let ctx = sxd_xpath::Context::new();
                        match xpath.evaluate(&ctx, doc.root()) {
                            Ok(val) => match val {
                                sxd_xpath::Value::String(s) => s,
                                sxd_xpath::Value::Number(n) => n.to_string(),
                                sxd_xpath::Value::Boolean(b) => b.to_string(),
                                sxd_xpath::Value::Nodeset(ns) => ns
                                    .iter()
                                    .map(|node| node.string_value())
                                    .collect::<Vec<_>>()
                                    .join(", "),
                            },
                            Err(_) => String::new(),
                        }
                    }
                    Ok(None) => String::new(),
                    Err(e) => {
                        return Err(crate::error::AppError::Pipeline(format!(
                            "Invalid XPath '{}': {:?}",
                            xpath_str, e
                        )));
                    }
                }
            }
            Err(_) => {
                // If the source isn't valid XML, try wrapping in a root element
                let wrapped = format!("<root>{}</root>", source);
                match sxd_document::parser::parse(&wrapped) {
                    Ok(package) => {
                        let doc = package.as_document();
                        let factory = sxd_xpath::Factory::new();
                        match factory.build(&xpath_str) {
                            Ok(Some(xpath)) => {
                                let ctx = sxd_xpath::Context::new();
                                match xpath.evaluate(&ctx, doc.root()) {
                                    Ok(val) => match val {
                                        sxd_xpath::Value::String(s) => s,
                                        sxd_xpath::Value::Number(n) => n.to_string(),
                                        sxd_xpath::Value::Boolean(b) => b.to_string(),
                                        sxd_xpath::Value::Nodeset(ns) => ns
                                            .iter()
                                            .map(|n| n.string_value())
                                            .collect::<Vec<_>>()
                                            .join(", "),
                                    },
                                    Err(_) => String::new(),
                                }
                            }
                            _ => String::new(),
                        }
                    }
                    Err(_) => String::new(),
                }
            }
        };

        self.variables.set_user(
            &settings.output_var,
            value.trim().to_string(),
            settings.capture,
        );
        Ok(())
    }

    pub(super) fn execute_parse_cookie(
        &mut self,
        settings: &ParseCookieSettings,
    ) -> crate::error::Result<()> {
        let source = self.variables.resolve_input(&settings.input_var);
        let cookie_name = self.variables.interpolate(&settings.cookie_name);

        // Source is expected to be a JSON object {"name":"value",...}
        let value = if let Ok(map) =
            serde_json::from_str::<std::collections::HashMap<String, String>>(&source)
        {
            map.get(&cookie_name).cloned().unwrap_or_default()
        } else {
            // Fallback: try parsing as "name=value; name2=value2" cookie header string
            source
                .split(';')
                .filter_map(|pair| {
                    let pair = pair.trim();
                    let eq = pair.find('=')?;
                    Some((&pair[..eq], &pair[eq + 1..]))
                })
                .find(|(name, _)| *name == cookie_name)
                .map(|(_, v)| v.to_string())
                .unwrap_or_default()
        };

        self.variables
            .set_user(&settings.output_var, value, settings.capture);
        Ok(())
    }

    pub(super) fn execute_lambda_parser(
        &mut self,
        settings: &LambdaParserSettings,
    ) -> crate::error::Result<()> {
        let input = self.variables.resolve_input(&settings.input_var);
        let lambda_expr = self.variables.interpolate(&settings.lambda_expression);

        // Use simple DSL parser for lambda expressions
        // Supports: x => x.split(',')[0], x => x.trim(), etc.
        let result = self.simple_lambda_parser(&input, &lambda_expr);

        self.variables
            .set_user(&settings.output_var, result, settings.capture);
        Ok(())
    }

    // Simple fallback parser for basic lambda expressions
    fn simple_lambda_parser(&self, input: &str, lambda_expr: &str) -> String {
        // Support simple patterns like: x => x.split(',')[0]
        if let Some(arrow_pos) = lambda_expr.find("=>") {
            let body = lambda_expr[arrow_pos + 2..].trim();
            let mut result = input.to_string();

            // Parse chained operations
            if body.starts_with("x.") || body.starts_with("input.") {
                let operations = body.split('.').skip(1);
                for op in operations {
                    if let Some(paren_pos) = op.find('(') {
                        let method = &op[..paren_pos];
                        let args = &op[paren_pos + 1..op.len() - 1];

                        result = match method {
                            "split" => {
                                let delimiter = args.trim_matches(|c| c == '\'' || c == '"');
                                let parts: Vec<&str> = result.split(delimiter).collect();
                                serde_json::to_string(&parts).unwrap_or(result)
                            }
                            "trim" => result.trim().to_string(),
                            "toUpperCase" => result.to_uppercase(),
                            "toLowerCase" => result.to_lowercase(),
                            "replace" => {
                                let parts: Vec<&str> = args.split(',').collect();
                                if parts.len() == 2 {
                                    let search =
                                        parts[0].trim().trim_matches(|c| c == '\'' || c == '"');
                                    let replace =
                                        parts[1].trim().trim_matches(|c| c == '\'' || c == '"');
                                    result.replace(search, replace)
                                } else {
                                    result
                                }
                            }
                            _ => result,
                        };
                    } else if let Some(bracket_pos) = op.find('[') {
                        // Array indexing: [0], [1], etc.
                        let method = &op[..bracket_pos];
                        if method.is_empty() {
                            let index_str = &op[bracket_pos + 1..op.len() - 1];
                            if let Ok(index) = index_str.parse::<usize>() {
                                // Try to parse as JSON array
                                if let Ok(arr) = serde_json::from_str::<Vec<String>>(&result) {
                                    result = arr.get(index).cloned().unwrap_or_default();
                                } else {
                                    result = String::new();
                                }
                            }
                        }
                    }
                }
            }

            result
        } else {
            input.to_string()
        }
    }

    // ── Unified Parse dispatch ────────────────────────────────────────
    pub(super) fn execute_parse(
        &mut self,
        s: &crate::pipeline::block::ParseSettings,
    ) -> crate::error::Result<()> {
        use crate::pipeline::block::{
            LambdaParserSettings, ParseCSSSettings, ParseCookieSettings, ParseJSONSettings,
            ParseLRSettings, ParseMode, ParseRegexSettings, ParseXPathSettings,
        };
        match &s.parse_mode {
            ParseMode::LR => self.execute_parse_lr(&ParseLRSettings {
                input_var: s.input_var.clone(),
                left: s.left.clone(),
                right: s.right.clone(),
                output_var: s.output_var.clone(),
                capture: s.capture,
                recursive: s.recursive,
                case_insensitive: s.case_insensitive,
            }),
            ParseMode::Regex => self.execute_parse_regex(&ParseRegexSettings {
                input_var: s.input_var.clone(),
                pattern: s.pattern.clone(),
                output_format: s.output_format.clone(),
                output_var: s.output_var.clone(),
                capture: s.capture,
                multi_line: s.multi_line,
            }),
            ParseMode::Json => self.execute_parse_json(&ParseJSONSettings {
                input_var: s.input_var.clone(),
                json_path: s.json_path.clone(),
                output_var: s.output_var.clone(),
                capture: s.capture,
            }),
            ParseMode::Css => self.execute_parse_css(&ParseCSSSettings {
                input_var: s.input_var.clone(),
                selector: s.selector.clone(),
                attribute: s.attribute.clone(),
                output_var: s.output_var.clone(),
                capture: s.capture,
                index: s.index,
            }),
            ParseMode::XPath => self.execute_parse_xpath(&ParseXPathSettings {
                input_var: s.input_var.clone(),
                xpath: s.xpath.clone(),
                output_var: s.output_var.clone(),
                capture: s.capture,
            }),
            ParseMode::Cookie => self.execute_parse_cookie(&ParseCookieSettings {
                input_var: s.input_var.clone(),
                cookie_name: s.cookie_name.clone(),
                output_var: s.output_var.clone(),
                capture: s.capture,
            }),
            ParseMode::Lambda => self.execute_lambda_parser(&LambdaParserSettings {
                input_var: s.input_var.clone(),
                lambda_expression: s.lambda_expression.clone(),
                output_var: s.output_var.clone(),
                capture: s.capture,
            }),
        }
    }
}

/// Find a literal delimiter and return its byte-range in `source`.
/// Regex escaping keeps case-insensitive parsing literal rather than allowing
/// user delimiter text to alter parser semantics.
fn find_literal(source: &str, needle: &str, case_insensitive: bool) -> Option<(usize, usize)> {
    if case_insensitive {
        regex::RegexBuilder::new(&regex::escape(needle))
            .case_insensitive(true)
            .build()
            .ok()?
            .find(source)
            .map(|matched| (matched.start(), matched.end()))
    } else {
        source
            .find(needle)
            .map(|start| (start, start + needle.len()))
    }
}

// ── JSONPath-lite helper functions ────────────────────────────────────────────
// Supports:
//   • Root selector `$`:        `$.user.name`            → same as `user.name`
//   • Dot-notation key:         `user.name`              → json["user"]["name"]
//   • Array index:              `items[0].id`            → json["items"][0]["id"]
//   • Array wildcard `[]`:      `servers[].host`         → all servers[*].host
//   • Array wildcard `[*]`:     `servers[*].host`        → same as above
//   • Dot wildcard `.*`:        `obj.*`                  → all values of an object
//   • Bare wildcard `*`:        `*`                      → all top-level values
//   • Nested wildcards:         `a[].b[].c`              → flatten all
//   • Filter equality:          `items[?(@.type=='vip')].name`
//   • Filter inequality:        `items[?(@.active!='false')].id`
//   • Filter numeric cmp:       `items[?(@.score>10)].name`  (>, <, >=, <=)
//   • Filter existence:         `items[?(@.optional)].id`
//   • $ + filter on root:       `$[?(@.type=='vip')].name`  (root array filter)
//
// All matched leaf values are joined with ", " when multiple results exist.

/// Top-level entry: tokenise `path` and traverse `root`, returning all matches joined.
pub fn evaluate_json_path(root: &serde_json::Value, path: &str) -> String {
    // Strip the standard JSONPath root selector `$` (and optional trailing `.`)
    // so that `$.items[*].id` behaves identically to `items[*].id`.
    let path = path.trim();
    let path = if path == "$" {
        // Bare `$` → return the root value itself
        return json_value_to_string(root);
    } else if let Some(rest) = path.strip_prefix("$.") {
        rest
    } else if let Some(_rest) = path.strip_prefix("$[") {
        // e.g. `$[0]` or `$[*]` — re-attach the `[`
        // We need to keep the leading bracket so the tokeniser handles it
        &path[1..] // strip just the `$`
    } else {
        path
    };
    let segs = tokenise_json_path(path);
    let results = traverse_json(&[root], &segs);
    results.join(", ")
}

/// Path segment kinds produced by the tokeniser.
#[derive(Debug)]
enum JsonSeg {
    /// Navigate into an object key: `name`
    Key(String),
    /// Index into an array: `[3]`
    Index(usize),
    /// Flatten all elements of an array (or all values of an object): `[]` / `[*]` / `.*`
    Wild,
    /// Filter array elements: `[?(@.key op value)]` or `[?(@.key)]` (existence)
    Filter(FilterExpr),
}

/// A parsed filter expression from `[?(...)]`.
#[derive(Debug)]
struct FilterExpr {
    /// Dot-path relative to current element, e.g. `type` from `@.type`
    field: String,
    /// None means existence check; Some holds (op, rhs)
    cmp: Option<(FilterOp, String)>,
}

#[derive(Debug)]
enum FilterOp {
    Eq,
    Ne,
    Gt,
    Lt,
    Gte,
    Lte,
}

/// Tokenise a JSONPath string into segments.
fn tokenise_json_path(path: &str) -> Vec<JsonSeg> {
    let mut segs: Vec<JsonSeg> = Vec::new();
    let mut key_buf = String::new();

    let mut chars = path.chars().peekable();
    while let Some(ch) = chars.next() {
        match ch {
            // Dot separator — flush key buffer, then check for `.*` wildcard
            '.' => {
                if !key_buf.is_empty() {
                    segs.push(JsonSeg::Key(key_buf.clone()));
                    key_buf.clear();
                }
                // `.*` after dot → wild over object values
                if chars.peek() == Some(&'*') {
                    chars.next();
                    segs.push(JsonSeg::Wild);
                }
            }
            // `*` at start of a key segment (bare wildcard) → wild
            '*' if key_buf.is_empty() => {
                segs.push(JsonSeg::Wild);
            }
            // Bracket — flush key buffer, then parse what's inside
            '[' => {
                if !key_buf.is_empty() {
                    segs.push(JsonSeg::Key(key_buf.clone()));
                    key_buf.clear();
                }
                // Collect everything until matching ']'
                let mut inner = String::new();
                for c2 in chars.by_ref() {
                    if c2 == ']' {
                        break;
                    }
                    inner.push(c2);
                }
                let trimmed = inner.trim();
                if trimmed.is_empty() || trimmed == "*" {
                    segs.push(JsonSeg::Wild);
                } else if let Ok(n) = trimmed.parse::<usize>() {
                    segs.push(JsonSeg::Index(n));
                } else if trimmed.starts_with("?(") && trimmed.ends_with(')') {
                    // Filter expression: ?(...)
                    let expr = &trimmed[2..trimmed.len() - 1].trim();
                    if let Some(seg) = parse_filter_expr(expr) {
                        segs.push(JsonSeg::Filter(seg));
                    }
                    // Unknown filter → skip silently (no crash)
                }
                // After ']' there may be a leading '.' — skip it
                if chars.peek() == Some(&'.') {
                    chars.next();
                }
            }
            c => key_buf.push(c),
        }
    }
    if !key_buf.is_empty() {
        segs.push(JsonSeg::Key(key_buf));
    }
    segs
}

/// Parse the inside of `?(...)`.
/// Handles:
///   `@.field`               → existence check
///   `@.field=='value'`      → equality
///   `@.field!='value'`      → inequality
///   `@.field>10`            → numeric greater-than (and <, >=, <=)
fn parse_filter_expr(expr: &str) -> Option<FilterExpr> {
    let expr = expr.trim();
    // Must start with `@.`
    let rest = expr.strip_prefix("@.")?;

    // Try operators longest-first to avoid `>` matching `>=`
    let ops: &[(&str, FilterOp)] = &[
        (">=", FilterOp::Gte),
        ("<=", FilterOp::Lte),
        ("!=", FilterOp::Ne),
        ("==", FilterOp::Eq),
        (">", FilterOp::Gt),
        ("<", FilterOp::Lt),
    ];
    for (sym, _op) in ops {
        if let Some(pos) = rest.find(sym) {
            let field = rest[..pos].trim().to_string();
            let raw_rhs = rest[pos + sym.len()..].trim();
            // Strip surrounding quotes if present
            let rhs = if (raw_rhs.starts_with('\'') && raw_rhs.ends_with('\''))
                || (raw_rhs.starts_with('"') && raw_rhs.ends_with('"'))
            {
                raw_rhs[1..raw_rhs.len() - 1].to_string()
            } else {
                raw_rhs.to_string()
            };
            // Build the correct variant from the reference
            let op_variant = match sym {
                s if *s == ">=" => FilterOp::Gte,
                s if *s == "<=" => FilterOp::Lte,
                s if *s == "!=" => FilterOp::Ne,
                s if *s == "==" => FilterOp::Eq,
                s if *s == ">" => FilterOp::Gt,
                _ => FilterOp::Lt,
            };
            return Some(FilterExpr {
                field,
                cmp: Some((op_variant, rhs)),
            });
        }
    }
    // No operator → existence check
    Some(FilterExpr {
        field: rest.trim().to_string(),
        cmp: None,
    })
}

/// Evaluate a filter against a single JSON object element.
fn filter_matches(node: &serde_json::Value, f: &FilterExpr) -> bool {
    // Support dotted sub-paths within the filter field, e.g. `@.user.active`
    let field_val = f.field.split('.').try_fold(node, |v, key| v.get(key));

    match &f.cmp {
        None => {
            // Existence check: field must be present and not null/false
            match field_val {
                Some(serde_json::Value::Null) | None => false,
                Some(serde_json::Value::Bool(b)) => *b,
                Some(_) => true,
            }
        }
        Some((op, rhs)) => {
            let lhs_str = match field_val {
                Some(v) => json_value_to_string(v),
                None => return false,
            };
            // Try numeric comparison first
            if let (Ok(l), Ok(r)) = (lhs_str.parse::<f64>(), rhs.parse::<f64>()) {
                return match op {
                    FilterOp::Eq => (l - r).abs() < f64::EPSILON,
                    FilterOp::Ne => (l - r).abs() >= f64::EPSILON,
                    FilterOp::Gt => l > r,
                    FilterOp::Lt => l < r,
                    FilterOp::Gte => l >= r,
                    FilterOp::Lte => l <= r,
                };
            }
            // Fall back to string comparison
            match op {
                FilterOp::Eq => lhs_str == *rhs,
                FilterOp::Ne => lhs_str != *rhs,
                FilterOp::Gt => lhs_str > *rhs,
                FilterOp::Lt => lhs_str < *rhs,
                FilterOp::Gte => lhs_str >= *rhs,
                FilterOp::Lte => lhs_str <= *rhs,
            }
        }
    }
}

/// Recursively traverse a slice of JSON values following `segs`.
/// Returns a flat `Vec<String>` of all leaf values reached.
fn traverse_json<'a>(nodes: &[&'a serde_json::Value], segs: &[JsonSeg]) -> Vec<String> {
    // Base case: no more segments → convert each node to string
    if segs.is_empty() {
        return nodes.iter().map(|v| json_value_to_string(v)).collect();
    }

    let mut next: Vec<&'a serde_json::Value> = Vec::new();

    for &node in nodes {
        match &segs[0] {
            JsonSeg::Key(k) => {
                if let Some(v) = node.get(k.as_str()) {
                    next.push(v);
                }
            }
            JsonSeg::Index(n) => {
                if let Some(v) = node.as_array().and_then(|a| a.get(*n)) {
                    next.push(v);
                }
            }
            JsonSeg::Wild => {
                // Arrays: all elements; Objects: all values
                if let Some(arr) = node.as_array() {
                    next.extend(arr.iter());
                } else if let Some(obj) = node.as_object() {
                    next.extend(obj.values());
                }
            }
            JsonSeg::Filter(f) => {
                if let Some(arr) = node.as_array() {
                    next.extend(arr.iter().filter(|el| filter_matches(el, f)));
                }
            }
        }
    }

    traverse_json(&next, &segs[1..])
}

/// Convert a serde_json::Value to its string representation.
/// Strings are returned as-is; other types use JSON serialisation.
fn json_value_to_string(v: &serde_json::Value) -> String {
    match v {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Null => String::new(),
        other => other.to_string(),
    }
}

#[cfg(test)]
mod jsonpath_tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_bare_wildcard_object() {
        let v = json!({"a": 1, "b": 2, "c": 3});
        let r = evaluate_json_path(&v, "*");
        println!("bare * on object: {:?}", r);
        // Should return all values
        assert!(!r.is_empty());
    }

    #[test]
    fn test_bare_wildcard_array() {
        let v = json!([1, 2, 3]);
        let r = evaluate_json_path(&v, "*");
        println!("bare * on array: {:?}", r);
        assert!(!r.is_empty());
    }

    #[test]
    fn test_dollar_star() {
        let v = json!({"a": 1, "b": 2});
        let r = evaluate_json_path(&v, "$.*");
        println!("$.* on object: {:?}", r);
        // Should return both values (not empty)
        assert!(!r.is_empty());
    }

    #[test]
    fn test_dollar_prefix_key() {
        let v = json!({"items": {"id": 42}});
        let r = evaluate_json_path(&v, "$.items.id");
        assert_eq!(r, "42");
    }

    #[test]
    fn test_dollar_array_wildcard() {
        let v = json!({"items": [{"id": 1}, {"id": 2}]});
        let r1 = evaluate_json_path(&v, "$.items[*].id");
        let r2 = evaluate_json_path(&v, "items[*].id");
        assert_eq!(r1, r2);
    }

    #[test]
    fn test_dollar_bare_root() {
        let v = json!({"key": "val"});
        // Bare $ returns the whole root as JSON string
        let r = evaluate_json_path(&v, "$");
        assert!(r.contains("key"));
    }

    #[test]
    fn test_filter_equality() {
        let v =
            json!({"items": [{"type": "vip", "name": "Alice"}, {"type": "basic", "name": "Bob"}]});
        let r = evaluate_json_path(&v, "items[?(@.type=='vip')].name");
        println!("filter eq: {:?}", r);
        assert_eq!(r, "Alice");
    }

    #[test]
    fn test_filter_on_root_array() {
        let v = json!([{"x": 1, "name": "Alice"}, {"x": 2, "name": "Bob"}]);
        let r = evaluate_json_path(&v, "[?(@.x==1)].name");
        println!("filter on root array: {:?}", r);
        assert_eq!(r, "Alice");
    }

    #[test]
    fn test_wildcard_bracket_star() {
        let v = json!({"items": [{"id": 1}, {"id": 2}]});
        let r = evaluate_json_path(&v, "items[*].id");
        println!("items[*].id: {:?}", r);
        assert_eq!(r, "1, 2");
    }

    #[test]
    fn test_dollar_filter_root_array() {
        // $[?(@.type=='vip')].name on root array
        let v = json!([{"type": "vip", "name": "Alice"}, {"type": "basic", "name": "Bob"}]);
        let r = evaluate_json_path(&v, "$[?(@.type=='vip')].name");
        assert_eq!(r, "Alice");
    }
}

// Additional edge cases based on user reports
#[test]
fn test_bare_star_as_full_path() {
    // User types just "*" as the json_path on a JSON object
    let v = serde_json::json!({"token": "abc", "user": "alice"});
    let r = evaluate_json_path(&v, "*");
    println!("bare * on object result: {:?}", r);
    assert!(!r.is_empty(), "bare * should return all values");
}

#[test]
fn test_bare_star_on_array() {
    // User types just "*" as path, source is a JSON array
    let v = serde_json::json!(["a", "b", "c"]);
    let r = evaluate_json_path(&v, "*");
    println!("bare * on array result: {:?}", r);
    assert_eq!(r, "a, b, c");
}

#[test]
fn test_dollar_bracket_star() {
    // $[*] on root array
    let v = serde_json::json!([{"id":1},{"id":2}]);
    let r = evaluate_json_path(&v, "$[*].id");
    println!("$[*].id: {:?}", r);
    assert_eq!(r, "1, 2");
}

#[test]
fn test_filter_no_quotes_equals() {
    // Some JSONPath impls allow == without quotes for numbers
    let v = serde_json::json!([{"score": 10, "name": "Alice"}, {"score": 5, "name": "Bob"}]);
    let r = evaluate_json_path(&v, "[?(@.score==10)].name");
    println!("filter numeric == : {:?}", r);
    assert_eq!(r, "Alice");
}

#[test]
fn test_filter_nested_path_on_object() {
    // users[?(@.active==true)].name
    let v = serde_json::json!({"users": [{"name": "Alice", "active": true}, {"name": "Bob", "active": false}]});
    let r = evaluate_json_path(&v, "users[?(@.active==true)].name");
    println!("filter bool true: {:?}", r);
    // "true" string comparison — does it work?
}

#[test]
fn test_wildcard_on_nested_array_of_values() {
    // items[*] where items is array of strings — should return all
    let v = serde_json::json!({"items": ["x", "y", "z"]});
    let r = evaluate_json_path(&v, "items[*]");
    println!("items[*] of string array: {:?}", r);
    assert_eq!(r, "x, y, z");
}

#[test]
fn test_filter_existence_check() {
    // [?(@.optional)] — existence filter
    let v = serde_json::json!([{"name": "Alice", "optional": "yes"}, {"name": "Bob"}]);
    let r = evaluate_json_path(&v, "[?(@.optional)].name");
    println!("existence filter: {:?}", r);
    assert_eq!(r, "Alice");
}

#[test]
fn parse_lr_honors_case_insensitive_setting() {
    use crate::pipeline::block::ParseLRSettings;

    let mut ctx = ExecutionContext::new("parser-test".into());
    ctx.variables
        .set_data("SOURCE", "prefix <Token>value</TOKEN> suffix".into());
    ctx.execute_parse_lr(&ParseLRSettings {
        input_var: "data.SOURCE".into(),
        left: "<token>".into(),
        right: "</token>".into(),
        output_var: "RESULT".into(),
        capture: false,
        recursive: false,
        case_insensitive: true,
    })
    .unwrap();

    assert_eq!(ctx.variables.get("RESULT").as_deref(), Some("value"));
}

#[test]
fn parse_regex_honors_multi_line_setting() {
    use crate::pipeline::block::ParseRegexSettings;

    let mut ctx = ExecutionContext::new("parser-test".into());
    ctx.variables
        .set_data("SOURCE", "first\nsecond\nthird".into());
    ctx.execute_parse_regex(&ParseRegexSettings {
        input_var: "data.SOURCE".into(),
        pattern: "^second$".into(),
        output_format: "$0".into(),
        output_var: "RESULT".into(),
        capture: false,
        multi_line: true,
    })
    .unwrap();

    assert_eq!(ctx.variables.get("RESULT").as_deref(), Some("second"));
}

#[test]
fn parse_css_negative_one_collects_all_matches() {
    use crate::pipeline::block::ParseCSSSettings;

    let mut ctx = ExecutionContext::new("parser-test".into());
    ctx.variables
        .set_data("SOURCE", "<main><p>one</p><p>two</p></main>".into());
    ctx.execute_parse_css(&ParseCSSSettings {
        input_var: "data.SOURCE".into(),
        selector: "p".into(),
        attribute: "text".into(),
        output_var: "RESULT".into(),
        capture: false,
        index: -1,
    })
    .unwrap();

    assert_eq!(ctx.variables.get("RESULT").as_deref(), Some("one, two"));
}

#[test]
fn parse_lr_rejects_empty_delimiters_without_looping() {
    use crate::pipeline::block::ParseLRSettings;

    let mut ctx = ExecutionContext::new("parser-test".into());
    ctx.variables.set_data("SOURCE", "any source".into());
    let error = ctx
        .execute_parse_lr(&ParseLRSettings {
            input_var: "data.SOURCE".into(),
            left: String::new(),
            right: "]".into(),
            output_var: "RESULT".into(),
            capture: false,
            recursive: true,
            case_insensitive: false,
        })
        .unwrap_err();

    assert!(error.to_string().contains("non-empty"));
}

#[test]
fn parse_regex_expands_two_digit_capture_references() {
    use crate::pipeline::block::ParseRegexSettings;

    let mut ctx = ExecutionContext::new("parser-test".into());
    ctx.variables.set_data("SOURCE", "abcdefghij".into());
    ctx.execute_parse_regex(&ParseRegexSettings {
        input_var: "data.SOURCE".into(),
        pattern: "(a)(b)(c)(d)(e)(f)(g)(h)(i)(j)".into(),
        output_format: "$10:$1".into(),
        output_var: "RESULT".into(),
        capture: false,
        multi_line: false,
    })
    .unwrap();

    assert_eq!(ctx.variables.get("RESULT").as_deref(), Some("j:a"));
}
