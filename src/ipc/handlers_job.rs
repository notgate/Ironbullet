use std::sync::Arc;
use tokio::sync::Mutex;

use ironbullet::export::format::RfxConfig;
use ironbullet::pipeline::{ProxyMode, ProxySettings, ProxySource, ProxySourceType};
use ironbullet::runner::job::{Job, JobType, ProxySourceConfig};

use super::{resolve_sidecar_path, AppState, IpcResponse};

pub(super) fn create_job(
    state: Arc<Mutex<AppState>>,
    data: serde_json::Value,
    eval_js: impl Fn(String) + Send + 'static,
) {
    let rt = tokio::runtime::Handle::try_current();
    if let Ok(handle) = rt {
        handle.spawn(async move {
            let mut s = state.lock().await;

            // Try full deserialization first; fall back to default + manual patching.
            // NOTE: the frontend only sends partial data, so deserialization almost always
            // fails (missing required `proxy_source` etc.).  We therefore always patch the
            // fields we care about explicitly below.
            let mut job = if let Ok(j) = serde_json::from_value::<Job>(data.clone()) {
                j
            } else {
                Job::default()
            };

            // ── job_type (CRITICAL — was never set before this fix) ───────────
            if let Some(jt) = data.get("job_type").and_then(|v| v.as_str()) {
                job.job_type = match jt {
                    "ProxyCheck" => JobType::ProxyCheck,
                    _ => JobType::Config,
                };
            }

            // ── basic fields ──────────────────────────────────────────────────
            if let Some(name) = data.get("name").and_then(|v| v.as_str()) {
                job.name = name.to_string();
            }
            if let Some(tc) = data.get("thread_count").and_then(|v| v.as_u64()) {
                job.thread_count = tc as usize;
            }

            // ── data source ───────────────────────────────────────────────────
            if let Some(ds) = data.get("data_source") {
                match serde_json::from_value::<ironbullet::runner::job::DataSource>(ds.clone()) {
                    Ok(new_ds) => {
                        eprintln!("[create_job] data_source: type={:?} value='{}'",
                            new_ds.source_type, new_ds.value);
                        job.data_source = new_ds;
                    }
                    Err(e) => {
                        // Deserialization failed — extract fields manually so we don't
                        // silently fall back to an empty data source.
                        eprintln!("[create_job] data_source deser failed: {} — raw: {}", e, ds);
                        let value = ds.get("value").and_then(|v| v.as_str()).unwrap_or("").to_string();
                        let source_type = match ds.get("source_type").and_then(|v| v.as_str()) {
                            Some("Folder") => ironbullet::runner::job::DataSourceType::Folder,
                            Some("Inline") => ironbullet::runner::job::DataSourceType::Inline,
                            Some("Url")    => ironbullet::runner::job::DataSourceType::Url,
                            Some("Range")  => ironbullet::runner::job::DataSourceType::Range,
                            _              => ironbullet::runner::job::DataSourceType::File,
                        };
                        job.data_source = ironbullet::runner::job::DataSource { source_type, value };
                    }
                }
            }

            // ── config_path: load pipeline from saved .opk/.svb/.rfx file ────────
            // When a config_path is provided the file takes precedence over the `pipeline` field.
            let config_path_str = data.get("config_path").and_then(|v| v.as_str()).unwrap_or("").to_string();
            if !config_path_str.is_empty() {
                match std::fs::read_to_string(&config_path_str) {
                    Ok(raw) => {
                        // Try parsing as-is (rfx/opk/svb are Pipeline JSON); also try RfxConfig wrapper
                        let loaded: Option<ironbullet::pipeline::Pipeline> =
                            serde_json::from_str::<ironbullet::pipeline::Pipeline>(&raw).ok()
                            .or_else(|| serde_json::from_str::<RfxConfig>(&raw).ok().map(|c| c.pipeline));
                        if let Some(mut pipeline) = loaded {
                            // Derive pipeline name from config filename if still default/untitled
                            let config_name = std::path::Path::new(&config_path_str)
                                .file_stem()
                                .and_then(|s| s.to_str())
                                .unwrap_or("Config");
                            // Check for default names: "New Config" or "New Config N" (frontend generates numbered defaults)
                            let is_default_name = pipeline.name.is_empty()
                                || pipeline.name == "New Config"
                                || pipeline.name.starts_with("New Config ");
                            if is_default_name {
                                pipeline.name = config_name.to_string();
                            }
                            eprintln!("[create_job] Loaded config from path: {} ({} blocks, name='{}')",
                                config_path_str, pipeline.blocks.len(), pipeline.name);
                            // Proxy groups are global (stored in GuiConfig, not per-file).
                            pipeline.proxy_settings.proxy_groups = s.config.proxy_groups.clone();
                            // Inherit proxy_mode + proxy_sources from the active in-memory pipeline
                            // so SOCKS5/HTTP proxies configured in the current session are always
                            // applied, even if the saved .rfx predates those settings (#58).
                            pipeline.proxy_settings.proxy_mode = s.pipeline.proxy_settings.proxy_mode.clone();
                            pipeline.proxy_settings.proxy_sources = s.pipeline.proxy_settings.proxy_sources.clone();
                            job.pipeline = pipeline;
                            job.config_path = Some(config_path_str);
                        } else {
                            eprintln!("[create_job] Failed to parse config at {}, falling back to active pipeline", config_path_str);
                            job.pipeline = s.pipeline.clone();
                        }
                    }
                    Err(e) => {
                        eprintln!("[create_job] Cannot read config file {}: {}", config_path_str, e);
                        job.pipeline = s.pipeline.clone();
                    }
                }
            } else {
                // ── pipeline snapshot from frontend (keeps proxy_settings etc. in sync) ──
                if let Some(pl) = data.get("pipeline") {
                    match serde_json::from_value::<ironbullet::pipeline::Pipeline>(pl.clone()) {
                        Ok(mut pipeline) => {
                            eprintln!("[create_job] Pipeline OK: {} blocks, proxy_mode={:?}, proxy_sources={}",
                                pipeline.blocks.len(),
                                pipeline.proxy_settings.proxy_mode,
                                pipeline.proxy_settings.proxy_sources.len());
                            // Fix #60: if pipeline has a default name ("New Config N") and the
                            // active tab was loaded from a file, rename using the file stem so
                            // the output file gets the correct name (e.g. httpbin_Success.txt).
                            let is_default_name = pipeline.name.is_empty()
                                || pipeline.name == "New Config"
                                || pipeline.name.starts_with("New Config ");
                            if is_default_name {
                                if let Some(ref path) = s.pipeline_path {
                                    let config_name = std::path::Path::new(path)
                                        .file_stem()
                                        .and_then(|s| s.to_str())
                                        .unwrap_or("Config");
                                    pipeline.name = config_name.to_string();
                                }
                            }
                            job.pipeline = pipeline;
                        }
                        Err(e) => {
                            eprintln!("[create_job] Pipeline deser FAILED: {}. Falling back to s.pipeline ({} blocks).",
                                e, s.pipeline.blocks.len());
                            job.pipeline = s.pipeline.clone();
                        }
                    }
                }
            }

            // (config_path already handled above - no need to reload)

            if let Some(url) = data.get("proxy_check_url").and_then(|v| v.as_str()) {
                if !url.is_empty() { job.proxy_check_url = url.to_string(); }
            }
            if let Some(list) = data.get("proxy_check_list").and_then(|v| v.as_str()) {
                if !list.is_empty() { job.proxy_check_list = list.to_string(); }
            }
            if let Some(pt) = data.get("proxy_check_type").and_then(|v| v.as_str()) {
                job.proxy_check_type = pt.to_string();
            }

            // ── custom user inputs (issue #62) ────────────────────────────────
            // Frontend sends custom_input_values: { "KEY": "value", ... }
            if let Some(vals) = data.get("custom_input_values").and_then(|v| v.as_object()) {
                for (k, v) in vals {
                    if let Some(val) = v.as_str() {
                        job.custom_input_values.insert(k.clone(), val.to_string());
                    }
                }
            }

            // ── per-job proxy override (config jobs only) ─────────────────────
            // proxy_override_mode: "pipeline" | "file" | "group"
            //   "file"  → use a specific proxy file, Rotate mode
            //   "group" → use one of the named proxy groups from the pipeline
            //   "pipeline" (default) → inherit pipeline's proxy_settings as-is
            if let Some(mode) = data.get("proxy_override_mode").and_then(|v| v.as_str()) {
                match mode {
                    "file" => {
                        if let Some(file) = data.get("proxy_override_file")
                            .and_then(|v| v.as_str()).filter(|s| !s.is_empty())
                        {
                            job.proxy_source = ProxySourceConfig {
                                settings: ProxySettings {
                                    proxy_mode: ProxyMode::Rotate,
                                    proxy_sources: vec![ProxySource {
                                        source_type: ProxySourceType::File,
                                        value: file.to_string(),
                                        refresh_interval_secs: 0,
                                        default_proxy_type: None,
                                    }],
                                    ..ProxySettings::default()
                                },
                            };
                            eprintln!("[create_job] proxy override: file = {}", file);
                        }
                    }
                    "group" => {
                        if let Some(group) = data.get("proxy_override_group")
                            .and_then(|v| v.as_str()).filter(|s| !s.is_empty())
                        {
                            // Clone the pipeline's proxy settings and override the active group.
                            // Auto-elevate proxy_mode from the group's own mode if the current
                            // mode is None — fixes #58/#59 where Sticky groups didn't activate.
                            let mut settings = job.pipeline.proxy_settings.clone();
                            settings.active_group = group.to_string();
                            if let Some(pg) = settings.proxy_groups.iter()
                                .chain(s.config.proxy_groups.iter())
                                .find(|g| g.name == group)
                            {
                                if matches!(settings.proxy_mode, ProxyMode::None) {
                                    settings.proxy_mode = pg.mode.clone();
                                    eprintln!("[create_job] auto-elevated proxy_mode to {:?} from group '{}'",
                                        pg.mode, group);
                                }
                            }
                            job.proxy_source = ProxySourceConfig { settings };
                            eprintln!("[create_job] proxy override: group = {}", group);
                        }
                    }
                    _ => { /* "pipeline" — inherit from job.pipeline.proxy_settings */ }
                }
            }

            // Use currently loaded pipeline for Config jobs unless a snapshot was provided
            if job.job_type == JobType::Config && data.get("pipeline").is_none() {
                job.pipeline = s.pipeline.clone();
            }

            let id = s.job_manager.add_job(job);
            let resp = IpcResponse::ok("job_created", serde_json::json!({ "id": id.to_string() }));
            eval_js(format!("window.__ipc_callback({})", serde_json::to_string(&resp).unwrap_or_default()));
        });
    }
}

