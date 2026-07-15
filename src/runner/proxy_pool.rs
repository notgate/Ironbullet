use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::RwLock;
use std::time::Instant;

fn detect_proxy_type_from_port(port_str: &str, fallback: ProxyType) -> ProxyType {
    if let Ok(port) = port_str.parse::<u16>() {
        match port {
            1080 | 1081 | 9050 | 9150 | 1086 | 1087 | 1088 => ProxyType::Socks5,
            _ => fallback,
        }
    } else {
        fallback
    }
}

pub struct ProxyPool {
    proxies: Vec<ProxyEntry>,
    index: AtomicUsize,
    /// RwLock instead of Mutex: concurrent reads from next_proxy() don't block each other.
    /// Only ban_proxy() needs exclusive write access.
    bans: RwLock<HashMap<String, Instant>>,
    ban_duration_secs: u64,
}

#[derive(Debug, Clone)]
pub struct ProxyEntry {
    pub proxy_type: ProxyType,
    pub address: String,
}

#[derive(Debug, Clone, Copy)]
pub enum ProxyType {
    Http,
    Https,
    Socks4,
    Socks5,
    /// Shadowsocks — proxy address is the sslocal listen endpoint.
    /// sslocal exposes a SOCKS5 interface locally, so this is emitted
    /// as `socks5://` when building the proxy URL for HTTP clients.
    /// Users must run `sslocal` separately configured for their SS server.
    Shadowsocks,
}

impl ProxyPool {
    pub fn new(proxies: Vec<ProxyEntry>, ban_duration_secs: u64) -> Self {
        Self {
            proxies,
            index: AtomicUsize::new(0),
            bans: RwLock::new(HashMap::new()),
            ban_duration_secs,
        }
    }

    pub fn from_file(path: &str, ban_duration_secs: u64) -> std::io::Result<Self> {
        Self::from_file_with_type(path, ban_duration_secs, None)
    }

