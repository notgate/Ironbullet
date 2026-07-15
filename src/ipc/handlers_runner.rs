use std::sync::Arc;
use tokio::sync::Mutex;

use ironbullet::pipeline::engine::ExecutionContext;
use ironbullet::runner::data_pool::DataPool;
use ironbullet::runner::proxy_pool::{ProxyPool, ProxyType};
use ironbullet::runner::{HitResult, RunnerOrchestrator, RunnerSetup};

use super::{resolve_sidecar_path, AppState, IpcResponse};

pub(super) fn debug_pipeline(
    state: Arc<Mutex<AppState>>,
    data: serde_json::Value,
    eval_js: impl Fn(String) + Send + 'static,
) {
    let rt = tokio::runtime::Handle::try_current();
    if let Ok(handle) = rt {
        handle.spawn(async move {
            let s = state.lock().await;
            // If the frontend sends a full pipeline snapshot, use it (keeps settings in sync)
            let frontend_pipeline = data.get("pipeline").and_then(|v| {
                serde_json::from_value::<ironbullet::pipeline::Pipeline>(v.clone()).ok()
            });
            let mut blocks = frontend_pipeline
                .as_ref()
                .map(|p| p.blocks.clone())
                .unwrap_or_else(|| s.pipeline.blocks.clone());
            // If block_ids is provided, restrict execution to only those blocks (Debug Block feature)
            if let Some(ids_val) = data.get("block_ids") {
                if let Some(ids) = ids_val.as_array() {
                    let ids: std::collections::HashSet<String> = ids
                        .iter()
                        .filter_map(|v| v.as_str().map(|s| s.to_string()))
                        .collect();
                    if !ids.is_empty() {
                        blocks.retain(|b| ids.contains(&b.id.to_string()));
                    }
                }
            }
            let data_settings = frontend_pipeline
                .as_ref()
                .map(|p| p.data_settings.clone())
                .unwrap_or_else(|| s.pipeline.data_settings.clone());
            let pm = s.plugin_manager.clone();
            let chrome_exe = {
                let cfg_path = s.config.chrome_executable_path.clone();
                if !cfg_path.is_empty() {
                    let p = std::path::PathBuf::from(&cfg_path);
                    if p.exists() {
                        Some(p)
                    } else {
                        super::find_chrome_executable()
                    }
                } else {
                    super::find_chrome_executable()
                }
            };
            drop(s); // Release lock before async execution

            let native_tx = ironbullet::sidecar::native::create_native_backend();

            let mut ctx = ExecutionContext::new(uuid::Uuid::new_v4().to_string());
            ctx.plugin_manager = Some(pm);
            ctx.chrome_executable_path = chrome_exe;

            // Populate test data variables from frontend
            if let Some(test_line) = data.get("test_data_line").and_then(|v| v.as_str()) {
                if !test_line.is_empty() {
                    let parts: Vec<&str> = test_line.split(data_settings.separator).collect();
                    for (i, slice_name) in data_settings.slices.iter().enumerate() {
                        if let Some(part) = parts.get(i) {
                            ctx.variables.set_input(slice_name, part.to_string());
                        }
                    }
                }
            }
            if let Some(test_proxy) = data.get("test_proxy").and_then(|v| v.as_str()) {
                if !test_proxy.is_empty() {
                    ctx.proxy = Some(
                        if ironbullet::sidecar::xray_pool::supports_uri(test_proxy) {
                            ironbullet::sidecar::xray_pool::resolve_proxy_uri(test_proxy)
                                .unwrap_or_else(|error| {
                                    eprintln!("[debug] encrypted proxy startup failed: {error}");
                                    test_proxy.to_string()
                                })
                        } else {
                            test_proxy.to_string()
                        },
                    );
                }
            }

            let exec_result = ctx.execute_blocks(&blocks, &native_tx).await;

            // Send last result as debug_step (backward compat)
            let result = ctx.block_results.last().cloned();
            let resp = IpcResponse::ok(
                "debug_step",
                serde_json::to_value(&result).unwrap_or_default(),
            );
            eval_js(format!(
                "window.__ipc_callback({})",
                serde_json::to_string(&resp).unwrap_or_default()
            ));

            // Always send all block results for the response viewer
            let resp = IpcResponse::ok(
                "debug_results",
                serde_json::to_value(&ctx.block_results).unwrap_or_default(),
            );
            eval_js(format!(
                "window.__ipc_callback({})",
                serde_json::to_string(&resp).unwrap_or_default()
            ));

            // Send network log for network viewer
            let resp = IpcResponse::ok(
                "network_log",
                serde_json::to_value(&ctx.network_log).unwrap_or_default(),
            );
            eval_js(format!(
                "window.__ipc_callback({})",
                serde_json::to_string(&resp).unwrap_or_default()
            ));

            // If there was an error, also send it as a log message
            if let Err(e) = exec_result {
                let resp = IpcResponse::err("debug_step", format!("Pipeline error: {}", e));
                eval_js(format!(
                    "window.__ipc_callback({})",
                    serde_json::to_string(&resp).unwrap_or_default()
                ));
            }
        });
    }
}

