//! Managed Xray Core local SOCKS5 adapters for VMess, VLESS, and Trojan URIs.
//!
//! The HTTP engines only understand HTTP/SOCKS proxy URLs. This module turns a
//! supported encrypted-proxy URI into an isolated Xray process with a loopback
//! SOCKS5 inbound and returns that local `socks5://` endpoint.

use std::{
    collections::HashMap,
    net::{TcpListener, TcpStream},
    path::PathBuf,
    process::{Child, Command, Stdio},
    sync::{Arc, Condvar, Mutex, OnceLock},
    time::Duration,
};

use base64::{engine::general_purpose, Engine as _};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use url::Url;

struct XrayEntry {
    local_url: String,
    child: Mutex<Child>,
    config_path: PathBuf,
}

impl XrayEntry {
    fn is_running(&self) -> Result<bool, String> {
        let mut child = self
            .child
            .lock()
            .map_err(|_| "Xray child lock poisoned".to_string())?;
        child
            .try_wait()
            .map(|status| status.is_none())
            .map_err(|error| error.to_string())
    }
}

impl Drop for XrayEntry {
    fn drop(&mut self) {
        if let Ok(child) = self.child.get_mut() {
            let _ = child.kill();
            let _ = child.wait();
        }
        let _ = std::fs::remove_file(&self.config_path);
    }
}

struct XraySlot {
    entry: OnceLock<Result<Arc<XrayEntry>, String>>,
}

impl XraySlot {
    fn new() -> Self {
        Self {
            entry: OnceLock::new(),
        }
    }
}

pub struct XrayPool {
    state: Mutex<PoolState>,
    resolvers_idle: Condvar,
}

struct PoolState {
    entries: HashMap<String, Arc<XraySlot>>,
    closing: bool,
    active_resolvers: usize,
}

struct ResolveGuard<'a> {
    pool: &'a XrayPool,
}

impl Drop for ResolveGuard<'_> {
    fn drop(&mut self) {
        if let Ok(mut state) = self.pool.state.lock() {
            state.active_resolvers = state.active_resolvers.saturating_sub(1);
            if state.active_resolvers == 0 {
                self.pool.resolvers_idle.notify_all();
            }
        }
    }
}

impl XrayPool {
    fn new() -> Self {
        Self {
            state: Mutex::new(PoolState {
                entries: HashMap::new(),
                closing: false,
                active_resolvers: 0,
            }),
            resolvers_idle: Condvar::new(),
        }
    }

    fn begin_resolve(&self) -> Result<ResolveGuard<'_>, String> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| "Xray pool lock poisoned".to_string())?;
        if state.closing {
            return Err("Xray pool is shutting down".to_string());
        }
        state.active_resolvers += 1;
        Ok(ResolveGuard { pool: self })
    }

    fn resolve(&self, uri: &str) -> Result<String, String> {
        self.resolve_with(uri, || start_xray(uri))
    }

    fn resolve_with<F>(&self, uri: &str, starter: F) -> Result<String, String>
    where
        F: Fn() -> Result<XrayEntry, String>,
    {
        let _resolve_guard = self.begin_resolve()?;
        loop {
            // The map lock is held only while selecting the per-URI slot. OnceLock
            // provides single-flight initialization for one URI while allowing
            // unrelated encrypted proxies to start in parallel.
            let slot = {
                let mut state = self
                    .state
                    .lock()
                    .map_err(|_| "Xray pool lock poisoned".to_string())?;
                state
                    .entries
                    .entry(uri.to_string())
                    .or_insert_with(|| Arc::new(XraySlot::new()))
                    .clone()
            };

            let result = slot.entry.get_or_init(|| starter().map(Arc::new)).clone();

            match result {
                Ok(entry) if entry.is_running()? => return Ok(entry.local_url.clone()),
                Ok(_) => {
                    self.remove_slot(uri, &slot)?;
                    // The cached process exited. Remove the stale slot and retry
                    // through a fresh single-flight initialization.
                }
                Err(error) => {
                    self.remove_slot(uri, &slot)?;
                    return Err(error);
                }
            }
        }
    }

    fn remove_slot(&self, uri: &str, expected: &Arc<XraySlot>) -> Result<(), String> {
        let removed = {
            let mut state = self
                .state
                .lock()
                .map_err(|_| "Xray pool lock poisoned".to_string())?;
            if state
                .entries
                .get(uri)
                .is_some_and(|current| Arc::ptr_eq(current, expected))
            {
                state.entries.remove(uri)
            } else {
                None
            }
        };
        drop(removed);
        Ok(())
    }

    fn shutdown(&self) -> usize {
        let entries = match self.state.lock() {
            Ok(mut state) => {
                state.closing = true;
                while state.active_resolvers > 0 {
                    state = match self.resolvers_idle.wait(state) {
                        Ok(state) => state,
                        Err(_) => return 0,
                    };
                }
                std::mem::take(&mut state.entries)
            }
            Err(_) => return 0,
        };
        let count = entries.len();
        drop(entries);
        count
    }
}

