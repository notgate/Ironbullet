use super::*;
use helpers::{urldecoding, urlencoding};

/// Apply the first exact mapping in a `source => destination` table.
/// Invalid and comment lines are ignored; unmatched input is preserved.
fn translate_exact(input: &str, mappings: &str) -> String {
    for raw_line in mappings.lines() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((source, destination)) = line.split_once("=>") else {
            continue;
        };
        if input == source.trim() {
            return destination.trim().to_string();
        }
    }
    input.to_string()
}

impl ExecutionContext {
    pub(super) fn execute_keycheck(
        &mut self,
        settings: &KeyCheckSettings,
    ) -> crate::error::Result<()> {
        use crate::pipeline::block::settings_check::KeychainMode;
        for keychain in &settings.keychains {
            // An empty condition list has no meaningful predicate. In particular,
            // `Iterator::all` would otherwise make an empty AND keychain match
            // unconditionally and classify every input as the configured result.
            if keychain.conditions.is_empty() {
                continue;
            }
            let matched = match keychain.mode {
                KeychainMode::And => keychain
                    .conditions
                    .iter()
                    .all(|cond| self.evaluate_condition(cond)),
                KeychainMode::Or => keychain
                    .conditions
                    .iter()
                    .any(|cond| self.evaluate_condition(cond)),
            };
            if matched {
                self.status = keychain.result;
                break;
            }
        }
        Ok(())
    }

    pub(super) fn evaluate_condition(&self, cond: &KeyCondition) -> bool {
        // Try the source as-is first (handles data.X, input.X, bare var names).
        // If that misses, try stripping a "data." prefix and checking user_vars —
        // this is a common authoring mistake where users write "data.MYVAR" for a
        // ParseJSON output that lives in user_vars, not the data namespace.
        let source_val = self
            .variables
            .get(&cond.source)
            .or_else(|| {
                cond.source
                    .strip_prefix("data.")
                    .and_then(|bare| self.variables.get(bare))
            })
            .unwrap_or_default();
        let target = self.variables.interpolate(&cond.value);

        match cond.comparison {
            Comparison::Contains => source_val.contains(&target),
            Comparison::NotContains => !source_val.contains(&target),
            Comparison::EqualTo => source_val == target,
            Comparison::NotEqualTo => source_val != target,
            Comparison::MatchesRegex => regex::Regex::new(&target)
                .map(|re| re.is_match(&source_val))
                .unwrap_or(false),
            Comparison::GreaterThan => source_val
                .parse::<f64>()
                .ok()
                .zip(target.parse::<f64>().ok())
                .is_some_and(|(source, target)| source > target),
            Comparison::LessThan => source_val
                .parse::<f64>()
                .ok()
                .zip(target.parse::<f64>().ok())
                .is_some_and(|(source, target)| source < target),
            Comparison::Exists => !source_val.is_empty(),
            Comparison::NotExists => source_val.is_empty(),
        }
    }

    pub(super) fn execute_string_function(
        &mut self,
        settings: &StringFunctionSettings,
    ) -> crate::error::Result<()> {
        let input = self.variables.resolve_input(&settings.input_var);
        let param1 = self.variables.interpolate(&settings.param1);
        let param2 = self.variables.interpolate(&settings.param2);

        let result = match settings.function_type {
            StringFnType::Replace => input.replace(&param1, &param2),
            StringFnType::Substring => {
                let start: usize = param1.parse().unwrap_or(0);
                let len: usize = param2.parse().unwrap_or(input.len());
                input.chars().skip(start).take(len).collect()
            }
            StringFnType::Trim => input.trim().to_string(),
            StringFnType::ToUpper => input.to_uppercase(),
            StringFnType::ToLower => input.to_lowercase(),
            StringFnType::URLEncode => urlencoding(&input),
            StringFnType::URLDecode => urldecoding(&input),
            StringFnType::Base64Encode => {
                use base64::Engine;
                base64::engine::general_purpose::STANDARD.encode(input.as_bytes())
            }
            StringFnType::Base64Decode => {
                use base64::Engine;
                match base64::engine::general_purpose::STANDARD.decode(input.as_bytes()) {
                    Ok(bytes) => String::from_utf8_lossy(&bytes).to_string(),
                    Err(_) => String::new(),
                }
            }
            StringFnType::Split => {
                let parts: Vec<String> = input.split(&param1).map(|s| s.to_string()).collect();
                serde_json::to_string(&parts).unwrap_or_default()
            }
            StringFnType::RandomString => {
                let len: usize = param1.parse().unwrap_or(16);
                use rand::Rng;
                let mut rng = rand::thread_rng();
                (0..len)
                    .map(|_| {
                        let idx = rng.gen_range(0..36);
                        if idx < 10 {
                            (b'0' + idx) as char
                        } else {
                            (b'a' + idx - 10) as char
                        }
                    })
                    .collect()
            }
            StringFnType::Reverse => input.chars().rev().collect(),
            StringFnType::Length => input.len().to_string(),
            _ => input,
        };

        self.variables
            .set_user(&settings.output_var, result, settings.capture);
        Ok(())
    }

    pub(super) fn execute_translate(
        &mut self,
        settings: &TranslateSettings,
    ) -> crate::error::Result<()> {
        let input = self.variables.resolve_input(&settings.input_var);
        let mappings = self.variables.interpolate(&settings.mappings);
        let result = translate_exact(&input, &mappings);
        self.variables
            .set_user(&settings.output_var, result, settings.capture);
        Ok(())
    }