pub(super) fn start_runner(
    state: Arc<Mutex<AppState>>,
    data: serde_json::Value,
    eval_js: impl Fn(String) + Send + 'static,
) {
    let rt = tokio::runtime::Handle::try_current();
    if let Ok(handle) = rt {
        let inner_handle = handle.clone();
        handle.spawn(async move {
            let threads = data.get("threads").and_then(|v| v.as_u64()).unwrap_or(100) as usize;
            let mut s = state.lock().await;

            // Start sidecar if needed
            let sidecar_path = resolve_sidecar_path(&s.config.sidecar_path);
            if !s.sidecar.is_running() {
                match s.sidecar.start(&sidecar_path).await {
                    Ok(_) => {}
                    Err(e) => {
                        let resp = IpcResponse::err(
                            "runner_error",
                            format!("Failed to start sidecar: {}", e),
                        );
                        eval_js(format!(
                            "window.__ipc_callback({})",
                            serde_json::to_string(&resp).unwrap_or_default()
                        ));
                        return;
                    }
                }
            }

            // Get a fresh sidecar tx by restarting
            s.sidecar.stop().await;
            let sidecar_tx = match s.sidecar.start(&sidecar_path).await {
                Ok(tx) => tx,
                Err(e) => {
                    let resp =
                        IpcResponse::err("runner_error", format!("Failed to start sidecar: {}", e));
                    eval_js(format!(
                        "window.__ipc_callback({})",
                        serde_json::to_string(&resp).unwrap_or_default()
                    ));
                    return;
                }
            };

            // Use the frontend pipeline snapshot when available so runner settings and
            // proxy settings match what the user sees in the current tab.
            let pipeline_snapshot = data
                .get("pipeline")
                .and_then(|v| {
                    serde_json::from_value::<ironbullet::pipeline::Pipeline>(v.clone()).ok()
                })
                .unwrap_or_else(|| s.pipeline.clone());
            let runner_settings = &pipeline_snapshot.runner_settings;

            // Load data from wordlist file
            let wordlist_path = data
                .get("wordlist_path")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let data_pool = if !wordlist_path.is_empty() {
                match DataPool::from_file_with_limits(
                    &wordlist_path,
                    true,
                    runner_settings.skip as usize,
                    runner_settings.take as usize,
                ) {
                    Ok(dp) => dp,
                    Err(e) => {
                        let resp = IpcResponse::err(
                            "runner_error",
                            format!("Failed to load wordlist '{}': {}", wordlist_path, e),
                        );
                        eval_js(format!(
                            "window.__ipc_callback({})",
                            serde_json::to_string(&resp).unwrap_or_default()
                        ));
                        return;
                    }
                }
            } else {
                // Try inline data from the data field
                let lines: Vec<String> = data
                    .get("data_lines")
                    .and_then(|v| v.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|v| v.as_str().map(|s| s.to_string()))
                            .collect()
                    })
                    .unwrap_or_default();
                DataPool::with_limits(
                    lines,
                    runner_settings.skip as usize,
                    runner_settings.take as usize,
                )
            };

            let ban_secs = pipeline_snapshot.proxy_settings.ban_duration_secs as u64;

            // Load proxies:
            // 1. Explicit proxy_path from the job (flat file) — highest priority
            // 2. Active proxy group's sources from pipeline proxy_settings
            // 3. Global proxy_sources from pipeline proxy_settings
            // 4. Empty pool (proxyless)
            let proxy_path = data
                .get("proxy_path")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let proxy_pool = if !proxy_path.is_empty() {
                match ProxyPool::from_file(&proxy_path, ban_secs) {
                    Ok(pp) => pp,
                    Err(e) => {
                        let resp = IpcResponse::err(
                            "runner_error",
                            format!("Failed to load proxies '{}': {}", proxy_path, e),
                        );
                        eval_js(format!(
                            "window.__ipc_callback({})",
                            serde_json::to_string(&resp).unwrap_or_default()
                        ));
                        return;
                    }
                }
            } else {
                // Try to load from pipeline proxy groups / sources
                let ps = &pipeline_snapshot.proxy_settings;
                // Gather all source paths from the active group (or all groups if no active)
                let sources: Vec<_> = if !ps.active_group.is_empty() {
                    ps.proxy_groups
                        .iter()
                        .filter(|g| g.name == ps.active_group)
                        .flat_map(|g| g.sources.iter())
                        .collect()
                } else if !ps.proxy_groups.is_empty() {
                    ps.proxy_groups
                        .iter()
                        .flat_map(|g| g.sources.iter())
                        .collect()
                } else {
                    ps.proxy_sources.iter().collect()
                };

                if !sources.is_empty() {
                    use ironbullet::pipeline::ProxySourceType;
                    let mut pool = ProxyPool::empty_with_ban(ban_secs);
                    for src in sources {
                        if matches!(src.source_type, ProxySourceType::File) {
                            let default_type = src.default_proxy_type.as_deref();
                            if let Err(e) = pool.load_from_file(&src.value, default_type) {
                                eprintln!(
                                    "[runner] warning: failed to load proxy source '{}': {}",
                                    src.value, e
                                );
                            }
                        }
                        // URL sources could be fetched here in future — skip for now
                    }
                    pool
                } else {
                    ProxyPool::empty()
                }
            };

            let (hits_tx, mut hits_rx) = tokio::sync::mpsc::channel::<HitResult>(1024);

            // Use frontend pipeline snapshot if provided (keeps runner_settings, proxy_settings, etc. in sync)
            let pipeline = data
                .get("pipeline")
                .and_then(|v| {
                    serde_json::from_value::<ironbullet::pipeline::Pipeline>(v.clone()).ok()
                })
                .unwrap_or_else(|| s.pipeline.clone());
            let proxy_mode = pipeline.proxy_settings.proxy_mode.clone();
            let pm = s.plugin_manager.clone();
            // Resolve chrome executable: prefer user-configured path → auto-discovery
            let chrome_exe = {
                let cfg_path = s.config.chrome_executable_path.clone();
                if !cfg_path.is_empty() {
                    let p = std::path::PathBuf::from(&cfg_path);
                    if p.exists() {
                        Some(p)
                    } else {
                        super::find_chrome_executable()
                    }
                } else {
                    super::find_chrome_executable()
                }
            };
            let runner = Arc::new(RunnerOrchestrator::new(RunnerSetup {
                pipeline,
                proxy_mode,
                data_pool,
                proxy_pool,
                sidecar_tx,
                thread_count: threads,
                hits_tx,
                plugin_manager: Some(pm),
                chrome_executable_path: chrome_exe,
                custom_input_values: std::collections::HashMap::new(),
            }));

            s.runner = Some(runner.clone());
            s.hits.clear();

            // Drop the lock before spawning long-running tasks
            drop(s);

            // Wrap eval_js in Arc<Mutex> so both spawned tasks can use it
            let eval_js = Arc::new(tokio::sync::Mutex::new(eval_js));

            // Notify frontend that runner has started
            {
                let ejs = eval_js.lock().await;
                let resp = IpcResponse::ok("runner_started", serde_json::json!(null));
                ejs(format!(
                    "window.__ipc_callback({})",
                    serde_json::to_string(&resp).unwrap_or_default()
                ));
            }

            // Spawn hit collector -- streams each hit to frontend
            let state2 = state.clone();
            let eval_js2 = eval_js.clone();
            inner_handle.spawn(async move {
                while let Some(hit) = hits_rx.recv().await {
                    let hit_data = serde_json::json!({
                        "data_line": hit.data_line,
                        "captures": hit.captures,
                        "proxy": hit.proxy,
                    });
                    let resp = IpcResponse::ok("runner_hit", hit_data);
                    let ejs = eval_js2.lock().await;
                    ejs(format!(
                        "window.__ipc_callback({})",
                        serde_json::to_string(&resp).unwrap_or_default()
                    ));
                    drop(ejs);
                    let mut s = state2.lock().await;
                    s.hits.push(hit);
                }
            });

            // Spawn runner
            let eval_js3 = eval_js.clone();
            inner_handle.spawn(async move {
                runner.start().await;
                // Runner finished -- notify frontend
                let ejs = eval_js3.lock().await;
                let resp = IpcResponse::ok("runner_stopped", serde_json::json!(null));
                ejs(format!(
                    "window.__ipc_callback({})",
                    serde_json::to_string(&resp).unwrap_or_default()
                ));
            });
        });
    }
}

