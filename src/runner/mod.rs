pub mod data_pool;
pub mod job;
pub mod job_manager;
pub mod output;
pub mod proxy_pool;
pub mod worker;

use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;
use tokio::sync::{mpsc, oneshot, Mutex};

use crate::pipeline::{Pipeline, ProxyMode};
use crate::sidecar::protocol::{SidecarRequest, SidecarResponse};
use data_pool::DataPool;
use proxy_pool::ProxyPool;

/// Single check result pushed to the live feed ring buffer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResultEntry {
    /// The data line that was checked (e.g. "user:pass")
    pub data_line: String,
    /// Outcome: "SUCCESS" | "FAIL" | "BAN" | "RETRY" | "ERROR" | "NONE"
    pub status: String,
    /// Proxy used for this check, if any
    pub proxy: Option<String>,
    /// Captures from a successful check (non-empty only for SUCCESS)
    #[serde(default)]
    pub captures: std::collections::HashMap<String, String>,
    /// Error message when status == "ERROR"
    #[serde(default)]
    pub error: Option<String>,
    /// Wall-clock millis since epoch
    pub ts_ms: u64,
    /// Per-block execution trace (block label, type, timing, request/response, variables)
    #[serde(default)]
    pub block_results: Vec<crate::pipeline::engine::BlockResult>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunnerStats {
    pub total: usize,
    pub processed: usize,
    pub consumed: usize,
    pub hits: usize,
    pub fails: usize,
    pub bans: usize,
    pub retries: usize,
    pub errors: usize,
    pub to_check: usize,
    pub cpm: f64,
    pub active_threads: usize,
    pub elapsed_secs: f64,
    #[serde(default)]
    pub recent_results: Vec<ResultEntry>,
}

/// Capacity of the live result ring buffer.  100 entries ≈ last few seconds
/// of work at typical CPM rates; keeping it small avoids lock contention.
const RESULT_FEED_CAP: usize = 500;

/// Explicit dependencies required to start a pipeline runner.
///
/// A named setup prevents GUI, queued-job, and CLI callers from accidentally
/// swapping positional arguments as the runner evolves.
pub struct RunnerSetup {
    pub pipeline: Pipeline,
    pub proxy_mode: ProxyMode,
    pub data_pool: DataPool,
    pub proxy_pool: ProxyPool,
    pub sidecar_tx: mpsc::Sender<(SidecarRequest, oneshot::Sender<SidecarResponse>)>,
    pub thread_count: usize,
    pub hits_tx: mpsc::Sender<HitResult>,
    pub plugin_manager: Option<Arc<crate::plugin::manager::PluginManager>>,
    pub chrome_executable_path: Option<std::path::PathBuf>,
    pub custom_input_values: std::collections::HashMap<String, String>,
}

pub struct RunnerOrchestrator {
    pipeline: Pipeline,
    proxy_mode: ProxyMode,
    data_pool: Arc<DataPool>,
    proxy_pool: Arc<ProxyPool>,
    sidecar_tx: mpsc::Sender<(SidecarRequest, oneshot::Sender<SidecarResponse>)>,
    thread_count: usize,
    running: Arc<AtomicBool>,
    cancelled: Arc<AtomicBool>,
    paused: Arc<AtomicBool>,
    stats: Arc<RunnerStatsInner>,
    hits_tx: mpsc::Sender<HitResult>,
    output_writer: Option<Arc<output::OutputWriter>>,
    plugin_manager: Option<Arc<crate::plugin::manager::PluginManager>>,
    result_feed: Arc<Mutex<VecDeque<ResultEntry>>>,
    /// Optional Chrome/Chromium executable override for BrowserOpen blocks.
    chrome_executable_path: Option<std::path::PathBuf>,
    /// Custom user input values injected into the globals namespace (issue #62).
    custom_input_values: std::collections::HashMap<String, String>,
}