    pub(super) fn execute_crypto_function(
        &mut self,
        settings: &CryptoFunctionSettings,
    ) -> crate::error::Result<()> {
        let input = self.variables.resolve_input(&settings.input_var);
        let key = self.variables.interpolate(&settings.key);

        let result = match settings.function_type {
            CryptoFnType::MD5 => {
                use md5::Digest;
                format!("{:x}", md5::Md5::digest(input.as_bytes()))
            }
            CryptoFnType::SHA1 => {
                use sha1::Digest;
                format!("{:x}", sha1::Sha1::digest(input.as_bytes()))
            }
            CryptoFnType::SHA256 => {
                use sha2::{Digest, Sha256};
                format!("{:x}", Sha256::digest(input.as_bytes()))
            }
            CryptoFnType::SHA384 => {
                use sha2::{Digest, Sha384};
                format!("{:x}", Sha384::digest(input.as_bytes()))
            }
            CryptoFnType::SHA512 => {
                use sha2::{Digest, Sha512};
                format!("{:x}", Sha512::digest(input.as_bytes()))
            }
            CryptoFnType::CRC32 => {
                let crc = crc32fast::hash(input.as_bytes());
                format!("{:08x}", crc)
            }
            CryptoFnType::HMACSHA256 => {
                use hmac::{Hmac, Mac};
                type HmacSha256 = Hmac<sha2::Sha256>;
                let mut mac = HmacSha256::new_from_slice(key.as_bytes()).map_err(|e| {
                    crate::error::AppError::Pipeline(format!("HMAC key error: {}", e))
                })?;
                mac.update(input.as_bytes());
                format!("{:x}", mac.finalize().into_bytes())
            }
            CryptoFnType::HMACSHA512 => {
                use hmac::{Hmac, Mac};
                type HmacSha512 = Hmac<sha2::Sha512>;
                let mut mac = HmacSha512::new_from_slice(key.as_bytes()).map_err(|e| {
                    crate::error::AppError::Pipeline(format!("HMAC key error: {}", e))
                })?;
                mac.update(input.as_bytes());
                format!("{:x}", mac.finalize().into_bytes())
            }
            CryptoFnType::HMACMD5 => {
                use hmac::{Hmac, Mac};
                type HmacMd5 = Hmac<md5::Md5>;
                let mut mac = HmacMd5::new_from_slice(key.as_bytes()).map_err(|e| {
                    crate::error::AppError::Pipeline(format!("HMAC key error: {}", e))
                })?;
                mac.update(input.as_bytes());
                format!("{:x}", mac.finalize().into_bytes())
            }
            CryptoFnType::BCryptHash => {
                let cost = key.parse::<u32>().unwrap_or(12);
                bcrypt::hash(input, cost).map_err(|e| {
                    crate::error::AppError::Pipeline(format!("BCrypt hash error: {}", e))
                })?
            }
            CryptoFnType::BCryptVerify => {
                // key = the hash to verify against
                let valid = bcrypt::verify(input, &key).map_err(|e| {
                    crate::error::AppError::Pipeline(format!("BCrypt verify error: {}", e))
                })?;
                valid.to_string()
            }
            CryptoFnType::AESEncrypt => {
                use aes_gcm::aead::generic_array::GenericArray;
                use aes_gcm::{aead::Aead, Aes256Gcm, KeyInit};

                let key_bytes = if key.len() == 64 {
                    // Hex-encoded 32-byte key
                    (0..key.len())
                        .step_by(2)
                        .filter_map(|i| u8::from_str_radix(&key[i..i + 2], 16).ok())
                        .collect::<Vec<u8>>()
                } else {
                    // Pad/truncate to 32 bytes
                    let mut k = key.as_bytes().to_vec();
                    k.resize(32, 0);
                    k
                };
                let cipher = Aes256Gcm::new(GenericArray::from_slice(&key_bytes));
                let nonce_bytes: [u8; 12] = rand::random();
                let nonce = GenericArray::from_slice(&nonce_bytes);
                let ciphertext = cipher.encrypt(nonce, input.as_bytes()).map_err(|e| {
                    crate::error::AppError::Pipeline(format!("AES encrypt error: {}", e))
                })?;
                // Output: hex(nonce) + ":" + hex(ciphertext)
                let nonce_hex: String = nonce_bytes.iter().map(|b| format!("{:02x}", b)).collect();
                let ct_hex: String = ciphertext.iter().map(|b| format!("{:02x}", b)).collect();
                format!("{}:{}", nonce_hex, ct_hex)
            }
            CryptoFnType::AESDecrypt => {
                use aes_gcm::aead::generic_array::GenericArray;
                use aes_gcm::{aead::Aead, Aes256Gcm, KeyInit};

                let key_bytes = if key.len() == 64 {
                    (0..key.len())
                        .step_by(2)
                        .filter_map(|i| u8::from_str_radix(&key[i..i + 2], 16).ok())
                        .collect::<Vec<u8>>()
                } else {
                    let mut k = key.as_bytes().to_vec();
                    k.resize(32, 0);
                    k
                };
                let cipher = Aes256Gcm::new(GenericArray::from_slice(&key_bytes));
                let parts: Vec<&str> = input.splitn(2, ':').collect();
                if parts.len() != 2 {
                    return Err(crate::error::AppError::Pipeline(
                        "AES decrypt: expected nonce:ciphertext format".into(),
                    ));
                }
                let nonce_bytes: Vec<u8> = (0..parts[0].len())
                    .step_by(2)
                    .filter_map(|i| u8::from_str_radix(&parts[0][i..i + 2], 16).ok())
                    .collect();
                let ct_bytes: Vec<u8> = (0..parts[1].len())
                    .step_by(2)
                    .filter_map(|i| u8::from_str_radix(&parts[1][i..i + 2], 16).ok())
                    .collect();
                let nonce = GenericArray::from_slice(&nonce_bytes);
                let plaintext = cipher.decrypt(nonce, ct_bytes.as_ref()).map_err(|e| {
                    crate::error::AppError::Pipeline(format!("AES decrypt error: {}", e))
                })?;
                String::from_utf8_lossy(&plaintext).to_string()
            }
            CryptoFnType::Base64Encode => {
                use base64::Engine;
                base64::engine::general_purpose::STANDARD.encode(input.as_bytes())
            }
            CryptoFnType::Base64Decode => {
                use base64::Engine;
                match base64::engine::general_purpose::STANDARD.decode(input.as_bytes()) {
                    Ok(bytes) => String::from_utf8_lossy(&bytes).to_string(),
                    Err(_) => String::new(),
                }
            }
        };

        self.variables
            .set_user(&settings.output_var, result, settings.capture);
        Ok(())
    }