pub(super) fn pause_runner(state: Arc<Mutex<AppState>>) {
    let rt = tokio::runtime::Handle::try_current();
    if let Ok(handle) = rt {
        handle.spawn(async move {
            let s = state.lock().await;
            if let Some(ref runner) = s.runner {
                runner.pause();
            }
        });
    }
}

pub(super) fn resume_runner(state: Arc<Mutex<AppState>>) {
    let rt = tokio::runtime::Handle::try_current();
    if let Ok(handle) = rt {
        handle.spawn(async move {
            let s = state.lock().await;
            if let Some(ref runner) = s.runner {
                runner.resume();
            }
        });
    }
}

pub(super) fn stop_runner(state: Arc<Mutex<AppState>>) {
    let rt = tokio::runtime::Handle::try_current();
    if let Ok(handle) = rt {
        handle.spawn(async move {
            let s = state.lock().await;
            if let Some(ref runner) = s.runner {
                runner.stop();
            }
        });
    }
}

pub(super) fn get_runner_stats(
    state: Arc<Mutex<AppState>>,
    eval_js: impl Fn(String) + Send + 'static,
) {
    let rt = tokio::runtime::Handle::try_current();
    if let Ok(handle) = rt {
        handle.spawn(async move {
            let s = state.lock().await;
            let stats = if let Some(ref runner) = s.runner {
                Some(runner.get_stats())
            } else {
                None
            };
            let resp = IpcResponse::ok(
                "runner_stats",
                serde_json::to_value(&stats).unwrap_or_default(),
            );
            eval_js(format!(
                "window.__ipc_callback({})",
                serde_json::to_string(&resp).unwrap_or_default()
            ));
        });
    }
}

pub(super) fn check_proxies(
    state: Arc<Mutex<AppState>>,
    eval_js: impl Fn(String) + Send + 'static,
) {
    let state = state.clone();
    let rt = tokio::runtime::Handle::try_current();
    if let Ok(handle) = rt {
        handle.spawn(async move {
            let s = state.lock().await;
            let mut all_sources = s.pipeline.proxy_settings.proxy_sources.clone();
            // Also gather sources from active proxy group
            if !s.pipeline.proxy_settings.active_group.is_empty() {
                if let Some(group) = s
                    .pipeline
                    .proxy_settings
                    .proxy_groups
                    .iter()
                    .find(|g| g.name == s.pipeline.proxy_settings.active_group)
                {
                    all_sources = group.sources.clone();
                }
            }
            // Get default proxy type from settings (each source can have its own)
            drop(s);

            // Load proxies from sources using proper parsing (handles SOCKS5, auth, etc.)
            let mut proxy_pool = ProxyPool::empty();
            for src in &all_sources {
                let default_type = src.default_proxy_type.as_deref().and_then(|s| {
                    match s.to_lowercase().as_str() {
                        "https" => Some(ProxyType::Https),
                        "socks4" => Some(ProxyType::Socks4),
                        "socks5" => Some(ProxyType::Socks5),
                        "shadowsocks" | "ss" => Some(ProxyType::Shadowsocks),
                        _ => Some(ProxyType::Http),
                    }
                });
                match src.source_type {
                    ironbullet::pipeline::ProxySourceType::File => {
                        let _ = proxy_pool
                            .load_from_file(&src.value, src.default_proxy_type.as_deref());
                    }
                    ironbullet::pipeline::ProxySourceType::Inline => {
                        // Parse inline proxies line by line
                        for line in src.value.lines().filter(|l| !l.trim().is_empty()) {
                            proxy_pool.load_from_string(line.trim(), default_type);
                        }
                    }
                    ironbullet::pipeline::ProxySourceType::Url => {
                        if let Ok(resp) = reqwest::get(&src.value).await {
                            if let Ok(text) = resp.text().await {
                                for line in text.lines().filter(|l| !l.trim().is_empty()) {
                                    proxy_pool.load_from_string(line.trim(), default_type);
                                }
                            }
                        }
                    }
                }
            }

            let total = proxy_pool.total();
            let mut alive = 0u32;
            let mut dead = 0u32;

            // Simple connectivity check: try to connect through each proxy
            for _ in 0..total {
                if let Some(proxy_url) = proxy_pool.next_proxy() {
                    let check_result = async {
                        let proxy = reqwest::Proxy::all(&proxy_url).map_err(|e| e.to_string())?;
                        let client = reqwest::Client::builder()
                            .proxy(proxy)
                            .timeout(std::time::Duration::from_secs(8))
                            .build()
                            .map_err(|e| e.to_string())?;
                        client
                            .get("https://httpbin.org/ip")
                            .send()
                            .await
                            .map_err(|e| e.to_string())?;
                        Ok::<_, String>(())
                    }
                    .await;

                    match check_result {
                        Ok(_) => alive += 1,
                        Err(_) => dead += 1,
                    }
                }
            }

            let resp = IpcResponse::ok(
                "proxy_check_result",
                serde_json::json!({
                    "alive": alive,
                    "dead": dead,
                    "total": total,
                }),
            );
            eval_js(format!(
                "window.__ipc_callback({})",
                serde_json::to_string(&resp).unwrap_or_default()
            ));
        });
    }
}

