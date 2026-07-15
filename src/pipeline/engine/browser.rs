use futures::StreamExt;
use uuid::Uuid;

use super::*;

impl ExecutionContext {
    pub(super) async fn execute_browser_open(
        &mut self,
        settings: &BrowserOpenSettings,
    ) -> crate::error::Result<()> {
        use chromiumoxide::browser::Browser;

        // ----------------------------------------------------------------
        // Browser::launch uses ws_url_from_output() which reads Chrome's
        // stderr pipe waiting for "listening on ws://..." — Chrome on Windows
        // does NOT write this to stderr, so it hangs forever.
        //
        // Instead: manually spawn Chrome with --remote-debugging-port, poll
        // /json/version over HTTP until it's ready, then connect via
        // Browser::connect(). This is the same approach used by Site Inspector
        // and avoids all stderr-reading issues.
        // ----------------------------------------------------------------

        // Pick a random-ish port in the ephemeral range
        let cdp_port: u16 = 9222
            + (std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .subsec_nanos()
                % 1000) as u16;

        // Resolve Chrome executable
        let chrome_exe = self
            .chrome_executable_path
            .clone()
            .or_else(find_chrome_exe)
            .ok_or_else(|| {
                crate::error::AppError::Pipeline(
                    "Chrome not found. Set chrome path in Settings → Paths.".to_string(),
                )
            })?;

        // Build args
        let mut chrome_args: Vec<String> = vec![
            format!("--remote-debugging-port={}", cdp_port),
            "--remote-debugging-address=127.0.0.1".to_string(),
            "--disable-background-networking".to_string(),
            "--disable-background-timer-throttling".to_string(),
            "--disable-backgrounding-occluded-windows".to_string(),
            "--disable-breakpad".to_string(),
            "--disable-client-side-phishing-detection".to_string(),
            "--disable-component-extensions-with-background-pages".to_string(),
            "--disable-default-apps".to_string(),
            "--disable-dev-shm-usage".to_string(),
            "--disable-hang-monitor".to_string(),
            "--disable-ipc-flooding-protection".to_string(),
            "--disable-popup-blocking".to_string(),
            "--disable-prompt-on-repost".to_string(),
            "--disable-renderer-backgrounding".to_string(),
            "--disable-sync".to_string(),
            "--metrics-recording-only".to_string(),
            "--no-first-run".to_string(),
            "--no-default-browser-check".to_string(),
            "--password-store=basic".to_string(),
            "--use-mock-keychain".to_string(),
            "--no-sandbox".to_string(),
            "--disable-setuid-sandbox".to_string(),
            format!(
                "--user-data-dir={}",
                std::env::temp_dir()
                    .join(format!("ib-chrome-{}", cdp_port))
                    .display()
            ),
        ];

        if settings.headless {
            chrome_args.push("--headless=new".to_string());
            chrome_args.push("--hide-scrollbars".to_string());
            chrome_args.push("--mute-audio".to_string());
        }

        // Proxy
        let effective_proxy = if !settings.proxy.is_empty() {
            let p = self.variables.interpolate(&settings.proxy);
            if p.is_empty() {
                self.proxy.clone()
            } else {
                Some(p)
            }
        } else {
            self.proxy.clone()
        };
        if let Some(ref proxy_str) = effective_proxy {
            if !proxy_str.is_empty() {
                let chrome_proxy = if proxy_str.contains("://") {
                    proxy_str.clone()
                } else {
                    format!("http://{}", proxy_str)
                };
                chrome_args.push(format!("--proxy-server={}", chrome_proxy));
            }
        }

        // Extra args from settings
        for arg in settings.extra_args.split_whitespace() {
            if !arg.is_empty() {
                chrome_args.push(arg.to_string());
            }
        }

        // Spawn Chrome process
        let mut cmd = std::process::Command::new(&chrome_exe);
        cmd.args(&chrome_args);
        cmd.stdout(std::process::Stdio::null());
        cmd.stderr(std::process::Stdio::null());
        cmd.stdin(std::process::Stdio::null());
        // Suppress console window on Windows (CREATE_NO_WINDOW = 0x08000000)
        #[cfg(all(target_os = "windows", not(target_env = "gnu")))]
        {
            use std::os::windows::process::CommandExt;
            cmd.creation_flags(0x08000000);
        }

        let _chrome_child = cmd.spawn().map_err(|e| {
            crate::error::AppError::Pipeline(format!(
                "Failed to spawn Chrome: {}\nPath: {:?}",
                e, chrome_exe
            ))
        })?;

        // Store child so it's dropped (and Chrome killed) when ExecutionContext drops
        self.chrome_child = Some(_chrome_child);

        // Poll /json/version until Chrome is ready (up to 15s)
        let http_client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(2))
            .build()
            .unwrap_or_default();
        let version_url = format!("http://127.0.0.1:{}/json/version", cdp_port);
        let ws_url = {
            let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(15);
            loop {
                if tokio::time::Instant::now() > deadline {
                    return Err(crate::error::AppError::Pipeline(
                        "Chrome started but did not open its debug port within 15 seconds."
                            .to_string(),
                    ));
                }
                match http_client.get(&version_url).send().await {
                    Ok(resp) => {
                        if let Ok(json) = resp.json::<serde_json::Value>().await {
                            if let Some(ws) =
                                json.get("webSocketDebuggerUrl").and_then(|v| v.as_str())
                            {
                                break ws.to_string();
                            }
                        }
                    }
                    Err(_) => {}
                }
                tokio::time::sleep(std::time::Duration::from_millis(200)).await;
            }
        };