    // ── Date Function ──

    pub(super) fn execute_date_function(
        &mut self,
        settings: &DateFunctionSettings,
    ) -> crate::error::Result<()> {
        let result = match settings.function_type {
            DateFnType::Now => chrono::Local::now().format(&settings.format).to_string(),
            DateFnType::UnixTimestamp => chrono::Utc::now().timestamp().to_string(),
            DateFnType::UnixToDate => {
                let input = self.variables.resolve_input(&settings.input_var);
                let ts: i64 = input.parse().unwrap_or(0);
                if let Some(dt) = chrono::DateTime::from_timestamp(ts, 0) {
                    dt.format(&settings.format).to_string()
                } else {
                    String::new()
                }
            }
            DateFnType::FormatDate => {
                let input = self.variables.resolve_input(&settings.input_var);
                if let Ok(dt) = chrono::NaiveDateTime::parse_from_str(&input, "%Y-%m-%d %H:%M:%S") {
                    dt.format(&settings.format).to_string()
                } else {
                    input
                }
            }
            DateFnType::ParseDate => {
                let input = self.variables.resolve_input(&settings.input_var);
                let fmt = self.variables.interpolate(&settings.format);
                if let Ok(dt) = chrono::NaiveDateTime::parse_from_str(&input, &fmt) {
                    dt.and_utc().timestamp().to_string()
                } else {
                    String::new()
                }
            }
            DateFnType::CurrentUnixTimeMs => {
                let ms = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis();
                ms.to_string()
            }
            DateFnType::Compute => {
                // Evaluate simple arithmetic expression with variable interpolation
                // Supports: +, -, *, /, % and parentheses (via naive recursive descent)
                let expr = self.variables.interpolate(&settings.param);
                compute_expr(&expr).to_string()
            }
            DateFnType::Round => {
                let input = self.variables.resolve_input(&settings.input_var);
                let places: u32 = settings.param.parse().unwrap_or(2);
                let val: f64 = input.parse().unwrap_or(0.0);
                let factor = 10f64.powi(places as i32);
                let rounded = (val * factor).round() / factor;
                if places == 0 {
                    (rounded as i64).to_string()
                } else {
                    format!("{:.prec$}", rounded, prec = places as usize)
                }
            }
            DateFnType::DateToUnix | DateFnType::DateToUnixMs => {
                let input = self.variables.resolve_input(&settings.input_var);
                let fmt = self.variables.interpolate(&settings.format);
                // Try parsing as NaiveDateTime first, then NaiveDate for date-only strings.
                let utc = chrono::NaiveDateTime::parse_from_str(&input, &fmt)
                    .map(|dt| dt.and_utc())
                    .or_else(|_| {
                        chrono::NaiveDate::parse_from_str(&input, &fmt)
                            .map(|d| d.and_hms_opt(0, 0, 0).unwrap_or_default().and_utc())
                    });
                match utc {
                    Ok(dt) => {
                        if matches!(settings.function_type, DateFnType::DateToUnixMs) {
                            dt.timestamp_millis().to_string()
                        } else {
                            dt.timestamp().to_string()
                        }
                    }
                    Err(_) => String::new(),
                }
            }
            DateFnType::AddTime | DateFnType::SubtractTime => {
                let input = self.variables.resolve_input(&settings.input_var);
                let ts: i64 = input
                    .parse()
                    .unwrap_or_else(|_| chrono::Utc::now().timestamp());
                let amount = settings.amount;
                let delta_secs = match settings.unit.as_str() {
                    "seconds" => amount,
                    "minutes" => amount * 60,
                    "hours" => amount * 3600,
                    "days" => amount * 86400,
                    _ => amount,
                };
                let new_ts = if matches!(settings.function_type, DateFnType::AddTime) {
                    ts + delta_secs
                } else {
                    ts - delta_secs
                };
                if let Some(dt) = chrono::DateTime::from_timestamp(new_ts, 0) {
                    dt.format(&settings.format).to_string()
                } else {
                    new_ts.to_string()
                }
            }
        };
        self.variables
            .set_user(&settings.output_var, result, settings.capture);
        Ok(())
    }

    // ── Case / Switch ──

    pub(super) fn execute_case_switch(
        &mut self,
        settings: &CaseSwitchSettings,
    ) -> crate::error::Result<()> {
        let input = self.variables.resolve_input(&settings.input_var);
        let result = settings
            .cases
            .iter()
            .find(|c| c.match_value == input)
            .map(|c| self.variables.interpolate(&c.result_value))
            .unwrap_or_else(|| self.variables.interpolate(&settings.default_value));
        self.variables
            .set_user(&settings.output_var, result, settings.capture);
        Ok(())
    }

