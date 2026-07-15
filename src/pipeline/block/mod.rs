mod settings_akamai;
mod settings_browser;
mod settings_bypass;
pub(crate) mod settings_check;
mod settings_control;
mod settings_data;
mod settings_extended_functions;
pub mod settings_functions;
mod settings_header_spoof;
mod settings_http;
mod settings_jwt;
mod settings_network;
mod settings_nudata;
mod settings_parse;
mod settings_protocol;

pub use settings_akamai::*;
pub use settings_browser::*;
pub use settings_bypass::*;
pub use settings_check::*;
pub use settings_control::*;
pub use settings_data::*;
pub use settings_extended_functions::*;
pub use settings_functions::*;
pub use settings_header_spoof::*;
pub use settings_http::*;
pub use settings_jwt::*;
pub use settings_network::*;
pub use settings_nudata::NuDataSensorSettings;
pub use settings_parse::*;
pub use settings_protocol::*;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Deserialize a UUID that tolerates empty strings (maps to nil UUID)
fn deserialize_uuid_tolerant<'de, D>(deserializer: D) -> Result<Uuid, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;
    if s.is_empty() {
        Ok(Uuid::nil())
    } else {
        Uuid::parse_str(&s).map_err(serde::de::Error::custom)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Block {
    #[serde(deserialize_with = "deserialize_uuid_tolerant")]
    pub id: Uuid,
    pub block_type: BlockType,
    pub label: String,
    pub disabled: bool,
    pub safe_mode: bool,
    pub settings: BlockSettings,
}

impl Block {
    pub fn new(block_type: BlockType) -> Self {
        let label = block_type.default_label().to_string();
        let settings = block_type.default_settings();
        Self {
            id: Uuid::new_v4(),
            block_type,
            label,
            disabled: false,
            safe_mode: false,
            settings,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BlockType {
    HttpRequest,
    ParseLR,
    ParseRegex,
    ParseJSON,
    ParseCSS,
    ParseXPath,
    ParseCookie,
    Parse,
    KeyCheck,
    StringFunction,
    Translate,
    ListFunction,
    CryptoFunction,
    ConversionFunction,
    IfElse,
    Loop,
    Delay,
    Script,
    Log,
    SetVariable,
    ClearCookies,
    // Networking
    Webhook,
    WebSocket,
    // Protocol requests
    TcpRequest,
    UdpRequest,
    FtpRequest,
    SshRequest,
    ImapRequest,
    SmtpRequest,
    PopRequest,
    // Bypass / Anti-bot
    CaptchaSolver,
    CloudflareBypass,
    LaravelCsrf,
    // Data management
    DateFunction,
    CaseSwitch,
    CookieContainer,
    // Browser automation
    BrowserOpen,
    NavigateTo,
    ClickElement,
    TypeText,
    WaitForElement,
    GetElementText,
    GetDom,
    Screenshot,
    ExecuteJs,
    // New blocks
    RandomUserAgent,
    OcrCaptcha,
    RecaptchaInvisible,
    XacfSensor,
    // Phase 1-4 blocks
    RandomData,
    DataDomeSensor,
    Plugin,
    AkamaiV3Sensor,
    // Organization
    Group,
    // Extended functions
    ByteArray,
    // File system
    FileSystem,
    Constants,
    Dictionary,
    FloatFunction,
    IntegerFunction,
    TimeFunction,
    GenerateGUID,
    PhoneCountry,
    LambdaParser,
    // Security / Auth utilities
    JwtToken,
    HeaderSpoof,
    NuDataSensor,
}

impl BlockType {
    pub fn default_label(&self) -> &'static str {
        match self {
            Self::HttpRequest => "HTTP Request",
            Self::ParseLR => "Parse LR",
            Self::ParseRegex => "Parse Regex",
            Self::ParseJSON => "Parse JSON",
            Self::ParseCSS => "Parse CSS",
            Self::ParseXPath => "Parse XPath",
            Self::ParseCookie => "Parse Cookie",
            Self::Parse => "Parse",
            Self::KeyCheck => "Key Check",
            Self::StringFunction => "String Function",
            Self::Translate => "Translate",
            Self::ListFunction => "List Function",
            Self::CryptoFunction => "Crypto Function",
            Self::ConversionFunction => "Conversion",
            Self::IfElse => "If / Else",
            Self::Loop => "Loop",
            Self::Delay => "Delay",
            Self::Script => "Script",
            Self::Log => "Log",
            Self::SetVariable => "Set Variable",
            Self::ClearCookies => "Clear Cookies",
            Self::Webhook => "Webhook",
            Self::WebSocket => "WebSocket",
            Self::TcpRequest => "TCP Request",
            Self::UdpRequest => "UDP Request",
            Self::FtpRequest => "FTP Request",
            Self::SshRequest => "SSH Request",
            Self::ImapRequest => "IMAP Request",
            Self::SmtpRequest => "SMTP Request",
            Self::PopRequest => "POP Request",
            Self::CaptchaSolver => "Captcha Solver",
            Self::CloudflareBypass => "Cloudflare Bypass",
            Self::LaravelCsrf => "Laravel CSRF",
            Self::DateFunction => "Date Function",
            Self::CaseSwitch => "Case / Switch",
            Self::CookieContainer => "Cookie Container",
            Self::BrowserOpen => "Browser Open",
            Self::NavigateTo => "Navigate To",
            Self::ClickElement => "Click Element",
            Self::TypeText => "Type Text",
            Self::WaitForElement => "Wait For Element",
            Self::GetElementText => "Get Element Text",
            Self::GetDom => "Get DOM",
            Self::Screenshot => "Screenshot",
            Self::ExecuteJs => "Execute JS",
            Self::RandomUserAgent => "Random User Agent",
            Self::OcrCaptcha => "OCR Captcha",
            Self::RecaptchaInvisible => "reCAPTCHA Invisible",
            Self::XacfSensor => "XACF Sensor",
            Self::RandomData => "Random Data",
            Self::DataDomeSensor => "DataDome Sensor",
            Self::Plugin => "Plugin Block",
            Self::AkamaiV3Sensor => "Akamai V3 Sensor",
            Self::Group => "Group",
            Self::ByteArray => "Byte Array",
            Self::Constants => "Constants",
            Self::Dictionary => "Dictionary",
            Self::FloatFunction => "Float Function",
            Self::IntegerFunction => "Integer Function",
            Self::TimeFunction => "Time Function",
            Self::GenerateGUID => "Generate GUID",
            Self::PhoneCountry => "Phone Country",
            Self::LambdaParser => "Lambda Parser",
            Self::FileSystem => "File System",
            Self::JwtToken => "JWT Token",
            Self::HeaderSpoof => "Header Spoof",
            Self::NuDataSensor => "NuData Sensor",
        }
    }

    pub fn category(&self) -> &'static str {
        match self {
            Self::HttpRequest
            | Self::TcpRequest
            | Self::UdpRequest
            | Self::FtpRequest
            | Self::SshRequest
            | Self::ImapRequest
            | Self::SmtpRequest
            | Self::PopRequest => "Requests",
            Self::CaptchaSolver
            | Self::CloudflareBypass
            | Self::LaravelCsrf
            | Self::OcrCaptcha
            | Self::RecaptchaInvisible => "Bypass",
            Self::XacfSensor | Self::DataDomeSensor | Self::AkamaiV3Sensor => "Sensors",
            Self::RandomUserAgent | Self::RandomData | Self::Plugin => "Utilities",
            Self::ParseLR
            | Self::ParseRegex
            | Self::ParseJSON
            | Self::ParseCSS
            | Self::ParseXPath
            | Self::ParseCookie
            | Self::LambdaParser
            | Self::Parse => "Parsing",
            Self::KeyCheck => "Checks",
            Self::StringFunction
            | Self::Translate
            | Self::ListFunction
            | Self::CryptoFunction
            | Self::ConversionFunction
            | Self::DateFunction
            | Self::CookieContainer
            | Self::ByteArray
            | Self::Constants
            | Self::Dictionary
            | Self::FloatFunction
            | Self::IntegerFunction
            | Self::TimeFunction
            | Self::GenerateGUID
            | Self::PhoneCountry => "Functions",
            Self::FileSystem => "FileSystem",
            Self::IfElse
            | Self::Loop
            | Self::Delay
            | Self::Script
            | Self::CaseSwitch
            | Self::Group => "Control",
            Self::Log
            | Self::SetVariable
            | Self::ClearCookies
            | Self::Webhook
            | Self::WebSocket => "Utilities",
            Self::BrowserOpen
            | Self::NavigateTo
            | Self::ClickElement
            | Self::TypeText
            | Self::WaitForElement
            | Self::GetElementText
            | Self::GetDom
            | Self::Screenshot
            | Self::ExecuteJs => "Browser",
            Self::JwtToken | Self::HeaderSpoof | Self::NuDataSensor => "Security",
        }
    }

    pub fn color(&self) -> &'static str {
        match self.category() {
            "Requests" => "#0078d4",
            "Parsing" => "#4ec9b0",
            "Checks" => "#d7ba7d",
            "Functions" => "#c586c0",
            "Control" => "#dcdcaa",
            "Utilities" => "#858585",
            "Bypass" => "#e5c07b",
            "Browser" => "#e06c75",
            "Sensors" => "#2dd4bf",
            "Data" => "#d4a96a",
            "FileSystem" => "#d4a96a",
            "Security" => "#f97316",
            _ => "#858585",
        }
    }

    pub fn default_settings(&self) -> BlockSettings {
        match self {
            Self::HttpRequest => BlockSettings::HttpRequest(HttpRequestSettings::default()),
            Self::ParseLR => BlockSettings::ParseLR(ParseLRSettings::default()),
            Self::ParseRegex => BlockSettings::ParseRegex(ParseRegexSettings::default()),
            Self::ParseJSON => BlockSettings::ParseJSON(ParseJSONSettings::default()),
            Self::ParseCSS => BlockSettings::ParseCSS(ParseCSSSettings::default()),
            Self::ParseXPath => BlockSettings::ParseXPath(ParseXPathSettings::default()),
            Self::ParseCookie => BlockSettings::ParseCookie(ParseCookieSettings::default()),
            Self::Parse => BlockSettings::Parse(ParseSettings::default()),
            Self::KeyCheck => BlockSettings::KeyCheck(KeyCheckSettings::default()),
            Self::StringFunction => {
                BlockSettings::StringFunction(StringFunctionSettings::default())
            }
            Self::Translate => BlockSettings::Translate(TranslateSettings::default()),
            Self::ListFunction => BlockSettings::ListFunction(ListFunctionSettings::default()),
            Self::CryptoFunction => {
                BlockSettings::CryptoFunction(CryptoFunctionSettings::default())
            }
            Self::ConversionFunction => {
                BlockSettings::ConversionFunction(ConversionFunctionSettings::default())
            }
            Self::IfElse => BlockSettings::IfElse(IfElseSettings::default()),
            Self::Loop => BlockSettings::Loop(LoopSettings::default()),
            Self::Delay => BlockSettings::Delay(DelaySettings::default()),
            Self::Script => BlockSettings::Script(ScriptSettings::default()),
            Self::Log => BlockSettings::Log(LogSettings::default()),
            Self::SetVariable => BlockSettings::SetVariable(SetVariableSettings::default()),
            Self::ClearCookies => BlockSettings::ClearCookies,
            Self::Webhook => BlockSettings::Webhook(WebhookSettings::default()),
            Self::WebSocket => BlockSettings::WebSocket(WebSocketSettings::default()),
            Self::TcpRequest => BlockSettings::TcpRequest(TcpRequestSettings::default()),
            Self::UdpRequest => BlockSettings::UdpRequest(UdpRequestSettings::default()),
            Self::FtpRequest => BlockSettings::FtpRequest(FtpRequestSettings::default()),
            Self::SshRequest => BlockSettings::SshRequest(SshRequestSettings::default()),
            Self::ImapRequest => BlockSettings::ImapRequest(ImapRequestSettings::default()),
            Self::SmtpRequest => BlockSettings::SmtpRequest(SmtpRequestSettings::default()),
            Self::PopRequest => BlockSettings::PopRequest(PopRequestSettings::default()),
            Self::CaptchaSolver => BlockSettings::CaptchaSolver(CaptchaSolverSettings::default()),
            Self::CloudflareBypass => {
                BlockSettings::CloudflareBypass(CloudflareBypassSettings::default())
            }
            Self::LaravelCsrf => BlockSettings::LaravelCsrf(LaravelCsrfSettings::default()),
            Self::DateFunction => BlockSettings::DateFunction(DateFunctionSettings::default()),
            Self::CaseSwitch => BlockSettings::CaseSwitch(CaseSwitchSettings::default()),
            Self::CookieContainer => {
                BlockSettings::CookieContainer(CookieContainerSettings::default())
            }
            Self::BrowserOpen => BlockSettings::BrowserOpen(BrowserOpenSettings::default()),
            Self::NavigateTo => BlockSettings::NavigateTo(NavigateToSettings::default()),
            Self::ClickElement => BlockSettings::ClickElement(ClickElementSettings::default()),
            Self::TypeText => BlockSettings::TypeText(TypeTextSettings::default()),
            Self::WaitForElement => {
                BlockSettings::WaitForElement(WaitForElementSettings::default())
            }
            Self::GetElementText => {
                BlockSettings::GetElementText(GetElementTextSettings::default())
            }
            Self::GetDom => BlockSettings::GetDom(GetDomSettings::default()),
            Self::Screenshot => BlockSettings::Screenshot(ScreenshotSettings::default()),
            Self::ExecuteJs => BlockSettings::ExecuteJs(ExecuteJsSettings::default()),
            Self::RandomUserAgent => {
                BlockSettings::RandomUserAgent(RandomUserAgentSettings::default())
            }
            Self::OcrCaptcha => BlockSettings::OcrCaptcha(OcrCaptchaSettings::default()),
            Self::RecaptchaInvisible => {
                BlockSettings::RecaptchaInvisible(RecaptchaInvisibleSettings::default())
            }
            Self::XacfSensor => BlockSettings::XacfSensor(XacfSensorSettings::default()),
            Self::RandomData => BlockSettings::RandomData(RandomDataSettings::default()),
            Self::DataDomeSensor => {
                BlockSettings::DataDomeSensor(DataDomeSensorSettings::default())
            }
            Self::Plugin => BlockSettings::Plugin(PluginBlockSettings::default()),
            Self::AkamaiV3Sensor => {
                BlockSettings::AkamaiV3Sensor(AkamaiV3SensorSettings::default())
            }
            Self::Group => BlockSettings::Group(GroupSettings::default()),
            Self::ByteArray => BlockSettings::ByteArray(ByteArraySettings::default()),
            Self::Constants => BlockSettings::Constants(ConstantsSettings::default()),
            Self::Dictionary => BlockSettings::Dictionary(DictionarySettings::default()),
            Self::FloatFunction => BlockSettings::FloatFunction(FloatFunctionSettings::default()),
            Self::IntegerFunction => {
                BlockSettings::IntegerFunction(IntegerFunctionSettings::default())
            }
            Self::TimeFunction => BlockSettings::TimeFunction(TimeFunctionSettings::default()),
            Self::GenerateGUID => BlockSettings::GenerateGUID(GenerateGUIDSettings::default()),
            Self::PhoneCountry => BlockSettings::PhoneCountry(PhoneCountrySettings::default()),
            Self::LambdaParser => BlockSettings::LambdaParser(LambdaParserSettings::default()),
            Self::FileSystem => BlockSettings::FileSystem(FileSystemSettings::default()),
            Self::JwtToken => BlockSettings::JwtToken(JwtSettings::default()),
            Self::HeaderSpoof => BlockSettings::HeaderSpoof(HeaderSpoofSettings::default()),
            Self::NuDataSensor => BlockSettings::NuDataSensor(NuDataSensorSettings::default()),
        }
    }
}

// ── Block Settings (type-specific) ──

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum BlockSettings {
    HttpRequest(HttpRequestSettings),
    ParseLR(ParseLRSettings),
    ParseRegex(ParseRegexSettings),
    ParseJSON(ParseJSONSettings),
    ParseCSS(ParseCSSSettings),
    ParseXPath(ParseXPathSettings),
    ParseCookie(ParseCookieSettings),
    Parse(ParseSettings),
    KeyCheck(KeyCheckSettings),
    StringFunction(StringFunctionSettings),
    Translate(TranslateSettings),
    ListFunction(ListFunctionSettings),
    CryptoFunction(CryptoFunctionSettings),
    ConversionFunction(ConversionFunctionSettings),
    IfElse(IfElseSettings),
    Loop(LoopSettings),
    Delay(DelaySettings),
    Script(ScriptSettings),
    Log(LogSettings),
    SetVariable(SetVariableSettings),
    ClearCookies,
    // Networking
    Webhook(WebhookSettings),
    WebSocket(WebSocketSettings),
    // Protocol requests
    TcpRequest(TcpRequestSettings),
    UdpRequest(UdpRequestSettings),
    FtpRequest(FtpRequestSettings),
    SshRequest(SshRequestSettings),
    ImapRequest(ImapRequestSettings),
    SmtpRequest(SmtpRequestSettings),
    PopRequest(PopRequestSettings),
    // Bypass / Anti-bot
    CaptchaSolver(CaptchaSolverSettings),
    CloudflareBypass(CloudflareBypassSettings),
    LaravelCsrf(LaravelCsrfSettings),
    // Data management
    DateFunction(DateFunctionSettings),
    CaseSwitch(CaseSwitchSettings),
    CookieContainer(CookieContainerSettings),
    // Browser automation
    BrowserOpen(BrowserOpenSettings),
    NavigateTo(NavigateToSettings),
    ClickElement(ClickElementSettings),
    TypeText(TypeTextSettings),
    WaitForElement(WaitForElementSettings),
    GetElementText(GetElementTextSettings),
    GetDom(GetDomSettings),
    Screenshot(ScreenshotSettings),
    ExecuteJs(ExecuteJsSettings),
    // New blocks
    RandomUserAgent(RandomUserAgentSettings),
    OcrCaptcha(OcrCaptchaSettings),
    RecaptchaInvisible(RecaptchaInvisibleSettings),
    XacfSensor(XacfSensorSettings),
    // Phase 1-4 blocks
    RandomData(RandomDataSettings),
    DataDomeSensor(DataDomeSensorSettings),
    Plugin(PluginBlockSettings),
    AkamaiV3Sensor(AkamaiV3SensorSettings),
    // Organization
    Group(GroupSettings),
    // Extended functions
    ByteArray(ByteArraySettings),
    Constants(ConstantsSettings),
    Dictionary(DictionarySettings),
    FloatFunction(FloatFunctionSettings),
    IntegerFunction(IntegerFunctionSettings),
    TimeFunction(TimeFunctionSettings),
    GenerateGUID(GenerateGUIDSettings),
    PhoneCountry(PhoneCountrySettings),
    LambdaParser(LambdaParserSettings),
    FileSystem(FileSystemSettings),
    JwtToken(JwtSettings),
    HeaderSpoof(HeaderSpoofSettings),
    NuDataSensor(NuDataSensorSettings),
}
