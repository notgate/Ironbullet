// WreqTLS is compiled only when the `wreq-tls` Cargo feature is enabled.

#[cfg(feature = "wreq-tls")]
mod inner {
    use crate::sidecar::protocol::{SidecarRequest, SidecarResponse};
    use std::time::Instant;
    use wreq::redirect;
    use wreq_util::Emulation;

    pub(crate) fn parse_emulation(s: &str) -> Emulation {
        match s {
            "Chrome100" => Emulation::Chrome100,
            "Chrome101" => Emulation::Chrome101,
            "Chrome104" => Emulation::Chrome104,
            "Chrome105" => Emulation::Chrome105,
            "Chrome106" => Emulation::Chrome106,
            "Chrome107" => Emulation::Chrome107,
            "Chrome108" => Emulation::Chrome108,
            "Chrome109" => Emulation::Chrome109,
            "Chrome110" => Emulation::Chrome110,
            "Chrome114" => Emulation::Chrome114,
            "Chrome116" => Emulation::Chrome116,
            "Chrome117" => Emulation::Chrome117,
            "Chrome118" => Emulation::Chrome118,
            "Chrome119" => Emulation::Chrome119,
            "Chrome120" => Emulation::Chrome120,
            "Chrome123" => Emulation::Chrome123,
            "Chrome124" => Emulation::Chrome124,
            "Chrome126" => Emulation::Chrome126,
            "Chrome127" => Emulation::Chrome127,
            "Chrome128" => Emulation::Chrome128,
            "Chrome129" => Emulation::Chrome129,
            "Chrome130" => Emulation::Chrome130,
            "Chrome131" => Emulation::Chrome131,
            "Chrome132" => Emulation::Chrome132,
            "Chrome133" => Emulation::Chrome133,
            "Chrome134" => Emulation::Chrome134,
            "Chrome135" => Emulation::Chrome135,
            "Chrome136" => Emulation::Chrome136,
            "Chrome137" => Emulation::Chrome137,
            "Edge101" => Emulation::Edge101,
            "Edge122" => Emulation::Edge122,
            "Edge127" => Emulation::Edge127,
            "Edge131" => Emulation::Edge131,
            "Edge134" => Emulation::Edge134,
            "Firefox109" => Emulation::Firefox109,
            "Firefox117" => Emulation::Firefox117,
            "Firefox128" => Emulation::Firefox128,
            "Firefox133" => Emulation::Firefox133,
            "Firefox135" => Emulation::Firefox135,
            "Firefox136" => Emulation::Firefox136,
            "Firefox139" => Emulation::Firefox139,
            "Safari15_3" => Emulation::Safari15_3,
            "Safari15_5" => Emulation::Safari15_5,
            "Safari15_6_1" => Emulation::Safari15_6_1,
            "Safari16" => Emulation::Safari16,
            "Safari16_5" => Emulation::Safari16_5,
            "Safari17_0" => Emulation::Safari17_0,
            "Safari17_2_1" => Emulation::Safari17_2_1,
            "Safari17_4_1" => Emulation::Safari17_4_1,
            "Safari17_5" => Emulation::Safari17_5,
            "Safari18" => Emulation::Safari18,
            "Safari18_2" => Emulation::Safari18_2,
            "Safari18_3" => Emulation::Safari18_3,
            "Safari18_3_1" => Emulation::Safari18_3_1,
            "OkHttp3_9" => Emulation::OkHttp3_9,
            "OkHttp3_11" => Emulation::OkHttp3_11,
            "OkHttp3_13" => Emulation::OkHttp3_13,
            "OkHttp3_14" => Emulation::OkHttp3_14,
            "OkHttp4_9" => Emulation::OkHttp4_9,
            "OkHttp4_10" => Emulation::OkHttp4_10,
            "OkHttp4_12" => Emulation::OkHttp4_12,
            "OkHttp5" => Emulation::OkHttp5,
            "Opera116" => Emulation::Opera116,
            "Opera117" => Emulation::Opera117,
            "Opera118" => Emulation::Opera118,
            "Opera119" => Emulation::Opera119,
            _ => Emulation::Chrome134,
        }
    }