pub(super) fn probe_url(data: serde_json::Value, eval_js: impl Fn(String) + Send + 'static) {
    let rt = tokio::runtime::Handle::try_current();
    if let Ok(handle) = rt {
        handle.spawn(async move {
            let url = data.get("url").and_then(|v| v.as_str()).unwrap_or("").to_string();
            if url.is_empty() {
                let resp = IpcResponse::ok("probe_result", serde_json::json!({ "error": "No URL provided" }));
                eval_js(format!("window.__ipc_callback({})", serde_json::to_string(&resp).unwrap_or_default()));
                return;
            }

            let start = std::time::Instant::now();
            let client = reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(15))
                .redirect(reqwest::redirect::Policy::limited(5))
                .danger_accept_invalid_certs(true)
                .default_headers({
                    let mut h = reqwest::header::HeaderMap::new();
                    h.insert(reqwest::header::USER_AGENT, "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36".parse().unwrap());
                    h.insert(reqwest::header::ACCEPT, "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8".parse().unwrap());
                    h.insert(reqwest::header::ACCEPT_LANGUAGE, "en-US,en;q=0.9".parse().unwrap());
                    h
                })
                .cookie_store(true)
                .build();

            let client = match client {
                Ok(c) => c,
                Err(e) => {
                    let resp = IpcResponse::ok("probe_result", serde_json::json!({ "error": format!("Failed to build HTTP client: {}", e) }));
                    eval_js(format!("window.__ipc_callback({})", serde_json::to_string(&resp).unwrap_or_default()));
                    return;
                }
            };

            match client.get(&url).send().await {
                Ok(response) => {
                    let status_code = response.status().as_u16();
                    let final_url = response.url().to_string();
                    let timing_ms = start.elapsed().as_millis() as u64;

                    // Collect headers
                    let mut headers = serde_json::Map::new();
                    for (name, value) in response.headers().iter() {
                        if let Ok(v) = value.to_str() {
                            headers.insert(name.to_string(), serde_json::Value::String(v.to_string()));
                        }
                    }

                    // Extract cookies from set-cookie headers
                    let mut cookies = serde_json::Map::new();
                    for value in response.headers().get_all(reqwest::header::SET_COOKIE).iter() {
                        if let Ok(v) = value.to_str() {
                            if let Some(eq_pos) = v.find('=') {
                                let name = v[..eq_pos].trim().to_string();
                                let rest = &v[eq_pos + 1..];
                                let val = rest.split(';').next().unwrap_or("").trim().to_string();
                                cookies.insert(name, serde_json::Value::String(val));
                            }
                        }
                    }

                    // Read body snippet (first 2KB)
                    let body_snippet = match response.text().await {
                        Ok(text) => {
                            if text.len() > 2048 {
                                text[..2048].to_string()
                            } else {
                                text
                            }
                        }
                        Err(_) => String::new(),
                    };

                    let resp = IpcResponse::ok("probe_result", serde_json::json!({
                        "status_code": status_code,
                        "headers": headers,
                        "cookies": cookies,
                        "body_snippet": body_snippet,
                        "final_url": final_url,
                        "timing_ms": timing_ms,
                    }));
                    eval_js(format!("window.__ipc_callback({})", serde_json::to_string(&resp).unwrap_or_default()));
                }
                Err(e) => {
                    let resp = IpcResponse::ok("probe_result", serde_json::json!({ "error": format!("Request failed: {}", e) }));
                    eval_js(format!("window.__ipc_callback({})", serde_json::to_string(&resp).unwrap_or_default()));
                }
            }
        });
    }
}

