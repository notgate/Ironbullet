mod browser;
mod bypass;
mod data;
mod functions;
mod header_spoof;
mod helpers;
mod http;
mod jwt;
mod logging;
mod nudata;
pub mod parsers;

#[cfg(test)]
mod http_tests;
mod protocol;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

// These need `pub(crate)` so child modules can access them via `use super::*`
pub(crate) use super::block::*;
pub(crate) use super::datadome;
pub(crate) use super::random_data;
pub(crate) use super::tls_profiles::TLS_PROFILES;
pub(crate) use super::variable::VariableStore;
pub(crate) use super::BotStatus;
pub(crate) use crate::sidecar::protocol::{SidecarRequest, SidecarResponse};

use helpers::elapsed_ms;

// ── RustTLS client slot — tracks proxy for proper cache invalidation ──
pub struct RustlsClientSlot {
    pub client: reqwest::Client,
    pub proxy: Option<String>,
}

impl std::fmt::Debug for RustlsClientSlot {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "RustlsClientSlot(proxy={:?})", self.proxy)
    }
}

// ── WreqTLS client slot — Unix only (BoringSSL cross-compile for Windows unsupported) ──
#[cfg(feature = "wreq-tls")]
pub struct WreqClientSlot {
    pub client: wreq::Client,
    pub emulation: String,
    pub proxy: Option<String>,
}

#[cfg(feature = "wreq-tls")]
impl std::fmt::Debug for WreqClientSlot {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "WreqClientSlot({})", self.emulation)
    }
}

// ── Browser handle wrappers (not serializable/cloneable) ──

#[derive(Default)]
pub struct BrowserHandle(pub Option<chromiumoxide::Browser>);

#[derive(Default)]
pub struct PageHandle(pub Option<chromiumoxide::Page>);