pub(crate) struct RunnerStatsInner {
    processed: AtomicUsize,
    consumed: AtomicUsize,
    hits: AtomicUsize,
    fails: AtomicUsize,
    bans: AtomicUsize,
    retries: AtomicUsize,
    errors: AtomicUsize,
    to_check: AtomicUsize,
    active_threads: AtomicUsize,
    start_time_ms: AtomicU64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct HitResult {
    pub data_line: String,
    pub captures: std::collections::HashMap<String, String>,
    pub proxy: Option<String>,
    /// Last HTTP response body (from RESPONSEBODY variable, if set)
    #[serde(default)]
    pub response: String,
    /// Last HTTP response headers (from last SOURCE.HEADERS variable, if set)
    #[serde(default)]
    pub headers: String,
    /// Last HTTP response status code (from RESPONSECODE variable)
    #[serde(default)]
    pub status: String,
}

impl RunnerOrchestrator {
    pub fn new(setup: RunnerSetup) -> Self {
        let RunnerSetup {
            pipeline,
            proxy_mode,
            data_pool,
            proxy_pool,
            sidecar_tx,
            thread_count,
            hits_tx,
            plugin_manager,
            chrome_executable_path,
            custom_input_values,
        } = setup;
        let ow = if pipeline.output_settings.save_to_file {
            Some(Arc::new(output::OutputWriter::new(
                &pipeline.output_settings,
                &pipeline.name,
            )))
        } else {
            None
        };
        Self {
            pipeline,
            proxy_mode,
            data_pool: Arc::new(data_pool),
            proxy_pool: Arc::new(proxy_pool),
            sidecar_tx,
            thread_count,
            running: Arc::new(AtomicBool::new(false)),
            cancelled: Arc::new(AtomicBool::new(false)),
            paused: Arc::new(AtomicBool::new(false)),
            stats: Arc::new(RunnerStatsInner {
                processed: AtomicUsize::new(0),
                consumed: AtomicUsize::new(0),
                hits: AtomicUsize::new(0),
                fails: AtomicUsize::new(0),
                bans: AtomicUsize::new(0),
                retries: AtomicUsize::new(0),
                errors: AtomicUsize::new(0),
                to_check: AtomicUsize::new(0),
                active_threads: AtomicUsize::new(0),
                start_time_ms: AtomicU64::new(0),
            }),
            hits_tx,
            output_writer: ow,
            plugin_manager,
            chrome_executable_path,
            result_feed: Arc::new(Mutex::new(VecDeque::with_capacity(RESULT_FEED_CAP))),
            custom_input_values,
        }
    }

    pub async fn start(&self) {
        if self.cancelled.load(Ordering::SeqCst) {
            return;
        }
        self.running.store(true, Ordering::SeqCst);
        if self.cancelled.load(Ordering::SeqCst) {
            self.running.store(false, Ordering::SeqCst);
            return;
        }
        self.paused.store(false, Ordering::SeqCst);

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        self.stats.start_time_ms.store(now, Ordering::SeqCst);

        let gradual = self.pipeline.runner_settings.start_threads_gradually;
        let delay_ms = self.pipeline.runner_settings.gradual_delay_ms as u64;
        let max_retries = self.pipeline.runner_settings.max_retries;
        let proxy_mode = self.proxy_mode.clone();

        // Cap total ramp-up time to 3 s so high thread counts don't stall.
        // e.g. 1000 threads with delay_ms=100 would otherwise take 100 s to start.
        let effective_delay_ms = if gradual && self.thread_count > 1 {
            let cap = (3000u64 / self.thread_count as u64).max(1);
            delay_ms.min(cap)
        } else {
            delay_ms
        };

        eprintln!(
            "[runner] starting {} threads (gradual={}, delay={}ms)",
            self.thread_count,
            gradual,
            if gradual { effective_delay_ms } else { 0 }
        );

        let mut handles = Vec::new();

        for i in 0..self.thread_count {
            if self.cancelled.load(Ordering::SeqCst) {
                break;
            }
            if gradual && i > 0 {
                tokio::time::sleep(std::time::Duration::from_millis(effective_delay_ms)).await;
                if self.cancelled.load(Ordering::SeqCst) {
                    break;
                }
            }

            let pipeline = self.pipeline.clone();
            let proxy_mode_w = proxy_mode.clone();
            let data_pool = self.data_pool.clone();
            let proxy_pool = self.proxy_pool.clone();
            let sidecar_tx = self.sidecar_tx.clone();
            let running = self.running.clone();
            let paused = self.paused.clone();
            let stats = self.stats.clone();
            let hits_tx = self.hits_tx.clone();
            let output_writer = self.output_writer.clone();
            let plugin_manager = self.plugin_manager.clone();
            let chrome_executable_path = self.chrome_executable_path.clone();
            let result_feed = self.result_feed.clone();
            let custom_inputs = self.custom_input_values.clone();

            let handle = tokio::spawn(async move {
                worker::run_worker(worker::WorkerRuntime {
                    pipeline,
                    proxy_mode: proxy_mode_w,
                    max_retries,
                    data_pool,
                    proxy_pool,
                    sidecar_tx,
                    running,
                    paused,
                    stats,
                    hits_tx,
                    output_writer,
                    plugin_manager,
                    chrome_executable_path,
                    result_feed,
                    custom_input_values: custom_inputs,
                })
                .await;
            });
            handles.push(handle);
        }

        for handle in handles {
            let _ = handle.await;
        }

        self.running.store(false, Ordering::SeqCst);
    }

