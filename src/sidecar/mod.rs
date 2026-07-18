pub mod native;
pub mod protocol;
pub mod session;
pub mod shadowsocks_pool;
#[cfg(feature = "wreq-tls")]
pub mod wreq_client;
pub mod xray_pool;

use dashmap::DashMap;
use dunce;
use std::process::Stdio;
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::{mpsc, oneshot};

use protocol::{SidecarRequest, SidecarResponse};

type ReqTx = mpsc::Sender<(SidecarRequest, oneshot::Sender<SidecarResponse>)>;
/// Lock-free concurrent map for in-flight request correlations.
/// DashMap shards the map internally so concurrent inserts/removes don't
/// serialize behind a single mutex — critical at 100+ worker threads.
type PendingMap = Arc<DashMap<String, oneshot::Sender<SidecarResponse>>>;

pub struct SidecarManager {
    process: Option<Child>,
    pending: PendingMap,
    writer_tx: Option<mpsc::Sender<String>>,
    req_tx: Option<ReqTx>,
    alive: Arc<std::sync::atomic::AtomicBool>,
    /// Broadcast channel for PROXY_EVENT lines emitted by the sidecar's MITM proxy.
    pub proxy_event_tx: Option<tokio::sync::broadcast::Sender<serde_json::Value>>,
}

impl SidecarManager {
    pub fn new() -> Self {
        Self {
            process: None,
            pending: Arc::new(DashMap::new()),
            writer_tx: None,
            req_tx: None,
            alive: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            proxy_event_tx: None,
        }
    }

