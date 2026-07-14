use uuid::Uuid;

use super::*; // includes TlsClient via block::settings_http::* and RustlsClientSlot

fn has_header(headers: &[Vec<String>], name: &str) -> bool {
    headers.iter().any(|header| {
        header
            .first()
            .is_some_and(|key| key.eq_ignore_ascii_case(name))
    })
}

impl ExecutionContext {
    fn clear_http_response_state(&mut self, var_prefix: &str) {
        self.variables.clear_data_prefix(var_prefix);
        for alias in ["RESPONSECODE", "ADDRESS", "LASTRESPONSE", "LASTHEADERS"] {
            self.variables.set_data(alias, String::new());
        }
    }

    pub(super) async fn execute_http_request(
        &mut self,
        block: &Block,
        settings: &HttpRequestSettings,
        sidecar_tx: &tokio::sync::mpsc::Sender<(
            SidecarRequest,
            tokio::sync::oneshot::Sender<SidecarResponse>,
        )>,
    ) -> crate::error::Result<()> {
        if matches!(settings.body_type, BodyType::Multipart) {
            return Err(crate::error::AppError::Pipeline(
                "Multipart HTTP bodies require a field/boundary schema and are not supported by the native pipeline engine.".into(),
            ));
        }

        let var_prefix = if settings.response_var.is_empty() {
            "SOURCE"
        } else {
            &settings.response_var
        };
        self.clear_http_response_state(var_prefix);

        // ── Interpolate common request fields ─────────────────────────────────
        let url = self.variables.interpolate(&settings.url);
        let body = if matches!(settings.body_type, BodyType::None) {
            String::new()
        } else {
            self.variables.interpolate(&settings.body)
        };
        let mut headers: Vec<Vec<String>> = settings
            .headers
            .iter()
            .map(|(k, v)| vec![self.variables.interpolate(k), self.variables.interpolate(v)])
            .collect();

        // Apply configured metadata only when the caller did not set an explicit
        // header. That keeps imported/manual headers authoritative.
        if !body.is_empty()
            && !settings.content_type.trim().is_empty()
            && !has_header(&headers, "Content-Type")
        {
            headers.push(vec![
                "Content-Type".into(),
                self.variables.interpolate(&settings.content_type),
            ]);
        }
        if let Some((username, password)) = &settings.basic_auth {
            if !has_header(&headers, "Authorization") {
                use base64::Engine as _;
                let credentials = format!(
                    "{}:{}",
                    self.variables.interpolate(username),
                    self.variables.interpolate(password)
                );
                headers.push(vec![
                    "Authorization".into(),
                    format!(
                        "Basic {}",
                        base64::engine::general_purpose::STANDARD.encode(credentials)
                    ),
                ]);
            }
        }

        // Inject SPOOF_HEADERS set by a preceding HeaderSpoof block.
        // SPOOF_HEADERS is consumed (cleared) after reading so it only applies
        // to the next HTTP block, not every subsequent one in the pipeline.
        // To apply to multiple requests, place a HeaderSpoof block before each one.
        if let Some(spoof_json) = self.variables.get("SPOOF_HEADERS") {
            // Consume immediately — clear before processing so a failed parse
            // doesn't leave a poisoned value that re-fires on the next request.
            self.variables
                .set_user("SPOOF_HEADERS", String::new(), false);
            if let Ok(pairs) = serde_json::from_str::<Vec<(String, String)>>(&spoof_json) {
                for (name, mut value) in pairs {
                    if name == "X-Forwarded-Host" && value.is_empty() {
                        // Fill in actual request host from the (interpolated) URL
                        value = url
                            .split("://")
                            .nth(1)
                            .unwrap_or("")
                            .split(&['/', '?', '#'][..])
                            .next()
                            .unwrap_or("")
                            .to_string();
                    }
                    if !name.is_empty() && !value.is_empty() {
                        headers.push(vec![name, value]);
                    }
                }
            }
        }

        // Inject custom cookies as a Cookie header (one per line: name=value)
        if !settings.custom_cookies.is_empty() {
            let cookie_str = self.variables.interpolate(&settings.custom_cookies);
            let cookies: Vec<String> = cookie_str
                .lines()
                .map(|l| l.trim())
                .filter(|l| !l.is_empty())
                .map(|l| l.to_string())
                .collect();
            if !cookies.is_empty() {
                headers.push(vec!["Cookie".into(), cookies.join("; ")]);
            }
        }

        // ── Build the protocol-level request (shared by both backends) ────────
        // Per-block JA3/browser overrides take precedence over pipeline-level settings.
        let effective_ja3 = if !settings.ja3_override.is_empty() {
            Some(settings.ja3_override.clone())
        } else {
            self.override_ja3.clone()
        };
        let effective_http2fp = if !settings.http2fp_override.is_empty() {
            Some(settings.http2fp_override.clone())
        } else {
            self.override_http2fp.clone()
        };
        let effective_browser = if !settings.browser_profile.is_empty() {
            Some(settings.browser_profile.clone())
        } else {
            None
        };

        let sidecar_req = SidecarRequest {
            id: Uuid::new_v4().to_string(),
            action: "request".into(),
            session: self.session_id.clone(),
            method: Some(settings.method.clone()),
            url: Some(url.clone()),
            headers: Some(headers.clone()),
            body: Some(body.clone()),
            timeout: Some(settings.timeout_ms as i64),
            proxy: self.proxy.clone(),
            browser: effective_browser,
            ja3: effective_ja3,
            http2fp: effective_http2fp,
            follow_redirects: Some(settings.follow_redirects && settings.auto_redirect),
            max_redirects: Some(settings.max_redirects as i64),
            ssl_verify: if settings.ssl_verify {
                None
            } else {
                Some(false)
            },
            custom_ciphers: if settings.cipher_suites.is_empty() {
                None
            } else {
                Some(settings.cipher_suites.clone())
            },
            proxy_insecure: if settings.proxy_insecure {
                Some(true)
            } else {
                None
            },
            ..Default::default()
        };

        // ── Dispatch to the chosen TLS backend ────────────────────────────────
        let resp = match settings.tls_client {
            TlsClient::RustTLS => {
                // Native reqwest + rustls. Reuse the session-scoped client so the
                // cookie jar persists across HTTP blocks in the same pipeline run.
                // Only reuse if the proxy matches — otherwise rebuild with new proxy.
                let current_proxy = self.proxy.clone();
                let existing = self.rustls_client.take().and_then(|slot| {
                    if slot.proxy == current_proxy {
                        Some(slot.client)
                    } else {
                        None
                    }
                });
                let (resp, client) = crate::sidecar::native::execute_rustls_request(
                    &sidecar_req,
                    settings.ssl_verify,
                    settings.proxy_insecure,
                    existing,
                )
                .await;
                self.rustls_client = Some(RustlsClientSlot {
                    client,
                    proxy: self.proxy.clone(),
                });
                resp
            }
            #[cfg(feature = "wreq-tls")]
            TlsClient::WreqTLS => {
                let emu = if settings.wreq_emulation.is_empty() {
                    "Chrome134"
                } else {
                    settings.wreq_emulation.as_str()
                };
                let current_proxy = self.proxy.clone();
                let existing = self.wreq_client.take().and_then(|slot| {
                    if slot.emulation == emu && slot.proxy == current_proxy {
                        Some(slot.client)
                    } else {
                        None
                    }
                });
                let (resp, client) = crate::sidecar::wreq_client::execute_wreq_request(
                    &sidecar_req,
                    emu,
                    settings.ssl_verify,
                    settings.proxy_insecure,
                    existing,
                )
                .await;
                self.wreq_client = Some(WreqClientSlot {
                    client,
                    emulation: emu.to_string(),
                    proxy: self.proxy.clone(),
                });
                resp
            }
            #[cfg(not(feature = "wreq-tls"))]
            TlsClient::WreqTLS => SidecarResponse {
                id: sidecar_req.id.clone(),
                error: Some("WreqTLS is not enabled in this build.".into()),
                ..Default::default()
            },
            TlsClient::AzureTLS => {
                // Go sidecar path — azuretls with JA3/TLS fingerprinting support.
                let (resp_tx, resp_rx) = tokio::sync::oneshot::channel();
                sidecar_tx.send((sidecar_req, resp_tx)).await
                    .map_err(|_| crate::error::AppError::Sidecar(
                        "Sidecar channel closed — is reqflow-sidecar running next to the executable?".into()
                    ))?;
                // Wrap with a timeout matching the block's own timeout_ms so a crashed
                // or unresponsive sidecar fails fast instead of hanging indefinitely.
                let timeout_dur =
                    std::time::Duration::from_millis((settings.timeout_ms as u64).max(5_000));
                tokio::time::timeout(timeout_dur, resp_rx)
                    .await
                    .map_err(|_| crate::error::AppError::Sidecar(
                        "Sidecar request timed out — reqflow-sidecar may have crashed or is not responding".into()
                    ))?
                    .map_err(|_| crate::error::AppError::Sidecar(
                        "Sidecar response channel closed — reqflow-sidecar process may have exited".into()
                    ))?
            }
        };

        if let Some(ref err) = resp.error {
            if !err.is_empty() {
                self.variables
                    .set_data(&format!("{}.ERROR", var_prefix), err.clone());
                return Err(crate::error::AppError::Sidecar(err.clone()));
            }
        }

        // ── Store response into pipeline variables ────────────────────────────
        // Body
        self.variables.set_data(var_prefix, resp.body.clone());
        // Status code
        self.variables
            .set_data(&format!("{}.STATUS", var_prefix), resp.status.to_string());
        // Final URL (after redirects)
        self.variables
            .set_data(&format!("{}.URL", var_prefix), resp.final_url.clone());
        // Response headers as JSON object
        if let Some(ref hdrs) = resp.headers {
            let hdr_str = serde_json::to_string(hdrs).unwrap_or_default();
            self.variables
                .set_data(&format!("{}.HEADERS", var_prefix), hdr_str);

            // Also expose individual headers as SOURCE.HEADERS.<Name>
            for (name, value) in hdrs {
                self.variables.set_data(
                    &format!("{}.HEADERS.{}", var_prefix, name.to_lowercase()),
                    value.clone(),
                );
            }
        }
        // Cookies as JSON object
        if let Some(ref cookies) = resp.cookies {
            let cookie_str = serde_json::to_string(cookies).unwrap_or_default();
            self.variables
                .set_data(&format!("{}.COOKIES", var_prefix), cookie_str);

            // Also expose individual cookies as SOURCE.COOKIES.<Name>
            for (name, value) in cookies {
                self.variables
                    .set_data(&format!("{}.COOKIES.{}", var_prefix, name), value.clone());
            }
        }

        // Backward-compat aliases used by legacy checks + output format variables
        self.variables
            .set_data("RESPONSECODE", resp.status.to_string());
        self.variables.set_data("ADDRESS", resp.final_url.clone());
        // LASTRESPONSE / LASTHEADERS: always-updated aliases for the {response} and
        // {headers} output format variables so they don't depend on the block's output var name.
        self.variables.set_data("LASTRESPONSE", resp.body.clone());
        if let Some(ref hdrs) = resp.headers {
            let hdr_str = serde_json::to_string(hdrs).unwrap_or_default();
            self.variables.set_data("LASTHEADERS", hdr_str);
        }

        // ── Update block result (shown in UI block overlay) ───────────────────
        if let Some(last) = self.block_results.last_mut() {
            last.request = Some(RequestInfo {
                method: settings.method.clone(),
                url: url.clone(),
                headers: headers
                    .iter()
                    .map(|h| {
                        (
                            h.get(0).cloned().unwrap_or_default(),
                            h.get(1).cloned().unwrap_or_default(),
                        )
                    })
                    .collect(),
                body: body.clone(),
            });
            last.response = Some(ResponseInfo {
                status_code: resp.status as u16,
                headers: resp.headers.clone().unwrap_or_default(),
                body: resp.body.clone(),
                final_url: resp.final_url.clone(),
                cookies: resp.cookies.clone().unwrap_or_default(),
                timing_ms: resp.timing_ms as u64,
            });
        }

        // ── Network log entry ─────────────────────────────────────────────────
        let cookies_sent: Vec<(String, String)> = settings
            .custom_cookies
            .lines()
            .map(|l| l.trim())
            .filter(|l| !l.is_empty())
            .filter_map(|l| {
                l.split_once('=')
                    .map(|(k, v)| (k.trim().to_string(), v.trim().to_string()))
            })
            .collect();
        let cookies_set: Vec<(String, String)> = resp
            .cookies
            .as_ref()
            .map(|c| c.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
            .unwrap_or_default();

        self.network_log.push(NetworkEntry {
            block_id: block.id,
            block_label: block.label.clone(),
            method: settings.method.clone(),
            url,
            status_code: resp.status as u16,
            timing_ms: resp.timing_ms as u64,
            response_size: resp.body.len(),
            cookies_set,
            cookies_sent,
        });

        Ok(())
    }
}
