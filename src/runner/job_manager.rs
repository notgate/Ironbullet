use std::collections::HashMap;
use std::sync::Arc;

use chrono::Utc;
use tokio::sync::{mpsc, oneshot};
use uuid::Uuid;

use super::data_pool::DataPool;
use super::job::{DataSourceType, Job, JobState, StartCondition};
use super::proxy_pool::{ProxyEntry, ProxyPool, ProxyType};
use super::{HitResult, RunnerOrchestrator, RunnerSetup, RunnerStats};
use crate::pipeline::{ProxyMode, ProxySettings, ProxySourceType};
use crate::sidecar::protocol::{SidecarRequest, SidecarResponse};

/// Build a ProxyPool and resolve the effective ProxyMode.
/// Returns (pool, effective_mode) — the mode accounts for group-level overrides
/// so Sticky groups auto-elevate even when the top-level mode is None (#58/#59).
fn build_proxy_pool(settings: &ProxySettings) -> (ProxyPool, ProxyMode) {
    let (sources, effective_mode) = if !settings.active_group.is_empty() {
        if let Some(g) = settings
            .proxy_groups
            .iter()
            .find(|g| g.name == settings.active_group)
        {
            let mode = if matches!(settings.proxy_mode, ProxyMode::None) {
                g.mode.clone()
            } else {
                settings.proxy_mode.clone()
            };
            (g.sources.as_slice(), mode)
        } else {
            (&settings.proxy_sources[..], settings.proxy_mode.clone())
        }
    } else {
        (&settings.proxy_sources[..], settings.proxy_mode.clone())
    };

    if matches!(effective_mode, ProxyMode::None) || sources.is_empty() {
        return (ProxyPool::empty(), ProxyMode::None);
    }

    let mut entries: Vec<ProxyEntry> = Vec::new();
    for src in sources {
        let raw_lines: Vec<String> = match src.source_type {
            ProxySourceType::File => std::fs::read_to_string(&src.value)
                .map(|c| {
                    c.lines()
                        .filter(|l| !l.trim().is_empty())
                        .map(|l| l.trim().to_string())
                        .collect()
                })
                .unwrap_or_default(),
            ProxySourceType::Inline => src
                .value
                .lines()
                .filter(|l| !l.trim().is_empty())
                .map(|l| l.trim().to_string())
                .collect(),
            ProxySourceType::Url => {
                // URL sources need async; skip for now (they're resolved at runtime)
                Vec::new()
            }
        };
        // Resolve the source-level default type override (e.g. socks5 for a plain ip:port list)
        let default_type =
            src.default_proxy_type
                .as_deref()
                .and_then(|t| match t.to_lowercase().as_str() {
                    "http" => Some(ProxyType::Http),
                    "https" => Some(ProxyType::Https),
                    "socks4" => Some(ProxyType::Socks4),
                    "socks5" => Some(ProxyType::Socks5),
                    _ => None,
                });
        entries.extend(
            raw_lines
                .into_iter()
                .filter_map(|l| parse_proxy_for_pool(&l, default_type)),
        );
    }

    (
        ProxyPool::new(entries, settings.ban_duration_secs),
        effective_mode,
    )
}

/// Detect proxy type from port number for untyped host:port lines.
/// Common SOCKS5 ports → Socks5, otherwise → fallback type.
fn detect_proxy_type_from_port(port_str: &str, fallback: ProxyType) -> ProxyType {
    if let Ok(port) = port_str.parse::<u16>() {
        match port {
            // Standard SOCKS5 ports
            1080 | 1081 => ProxyType::Socks5,
            // Tor SOCKS5 ports
            9050 | 9150 => ProxyType::Socks5,
            // Shadowsocks common local ports (when ss:// wasn't used)
            1086 | 1087 | 1088 => ProxyType::Socks5,
            // All other ports → use fallback
            _ => fallback,
        }
    } else {
        fallback
    }
}

