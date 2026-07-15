use crate::pipeline::block::*;
use crate::pipeline::*;

use super::ImportResult;

// ────────────────────────────────────────────────────────────
// SVB (SilverBullet / OpenBullet 1) importer
// ────────────────────────────────────────────────────────────

/// Import a SilverBullet .svb config (text file with [SETTINGS] + [SCRIPT] sections)
pub(super) fn import_svb(content: &str) -> Result<ImportResult, String> {
    let mut pipeline = Pipeline::default();
    let mut warnings = Vec::new();

    // Split into [SETTINGS] and [SCRIPT] sections
    let script_start = content.find("[SCRIPT]");
    let settings_str = if let Some(script_pos) = script_start {
        let settings_block = &content[..script_pos];
        // Strip [SETTINGS] header
        settings_block
            .trim_start()
            .strip_prefix("[SETTINGS]")
            .unwrap_or(settings_block)
            .trim()
    } else {
        ""
    };
    let script_str = if let Some(script_pos) = script_start {
        &content[script_pos + 8..] // skip "[SCRIPT]"
    } else {
        return Err("No [SCRIPT] section found in .svb file".into());
    };

    // Parse settings JSON
    if !settings_str.is_empty() {
        apply_svb_settings(&mut pipeline, settings_str);
    }

    // Parse script
    pipeline.blocks = parse_svb_script(script_str, &mut warnings)?;

    if pipeline.blocks.is_empty() {
        return Err("No blocks found in SVB script".into());
    }

    Ok(ImportResult {
        pipeline,
        warnings,
        security_issues: Vec::new(),
    })
}

/// Map SVB settings JSON fields to Pipeline settings
pub(super) fn apply_svb_settings(pipeline: &mut Pipeline, json_str: &str) {
    let json: serde_json::Value = match serde_json::from_str(json_str) {
        Ok(v) => v,
        Err(_) => return,
    };

    if let Some(name) = json.get("Name").and_then(|v| v.as_str()) {
        pipeline.name = name.trim().to_string();
    }
    if let Some(author) = json.get("Author").and_then(|v| v.as_str()) {
        if !author.is_empty() {
            pipeline.author = author.to_string();
        }
    }
    if let Some(bots) = json.get("SuggestedBots").and_then(|v| v.as_u64()) {
        if bots > 0 {
            pipeline.runner_settings.threads = bots as u32;
        }
    }
    if json
        .get("NeedsProxies")
        .and_then(|v| v.as_bool())
        .unwrap_or(false)
    {
        pipeline.proxy_settings.proxy_mode = ProxyMode::Rotate;
    }

    // Wordlist type → data slices
    let wl = json
        .get("AllowedWordlist1")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    match wl {
        "MailPass" | "Credentials" | "" => {
            pipeline.data_settings.separator = ':';
            pipeline.data_settings.slices = vec!["USER".into(), "PASS".into()];
        }
        "Emails" => {
            pipeline.data_settings.separator = ':';
            pipeline.data_settings.slices = vec!["EMAIL".into()];
        }
        _ => {
            pipeline.data_settings.wordlist_type = wl.to_string();
        }
    }
}

/// Parse the [SCRIPT] section of a SVB file into blocks
pub(super) fn parse_svb_script(
    content: &str,
    warnings: &mut Vec<String>,
) -> Result<Vec<Block>, String> {
    let lines: Vec<&str> = content.lines().collect();
    let mut blocks = Vec::new();
    let mut i = 0;

    while i < lines.len() {
        let raw = lines[i];
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            i += 1;
            continue;
        }

        // Skip indented lines that belong to a previous block (shouldn't happen at top level)
        if raw.starts_with(' ') || raw.starts_with('\t') {
            i += 1;
            continue;
        }

        // Parse optional #label or !#label prefix
        let (label, disabled, cmd) = parse_svb_prefix(trimmed);

        if cmd.starts_with("REQUEST ") {
            let (block, next_i) = parse_svb_request(&label, disabled, cmd, &lines, i + 1);
            blocks.push(block);
            i = next_i;
        } else if cmd == "KEYCHECK" || cmd.starts_with("KEYCHECK ") {
            let (block, next_i) = parse_svb_keycheck(&label, disabled, &lines, i + 1);
            blocks.push(block);
            i = next_i;
        } else if cmd.starts_with("PARSE ") {
            if let Some(block) = parse_svb_parse_line(&label, disabled, cmd) {
                blocks.push(block);
            }
            i += 1;
        } else if cmd.starts_with("FUNCTION ") {
            let (block_opt, next_i) =
                parse_svb_function(&label, disabled, cmd, &lines, i + 1, warnings);
            if let Some(block) = block_opt {
                blocks.push(block);
            }
            i = next_i;
        } else if cmd.starts_with("IF ") {
            let (block, next_i) = parse_svb_if(cmd, &lines, i + 1);
            blocks.push(block);
            i = next_i;
        } else if cmd.starts_with("SET ") {
            if let Some(block) = parse_svb_set(&label, disabled, cmd) {
                blocks.push(block);
            }
            i += 1;
        } else if cmd.starts_with("UTILITY ") {
            blocks.push(parse_svb_utility(&label, disabled, cmd));
            i += 1;
        } else {
            // Unknown top-level command
            let cmd_word = cmd.split_whitespace().next().unwrap_or(cmd);
            if cmd_word != "ELSE" && cmd_word != "ENDIF" {
                warnings.push(format!("Unknown SVB command: {}", cmd_word));
            }
            i += 1;
        }
    }

    Ok(blocks)
}