    // ── List Function ──

    pub(super) fn execute_list_function(
        &mut self,
        settings: &ListFunctionSettings,
    ) -> crate::error::Result<()> {
        let input = self.variables.resolve_input(&settings.input_var);
        let param1 = self.variables.interpolate(&settings.param1);

        let items: Vec<String> =
            serde_json::from_str(&input).unwrap_or_else(|_| vec![input.clone()]);

        let result = match settings.function_type {
            ListFnType::Join => items.join(&param1),
            ListFnType::Sort => {
                let mut sorted = items;
                sorted.sort();
                serde_json::to_string(&sorted).unwrap_or_default()
            }
            ListFnType::Shuffle => {
                use rand::seq::SliceRandom;
                let mut shuffled = items;
                shuffled.shuffle(&mut rand::thread_rng());
                serde_json::to_string(&shuffled).unwrap_or_default()
            }
            ListFnType::Add => {
                let mut list = items;
                list.push(param1);
                serde_json::to_string(&list).unwrap_or_default()
            }
            ListFnType::Remove => {
                let list: Vec<String> = items.into_iter().filter(|i| *i != param1).collect();
                serde_json::to_string(&list).unwrap_or_default()
            }
            ListFnType::Deduplicate => {
                let mut seen = std::collections::HashSet::new();
                let deduped: Vec<String> = items
                    .into_iter()
                    .filter(|i| seen.insert(i.clone()))
                    .collect();
                serde_json::to_string(&deduped).unwrap_or_default()
            }
            ListFnType::RandomItem => {
                use rand::seq::SliceRandom;
                items
                    .choose(&mut rand::thread_rng())
                    .cloned()
                    .unwrap_or_default()
            }
            ListFnType::Length => items.len().to_string(),
        };

        self.variables
            .set_user(&settings.output_var, result, settings.capture);
        Ok(())
    }

    // ── Conversion Function ──

