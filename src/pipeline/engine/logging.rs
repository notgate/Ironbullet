use super::*;
use helpers::truncate_display;

impl ExecutionContext {
    pub(super) fn block_execution_log(&self, block: &Block) -> String {
        match &block.settings {
            BlockSettings::HttpRequest(_) => {
                if let Some(br) = self.block_results.last() {
                    let method = br
                        .request
                        .as_ref()
                        .map(|r| r.method.as_str())
                        .unwrap_or("?");
                    let url = br
                        .request
                        .as_ref()
                        .map(|r| truncate_display(&r.url, 60))
                        .unwrap_or_else(|| "?".into());
                    let status = br.response.as_ref().map(|r| r.status_code).unwrap_or(0);
                    let timing = br.response.as_ref().map(|r| r.timing_ms).unwrap_or(0);
                    format!("{} {} → {} ({}ms)", method, url, status, timing)
                } else {
                    "HTTP request completed".into()
                }
            }
            BlockSettings::ParseLR(s) => {
                let val = self.variables.get(&s.output_var).unwrap_or_default();
                format!("{} = {}", s.output_var, truncate_display(&val, 60))
            }
            BlockSettings::ParseRegex(s) => {
                let val = self.variables.get(&s.output_var).unwrap_or_default();
                format!("{} = {}", s.output_var, truncate_display(&val, 60))
            }
            BlockSettings::ParseJSON(s) => {
                let val = self.variables.get(&s.output_var).unwrap_or_default();
                format!("{} = {}", s.output_var, truncate_display(&val, 60))
            }
            BlockSettings::ParseCSS(s) => {
                let val = self.variables.get(&s.output_var).unwrap_or_default();
                format!("{} = {}", s.output_var, truncate_display(&val, 60))
            }
            BlockSettings::ParseXPath(s) => {
                let val = self.variables.get(&s.output_var).unwrap_or_default();
                format!("{} = {}", s.output_var, truncate_display(&val, 60))
            }
            BlockSettings::ParseCookie(s) => {
                let val = self.variables.get(&s.output_var).unwrap_or_default();
                format!("{} = {}", s.output_var, truncate_display(&val, 60))
            }
            BlockSettings::Parse(s) => {
                let val = self.variables.get(&s.output_var).unwrap_or_default();
                format!(
                    "{:?}({}) → {} = {}",
                    s.parse_mode,
                    s.input_var,
                    s.output_var,
                    truncate_display(&val, 50)
                )
            }
            BlockSettings::KeyCheck(_) => {
                format!("status → {:?}", self.status)
            }
            BlockSettings::StringFunction(s) => {
                let val = self.variables.get(&s.output_var).unwrap_or_default();
                format!(
                    "{:?}({}) → {} = {}",
                    s.function_type,
                    s.input_var,
                    s.output_var,
                    truncate_display(&val, 40)
                )
            }
            BlockSettings::Translate(s) => {
                let val = self.variables.get(&s.output_var).unwrap_or_default();
                format!(
                    "{} → {} = {}",
                    s.input_var,
                    s.output_var,
                    truncate_display(&val, 40)
                )
            }
            BlockSettings::CryptoFunction(s) => {
                let val = self.variables.get(&s.output_var).unwrap_or_default();
                format!(
                    "{:?}({}) → {} = {}",
                    s.function_type,
                    s.input_var,
                    s.output_var,
                    truncate_display(&val, 40)
                )
            }
            BlockSettings::ListFunction(s) => {
                let val = self.variables.get(&s.output_var).unwrap_or_default();
                format!(
                    "{:?}({}) → {} = {}",
                    s.function_type,
                    s.input_var,
                    s.output_var,
                    truncate_display(&val, 40)
                )
            }
            BlockSettings::ConversionFunction(s) => {
                let val = self.variables.get(&s.output_var).unwrap_or_default();
                format!(
                    "{:?}({}) → {} = {}",
                    s.op,
                    s.input_var,
                    s.output_var,
                    truncate_display(&val, 40)
                )
            }
            BlockSettings::DateFunction(s) => {
                let val = self.variables.get(&s.output_var).unwrap_or_default();
                format!(
                    "{:?} → {} = {}",
                    s.function_type,
                    s.output_var,
                    truncate_display(&val, 40)
                )
            }
            BlockSettings::CaseSwitch(s) => {
                let val = self.variables.get(&s.output_var).unwrap_or_default();
                format!("switch {} → {} = {}", s.input_var, s.output_var, val)
            }
            BlockSettings::CookieContainer(s) => {
                let val = self.variables.get(&s.output_var).unwrap_or_default();
                let count = val.matches(';').count() + if val.is_empty() { 0 } else { 1 };
                format!("{} cookies → {}", count, s.output_var)
            }
            BlockSettings::IfElse(_) => "condition evaluated, branch executed".into(),
            BlockSettings::Loop(s) => match s.loop_type {
                LoopType::ForEach => format!("foreach {} ({} blocks)", s.item_var, s.blocks.len()),
                LoopType::Repeat => format!("repeat {}x ({} blocks)", s.count, s.blocks.len()),
            },
            BlockSettings::Delay(s) => {
                if s.min_ms == s.max_ms {
                    format!("{}ms", s.min_ms)
                } else {
                    format!("{}-{}ms", s.min_ms, s.max_ms)
                }
            }
            BlockSettings::SetVariable(s) => {
                let val = self.variables.get(&s.name).unwrap_or_default();
                format!("{} = {}", s.name, truncate_display(&val, 60))
            }
            BlockSettings::Log(_) => String::new(), // Already logged by handler
            BlockSettings::ClearCookies => "session cookies cleared".into(),
            BlockSettings::Script(s) => {
                let val = self.variables.get(&s.output_var).unwrap_or_default();
                format!("{} = {}", s.output_var, truncate_display(&val, 60))
            }
            // Browser
            BlockSettings::BrowserOpen(s) => {
                format!(
                    "launched {} {}",
                    s.browser_type,
                    if s.headless { "(headless)" } else { "(headed)" }
                )
            }
            BlockSettings::NavigateTo(s) => {
                format!("navigated to {}", truncate_display(&s.url, 60))
            }
            BlockSettings::ClickElement(s) => format!("clicked {}", s.selector),
            BlockSettings::TypeText(s) => format!("typed into {}", s.selector),
            BlockSettings::WaitForElement(s) => format!("waited for {} [{}]", s.selector, s.state),
            BlockSettings::GetElementText(s) => {
                let val = self.variables.get(&s.output_var).unwrap_or_default();
                format!("{} = {}", s.output_var, truncate_display(&val, 60))
            }
            BlockSettings::GetDom(s) => {
                let val = self.variables.get(&s.output_var).unwrap_or_default();
                format!("{} = {}", s.output_var, truncate_display(&val, 60))
            }
            BlockSettings::Screenshot(s) => {
                if s.full_page {
                    "full page screenshot".into()
                } else if !s.selector.is_empty() {
                    format!("screenshot of {}", s.selector)
                } else {
                    "viewport screenshot".into()
                }
            }
            BlockSettings::ExecuteJs(s) => {
                let val = self.variables.get(&s.output_var).unwrap_or_default();
                format!("{} = {}", s.output_var, truncate_display(&val, 60))
            }
            // Networking
            BlockSettings::Webhook(s) => format!("{} {}", s.method, truncate_display(&s.url, 60)),
            BlockSettings::WebSocket(s) => format!("{} {}", s.action, truncate_display(&s.url, 60)),
            // Protocol
            BlockSettings::TcpRequest(s) => format!("{}:{}", s.host, s.port),
            BlockSettings::UdpRequest(s) => format!("{}:{}", s.host, s.port),
            BlockSettings::FtpRequest(s) => format!("{}:{} {}", s.host, s.port, s.command),
            BlockSettings::SshRequest(s) => format!("{}:{}", s.host, s.port),
            BlockSettings::ImapRequest(s) => format!("{}:{} {}", s.host, s.port, s.command),
            BlockSettings::SmtpRequest(s) => format!("{}:{} {}", s.host, s.port, s.command),
            BlockSettings::PopRequest(s) => format!("{}:{} {}", s.host, s.port, s.command),
            // Bypass
            BlockSettings::CaptchaSolver(s) => format!("{} {}", s.solver_service, s.captcha_type),
            BlockSettings::CloudflareBypass(s) => format!("{}", truncate_display(&s.url, 60)),
            BlockSettings::LaravelCsrf(s) => {
                format!("{} → {}", truncate_display(&s.url, 40), s.output_var)
            }
            // New blocks
            BlockSettings::RandomUserAgent(s) => {
                let val = self.variables.get(&s.output_var).unwrap_or_default();
                format!("{} = {}", s.output_var, truncate_display(&val, 60))
            }
            BlockSettings::OcrCaptcha(s) => {
                let val = self.variables.get(&s.output_var).unwrap_or_default();
                format!("{} = {}", s.output_var, truncate_display(&val, 60))
            }
            BlockSettings::RecaptchaInvisible(s) => {
                let val = self.variables.get(&s.output_var).unwrap_or_default();
                format!("{} = {}", s.output_var, truncate_display(&val, 40))
            }
            BlockSettings::XacfSensor(s) => {
                let val = self.variables.get(&s.output_var).unwrap_or_default();
                format!("{} → {} ({}b)", s.bundle_id, s.output_var, val.len())
            }
            BlockSettings::RandomData(s) => {
                let val = self.variables.get(&s.output_var).unwrap_or_default();
                format!(
                    "{:?} → {} = {}",
                    s.data_type,
                    s.output_var,
                    truncate_display(&val, 60)
                )
            }
            BlockSettings::DataDomeSensor(s) => {
                let val = self.variables.get(&s.output_var).unwrap_or_default();
                format!(
                    "{} → {} ({}b)",
                    truncate_display(&s.site_url, 30),
                    s.output_var,
                    val.len()
                )
            }
            BlockSettings::Plugin(s) => {
                let val = self.variables.get(&s.output_var).unwrap_or_default();
                if val.is_empty() {
                    format!("plugin: {} (no output)", s.plugin_block_type)
                } else {
                    format!("{} = {}", s.output_var, truncate_display(&val, 60))
                }
            }
            BlockSettings::AkamaiV3Sensor(s) => {
                let val = self.variables.get(&s.output_var).unwrap_or_default();
                format!("{:?} → {} ({}b)", s.mode, s.output_var, val.len())
            }
            BlockSettings::Group(s) => {
                format!(
                    "{} block{}",
                    s.blocks.len(),
                    if s.blocks.len() != 1 { "s" } else { "" }
                )
            }
            // Extended functions
            BlockSettings::ByteArray(s) => {
                let val = self.variables.get(&s.output_var).unwrap_or_default();
                format!(
                    "{:?}({}) → {} = {}",
                    s.operation,
                    s.input_var,
                    s.output_var,
                    truncate_display(&val, 40)
                )
            }
            BlockSettings::Constants(s) => {
                format!(
                    "{} constant{} defined",
                    s.constants.len(),
                    if s.constants.len() != 1 { "s" } else { "" }
                )
            }
            BlockSettings::Dictionary(s) => {
                let val = self.variables.get(&s.output_var).unwrap_or_default();
                format!(
                    "{:?}({}) → {} = {}",
                    s.operation,
                    s.dict_var,
                    s.output_var,
                    truncate_display(&val, 40)
                )
            }
            BlockSettings::FloatFunction(s) => {
                let val = self.variables.get(&s.output_var).unwrap_or_default();
                format!(
                    "{:?}({}) → {} = {}",
                    s.function_type,
                    s.input_var,
                    s.output_var,
                    truncate_display(&val, 40)
                )
            }
            BlockSettings::IntegerFunction(s) => {
                let val = self.variables.get(&s.output_var).unwrap_or_default();
                format!(
                    "{:?}({}) → {} = {}",
                    s.function_type,
                    s.input_var,
                    s.output_var,
                    truncate_display(&val, 40)
                )
            }
            BlockSettings::TimeFunction(s) => {
                let val = self.variables.get(&s.output_var).unwrap_or_default();
                format!(
                    "{:?} → {} = {}",
                    s.function_type,
                    s.output_var,
                    truncate_display(&val, 40)
                )
            }
            BlockSettings::GenerateGUID(s) => {
                let val = self.variables.get(&s.output_var).unwrap_or_default();
                format!(
                    "{:?} → {} = {}",
                    s.guid_version,
                    s.output_var,
                    truncate_display(&val, 40)
                )
            }
            BlockSettings::PhoneCountry(s) => {
                let val = self.variables.get(&s.output_var).unwrap_or_default();
                format!(
                    "{:?}({}) → {} = {}",
                    s.output_format,
                    s.input_var,
                    s.output_var,
                    truncate_display(&val, 40)
                )
            }
            BlockSettings::LambdaParser(s) => {
                let val = self.variables.get(&s.output_var).unwrap_or_default();
                format!("{} = {}", s.output_var, truncate_display(&val, 60))
            }
            BlockSettings::FileSystem(s) => {
                let val = self.variables.get(&s.output_var).unwrap_or_default();
                let out = if val.is_empty() {
                    String::new()
                } else {
                    format!(" → {} = {}", s.output_var, truncate_display(&val, 40))
                };
                format!("{:?}({}){}", s.op, s.path, out)
            }
            BlockSettings::JwtToken(s) => {
                let action = if matches!(s.action, JwtAction::Sign) {
                    "Sign"
                } else {
                    "Decode"
                };
                let val = self.variables.get(&s.output_var).unwrap_or_default();
                format!(
                    "JWT {} → {} = {}",
                    action,
                    s.output_var,
                    truncate_display(&val, 40)
                )
            }
            BlockSettings::HeaderSpoof(_) => {
                let ip = self.variables.get("SPOOF_IP").unwrap_or_default();
                format!(
                    "HeaderSpoof → SPOOF_IP = {}",
                    if ip.is_empty() {
                        "<pending>".to_string()
                    } else {
                        ip
                    }
                )
            }
            BlockSettings::NuDataSensor(s) => {
                let val = self.variables.get(&s.output_var).unwrap_or_default();
                format!(
                    "NuDataSensor → {} = {}",
                    s.output_var,
                    truncate_display(&val, 40)
                )
            }
        }
    }
}
