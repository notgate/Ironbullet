use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::{mpsc, oneshot, Mutex};
use uuid::Uuid;

use super::data_pool::DataPool;
use super::output::OutputWriter;
use super::proxy_pool::ProxyPool;
use super::{HitResult, ResultEntry, RunnerStatsInner, RESULT_FEED_CAP};
use crate::pipeline::engine::ExecutionContext;
use crate::pipeline::{BotStatus, Pipeline, ProxyMode};
use crate::sidecar::protocol::{SidecarRequest, SidecarResponse};

/// Per-worker execution state cloned from a `RunnerOrchestrator` at spawn time.
pub(crate) struct WorkerRuntime {
    pub(crate) pipeline: Pipeline,
    pub(crate) proxy_mode: ProxyMode,
    pub(crate) max_retries: u32,
    pub(crate) data_pool: Arc<DataPool>,
    pub(crate) proxy_pool: Arc<ProxyPool>,
    pub(crate) sidecar_tx: mpsc::Sender<(SidecarRequest, oneshot::Sender<SidecarResponse>)>,
    pub(crate) running: Arc<AtomicBool>,
    pub(crate) paused: Arc<AtomicBool>,
    pub(crate) stats: Arc<RunnerStatsInner>,
    pub(crate) hits_tx: mpsc::Sender<HitResult>,
    pub(crate) output_writer: Option<Arc<OutputWriter>>,
    pub(crate) plugin_manager: Option<Arc<crate::plugin::manager::PluginManager>>,
    pub(crate) chrome_executable_path: Option<std::path::PathBuf>,
    pub(crate) result_feed: Arc<Mutex<VecDeque<ResultEntry>>>,
    pub(crate) custom_input_values: std::collections::HashMap<String, String>,
}

