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
    sync::{Mutex, OnceLock},
    time::Duration,
};

use base64::{engine::general_purpose, Engine as _};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use url::Url;

struct XrayEntry {
    local_url: String,
    child: Child,
}

pub struct XrayPool {
    entries: Mutex<HashMap<String, XrayEntry>>,
}

impl XrayPool {
    fn new() -> Self {
        Self {
            entries: Mutex::new(HashMap::new()),
        }
    }

    fn resolve(&self, uri: &str) -> Result<String, String> {
        let cached = {
            let mut entries = self.entries.lock().map_err(|_| "Xray pool lock poisoned")?;
            let status = if let Some(entry) = entries.get_mut(uri) {
                match entry.child.try_wait().map_err(|e| e.to_string())? {
                    None => Some(entry.local_url.clone()),
                    Some(_) => None,
                }
            } else {
                None
            };
            if status.is_none() {
                entries.remove(uri);
            }
            status
        };
        if let Some(url) = cached {
            return Ok(url);
        }

        let spec = parse_uri(uri)?;
        let port = free_port()?;
        let local_url = format!("socks5://127.0.0.1:{port}");
        let config_path = write_config(uri, &spec, port)?;
        let xray = xray_path()?;
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
            return Err("Xray Core did not open its local SOCKS listener; check the proxy URI and bundled runtime".into());
        }

        let mut entries = self.entries.lock().map_err(|_| "Xray pool lock poisoned")?;
        entries.insert(
            uri.to_string(),
            XrayEntry {
                local_url: local_url.clone(),
                child,
            },
        );
        Ok(local_url)
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