    pub(super) fn execute_conversion_function(
        &mut self,
        settings: &ConversionFunctionSettings,
    ) -> crate::error::Result<()> {
        use super::data::{
            bytes_to_csv, number_to_words, parse_bytes, readable_size, words_to_number,
        };
        use crate::pipeline::block::settings_functions::ConversionOp;

        let input = self.variables.resolve_input(&settings.input_var);

        let result = match settings.op {
            ConversionOp::StringToInt => input.trim().parse::<i64>().unwrap_or(0).to_string(),
            ConversionOp::IntToString
            | ConversionOp::FloatToString
            | ConversionOp::BoolToString => input.trim().to_string(),
            ConversionOp::StringToFloat => input.trim().parse::<f64>().unwrap_or(0.0).to_string(),
            ConversionOp::StringToBool => match input.trim().to_lowercase().as_str() {
                "true" | "1" | "yes" => "true".into(),
                _ => "false".into(),
            },
            ConversionOp::IntToFloat => {
                let v = input.trim().parse::<i64>().unwrap_or(0);
                format!("{:.1}", v as f64)
            }
            ConversionOp::FloatToInt => {
                let v = input.trim().parse::<f64>().unwrap_or(0.0);
                (v as i64).to_string()
            }
            ConversionOp::Base64Encode => {
                use base64::Engine;
                base64::engine::general_purpose::STANDARD.encode(input.as_bytes())
            }
            ConversionOp::Base64Decode => {
                use base64::Engine;
                match base64::engine::general_purpose::STANDARD.decode(input.trim()) {
                    Ok(bytes) => String::from_utf8_lossy(&bytes).to_string(),
                    Err(_) => String::new(),
                }
            }
            ConversionOp::HexEncode => input
                .bytes()
                .map(|b| format!("{:02x}", b))
                .collect::<String>(),
            ConversionOp::HexDecode => {
                let hex: String = input.chars().filter(|c| c.is_ascii_hexdigit()).collect();
                let bytes: Vec<u8> = (0..hex.len())
                    .step_by(2)
                    .filter_map(|i| u8::from_str_radix(hex.get(i..i + 2)?, 16).ok())
                    .collect();
                String::from_utf8_lossy(&bytes).to_string()
            }
            ConversionOp::UrlEncode => input
                .bytes()
                .map(|b| match b {
                    b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                        (b as char).to_string()
                    }
                    _ => format!("%{:02X}", b),
                })
                .collect(),
            ConversionOp::UrlDecode => {
                let mut out = String::new();
                let bytes = input.as_bytes();
                let mut i = 0;
                while i < bytes.len() {
                    if bytes[i] == b'%' && i + 2 < bytes.len() {
                        if let Ok(b) = u8::from_str_radix(
                            std::str::from_utf8(&bytes[i + 1..i + 3]).unwrap_or(""),
                            16,
                        ) {
                            out.push(b as char);
                            i += 3;
                            continue;
                        }
                    } else if bytes[i] == b'+' {
                        out.push(' ');
                        i += 1;
                        continue;
                    }
                    out.push(bytes[i] as char);
                    i += 1;
                }
                out
            }
            ConversionOp::HtmlEncode => input
                .replace('&', "&amp;")
                .replace('<', "&lt;")
                .replace('>', "&gt;")
                .replace('"', "&quot;")
                .replace('\'', "&#39;"),
            ConversionOp::HtmlDecode => input
                .replace("&amp;", "&")
                .replace("&lt;", "<")
                .replace("&gt;", ">")
                .replace("&quot;", "\"")
                .replace("&#39;", "'"),
            ConversionOp::StringToBytes => {
                let bytes: Vec<u8> = match settings.encoding.as_str() {
                    "utf16" | "utf-16" => {
                        let utf16: Vec<u16> = input.encode_utf16().collect();
                        utf16.iter().flat_map(|u| u.to_be_bytes()).collect()
                    }
                    _ => input.into_bytes(),
                };
                bytes_to_csv(&bytes)
            }
            ConversionOp::BytesToString => {
                let bytes = parse_bytes(&input);
                String::from_utf8_lossy(&bytes).to_string()
            }
            ConversionOp::IntToBytes => {
                let n: i64 = input.trim().parse().unwrap_or(0);
                let count = (settings.byte_count as usize).clamp(1, 8);
                let all = if settings.endianness == "little" {
                    n.to_le_bytes()
                } else {
                    n.to_be_bytes()
                };
                let bytes = if settings.endianness == "little" {
                    all[..count].to_vec()
                } else {
                    all[8 - count..].to_vec()
                };
                bytes_to_csv(&bytes)
            }
            ConversionOp::BytesToInt => {
                let bytes = parse_bytes(&input);
                let mut arr = [0u8; 8];
                let start = 8usize.saturating_sub(bytes.len());
                for (i, b) in bytes.iter().enumerate() {
                    arr[start + i] = *b;
                }
                i64::from_be_bytes(arr).to_string()
            }
            ConversionOp::BigIntToBytes => {
                use num_bigint::BigInt;
                use std::str::FromStr;
                match BigInt::from_str(input.trim()) {
                    Ok(n) => {
                        let (_, b) = n.to_bytes_be();
                        bytes_to_csv(&b)
                    }
                    Err(_) => String::new(),
                }
            }
            ConversionOp::BytesToBigInt => {
                use num_bigint::BigUint;
                let bytes = parse_bytes(&input);
                BigUint::from_bytes_be(&bytes).to_string()
            }
            ConversionOp::BytesToBinaryString => {
                let bytes = parse_bytes(&input);
                bytes
                    .iter()
                    .map(|b| format!("{:08b}", b))
                    .collect::<Vec<_>>()
                    .join(" ")
            }
            ConversionOp::BinaryStringToBytes => {
                let cleaned: String = input.chars().filter(|c| *c == '0' || *c == '1').collect();
                let bytes: Vec<u8> = cleaned
                    .as_bytes()
                    .chunks(8)
                    .filter_map(|chunk| {
                        u8::from_str_radix(std::str::from_utf8(chunk).ok()?, 2).ok()
                    })
                    .collect();
                bytes_to_csv(&bytes)
            }
            ConversionOp::ReadableSize => {
                let n: i64 = input.trim().parse().unwrap_or(0);
                readable_size(n)
            }
            ConversionOp::NumberToWords => {
                let n: i64 = input.trim().parse().unwrap_or(0);
                number_to_words(n)
            }
            ConversionOp::WordsToNumber => words_to_number(&input).to_string(),
            ConversionOp::SvgToPng => {
                use base64::Engine;
                use resvg::tiny_skia;
                use resvg::usvg;
                let mut opt = usvg::Options::default();
                opt.fontdb_mut().load_system_fonts();
                match usvg::Tree::from_str(&input, &opt) {
                    Ok(tree) => {
                        let size = tree.size();
                        let w = (size.width().ceil() as u32).max(1);
                        let h = (size.height().ceil() as u32).max(1);
                        if let Some(mut pixmap) = tiny_skia::Pixmap::new(w, h) {
                            resvg::render(
                                &tree,
                                tiny_skia::Transform::default(),
                                &mut pixmap.as_mut(),
                            );
                            match pixmap.encode_png() {
                                Ok(png) => base64::engine::general_purpose::STANDARD.encode(&png),
                                Err(_) => String::new(),
                            }
                        } else {
                            String::new()
                        }
                    }
                    Err(_) => String::new(),
                }
            }
        };