/// Parse a single proxy line into a ProxyEntry.
/// `default_type` is used for plain `host:port` or `host:port:user:pass` lines that
/// carry no protocol prefix — it lets a whole source be declared as SOCKS5, for example.
///
/// FIX #56: Auto-detect SOCKS5 from common ports (1080, 1081, 9050, 9150) when no
/// explicit type is provided. This fixes the issue where SOCKS5 proxies work in
/// Debugger but fail in Jobs mode (plain host:port was defaulting to HTTP).
fn parse_proxy_for_pool(line: &str, default_type: Option<ProxyType>) -> Option<ProxyEntry> {
    // Strip SIP002 / URL fragment labels (e.g. `ss://...@host:port#Dubai%2C%20UAE`)
    let line = if let Some(pos) = line.find('#') {
        &line[..pos]
    } else {
        line
    };
    let line = line.trim();
    if line.is_empty() {
        return None;
    }

    let (proxy_type, address) = if crate::sidecar::xray_pool::supports_uri(line) {
        match crate::sidecar::xray_pool::resolve_proxy_uri(line) {
            Ok(local) => (
                ProxyType::Socks5,
                local
                    .strip_prefix("socks5://")
                    .unwrap_or(&local)
                    .to_string(),
            ),
            Err(error) => {
                eprintln!("[xray-pool] failed to resolve proxy URI: {error}");
                return None;
            }
        }
    } else if line.starts_with("ss://") {
        // Shadowsocks — spin up (or reuse) a local SOCKS5 tunnel and use that.
        let local = crate::sidecar::shadowsocks_pool::resolve_ss_proxy(line);
        // local is `socks5://127.0.0.1:<port>` — parse it as a regular Socks5 entry
        let rest = local.strip_prefix("socks5://").unwrap_or(&local);
        (ProxyType::Socks5, rest.to_string())
    } else if let Some(rest) = line.strip_prefix("socks5://") {
        (ProxyType::Socks5, rest.to_string())
    } else if let Some(rest) = line.strip_prefix("socks4://") {
        (ProxyType::Socks4, rest.to_string())
    } else if let Some(rest) = line.strip_prefix("https://") {
        (ProxyType::Https, rest.to_string())
    } else if let Some(rest) = line.strip_prefix("http://") {
        (ProxyType::Http, rest.to_string())
    } else {
        let parts: Vec<&str> = line.split(':').collect();
        match parts.len() {
            // host:port — intelligent detection
            2 => {
                let fallback = default_type.unwrap_or(ProxyType::Http);
                let detected_type = detect_proxy_type_from_port(parts[1], fallback);
                (detected_type, format!("{}:{}", parts[0], parts[1]))
            }
            // ip:port:user:pass — preserve source-level type, inject credentials
            4 => {
                let fallback = default_type.unwrap_or(ProxyType::Http);
                let detected_type = detect_proxy_type_from_port(parts[1], fallback);
                (
                    detected_type,
                    format!("{}:{}@{}:{}", parts[2], parts[3], parts[0], parts[1]),
                )
            }
            // type:host:port:user:pass
            5 => {
                let pt = match parts[0].to_lowercase().as_str() {
                    "https" => ProxyType::Https,
                    "socks4" => ProxyType::Socks4,
                    "socks5" => ProxyType::Socks5,
                    _ => ProxyType::Http,
                };
                (
                    pt,
                    format!("{}:{}@{}:{}", parts[3], parts[4], parts[1], parts[2]),
                )
            }
            _ => return None,
        }
    };
    Some(ProxyEntry {
        proxy_type,
        address,
    })
}

pub struct ProxyCheckHandle {
    pub processed: Arc<std::sync::atomic::AtomicUsize>,
    pub hits: Arc<std::sync::atomic::AtomicUsize>,
    pub fails: Arc<std::sync::atomic::AtomicUsize>,
    pub errors: Arc<std::sync::atomic::AtomicUsize>,
    pub active_threads: Arc<std::sync::atomic::AtomicUsize>,
    pub total: usize,
    pub running: Arc<std::sync::atomic::AtomicBool>,
    pub start_ms: u64,
}

impl ProxyCheckHandle {
    pub fn get_stats(&self) -> RunnerStats {
        use std::sync::atomic::Ordering;
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        let elapsed_secs = (now.saturating_sub(self.start_ms)) as f64 / 1000.0;
        let processed = self.processed.load(Ordering::Relaxed);
        let cpm = if elapsed_secs > 0.0 {
            processed as f64 / elapsed_secs * 60.0
        } else {
            0.0
        };
        RunnerStats {
            total: self.total,
            processed,
            consumed: processed,
            hits: self.hits.load(Ordering::Relaxed),
            fails: self.fails.load(Ordering::Relaxed),
            bans: 0,
            retries: 0,
            errors: self.errors.load(Ordering::Relaxed),
            to_check: 0,
            cpm,
            active_threads: self.active_threads.load(Ordering::Relaxed),
            elapsed_secs,
            recent_results: vec![],
        }
    }
}