    pub(crate) fn build_wreq_client(
        emulation: &str,
        ssl_verify: bool,
        proxy_insecure: bool,
        proxy_url: Option<&str>,
    ) -> Result<wreq::Client, wreq::Error> {
        let emu = parse_emulation(emulation);
        let mut builder = wreq::Client::builder()
            .emulation(emu)
            .cookie_store(true)
            // Connect timeout — prevents dead hosts hanging for OS TCP timeout (~2 min)
            .connect_timeout(std::time::Duration::from_secs(10))
            // Cap idle pool to avoid fd exhaustion at high thread counts
            .pool_max_idle_per_host(4)
            .pool_idle_timeout(std::time::Duration::from_secs(30))
            .tcp_keepalive(std::time::Duration::from_secs(30));
        // Disable cert verification when ssl_verify=false OR proxy_insecure=true.
        // wreq applies TLS config at the client level, so proxy_insecure unavoidably
        // also disables target TLS verification. Use AzureTLS to avoid this.
        if !ssl_verify || proxy_insecure {
            builder = builder.cert_verification(false);
        }
        if let Some(proxy) = proxy_url {
            if !proxy.is_empty() {
                if let Ok(p) = wreq::Proxy::all(proxy) {
                    builder = builder.proxy(p);
                }
            }
        }
        builder.build()
    }

    pub(crate) async fn execute_wreq_request(
        req: &SidecarRequest,
        emulation: &str,
        ssl_verify: bool,
        proxy_insecure: bool,
        existing_client: Option<wreq::Client>,
    ) -> (SidecarResponse, wreq::Client) {
        let proxy_str = req.proxy.as_deref();
        let client = existing_client.unwrap_or_else(|| {
            build_wreq_client(emulation, ssl_verify, proxy_insecure, proxy_str).unwrap_or_else(
                |e| {
                    eprintln!("[wreq] client build error: {e}");
                    wreq::Client::new()
                },
            )
        });

        let url = match req.url.as_deref() {
            Some(u) if !u.is_empty() => u.to_string(),
            _ => {
                return (
                    SidecarResponse {
                        id: req.id.clone(),
                        error: Some("WreqTLS: missing URL".into()),
                        ..Default::default()
                    },
                    client,
                );
            }
        };

        let method_str = req.method.as_deref().unwrap_or("GET");
        let method = match wreq::Method::from_bytes(method_str.as_bytes()) {
            Ok(m) => m,
            Err(_) => {
                return (
                    SidecarResponse {
                        id: req.id.clone(),
                        error: Some(format!("WreqTLS: invalid method '{method_str}'")),
                        ..Default::default()
                    },
                    client,
                );
            }
        };

        let mut rb = client.request(method, &url);

        if let Some(headers) = &req.headers {
            for pair in headers {
                if pair.len() == 2 {
                    rb = rb.header(&pair[0], &pair[1]);
                }
            }
        }

        if let Some(body) = req.body.as_deref() {
            if !body.is_empty() {
                rb = rb.body(body.to_string());
            }
        }

        if let Some(ms) = req.timeout {
            rb = rb.timeout(std::time::Duration::from_millis(ms as u64));
        }

        let follow = req.follow_redirects.unwrap_or(true);
        let max_redir = req.max_redirects.unwrap_or(8) as usize;
        if follow && max_redir > 0 {
            rb = rb.redirect(redirect::Policy::limited(max_redir));
        } else {
            rb = rb.redirect(redirect::Policy::none());
        }

        let t0 = Instant::now();
        let result = rb.send().await;
        let timing_ms = t0.elapsed().as_millis() as i64;

        match result {
            Ok(resp) => {
                let status = resp.status().as_u16() as i32;
                let final_url = resp.uri().to_string();

                let mut headers_map = std::collections::HashMap::new();
                for (name, value) in resp.headers() {
                    headers_map.insert(
                        name.as_str().to_string(),
                        value.to_str().unwrap_or("").to_string(),
                    );
                }

                // Extract cookies via resp.cookies() — the correct source when
                // cookie_store(true) is active. Reading Set-Cookie from resp.headers()
                // always returns empty because wreq/reqwest consumes those headers into
                // the internal jar before we see them.
                let mut cookies_map = std::collections::HashMap::new();
                for cookie in resp.cookies() {
                    cookies_map.insert(cookie.name().to_string(), cookie.value().to_string());
                }

                let body = match resp.text().await {
                    Ok(t) => t,
                    Err(e) => {
                        eprintln!("[wreq] body read error: {e}");
                        String::new()
                    }
                };

                (
                    SidecarResponse {
                        id: req.id.clone(),
                        status,
                        body,
                        final_url,
                        headers: Some(headers_map),
                        cookies: Some(cookies_map),
                        timing_ms,
                        error: None,
                        ..Default::default()
                    },
                    client,
                )
            }
            Err(e) => {
                eprintln!("[wreq] request error: {e}");
                (
                    SidecarResponse {
                        id: req.id.clone(),
                        error: Some(format!("WreqTLS: {e}")),
                        timing_ms,
                        ..Default::default()
                    },
                    client,
                )
            }
        }
    }
}

#[cfg(feature = "wreq-tls")]
pub(crate) use inner::execute_wreq_request;