fn start_xray(uri: &str) -> Result<XrayEntry, String> {
    let spec = parse_uri(uri)?;
    let port = free_port()?;
    let local_url = format!("socks5://127.0.0.1:{port}");
    let xray = xray_path()?;
    let config_path = write_config(uri, &spec, port)?;
    let mut config_guard = TempConfigGuard::new(config_path.clone());
    let mut command = Command::new(xray);
    command
        .args(["run", "-c"])
        .arg(&config_path)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    // Xray is a console-subsystem executable on Windows. Stream redirection
    // alone does not stop Windows from allocating a console for each managed
    // URI; CREATE_NO_WINDOW keeps proxy checks and imports non-intrusive.
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
    let child = command
        .spawn()
        .map_err(|e| format!("Cannot start bundled Xray Core: {e}"))?;

    // Xray binds quickly. Verify the listener so a malformed config is not
    // returned as a working proxy endpoint.
    if !wait_for_port(port, Duration::from_secs(3)) {
        let mut child = child;
        let _ = child.kill();
        let _ = child.wait();
        return Err("Xray Core did not open its local SOCKS listener; check the proxy URI and bundled runtime".into());
    }

    config_guard.disarm();
    Ok(XrayEntry {
        local_url,
        child: Mutex::new(child),
        config_path,
    })
}

struct TempConfigGuard {
    path: Option<PathBuf>,
}

impl TempConfigGuard {
    fn new(path: PathBuf) -> Self {
        Self { path: Some(path) }
    }

    fn disarm(&mut self) {
        self.path = None;
    }
}

impl Drop for TempConfigGuard {
    fn drop(&mut self) {
        if let Some(path) = self.path.take() {
            let _ = std::fs::remove_file(path);
        }
    }
}

static POOL: OnceLock<XrayPool> = OnceLock::new();

fn pool() -> &'static XrayPool {
    POOL.get_or_init(XrayPool::new)
}

/// Returns true for the encrypted proxy URI schemes handled by Xray Core.
pub fn supports_uri(uri: &str) -> bool {
    let value = uri.trim().to_ascii_lowercase();
    value.starts_with("vmess://") || value.starts_with("vless://") || value.starts_with("trojan://")
}

/// Resolve a VMess, VLESS, or Trojan URI to a managed local SOCKS5 endpoint.
pub fn resolve_proxy_uri(uri: &str) -> Result<String, String> {
    pool().resolve(uri.trim())
}

/// Stop every managed Xray process and remove its generated configuration.
/// This must be called on both GUI and CLI shutdown because child processes are
/// not terminated when `std::process::Child` handles are merely dropped.
pub fn shutdown_all() -> usize {
    pool().shutdown()
}

#[derive(Debug, Clone)]
struct XraySpec {
    protocol: &'static str,
    host: String,
    port: u16,
    user: Value,
    stream: Value,
}

fn parse_uri(uri: &str) -> Result<XraySpec, String> {
    if uri.starts_with("vmess://") {
        return parse_vmess(uri);
    }
    let url = Url::parse(uri).map_err(|e| format!("Invalid proxy URI: {e}"))?;
    let host = url.host_str().ok_or("Proxy URI has no host")?.to_string();
    let port = url.port().ok_or("Proxy URI has no port")?;
    let query: HashMap<String, String> = url.query_pairs().into_owned().collect();
    let stream = stream_settings(&query);
    match url.scheme() {
        "vless" => {
            let id = url.username();
            if id.is_empty() {
                return Err("VLESS URI has no UUID user ID".into());
            }
            Ok(XraySpec {
                protocol: "vless",
                host,
                port,
                user: json!({"id": id, "encryption": query.get("encryption").cloned().unwrap_or_else(|| "none".into()), "flow": query.get("flow").cloned().unwrap_or_default()}),
                stream,
            })
        }
        "trojan" => {
            let password = url.username();
            if password.is_empty() {
                return Err("Trojan URI has no password".into());
            }
            Ok(XraySpec {
                protocol: "trojan",
                host,
                port,
                user: json!({"password": password}),
                stream,
            })
        }
        _ => Err("Unsupported Xray proxy scheme".into()),
    }
}

