use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use super::random_data;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Variable {
    pub name: String,
    pub value: VariableValue,
    pub is_capture: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum VariableValue {
    String(String),
    List(Vec<String>),
    Int(i64),
    Float(f64),
    Bool(bool),
}

impl VariableValue {
    pub fn as_str(&self) -> String {
        match self {
            Self::String(s) => s.clone(),
            Self::List(l) => l.join(", "),
            Self::Int(i) => i.to_string(),
            Self::Float(f) => f.to_string(),
            Self::Bool(b) => b.to_string(),
        }
    }

    pub fn as_list(&self) -> Vec<String> {
        match self {
            Self::List(l) => l.clone(),
            Self::String(s) => vec![s.clone()],
            _ => vec![self.as_str()],
        }
    }
}

impl From<String> for VariableValue {
    fn from(s: String) -> Self {
        Self::String(s)
    }
}

impl From<&str> for VariableValue {
    fn from(s: &str) -> Self {
        Self::String(s.to_string())
    }
}

impl From<Vec<String>> for VariableValue {
    fn from(l: Vec<String>) -> Self {
        Self::List(l)
    }
}

impl From<i64> for VariableValue {
    fn from(i: i64) -> Self {
        Self::Int(i)
    }
}

/// Variable store with scoped access.
/// Scopes: input.*, data.*, globals.*, @ (user vars)
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct VariableStore {
    pub input: HashMap<String, String>,
    pub data: HashMap<String, String>,
    pub globals: HashMap<String, String>,
    pub user_vars: HashMap<String, Variable>,
}

impl VariableStore {
    pub fn new() -> Self {
        Self::default()
    }

    /// Resolve a variable reference like "input.EMAIL", "data.SOURCE", "@token"
    pub fn get(&self, key: &str) -> Option<String> {
        if let Some(rest) = key.strip_prefix("input.") {
            self.input.get(rest).cloned()
        } else if let Some(rest) = key.strip_prefix("data.") {
            // data. prefix: check data namespace first, fall back to user_vars.
            // ParseJSON/JwtToken/etc. write to user_vars; configs often use
            // "data.MYVAR" for these — fall back so both forms resolve correctly.
            self.data
                .get(rest)
                .cloned()
                .or_else(|| self.user_vars.get(rest).map(|v| v.value.as_str()))
        } else if let Some(rest) = key.strip_prefix("globals.") {
            self.globals.get(rest).cloned()
        } else {
            // Bare name: check user_vars first (ParseJSON output), then data namespace.
            // HTTP blocks store RESPONSECODE/LASTRESPONSE/SOURCE via set_data —
            // a bare "RESPONSECODE" should still resolve for convenience.
            self.user_vars
                .get(key)
                .map(|v| v.value.as_str())
                .or_else(|| self.data.get(key).cloned())
        }
    }

    pub fn set_user(&mut self, name: &str, value: String, is_capture: bool) {
        self.user_vars.insert(
            name.to_string(),
            Variable {
                name: name.to_string(),
                value: VariableValue::String(value),
                is_capture,
            },
        );
    }

    pub fn set_data(&mut self, key: &str, value: String) {
        self.data.insert(key.to_string(), value);
    }

    /// Clear a data variable and any children below its dotted namespace.
    /// HTTP response variables use a prefix (for example `RESPONSE.STATUS`), so
    /// a failed request must clear the full prior response tree rather than leave
    /// stale headers or cookies available to a later KeyCheck.
    pub fn clear_data_prefix(&mut self, prefix: &str) {
        let dotted_prefix = format!("{prefix}.");
        self.data
            .retain(|key, _| key != prefix && !key.starts_with(&dotted_prefix));
    }

    pub fn set_input(&mut self, key: &str, value: String) {
        self.input.insert(key.to_string(), value);
    }

    /// Interpolate a string with variable references: `<input.EMAIL>`, `<data.SOURCE>`, `<@token>`
    /// Format: any text with `<var_ref>` placeholders
    pub fn interpolate(&self, template: &str) -> String {
        let mut result = String::with_capacity(template.len());
        let mut chars = template.chars().peekable();

        while let Some(ch) = chars.next() {
            if ch == '<' {
                // Look for closing >
                let mut var_name = String::new();
                let mut found_close = false;
                for ch2 in chars.by_ref() {
                    if ch2 == '>' {
                        found_close = true;
                        break;
                    }
                    var_name.push(ch2);
                }
                if found_close && !var_name.is_empty() {
                    if let Some(val) = resolve_random_function(&var_name) {
                        result.push_str(&val);
                    } else if let Some(val) = self.get(&var_name) {
                        result.push_str(&val);
                    } else {
                        // Leave unresolved vars as-is
                        result.push('<');
                        result.push_str(&var_name);
                        result.push('>');
                    }
                } else {
                    result.push('<');
                    result.push_str(&var_name);
                }
            } else {
                result.push(ch);
            }
        }

        result
    }

    /// Resolve an input_var field: try variable lookup first, then interpolate as literal/template.
    /// This allows users to type either a variable name ("myvar") or a raw value ("12345")
    /// or a template with placeholders ("<myvar>_suffix").
    pub fn resolve_input(&self, input_var: &str) -> String {
        if let Some(val) = self.get(input_var) {
            val
        } else {
            self.interpolate(input_var)
        }
    }

    /// Get all captures (variables marked as CAP)
    pub fn captures(&self) -> HashMap<String, String> {
        self.user_vars
            .iter()
            .filter(|(_, v)| v.is_capture)
            .map(|(k, v)| (k.clone(), v.value.as_str()))
            .collect()
    }

    /// Snapshot of all variables for debug display
    pub fn snapshot(&self) -> HashMap<String, String> {
        let mut map = HashMap::new();
        for (k, v) in &self.input {
            map.insert(format!("input.{}", k), v.clone());
        }
        for (k, v) in &self.data {
            map.insert(format!("data.{}", k), v.clone());
        }
        for (k, v) in &self.globals {
            map.insert(format!("globals.{}", k), v.clone());
        }
        for (k, v) in &self.user_vars {
            map.insert(k.clone(), v.value.as_str());
        }
        map
    }
}

/// Resolve `<random.XXX>` inline function calls.
/// Returns Some(value) if the name matches a random function, None otherwise.
fn resolve_random_function(name: &str) -> Option<String> {
    if !name.starts_with("random.") {
        return None;
    }
    let rest = &name[7..]; // strip "random."
    match rest {
        "email" => Some(random_data::random_email()),
        "uuid" => Some(random_data::random_uuid()),
        "phone" => Some(random_data::random_phone()),
        "string" => Some(random_data::random_string(16, "alphanumeric", "")),
        "number" => Some(random_data::random_number(0, 100, false)),
        "name.first" => Some(random_data::random_first_name()),
        "name.last" => Some(random_data::random_last_name()),
        "name.full" => Some(random_data::random_full_name()),
        "address.street" => Some(random_data::random_street_address()),
        "address.city" => Some(random_data::random_city()),
        "address.state" => Some(random_data::random_state()),
        "address.zip" => Some(random_data::random_zip()),
        _ => {
            // Handle parameterized forms: random.string.32, random.number.1.100
            if let Some(len_str) = rest.strip_prefix("string.") {
                let len: usize = len_str.parse().unwrap_or(16);
                Some(random_data::random_string(len, "alphanumeric", ""))
            } else if let Some(params) = rest.strip_prefix("number.") {
                let parts: Vec<&str> = params.splitn(2, '.').collect();
                let min: i64 = parts.first().and_then(|s| s.parse().ok()).unwrap_or(0);
                let max: i64 = parts.get(1).and_then(|s| s.parse().ok()).unwrap_or(100);
                Some(random_data::random_number(min, max, false))
            } else {
                None
            }
        }
    }
}