pub struct JobManager {
    jobs: Vec<Job>,
    runners: HashMap<Uuid, Arc<RunnerOrchestrator>>,
    /// Per-job hits database (used by both config and proxy-check jobs)
    job_hits: HashMap<Uuid, Vec<HitResult>>,
    /// Stats handles for proxy-check jobs (no RunnerOrchestrator)
    proxy_handles: HashMap<Uuid, ProxyCheckHandle>,
}

impl JobManager {
    pub fn new() -> Self {
        Self {
            jobs: Vec::new(),
            runners: HashMap::new(),
            job_hits: HashMap::new(),
            proxy_handles: HashMap::new(),
        }
    }

    pub fn add_job(&mut self, job: Job) -> Uuid {
        let id = job.id;
        self.jobs.push(job);
        self.job_hits.insert(id, Vec::new());
        id
    }

    pub fn remove_job(&mut self, id: Uuid) -> bool {
        if let Some(runner) = self.runners.get(&id) {
            runner.stop();
        }
        if let Some(h) = self.proxy_handles.get(&id) {
            h.running.store(false, std::sync::atomic::Ordering::SeqCst);
        }
        self.runners.remove(&id);
        self.proxy_handles.remove(&id);
        self.job_hits.remove(&id);
        let len = self.jobs.len();
        self.jobs.retain(|j| j.id != id);
        self.jobs.len() < len
    }

    pub fn list_jobs(&mut self) -> &[Job] {
        // Refresh live stats for all running jobs before serializing so the UI
        // always sees current hits/fails/errors without a separate poll call.
        let running_ids: Vec<Uuid> = self
            .jobs
            .iter()
            .filter(|j| j.state == super::job::JobState::Running)
            .map(|j| j.id)
            .collect();
        for id in running_ids {
            self.update_job_stats(id);
        }
        &self.jobs
    }

    pub fn get_job_mut(&mut self, id: Uuid) -> Option<&mut Job> {
        self.jobs.iter_mut().find(|j| j.id == id)
    }

    pub fn get_job_stats(&self, id: Uuid) -> Option<RunnerStats> {
        if let Some(r) = self.runners.get(&id) {
            return Some(r.get_stats());
        }
        self.proxy_handles.get(&id).map(|h| h.get_stats())
    }

    /// Like get_job_stats but includes recent_results (block-level debug log).
    /// Only call this from get_job_debug_log — never from jobs_list broadcasts.
    pub fn get_job_stats_full(&self, id: Uuid) -> Option<RunnerStats> {
        if let Some(r) = self.runners.get(&id) {
            return Some(r.get_stats_full());
        }
        self.proxy_handles.get(&id).map(|h| h.get_stats())
    }

    pub fn get_job_hits(&self, id: Uuid) -> Vec<HitResult> {
        self.job_hits.get(&id).cloned().unwrap_or_default()
    }

    /// Return only hits newer than `since_index` (0-based).
    /// Used by the frontend for incremental updates instead of re-sending the full list.
    pub fn get_job_hits_since(&self, id: Uuid, since_index: usize) -> Vec<HitResult> {
        self.job_hits
            .get(&id)
            .map(|hits| hits[since_index.min(hits.len())..].to_vec())
            .unwrap_or_default()
    }

    pub fn get_job_hit_count(&self, id: Uuid) -> usize {
        self.job_hits.get(&id).map(|h| h.len()).unwrap_or(0)
    }

    pub fn add_hit(&mut self, id: Uuid, hit: HitResult) {
        if let Some(hits) = self.job_hits.get_mut(&id) {
            hits.push(hit);
        }
    }

    pub fn pause_job(&mut self, id: Uuid) -> bool {
        if let Some(runner) = self.runners.get(&id) {
            runner.pause();
            if let Some(job) = self.get_job_mut(id) {
                job.state = JobState::Paused;
            }
            true
        } else {
            false
        }
    }