pub(super) fn remove_job(
    state: Arc<Mutex<AppState>>,
    data: serde_json::Value,
    eval_js: impl Fn(String) + Send + 'static,
) {
    let rt = tokio::runtime::Handle::try_current();
    if let Ok(handle) = rt {
        handle.spawn(async move {
            let mut s = state.lock().await;
            if let Some(id) = data.get("id").and_then(|v| v.as_str()) {
                if let Ok(uuid) = uuid::Uuid::parse_str(id) {
                    s.job_manager.remove_job(uuid);
                }
            }
            let jobs = s.job_manager.list_jobs();
            let resp = IpcResponse::ok("jobs_list", serde_json::to_value(jobs).unwrap_or_default());
            eval_js(format!(
                "window.__ipc_callback({})",
                serde_json::to_string(&resp).unwrap_or_default()
            ));
        });
    }
}

pub(super) fn list_jobs(state: Arc<Mutex<AppState>>, eval_js: impl Fn(String) + Send + 'static) {
    let rt = tokio::runtime::Handle::try_current();
    if let Ok(handle) = rt {
        handle.spawn(async move {
            let mut s = state.lock().await;
            let jobs = s.job_manager.list_jobs();
            let resp = IpcResponse::ok("jobs_list", serde_json::to_value(jobs).unwrap_or_default());
            eval_js(format!(
                "window.__ipc_callback({})",
                serde_json::to_string(&resp).unwrap_or_default()
            ));
        });
    }
}