        // Connect to running Chrome via CDP WebSocket — no stderr reading, no hang
        let (browser, mut handler) = tokio::time::timeout(
            std::time::Duration::from_secs(10),
            Browser::connect(&ws_url),
        )
        .await
        .map_err(|_| crate::error::AppError::Pipeline("Browser::connect timed out".to_string()))?
        .map_err(|e| crate::error::AppError::Pipeline(format!("Browser connect failed: {}", e)))?;

        // Drive CDP events in background — must stay alive for all browser commands to work
        tokio::spawn(async move { while handler.next().await.is_some() {} });

        self.browser = BrowserHandle(Some(browser));
        self.page = PageHandle(None);
        Ok(())
    }

    pub(super) async fn execute_navigate_to(
        &mut self,
        settings: &NavigateToSettings,
        block_id_for_nav: Uuid,
        block_label_for_nav: String,
    ) -> crate::error::Result<()> {
        let browser = self.browser.0.as_ref().ok_or_else(|| {
            crate::error::AppError::Pipeline("No browser open. Use BrowserOpen first.".into())
        })?;

        let url = self.variables.interpolate(&settings.url);
        let nav_start = std::time::Instant::now();

        // Inject custom cookies via CDP before navigation
        if !settings.custom_cookies.is_empty() {
            let cookie_str = self.variables.interpolate(&settings.custom_cookies);
            // Need a page to set cookies on - create one if needed
            let page_for_cookies = if let Some(ref existing) = self.page.0 {
                existing.clone()
            } else {
                browser.new_page("about:blank").await.map_err(|e| {
                    crate::error::AppError::Pipeline(format!("New page failed: {}", e))
                })?
            };
            let domain = reqwest::Url::parse(&url)
                .ok()
                .and_then(|u| u.host_str().map(|h| h.to_string()))
                .unwrap_or_default();
            for line in cookie_str
                .lines()
                .map(|l| l.trim())
                .filter(|l| !l.is_empty())
            {
                if let Some((name, value)) = line.split_once('=') {
                    let params =
                        chromiumoxide::cdp::browser_protocol::network::SetCookieParams::builder()
                            .name(name.trim())
                            .value(value.trim())
                            .domain(domain.clone())
                            .build()
                            .unwrap();
                    let _ = page_for_cookies.execute(params).await;
                }
            }
            if self.page.0.is_none() {
                self.page = PageHandle(Some(page_for_cookies));
            }
        }

        let nav_timeout = std::time::Duration::from_millis(settings.timeout_ms.max(5000));

        let page = if let Some(ref existing) = self.page.0 {
            // Existing page: navigate with timeout so a stalled page doesn't block forever
            tokio::time::timeout(nav_timeout, existing.goto(&url))
                .await
                .map_err(|_| crate::error::AppError::Pipeline("Navigate timed out".to_string()))?
                .map_err(|e| crate::error::AppError::Pipeline(format!("Navigate failed: {}", e)))?;
            existing.clone()
        } else {
            // New page: create blank page first, then navigate with timeout.
            // new_page(url) blocks until the initial navigation completes — on pages
            // with persistent JS (waiting rooms, keep-alives) it never returns.
            // Instead: open blank page, then goto() with our own timeout.
            let p = tokio::time::timeout(
                std::time::Duration::from_secs(10),
                browser.new_page("about:blank"),
            )
            .await
            .map_err(|_| {
                crate::error::AppError::Pipeline("Browser new page timed out".to_string())
            })?
            .map_err(|e| crate::error::AppError::Pipeline(format!("New page failed: {}", e)))?;

            // Navigate to target URL; ignore navigation errors (e.g. waiting rooms that
            // never fire a load event) — we'll read whatever content the page has.
            let _ = tokio::time::timeout(nav_timeout, p.goto(&url)).await;
            p
        };

        // Additional wait: give the page a moment to settle after navigation.
        // Ignore the result — we proceed regardless of whether it completes.
        let _ = tokio::time::timeout(
            std::time::Duration::from_millis(2000),
            page.wait_for_navigation(),
        )
        .await;
        let nav_elapsed = nav_start.elapsed().as_millis() as u64;

        // Store page content in variables
        let content = page.content().await.unwrap_or_default();
        self.variables.set_data("SOURCE", content.clone());
        let current_url = page
            .url()
            .await
            .map(|u| u.map(|u| u.to_string()).unwrap_or_default())
            .unwrap_or_default();
        self.variables.set_data("ADDRESS", current_url.clone());

        // Populate BlockResult with response info so ResponseViewer can display page HTML
        if let Some(last) = self.block_results.last_mut() {
            last.response = Some(ResponseInfo {
                status_code: 200,
                headers: std::collections::HashMap::new(),
                body: content.clone(),
                final_url: current_url.clone(),
                cookies: std::collections::HashMap::new(),
                timing_ms: nav_elapsed,
            });
        }

        // Add to network log so NetworkViewer shows browser navigations
        let cookies_sent: Vec<(String, String)> = if !settings.custom_cookies.is_empty() {
            self.variables
                .interpolate(&settings.custom_cookies)
                .lines()
                .map(|l| l.trim())
                .filter(|l| !l.is_empty())
                .filter_map(|l| {
                    l.split_once('=')
                        .map(|(k, v)| (k.trim().to_string(), v.trim().to_string()))
                })
                .collect()
        } else {
            Vec::new()
        };
        self.network_log.push(NetworkEntry {
            block_id: block_id_for_nav,
            block_label: block_label_for_nav,
            method: "GET".to_string(),
            url: url.clone(),
            status_code: 200,
            timing_ms: nav_elapsed,
            response_size: content.len(),
            cookies_set: Vec::new(),
            cookies_sent,
        });

        self.page = PageHandle(Some(page));
        Ok(())
    }

    pub(super) async fn execute_click_element(
        &mut self,
        settings: &ClickElementSettings,
    ) -> crate::error::Result<()> {
        let page = self.page.0.as_ref().ok_or_else(|| {
            crate::error::AppError::Pipeline("No page open. Use NavigateTo first.".into())
        })?;

        let selector = self.variables.interpolate(&settings.selector);
        let element = page.find_element(&selector).await.map_err(|e| {
            crate::error::AppError::Pipeline(format!("Element not found '{}': {}", selector, e))
        })?;

        element
            .click()
            .await
            .map_err(|e| crate::error::AppError::Pipeline(format!("Click failed: {}", e)))?;

        if settings.wait_for_navigation {
            let _ = page.wait_for_navigation().await;
            if let Ok(content) = page.content().await {
                self.variables.set_data("SOURCE", content);
            }
        }

        Ok(())
    }

    pub(super) async fn execute_type_text(
        &mut self,
        settings: &TypeTextSettings,
    ) -> crate::error::Result<()> {
        let page = self.page.0.as_ref().ok_or_else(|| {
            crate::error::AppError::Pipeline("No page open. Use NavigateTo first.".into())
        })?;

        let selector = self.variables.interpolate(&settings.selector);
        let text = self.variables.interpolate(&settings.text);

        let element = page.find_element(&selector).await.map_err(|e| {
            crate::error::AppError::Pipeline(format!("Element not found '{}': {}", selector, e))
        })?;

        if settings.clear_first {
            element.click().await.map_err(|e| {
                crate::error::AppError::Pipeline(format!("Click for clear failed: {}", e))
            })?;
            // Select all and delete to clear
            page.execute(
                chromiumoxide::cdp::browser_protocol::input::DispatchKeyEventParams::builder()
                    .r#type(
                        chromiumoxide::cdp::browser_protocol::input::DispatchKeyEventType::KeyDown,
                    )
                    .key("a".to_string())
                    .modifiers(2) // Ctrl
                    .build()
                    .unwrap(),
            )
            .await
            .ok();
            page.execute(
                chromiumoxide::cdp::browser_protocol::input::DispatchKeyEventParams::builder()
                    .r#type(
                        chromiumoxide::cdp::browser_protocol::input::DispatchKeyEventType::KeyDown,
                    )
                    .key("Backspace".to_string())
                    .build()
                    .unwrap(),
            )
            .await
            .ok();
        }

        element
            .type_str(&text)
            .await
            .map_err(|e| crate::error::AppError::Pipeline(format!("Type text failed: {}", e)))?;

        Ok(())
    }

    pub(super) async fn execute_wait_for_element(
        &mut self,
        settings: &WaitForElementSettings,
    ) -> crate::error::Result<()> {
        let page = self.page.0.as_ref().ok_or_else(|| {
            crate::error::AppError::Pipeline("No page open. Use NavigateTo first.".into())
        })?;

        let selector = self.variables.interpolate(&settings.selector);
        let timeout = std::time::Duration::from_millis(settings.timeout_ms);
        let start = std::time::Instant::now();

        loop {
            match page.find_element(&selector).await {
                Ok(_element) => {
                    // Element found - check state if needed
                    match settings.state.as_str() {
                        "visible" | "attached" => break,
                        _ => break,
                    }
                }
                Err(_) => {
                    if start.elapsed() > timeout {
                        return Err(crate::error::AppError::Pipeline(format!(
                            "Timeout waiting for element '{}' ({}ms)",
                            selector, settings.timeout_ms
                        )));
                    }
                    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
                }
            }
        }

        Ok(())
    }

    pub(super) async fn execute_get_element_text(
        &mut self,
        settings: &GetElementTextSettings,
    ) -> crate::error::Result<()> {
        let page = self.page.0.as_ref().ok_or_else(|| {
            crate::error::AppError::Pipeline("No page open. Use NavigateTo first.".into())
        })?;

        let selector = self.variables.interpolate(&settings.selector);
        let element = page.find_element(&selector).await.map_err(|e| {
            crate::error::AppError::Pipeline(format!("Element not found '{}': {}", selector, e))
        })?;

        let value = if settings.attribute.is_empty() || settings.attribute == "innerText" {
            element
                .inner_text()
                .await
                .map_err(|e| crate::error::AppError::Pipeline(format!("Get text failed: {}", e)))?
                .unwrap_or_default()
        } else {
            element
                .attribute(&settings.attribute)
                .await
                .map_err(|e| {
                    crate::error::AppError::Pipeline(format!("Get attribute failed: {}", e))
                })?
                .unwrap_or_default()
        };

        self.variables
            .set_user(&settings.output_var, value, settings.capture);
        Ok(())
    }

    pub(super) async fn execute_get_dom(
        &mut self,
        settings: &GetDomSettings,
    ) -> crate::error::Result<()> {
        let page = self.page.0.as_ref().ok_or_else(|| {
            crate::error::AppError::Pipeline("No page open. Use NavigateTo first.".into())
        })?;
        let selector = self.variables.interpolate(&settings.selector);
        let value = if selector.trim().is_empty() {
            page.content()
                .await
                .map_err(|e| crate::error::AppError::Pipeline(format!("Get DOM failed: {}", e)))?
        } else {
            let selector_json = serde_json::to_string(&selector).map_err(|e| {
                crate::error::AppError::Pipeline(format!("Cannot encode DOM selector: {}", e))
            })?;
            let property = if settings.outer_html {
                "outerHTML"
            } else {
                "innerHTML"
            };
            let expression = format!(
                "(() => {{ const node = document.querySelector({}); return node ? node.{} : null; }})()",
                selector_json, property
            );
            let result = page.evaluate_expression(&expression).await.map_err(|e| {
                crate::error::AppError::Pipeline(format!(
                    "Get DOM failed for '{}': {}",
                    selector, e
                ))
            })?;
            match result.value() {
                Some(serde_json::Value::String(html)) => html.clone(),
                Some(serde_json::Value::Null) | None => {
                    return Err(crate::error::AppError::Pipeline(format!(
                        "DOM element not found '{}'",
                        selector
                    )));
                }
                Some(other) => other.to_string(),
            }
        };
        self.variables
            .set_user(&settings.output_var, value, settings.capture);
        Ok(())
    }

    pub(super) async fn execute_screenshot(
        &mut self,
        settings: &ScreenshotSettings,
    ) -> crate::error::Result<()> {
        let page = self.page.0.as_ref().ok_or_else(|| {
            crate::error::AppError::Pipeline("No page open. Use NavigateTo first.".into())
        })?;

        let bytes = if !settings.selector.is_empty() {
            let selector = self.variables.interpolate(&settings.selector);
            let element = page.find_element(&selector).await.map_err(|e| {
                crate::error::AppError::Pipeline(format!("Element not found: {}", e))
            })?;
            element
                .screenshot(
                    chromiumoxide::cdp::browser_protocol::page::CaptureScreenshotFormat::Png,
                )
                .await
                .map_err(|e| {
                    crate::error::AppError::Pipeline(format!("Screenshot failed: {}", e))
                })?
        } else {
            page.screenshot(
                chromiumoxide::cdp::browser_protocol::page::CaptureScreenshotParams::builder()
                    .format(
                        chromiumoxide::cdp::browser_protocol::page::CaptureScreenshotFormat::Png,
                    )
                    .build(),
            )
            .await
            .map_err(|e| crate::error::AppError::Pipeline(format!("Screenshot failed: {}", e)))?
        };

        use base64::Engine;
        let b64 = base64::engine::general_purpose::STANDARD.encode(&bytes);
        self.variables.set_user(&settings.output_var, b64, false);
        Ok(())
    }

    pub(super) async fn execute_js(
        &mut self,
        settings: &ExecuteJsSettings,
    ) -> crate::error::Result<()> {
        let page = self.page.0.as_ref().ok_or_else(|| {
            crate::error::AppError::Pipeline("No page open. Use NavigateTo first.".into())
        })?;

        let code = self.variables.interpolate(&settings.code);
        let result = page
            .evaluate_expression(&code)
            .await
            .map_err(|e| crate::error::AppError::Pipeline(format!("JS execution failed: {}", e)))?;

        let value = match result.value() {
            Some(v) => match v {
                serde_json::Value::String(s) => s.clone(),
                other => other.to_string(),
            },
            None => String::new(),
        };

        self.variables
            .set_user(&settings.output_var, value, settings.capture);
        Ok(())
    }
}