// ────────────────────────────────────────────────────────────
// SVB line prefix parser
// ────────────────────────────────────────────────────────────

/// Parse SVB line prefix: `#LABEL cmd...` or `!#LABEL cmd...` or plain `cmd...`
pub(super) fn parse_svb_prefix(line: &str) -> (String, bool, &str) {
    if line.starts_with("!#") {
        let rest = &line[2..];
        if let Some(space_pos) = rest.find(' ') {
            let label = rest[..space_pos].to_string();
            let cmd = rest[space_pos + 1..].trim();
            (label, true, cmd)
        } else {
            (rest.to_string(), true, "")
        }
    } else if line.starts_with('#') {
        let rest = &line[1..];
        if let Some(space_pos) = rest.find(' ') {
            let label = rest[..space_pos].to_string();
            let cmd = rest[space_pos + 1..].trim();
            (label, false, cmd)
        } else {
            (rest.to_string(), false, "")
        }
    } else {
        (String::new(), false, line)
    }
}

// ────────────────────────────────────────────────────────────
// SVB REQUEST parser
// ────────────────────────────────────────────────────────────

fn parse_svb_request(
    label: &str,
    disabled: bool,
    cmd: &str,
    lines: &[&str],
    start: usize,
) -> (Block, usize) {
    let mut block = Block::new(BlockType::HttpRequest);
    if !label.is_empty() {
        block.label = label.to_string();
    }
    block.disabled = disabled;

    // Parse "REQUEST METHOD "url" [AutoRedirect=FALSE]"
    let after_request = &cmd[8..]; // skip "REQUEST "
    let (method, rest) = after_request.split_once(' ').unwrap_or((after_request, ""));
    let (url_raw, _opts) = svb_extract_quoted(rest.trim());
    let auto_redirect = !cmd.contains("AutoRedirect=FALSE");

    if let BlockSettings::HttpRequest(ref mut s) = block.settings {
        s.method = method.to_string();
        s.url = convert_svb_var_refs(&url_raw);
        if !auto_redirect {
            s.follow_redirects = false;
            s.auto_redirect = false;
        }
    }

    // Collect following indented lines: CONTENT, CONTENTTYPE, HEADER
    let mut i = start;
    let mut headers: Vec<(String, String)> = Vec::new();
    let mut body = String::new();
    let mut content_type = String::new();

    while i < lines.len() {
        let raw = lines[i];
        let trimmed = raw.trim();

        if trimmed.is_empty() {
            // Check if next non-empty line is indented (still part of this block)
            let mut j = i + 1;
            while j < lines.len() && lines[j].trim().is_empty() {
                j += 1;
            }
            if j < lines.len() && (lines[j].starts_with(' ') || lines[j].starts_with('\t')) {
                i += 1;
                continue;
            }
            // Blank line followed by non-indented = end of block
            i += 1;
            break;
        }

        // Non-indented line = new command
        if !raw.starts_with(' ') && !raw.starts_with('\t') {
            break;
        }

        if trimmed.starts_with("CONTENT ") {
            let (val, _) = svb_extract_quoted(&trimmed[8..]);
            body = convert_svb_var_refs(&val);
        } else if trimmed.starts_with("CONTENTTYPE ") {
            let (val, _) = svb_extract_quoted(&trimmed[12..]);
            content_type = val;
        } else if trimmed.starts_with("HEADER ") {
            let (val, _) = svb_extract_quoted(&trimmed[7..]);
            // Header format: "Key: Value" (split on first ": ")
            if let Some((k, v)) = val.split_once(": ") {
                headers.push((k.to_string(), convert_svb_var_refs(v)));
            } else if let Some((k, v)) = val.split_once(':') {
                headers.push((k.to_string(), convert_svb_var_refs(v.trim())));
            }
        }
        // Ignore unknown indented lines (Content-Length, etc.)

        i += 1;
    }

    if let BlockSettings::HttpRequest(ref mut s) = block.settings {
        if !body.is_empty() {
            s.body = body;
            s.body_type = BodyType::Standard;
        }
        if !content_type.is_empty() {
            s.content_type = content_type;
        }
        if !headers.is_empty() {
            s.headers = headers;
        }
    }

    (block, i)
}