pub(super) fn start_job(
    state: Arc<Mutex<AppState>>,
    data: serde_json::Value,
    eval_js: impl Fn(String) + Send + 'static,
) {
    let rt = tokio::runtime::Handle::try_current();
    if let Ok(handle) = rt {
        let inner_handle = handle.clone();
        handle.spawn(async move {
            let mut s = state.lock().await;
            let id_str = data.get("id").and_then(|v| v.as_str()).unwrap_or("");
            let uuid = match uuid::Uuid::parse_str(id_str) {
                Ok(u) => u,
                Err(_) => {
                    let resp = IpcResponse::err("job_stats_update", "Invalid job ID".into());
                    eval_js(format!("window.__ipc_callback({})", serde_json::to_string(&resp).unwrap_or_default()));
                    return;
                }
            };

            // Proxy check jobs bypass the sidecar and runner entirely
            let is_proxy_check = s.job_manager.get_job_mut(uuid)
                .map(|j| j.job_type == ironbullet::runner::job::JobType::ProxyCheck)
                .unwrap_or(false);

            if is_proxy_check {
                let pc_rx = s.job_manager.start_proxy_check_job(uuid, inner_handle.clone());

                if pc_rx.is_none() {
                    // Proxy list was empty or unreadable
                    let resp = IpcResponse::err("job_start_error",
                        "Proxy list is empty or could not be read. \
                         Check that your proxy list file exists and is not empty.".into());
                    eval_js(format!("window.__ipc_callback({})",
                        serde_json::to_string(&resp).unwrap_or_default()));
                    let jobs = s.job_manager.list_jobs();
                    let jobs_resp = IpcResponse::ok("jobs_list", serde_json::to_value(jobs).unwrap_or_default());
                    eval_js(format!("window.__ipc_callback({})",
                        serde_json::to_string(&jobs_resp).unwrap_or_default()));
                    return;
                }

                // Immediate jobs_list so UI transitions Queued → Running right away
                {
                    let jobs = s.job_manager.list_jobs();
                    let resp = IpcResponse::ok("jobs_list", serde_json::to_value(jobs).unwrap_or_default());
                    eval_js(format!("window.__ipc_callback({})", serde_json::to_string(&resp).unwrap_or_default()));
                }
                drop(s);

                if let Some((mut hits_rx, generation)) = pc_rx {
                    let state2 = state.clone();
                    let state3 = state.clone();
                    let state4 = state.clone();
                    let (js_tx, mut js_rx) = tokio::sync::mpsc::channel::<String>(256);
                    inner_handle.spawn(async move { while let Some(js) = js_rx.recv().await { eval_js(js); } });

                    let js_tx2 = js_tx.clone();
                    inner_handle.spawn(async move {
                        while let Some(hit) = hits_rx.recv().await {
                            let mut s = state2.lock().await;
                            if !s.job_manager.is_current_generation(uuid, generation) {
                                break;
                            }
                            let status = hit.captures.get("status").map(|s| s.as_str()).unwrap_or("alive");
                            if status == "alive" {
                                s.job_manager.add_hit(uuid, hit.clone());
                            }
                            let hit_payload = serde_json::json!({
                                "job_id": uuid.to_string(),
                                "data_line": hit.data_line,
                                "captures": hit.captures,
                                "proxy": hit.proxy,
                            });
                            drop(s);
                            let hit_resp = IpcResponse::ok("runner_hit", hit_payload);
                            let _ = js_tx2.send(format!("window.__ipc_callback({})",
                                serde_json::to_string(&hit_resp).unwrap_or_default())).await;
                        }

                        // All tasks done — snapshot counters and preserve a
                        // manually requested Stopped state.
                        let mut s = state3.lock().await;
                        if !s.job_manager.complete_proxy_check_job(uuid, generation) {
                            return;
                        }
                        let jobs = s.job_manager.list_jobs();

                        let resp = IpcResponse::ok("jobs_list", serde_json::to_value(jobs).unwrap_or_default());
                        let _ = js_tx2.send(format!("window.__ipc_callback({})",
                            serde_json::to_string(&resp).unwrap_or_default())).await;
                    });

                    // ── Periodic stats push (500 ms) so dead-proxy progress shows too ──
                    let js_tx3 = js_tx.clone();
                    inner_handle.spawn(async move {
                        loop {
                            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                            let mut s = state4.lock().await;
                            if !s.job_manager.is_current_generation(uuid, generation) {
                                break;
                            }
                            let still_running = s.job_manager.get_job_mut(uuid)
                                .map(|j| j.state == ironbullet::runner::job::JobState::Running)
                                .unwrap_or(false);
                            if !still_running { break; }
                            s.job_manager.update_job_stats(uuid);
                            let jobs = s.job_manager.list_jobs();
                            let resp = IpcResponse::ok("jobs_list", serde_json::to_value(jobs).unwrap_or_default());
                            let _ = js_tx3.send(format!("window.__ipc_callback({})",
                                serde_json::to_string(&resp).unwrap_or_default())).await;
                        }
                    });
                }
                return;
            }

            // ── Preflight: verify referenced files exist before starting ──
            {
                let mut missing: Vec<String> = Vec::new();
                if let Some(job) = s.job_manager.get_job_mut(uuid) {
                    // Wordlist / data source
                    if job.data_source.source_type == ironbullet::runner::job::DataSourceType::File
                        && !job.data_source.value.is_empty()
                    {
                        let p = std::path::Path::new(&job.data_source.value);
                        if !p.exists() {
                            missing.push(format!("Wordlist: {}", job.data_source.value));
                        }
                    }
                    // Proxy file sources
                    for src in &job.proxy_source.settings.proxy_sources {
                        if src.source_type == ironbullet::pipeline::ProxySourceType::File
                            && !src.value.is_empty()
                        {
                            let p = std::path::Path::new(&src.value);
                            if !p.exists() {
                                missing.push(format!("Proxy list: {}", src.value));
                            }
                        }
                    }
                }
                if !missing.is_empty() {
                    let msg = format!(
                        "Cannot start job — the following files were not found:\n{}",
                        missing.join("\n")
                    );
                    let resp = IpcResponse::err("job_preflight_error", msg);
                    eval_js(format!("window.__ipc_callback({})",
                        serde_json::to_string(&resp).unwrap_or_default()));
                    return;
                }
            }

            // Get or start sidecar (reuses existing process if already running)
            let sidecar_path = resolve_sidecar_path(&s.config.sidecar_path);
            let sidecar_tx = match s.sidecar.get_or_start(&sidecar_path).await {
                Ok(tx) => tx,
                Err(e) => {
                    let resp = IpcResponse::err("job_stats_update", format!("Failed to start sidecar: {}", e));
                    eval_js(format!("window.__ipc_callback({})", serde_json::to_string(&resp).unwrap_or_default()));
                    return;
                }
            };

            // Health check: verify sidecar channel is not closed immediately after start
            if sidecar_tx.is_closed() {
                let resp = IpcResponse::err("job_start_error", "Sidecar process died immediately after start. Check that reqflow-sidecar is a valid executable.".to_string());
                eval_js(format!("window.__ipc_callback({})", serde_json::to_string(&resp).unwrap_or_default()));
                let jobs = s.job_manager.list_jobs();
                let jobs_resp = IpcResponse::ok("jobs_list", serde_json::to_value(jobs).unwrap_or_default());
                eval_js(format!("window.__ipc_callback({})", serde_json::to_string(&jobs_resp).unwrap_or_default()));
                return;
            }

            let pm = s.plugin_manager.clone();
            let chrome_exe = {
                let cfg_path = s.config.chrome_executable_path.clone();
                if !cfg_path.is_empty() {
                    let p = std::path::PathBuf::from(&cfg_path);
                    if p.exists() { Some(p) } else { super::find_chrome_executable() }
                } else {
                    super::find_chrome_executable()
                }
            };
            let result = s.job_manager.start_job(uuid, sidecar_tx, Some(pm), chrome_exe);

            if result.is_none() {
                // Data source was empty or unreadable — surface a clear error with path info.
                let ds_value = s.job_manager.get_job_mut(uuid)
                    .map(|j| j.data_source.value.clone())
                    .unwrap_or_default();
                let msg = if ds_value.is_empty() {
                    "No wordlist configured. Set a data source path in the job settings.".to_string()
                } else {
                    format!("Wordlist '{}' could not be read or is empty. Check the path and file contents.", ds_value)
                };
                let resp = IpcResponse::err("job_start_error", msg);
                eval_js(format!("window.__ipc_callback({})",
                    serde_json::to_string(&resp).unwrap_or_default()));
                let jobs = s.job_manager.list_jobs();
                let jobs_resp = IpcResponse::ok("jobs_list", serde_json::to_value(jobs).unwrap_or_default());
                eval_js(format!("window.__ipc_callback({})",
                    serde_json::to_string(&jobs_resp).unwrap_or_default()));
                return;
            }
            drop(s);

            if let Some((runner, mut hits_rx, generation)) = result {
                let job_id = uuid;
                let state2 = state.clone();

                // Single channel → single eval_js consumer (eval_js can only be moved once)
                let (js_tx, mut js_rx) = tokio::sync::mpsc::channel::<String>(512);
                inner_handle.spawn(async move {
                    while let Some(js) = js_rx.recv().await { eval_js(js); }
                });

                // Hit collector: store hit AND broadcast runner_hit for immediate live append
                let js_hit = js_tx.clone();
                inner_handle.spawn(async move {
                    while let Some(hit) = hits_rx.recv().await {
                        let mut s = state2.lock().await;
                        if !s.job_manager.is_current_generation(job_id, generation) {
                            break;
                        }
                        s.job_manager.add_hit(job_id, hit.clone());
                        // Push runner_hit with job_id so frontend routes it to the correct
                        // per-job hits view in the Hits Database panel.
                        let hit_payload = serde_json::json!({
                            "job_id": job_id.to_string(),
                            "data_line": hit.data_line,
                            "captures": hit.captures,
                            "proxy": hit.proxy,
                        });
                        let resp = IpcResponse::ok("runner_hit", hit_payload);
                        let _ = js_hit.send(format!("window.__ipc_callback({})",
                            serde_json::to_string(&resp).unwrap_or_default())).await;
                    }
                });

                // Spawn periodic stats push
                let state3 = state.clone();
                let js_tx2 = js_tx.clone();
                let runner2 = runner.clone();
                inner_handle.spawn(async move {
                    loop {
                        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                        let mut s = state3.lock().await;
                        if !s.job_manager.is_current_generation(job_id, generation) {
                            break;
                        }
                        s.job_manager.update_job_stats(job_id);
                        let jobs = s.job_manager.list_jobs();
                        let resp = IpcResponse::ok("jobs_list", serde_json::to_value(jobs).unwrap_or_default());
                        let _ = js_tx2.send(format!("window.__ipc_callback({})", serde_json::to_string(&resp).unwrap_or_default())).await;
                        if !runner2.is_running() { break; }
                    }
                });

                let state4 = state.clone();
                inner_handle.spawn(async move {
                    runner.start().await;
                    let mut s = state4.lock().await;
                    if !s.job_manager.complete_job(job_id, generation) {
                        return;
                    }
                    let jobs = s.job_manager.list_jobs();
                    let resp = IpcResponse::ok("jobs_list", serde_json::to_value(jobs).unwrap_or_default());
                    let _ = js_tx.send(format!("window.__ipc_callback({})", serde_json::to_string(&resp).unwrap_or_default())).await;
                });
            }
        });
    }
}