pub(crate) async fn run_worker(runtime: WorkerRuntime) {
    let WorkerRuntime {
        pipeline,
        proxy_mode,
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
        custom_input_values,
    } = runtime;
    stats.active_threads.fetch_add(1, Ordering::Relaxed);

    // Session IDs are generated PER CREDENTIAL so the azuretls cookie jar resets
    // between checks. A shared worker session accumulated cookies across credentials,
    // which caused false errors when one blocked/captcha'd session tainted all others.

    let sticky_proxy: Option<String> = if matches!(proxy_mode, ProxyMode::Sticky) {
        proxy_pool.next_proxy()
    } else {
        None
    };

    while running.load(Ordering::Relaxed) {
        while paused.load(Ordering::Relaxed) && running.load(Ordering::Relaxed) {
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        }
        if !running.load(Ordering::Relaxed) {
            break;
        }

        let (data_line, retry_count) = match data_pool.next_line() {
            Some(entry) => entry,
            None => break,
        };

        let proxy = match &sticky_proxy {
            Some(p) => Some(p.clone()),
            None => proxy_pool.next_proxy(),
        };

        // Fresh session per credential — cookie jars are isolated so one blocked
        // account cannot taint the azuretls state seen by subsequent credentials.
        let session_id = Uuid::new_v4().to_string();
        {
            let new_sess = SidecarRequest {
                id: Uuid::new_v4().to_string(),
                action: "new_session".into(),
                session: session_id.clone(),
                browser: Some(pipeline.browser_settings.browser.clone()),
                ja3: pipeline.browser_settings.ja3.clone(),
                http2fp: pipeline.browser_settings.http2_fingerprint.clone(),
                proxy: proxy.clone(),
                follow_redirects: Some(true),
                max_redirects: Some(8),
                ..Default::default()
            };
            let (tx, _) = oneshot::channel();
            let _ = sidecar_tx.send((new_sess, tx)).await;
        }

        let mut ctx = ExecutionContext::new(session_id.clone());
        ctx.proxy = proxy.clone();
        ctx.plugin_manager = plugin_manager.clone();
        ctx.chrome_executable_path = chrome_executable_path.clone();

        // Inject custom user input values into globals namespace (issue #62).
        // Accessible in configs via <globals.KEY> or <KEY>.
        for (k, v) in &custom_input_values {
            ctx.variables.globals.insert(k.clone(), v.clone());
        }

        let parts: Vec<&str> = data_line.split(pipeline.data_settings.separator).collect();
        for (i, slice_name) in pipeline.data_settings.slices.iter().enumerate() {
            if let Some(part) = parts.get(i) {
                ctx.variables.set_input(slice_name, part.to_string());
            }
        }

        let result = ctx.execute_blocks(&pipeline.blocks, &sidecar_tx).await;

        // Release the azuretls session so its cookie jar and transport state
        // don't persist to the next credential checked by this worker.
        {
            let close_sess = SidecarRequest {
                id: Uuid::new_v4().to_string(),
                action: "close_session".into(),
                session: session_id.clone(),
                ..Default::default()
            };
            let (tx, _) = oneshot::channel();
            let _ = sidecar_tx.send((close_sess, tx)).await;
        }

        // processed = unique lines attempted (no retries). Retries are tracked separately.
        // This keeps processed consistent with data_pool.consumed() for the UI Done counter.
        if retry_count == 0 {
            stats.processed.fetch_add(1, Ordering::Relaxed);
            stats.consumed.fetch_add(1, Ordering::Relaxed);
        }

        let ts_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;

        // Extract response context for {status}, {response}, {headers} output format variables.
        // RESPONSECODE, LASTRESPONSE, LASTHEADERS are set by the HTTP block engine after every request.
        // HTTP blocks store RESPONSECODE / LASTRESPONSE / LASTHEADERS via set_data(),
        // so they live in the data namespace and must be accessed with the "data." prefix.
        let resp_status = ctx.variables.get("data.RESPONSECODE").unwrap_or_default();
        let resp_body = ctx.variables.get("data.LASTRESPONSE").unwrap_or_default();
        let resp_headers = ctx.variables.get("data.LASTHEADERS").unwrap_or_default();

        match ctx.status {
            BotStatus::Success => {
                stats.hits.fetch_add(1, Ordering::Relaxed);
                let captures = ctx.variables.captures();
                let hit = HitResult {
                    data_line: data_line.clone(),
                    captures: captures.clone(),
                    proxy: proxy.clone(),
                    response: resp_body.clone(),
                    headers: resp_headers.clone(),
                    status: resp_status.clone(),
                };
                if let Some(ref ow) = output_writer {
                    ow.write_hit(&hit, BotStatus::Success);
                }
                let _ = hits_tx.send(hit).await;
                if let Ok(mut feed) = result_feed.try_lock() {
                    if feed.len() >= RESULT_FEED_CAP {
                        feed.pop_front();
                    }
                    feed.push_back(ResultEntry {
                        data_line: data_line.clone(),
                        status: "SUCCESS".into(),
                        proxy: proxy.clone(),
                        captures,
                        error: None,
                        ts_ms,
                        block_results: ctx.block_results.clone(),
                    });
                }
            }
            BotStatus::Fail => {
                stats.fails.fetch_add(1, Ordering::Relaxed);
                if let Ok(mut feed) = result_feed.try_lock() {
                    if feed.len() >= RESULT_FEED_CAP {
                        feed.pop_front();
                    }
                    feed.push_back(ResultEntry {
                        data_line: data_line.clone(),
                        status: "FAIL".into(),
                        proxy: proxy.clone(),
                        captures: Default::default(),
                        error: None,
                        ts_ms,
                        block_results: ctx.block_results.clone(),
                    });
                }
            }
            BotStatus::Ban => {
                stats.bans.fetch_add(1, Ordering::Relaxed);
                // Ban the proxy that triggered this result so it is skipped for
                // future requests during the ban window.
                if let Some(ref p) = ctx.proxy {
                    proxy_pool.ban_proxy(p);
                }
                // Re-queue the credential for retry on a different proxy.
                // A BAN means the target blocked this proxy's IP, not that the
                // credential is invalid. Dropping the account here causes the
                // Done/Total counter to fall short — accounts are silently lost.
                if retry_count < max_retries {
                    stats.retries.fetch_add(1, Ordering::Relaxed);
                    data_pool.return_line(data_line.clone(), retry_count + 1);
                    if let Ok(mut feed) = result_feed.try_lock() {
                        if feed.len() >= RESULT_FEED_CAP {
                            feed.pop_front();
                        }
                        feed.push_back(ResultEntry {
                            data_line: data_line.clone(),
                            status: "BAN".into(),
                            proxy: proxy.clone(),
                            captures: Default::default(),
                            error: None,
                            ts_ms,
                            block_results: ctx.block_results.clone(),
                        });
                    }
                } else {
                    // Max retries exhausted — stash for final replay (issue #64).
                    data_pool.stash_error(data_line.clone());
                    stats.errors.fetch_add(1, Ordering::Relaxed);
                    if let Ok(mut feed) = result_feed.try_lock() {
                        if feed.len() >= RESULT_FEED_CAP {
                            feed.pop_front();
                        }
                        feed.push_back(ResultEntry {
                            data_line: data_line.clone(),
                            status: "BAN".into(),
                            proxy: proxy.clone(),
                            captures: Default::default(),
                            error: Some(
                                "Max retries exceeded (all proxies banned for this credential)"
                                    .into(),
                            ),
                            ts_ms,
                            block_results: ctx.block_results.clone(),
                        });
                    }
                }
            }
            BotStatus::Retry | BotStatus::Error => {
                // Treat transient network errors (timeout, connection refused, proxy errors)
                // as retryable — these are proxy-side failures, not credential failures.
                // The credential should be re-queued rather than discarded as an error.
                let is_network_error = result
                    .as_ref()
                    .err()
                    .map(|e| {
                        let msg = e.to_string().to_lowercase();
                        // Timeout errors
                        msg.contains("timed out") ||
                    msg.contains("request timed out") ||
                    msg.contains("context deadline exceeded") ||
                    msg.contains("i/o timeout") ||
                    // Connection-level errors
                    msg.contains("connection refused") ||
                    msg.contains("connection reset") ||
                    msg.contains("reset by peer") ||
                    msg.contains("connection closed") ||
                    msg.contains("connection aborted") ||
                    msg.contains("broken pipe") ||
                    msg.contains("use of closed network connection") ||
                    // EOF / empty response — very common from overloaded/rate-limiting proxies
                    msg.contains("eof") ||
                    msg.contains("empty response") ||
                    msg.contains("unexpected eof") ||
                    // Proxy-specific errors (from Go sidecar enriched messages)
                    msg.contains("proxyconnect") ||
                    msg.contains("proxy tunnel error") ||
                    msg.contains("proxy refused") ||
                    msg.contains("proxy connect timed out") ||
                    // Network reachability
                    msg.contains("host unreachable") ||
                    msg.contains("network is unreachable") ||
                    msg.contains("no route to host") ||
                    // Windows port exhaustion
                    msg.contains("wsaeaddrinuse") ||
                    msg.contains("only one usage of each socket address")
                    })
                    .unwrap_or(false);
                let is_retry_status = matches!(ctx.status, BotStatus::Retry) || is_network_error;
                if is_retry_status && retry_count < max_retries {
                    stats.retries.fetch_add(1, Ordering::Relaxed);
                    data_pool.return_line(data_line.clone(), retry_count + 1);
                    if let Ok(mut feed) = result_feed.try_lock() {
                        if feed.len() >= RESULT_FEED_CAP {
                            feed.pop_front();
                        }
                        feed.push_back(ResultEntry {
                            data_line: data_line.clone(),
                            status: "RETRY".into(),
                            proxy: proxy.clone(),
                            captures: Default::default(),
                            error: None,
                            ts_ms,
                            block_results: ctx.block_results.clone(),
                        });
                    }
                } else {
                    // Max retries exhausted — stash for final replay pass (issue #64).
                    // Transient network/proxy errors shouldn't permanently discard credentials.
                    data_pool.stash_error(data_line.clone());
                    stats.errors.fetch_add(1, Ordering::Relaxed);
                    let err_msg = result.as_ref().err().map(|e| e.to_string());
                    if let Some(ref ow) = output_writer {
                        let mut err_caps = std::collections::HashMap::new();
                        if let Some(ref msg) = err_msg {
                            err_caps.insert("_error".to_string(), msg.clone());
                        }
                        ow.write_hit(
                            &HitResult {
                                data_line: data_line.clone(),
                                captures: err_caps,
                                proxy: proxy.clone(),
                                status: resp_status.clone(),
                                ..Default::default()
                            },
                            BotStatus::Error,
                        );
                    }
                    if let Ok(mut feed) = result_feed.try_lock() {
                        if feed.len() >= RESULT_FEED_CAP {
                            feed.pop_front();
                        }
                        feed.push_back(ResultEntry {
                            data_line: data_line.clone(),
                            status: "ERROR".into(),
                            proxy: proxy.clone(),
                            captures: Default::default(),
                            error: err_msg,
                            ts_ms,
                            block_results: ctx.block_results.clone(),
                        });
                    }
                }
            }
            BotStatus::Custom => {
                // Custom = hit with a distinct category (e.g. FREE tier account).
                // Write to file and stdout just like Success, but tagged as Custom.
                stats.hits.fetch_add(1, Ordering::Relaxed);
                let captures = ctx.variables.captures();
                let hit = HitResult {
                    data_line: data_line.clone(),
                    captures: captures.clone(),
                    proxy: proxy.clone(),
                    response: resp_body.clone(),
                    headers: resp_headers.clone(),
                    status: resp_status.clone(),
                };
                if let Some(ref ow) = output_writer {
                    ow.write_hit(&hit, BotStatus::Custom);
                }
                let _ = hits_tx.send(hit).await;
                if let Ok(mut feed) = result_feed.try_lock() {
                    if feed.len() >= RESULT_FEED_CAP {
                        feed.pop_front();
                    }
                    feed.push_back(ResultEntry {
                        data_line: data_line.clone(),
                        status: "CUSTOM".into(),
                        proxy: proxy.clone(),
                        captures,
                        error: None,
                        ts_ms,
                        block_results: ctx.block_results.clone(),
                    });
                }
            }
            BotStatus::ToCheck => {
                // ToCheck = ambiguous result needing manual verification.
                // Saved to file + hits feed like Success/Custom, but tracked separately.
                stats.to_check.fetch_add(1, Ordering::Relaxed);
                let captures = ctx.variables.captures();
                let hit = HitResult {
                    data_line: data_line.clone(),
                    captures: captures.clone(),
                    proxy: proxy.clone(),
                    response: resp_body.clone(),
                    headers: resp_headers.clone(),
                    status: resp_status.clone(),
                };
                if let Some(ref ow) = output_writer {
                    ow.write_hit(&hit, BotStatus::ToCheck);
                }
                let _ = hits_tx.send(hit).await;
                if let Ok(mut feed) = result_feed.try_lock() {
                    if feed.len() >= RESULT_FEED_CAP {
                        feed.pop_front();
                    }
                    feed.push_back(ResultEntry {
                        data_line: data_line.clone(),
                        status: "TO_CHECK".into(),
                        proxy: proxy.clone(),
                        captures,
                        error: None,
                        ts_ms,
                        block_results: ctx.block_results.clone(),
                    });
                }
            }
            BotStatus::None => {
                // No KeyCheck block ran or no condition matched — treat as Fail.
                // This prevents 0/0/0 counters when a config has no outcome blocks.
                if result.is_err() {
                    stats.errors.fetch_add(1, Ordering::Relaxed);
                    let err_msg = result.err().map(|e| e.to_string());
                    if let Some(ref ow) = output_writer {
                        let mut err_caps = std::collections::HashMap::new();
                        if let Some(ref msg) = err_msg {
                            err_caps.insert("_error".to_string(), msg.clone());
                        }
                        ow.write_hit(
                            &HitResult {
                                data_line: data_line.clone(),
                                captures: err_caps,
                                proxy: proxy.clone(),
                                status: resp_status.clone(),
                                ..Default::default()
                            },
                            BotStatus::Error,
                        );
                    }
                    if let Ok(mut feed) = result_feed.try_lock() {
                        if feed.len() >= RESULT_FEED_CAP {
                            feed.pop_front();
                        }
                        feed.push_back(ResultEntry {
                            data_line: data_line.clone(),
                            status: "ERROR".into(),
                            proxy: proxy.clone(),
                            captures: Default::default(),
                            error: err_msg,
                            ts_ms,
                            block_results: ctx.block_results.clone(),
                        });
                    }
                } else {
                    // Result succeeded but no KeyCheck matched — count as Fail
                    stats.fails.fetch_add(1, Ordering::Relaxed);
                    if let Ok(mut feed) = result_feed.try_lock() {
                        if feed.len() >= RESULT_FEED_CAP {
                            feed.pop_front();
                        }
                        feed.push_back(ResultEntry {
                            data_line: data_line.clone(),
                            status: "FAIL".into(),
                            proxy: proxy.clone(),
                            captures: Default::default(),
                            error: None,
                            ts_ms,
                            block_results: ctx.block_results.clone(),
                        });
                    }
                }
            }
        }
    }

    // Each credential's session was already closed inside the loop above.
    stats.active_threads.fetch_sub(1, Ordering::Relaxed);
}