// ────────────────────────────────────────────────────────────
// SVB KEYCHECK parser
// ────────────────────────────────────────────────────────────

fn parse_svb_keycheck(label: &str, disabled: bool, lines: &[&str], start: usize) -> (Block, usize) {
    let mut block = Block::new(BlockType::KeyCheck);
    if !label.is_empty() {
        block.label = label.to_string();
    }
    block.disabled = disabled;

    if let BlockSettings::KeyCheck(ref mut s) = block.settings {
        s.keychains.clear();
        let mut current_keychain: Option<Keychain> = None;

        let mut i = start;
        while i < lines.len() {
            let raw = lines[i];
            let trimmed = raw.trim();

            if trimmed.is_empty() {
                // Check if next non-empty line is indented
                let mut j = i + 1;
                while j < lines.len() && lines[j].trim().is_empty() {
                    j += 1;
                }
                if j < lines.len() && (lines[j].starts_with(' ') || lines[j].starts_with('\t')) {
                    i += 1;
                    continue;
                }
                i += 1;
                break;
            }

            if !raw.starts_with(' ') && !raw.starts_with('\t') {
                break;
            }

            if trimmed.starts_with("KEYCHAIN ") {
                // Save previous keychain
                if let Some(kc) = current_keychain.take() {
                    if !kc.conditions.is_empty() {
                        s.keychains.push(kc);
                    }
                }
                // Parse: "KEYCHAIN Status [Label] OR"
                let parts: Vec<&str> = trimmed.split_whitespace().collect();
                let status = match parts.get(1).copied().unwrap_or("") {
                    "Success" => BotStatus::Success,
                    "Failure" => BotStatus::Fail,
                    "Ban" => BotStatus::Ban,
                    "Retry" => BotStatus::Retry,
                    "Custom" => BotStatus::Custom,
                    _ => BotStatus::None,
                };
                current_keychain = Some(Keychain {
                    result: status,
                    conditions: Vec::new(),
                    mode: crate::pipeline::block::settings_check::KeychainMode::And,
                });
            } else if trimmed.starts_with("KEY ") {
                if let Some(ref mut kc) = current_keychain {
                    if let Some(cond) = parse_svb_key(&trimmed[4..]) {
                        kc.conditions.push(cond);
                    }
                }
            }

            i += 1;
        }

        if let Some(kc) = current_keychain {
            if !kc.conditions.is_empty() {
                s.keychains.push(kc);
            }
        }

        return (block, i);
    }

    (block, start)
}

/// Parse SVB KEY condition: `"value"` or `"<VAR>" Comparison "value"`
pub(super) fn parse_svb_key(after_key: &str) -> Option<KeyCondition> {
    let after = after_key.trim();

    // Extract first quoted string
    let (first_val, rest) = svb_extract_quoted(after);
    if first_val.is_empty() && !after.starts_with('"') {
        return None;
    }
    let rest = rest.trim();

    // If rest is empty, it's a simple KEY "value" → Contains against SOURCE
    if rest.is_empty() {
        return Some(KeyCondition {
            source: "data.SOURCE".to_string(),
            comparison: Comparison::Contains,
            value: first_val,
        });
    }

    // Check if first_val is a <VAR> reference
    if first_val.starts_with('<') && first_val.ends_with('>') {
        let var_name = &first_val[1..first_val.len() - 1];
        let source = convert_svb_source_name(var_name);

        // Parse comparison and value
        let (comparison_str, value_part) = rest.split_once(' ').unwrap_or((rest, ""));
        let comparison = match comparison_str {
            "Contains" => Comparison::Contains,
            "DoesNotContain" => Comparison::NotContains,
            "EqualTo" | "Is" => Comparison::EqualTo,
            "NotEqualTo" | "IsNot" => Comparison::NotEqualTo,
            "MatchesRegex" => Comparison::MatchesRegex,
            "GreaterThan" => Comparison::GreaterThan,
            "LessThan" | "LessThanOrEqual" => Comparison::LessThan,
            "Exists" => Comparison::Exists,
            "DoesNotExist" => Comparison::NotExists,
            _ => Comparison::Contains,
        };
        let (value, _) = svb_extract_quoted(value_part.trim());
        return Some(KeyCondition {
            source,
            comparison,
            value,
        });
    }

    // Not a var ref — simple KEY "value" with trailing junk
    Some(KeyCondition {
        source: "data.SOURCE".to_string(),
        comparison: Comparison::Contains,
        value: first_val,
    })
}