    pub fn resume_job(&mut self, id: Uuid) -> bool {
        if let Some(runner) = self.runners.get(&id) {
            runner.resume();
            if let Some(job) = self.get_job_mut(id) {
                job.state = JobState::Running;
            }
            true
        } else {
            false
        }
    }

    /// Stop a job.  Returns whether a job was found.
    /// For config jobs: signals the RunnerOrchestrator to stop workers.
    /// For proxy-check jobs: sets the cancellation flag so in-flight tasks abort early.
    pub fn stop_job(&mut self, id: Uuid) -> bool {
        // Config job runner
        if let Some(runner) = self.runners.get(&id) {
            runner.stop();
        }
        // Proxy-check cancellation flag
        if let Some(h) = self.proxy_handles.get(&id) {
            h.running.store(false, std::sync::atomic::Ordering::SeqCst);
        }
        if let Some(job) = self.get_job_mut(id) {
            job.state = JobState::Stopped;
            job.completed = Some(Utc::now());
            true
        } else {
            false
        }
    }

    /// Returns true if any config job is currently Running.
    /// Used to decide whether to tear down the shared sidecar process on stop.
    pub fn any_config_job_running(&self) -> bool {
        use crate::runner::job::JobType;
        self.jobs
            .iter()
            .any(|j| j.job_type == JobType::Config && j.state == JobState::Running)
    }

    pub fn start_job(
        &mut self,
        id: Uuid,
        sidecar_tx: mpsc::Sender<(SidecarRequest, oneshot::Sender<SidecarResponse>)>,
        plugin_manager: Option<Arc<crate::plugin::manager::PluginManager>>,
        chrome_executable_path: Option<std::path::PathBuf>,
    ) -> Option<(Arc<RunnerOrchestrator>, mpsc::Receiver<HitResult>)> {
        let job = self.jobs.iter_mut().find(|j| j.id == id)?;

        // Load data from source
        let data_lines = match job.data_source.source_type {
            DataSourceType::File => match std::fs::read_to_string(&job.data_source.value) {
                Ok(content) => content
                    .lines()
                    .filter(|l| !l.trim().is_empty())
                    .map(|l| l.to_string())
                    .collect::<Vec<_>>(),
                Err(e) => {
                    eprintln!(
                        "[job] start_job: cannot read file '{}': {}",
                        job.data_source.value, e
                    );
                    Vec::new()
                }
            },
            DataSourceType::Folder => {
                // Read all .txt / .csv files in the folder, concatenate their lines
                let mut all_lines: Vec<String> = Vec::new();
                if let Ok(rd) = std::fs::read_dir(&job.data_source.value) {
                    let mut paths: Vec<_> = rd
                        .filter_map(|e| e.ok())
                        .map(|e| e.path())
                        .filter(|p| {
                            p.is_file()
                                && matches!(
                                    p.extension().and_then(|s| s.to_str()),
                                    Some("txt") | Some("csv") | Some("lst") | Some("dat")
                                )
                        })
                        .collect();
                    paths.sort();
                    for path in paths {
                        if let Ok(content) = std::fs::read_to_string(&path) {
                            all_lines.extend(
                                content
                                    .lines()
                                    .filter(|l| !l.trim().is_empty())
                                    .map(|l| l.to_string()),
                            );
                        }
                    }
                }
                all_lines
            }
            DataSourceType::Inline => job
                .data_source
                .value
                .lines()
                .filter(|l| !l.trim().is_empty())
                .map(|l| l.to_string())
                .collect::<Vec<_>>(),
            _ => Vec::new(),
        };

        // Guard: if the data source resolved to zero lines, refuse to start rather
        // than silently completing at 0%. The caller receives None and should surface
        // an error to the frontend.
        if data_lines.is_empty() {
            eprintln!("[job] start_job: data source '{:?}' value='{}' resolved to 0 lines — aborting start",
                job.data_source.source_type, job.data_source.value);
            return None;
        }

        let data_pool = DataPool::with_limits(
            data_lines,
            job.pipeline.runner_settings.skip as usize,
            job.pipeline.runner_settings.take as usize,
        );
        let proxy_settings_ref = if !matches!(job.proxy_source.settings.proxy_mode, ProxyMode::None)
        {
            &job.proxy_source.settings
        } else {
            &job.pipeline.proxy_settings
        };
        let (proxy_pool, proxy_mode) = build_proxy_pool(proxy_settings_ref);
        let (hits_tx, hits_rx) = mpsc::channel::<HitResult>(1024);

        let runner = Arc::new(RunnerOrchestrator::new(RunnerSetup {
            pipeline: job.pipeline.clone(),
            proxy_mode,
            data_pool,
            proxy_pool,
            sidecar_tx,
            thread_count: job.thread_count,
            hits_tx,
            plugin_manager,
            chrome_executable_path,
            custom_input_values: job.custom_input_values.clone(),
        }));

        job.state = JobState::Running;
        job.started = Some(Utc::now());
        self.runners.insert(id, runner.clone());

        Some((runner, hits_rx))
    }