pub(super) fn pause_job(
    state: Arc<Mutex<AppState>>,
    data: serde_json::Value,
    eval_js: impl Fn(String) + Send + 'static,
) {
    let rt = tokio::runtime::Handle::try_current();
    if let Ok(handle) = rt {
        handle.spawn(async move {
            let mut s = state.lock().await;
            if let Some(id) = data.get("id").and_then(|v| v.as_str()) {
                if let Ok(uuid) = uuid::Uuid::parse_str(id) {
                    s.job_manager.pause_job(uuid);
                }
            }
            let jobs = s.job_manager.list_jobs();
            let resp = IpcResponse::ok("jobs_list", serde_json::to_value(jobs).unwrap_or_default());
            eval_js(format!(
                "window.__ipc_callback({})",
                serde_json::to_string(&resp).unwrap_or_default()
            ));
        });
    }
}

pub(super) fn resume_job(
    state: Arc<Mutex<AppState>>,
    data: serde_json::Value,
    eval_js: impl Fn(String) + Send + 'static,
) {
    let rt = tokio::runtime::Handle::try_current();
    if let Ok(handle) = rt {
        handle.spawn(async move {
            let mut s = state.lock().await;
            if let Some(id) = data.get("id").and_then(|v| v.as_str()) {
                if let Ok(uuid) = uuid::Uuid::parse_str(id) {
                    s.job_manager.resume_job(uuid);
                }
            }
            let jobs = s.job_manager.list_jobs();
            let resp = IpcResponse::ok("jobs_list", serde_json::to_value(jobs).unwrap_or_default());
            eval_js(format!(
                "window.__ipc_callback({})",
                serde_json::to_string(&resp).unwrap_or_default()
            ));
        });
    }
}