// ────────────────────────────────────────────────────────────
// SVB PARSE parser (single-line)
// ────────────────────────────────────────────────────────────

fn parse_svb_parse_line(label: &str, disabled: bool, cmd: &str) -> Option<Block> {
    let after = cmd.strip_prefix("PARSE ")?.trim();

    // Extract source (first quoted string)
    let (source_raw, rest) = svb_extract_quoted(after);
    let source = convert_svb_source_ref(&source_raw);
    let rest = rest.trim();

    if rest.starts_with("LR ") {
        let rest = &rest[3..];
        let (left, rest) = svb_extract_quoted(rest.trim());
        let (right, rest) = svb_extract_quoted(rest.trim());
        let (is_capture, output_var) = parse_svb_output(rest);

        let mut block = Block::new(BlockType::ParseLR);
        if !label.is_empty() {
            block.label = label.to_string();
        }
        block.disabled = disabled;
        if let BlockSettings::ParseLR(ref mut s) = block.settings {
            s.input_var = source;
            s.left = left;
            s.right = right;
            s.output_var = output_var;
            s.capture = is_capture;
        }
        Some(block)
    } else if rest.starts_with("JSON ") {
        let rest = &rest[5..];
        let (json_path, rest) = svb_extract_quoted(rest.trim());
        let (is_capture, output_var) = parse_svb_output(rest);

        let mut block = Block::new(BlockType::ParseJSON);
        if !label.is_empty() {
            block.label = label.to_string();
        }
        block.disabled = disabled;
        if let BlockSettings::ParseJSON(ref mut s) = block.settings {
            s.input_var = source;
            s.json_path = json_path;
            s.output_var = output_var;
            s.capture = is_capture;
        }
        Some(block)
    } else {
        // Unknown parse mode → Script
        let mut block = Block::new(BlockType::Script);
        block.label = if label.is_empty() {
            "Parse".into()
        } else {
            label.to_string()
        };
        block.disabled = disabled;
        if let BlockSettings::Script(ref mut s) = block.settings {
            s.code = format!("// SVB PARSE with unknown mode\n// {}", cmd);
        }
        Some(block)
    }
}

// ────────────────────────────────────────────────────────────
// SVB FUNCTION parser
// ────────────────────────────────────────────────────────────