    /// Check start conditions for queued jobs (delayed/scheduled)
    pub fn tick(&mut self) -> Vec<Uuid> {
        let now = Utc::now();
        let mut ready = Vec::new();

        for job in &mut self.jobs {
            if job.state != JobState::Queued {
                continue;
            }
            let should_start = match &job.start_condition {
                StartCondition::Immediate => true,
                StartCondition::Delayed { delay_secs } => {
                    let elapsed = (now - job.created).num_seconds();
                    elapsed >= *delay_secs as i64
                }
                StartCondition::Scheduled { at } => now >= *at,
            };
            if should_start {
                job.state = JobState::Waiting;
                ready.push(job.id);
            }
        }

        ready
    }

    pub fn update_job_stats(&mut self, id: Uuid) {
        if let Some(stats) = self.get_job_stats(id) {
            // recent_results is already populated by get_stats() from the ring buffer —
            // do NOT clear it here, that was discarding the live feed data.
            if let Some(job) = self.get_job_mut(id) {
                job.stats = stats;
            }
        }
    }

    pub fn complete_job(&mut self, id: Uuid) {
        self.update_job_stats(id);
        self.runners.remove(&id);
        if let Some(job) = self.get_job_mut(id) {
            job.state = super::job::JobState::Completed;
            job.completed = Some(chrono::Utc::now());
        }
    }
}