impl Clone for BrowserHandle {
    fn clone(&self) -> Self {
        Self(None)
    }
}
impl Clone for PageHandle {
    fn clone(&self) -> Self {
        Self(None)
    }
}
impl std::fmt::Debug for BrowserHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(
            f,
            "BrowserHandle({})",
            if self.0.is_some() { "active" } else { "none" }
        )
    }
}
impl std::fmt::Debug for PageHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(
            f,
            "PageHandle({})",
            if self.0.is_some() { "active" } else { "none" }
        )
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ExecutionContext {
    pub variables: VariableStore,
    pub status: BotStatus,
    pub session_id: String,
    pub proxy: Option<String>,
    pub log: Vec<LogEntry>,
    pub block_results: Vec<BlockResult>,
    pub network_log: Vec<NetworkEntry>,
    #[serde(skip)]
    pub browser: BrowserHandle,
    #[serde(skip)]
    pub page: PageHandle,
    #[serde(skip)]
    pub override_ja3: Option<String>,
    #[serde(skip)]
    pub override_http2fp: Option<String>,
    #[serde(skip)]
    pub plugin_manager: Option<std::sync::Arc<crate::plugin::manager::PluginManager>>,
    /// Optional override for the Chrome/Chromium executable used by BrowserOpen blocks.
    /// Set from GuiConfig.chrome_executable_path at job start time.
    #[serde(skip)]
    pub chrome_executable_path: Option<std::path::PathBuf>,
    /// Handle to the Chrome child process spawned by BrowserOpen.
    /// Kept alive so Chrome stays running while the pipeline runs;
    /// dropped (and Chrome killed) when ExecutionContext is dropped.
    #[serde(skip)]
    pub chrome_child: Option<std::process::Child>,
    /// Reusable reqwest client for RustTLS requests — persists cookie jar between
    /// HTTP blocks within a single pipeline execution so multi-step login flows work.
    /// Reset for every new ExecutionContext (i.e. every credential checked).
    /// Wrapped in RustlsClientSlot to track the proxy and invalidate on change.
    #[serde(skip)]
    pub rustls_client: Option<RustlsClientSlot>,
    /// Reusable wreq client for WreqTLS requests — Unix only (BoringSSL).
    #[cfg(feature = "wreq-tls")]
    #[serde(skip)]
    pub wreq_client: Option<WreqClientSlot>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkEntry {
    pub block_id: Uuid,
    pub block_label: String,
    pub method: String,
    pub url: String,
    pub status_code: u16,
    pub timing_ms: u64,
    pub response_size: usize,
    pub cookies_set: Vec<(String, String)>,
    pub cookies_sent: Vec<(String, String)>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry {
    pub timestamp_ms: u64,
    pub block_id: Uuid,
    pub block_label: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockResult {
    pub block_id: Uuid,
    pub block_label: String,
    pub block_type: BlockType,
    pub success: bool,
    pub timing_ms: u64,
    pub variables_after: std::collections::HashMap<String, String>,
    pub log_message: String,
    // HTTP-specific
    pub request: Option<RequestInfo>,
    pub response: Option<ResponseInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequestInfo {
    pub method: String,
    pub url: String,
    pub headers: Vec<(String, String)>,
    pub body: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResponseInfo {
    pub status_code: u16,
    pub headers: std::collections::HashMap<String, String>,
    pub body: String,
    pub final_url: String,
    pub cookies: std::collections::HashMap<String, String>,
    pub timing_ms: u64,
}

impl ExecutionContext {
    pub fn new(session_id: String) -> Self {
        Self {
            variables: VariableStore::new(),
            status: BotStatus::None,
            session_id,
            proxy: None,
            log: Vec::new(),
            block_results: Vec::new(),
            network_log: Vec::new(),
            browser: BrowserHandle(None),
            page: PageHandle(None),
            override_ja3: None,
            override_http2fp: None,
            plugin_manager: None,
            chrome_executable_path: None,
            chrome_child: None,
            rustls_client: None,
            #[cfg(feature = "wreq-tls")]
            wreq_client: None,
        }
    }

    /// Execute a list of blocks sequentially.
    /// Returns after all blocks are run or status changes to non-None.
    pub fn execute_blocks<'a>(
        &'a mut self,
        blocks: &'a [Block],
        sidecar_tx: &'a tokio::sync::mpsc::Sender<(
            SidecarRequest,
            tokio::sync::oneshot::Sender<SidecarResponse>,
        )>,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = crate::error::Result<()>> + Send + 'a>>
    {
        Box::pin(async move {
            for block in blocks {
                if block.disabled {
                    continue;
                }

                // Push BlockResult BEFORE execution so execute_http_request's
                // last_mut() points to the correct entry for this block
                self.block_results.push(BlockResult {
                    block_id: block.id,
                    block_label: block.label.clone(),
                    block_type: block.block_type,
                    success: true,
                    timing_ms: 0,
                    variables_after: std::collections::HashMap::new(),
                    log_message: String::new(),
                    request: None,
                    response: None,
                });

                let start = std::time::Instant::now();
                let result = self.execute_block(block, sidecar_tx).await;
                let elapsed = start.elapsed().as_millis() as u64;

                // Update the block result with timing and variables
                if let Some(br) = self.block_results.last_mut() {
                    br.timing_ms = elapsed;
                    br.variables_after = self.variables.snapshot();
                }

                match result {
                    Ok(()) => {
                        let msg = self.block_execution_log(block);
                        if let Some(br) = self.block_results.last_mut() {
                            br.log_message = msg.clone();
                        }
                        if !msg.is_empty() {
                            self.log.push(LogEntry {
                                timestamp_ms: elapsed_ms(),
                                block_id: block.id,
                                block_label: block.label.clone(),
                                message: msg,
                            });
                        }
                    }
                    Err(e) if block.safe_mode => {
                        let msg = format!("Error (safe mode): {}", e);
                        if let Some(br) = self.block_results.last_mut() {
                            br.success = false;
                            br.log_message = msg.clone();
                        }
                        self.log.push(LogEntry {
                            timestamp_ms: elapsed_ms(),
                            block_id: block.id,
                            block_label: block.label.clone(),
                            message: msg,
                        });
                    }
                    Err(e) => {
                        self.status = BotStatus::Error;
                        let msg = format!("Error: {}", e);
                        if let Some(br) = self.block_results.last_mut() {
                            br.success = false;
                            br.log_message = msg.clone();
                        }
                        self.log.push(LogEntry {
                            timestamp_ms: elapsed_ms(),
                            block_id: block.id,
                            block_label: block.label.clone(),
                            message: msg.clone(),
                        });
                        return Err(e);
                    }
                }

                // Halt on terminal/retry statuses; continue on Success/Fail so
                // downstream blocks (e.g. ParseJSON to capture balance) can still run.
                match self.status {
                    BotStatus::Error | BotStatus::Ban | BotStatus::Retry => break,
                    BotStatus::Fail => {
                        // Early-exit optimisation: if this KeyCheck has stop_on_fail enabled,
                        // skip all remaining Parse/function blocks — the entry is already
                        // classified as a Fail and nothing downstream can change that.
                        if let BlockSettings::KeyCheck(ref ks) = block.settings {
                            if ks.stop_on_fail {
                                break;
                            }
                        }
                    }
                    _ => {}
                }
            }
            Ok(())
        })
    }

    async fn execute_block(
        &mut self,
        block: &Block,
        sidecar_tx: &tokio::sync::mpsc::Sender<(
            SidecarRequest,
            tokio::sync::oneshot::Sender<SidecarResponse>,
        )>,
    ) -> crate::error::Result<()> {
        match &block.settings {
            BlockSettings::HttpRequest(settings) => {
                self.execute_http_request(block, settings, sidecar_tx).await
            }
            BlockSettings::ParseLR(settings) => self.execute_parse_lr(settings),
            BlockSettings::ParseRegex(settings) => self.execute_parse_regex(settings),
            BlockSettings::ParseJSON(settings) => self.execute_parse_json(settings),
            BlockSettings::ParseCookie(settings) => self.execute_parse_cookie(settings),
            BlockSettings::Parse(settings) => self.execute_parse(settings),
            BlockSettings::KeyCheck(settings) => self.execute_keycheck(settings),
            BlockSettings::StringFunction(settings) => self.execute_string_function(settings),
            BlockSettings::Translate(settings) => self.execute_translate(settings),
            BlockSettings::CryptoFunction(settings) => self.execute_crypto_function(settings),
            BlockSettings::Delay(settings) => self.execute_delay(settings).await,
            BlockSettings::SetVariable(settings) => {
                let value = self.variables.interpolate(&settings.value);
                self.variables
                    .set_user(&settings.name, value, settings.capture);
                Ok(())
            }
            BlockSettings::Log(settings) => {
                let message = self.variables.interpolate(&settings.message);
                self.log.push(LogEntry {
                    timestamp_ms: elapsed_ms(),
                    block_id: block.id,
                    block_label: block.label.clone(),
                    message,
                });
                Ok(())
            }
            BlockSettings::ClearCookies => {
                let req = SidecarRequest {
                    id: Uuid::new_v4().to_string(),
                    action: "clear_cookies".into(),
                    session: self.session_id.clone(),
                    method: None,
                    url: None,
                    headers: None,
                    body: None,
                    timeout: None,
                    proxy: None,
                    browser: None,
                    ja3: None,
                    http2fp: None,
                    follow_redirects: None,
                    max_redirects: None,
                    ssl_verify: None,
                    custom_ciphers: None,

                    ..Default::default()
                };
                let (resp_tx, resp_rx) = tokio::sync::oneshot::channel();
                let _ = sidecar_tx.send((req, resp_tx)).await;
                let _ = resp_rx.await;
                Ok(())
            }
            BlockSettings::IfElse(settings) => {
                let cond = self.evaluate_condition(&settings.condition);
                let branch = if cond {
                    &settings.true_blocks
                } else {
                    &settings.false_blocks
                };
                self.execute_blocks(branch, sidecar_tx).await
            }
            BlockSettings::Loop(settings) => self.execute_loop(block, settings, sidecar_tx).await,
            BlockSettings::DateFunction(settings) => self.execute_date_function(settings),
            BlockSettings::CaseSwitch(settings) => self.execute_case_switch(settings),
            BlockSettings::ListFunction(settings) => self.execute_list_function(settings),
            BlockSettings::ConversionFunction(settings) => {
                self.execute_conversion_function(settings)
            }
            BlockSettings::CookieContainer(settings) => self.execute_cookie_container(settings),
            BlockSettings::Script(_) => Err(crate::error::AppError::Pipeline(
                "Script blocks are not supported by the native pipeline engine.".into(),
            )),
            // Browser automation
            BlockSettings::BrowserOpen(settings) => self.execute_browser_open(settings).await,
            BlockSettings::NavigateTo(settings) => {
                self.execute_navigate_to(settings, block.id, block.label.clone())
                    .await
            }
            BlockSettings::ClickElement(settings) => self.execute_click_element(settings).await,
            BlockSettings::TypeText(settings) => self.execute_type_text(settings).await,
            BlockSettings::WaitForElement(settings) => {
                self.execute_wait_for_element(settings).await
            }
            BlockSettings::GetElementText(settings) => {
                self.execute_get_element_text(settings).await
            }
            BlockSettings::GetDom(settings) => self.execute_get_dom(settings).await,
            BlockSettings::Screenshot(settings) => self.execute_screenshot(settings).await,
            BlockSettings::ExecuteJs(settings) => self.execute_js(settings).await,
            // Networking (send via sidecar like HTTP)
            BlockSettings::Webhook(settings) => self.execute_webhook(settings, sidecar_tx).await,
            // New blocks
            BlockSettings::RandomUserAgent(settings) => self.execute_random_user_agent(settings),
            BlockSettings::OcrCaptcha(settings) => self.execute_ocr_captcha(settings).await,
            BlockSettings::RecaptchaInvisible(settings) => {
                self.execute_recaptcha_invisible(settings, sidecar_tx).await
            }
            BlockSettings::XacfSensor(settings) => self.execute_xacf_sensor(settings),
            BlockSettings::RandomData(settings) => self.execute_random_data(settings),
            BlockSettings::DataDomeSensor(settings) => self.execute_datadome_sensor(settings),
            BlockSettings::Plugin(settings) => self.execute_plugin_block(settings),
            BlockSettings::AkamaiV3Sensor(settings) => self.execute_akamai_v3_sensor(settings),
            BlockSettings::Group(settings) => {
                // Execute child blocks sequentially (like a mini-pipeline)
                self.execute_blocks(&settings.blocks, sidecar_tx).await
            }
            BlockSettings::ParseCSS(settings) => self.execute_parse_css(settings),
            BlockSettings::ParseXPath(settings) => self.execute_parse_xpath(settings),
            // Protocol blocks
            BlockSettings::TcpRequest(settings) => self.execute_tcp_request(block, settings).await,
            BlockSettings::UdpRequest(settings) => self.execute_udp_request(block, settings).await,
            BlockSettings::FtpRequest(settings) => self.execute_ftp_request(block, settings).await,
            BlockSettings::SshRequest(settings) => self.execute_ssh_request(block, settings).await,
            BlockSettings::ImapRequest(settings) => {
                self.execute_imap_request(block, settings).await
            }
            BlockSettings::SmtpRequest(settings) => {
                self.execute_smtp_request(block, settings).await
            }
            BlockSettings::PopRequest(settings) => self.execute_pop_request(block, settings).await,
            BlockSettings::WebSocket(_settings) => Err(crate::error::AppError::Pipeline(
                "WebSocket requires tokio-tungstenite (not yet added)".into(),
            )),
            // Bypass blocks
            BlockSettings::CaptchaSolver(settings) => {
                self.execute_captcha_solver(settings, sidecar_tx).await
            }
            BlockSettings::CloudflareBypass(settings) => {
                self.execute_cloudflare_bypass(settings, sidecar_tx).await
            }
            BlockSettings::LaravelCsrf(settings) => {
                self.execute_laravel_csrf(settings, sidecar_tx).await
            }
            // Extended functions
            BlockSettings::ByteArray(settings) => self.execute_byte_array(settings),
            BlockSettings::Constants(settings) => self.execute_constants(settings),
            BlockSettings::Dictionary(settings) => self.execute_dictionary(settings),
            BlockSettings::FloatFunction(settings) => self.execute_float_function(settings),
            BlockSettings::IntegerFunction(settings) => self.execute_integer_function(settings),
            BlockSettings::TimeFunction(settings) => self.execute_time_function(settings),
            BlockSettings::GenerateGUID(settings) => self.execute_generate_guid(settings),
            BlockSettings::PhoneCountry(settings) => self.execute_phone_country(settings),
            BlockSettings::LambdaParser(settings) => self.execute_lambda_parser(settings),
            BlockSettings::FileSystem(settings) => self.execute_file_system(settings),
            BlockSettings::JwtToken(settings) => {
                self.execute_jwt_token(block, settings).await?;
                Ok(())
            }
            BlockSettings::HeaderSpoof(settings) => {
                self.execute_header_spoof(block, settings).await?;
                Ok(())
            }
            BlockSettings::NuDataSensor(settings) => {
                self.execute_nudata_sensor(settings).await?;
                Ok(())
            }
        }
    }

    async fn execute_delay(&self, settings: &DelaySettings) -> crate::error::Result<()> {
        let ms = if settings.min_ms == settings.max_ms {
            settings.min_ms
        } else {
            use rand::Rng;
            let mut rng = rand::thread_rng();
            rng.gen_range(settings.min_ms..=settings.max_ms)
        };
        tokio::time::sleep(std::time::Duration::from_millis(ms)).await;
        Ok(())
    }

    async fn execute_loop(
        &mut self,
        _block: &Block,
        settings: &LoopSettings,
        sidecar_tx: &tokio::sync::mpsc::Sender<(
            SidecarRequest,
            tokio::sync::oneshot::Sender<SidecarResponse>,
        )>,
    ) -> crate::error::Result<()> {
        match settings.loop_type {
            LoopType::ForEach => {
                let list_str = self.variables.get(&settings.list_var).unwrap_or_default();
                let items: Vec<String> =
                    serde_json::from_str(&list_str).unwrap_or_else(|_| vec![list_str]);
                for item in items {
                    self.variables.set_user(&settings.item_var, item, false);
                    self.execute_blocks(&settings.blocks, sidecar_tx).await?;
                    if self.status != BotStatus::None {
                        break;
                    }
                }
            }
            LoopType::Repeat => {
                for _ in 0..settings.count {
                    self.execute_blocks(&settings.blocks, sidecar_tx).await?;
                    if self.status != BotStatus::None {
                        break;
                    }
                }
            }
        }
        Ok(())
    }
}