    pub fn from_file_with_type(
        path: &str,
        ban_duration_secs: u64,
        default_type: Option<ProxyType>,
    ) -> std::io::Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let proxies: Vec<ProxyEntry> = content
            .lines()
            .filter(|l| !l.trim().is_empty())
            .filter_map(|l| parse_proxy_line(l.trim(), default_type))
            .collect();
        Ok(Self::new(proxies, ban_duration_secs))
    }

    pub fn empty() -> Self {
        Self::new(Vec::new(), 300)
    }

    pub fn empty_with_ban(ban_duration_secs: u64) -> Self {
        Self::new(Vec::new(), ban_duration_secs)
    }

    /// Load proxies from a file into this pool, merging with any already loaded.
    /// `default_type_str` is the pipeline-level proxy type string (e.g. "http").
    pub fn load_from_file(
        &mut self,
        path: &str,
        default_type_str: Option<&str>,
    ) -> std::io::Result<()> {
        let default_type = default_type_str.and_then(|s| match s.to_lowercase().as_str() {
            "https" => Some(ProxyType::Https),
            "socks4" => Some(ProxyType::Socks4),
            "socks5" => Some(ProxyType::Socks5),
            "shadowsocks" | "ss" => Some(ProxyType::Shadowsocks),
            _ => Some(ProxyType::Http),
        });
        let content = std::fs::read_to_string(path)?;
        let new_proxies: Vec<ProxyEntry> = content
            .lines()
            .filter(|l| !l.trim().is_empty())
            .filter_map(|l| parse_proxy_line(l.trim(), default_type))
            .collect();
        self.proxies.extend(new_proxies);
        Ok(())
    }

    /// Load a single proxy line into this pool.
    /// `default_type` is the optional proxy type to use if the line doesn't specify one.
    pub fn load_from_string(&mut self, line: &str, default_type: Option<ProxyType>) {
        if let Some(entry) = parse_proxy_line(line.trim(), default_type) {
            self.proxies.push(entry);
        }
    }

    pub fn next_proxy(&self) -> Option<String> {
        if self.proxies.is_empty() {
            return None;
        }

        let now = Instant::now();

        // Acquire ban-list read lock — multiple threads can hold this simultaneously.
        // If poisoned, recover the data rather than returning None and stalling the caller.
        let bans = match self.bans.read() {
            Ok(g) => g,
            Err(e) => e.into_inner(),
        };

        // Try to find an unbanned proxy (scan at most proxies.len() candidates)
        let start = self.index.fetch_add(1, Ordering::Relaxed);
        for i in 0..self.proxies.len() {
            let idx = (start + i) % self.proxies.len();
            let proxy = &self.proxies[idx];
            let proxy_str = proxy.to_string();

            match bans.get(&proxy_str) {
                Some(ban_time)
                    if now.duration_since(*ban_time).as_secs() < self.ban_duration_secs =>
                {
                    // Still banned — skip
                }
                _ => return Some(self.proxies[idx].to_proxy_url()),
            }
        }

        // All proxies banned — return the round-robin slot anyway so work continues
        // (better to hit a banned proxy than to stall all threads with None)
        let idx = start % self.proxies.len();
        Some(self.proxies[idx].to_proxy_url())
    }

    pub fn ban_proxy(&self, proxy: &str) {
        // Ban the proxy using the URL form that next_proxy() also uses for the lookup.
        // Both job_manager and proxy_pool store Shadowsocks entries as
        // ProxyType::Socks5 with address = "127.0.0.1:<port>", so to_string() and
        // to_proxy_url() both emit "socks5://127.0.0.1:<port>". Using the canonical
        // ss:// form here caused a key mismatch — bans never applied to SS proxies.
        if let Ok(mut bans) = self.bans.write() {
            bans.insert(proxy.to_string(), Instant::now());
        }
    }

    pub fn total(&self) -> usize {
        self.proxies.len()
    }

    pub fn active(&self) -> usize {
        let bans = self.bans.read().ok();
        let now = Instant::now();
        let banned = bans
            .map(|b| {
                b.values()
                    .filter(|t| now.duration_since(**t).as_secs() < self.ban_duration_secs)
                    .count()
            })
            .unwrap_or(0);
        self.proxies.len().saturating_sub(banned)
    }
}

impl std::fmt::Display for ProxyEntry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let prefix = match self.proxy_type {
            ProxyType::Http => "http://",
            ProxyType::Https => "https://",
            ProxyType::Socks4 => "socks4://",
            ProxyType::Socks5 => "socks5://",
            // Shadowsocks: Display emits the raw ss:// form for logging/UI.
            // Use `to_proxy_url()` to get the actual SOCKS5 local tunnel URL.
            ProxyType::Shadowsocks => "ss://",
        };
        write!(f, "{}{}", prefix, self.address)
    }
}

impl ProxyEntry {
    /// Returns the proxy URL to pass to HTTP clients.
    ///
    /// For Shadowsocks entries, this spins up (or reuses) an embedded local
    /// SOCKS5 listener and returns `socks5://127.0.0.1:<port>`.
    /// For all other types, this is equivalent to `self.to_string()`.
    pub fn to_proxy_url(&self) -> String {
        match self.proxy_type {
            ProxyType::Shadowsocks => {
                let ss_url = format!("ss://{}", self.address);
                crate::sidecar::shadowsocks_pool::resolve_ss_proxy(&ss_url)
            }
            _ => self.to_string(),
        }
    }
}