pub(super) fn site_inspect(
    state: Arc<Mutex<AppState>>,
    data: serde_json::Value,
    eval_js: impl Fn(String) + Send + 'static,
) {
    let rt = tokio::runtime::Handle::try_current();
    if let Ok(handle) = rt {
        handle.spawn(async move {
            let url = data
                .get("url")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let method = data
                .get("method")
                .and_then(|v| v.as_str())
                .unwrap_or("GET")
                .to_string();
            let proxy = data
                .get("proxy")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            let body = data
                .get("body")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            let browser = data
                .get("browser")
                .and_then(|v| v.as_str())
                .unwrap_or("chrome")
                .to_string();
            // Extra headers the user typed in the inspector
            let extra_headers: Vec<[String; 2]> = data
                .get("headers")
                .and_then(|v| serde_json::from_value(v.clone()).ok())
                .unwrap_or_default();

            if url.is_empty() {
                let resp = IpcResponse::ok(
                    "site_inspect_result",
                    serde_json::json!({ "error": "No URL provided" }),
                );
                eval_js(format!(
                    "window.__ipc_callback({})",
                    serde_json::to_string(&resp).unwrap_or_default()
                ));
                return;
            }

            // Attempt to use the AzureTLS sidecar for real TLS fingerprinting
            let mut s = state.lock().await;
            let sidecar_path = resolve_sidecar_path(&s.config.sidecar_path);
            let sidecar_tx = s.sidecar.get_or_start(&sidecar_path).await.ok();
            drop(s);

            if let Some(tx) = sidecar_tx {
                let session_id = format!("__inspector_{}", uuid::Uuid::new_v4());

                // Create session
                let (resp_tx, resp_rx) = tokio::sync::oneshot::channel();
                let _ = tx
                    .send((
                        ironbullet::sidecar::protocol::SidecarRequest {
                            id: uuid::Uuid::new_v4().to_string(),
                            action: "new_session".into(),
                            session: session_id.clone(),
                            browser: Some(browser.clone()),
                            proxy: proxy.clone(),
                            follow_redirects: Some(true),
                            max_redirects: Some(8),
                            ssl_verify: None,
                            ja3: None,
                            http2fp: None,
                            url: None,
                            method: None,
                            headers: None,
                            body: None,
                            timeout: None,
                            custom_ciphers: None,
                            return_request_headers: None,
                            proxy_insecure: None,
                        },
                        resp_tx,
                    ))
                    .await;
                let _ = resp_rx.await;

                // Make the request
                let (resp_tx2, resp_rx2) = tokio::sync::oneshot::channel();
                let hdrs: Vec<Vec<String>> = extra_headers
                    .iter()
                    .map(|h| vec![h[0].clone(), h[1].clone()])
                    .collect();
                let _ = tx
                    .send((
                        ironbullet::sidecar::protocol::SidecarRequest {
                            id: uuid::Uuid::new_v4().to_string(),
                            action: "request".into(),
                            session: session_id.clone(),
                            method: Some(method.clone()),
                            url: Some(url.clone()),
                            headers: if hdrs.is_empty() { None } else { Some(hdrs) },
                            body: body.clone(),
                            timeout: Some(20_000),
                            proxy: proxy.clone(),
                            browser: Some(browser.clone()),
                            follow_redirects: Some(true),
                            max_redirects: Some(8),
                            ssl_verify: None,
                            ja3: None,
                            http2fp: None,
                            custom_ciphers: None,
                            return_request_headers: Some(true), // capture what was actually sent
                            proxy_insecure: None,
                        },
                        resp_tx2,
                    ))
                    .await;

                let sidecar_resp = resp_rx2.await.ok();

                // Close session
                let (close_tx, _) = tokio::sync::oneshot::channel();
                let _ = tx
                    .send((
                        ironbullet::sidecar::protocol::SidecarRequest {
                            id: uuid::Uuid::new_v4().to_string(),
                            action: "close_session".into(),
                            session: session_id,
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
                            return_request_headers: None,
                            proxy_insecure: None,
                        },
                        close_tx,
                    ))
                    .await;

                if let Some(sr) = sidecar_resp {
                    let resp = IpcResponse::ok(
                        "site_inspect_result",
                        serde_json::json!({
                            "status":          sr.status,
                            "final_url":       sr.final_url,
                            "timing_ms":       sr.timing_ms,
                            "headers":         sr.headers.unwrap_or_default(),
                            "request_headers": sr.request_headers.unwrap_or_default(),
                            "cookies":         sr.cookies.unwrap_or_default(),
                            "body":            sr.body,
                            "error":           sr.error,
                            "via":             "azuretls",
                            "browser":         browser,
                        }),
                    );
                    eval_js(format!(
                        "window.__ipc_callback({})",
                        serde_json::to_string(&resp).unwrap_or_default()
                    ));
                    return;
                }
            }

            // Fallback: native reqwest (no TLS fingerprinting)
            let start = std::time::Instant::now();
            let client = reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(20))
                .redirect(reqwest::redirect::Policy::limited(8))
                .danger_accept_invalid_certs(true)
                .cookie_store(true)
                .build();

            let client = match client {
                Ok(c) => c,
                Err(e) => {
                    let resp = IpcResponse::ok(
                        "site_inspect_result",
                        serde_json::json!({ "error": format!("Client error: {}", e) }),
                    );
                    eval_js(format!(
                        "window.__ipc_callback({})",
                        serde_json::to_string(&resp).unwrap_or_default()
                    ));
                    return;
                }
            };

            let mut req = client.request(
                reqwest::Method::from_bytes(method.as_bytes()).unwrap_or(reqwest::Method::GET),
                &url,
            );
            for h in &extra_headers {
                req = req.header(&h[0], &h[1]);
            }
            if let Some(b) = body {
                req = req.body(b);
            }
            if let Some(p) = proxy {
                if let Ok(prx) = reqwest::Proxy::all(&p) {
                    // Can't override per-request proxy in reqwest easily, skip
                    let _ = prx;
                }
            }

            match req.send().await {
                Ok(response) => {
                    let status = response.status().as_u16() as i32;
                    let final_url = response.url().to_string();
                    let timing_ms = start.elapsed().as_millis() as i64;
                    let mut headers = std::collections::HashMap::new();
                    for (k, v) in response.headers() {
                        if let Ok(val) = v.to_str() {
                            headers.insert(k.to_string(), val.to_string());
                        }
                    }
                    let body = response.text().await.unwrap_or_default();
                    let resp = IpcResponse::ok(
                        "site_inspect_result",
                        serde_json::json!({
                            "status":          status,
                            "final_url":       final_url,
                            "timing_ms":       timing_ms,
                            "headers":         headers,
                            "request_headers": serde_json::Value::Object(serde_json::Map::new()),
                            "cookies":         serde_json::Value::Object(serde_json::Map::new()),
                            "body":            body,
                            "error":           null,
                            "via":             "reqwest",
                        }),
                    );
                    eval_js(format!(
                        "window.__ipc_callback({})",
                        serde_json::to_string(&resp).unwrap_or_default()
                    ));
                }
                Err(e) => {
                    let resp = IpcResponse::ok(
                        "site_inspect_result",
                        serde_json::json!({ "error": format!("Request failed: {}", e) }),
                    );
                    eval_js(format!(
                        "window.__ipc_callback({})",
                        serde_json::to_string(&resp).unwrap_or_default()
                    ));
                }
            }
        });
    }
}