    /// Start the sidecar process. Returns a cloneable sender for sending requests.
    pub async fn start(&mut self, sidecar_path: &str) -> crate::error::Result<ReqTx> {
        // Resolve sidecar path relative to the exe directory if the raw path doesn't exist.
        // Handles the common case where sidecar_path is just "reqflow-sidecar.exe" but
        // the process CWD differs from the exe directory (common on Windows).
        let sidecar_name = if cfg!(target_os = "windows") {
            "reqflow-sidecar.exe"
        } else {
            "reqflow-sidecar"
        };
        let exe_dir = std::env::current_exe()
            .ok()
            .and_then(|e| e.parent().map(|p| p.to_path_buf()));

        let path = {
            let p = std::path::PathBuf::from(sidecar_path);
            if p.exists() {
                // Strip \\?\ UNC prefix — CreateProcess on Windows rejects extended-length
                // paths (os error 123). Use dunce::canonicalize (resolves symlinks + strips
                // \\?\) when possible, fall back to dunce::simplified (strips \\?\ only).
                dunce::canonicalize(&p).unwrap_or_else(|_| dunce::simplified(&p).to_path_buf())
            } else if let Some(ref dir) = exe_dir {
                // Try the filename part of sidecar_path next to the exe
                let by_name = dir.join(
                    std::path::Path::new(sidecar_path)
                        .file_name()
                        .unwrap_or_else(|| std::ffi::OsStr::new(sidecar_name)),
                );
                if by_name.exists() {
                    dunce::simplified(&by_name).to_path_buf()
                } else {
                    // Try canonical sidecar name next to exe
                    let canonical = dir.join(sidecar_name);
                    if canonical.exists() {
                        dunce::simplified(&canonical).to_path_buf()
                    } else {
                        p
                    }
                }
            } else {
                p
            }
        };

        if !path.exists() {
            return Err(crate::error::AppError::Sidecar(format!(
                "Sidecar not found at '{}'.\n\
                 Make sure {} is in the same folder as the IronBullet executable.\n\
                 Executable directory: {}",
                path.display(),
                sidecar_name,
                exe_dir
                    .map(|d| d.display().to_string())
                    .unwrap_or_else(|| "<unknown>".into())
            )));
        }

        // Set the working directory to the sidecar's own folder so that any
        // relative paths the sidecar uses resolve correctly.
        // Avoid Path::new(".") as current_dir fallback — on Windows, "." can trigger
        // os error 123 (invalid path syntax) if the process CWD is a UNC or network path.
        // Prefer the exe directory as a safe absolute fallback.
        let sidecar_dir_owned;
        let sidecar_dir =
            if let Some(parent) = path.parent().filter(|p| p != &std::path::Path::new("")) {
                parent
            } else {
                sidecar_dir_owned = std::env::current_exe()
                    .ok()
                    .and_then(|e| e.parent().map(|p| p.to_path_buf()))
                    .unwrap_or_else(|| std::path::PathBuf::from("."));
                sidecar_dir_owned.as_path()
            };

        let mut cmd = Command::new(&path);
        cmd.current_dir(sidecar_dir)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .kill_on_drop(true);

        #[cfg(windows)]
        {
            cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW
        }

        let mut child = cmd.spawn().map_err(|e| {
            crate::error::AppError::Sidecar(format!(
                "Failed to spawn sidecar '{}': {}\n\
                 (file exists={}, is_executable={})",
                sidecar_path,
                e,
                path.exists(),
                path.metadata()
                    .map(|_m| {
                        #[cfg(unix)]
                        {
                            use std::os::unix::fs::PermissionsExt;
                            _m.permissions().mode() & 0o111 != 0
                        }
                        #[cfg(not(unix))]
                        {
                            true
                        }
                    })
                    .unwrap_or(false)
            ))
        })?;

        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| crate::error::AppError::Sidecar("No stdin".into()))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| crate::error::AppError::Sidecar("No stdout".into()))?;

        // Writer channel — micro-batching: drain all pending messages before flushing.
        // Previously we flushed after every individual message, which at 100+ threads
        // means 100+ separate flush() syscalls per wave.  Draining the channel first
        // and flushing once per batch cuts syscall overhead significantly.
        let (writer_tx, mut writer_rx) = mpsc::channel::<String>(4096);
        let mut stdin = stdin;
        tokio::spawn(async move {
            while let Some(line) = writer_rx.recv().await {
                if stdin.write_all(line.as_bytes()).await.is_err() {
                    break;
                }
                if stdin.write_all(b"\n").await.is_err() {
                    break;
                }
                // Drain any additional pending messages before flushing once
                loop {
                    match writer_rx.try_recv() {
                        Ok(next) => {
                            if stdin.write_all(next.as_bytes()).await.is_err() {
                                return;
                            }
                            if stdin.write_all(b"\n").await.is_err() {
                                return;
                            }
                        }
                        Err(_) => break, // Channel empty — flush what we have
                    }
                }
                let _ = stdin.flush().await; // One flush per batch, not per message
            }
        });

        self.alive.store(true, std::sync::atomic::Ordering::SeqCst);

        let (proxy_tx, _) = tokio::sync::broadcast::channel::<serde_json::Value>(1024);
        self.proxy_event_tx = Some(proxy_tx.clone());

        let pending = self.pending.clone();
        let alive_flag = self.alive.clone();
        tokio::spawn(async move {
            let reader = BufReader::new(stdout);
            let mut lines = reader.lines();
            while let Ok(Some(line)) = lines.next_line().await {
                if let Some(json_str) = line.strip_prefix("PROXY_EVENT:") {
                    // Route proxy events to broadcast channel
                    if let Ok(val) = serde_json::from_str::<serde_json::Value>(json_str) {
                        let _ = proxy_tx.send(val);
                    }
                } else if let Ok(resp) = serde_json::from_str::<SidecarResponse>(&line) {
                    if let Some((_, tx)) = pending.remove(&resp.id) {
                        let _ = tx.send(resp);
                    }
                }
            }
            alive_flag.store(false, std::sync::atomic::Ordering::SeqCst);
        });

        // Request multiplexer channel — register correlation then enqueue write.
        // DashMap insert is non-blocking so this loop is never delayed by another thread's lookup.
        let (req_tx, mut req_rx) =
            mpsc::channel::<(SidecarRequest, oneshot::Sender<SidecarResponse>)>(4096);
        let writer_tx2 = writer_tx.clone();
        let pending2 = self.pending.clone();
        tokio::spawn(async move {
            while let Some((req, resp_tx)) = req_rx.recv().await {
                let id = req.id.clone();
                pending2.insert(id, resp_tx); // DashMap — no await, no mutex
                if let Ok(json) = serde_json::to_string(&req) {
                    let _ = writer_tx2.send(json).await;
                }
            }
        });

        self.process = Some(child);
        self.writer_tx = Some(writer_tx);
        self.req_tx = Some(req_tx.clone());
        Ok(req_tx)
    }

    /// Return a sender for an already-running sidecar, or None if not running.
    pub fn get_req_tx(&self) -> Option<ReqTx> {
        self.req_tx.clone()
    }

    /// Get or start: reuse existing sender if the sidecar is running, otherwise start fresh.
    pub async fn get_or_start(&mut self, sidecar_path: &str) -> crate::error::Result<ReqTx> {
        if self.is_running() {
            if let Some(tx) = self.get_req_tx() {
                return Ok(tx);
            }
        }
        // Not running or no tx — start fresh
        self.stop().await;
        self.start(sidecar_path).await
    }

    pub async fn stop(&mut self) {
        if let Some(mut child) = self.process.take() {
            let _ = child.kill().await;
        }
        self.writer_tx = None;
        self.req_tx = None;
        self.proxy_event_tx = None;
        self.alive.store(false, std::sync::atomic::Ordering::SeqCst);
    }

    pub fn is_running(&self) -> bool {
        self.process.is_some() && self.alive.load(std::sync::atomic::Ordering::Relaxed)
    }
}