fn parse_svb_function(
    label: &str,
    disabled: bool,
    cmd: &str,
    lines: &[&str],
    start: usize,
    warnings: &mut Vec<String>,
) -> (Option<Block>, usize) {
    let after = cmd.strip_prefix("FUNCTION ").unwrap_or("").trim();

    // GetRandomUA [BROWSER IOS] -> VAR "name"
    if after.starts_with("GetRandomUA") {
        let (_, output_var) = parse_svb_output(after);
        let mut block = Block::new(BlockType::RandomUserAgent);
        block.label = if label.is_empty() {
            "Random UA".into()
        } else {
            label.to_string()
        };
        block.disabled = disabled;
        if let BlockSettings::RandomUserAgent(ref mut s) = block.settings {
            s.output_var = output_var;
        }
        return (Some(block), start);
    }

    // RandomString "pattern" -> VAR "name"
    if after.starts_with("RandomString ") {
        let rest = &after[13..];
        let (pattern, rest) = svb_extract_quoted(rest.trim());
        let (_, output_var) = parse_svb_output(rest);

        let mut block = Block::new(BlockType::RandomData);
        block.label = if label.is_empty() {
            "Random String".into()
        } else {
            label.to_string()
        };
        block.disabled = disabled;
        if let BlockSettings::RandomData(ref mut s) = block.settings {
            s.data_type = RandomDataType::String;
            s.output_var = output_var;
            // Count pattern chars: ?u = upper, ?l = lower, ?d = digit, ?h = hex
            let char_count = pattern.matches('?').count();
            s.string_length = if char_count > 0 {
                char_count as u32
            } else {
                pattern.len() as u32
            };
            if pattern.contains("?h") {
                s.string_charset = "custom".into();
                s.custom_chars = "0123456789abcdef".into();
            } else {
                s.string_charset = "alphanumeric".into();
            }
        }
        return (Some(block), start);
    }

    // Constant "value" -> VAR/CAP "name"
    if after.starts_with("Constant ") {
        let rest = &after[9..];
        let (value, rest) = svb_extract_quoted(rest.trim());
        let (is_capture, output_var) = parse_svb_output(rest);

        let mut block = Block::new(BlockType::SetVariable);
        block.label = if label.is_empty() {
            "Constant".into()
        } else {
            label.to_string()
        };
        block.disabled = disabled;
        if let BlockSettings::SetVariable(ref mut s) = block.settings {
            s.name = output_var;
            s.value = convert_svb_var_refs(&value);
            s.capture = is_capture;
        }
        return (Some(block), start);
    }

    // ToUppercase/ToLowercase "input" -> VAR "name"
    if after.starts_with("ToUppercase ") || after.starts_with("ToLowercase ") {
        let is_upper = after.starts_with("ToUppercase");
        let rest = if is_upper { &after[12..] } else { &after[12..] };
        let (input_raw, rest) = svb_extract_quoted(rest.trim());
        let (is_capture, output_var) = parse_svb_output(rest);
        let input_var = extract_svb_simple_var(&input_raw);

        let fn_type = if is_upper {
            StringFnType::ToUpper
        } else {
            StringFnType::ToLower
        };
        let mut block = Block::new(BlockType::StringFunction);
        block.label = if label.is_empty() {
            format!("{:?}", fn_type)
        } else {
            label.to_string()
        };
        block.disabled = disabled;
        if let BlockSettings::StringFunction(ref mut s) = block.settings {
            s.function_type = fn_type;
            s.input_var = input_var;
            s.output_var = output_var;
            s.capture = is_capture;
        }
        return (Some(block), start);
    }

    // Replace "what" "with" "input" -> VAR/CAP "name"
    if after.starts_with("Replace ") {
        let rest = &after[8..];
        let (what, rest) = svb_extract_quoted(rest.trim());
        let (with, rest) = svb_extract_quoted(rest.trim());
        let (input_raw, rest) = svb_extract_quoted(rest.trim());
        let (is_capture, output_var) = parse_svb_output(rest);
        let input_var = extract_svb_simple_var(&input_raw);

        let mut block = Block::new(BlockType::StringFunction);
        block.label = if label.is_empty() {
            "Replace".into()
        } else {
            label.to_string()
        };
        block.disabled = disabled;
        if let BlockSettings::StringFunction(ref mut s) = block.settings {
            s.function_type = StringFnType::Replace;
            s.input_var = input_var;
            s.output_var = output_var;
            s.capture = is_capture;
            s.param1 = what;
            s.param2 = with;
        }
        return (Some(block), start);
    }

    // Split "separator" index "input" -> VAR "name"
    if after.starts_with("Split ") {
        let rest = &after[6..];
        let (sep, rest) = svb_extract_quoted(rest.trim());
        let rest = rest.trim();
        // Index is an unquoted number
        let (index_str, rest) = rest.split_once(' ').unwrap_or((rest, ""));
        let (input_raw, rest) = svb_extract_quoted(rest.trim());
        let (_, output_var) = parse_svb_output(rest);
        let input_var = extract_svb_simple_var(&input_raw);

        let mut block = Block::new(BlockType::Script);
        block.label = if label.is_empty() {
            "Split".into()
        } else {
            label.to_string()
        };
        block.disabled = disabled;
        if let BlockSettings::Script(ref mut s) = block.settings {
            s.code = format!(
                "// SVB Split: split <{}> by \"{}\" take index {}, store in {}\n// Implement in pipeline logic",
                input_var, sep, index_str, output_var
            );
        }
        return (Some(block), start);
    }

    // Unescape "input" -> VAR/CAP "name"
    if after.starts_with("Unescape ") {
        let rest = &after[9..];
        let (input_raw, rest) = svb_extract_quoted(rest.trim());
        let (is_capture, output_var) = parse_svb_output(rest);
        let input_var = extract_svb_simple_var(&input_raw);

        let mut block = Block::new(BlockType::Script);
        block.label = if label.is_empty() {
            "Unescape".into()
        } else {
            label.to_string()
        };
        block.disabled = disabled;
        if let BlockSettings::Script(ref mut s) = block.settings {
            s.code = format!(
                "// SVB Unescape: unescape <{}>, store as {} (capture={})\n// Implement HTML/Unicode unescape",
                input_var, output_var, is_capture
            );
        }
        return (Some(block), start);
    }

    // UnixTimeToDate "format" "input" -> VAR/CAP "name"
    if after.starts_with("UnixTimeToDate ") {
        let rest = &after[15..];
        let (format_str, rest) = svb_extract_quoted(rest.trim());
        let (input_raw, rest) = svb_extract_quoted(rest.trim());
        let (is_capture, output_var) = parse_svb_output(rest);
        let input_var = extract_svb_simple_var(&input_raw);

        let mut block = Block::new(BlockType::Script);
        block.label = if label.is_empty() {
            "Unix Time to Date".into()
        } else {
            label.to_string()
        };
        block.disabled = disabled;
        if let BlockSettings::Script(ref mut s) = block.settings {
            s.code = format!(
                "// SVB UnixTimeToDate: convert <{}> with format \"{}\" → {} (capture={})",
                input_var, format_str, output_var, is_capture
            );
        }
        return (Some(block), start);
    }

    // Translate (multi-line with KEY/VALUE pairs)
    if after.starts_with("Translate") {
        return parse_svb_translate(label, disabled, lines, start, warnings);
    }

    // Unknown function type
    let fn_name = after.split_whitespace().next().unwrap_or("unknown");
    warnings.push(format!("Unknown SVB function: {}", fn_name));
    let mut block = Block::new(BlockType::Script);
    block.label = if label.is_empty() {
        format!("SVB: {}", fn_name)
    } else {
        label.to_string()
    };
    block.disabled = disabled;
    if let BlockSettings::Script(ref mut s) = block.settings {
        s.code = format!("// Unknown SVB FUNCTION: {}", after);
    }
    (Some(block), start)
}