// ── Browser capture — Go sidecar MITM proxy ─────────────────────────────────
// Delegates MITM proxy to the Go sidecar which uses Go's crypto/tls — far more
// battle-tested than a hand-rolled Rust TLS MITM stack. Chrome is launched with
// --proxy-server pointing at the sidecar proxy.

pub(super) fn inspect_browser_open(
    state: Arc<Mutex<AppState>>,
    data: serde_json::Value,
    eval_js: impl Fn(String) + Send + 'static,
) {
    let rt = tokio::runtime::Handle::try_current();
    if rt.is_err() {
        eval_js(format!("window.__ipc_callback({})",
            serde_json::to_string(&IpcResponse::ok("inspector_browser_event",
                serde_json::json!({ "type": "error", "message": "Internal error: no tokio runtime" })
            )).unwrap_or_default()
        ));
        return;
    }
    let Ok(handle) = rt else { return };

    let (js_tx, mut js_rx) = tokio::sync::mpsc::channel::<String>(1024);
    handle.spawn(async move {
        while let Some(js) = js_rx.recv().await {
            eval_js(js);
        }
    });
    let js = js_tx.clone();

    fn emit_browser(tx: &tokio::sync::mpsc::Sender<String>, payload: serde_json::Value) {
        let resp = IpcResponse::ok("inspector_browser_event", payload);
        let _ = tx.try_send(format!(
            "window.__ipc_callback({})",
            serde_json::to_string(&resp).unwrap_or_default()
        ));
    }
    fn emit_proxy(tx: &tokio::sync::mpsc::Sender<String>, payload: serde_json::Value) {
        let resp = IpcResponse::ok("inspector_proxy_event", payload);
        let _ = tx.try_send(format!(
            "window.__ipc_callback({})",
            serde_json::to_string(&resp).unwrap_or_default()
        ));
    }

    let chrome_exe = {
        let from_cfg = state
            .try_lock()
            .map(|s| {
                let p = s.config.chrome_executable_path.clone();
                if !p.is_empty() {
                    let pb = std::path::PathBuf::from(&p);
                    if pb.exists() {
                        Some(pb)
                    } else {
                        None
                    }
                } else {
                    None
                }
            })
            .ok()
            .flatten();
        from_cfg.or_else(|| super::find_chrome_executable())
    };

    if chrome_exe.is_none() {
        emit_browser(
            &js,
            serde_json::json!({
                "type": "error",
                "message": "Chrome not found. Set the path in Settings → Paths."
            }),
        );
        return;
    }

    let raw_url = data
        .get("url")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim()
        .to_string();
    if raw_url.is_empty() {
        emit_browser(
            &js,
            serde_json::json!({ "type": "error", "message": "No URL provided." }),
        );
        return;
    }
    let url = if raw_url.starts_with("http://") || raw_url.starts_with("https://") {
        raw_url
    } else {
        format!("https://{}", raw_url)
    };

    let state_clone = state.clone();
    let task = handle.spawn(async move {
        // Ensure sidecar is running
        let sidecar_path = {
            let s = state_clone.lock().await;
            s.config.sidecar_path.clone()
        };
        let req_tx = {
            let mut s = state_clone.lock().await;
            match s.sidecar.get_or_start(&sidecar_path).await {
                Ok(tx) => tx,
                Err(e) => {
                    emit_browser(&js, serde_json::json!({ "type": "error", "message": format!("Sidecar start failed: {e}") }));
                    return;
                }
            }
        };

        // Subscribe to proxy events BEFORE starting the proxy
        let mut proxy_event_rx = {
            let s = state_clone.lock().await;
            match s.sidecar.proxy_event_tx.as_ref().map(|t| t.subscribe()) {
                Some(rx) => rx,
                None => {
                    emit_browser(&js, serde_json::json!({ "type": "error", "message": "No proxy event channel" }));
                    return;
                }
            }
        };

        emit_browser(&js, serde_json::json!({ "type": "status", "message": "Starting MITM proxy..." }));

        // Send start_mitm_proxy to sidecar
        let req_id = uuid::Uuid::new_v4().to_string();
        let (resp_tx, resp_rx) = tokio::sync::oneshot::channel();
        let mitm_req = ironbullet::sidecar::protocol::SidecarRequest {
            id: req_id.clone(),
            action: "start_mitm_proxy".to_string(),
            session: String::new(),
            ..Default::default()
        };
        if req_tx.send((mitm_req, resp_tx)).await.is_err() {
            emit_browser(&js, serde_json::json!({ "type": "error", "message": "Sidecar channel closed" }));
            return;
        }
        let mitm_resp = match tokio::time::timeout(std::time::Duration::from_secs(10), resp_rx).await {
            Ok(Ok(r)) => r,
            _ => {
                emit_browser(&js, serde_json::json!({ "type": "error", "message": "Sidecar MITM start timed out" }));
                return;
            }
        };
        if let Some(e) = &mitm_resp.error {
            emit_browser(&js, serde_json::json!({ "type": "error", "message": format!("MITM proxy error: {e}") }));
            return;
        }

        // Parse port and CA cert from response
        let mitm_info: serde_json::Value = serde_json::from_str(&mitm_resp.body).unwrap_or_default();
        let proxy_port = mitm_info.get("port").and_then(|v| v.as_u64()).unwrap_or(8877) as u16;
        let ca_cert_pem = mitm_info.get("ca_cert_pem").and_then(|v| v.as_str()).unwrap_or("").to_string();

        emit_browser(&js, serde_json::json!({ "type": "status", "message": format!("Proxy on port {}, launching Chrome...", proxy_port) }));

        // Install CA cert into Chrome profile
        let profile_dir = std::env::temp_dir().join(format!("ib-chrome-{}", uuid::Uuid::new_v4()));
        let _ = tokio::fs::create_dir_all(&profile_dir).await;
        if !ca_cert_pem.is_empty() {
            install_ca_into_chrome_profile(&profile_dir, &ca_cert_pem).await;
        }

        // Launch Chrome
        let chrome_exe = chrome_exe.unwrap();
        let mut cmd = std::process::Command::new(&chrome_exe);
        cmd.args([
            format!("--proxy-server=http://127.0.0.1:{}", proxy_port),
            format!("--user-data-dir={}", profile_dir.display()),
            "--no-first-run".to_string(),
            "--no-default-browser-check".to_string(),
            "--disable-sync".to_string(),
            "--disable-extensions".to_string(),
            "--disable-background-networking".to_string(),
            "--no-sandbox".to_string(),
            "--ignore-certificate-errors".to_string(),
            "--test-type".to_string(),
            url.clone(),
        ]);
        cmd.stdout(std::process::Stdio::null());
        cmd.stderr(std::process::Stdio::null());
        cmd.stdin(std::process::Stdio::null());

        let mut chrome_child = match cmd.spawn() {
            Ok(c) => c,
            Err(e) => {
                emit_browser(&js, serde_json::json!({ "type": "error", "message": format!("Chrome launch failed: {e}") }));
                return;
            }
        };

        {
            let mut s = state_clone.lock().await;
            s.browser_capture_profile = Some(profile_dir.clone());
            s.inspect_proxy_port = Some(proxy_port);
        }

        // Tell frontend Chrome is open
        emit_browser(&js, serde_json::json!({ "type": "opened", "url": url }));
        emit_proxy(&js, serde_json::json!({ "type": "ready", "port": proxy_port, "ca_cert_pem": ca_cert_pem }));

        // Forward proxy events from sidecar broadcast to frontend
        let js_events = js.clone();
        let event_task = tokio::spawn(async move {
            loop {
                match proxy_event_rx.recv().await {
                    Ok(ev) => emit_proxy(&js_events, ev),
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                }
            }
        });

        // Wait for Chrome to exit
        tokio::task::spawn_blocking(move || { let _ = chrome_child.wait(); }).await.ok();

        // Cleanup
        event_task.abort();
        {
            let mut s = state_clone.lock().await;
            s.inspect_proxy_port = None;
            if let Some(p) = s.browser_capture_profile.take() {
                tokio::spawn(async move {
                    let _ = tokio::time::timeout(
                        std::time::Duration::from_secs(3),
                        tokio::fs::remove_dir_all(p),
                    ).await;
                });
            }
        }
        emit_browser(&js, serde_json::json!({ "type": "closed" }));
        emit_proxy(&js, serde_json::json!({ "type": "stopped" }));
    });

    let abort = task.abort_handle();
    let state2 = state.clone();
    handle.spawn(async move {
        let mut s = state2.lock().await;
        if let Some(old) = s.browser_capture_abort.take() {
            old.abort();
        }
        s.browser_capture_abort = Some(abort);
    });
}