        self.variables
            .set_user(&settings.output_var, result, settings.capture);
        Ok(())
    }

    // ── Cookie Container (OpenBullet-style) ──

    pub(super) fn execute_cookie_container(
        &mut self,
        settings: &CookieContainerSettings,
    ) -> crate::error::Result<()> {
        let raw_text = if settings.source_type == "file" {
            let path = self.variables.interpolate(&settings.source);
            std::fs::read_to_string(&path).map_err(|e| {
                crate::error::AppError::Pipeline(format!("Cookie file read error: {}", e))
            })?
        } else {
            self.variables.interpolate(&settings.source)
        };

        let domain_filter = self.variables.interpolate(&settings.domain);

        // Parse cookies -- supports Netscape format (tab-separated) and simple name=value
        let mut cookies: Vec<(String, String)> = Vec::new();
        let mut netscape_lines: Vec<String> = Vec::new();
        let mut seen_keys = std::collections::HashSet::new();

        for line in raw_text.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            let parts: Vec<&str> = line.split('\t').collect();
            if parts.len() >= 7 {
                // Netscape format: domain, flag, path, secure, expiry, name, value
                let cookie_domain = parts[0];
                let name = parts[5];
                let value = parts[6];

                // Domain filter
                if !domain_filter.is_empty() && !cookie_domain.contains(&domain_filter) {
                    continue;
                }

                // Deduplicate
                if seen_keys.insert(name.to_string()) {
                    cookies.push((name.to_string(), value.to_string()));
                    netscape_lines.push(line.to_string());
                }
            } else if let Some(eq) = line.find('=') {
                // Simple name=value format
                let name = line[..eq].trim();
                let value = line[eq + 1..].trim();
                if seen_keys.insert(name.to_string()) {
                    cookies.push((name.to_string(), value.to_string()));
                }
            }
        }

        // Store as "name=value; name2=value2" format
        let cookie_string = cookies
            .iter()
            .map(|(n, v)| format!("{}={}", n, v))
            .collect::<Vec<_>>()
            .join("; ");
        self.variables
            .set_user(&settings.output_var, cookie_string, settings.capture);

        // Optionally store in Netscape format
        if settings.save_netscape && !netscape_lines.is_empty() {
            self.variables.set_user(
                &format!("{}_NETSCAPE", settings.output_var),
                netscape_lines.join("\n"),
                false,
            );
        }

        Ok(())
    }

    // ── ByteArray Function ──

    pub(super) fn execute_byte_array(
        &mut self,
        settings: &ByteArraySettings,
    ) -> crate::error::Result<()> {
        let input = self.variables.resolve_input(&settings.input_var);

        let result = match settings.operation {
            ByteArrayOp::ToHex => input
                .as_bytes()
                .iter()
                .map(|b| format!("{:02x}", b))
                .collect::<String>(),
            ByteArrayOp::FromHex => {
                let hex_clean = input
                    .chars()
                    .filter(|c| c.is_ascii_hexdigit())
                    .collect::<String>();
                let bytes: Vec<u8> = (0..hex_clean.len())
                    .step_by(2)
                    .filter_map(|i| u8::from_str_radix(&hex_clean[i..i + 2], 16).ok())
                    .collect();
                String::from_utf8_lossy(&bytes).to_string()
            }
            ByteArrayOp::ToBase64 => {
                use base64::Engine;
                base64::engine::general_purpose::STANDARD.encode(input.as_bytes())
            }
            ByteArrayOp::FromBase64 => {
                use base64::Engine;
                match base64::engine::general_purpose::STANDARD.decode(input.as_bytes()) {
                    Ok(bytes) => String::from_utf8_lossy(&bytes).to_string(),
                    Err(_) => String::new(),
                }
            }
            ByteArrayOp::ToUtf8 => {
                // Interpret input as comma-separated byte values
                let bytes: Vec<u8> = input
                    .split(',')
                    .filter_map(|s| s.trim().parse().ok())
                    .collect();
                String::from_utf8_lossy(&bytes).to_string()
            }
            ByteArrayOp::FromUtf8 => {
                // Convert string to comma-separated byte values
                input
                    .as_bytes()
                    .iter()
                    .map(|b| b.to_string())
                    .collect::<Vec<_>>()
                    .join(",")
            }
        };

        self.variables
            .set_user(&settings.output_var, result, settings.capture);
        Ok(())
    }

    // ── Constants Block ──

    pub(super) fn execute_constants(
        &mut self,
        settings: &ConstantsSettings,
    ) -> crate::error::Result<()> {
        for constant in &settings.constants {
            let value = self.variables.interpolate(&constant.value);
            self.variables.set_user(&constant.name, value, false);
        }
        Ok(())
    }

    // ── Dictionary Function ──

    pub(super) fn execute_dictionary(
        &mut self,
        settings: &DictionarySettings,
    ) -> crate::error::Result<()> {
        use serde_json::Value;

        let dict_var = self.variables.interpolate(&settings.dict_var);
        let key = self.variables.interpolate(&settings.key);
        let value = self.variables.interpolate(&settings.value);

        let result = match settings.operation {
            DictOp::Get => {
                // Parse dict as JSON object and get key
                if let Ok(Value::Object(map)) = serde_json::from_str::<Value>(&dict_var) {
                    map.get(&key)
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string()
                } else {
                    String::new()
                }
            }
            DictOp::Set => {
                // Parse, insert key-value, serialize back
                let mut map = if let Ok(Value::Object(m)) = serde_json::from_str::<Value>(&dict_var)
                {
                    m
                } else {
                    serde_json::Map::new()
                };
                map.insert(key.clone(), Value::String(value.clone()));
                serde_json::to_string(&map).unwrap_or_default()
            }
            DictOp::Remove => {
                // Parse, remove key, serialize back
                let mut map = if let Ok(Value::Object(m)) = serde_json::from_str::<Value>(&dict_var)
                {
                    m
                } else {
                    serde_json::Map::new()
                };
                map.remove(&key);
                serde_json::to_string(&map).unwrap_or_default()
            }
            DictOp::Exists => {
                // Check if key exists
                if let Ok(Value::Object(map)) = serde_json::from_str::<Value>(&dict_var) {
                    if map.contains_key(&key) {
                        "true"
                    } else {
                        "false"
                    }
                } else {
                    "false"
                }
                .to_string()
            }
            DictOp::Keys => {
                // Get all keys as JSON array
                if let Ok(Value::Object(map)) = serde_json::from_str::<Value>(&dict_var) {
                    let keys: Vec<&str> = map.keys().map(|k| k.as_str()).collect();
                    serde_json::to_string(&keys).unwrap_or_default()
                } else {
                    "[]".to_string()
                }
            }
            DictOp::Values => {
                // Get all values as JSON array
                if let Ok(Value::Object(map)) = serde_json::from_str::<Value>(&dict_var) {
                    let values: Vec<String> = map
                        .values()
                        .filter_map(|v| v.as_str())
                        .map(|s| s.to_string())
                        .collect();
                    serde_json::to_string(&values).unwrap_or_default()
                } else {
                    "[]".to_string()
                }
            }
        };

        // For Set and Remove, update the dict variable
        match settings.operation {
            DictOp::Set | DictOp::Remove => {
                self.variables
                    .set_user(&settings.dict_var, result.clone(), false);
                self.variables
                    .set_user(&settings.output_var, result, settings.capture);
            }
            _ => {
                self.variables
                    .set_user(&settings.output_var, result, settings.capture);
            }
        }

        Ok(())
    }

    // ── Float Function ──

    pub(super) fn execute_float_function(
        &mut self,
        settings: &FloatFunctionSettings,
    ) -> crate::error::Result<()> {
        let input = self.variables.resolve_input(&settings.input_var);
        let param1 = self.variables.interpolate(&settings.param1);
        let param2 = self.variables.interpolate(&settings.param2);

        let val: f64 = input.parse().unwrap_or(0.0);
        let p1: f64 = param1.parse().unwrap_or(0.0);
        let _p2: f64 = param2.parse().unwrap_or(0.0);

        let result = match settings.function_type {
            FloatFnType::Round => {
                let places: u32 = param1.parse().unwrap_or(0);
                let mult = 10_f64.powi(places as i32);
                ((val * mult).round() / mult).to_string()
            }
            FloatFnType::Ceil => val.ceil().to_string(),
            FloatFnType::Floor => val.floor().to_string(),
            FloatFnType::Abs => val.abs().to_string(),
            FloatFnType::Add => (val + p1).to_string(),
            FloatFnType::Subtract => (val - p1).to_string(),
            FloatFnType::Multiply => (val * p1).to_string(),
            FloatFnType::Divide => {
                if p1 != 0.0 {
                    (val / p1).to_string()
                } else {
                    "0".to_string()
                }
            }
            FloatFnType::Power => val.powf(p1).to_string(),
            FloatFnType::Sqrt => val.sqrt().to_string(),
            FloatFnType::Min => val.min(p1).to_string(),
            FloatFnType::Max => val.max(p1).to_string(),
        };

        self.variables
            .set_user(&settings.output_var, result, settings.capture);
        Ok(())
    }

    // ── Integer Function ──

    pub(super) fn execute_integer_function(
        &mut self,
        settings: &IntegerFunctionSettings,
    ) -> crate::error::Result<()> {
        let input = self.variables.resolve_input(&settings.input_var);
        let param1 = self.variables.interpolate(&settings.param1);
        let param2 = self.variables.interpolate(&settings.param2);

        let val: i64 = input.parse().unwrap_or(0);
        let p1: i64 = param1.parse().unwrap_or(0);
        let _p2: i64 = param2.parse().unwrap_or(0);

        let result = match settings.function_type {
            IntegerFnType::Add => (val + p1).to_string(),
            IntegerFnType::Subtract => (val - p1).to_string(),
            IntegerFnType::Multiply => (val * p1).to_string(),
            IntegerFnType::Divide => {
                if p1 != 0 {
                    (val / p1).to_string()
                } else {
                    "0".to_string()
                }
            }
            IntegerFnType::Modulo => {
                if p1 != 0 {
                    (val % p1).to_string()
                } else {
                    "0".to_string()
                }
            }
            IntegerFnType::Power => {
                if p1 >= 0 {
                    (val.pow(p1 as u32)).to_string()
                } else {
                    "0".to_string()
                }
            }
            IntegerFnType::Abs => val.abs().to_string(),
            IntegerFnType::Min => val.min(p1).to_string(),
            IntegerFnType::Max => val.max(p1).to_string(),
        };

        self.variables
            .set_user(&settings.output_var, result, settings.capture);
        Ok(())
    }

    // ── Time Function ──

    pub(super) fn execute_time_function(
        &mut self,
        settings: &TimeFunctionSettings,
    ) -> crate::error::Result<()> {
        use chrono::{DateTime, TimeZone, Utc};
        use chrono_tz::Tz;

        let input = self.variables.resolve_input(&settings.input_var);
        let _tz_str = self.variables.interpolate(&settings.timezone);
        let target_tz_str = self.variables.interpolate(&settings.target_timezone);
        let format = self.variables.interpolate(&settings.format);

        let result = match settings.function_type {
            TimeFnType::ConvertTimezone => {
                // Parse input as UTC timestamp, convert to target timezone
                if let Ok(dt) = DateTime::parse_from_rfc3339(&input) {
                    if let Ok(target_tz) = target_tz_str.parse::<Tz>() {
                        let converted = dt.with_timezone(&target_tz);
                        converted.format(&format).to_string()
                    } else {
                        input
                    }
                } else if let Ok(timestamp) = input.parse::<i64>() {
                    if let Ok(target_tz) = target_tz_str.parse::<Tz>() {
                        let dt = Utc.timestamp_opt(timestamp, 0).unwrap();
                        let converted = dt.with_timezone(&target_tz);
                        converted.format(&format).to_string()
                    } else {
                        input
                    }
                } else {
                    input
                }
            }
            TimeFnType::GetTimezone => {
                // Extract timezone from timestamp
                if let Ok(dt) = DateTime::parse_from_rfc3339(&input) {
                    dt.timezone().to_string()
                } else {
                    "UTC".to_string()
                }
            }
            TimeFnType::IsDST => {
                // Check if date is in DST (simplified - always returns false for UTC)
                "false".to_string()
            }
            TimeFnType::DurationBetween => {
                // Calculate duration between two timestamps
                let param1 = self.variables.interpolate(&settings.target_timezone);
                if let (Ok(ts1), Ok(ts2)) = (input.parse::<i64>(), param1.parse::<i64>()) {
                    let diff = (ts2 - ts1).abs();
                    diff.to_string()
                } else {
                    "0".to_string()
                }
            }
            TimeFnType::AddDuration => {
                // Add duration (in seconds) to timestamp
                if let Ok(timestamp) = input.parse::<i64>() {
                    let duration: i64 = format.parse().unwrap_or(0);
                    (timestamp + duration).to_string()
                } else {
                    input
                }
            }
            TimeFnType::SubtractDuration => {
                // Subtract duration (in seconds) from timestamp
                if let Ok(timestamp) = input.parse::<i64>() {
                    let duration: i64 = format.parse().unwrap_or(0);
                    (timestamp - duration).to_string()
                } else {
                    input
                }
            }
        };

        self.variables
            .set_user(&settings.output_var, result, settings.capture);
        Ok(())
    }

    // ── Generate GUID ──

    pub(super) fn execute_generate_guid(
        &mut self,
        settings: &GenerateGUIDSettings,
    ) -> crate::error::Result<()> {
        use uuid::Uuid;

        let result = match settings.guid_version {
            GUIDVersion::V1 => {
                // Timestamp-based UUID (use v4 as v1 requires MAC address)
                Uuid::new_v4().to_string()
            }
            GUIDVersion::V4 => {
                // Random UUID
                Uuid::new_v4().to_string()
            }
            GUIDVersion::V5 => {
                // Hash-based UUID (use v4 as v5 not available in this uuid version)
                Uuid::new_v4().to_string()
            }
        };

        self.variables
            .set_user(&settings.output_var, result, settings.capture);
        Ok(())
    }

    // ── Phone Country ──

    pub(super) fn execute_phone_country(
        &mut self,
        settings: &PhoneCountrySettings,
    ) -> crate::error::Result<()> {
        let input = self.variables.resolve_input(&settings.input_var);

        let result = match phonenumber::parse(None, &input) {
            Ok(number) => {
                let country = number.country();
                let country_code = country.code();
                match settings.output_format {
                    PhoneOutputFormat::CountryCode => {
                        // Return numeric country code
                        country_code.to_string()
                    }
                    PhoneOutputFormat::CountryName => {
                        // Map country code to name (simplified)
                        match country_code {
                            1 => "United States",
                            44 => "United Kingdom",
                            86 => "China",
                            91 => "India",
                            81 => "Japan",
                            49 => "Germany",
                            33 => "France",
                            39 => "Italy",
                            7 => "Russia",
                            82 => "South Korea",
                            _ => "Unknown",
                        }
                        .to_string()
                    }
                    PhoneOutputFormat::ISO2 => {
                        // Return ISO-2 code
                        format!("{:?}", country.id())
                    }
                    PhoneOutputFormat::ISO3 => {
                        // Return ISO-3 code (simplified - same as ISO2)
                        format!("{:?}", country.id())
                    }
                }
            }
            Err(_) => String::new(),
        };

        self.variables
            .set_user(&settings.output_var, result, settings.capture);
        Ok(())
    }
}

