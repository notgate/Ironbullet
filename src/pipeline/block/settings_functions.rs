use serde::{Deserialize, Serialize};

use super::Block;

// ── String Function ──

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StringFunctionSettings {
    pub function_type: StringFnType,
    pub input_var: String,
    pub output_var: String,
    pub capture: bool,
    pub param1: String,
    pub param2: String,
}

impl Default for StringFunctionSettings {
    fn default() -> Self {
        Self {
            function_type: StringFnType::Replace,
            input_var: String::new(),
            output_var: "RESULT".into(),
            capture: false,
            param1: String::new(),
            param2: String::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum StringFnType {
    Replace,
    Substring,
    Trim,
    ToUpper,
    ToLower,
    URLEncode,
    URLDecode,
    Base64Encode,
    Base64Decode,
    HTMLEntityEncode,
    HTMLEntityDecode,
    Split,
    RandomString,
    Reverse,
    Length,
}

// ── Translate ──

/// Deterministic exact-match lookup table. Each non-empty mapping line is
/// `source => destination`; comments start with `#`. Unmatched input is passed
/// through unchanged, which makes partial tables safe in a pipeline.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranslateSettings {
    pub input_var: String,
    pub output_var: String,
    pub capture: bool,
    #[serde(default)]
    pub mappings: String,
}

impl Default for TranslateSettings {
    fn default() -> Self {
        Self {
            input_var: String::new(),
            output_var: "TRANSLATED".into(),
            capture: false,
            mappings: String::new(),
        }
    }
}

// ── List Function ──

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListFunctionSettings {
    pub function_type: ListFnType,
    pub input_var: String,
    pub output_var: String,
    pub capture: bool,
    pub param1: String,
}

impl Default for ListFunctionSettings {
    fn default() -> Self {
        Self {
            function_type: ListFnType::Join,
            input_var: String::new(),
            output_var: "RESULT".into(),
            capture: false,
            param1: String::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ListFnType {
    Join,
    Sort,
    Shuffle,
    Add,
    Remove,
    Deduplicate,
    RandomItem,
    Length,
}

// ── Crypto Function ──

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CryptoFunctionSettings {
    pub function_type: CryptoFnType,
    pub input_var: String,
    pub output_var: String,
    pub capture: bool,
    pub key: String,
}

impl Default for CryptoFunctionSettings {
    fn default() -> Self {
        Self {
            function_type: CryptoFnType::MD5,
            input_var: String::new(),
            output_var: "HASH".into(),
            capture: false,
            key: String::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CryptoFnType {
    MD5,
    SHA1,
    SHA256,
    SHA512,
    SHA384,
    CRC32,
    HMACSHA256,
    HMACSHA512,
    HMACMD5,
    BCryptHash,
    BCryptVerify,
    Base64Encode,
    Base64Decode,
    AESEncrypt,
    AESDecrypt,
}

// ── Conversion Function ──

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "PascalCase")]
pub enum ConversionOp {
    StringToInt,
    IntToString,
    StringToFloat,
    FloatToString,
    BoolToString,
    StringToBool,
    IntToFloat,
    FloatToInt,
    Base64Encode,
    Base64Decode,
    HexEncode,
    HexDecode,
    UrlEncode,
    UrlDecode,
    HtmlEncode,
    HtmlDecode,
    StringToBytes,
    BytesToString,
    IntToBytes,
    BytesToInt,
    BigIntToBytes,
    BytesToBigInt,
    BytesToBinaryString,
    BinaryStringToBytes,
    ReadableSize,
    NumberToWords,
    WordsToNumber,
    SvgToPng,
}

impl Default for ConversionOp {
    fn default() -> Self {
        ConversionOp::Base64Encode
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversionFunctionSettings {
    #[serde(default)]
    pub op: ConversionOp,
    pub input_var: String,
    pub output_var: String,
    pub capture: bool,
    #[serde(default)]
    pub encoding: String,
    #[serde(default)]
    pub endianness: String,
    #[serde(default = "default_byte_count")]
    pub byte_count: u32,
    #[serde(default)]
    pub from_type: String,
    #[serde(default)]
    pub to_type: String,
}

fn default_byte_count() -> u32 {
    4
}

impl Default for ConversionFunctionSettings {
    fn default() -> Self {
        Self {
            op: ConversionOp::Base64Encode,
            input_var: String::new(),
            output_var: "CONVERTED".into(),
            capture: false,
            encoding: "utf8".into(),
            endianness: "big".into(),
            byte_count: 4,
            from_type: String::new(),
            to_type: String::new(),
        }
    }
}

// ── Date Function ──

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DateFunctionSettings {
    pub function_type: DateFnType,
    pub input_var: String,
    pub output_var: String,
    pub format: String,
    pub amount: i64,
    pub unit: String,
    pub capture: bool,
    /// Extra parameter: expression string (Compute), decimal places (Round), etc.
    #[serde(default)]
    pub param: String,
}

impl Default for DateFunctionSettings {
    fn default() -> Self {
        Self {
            function_type: DateFnType::Now,
            input_var: String::new(),
            output_var: "DATE".into(),
            format: "%Y-%m-%d %H:%M:%S".into(),
            amount: 0,
            unit: "seconds".into(),
            capture: false,
            param: String::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DateFnType {
    Now,
    FormatDate,
    ParseDate,
    AddTime,
    SubtractTime,
    UnixTimestamp,
    UnixToDate,
    /// Current Unix timestamp in milliseconds
    CurrentUnixTimeMs,
    /// Evaluate a math/arithmetic expression with variable interpolation
    Compute,
    /// Round a number to N decimal places (param = decimal places, default 2)
    Round,
    /// Parse a date/datetime string to Unix timestamp in seconds
    DateToUnix,
    /// Parse a date/datetime string to Unix timestamp in milliseconds
    DateToUnixMs,
}

// ── Case / Switch ──

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaseSwitchSettings {
    pub input_var: String,
    pub cases: Vec<CaseBranch>,
    pub default_value: String,
    pub output_var: String,
    pub capture: bool,
}

impl Default for CaseSwitchSettings {
    fn default() -> Self {
        Self {
            input_var: "data.RESPONSECODE".into(),
            cases: vec![
                CaseBranch {
                    match_value: "200".into(),
                    result_value: "SUCCESS".into(),
                },
                CaseBranch {
                    match_value: "403".into(),
                    result_value: "BAN".into(),
                },
            ],
            default_value: "FAIL".into(),
            output_var: "RESULT".into(),
            capture: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaseBranch {
    pub match_value: String,
    pub result_value: String,
}

// ── Cookie Container (OpenBullet-style) ──

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CookieContainerSettings {
    /// Path to cookie file (Netscape format) or raw cookie text
    pub source: String,
    /// "file" or "text" — read from file path or use source as raw text
    #[serde(default = "default_cookie_source_type")]
    pub source_type: String,
    /// Domain to filter cookies by (empty = all)
    pub domain: String,
    /// Variable name to store extracted cookies
    pub output_var: String,
    /// Whether to capture as user variable
    pub capture: bool,
    /// Also store in Netscape format variable
    #[serde(default)]
    pub save_netscape: bool,
}

fn default_cookie_source_type() -> String {
    "text".to_string()
}

impl Default for CookieContainerSettings {
    fn default() -> Self {
        Self {
            source: String::new(),
            source_type: "text".into(),
            domain: String::new(),
            output_var: "COOKIES".into(),
            capture: false,
            save_netscape: false,
        }
    }
}

// ── Group (organizational container) ──

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroupSettings {
    #[serde(default)]
    pub blocks: Vec<Block>,
    #[serde(default = "default_true")]
    pub collapsed: bool,
}

fn default_true() -> bool {
    true
}

impl Default for GroupSettings {
    fn default() -> Self {
        Self {
            blocks: Vec::new(),
            collapsed: false,
        }
    }
}