/// Parse SVB FUNCTION Translate (multi-line KEY/VALUE lookup table)
fn parse_svb_translate(
    label: &str,
    disabled: bool,
    lines: &[&str],
    start: usize,
    _warnings: &mut Vec<String>,
) -> (Option<Block>, usize) {
    let mut i = start;
    let mut entries = Vec::new();
    let mut input_var = String::new();
    let mut output_var = "RESULT".to_string();

    // Collect KEY "k" VALUE "v" lines and the final "input" -> VAR "name" line
    while i < lines.len() {
        let raw = lines[i];
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            let mut j = i + 1;
            while j < lines.len() && lines[j].trim().is_empty() {
                j += 1;
            }
            if j < lines.len() && (lines[j].starts_with(' ') || lines[j].starts_with('\t')) {
                i += 1;
                continue;
            }
            i += 1;
            break;
        }
        if !raw.starts_with(' ') && !raw.starts_with('\t') {
            break;
        }

        if trimmed.starts_with("KEY ") {
            // KEY "k" VALUE "v"
            let rest = &trimmed[4..];
            let (key, rest) = svb_extract_quoted(rest.trim());
            let rest = rest.trim();
            if rest.starts_with("VALUE ") {
                let (val, _) = svb_extract_quoted(&rest[6..]);
                entries.push((key, val));
            }
        } else if trimmed.contains("-> VAR ") || trimmed.contains("-> CAP ") {
            // Final line: "input" -> VAR "name"
            let (input_raw, _) = svb_extract_quoted(trimmed);
            input_var = extract_svb_simple_var(&input_raw);
            let (_, out) = parse_svb_output(trimmed);
            output_var = out;
            i += 1;
            break;
        }

        i += 1;
    }

    let mut block = Block::new(BlockType::Translate);
    block.label = if label.is_empty() {
        "Translate".into()
    } else {
        label.to_string()
    };
    block.disabled = disabled;
    if let BlockSettings::Translate(ref mut s) = block.settings {
        s.input_var = input_var;
        s.output_var = output_var;
        s.mappings = entries
            .iter()
            .map(|(key, value)| format!("{} => {}", key, value))
            .collect::<Vec<_>>()
            .join("\n");
    }

    (Some(block), i)
}

// ────────────────────────────────────────────────────────────
// SVB IF/ELSE/ENDIF parser
// ────────────────────────────────────────────────────────────