fn parse_vmess(uri: &str) -> Result<XraySpec, String> {
    let encoded = uri
        .trim_start_matches("vmess://")
        .split('#')
        .next()
        .unwrap_or_default();
    let decoded = general_purpose::STANDARD
        .decode(encoded)
        .or_else(|_| general_purpose::URL_SAFE_NO_PAD.decode(encoded))
        .map_err(|_| "VMess URI payload is not valid base64")?;
    let value: Value =
        serde_json::from_slice(&decoded).map_err(|e| format!("VMess URI JSON is invalid: {e}"))?;
    let field = |name: &str| value.get(name).and_then(Value::as_str).unwrap_or_default();
    let host = field("add").to_string();
    if host.is_empty() {
        return Err("VMess URI has no server address".into());
    }
    let port = field("port")
        .parse::<u16>()
        .map_err(|_| "VMess URI has an invalid port")?;
    let id = field("id");
    if id.is_empty() {
        return Err("VMess URI has no UUID user ID".into());
    }
    let mut query = HashMap::new();
    query.insert("type".to_string(), {
        let value = field("net");
        if value.is_empty() {
            "tcp".into()
        } else {
            value.into()
        }
    });
    query.insert("security".to_string(), field("tls").to_string());
    query.insert("sni".to_string(), field("sni").to_string());
    query.insert("host".to_string(), field("host").to_string());
    query.insert("path".to_string(), field("path").to_string());
    let security = {
        let value = field("scy");
        if value.is_empty() {
            "auto"
        } else {
            value
        }
    };
    Ok(XraySpec {
        protocol: "vmess",
        host,
        port,
        user: json!({
            "id": id,
            "alterId": field("aid").parse::<u32>().unwrap_or(0),
            "security": security,
        }),
        stream: stream_settings(&query),
    })
}