    pub fn pause(&self) {
        self.paused.store(true, Ordering::SeqCst);
    }

    pub fn resume(&self) {
        self.paused.store(false, Ordering::SeqCst);
    }

    pub fn stop(&self) {
        self.cancelled.store(true, Ordering::SeqCst);
        self.running.store(false, Ordering::SeqCst);
    }

    /// Returns live stats WITHOUT the recent_results ring buffer.
    /// Use this for high-frequency broadcasts (jobs_list every 500ms) to avoid
    /// sending up to 500 ResultEntry structs (with full block_results) per IPC
    /// message — fix #61: WebView2 OOM caused by the cumulative JSON size.
    pub fn get_stats(&self) -> RunnerStats {
        self.get_stats_with_results(false)
    }

    /// Returns live stats WITH the recent_results ring buffer populated.
    /// Only called on demand by get_job_debug_log.
    pub fn get_stats_full(&self) -> RunnerStats {
        self.get_stats_with_results(true)
    }

    fn get_stats_with_results(&self, include_results: bool) -> RunnerStats {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        let start = self.stats.start_time_ms.load(Ordering::Relaxed);
        let elapsed_secs = if start > 0 {
            (now - start) as f64 / 1000.0
        } else {
            0.0
        };
        let processed = self.stats.processed.load(Ordering::Relaxed);
        let cpm = if elapsed_secs > 0.0 {
            processed as f64 / elapsed_secs * 60.0
        } else {
            0.0
        };

        let recent_results = if include_results {
            // Snapshot the live feed (non-blocking try_lock; return empty if contended)
            self.result_feed
                .try_lock()
                .map(|feed| feed.iter().cloned().collect::<Vec<_>>())
                .unwrap_or_default()
        } else {
            vec![]
        };

        RunnerStats {
            total: self.data_pool.total(),
            processed,
            consumed: self.data_pool.consumed(),
            hits: self.stats.hits.load(Ordering::Relaxed),
            fails: self.stats.fails.load(Ordering::Relaxed),
            bans: self.stats.bans.load(Ordering::Relaxed),
            retries: self.stats.retries.load(Ordering::Relaxed),
            errors: self.stats.errors.load(Ordering::Relaxed),
            to_check: self.stats.to_check.load(Ordering::Relaxed),
            cpm,
            active_threads: self.stats.active_threads.load(Ordering::Relaxed),
            elapsed_secs,
            recent_results,
        }
    }

    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::Relaxed)
    }
}

#[cfg(test)]
mod cancellation_tests {
    use super::*;

    #[tokio::test]
    async fn stop_before_start_prevents_any_input_from_being_consumed() {
        let (sidecar_tx, _sidecar_rx) = mpsc::channel(1);
        let (hits_tx, _hits_rx) = mpsc::channel(1);
        let runner = RunnerOrchestrator::new(RunnerSetup {
            pipeline: Pipeline::default(),
            proxy_mode: ProxyMode::None,
            data_pool: DataPool::new(vec!["must-not-run".into()]),
            proxy_pool: ProxyPool::empty(),
            sidecar_tx,
            thread_count: 4,
            hits_tx,
            plugin_manager: None,
            chrome_executable_path: None,
            custom_input_values: std::collections::HashMap::new(),
        });

        runner.stop();
        tokio::time::timeout(std::time::Duration::from_millis(100), runner.start())
            .await
            .expect("cancelled runner start should return immediately");

        let stats = runner.get_stats();
        assert_eq!(stats.consumed, 0);
        assert_eq!(stats.processed, 0);
        assert!(!runner.is_running());
    }
}