fn parse_svb_if(cmd: &str, lines: &[&str], start: usize) -> (Block, usize) {
    // Parse: IF "<VAR>" CONTAINS "value"
    let mut block = Block::new(BlockType::IfElse);
    block.label = "If / Else".to_string();

    // Extract condition from the IF line
    let after_if = cmd.strip_prefix("IF ").unwrap_or("").trim();
    let (source_raw, rest) = svb_extract_quoted(after_if);
    let source_var = extract_svb_simple_var(&source_raw);
    let rest = rest.trim();
    let (comparison_str, value_part) = rest.split_once(' ').unwrap_or((rest, ""));
    let comparison = match comparison_str {
        "CONTAINS" | "Contains" => Comparison::Contains,
        "DOESNOTCONTAIN" | "DoesNotContain" => Comparison::NotContains,
        "EQUALTO" | "EqualTo" | "IS" => Comparison::EqualTo,
        "NOTEQUALTO" | "NotEqualTo" => Comparison::NotEqualTo,
        _ => Comparison::Contains,
    };
    let (value, _) = svb_extract_quoted(value_part.trim());

    if let BlockSettings::IfElse(ref mut s) = block.settings {
        s.condition = KeyCondition {
            source: source_var,
            comparison,
            value,
        };
    }

    // Collect true/false branch lines (as inner blocks)
    let mut true_blocks = Vec::new();
    let mut false_blocks = Vec::new();
    let mut in_else = false;
    let mut i = start;

    while i < lines.len() {
        let trimmed = lines[i].trim();

        // Parse prefix for inner lines too
        let (inner_label, inner_disabled, inner_cmd) = parse_svb_prefix(trimmed);

        if inner_cmd == "ENDIF" {
            i += 1;
            break;
        }
        if inner_cmd == "ELSE" {
            in_else = true;
            i += 1;
            continue;
        }
        if trimmed.is_empty() {
            i += 1;
            continue;
        }

        // Parse inner command as a block
        if inner_cmd.starts_with("SET ") {
            if let Some(inner_block) = parse_svb_set(&inner_label, inner_disabled, inner_cmd) {
                if in_else {
                    false_blocks.push(inner_block);
                } else {
                    true_blocks.push(inner_block);
                }
            }
        } else if inner_cmd.starts_with("FUNCTION ") {
            // Simple inline function (no multi-line in IF blocks)
            let mut dummy_warnings = Vec::new();
            let (block_opt, _) = parse_svb_function(
                &inner_label,
                inner_disabled,
                inner_cmd,
                lines,
                i + 1,
                &mut dummy_warnings,
            );
            if let Some(inner_block) = block_opt {
                if in_else {
                    false_blocks.push(inner_block);
                } else {
                    true_blocks.push(inner_block);
                }
            }
        } else {
            // Generic inner content → Script
            let mut inner_block = Block::new(BlockType::Script);
            inner_block.label = if inner_label.is_empty() {
                "Script".into()
            } else {
                inner_label
            };
            inner_block.disabled = inner_disabled;
            if let BlockSettings::Script(ref mut s) = inner_block.settings {
                s.code = format!("// {}", inner_cmd);
            }
            if in_else {
                false_blocks.push(inner_block);
            } else {
                true_blocks.push(inner_block);
            }
        }

        i += 1;
    }

    if let BlockSettings::IfElse(ref mut s) = block.settings {
        s.true_blocks = true_blocks;
        s.false_blocks = false_blocks;
    }

    (block, i)
}

// ────────────────────────────────────────────────────────────
// SVB SET parser
// ────────────────────────────────────────────────────────────

/// Parse: SET CAP "name" VAR "value" or SET VAR "name" VAR "value"
fn parse_svb_set(label: &str, disabled: bool, cmd: &str) -> Option<Block> {
    let after = cmd.strip_prefix("SET ")?.trim();

    let is_capture = after.starts_with("CAP ");
    let rest = if is_capture {
        &after[4..]
    } else if after.starts_with("VAR ") {
        &after[4..]
    } else {
        after
    };

    let (name, rest) = svb_extract_quoted(rest.trim());
    let rest = rest.trim();
    // Skip "VAR" keyword if present
    let rest = rest.strip_prefix("VAR ").unwrap_or(rest).trim();
    let (value, _) = svb_extract_quoted(rest);

    let mut block = Block::new(BlockType::SetVariable);
    block.label = if label.is_empty() {
        "Set Variable".into()
    } else {
        label.to_string()
    };
    block.disabled = disabled;
    if let BlockSettings::SetVariable(ref mut s) = block.settings {
        s.name = name;
        s.value = convert_svb_var_refs(&value);
        s.capture = is_capture;
    }
    Some(block)
}