fn stream_settings(query: &HashMap<String, String>) -> Value {
    let network = query
        .get("type")
        .or_else(|| query.get("network"))
        .cloned()
        .unwrap_or_else(|| "tcp".into());
    let security = query.get("security").cloned().unwrap_or_default();
    let mut stream = serde_json::Map::new();
    stream.insert("network".into(), Value::String(network.clone()));
    if security == "tls" || security == "reality" {
        stream.insert("security".into(), Value::String(security.clone()));
        let mut tls = serde_json::Map::new();
        if let Some(sni) = query.get("sni").filter(|s| !s.is_empty()) {
            tls.insert("serverName".into(), Value::String(sni.clone()));
        }
        if let Some(fingerprint) = query.get("fp").filter(|s| !s.is_empty()) {
            tls.insert("fingerprint".into(), Value::String(fingerprint.clone()));
        }
        if let Some(alpn) = query.get("alpn").filter(|s| !s.is_empty()) {
            tls.insert(
                "alpn".into(),
                Value::Array(alpn.split(',').map(|value| json!(value.trim())).collect()),
            );
        }
        if query
            .get("allowInsecure")
            .is_some_and(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        {
            tls.insert("allowInsecure".into(), Value::Bool(true));
        }
        if security == "tls" {
            stream.insert("tlsSettings".into(), Value::Object(tls));
        } else {
            let mut reality = tls;
            if let Some(key) = query.get("pbk").filter(|s| !s.is_empty()) {
                reality.insert("publicKey".into(), Value::String(key.clone()));
            }
            if let Some(short_id) = query.get("sid").filter(|s| !s.is_empty()) {
                reality.insert("shortId".into(), Value::String(short_id.clone()));
            }
            if let Some(spider_x) = query.get("spx").filter(|s| !s.is_empty()) {
                reality.insert("spiderX".into(), Value::String(spider_x.clone()));
            }
            stream.insert("realitySettings".into(), Value::Object(reality));
        }
    }
    if network == "ws" {
        let mut ws = serde_json::Map::new();
        if let Some(path) = query.get("path").filter(|s| !s.is_empty()) {
            ws.insert("path".into(), Value::String(path.clone()));
        }
        if let Some(host) = query.get("host").filter(|s| !s.is_empty()) {
            ws.insert("headers".into(), json!({"Host": host}));
        }
        stream.insert("wsSettings".into(), Value::Object(ws));
    }
    if network == "grpc" {
        if let Some(service) = query
            .get("serviceName")
            .or_else(|| query.get("path"))
            .filter(|s| !s.is_empty())
        {
            stream.insert("grpcSettings".into(), json!({"serviceName": service}));
        }
    }
    Value::Object(stream)
}

fn write_config(uri: &str, spec: &XraySpec, local_port: u16) -> Result<PathBuf, String> {
    let outbound = json!({
        "protocol": spec.protocol,
        "settings": {"vnext": [{"address": spec.host, "port": spec.port, "users": [spec.user]}]},
        "streamSettings": spec.stream,
    });
    let outbound = if spec.protocol == "trojan" {
        json!({"protocol": "trojan", "settings": {"servers": [{"address": spec.host, "port": spec.port, "password": spec.user["password"]}]}, "streamSettings": spec.stream})
    } else {
        outbound
    };
    let config = json!({
        "log": {"loglevel": "warning"},
        "inbounds": [{"listen": "127.0.0.1", "port": local_port, "protocol": "socks", "settings": {"auth": "noauth", "udp": false}}],
        "outbounds": [outbound],
    });
    let digest = format!("{:x}", Sha256::digest(uri.as_bytes()));
    let dir = dirs::cache_dir()
        .unwrap_or_else(std::env::temp_dir)
        .join("Ironbullet")
        .join("xray");
    std::fs::create_dir_all(&dir)
        .map_err(|e| format!("Cannot create Xray config directory: {e}"))?;
    let path = dir.join(format!("{digest}.json"));
    std::fs::write(
        &path,
        serde_json::to_vec_pretty(&config).map_err(|e| e.to_string())?,
    )
    .map_err(|e| format!("Cannot write Xray config: {e}"))?;
    Ok(path)
}

fn xray_path() -> Result<PathBuf, String> {
    if let Ok(value) = std::env::var("IRONBULLET_XRAY_PATH") {
        let path = PathBuf::from(value);
        if path.is_file() {
            return Ok(path);
        }
    }
    let name = if cfg!(windows) { "xray.exe" } else { "xray" };
    let path = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(|dir| dir.join(name)))
        .ok_or("Cannot resolve Ironbullet executable directory for bundled Xray Core")?;
    if path.is_file() {
        Ok(path)
    } else {
        Err(format!(
            "Bundled Xray Core is missing at {}",
            path.display()
        ))
    }
}

fn free_port() -> Result<u16, String> {
    let listener = TcpListener::bind("127.0.0.1:0")
        .map_err(|e| format!("Cannot allocate local SOCKS port: {e}"))?;
    listener
        .local_addr()
        .map(|address| address.port())
        .map_err(|e| e.to_string())
}