fn parse_proxy_line(line: &str, default_type: Option<ProxyType>) -> Option<ProxyEntry> {
    let fallback = default_type.unwrap_or(ProxyType::Http);

    // VMess/VLESS/Trojan are not HTTP proxy URLs themselves. Resolve each URI
    // through the bundled Xray Core into a local SOCKS5 endpoint first.
    if crate::sidecar::xray_pool::supports_uri(line) {
        match crate::sidecar::xray_pool::resolve_proxy_uri(line) {
            Ok(local) => {
                return Some(ProxyEntry {
                    proxy_type: ProxyType::Socks5,
                    address: local
                        .strip_prefix("socks5://")
                        .unwrap_or(&local)
                        .to_string(),
                });
            }
            Err(error) => {
                eprintln!("[xray-pool] failed to resolve proxy URI: {error}");
                return None;
            }
        }
    }

    // ── URL-scheme prefix (http://, https://, socks4://, socks5://, ss://) ────
    // Pass through as-is: reqwest/wreq/azuretls all accept full URL proxy strings.
    // ss:// (Shadowsocks): stored with the address intact; emitted as socks5://
    // at use-time since sslocal exposes a local SOCKS5 interface.
    for (prefix, proxy_type) in &[
        ("socks5://", ProxyType::Socks5),
        ("socks4://", ProxyType::Socks4),
        ("https://", ProxyType::Https),
        ("http://", ProxyType::Http),
        ("ss://", ProxyType::Shadowsocks),
    ] {
        if let Some(rest) = line.strip_prefix(prefix) {
            return Some(ProxyEntry {
                proxy_type: *proxy_type,
                address: rest.to_string(),
            });
        }
    }

    // ── Colon-separated formats (no scheme prefix) ───────────────────────────
    // Split only on ':' but be careful: the '@' separator in user:pass@host:port
    // means we can't just count colons naively.
    //
    // Supported formats (OB2-compatible):
    //   host:port                          → 2 parts
    //   host:port:user:pass                → 4 parts (OB2 standard auth)
    //   type:host:port:user:pass           → 5 parts (explicit type)
    //   user:pass@host:port                → contains '@', 3 colons total
    //   type://user:pass@host:port         → handled above by URL prefix

    // Handle user:pass@host:port (contains '@')
    if let Some(at_pos) = line.rfind('@') {
        let user_pass = &line[..at_pos];
        let host_port = &line[at_pos + 1..];
        // user_pass may be "user:pass" (1 colon) or just "user" (no colon)
        let (user, pass) = if let Some(colon) = user_pass.find(':') {
            (&user_pass[..colon], &user_pass[colon + 1..])
        } else {
            (user_pass, "")
        };
        let address = if pass.is_empty() {
            format!("{}@{}", user, host_port)
        } else {
            format!("{}:{}@{}", user, pass, host_port)
        };
        let host_port_parts: Vec<&str> = host_port.split(':').collect();
        let proxy_type = if host_port_parts.len() == 2 {
            detect_proxy_type_from_port(host_port_parts[1], fallback)
        } else {
            fallback
        };
        return Some(ProxyEntry {
            proxy_type,
            address,
        });
    }

    let parts: Vec<&str> = line.split(':').collect();
    match parts.len() {
        // host:port
        2 => Some(ProxyEntry {
            proxy_type: detect_proxy_type_from_port(parts[1], fallback),
            address: format!("{}:{}", parts[0], parts[1]),
        }),
        // host:port:user:pass  (OB2 standard)
        4 => Some(ProxyEntry {
            proxy_type: detect_proxy_type_from_port(parts[1], fallback),
            address: format!("{}:{}@{}:{}", parts[2], parts[3], parts[0], parts[1]),
        }),
        // type:host:port:user:pass
        5 => {
            let proxy_type = match parts[0].to_lowercase().as_str() {
                "http" => ProxyType::Http,
                "https" => ProxyType::Https,
                "socks4" => ProxyType::Socks4,
                "socks5" => ProxyType::Socks5,
                "shadowsocks" | "ss" => ProxyType::Shadowsocks,
                _ => fallback,
            };
            Some(ProxyEntry {
                proxy_type,
                address: format!("{}:{}@{}:{}", parts[3], parts[4], parts[1], parts[2]),
            })
        }
        _ => None,
    }
}