/// Locate the Chrome/Chromium executable on the current platform.
/// Returns None if Chrome is not found.
fn find_chrome_exe() -> Option<std::path::PathBuf> {
    #[cfg(target_os = "windows")]
    {
        let mut paths: Vec<String> = Vec::new();
        if let Ok(local) = std::env::var("LOCALAPPDATA") {
            paths.push(format!(r"{}\Google\Chrome\Application\chrome.exe", local));
            paths.push(format!(r"{}\Chromium\Application\chrome.exe", local));
        }
        let fixed: &[&str] = &[
            r"C:\Program Files\Google\Chrome\Application\chrome.exe",
            r"C:\Program Files (x86)\Google\Chrome\Application\chrome.exe",
            r"C:\Program Files\Chromium\Application\chrome.exe",
        ];
        for p in fixed
            .iter()
            .copied()
            .chain(paths.iter().map(|s| s.as_str()))
        {
            let path = std::path::Path::new(p);
            if path.exists() {
                return Some(path.to_path_buf());
            }
        }
        // where.exe fallback
        for name in &["chrome.exe", "chromium.exe"] {
            let mut where_cmd = std::process::Command::new("where");
            where_cmd.arg(name);
            #[cfg(not(target_env = "gnu"))]
            {
                use std::os::windows::process::CommandExt;
                where_cmd.creation_flags(0x08000000);
            }
            if let Ok(out) = where_cmd.output() {
                if out.status.success() {
                    let s = String::from_utf8_lossy(&out.stdout);
                    if let Some(first) = s.lines().next() {
                        let p = std::path::PathBuf::from(first.trim());
                        if p.exists() {
                            return Some(p);
                        }
                    }
                }
            }
        }
        None
    }
    #[cfg(not(target_os = "windows"))]
    {
        for name in &[
            "google-chrome",
            "chromium-browser",
            "chromium",
            "google-chrome-stable",
        ] {
            if let Ok(out) = std::process::Command::new("which").arg(name).output() {
                if out.status.success() {
                    let s = String::from_utf8_lossy(&out.stdout);
                    let p = std::path::PathBuf::from(s.trim());
                    if p.exists() {
                        return Some(p);
                    }
                }
            }
        }
        None
    }
}