pub(super) fn inspect_browser_close(
    state: Arc<Mutex<AppState>>,
    _eval_js: impl Fn(String) + Send + 'static,
) {
    let rt = tokio::runtime::Handle::try_current();
    if let Ok(handle) = rt {
        handle.spawn(async move {
            let mut s = state.lock().await;
            if let Some(h) = s.browser_capture_abort.take() {
                h.abort();
            }
            s.inspect_proxy_port = None;
            if let Some(p) = s.browser_capture_profile.take() {
                tokio::spawn(async move {
                    let _ = tokio::time::timeout(
                        std::time::Duration::from_secs(3),
                        tokio::fs::remove_dir_all(p),
                    )
                    .await;
                });
            }
        });
    }
}

pub(super) fn inspect_proxy_start(
    state: Arc<Mutex<AppState>>,
    data: serde_json::Value,
    eval_js: impl Fn(String) + Send + 'static,
) {
    let rt = tokio::runtime::Handle::try_current();
    let Ok(handle) = rt else { return };
    let port = data.get("port").and_then(|v| v.as_u64()).unwrap_or(0) as u16;
    let (js_tx, mut js_rx) = tokio::sync::mpsc::channel::<String>(512);
    handle.spawn(async move {
        while let Some(js) = js_rx.recv().await {
            eval_js(js);
        }
    });
    let js = js_tx.clone();

    fn emit(tx: &tokio::sync::mpsc::Sender<String>, payload: serde_json::Value) {
        let resp = IpcResponse::ok("inspector_proxy_event", payload);
        let _ = tx.try_send(format!(
            "window.__ipc_callback({})",
            serde_json::to_string(&resp).unwrap_or_default()
        ));
    }

    let state2 = state.clone();
    let task = handle.spawn(async move {
        {
            let mut s = state2.lock().await;
            if let Some(h) = s.inspect_proxy_abort.take() { h.abort(); }
            s.inspect_proxy_port = None;
        }

        let sidecar_path = { state2.lock().await.config.sidecar_path.clone() };
        let req_tx = {
            let mut s = state2.lock().await;
            match s.sidecar.get_or_start(&sidecar_path).await {
                Ok(tx) => tx,
                Err(e) => { emit(&js, serde_json::json!({ "type": "error", "message": format!("Sidecar: {e}") })); return; }
            }
        };

        let mut proxy_rx = {
            let s = state2.lock().await;
            match s.sidecar.proxy_event_tx.as_ref().map(|t| t.subscribe()) {
                Some(rx) => rx,
                None => { emit(&js, serde_json::json!({ "type": "error", "message": "No proxy channel" })); return; }
            }
        };

        let req_id = uuid::Uuid::new_v4().to_string();
        let (resp_tx, resp_rx) = tokio::sync::oneshot::channel();
        let body = if port > 0 { format!(r#"{{"port":{}}}"#, port) } else { "{}".to_string() };
        let mitm_req = ironbullet::sidecar::protocol::SidecarRequest {
            id: req_id,
            action: "start_mitm_proxy".to_string(),
            session: String::new(),
            body: Some(body),
            ..Default::default()
        };
        if req_tx.send((mitm_req, resp_tx)).await.is_err() { return; }
        let mitm_resp = match tokio::time::timeout(std::time::Duration::from_secs(10), resp_rx).await {
            Ok(Ok(r)) => r,
            _ => { emit(&js, serde_json::json!({ "type": "error", "message": "MITM start timed out" })); return; }
        };
        if let Some(e) = &mitm_resp.error {
            emit(&js, serde_json::json!({ "type": "error", "message": format!("{e}") }));
            return;
        }

        let info: serde_json::Value = serde_json::from_str(&mitm_resp.body).unwrap_or_default();
        let actual_port = info.get("port").and_then(|v| v.as_u64()).unwrap_or(8877) as u16;
        let ca_pem = info.get("ca_cert_pem").and_then(|v| v.as_str()).unwrap_or("").to_string();

        { state2.lock().await.inspect_proxy_port = Some(actual_port); }

        emit(&js, serde_json::json!({
            "type": "ready", "port": actual_port, "ca_cert_pem": ca_pem,
            "message": format!("MITM proxy on 127.0.0.1:{actual_port}. Install the CA cert to decrypt HTTPS.")
        }));

        // Forward events until aborted
        loop {
            match proxy_rx.recv().await {
                Ok(ev) => emit(&js, ev),
                Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
            }
        }
    });

    let abort = task.abort_handle();
    let state3 = state.clone();
    handle.spawn(async move {
        let mut s = state3.lock().await;
        s.inspect_proxy_abort = Some(abort);
    });
}

pub(super) fn inspect_proxy_stop(
    state: Arc<Mutex<AppState>>,
    _eval_js: impl Fn(String) + Send + 'static,
) {
    let rt = tokio::runtime::Handle::try_current();
    if let Ok(handle) = rt {
        handle.spawn(async move {
            let mut s = state.lock().await;
            if let Some(h) = s.inspect_proxy_abort.take() {
                h.abort();
            }
            s.inspect_proxy_port = None;
        });
    }
}
fn compute_ca_spki_hash(cert_pem: &str) -> Option<String> {
    use base64::Engine;
    use sha2::Digest;
    let der = rustls_pemfile::certs(&mut cert_pem.as_bytes())
        .next()?
        .ok()?;
    let spki = extract_spki_from_cert_der(&der)?;
    let hash = sha2::Sha256::digest(spki);
    Some(base64::engine::general_purpose::STANDARD.encode(hash))
}

fn extract_spki_from_cert_der(der: &[u8]) -> Option<Vec<u8>> {
    // Walk Certificate DER: SEQUENCE { tbsCertificate SEQUENCE { ... SPKI ... } }
    let cert_body = der_seq_body(der)?;
    let tbs_body = der_seq_body(cert_body)?;
    let mut pos = tbs_body;
    // version [0] optional
    if pos.first() == Some(&0xa0) {
        pos = der_skip(pos)?.0;
    }
    pos = der_skip(pos)?.0; // serialNumber
    pos = der_skip(pos)?.0; // signature
    pos = der_skip(pos)?.0; // issuer
    pos = der_skip(pos)?.0; // validity
    pos = der_skip(pos)?.0; // subject
                            // subjectPublicKeyInfo — return the full TLV
    let (rest, spki_tlv) = der_skip(pos)?;
    let _ = rest;
    Some(spki_tlv.to_vec())
}

fn der_seq_body(data: &[u8]) -> Option<&[u8]> {
    if data.first() != Some(&0x30) {
        return None;
    }
    let (len, hl) = der_len(&data[1..])?;
    Some(&data[1 + hl..1 + hl + len])
}

fn der_skip(data: &[u8]) -> Option<(&[u8], &[u8])> {
    if data.is_empty() {
        return None;
    }
    let (len, hl) = der_len(&data[1..])?;
    let end = 1 + hl + len;
    if end > data.len() {
        return None;
    }
    Some((&data[end..], &data[..end]))
}

fn der_len(data: &[u8]) -> Option<(usize, usize)> {
    let b = *data.first()? as usize;
    if b < 0x80 {
        return Some((b, 1));
    }
    let n = b & 0x7f;
    if n == 0 || n > 4 || data.len() < 1 + n {
        return None;
    }
    let mut len = 0usize;
    for i in 0..n {
        len = (len << 8) | data[1 + i] as usize;
    }
    Some((len, 1 + n))
}

/// Write the CA cert into the Chrome profile's NSS database so Chrome trusts it.
/// On Linux/Mac: uses certutil if available.
/// On Windows: imports into the Windows certificate store (CurrentUser\Root).
/// Falls back gracefully — Chrome's --ignore-certificate-errors handles the rest.
async fn install_ca_into_chrome_profile(profile_dir: &std::path::Path, ca_pem: &str) {
    // Write PEM to a temp file
    let pem_path = profile_dir.join("ib-ca.pem");
    if tokio::fs::write(&pem_path, ca_pem.as_bytes())
        .await
        .is_err()
    {
        return;
    }

    // Try certutil (ships with libnss3-tools on Linux, or nss on Mac)
    #[cfg(not(target_os = "windows"))]
    {
        // Chrome's NSS DB is at <profile>/Default/
        let nssdb = profile_dir.join("Default");
        let _ = tokio::fs::create_dir_all(&nssdb).await;
        // Try to find certutil
        for certutil_path in &["certutil", "/usr/bin/certutil", "/usr/local/bin/certutil"] {
            let out = tokio::process::Command::new(certutil_path)
                .args([
                    "-A",
                    "-n",
                    "IronBullet Inspector CA",
                    "-t",
                    "CT,,",
                    "-i",
                    pem_path.to_str().unwrap_or(""),
                    "-d",
                    &format!("sql:{}", nssdb.display()),
                ])
                .output()
                .await;
            if out.map(|o| o.status.success()).unwrap_or(false) {
                break;
            }
        }
    }

    // On Windows: add to CurrentUser\Root store
    #[cfg(target_os = "windows")]
    {
        // Use certutil.exe (built into Windows)
        let _ = tokio::process::Command::new("certutil")
            .args([
                "-addstore",
                "-user",
                "Root",
                pem_path.to_str().unwrap_or(""),
            ])
            .output()
            .await;
    }
}