fn wait_for_port(port: u16, timeout: Duration) -> bool {
    let start = std::time::Instant::now();
    while start.elapsed() < timeout {
        if TcpStream::connect_timeout(
            &format!("127.0.0.1:{port}").parse().unwrap(),
            Duration::from_millis(100),
        )
        .is_ok()
        {
            return true;
        }
        std::thread::sleep(Duration::from_millis(25));
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        mpsc, Barrier,
    };

    fn sleeper_entry(name: &str) -> XrayEntry {
        #[cfg(windows)]
        let child = {
            use std::os::windows::process::CommandExt;
            let mut command = Command::new("powershell.exe");
            command
                .args(["-NoProfile", "-Command", "Start-Sleep -Seconds 30"])
                .creation_flags(0x0800_0000);
            command.spawn().expect("spawn Windows sleeper")
        };
        #[cfg(not(windows))]
        let child = Command::new("sh")
            .args(["-c", "sleep 30"])
            .spawn()
            .expect("spawn Unix sleeper");

        let config_path = std::env::temp_dir().join(format!(
            "ironbullet-xray-pool-test-{name}-{}.json",
            uuid::Uuid::new_v4()
        ));
        std::fs::write(&config_path, b"{}").expect("write fake config");
        XrayEntry {
            local_url: format!("socks5://127.0.0.1:{name}"),
            child: Mutex::new(child),
            config_path,
        }
    }

    fn process_exists(pid: u32) -> bool {
        #[cfg(windows)]
        {
            Command::new("powershell.exe")
                .args([
                    "-NoProfile",
                    "-Command",
                    &format!(
                        "if (Get-Process -Id {pid} -ErrorAction SilentlyContinue) {{ exit 0 }} else {{ exit 1 }}"
                    ),
                ])
                .status()
                .is_ok_and(|status| status.success())
        }
        #[cfg(not(windows))]
        {
            std::path::Path::new(&format!("/proc/{pid}")).exists()
        }
    }

    #[test]
    fn same_uri_initialization_is_single_flight() {
        const CALLERS: usize = 24;
        let pool = Arc::new(XrayPool::new());
        let starts = Arc::new(AtomicUsize::new(0));
        let barrier = Arc::new(Barrier::new(CALLERS));
        let mut callers = Vec::new();

        for _ in 0..CALLERS {
            let pool = pool.clone();
            let starts = starts.clone();
            let barrier = barrier.clone();
            callers.push(std::thread::spawn(move || {
                barrier.wait();
                pool.resolve_with("vless://same", || {
                    starts.fetch_add(1, Ordering::SeqCst);
                    std::thread::sleep(Duration::from_millis(75));
                    Ok(sleeper_entry("41001"))
                })
            }));
        }

        let urls: Vec<String> = callers
            .into_iter()
            .map(|caller| {
                caller
                    .join()
                    .expect("caller panicked")
                    .expect("resolve failed")
            })
            .collect();
        assert_eq!(starts.load(Ordering::SeqCst), 1);
        assert!(urls.iter().all(|url| url == &urls[0]));
        assert_eq!(pool.shutdown(), 1);
    }

    #[test]
    fn distinct_uris_initialize_in_parallel() {
        let pool = Arc::new(XrayPool::new());
        let barrier = Arc::new(Barrier::new(2));
        let release = Arc::new(Barrier::new(3));
        let (entered_tx, entered_rx) = mpsc::sync_channel(2);
        let mut callers = Vec::new();

        for (uri, port) in [("vless://one", "41002"), ("vless://two", "41003")] {
            let pool = pool.clone();
            let barrier = barrier.clone();
            let release = release.clone();
            let entered_tx = entered_tx.clone();
            callers.push(std::thread::spawn(move || {
                barrier.wait();
                pool.resolve_with(uri, || {
                    entered_tx.send(uri).expect("report initializer entry");
                    release.wait();
                    Ok(sleeper_entry(port))
                })
            }));
        }

        let mut entered = vec![
            entered_rx.recv().expect("first initializer did not enter"),
            entered_rx.recv().expect("second initializer did not enter"),
        ];
        entered.sort_unstable();
        assert_eq!(entered, vec!["vless://one", "vless://two"]);
        release.wait();

        for caller in callers {
            caller
                .join()
                .expect("caller panicked")
                .expect("resolve failed");
        }
        assert_eq!(pool.shutdown(), 2);
    }

    #[test]
    fn shutdown_waits_for_in_flight_initializer_and_rejects_new_resolves() {
        let pool = Arc::new(XrayPool::new());
        let (entered_tx, entered_rx) = mpsc::sync_channel(1);
        let (release_tx, release_rx) = mpsc::sync_channel(1);
        let child_pid = Arc::new(AtomicUsize::new(0));
        let config_path = Arc::new(Mutex::new(PathBuf::new()));

        let resolver_pool = pool.clone();
        let resolver_pid = child_pid.clone();
        let resolver_config = config_path.clone();
        let resolver = std::thread::spawn(move || {
            resolver_pool.resolve_with("vless://slow-start", || {
                entered_tx.send(()).expect("report initializer entry");
                release_rx.recv().expect("release initializer");
                let entry = sleeper_entry("41005");
                resolver_pid.store(
                    entry.child.lock().expect("child lock").id() as usize,
                    Ordering::SeqCst,
                );
                *resolver_config.lock().expect("config path lock") = entry.config_path.clone();
                Ok(entry)
            })
        });

        entered_rx.recv().expect("initializer did not enter");
        let shutdown_pool = pool.clone();
        let shutdown = std::thread::spawn(move || shutdown_pool.shutdown());

        loop {
            if pool.state.lock().expect("pool state lock").closing {
                break;
            }
            std::thread::yield_now();
        }
        let rejected = pool.resolve_with("vless://late", || panic!("late starter ran"));
        assert_eq!(rejected.unwrap_err(), "Xray pool is shutting down");

        release_tx.send(()).expect("release initializer");
        resolver
            .join()
            .expect("resolver panicked")
            .expect("in-flight resolver failed");
        assert_eq!(shutdown.join().expect("shutdown panicked"), 1);

        let pid = child_pid.load(Ordering::SeqCst) as u32;
        let config = config_path.lock().expect("config path lock").clone();
        assert!(!process_exists(pid), "initializer child survived shutdown");
        assert!(!config.exists(), "initializer config survived shutdown");
    }

    #[test]
    fn shutdown_kills_child_and_removes_config() {
        let pool = XrayPool::new();
        let child_pid = AtomicUsize::new(0);
        let config_path = Mutex::new(PathBuf::new());

        pool.resolve_with("trojan://shutdown", || {
            let entry = sleeper_entry("41004");
            child_pid.store(
                entry.child.lock().expect("child lock").id() as usize,
                Ordering::SeqCst,
            );
            *config_path.lock().expect("config path lock") = entry.config_path.clone();
            Ok(entry)
        })
        .expect("resolve failed");

        let child_pid = child_pid.load(Ordering::SeqCst) as u32;
        let config_path = config_path.lock().expect("config path lock").clone();
        assert!(process_exists(child_pid));
        assert!(config_path.exists());
        assert_eq!(pool.shutdown(), 1);
        assert!(
            !process_exists(child_pid),
            "managed child survived shutdown"
        );
        assert!(!config_path.exists(), "generated config survived shutdown");
    }

    #[test]
    fn parses_vless_and_builds_tls_stream() {
        let spec = parse_uri("vless://123e4567-e89b-12d3-a456-426614174000@example.test:443?encryption=none&security=tls&sni=cdn.example.test&type=ws&path=%2Fsocket").unwrap();
        assert_eq!(spec.protocol, "vless");
        assert_eq!(spec.host, "example.test");
        assert_eq!(spec.port, 443);
        assert_eq!(spec.stream["network"], "ws");
        assert_eq!(spec.stream["security"], "tls");
    }

    #[test]
    fn parses_vless_reality_stream() {
        let spec = parse_uri("vless://123e4567-e89b-12d3-a456-426614174000@example.test:443?security=reality&sni=www.example.test&pbk=test-public-key&sid=abcd&fp=chrome&type=tcp").unwrap();
        assert_eq!(spec.stream["security"], "reality");
        assert_eq!(
            spec.stream["realitySettings"]["publicKey"],
            "test-public-key"
        );
        assert_eq!(spec.stream["realitySettings"]["shortId"], "abcd");
        assert_eq!(spec.stream["realitySettings"]["fingerprint"], "chrome");
    }

    #[test]
    fn parses_vless_reality_vision_uri() {
        let spec = parse_uri("vless://123e4567-e89b-12d3-a456-426614174000@198.51.100.42:443?security=reality&encryption=none&pbk=public-key&fp=firefox&type=tcp&flow=xtls-rprx-vision&sni=cdn.example.test&sid=0123456789abcdef").unwrap();
        assert_eq!(spec.protocol, "vless");
        assert_eq!(spec.user["flow"], "xtls-rprx-vision");
        assert_eq!(spec.stream["security"], "reality");
        assert_eq!(
            spec.stream["realitySettings"]["serverName"],
            "cdn.example.test"
        );
        assert_eq!(spec.stream["realitySettings"]["fingerprint"], "firefox");
    }

    #[test]
    fn parses_trojan_uri() {
        let spec =
            parse_uri("trojan://secret@example.test:443?security=tls&sni=example.test&type=tcp")
                .unwrap();
        assert_eq!(spec.protocol, "trojan");
        assert_eq!(spec.user["password"], "secret");
    }

    #[test]
    fn parses_vmess_base64_json() {
        let payload = general_purpose::STANDARD.encode(r#"{"v":"2","ps":"demo","add":"vmess.example.test","port":"443","id":"123e4567-e89b-12d3-a456-426614174000","aid":"0","scy":"auto","net":"ws","host":"cdn.example.test","path":"/socket","tls":"tls","sni":"cdn.example.test"}"#);
        let spec = parse_uri(&format!("vmess://{payload}")).unwrap();
        assert_eq!(spec.protocol, "vmess");
        assert_eq!(spec.host, "vmess.example.test");
        assert_eq!(spec.stream["network"], "ws");
    }
}