/// Very small arithmetic evaluator supporting +, -, *, /, % and parens.
fn compute_expr(expr: &str) -> f64 {
    let tokens: Vec<char> = expr.chars().filter(|c| !c.is_whitespace()).collect();
    let mut pos = 0usize;
    parse_additive(&tokens, &mut pos)
}

fn parse_additive(tokens: &[char], pos: &mut usize) -> f64 {
    let mut result = parse_multiplicative(tokens, pos);
    while *pos < tokens.len() {
        match tokens[*pos] {
            '+' => {
                *pos += 1;
                result += parse_multiplicative(tokens, pos);
            }
            '-' => {
                *pos += 1;
                result -= parse_multiplicative(tokens, pos);
            }
            _ => break,
        }
    }
    result
}

fn parse_multiplicative(tokens: &[char], pos: &mut usize) -> f64 {
    let mut result = parse_unary(tokens, pos);
    while *pos < tokens.len() {
        match tokens[*pos] {
            '*' => {
                *pos += 1;
                result *= parse_unary(tokens, pos);
            }
            '/' => {
                *pos += 1;
                let d = parse_unary(tokens, pos);
                result = if d != 0.0 { result / d } else { f64::NAN };
            }
            '%' => {
                *pos += 1;
                let d = parse_unary(tokens, pos);
                result = if d != 0.0 { result % d } else { f64::NAN };
            }
            _ => break,
        }
    }
    result
}