impl JobManager {
    pub fn start_proxy_check_job(
        &mut self,
        id: Uuid,
        handle: tokio::runtime::Handle,
    ) -> Option<tokio::sync::mpsc::Receiver<HitResult>> {
        use crate::runner::job::JobType;
        use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

        let job = self.jobs.iter_mut().find(|j| j.id == id)?;
        if job.job_type != JobType::ProxyCheck {
            return None;
        }

        let proxy_list_path = job.proxy_check_list.clone();
        let check_url = job.proxy_check_url.clone();
        let thread_count = job.thread_count.max(1);
        let proxy_check_type = job.proxy_check_type.clone();
        job.state = JobState::Running;
        job.started = Some(Utc::now());

        let proxies: Vec<String> = if proxy_list_path.is_empty() {
            Vec::new()
        } else {
            std::fs::read_to_string(&proxy_list_path)
                .unwrap_or_default()
                .lines()
                .filter(|l| !l.trim().is_empty())
                .map(|l| l.trim().to_string())
                .collect()
        };

        let total = proxies.len();
        eprintln!(
            "[proxy_check] starting: {} proxies, {} threads, url={}",
            total, thread_count, check_url
        );

        // Guard: empty proxy list → refuse to start
        if total == 0 {
            eprintln!(
                "[proxy_check] proxy list '{}' resolved to 0 proxies — aborting start",
                proxy_list_path
            );
            job.state = JobState::Waiting;
            job.started = None;
            return None;
        }

        // ── Atomic stats shared with every spawned task ────────────────────
        let processed_ctr = Arc::new(AtomicUsize::new(0));
        let hits_ctr = Arc::new(AtomicUsize::new(0));
        let fails_ctr = Arc::new(AtomicUsize::new(0));
        let errors_ctr = Arc::new(AtomicUsize::new(0));
        let active_threads_ctr = Arc::new(AtomicUsize::new(0));
        let running_flag = Arc::new(AtomicBool::new(true));

        let start_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;

        // Store the handle so get_job_stats / stop_job / remove_job can access it
        self.proxy_handles.insert(
            id,
            ProxyCheckHandle {
                processed: processed_ctr.clone(),
                hits: hits_ctr.clone(),
                fails: fails_ctr.clone(),
                errors: errors_ctr.clone(),
                active_threads: active_threads_ctr.clone(),
                total,
                running: running_flag.clone(),
                start_ms,
            },
        );

        let (hits_tx, hits_rx) = tokio::sync::mpsc::channel::<HitResult>(4096);
        let semaphore = Arc::new(tokio::sync::Semaphore::new(thread_count));

        for proxy in proxies {
            let tx = hits_tx.clone();
            let url = check_url.clone();
            let sem = semaphore.clone();
            let running = running_flag.clone();
            let processed = processed_ctr.clone();
            let hits = hits_ctr.clone();
            let fails = fails_ctr.clone();
            let errors = errors_ctr.clone();
            let active = active_threads_ctr.clone();
            let check_type = proxy_check_type.clone();

            handle.spawn(async move {
                let _permit = sem.acquire().await.ok();

                if !running.load(Ordering::Relaxed) {
                    return;
                }

                let proxy_url = if crate::sidecar::xray_pool::supports_uri(&proxy) {
                    match crate::sidecar::xray_pool::resolve_proxy_uri(&proxy) {
                        Ok(local_url) => local_url,
                        Err(error) => {
                            errors.fetch_add(1, Ordering::Relaxed);
                            processed.fetch_add(1, Ordering::Relaxed);
                            let mut captures = std::collections::HashMap::new();
                            captures.insert("status".into(), "error".into());
                            captures.insert("stage".into(), "xray_start".into());
                            captures.insert("message".into(), format!("Encrypted proxy could not start: {error}"));
                            let _ = tx.send(HitResult { data_line: proxy, captures, proxy: None, ..Default::default() }).await;
                            return;
                        }
                    }
                } else if proxy.starts_with("http://") || proxy.starts_with("https://") || proxy.starts_with("socks5://") || proxy.starts_with("socks4://") {
                    proxy.clone()
                } else {
                    let scheme = match check_type.to_lowercase().as_str() {
                        "socks5" => "socks5",
                        "socks4" => "socks4",
                        "https" => "https",
                        _ => "http",
                    };
                    format!("{}://{}", scheme, proxy)
                };

                let client_result = reqwest::Client::builder()
                    .proxy(
                        reqwest::Proxy::all(&proxy_url)
                            .unwrap_or_else(|_| reqwest::Proxy::all("http://127.0.0.1:1").unwrap()),
                    )
                    .connect_timeout(std::time::Duration::from_secs(10))
                    .timeout(std::time::Duration::from_secs(15))
                    .build();

                match client_result {
                    Err(_) => {
                        errors.fetch_add(1, Ordering::Relaxed);
                        processed.fetch_add(1, Ordering::Relaxed);
                        let mut captures = std::collections::HashMap::new();
                        captures.insert("status".into(), "error".into());
                        let _ = tx
                            .send(HitResult {
                                data_line: proxy,
                                captures,
                                proxy: None,
                                ..Default::default()
                            })
                            .await;
                    }
                    Ok(client) => {
                        active.fetch_add(1, Ordering::Relaxed);
                        let req_start = std::time::Instant::now();
                        let result = client.get(&url).send().await;
                        let latency_ms = req_start.elapsed().as_millis();
                        active.fetch_sub(1, Ordering::Relaxed);
                        processed.fetch_add(1, Ordering::Relaxed);

                        let mut captures = std::collections::HashMap::new();
                        captures.insert("latency_ms".into(), latency_ms.to_string());

                        match result {
                            Ok(_) => {
                                hits.fetch_add(1, Ordering::Relaxed);
                                captures.insert("status".into(), "alive".into());
                            }
                            Err(_) => {
                                fails.fetch_add(1, Ordering::Relaxed);
                                captures.insert("status".into(), "dead".into());
                            }
                        }

                        let _ = tx
                            .send(HitResult {
                                data_line: proxy,
                                captures,
                                proxy: None,
                                ..Default::default()
                            })
                            .await;
                    }
                }
            });
        }

        Some(hits_rx)
    }
}