// ────────────────────────────────────────────────────────────
// SVB UTILITY parser
// ────────────────────────────────────────────────────────────

fn parse_svb_utility(label: &str, disabled: bool, cmd: &str) -> Block {
    let mut block = Block::new(BlockType::Script);
    block.label = if label.is_empty() {
        "Utility".into()
    } else {
        label.to_string()
    };
    block.disabled = disabled;
    if let BlockSettings::Script(ref mut s) = block.settings {
        s.code = format!("// SVB UTILITY — review and implement manually\n// {}", cmd);
    }
    block
}

// ────────────────────────────────────────────────────────────
// SVB helpers
// ────────────────────────────────────────────────────────────

/// Extract next quoted string from SVB text. Returns (extracted_string, rest_of_input)
pub(super) fn svb_extract_quoted(s: &str) -> (String, &str) {
    let s = s.trim();
    if !s.starts_with('"') {
        return (String::new(), s);
    }
    let bytes = s.as_bytes();
    let mut i = 1;
    while i < bytes.len() {
        if bytes[i] == b'\\' && i + 1 < bytes.len() {
            i += 2;
            continue;
        }
        if bytes[i] == b'"' {
            let inner = &s[1..i];
            let rest = &s[i + 1..];
            return (inner.replace("\\\"", "\""), rest);
        }
        i += 1;
    }
    // No closing quote
    (s[1..].to_string(), "")
}

/// Convert SVB source name to ironbullet variable name
/// SOURCE → data.SOURCE, RESPONSECODE → data.RESPONSECODE, other → as-is
pub(super) fn convert_svb_source_name(name: &str) -> String {
    match name {
        "SOURCE" => "data.SOURCE".to_string(),
        "RESPONSECODE" => "data.RESPONSECODE".to_string(),
        "ADDRESS" => "data.ADDRESS".to_string(),
        _ => name.to_string(),
    }
}

/// Convert SVB source reference (from PARSE) to ironbullet variable name
/// `<SOURCE>` → data.SOURCE, `<COOKIES(name)>` → data.COOKIES["name"], `<varName>` → varName
pub(super) fn convert_svb_source_ref(source: &str) -> String {
    if source == "<SOURCE>" {
        return "data.SOURCE".to_string();
    }
    if source == "<RESPONSECODE>" {
        return "data.RESPONSECODE".to_string();
    }

    // <COOKIES(name)> → data.COOKIES["name"]
    if source.starts_with("<COOKIES(") && source.ends_with(")>") {
        let name = &source[9..source.len() - 2];
        return format!("data.COOKIES[\"{}\"]", name);
    }

    // <HEADERS(name)> → data.HEADERS["name"]
    if source.starts_with("<HEADERS(") && source.ends_with(")>") {
        let name = &source[9..source.len() - 2];
        return format!("data.HEADERS[\"{}\"]", name);
    }

    // <varName> → varName
    if source.starts_with('<') && source.ends_with('>') {
        return source[1..source.len() - 1].to_string();
    }

    source.to_string()
}

/// Convert SVB data variable references: <USER> → <input.USER>, <PASS> → <input.PASS>
pub(super) fn convert_svb_var_refs(text: &str) -> String {
    text.replace("<USER>", "<input.USER>")
        .replace("<PASS>", "<input.PASS>")
        .replace("<EMAIL>", "<input.EMAIL>")
}

/// Extract simple variable name from SVB input string
/// `<varName>` → varName, `<input.USER>` → input.USER, other → as-is
pub(super) fn extract_svb_simple_var(input: &str) -> String {
    let input = convert_svb_var_refs(input);
    if input.starts_with('<') && input.ends_with('>') && !input[1..input.len() - 1].contains('<') {
        input[1..input.len() - 1].to_string()
    } else {
        input
    }
}

/// Parse SVB output: `-> VAR "name"` or `-> CAP "name"` from a string
/// Returns (is_capture, variable_name)
pub(super) fn parse_svb_output(s: &str) -> (bool, String) {
    if let Some(pos) = s.find("-> VAR ") {
        let rest = &s[pos + 7..];
        let (name, _) = svb_extract_quoted(rest.trim());
        return (
            false,
            if name.is_empty() {
                "RESULT".into()
            } else {
                name
            },
        );
    }
    if let Some(pos) = s.find("-> CAP ") {
        let rest = &s[pos + 7..];
        let (name, _) = svb_extract_quoted(rest.trim());
        return (
            true,
            if name.is_empty() {
                "RESULT".into()
            } else {
                name
            },
        );
    }
    (false, "RESULT".to_string())
}