fn parse_unary(tokens: &[char], pos: &mut usize) -> f64 {
    if *pos < tokens.len() && tokens[*pos] == '-' {
        *pos += 1;
        return -parse_primary(tokens, pos);
    }
    parse_primary(tokens, pos)
}

fn parse_primary(tokens: &[char], pos: &mut usize) -> f64 {
    if *pos >= tokens.len() {
        return 0.0;
    }
    if tokens[*pos] == '(' {
        *pos += 1;
        let v = parse_additive(tokens, pos);
        if *pos < tokens.len() && tokens[*pos] == ')' {
            *pos += 1;
        }
        return v;
    }
    let start = *pos;
    while *pos < tokens.len() && (tokens[*pos].is_ascii_digit() || tokens[*pos] == '.') {
        *pos += 1;
    }
    tokens[start..*pos]
        .iter()
        .collect::<String>()
        .parse::<f64>()
        .unwrap_or(0.0)
}

#[cfg(test)]
mod tests {
    use super::translate_exact;

    #[test]
    fn translate_uses_first_exact_mapping_and_preserves_unmatched_input() {
        let table =
            "\n# status names\n pending => queued \npending => should-not-win\nempty =>\ninvalid\n";
        assert_eq!(translate_exact("pending", table), "queued");
        assert_eq!(translate_exact("empty", table), "");
        assert_eq!(translate_exact("unknown", table), "unknown");
    }
}