pub(super) fn stop_job(
    state: Arc<Mutex<AppState>>,
    data: serde_json::Value,
    eval_js: impl Fn(String) + Send + 'static,
) {
    let rt = tokio::runtime::Handle::try_current();
    if let Ok(handle) = rt {
        handle.spawn(async move {
            let mut s = state.lock().await;
            if let Some(id) = data.get("id").and_then(|v| v.as_str()) {
                if let Ok(uuid) = uuid::Uuid::parse_str(id) {
                    s.job_manager.stop_job(uuid);
                    // Kill sidecar if no config jobs remain running.
                    // Proxy-check jobs don't use the sidecar, so only check config jobs.
                    if !s.job_manager.any_config_job_running() {
                        s.sidecar.stop().await;
                        eprintln!("[stop_job] No running config jobs — sidecar stopped.");
                    }
                }
            }
            let jobs = s.job_manager.list_jobs();
            let resp = IpcResponse::ok("jobs_list", serde_json::to_value(jobs).unwrap_or_default());
            eval_js(format!(
                "window.__ipc_callback({})",
                serde_json::to_string(&resp).unwrap_or_default()
            ));
        });
    }
}

pub(super) fn get_job_stats(
    state: Arc<Mutex<AppState>>,
    data: serde_json::Value,
    eval_js: impl Fn(String) + Send + 'static,
) {
    let rt = tokio::runtime::Handle::try_current();
    if let Ok(handle) = rt {
        handle.spawn(async move {
            let mut s = state.lock().await;
            if let Some(id) = data.get("id").and_then(|v| v.as_str()) {
                if let Ok(uuid) = uuid::Uuid::parse_str(id) {
                    s.job_manager.update_job_stats(uuid);
                    let stats = s.job_manager.get_job_stats(uuid);
                    let resp = IpcResponse::ok(
                        "job_stats_update",
                        serde_json::json!({
                            "id": id,
                            "stats": serde_json::to_value(&stats).unwrap_or_default(),
                        }),
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

pub(super) fn get_job_debug_log(
    state: Arc<Mutex<AppState>>,
    data: serde_json::Value,
    eval_js: impl Fn(String) + Send + 'static,
) {
    let rt = tokio::runtime::Handle::try_current();
    if let Ok(handle) = rt {
        handle.spawn(async move {
            let s = state.lock().await;
            if let Some(id) = data.get("id").and_then(|v| v.as_str()) {
                if let Ok(uuid) = uuid::Uuid::parse_str(id) {
                    let filter = data
                        .get("filter")
                        .and_then(|v| v.as_str())
                        .unwrap_or("ALL")
                        .to_string();
                    let results: Vec<_> = s
                        .job_manager
                        .get_job_stats_full(uuid)
                        .map(|stats| {
                            stats
                                .recent_results
                                .into_iter()
                                .filter(|r| filter == "ALL" || r.status == filter)
                                .collect()
                        })
                        .unwrap_or_default();
                    let resp = IpcResponse::ok(
                        "job_debug_log",
                        serde_json::json!({
                            "id": id,
                            "results": serde_json::to_value(&results).unwrap_or_default(),
                        }),
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

pub(super) fn get_job_hits(
    state: Arc<Mutex<AppState>>,
    data: serde_json::Value,
    eval_js: impl Fn(String) + Send + 'static,
) {
    let rt = tokio::runtime::Handle::try_current();
    if let Ok(handle) = rt {
        handle.spawn(async move {
            let s = state.lock().await;
            if let Some(id) = data.get("id").and_then(|v| v.as_str()) {
                if let Ok(uuid) = uuid::Uuid::parse_str(id) {
                    let hits = s.job_manager.get_job_hits(uuid);
                    let resp = IpcResponse::ok(
                        "job_hits",
                        serde_json::json!({
                            "id": id,
                            "hits": serde_json::to_value(&hits).unwrap_or_default(),
                        }),
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

pub(super) fn update_job(
    state: Arc<Mutex<AppState>>,
    data: serde_json::Value,
    eval_js: impl Fn(String) + Send + 'static,
) {
    let rt = tokio::runtime::Handle::try_current();
    if let Ok(handle) = rt {
        handle.spawn(async move {
            let mut s = state.lock().await;
            let id_str = data
                .get("id")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            if let Ok(uuid) = uuid::Uuid::parse_str(&id_str) {
                if let Some(job) = s.job_manager.get_job_mut(uuid) {
                    if let Some(name) = data.get("name").and_then(|v| v.as_str()) {
                        job.name = name.to_string();
                    }
                    if let Some(tc) = data.get("thread_count").and_then(|v| v.as_u64()) {
                        job.thread_count = tc as usize;
                    }
                    if let Some(ds) = data.get("data_source") {
                        if let Ok(new_ds) = serde_json::from_value::<
                            ironbullet::runner::job::DataSource,
                        >(ds.clone())
                        {
                            job.data_source = new_ds;
                        }
                    }
                    if let Some(url) = data.get("proxy_check_url").and_then(|v| v.as_str()) {
                        job.proxy_check_url = url.to_string();
                    }
                    if let Some(list) = data.get("proxy_check_list").and_then(|v| v.as_str()) {
                        job.proxy_check_list = list.to_string();
                    }
                    if let Some(pt) = data.get("proxy_check_type").and_then(|v| v.as_str()) {
                        job.proxy_check_type = pt.to_string();
                    }
                }
            }
            let resp = IpcResponse::ok("job_updated", serde_json::json!({ "id": id_str }));
            eval_js(format!(
                "window.__ipc_callback({})",
                serde_json::to_string(&resp).unwrap_or_default()
            ));
        });
    }
}
