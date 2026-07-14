use super::*;
use std::sync::Arc;

impl ExecutionContext {
    // ── TCP Request ──

    pub(super) async fn execute_tcp_request(
        &mut self,
        _block: &Block,
        settings: &TcpRequestSettings,
    ) -> crate::error::Result<()> {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};

        let host = self.variables.interpolate(&settings.host);
        let data = self.variables.interpolate(&settings.data);
        let timeout = std::time::Duration::from_millis(settings.timeout_ms);
        let addr = format!("{}:{}", host, settings.port);

        let stream = tokio::time::timeout(timeout, tokio::net::TcpStream::connect(&addr))
            .await
            .map_err(|_| {
                crate::error::AppError::Pipeline(format!("TCP connect timeout: {}", addr))
            })?
            .map_err(|e| {
                crate::error::AppError::Pipeline(format!("TCP connect failed {}: {}", addr, e))
            })?;

        let response_body = if settings.use_tls {
            let tls_config = {
                let mut cfg = rustls::ClientConfig::builder()
                    .with_root_certificates({
                        let mut roots = rustls::RootCertStore::empty();
                        roots.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
                        roots
                    })
                    .with_no_client_auth();
                if !settings.ssl_verify {
                    cfg.dangerous().set_certificate_verifier(Arc::new(
                        crate::pipeline::engine::protocol::NoVerify,
                    ));
                }
                Arc::new(cfg)
            };
            let connector = tokio_rustls::TlsConnector::from(tls_config);
            let sn = rustls::pki_types::ServerName::try_from(host.as_str())
                .map_err(|e| crate::error::AppError::Pipeline(format!("Invalid hostname: {}", e)))?
                .to_owned();
            let mut tls_stream = connector.connect(sn, stream).await.map_err(|e| {
                crate::error::AppError::Pipeline(format!("TLS handshake failed: {}", e))
            })?;

            if !data.is_empty() {
                tls_stream.write_all(data.as_bytes()).await.map_err(|e| {
                    crate::error::AppError::Pipeline(format!("TLS write error: {}", e))
                })?;
                tls_stream.flush().await.ok();
            }

            let mut buf = vec![0u8; 65536];
            match tokio::time::timeout(timeout, tls_stream.read(&mut buf)).await {
                Ok(Ok(n)) => String::from_utf8_lossy(&buf[..n]).to_string(),
                _ => String::new(),
            }
        } else {
            let mut stream = stream;
            if !data.is_empty() {
                stream.write_all(data.as_bytes()).await.map_err(|e| {
                    crate::error::AppError::Pipeline(format!("TCP write error: {}", e))
                })?;
                stream.flush().await.ok();
            }

            let mut buf = vec![0u8; 65536];
            match tokio::time::timeout(timeout, stream.read(&mut buf)).await {
                Ok(Ok(n)) => String::from_utf8_lossy(&buf[..n]).to_string(),
                _ => String::new(),
            }
        };

        self.variables.set_user(
            &settings.output_var,
            response_body.clone(),
            settings.capture,
        );

        if let Some(last) = self.block_results.last_mut() {
            last.request = Some(RequestInfo {
                method: if settings.use_tls {
                    "TCP+TLS".into()
                } else {
                    "TCP".into()
                },
                url: addr,
                headers: vec![],
                body: data,
            });
            last.response = Some(ResponseInfo {
                status_code: if response_body.is_empty() { 0 } else { 200 },
                headers: std::collections::HashMap::new(),
                body: response_body,
                final_url: String::new(),
                cookies: std::collections::HashMap::new(),
                timing_ms: 0,
            });
        }

        Ok(())
    }

    // ── UDP Request ──

    pub(super) async fn execute_udp_request(
        &mut self,
        _block: &Block,
        settings: &UdpRequestSettings,
    ) -> crate::error::Result<()> {
        let host = self.variables.interpolate(&settings.host);
        let data = self.variables.interpolate(&settings.data);
        let timeout = std::time::Duration::from_millis(settings.timeout_ms);
        let addr = format!("{}:{}", host, settings.port);

        let socket = tokio::net::UdpSocket::bind("0.0.0.0:0")
            .await
            .map_err(|e| crate::error::AppError::Pipeline(format!("UDP bind error: {}", e)))?;

        socket
            .send_to(data.as_bytes(), &addr)
            .await
            .map_err(|e| crate::error::AppError::Pipeline(format!("UDP send error: {}", e)))?;

        let mut buf = vec![0u8; 65536];
        let response_body = match tokio::time::timeout(timeout, socket.recv_from(&mut buf)).await {
            Ok(Ok((n, _src))) => String::from_utf8_lossy(&buf[..n]).to_string(),
            _ => String::new(),
        };

        self.variables.set_user(
            &settings.output_var,
            response_body.clone(),
            settings.capture,
        );

        if let Some(last) = self.block_results.last_mut() {
            last.request = Some(RequestInfo {
                method: "UDP".into(),
                url: addr,
                headers: vec![],
                body: data,
            });
            last.response = Some(ResponseInfo {
                status_code: if response_body.is_empty() { 0 } else { 200 },
                headers: std::collections::HashMap::new(),
                body: response_body,
                final_url: String::new(),
                cookies: std::collections::HashMap::new(),
                timing_ms: 0,
            });
        }

        Ok(())
    }

    // ── FTP Request ──

    pub(super) async fn execute_ftp_request(
        &mut self,
        _block: &Block,
        settings: &FtpRequestSettings,
    ) -> crate::error::Result<()> {
        use tokio::io::{AsyncBufReadExt, AsyncWriteExt};

        let host = self.variables.interpolate(&settings.host);
        let username = self.variables.interpolate(&settings.username);
        let password = self.variables.interpolate(&settings.password);
        let cmd_base = self.variables.interpolate(&settings.command);
        let remote_path = self.variables.interpolate(&settings.remote_path);
        let timeout = std::time::Duration::from_millis(settings.timeout_ms);
        let addr = format!("{}:{}", host, settings.port);

        // Build the full FTP command: for commands that take a path, append remote_path
        let command = if !remote_path.is_empty()
            && matches!(
                cmd_base.as_str(),
                "RETR" | "STOR" | "DELE" | "MKD" | "RMD" | "CWD"
            ) {
            format!("{} {}", cmd_base, remote_path)
        } else {
            cmd_base.clone()
        };

        let stream = tokio::time::timeout(timeout, tokio::net::TcpStream::connect(&addr))
            .await
            .map_err(|_| {
                crate::error::AppError::Pipeline(format!("FTP connect timeout: {}", addr))
            })?
            .map_err(|e| {
                crate::error::AppError::Pipeline(format!("FTP connect failed {}: {}", addr, e))
            })?;

        let (reader, writer) = tokio::io::split(stream);
        let mut reader = tokio::io::BufReader::new(reader);
        let mut writer = writer;
        let mut transcript = String::new();

        // Read the server banner — may be multi-line (e.g. "220-...\r\n220 ...\r\n").
        // Must consume the full banner before sending USER, otherwise the server's
        // remaining banner lines land in the USER-response reader causing a false
        // response code mismatch (race condition, issue #11).
        let mut line = String::new();
        loop {
            line.clear();
            match tokio::time::timeout(
                std::time::Duration::from_secs(5),
                reader.read_line(&mut line),
            )
            .await
            {
                Ok(Ok(0)) | Err(_) | Ok(Err(_)) => break,
                Ok(Ok(_)) => {
                    transcript.push_str(&format!("S: {}", line));
                    // FTP multi-line: continuation lines have "XYZ-" (dash at pos 3).
                    // A space at pos 3 (or line shorter than 4) ends the response.
                    if line.len() < 4 || line.as_bytes()[3] != b'-' {
                        break;
                    }
                }
            }
        }

        // Validate banner code — must be 2xx to proceed.
        let banner_code: u16 = transcript
            .lines()
            .last()
            .and_then(|l| l.strip_prefix("S: "))
            .and_then(|l| l.get(..3))
            .and_then(|c| c.parse().ok())
            .unwrap_or(0);

        let commands: Vec<String> = if banner_code >= 200 && banner_code < 300 {
            vec![
                format!("USER {}", username),
                format!("PASS {}", password),
                command.clone(),
                "QUIT".into(),
            ]
        } else {
            // Server rejected connection — send QUIT and bail.
            vec!["QUIT".into()]
        };

        let mut last_code: u16 = banner_code;
        for cmd in &commands {
            if cmd.is_empty() {
                continue;
            }
            writer
                .write_all(format!("{}\r\n", cmd).as_bytes())
                .await
                .ok();
            writer.flush().await.ok();
            transcript.push_str(&format!("C: {}\r\n", cmd));

            // Read FTP response (multi-line: "123-...\r\n123 ...\r\n").
            // Wait for the terminating line (space at position 3) before sending
            // the next command — fixes the race condition in issue #11.
            loop {
                line.clear();
                match tokio::time::timeout(
                    std::time::Duration::from_secs(5),
                    reader.read_line(&mut line),
                )
                .await
                {
                    Ok(Ok(0)) | Err(_) | Ok(Err(_)) => break,
                    Ok(Ok(_)) => {
                        transcript.push_str(&format!("S: {}", line));
                        if let Ok(code) = line.get(..3).unwrap_or("").parse::<u16>() {
                            last_code = code;
                        }
                        if line.len() < 4 || line.as_bytes()[3] != b'-' {
                            break;
                        }
                    }
                }
            }

            // Stop after QUIT or if the server sends a fatal error (5xx).
            if cmd == "QUIT" {
                break;
            }
            if last_code >= 500 {
                break;
            }
        }

        self.variables
            .set_user(&settings.output_var, transcript.clone(), settings.capture);

        if let Some(last) = self.block_results.last_mut() {
            last.request = Some(RequestInfo {
                method: "FTP".into(),
                url: addr,
                headers: vec![],
                body: commands
                    .iter()
                    .filter(|c| !c.is_empty())
                    .cloned()
                    .collect::<Vec<_>>()
                    .join("\r\n"),
            });
            last.response = Some(ResponseInfo {
                status_code: last_code,
                headers: std::collections::HashMap::new(),
                body: transcript,
                final_url: String::new(),
                cookies: std::collections::HashMap::new(),
                timing_ms: 0,
            });
        }

        Ok(())
    }

    // ── SSH Request (banner grab + auth attempt via raw protocol) ──

    #[cfg(not(windows))]
    pub(super) async fn execute_ssh_request(
        &mut self,
        _block: &Block,
        settings: &SshRequestSettings,
    ) -> crate::error::Result<()> {
        let host = self.variables.interpolate(&settings.host);
        let username = self.variables.interpolate(&settings.username);
        let password = self.variables.interpolate(&settings.password);
        let action = self.variables.interpolate(&settings.command);
        let ssh_cmd = self.variables.interpolate(&settings.ssh_cmd);
        let timeout_ms = settings.timeout_ms.min(u32::MAX as u64) as u32;
        let timeout = std::time::Duration::from_millis(settings.timeout_ms);
        let addr = format!("{}:{}", host, settings.port);
        let request_body = format!("user={} action={} ssh_cmd={}", username, action, ssh_cmd);

        let addr_for_task = addr.clone();
        let host_for_task = host.clone();
        let username_for_task = username.clone();
        let password_for_task = password.clone();
        let action_for_task = action.clone();
        let ssh_cmd_for_task = ssh_cmd.clone();

        let started = std::time::Instant::now();
        let (status_code, headers, transcript) =
            tokio::task::spawn_blocking(move || -> crate::error::Result<_> {
                use ssh2::Session;
                use std::collections::HashMap;
                use std::io::Read;
                use std::net::TcpStream;

                let mut headers = HashMap::new();
                let tcp = TcpStream::connect(&addr_for_task).map_err(|e| {
                    crate::error::AppError::Pipeline(format!(
                        "SSH connect failed {}: {}",
                        addr_for_task, e
                    ))
                })?;
                tcp.set_read_timeout(Some(timeout)).ok();
                tcp.set_write_timeout(Some(timeout)).ok();

                let mut sess = Session::new().map_err(|e| {
                    crate::error::AppError::Pipeline(format!("SSH session init failed: {}", e))
                })?;
                sess.set_tcp_stream(tcp);
                sess.set_timeout(timeout_ms);
                sess.handshake().map_err(|e| {
                    crate::error::AppError::Pipeline(format!(
                        "SSH handshake failed {}: {}",
                        addr_for_task, e
                    ))
                })?;

                let banner = sess.banner().unwrap_or_default().trim().to_string();
                if !banner.is_empty() {
                    headers.insert("Server-Banner".into(), banner.clone());
                }
                headers.insert("SSH-Action".into(), action_for_task.clone());

                let mut transcript = String::new();
                if !banner.is_empty() {
                    transcript.push_str(&format!("S: {}\n", banner));
                }
                transcript.push_str(&format!("Auth: password login for {}\n", username_for_task));

                sess.userauth_password(&username_for_task, &password_for_task)
                    .map_err(|e| {
                        crate::error::AppError::Pipeline(format!(
                            "SSH authentication failed for {}@{}: {}",
                            username_for_task, host_for_task, e
                        ))
                    })?;
                if !sess.authenticated() {
                    return Err(crate::error::AppError::Pipeline(format!(
                        "SSH authentication failed for {}@{}",
                        username_for_task, host_for_task
                    )));
                }

                transcript.push_str("Auth result: OK\n");

                if action_for_task == "exec" {
                    let cmd = if ssh_cmd_for_task.trim().is_empty() {
                        "whoami"
                    } else {
                        ssh_cmd_for_task.trim()
                    };
                    let mut channel = sess.channel_session().map_err(|e| {
                        crate::error::AppError::Pipeline(format!("SSH channel open failed: {}", e))
                    })?;
                    channel.exec(cmd).map_err(|e| {
                        crate::error::AppError::Pipeline(format!(
                            "SSH exec failed for '{}': {}",
                            cmd, e
                        ))
                    })?;

                    let mut stdout = String::new();
                    channel.read_to_string(&mut stdout).ok();
                    let mut stderr = String::new();
                    channel.stderr().read_to_string(&mut stderr).ok();
                    channel.wait_close().ok();
                    let exit_status = channel.exit_status().unwrap_or_default();
                    headers.insert("SSH-Exit-Status".into(), exit_status.to_string());
                    transcript.push_str(&format!("Command: {}\n", cmd));
                    if !stdout.trim().is_empty() {
                        transcript.push_str("STDOUT:\n");
                        transcript.push_str(stdout.trim_end());
                        transcript.push('\n');
                    }
                    if !stderr.trim().is_empty() {
                        transcript.push_str("STDERR:\n");
                        transcript.push_str(stderr.trim_end());
                        transcript.push('\n');
                    }
                    Ok((
                        if exit_status == 0 { 200 } else { 500 },
                        headers,
                        transcript,
                    ))
                } else {
                    Ok((200, headers, transcript))
                }
            })
            .await
            .map_err(|e| {
                crate::error::AppError::Pipeline(format!("SSH worker join failed: {}", e))
            })??;

        self.variables
            .set_user(&settings.output_var, transcript.clone(), settings.capture);

        if let Some(last) = self.block_results.last_mut() {
            last.request = Some(RequestInfo {
                method: "SSH".into(),
                url: addr,
                headers: vec![],
                body: request_body,
            });
            last.response = Some(ResponseInfo {
                status_code,
                headers,
                body: transcript,
                final_url: String::new(),
                cookies: std::collections::HashMap::new(),
                timing_ms: started.elapsed().as_millis() as u64,
            });
        }

        Ok(())
    }

    #[cfg(windows)]
    pub(super) async fn execute_ssh_request(
        &mut self,
        _block: &Block,
        settings: &SshRequestSettings,
    ) -> crate::error::Result<()> {
        self.variables
            .set_user(&settings.output_var, String::new(), settings.capture);

        Err(crate::error::AppError::Pipeline(
            "SSH Request is currently supported on Linux builds only; Windows release builds exclude libssh2/OpenSSL to keep packaging stable.".into(),
        ))
    }

    // ── IMAP Request ──

    pub(super) async fn execute_imap_request(
        &mut self,
        _block: &Block,
        settings: &ImapRequestSettings,
    ) -> crate::error::Result<()> {
        use tokio::io::{AsyncBufReadExt, AsyncWriteExt};

        let host = self.variables.interpolate(&settings.host);
        let username = self.variables.interpolate(&settings.username);
        let password = self.variables.interpolate(&settings.password);
        let command = self.variables.interpolate(&settings.command);
        let mailbox = self.variables.interpolate(&settings.mailbox);
        let timeout = std::time::Duration::from_millis(settings.timeout_ms);
        let addr = format!("{}:{}", host, settings.port);

        // Build the extra IMAP command from action + args
        let extra_cmd: Option<String> = match command.as_str() {
            "LOGIN" | "" => None,
            "SELECT" => Some(format!(
                "SELECT {}",
                if mailbox.is_empty() {
                    "INBOX"
                } else {
                    &mailbox
                }
            )),
            "FETCH" => Some(format!(
                "SELECT {}\r\na002b FETCH {} BODY[]",
                if mailbox.is_empty() {
                    "INBOX"
                } else {
                    &mailbox
                },
                settings.message_num
            )),
            "SEARCH ALL" => Some(format!(
                "SELECT {}\r\na002b SEARCH ALL",
                if mailbox.is_empty() {
                    "INBOX"
                } else {
                    &mailbox
                }
            )),
            other => Some(other.to_string()),
        };

        let stream = tokio::time::timeout(timeout, tokio::net::TcpStream::connect(&addr))
            .await
            .map_err(|_| {
                crate::error::AppError::Pipeline(format!("IMAP connect timeout: {}", addr))
            })?
            .map_err(|e| {
                crate::error::AppError::Pipeline(format!("IMAP connect failed {}: {}", addr, e))
            })?;

        let mut transcript = String::new();
        let mut last_ok = false;

        if settings.use_tls {
            let tls_config = {
                let mut cfg = rustls::ClientConfig::builder()
                    .with_root_certificates({
                        let mut roots = rustls::RootCertStore::empty();
                        roots.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
                        roots
                    })
                    .with_no_client_auth();
                if !settings.ssl_verify {
                    cfg.dangerous().set_certificate_verifier(Arc::new(
                        crate::pipeline::engine::protocol::NoVerify,
                    ));
                }
                Arc::new(cfg)
            };
            let connector = tokio_rustls::TlsConnector::from(tls_config);
            let sn = rustls::pki_types::ServerName::try_from(host.as_str())
                .map_err(|e| crate::error::AppError::Pipeline(format!("Invalid hostname: {}", e)))?
                .to_owned();
            let tls = connector
                .connect(sn, stream)
                .await
                .map_err(|e| crate::error::AppError::Pipeline(format!("TLS handshake: {}", e)))?;
            let (reader, writer) = tokio::io::split(tls);
            let mut reader = tokio::io::BufReader::new(reader);
            let mut writer = writer;

            // Banner
            let mut line = String::new();
            if let Ok(Ok(n)) = tokio::time::timeout(timeout, reader.read_line(&mut line)).await {
                if n > 0 {
                    transcript.push_str(&format!("S: {}", line));
                }
            }

            // LOGIN
            let login_cmd = format!("a001 LOGIN {} {}", username, password);
            writer
                .write_all(format!("{}\r\n", login_cmd).as_bytes())
                .await
                .ok();
            writer.flush().await.ok();
            transcript.push_str(&format!("C: a001 LOGIN {} ****\r\n", username));
            loop {
                line.clear();
                match tokio::time::timeout(
                    std::time::Duration::from_secs(5),
                    reader.read_line(&mut line),
                )
                .await
                {
                    Ok(Ok(0)) | Err(_) | Ok(Err(_)) => break,
                    Ok(Ok(_)) => {
                        transcript.push_str(&format!("S: {}", line));
                        if line.starts_with("a001 ") {
                            last_ok = line.contains(" OK ");
                            break;
                        }
                    }
                }
            }

            // Extra IMAP command if action requires it
            if last_ok {
                if let Some(ref extra_imap) = extra_cmd {
                    for (tag_n, sub_cmd) in extra_imap
                        .split("\r\n")
                        .filter(|s| !s.is_empty())
                        .enumerate()
                    {
                        let tag = format!("a{:03}", tag_n + 2);
                        let tagged = format!("{} {}", tag, sub_cmd);
                        writer
                            .write_all(format!("{}\r\n", tagged).as_bytes())
                            .await
                            .ok();
                        writer.flush().await.ok();
                        transcript.push_str(&format!("C: {}\r\n", tagged));
                        loop {
                            line.clear();
                            match tokio::time::timeout(
                                std::time::Duration::from_secs(5),
                                reader.read_line(&mut line),
                            )
                            .await
                            {
                                Ok(Ok(0)) | Err(_) | Ok(Err(_)) => break,
                                Ok(Ok(_)) => {
                                    transcript.push_str(&format!("S: {}", line));
                                    if line.starts_with(&format!("{} ", tag)) {
                                        break;
                                    }
                                }
                            }
                        }
                    }
                }
            }

            // LOGOUT
            writer.write_all(b"a003 LOGOUT\r\n").await.ok();
            writer.flush().await.ok();
        } else {
            let (reader, writer) = tokio::io::split(stream);
            let mut reader = tokio::io::BufReader::new(reader);
            let mut writer = writer;

            let mut line = String::new();
            if let Ok(Ok(n)) = tokio::time::timeout(timeout, reader.read_line(&mut line)).await {
                if n > 0 {
                    transcript.push_str(&format!("S: {}", line));
                }
            }

            let login_cmd = format!("a001 LOGIN {} {}", username, password);
            writer
                .write_all(format!("{}\r\n", login_cmd).as_bytes())
                .await
                .ok();
            writer.flush().await.ok();
            transcript.push_str(&format!("C: a001 LOGIN {} ****\r\n", username));
            loop {
                line.clear();
                match tokio::time::timeout(
                    std::time::Duration::from_secs(5),
                    reader.read_line(&mut line),
                )
                .await
                {
                    Ok(Ok(0)) | Err(_) | Ok(Err(_)) => break,
                    Ok(Ok(_)) => {
                        transcript.push_str(&format!("S: {}", line));
                        if line.starts_with("a001 ") {
                            last_ok = line.contains(" OK ");
                            break;
                        }
                    }
                }
            }

            if last_ok {
                if let Some(ref extra_imap) = extra_cmd {
                    for (tag_n, sub_cmd) in extra_imap
                        .split("\r\n")
                        .filter(|s| !s.is_empty())
                        .enumerate()
                    {
                        let tag = format!("a{:03}", tag_n + 2);
                        let tagged = format!("{} {}", tag, sub_cmd);
                        writer
                            .write_all(format!("{}\r\n", tagged).as_bytes())
                            .await
                            .ok();
                        writer.flush().await.ok();
                        transcript.push_str(&format!("C: {}\r\n", tagged));
                        loop {
                            line.clear();
                            match tokio::time::timeout(
                                std::time::Duration::from_secs(5),
                                reader.read_line(&mut line),
                            )
                            .await
                            {
                                Ok(Ok(0)) | Err(_) | Ok(Err(_)) => break,
                                Ok(Ok(_)) => {
                                    transcript.push_str(&format!("S: {}", line));
                                    if line.starts_with(&format!("{} ", tag)) {
                                        break;
                                    }
                                }
                            }
                        }
                    }
                }
            }

            writer.write_all(b"a003 LOGOUT\r\n").await.ok();
            writer.flush().await.ok();
        }

        self.variables
            .set_user(&settings.output_var, transcript.clone(), settings.capture);

        if let Some(last) = self.block_results.last_mut() {
            last.request = Some(RequestInfo {
                method: "IMAP".into(),
                url: addr,
                headers: vec![],
                body: format!("LOGIN {} ****\n{}", username, command),
            });
            last.response = Some(ResponseInfo {
                status_code: if last_ok { 200 } else { 401 },
                headers: std::collections::HashMap::new(),
                body: transcript,
                final_url: String::new(),
                cookies: std::collections::HashMap::new(),
                timing_ms: 0,
            });
        }

        Ok(())
    }

    // ── SMTP Request ──

    pub(super) async fn execute_smtp_request(
        &mut self,
        _block: &Block,
        settings: &SmtpRequestSettings,
    ) -> crate::error::Result<()> {
        use tokio::io::{AsyncBufReadExt, AsyncWriteExt};

        let host = self.variables.interpolate(&settings.host);
        let username = self.variables.interpolate(&settings.username);
        let password = self.variables.interpolate(&settings.password);
        let _command = self.variables.interpolate(&settings.command);
        let timeout = std::time::Duration::from_millis(settings.timeout_ms);
        let addr = format!("{}:{}", host, settings.port);
        let is_send = settings.action == "SEND_EMAIL";
        let from = self.variables.interpolate(if settings.from.is_empty() {
            &settings.username
        } else {
            &settings.from
        });
        let to = self.variables.interpolate(&settings.to);
        let subject = self.variables.interpolate(&settings.subject);
        let body_tpl = self.variables.interpolate(&settings.body_template);

        let stream = tokio::time::timeout(timeout, tokio::net::TcpStream::connect(&addr))
            .await
            .map_err(|_| {
                crate::error::AppError::Pipeline(format!("SMTP connect timeout: {}", addr))
            })?
            .map_err(|e| {
                crate::error::AppError::Pipeline(format!("SMTP connect failed {}: {}", addr, e))
            })?;

        let mut transcript = String::new();
        let mut auth_ok = false;

        // Helper closure for reading SMTP multi-line responses
        macro_rules! smtp_read {
            ($reader:expr, $transcript:expr) => {{
                let mut last_code: u16 = 0;
                loop {
                    let mut line = String::new();
                    match tokio::time::timeout(
                        std::time::Duration::from_secs(5),
                        $reader.read_line(&mut line),
                    )
                    .await
                    {
                        Ok(Ok(0)) | Err(_) | Ok(Err(_)) => break,
                        Ok(Ok(_)) => {
                            $transcript.push_str(&format!("S: {}", line));
                            if let Ok(c) = line.get(..3).unwrap_or("").parse::<u16>() {
                                last_code = c;
                            }
                            if line.len() >= 4 && line.as_bytes()[3] != b'-' {
                                break;
                            }
                        }
                    }
                }
                last_code
            }};
        }

        if settings.use_tls {
            let tls_config = {
                let mut cfg = rustls::ClientConfig::builder()
                    .with_root_certificates({
                        let mut roots = rustls::RootCertStore::empty();
                        roots.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
                        roots
                    })
                    .with_no_client_auth();
                if !settings.ssl_verify {
                    cfg.dangerous().set_certificate_verifier(Arc::new(
                        crate::pipeline::engine::protocol::NoVerify,
                    ));
                }
                Arc::new(cfg)
            };
            let connector = tokio_rustls::TlsConnector::from(tls_config);
            let sn = rustls::pki_types::ServerName::try_from(host.as_str())
                .map_err(|e| crate::error::AppError::Pipeline(format!("Invalid hostname: {}", e)))?
                .to_owned();
            let tls = connector
                .connect(sn, stream)
                .await
                .map_err(|e| crate::error::AppError::Pipeline(format!("TLS handshake: {}", e)))?;
            let (reader, writer) = tokio::io::split(tls);
            let mut reader = tokio::io::BufReader::new(reader);
            let mut writer = writer;

            smtp_read!(reader, transcript);

            // EHLO
            writer
                .write_all(format!("EHLO ironbullet\r\n").as_bytes())
                .await
                .ok();
            writer.flush().await.ok();
            transcript.push_str("C: EHLO ironbullet\r\n");
            smtp_read!(reader, transcript);

            // AUTH LOGIN
            if !username.is_empty() {
                use base64::Engine;
                writer.write_all(b"AUTH LOGIN\r\n").await.ok();
                writer.flush().await.ok();
                transcript.push_str("C: AUTH LOGIN\r\n");
                smtp_read!(reader, transcript);

                let b64_user =
                    base64::engine::general_purpose::STANDARD.encode(username.as_bytes());
                writer
                    .write_all(format!("{}\r\n", b64_user).as_bytes())
                    .await
                    .ok();
                writer.flush().await.ok();
                transcript.push_str(&format!("C: {}\r\n", b64_user));
                smtp_read!(reader, transcript);

                let b64_pass =
                    base64::engine::general_purpose::STANDARD.encode(password.as_bytes());
                writer
                    .write_all(format!("{}\r\n", b64_pass).as_bytes())
                    .await
                    .ok();
                writer.flush().await.ok();
                transcript.push_str("C: ****\r\n");
                let code = smtp_read!(reader, transcript);
                auth_ok = code == 235;
            }

            // SEND_EMAIL or VERIFY
            if is_send && (auth_ok || username.is_empty()) {
                let mail_from = if from.is_empty() {
                    username.clone()
                } else {
                    from.clone()
                };
                writer
                    .write_all(format!("MAIL FROM:<{}>\r\n", mail_from).as_bytes())
                    .await
                    .ok();
                writer.flush().await.ok();
                transcript.push_str(&format!("C: MAIL FROM:<{}>\r\n", mail_from));
                smtp_read!(reader, transcript);

                for rcpt in to.split(',').map(|s| s.trim()).filter(|s| !s.is_empty()) {
                    writer
                        .write_all(format!("RCPT TO:<{}>\r\n", rcpt).as_bytes())
                        .await
                        .ok();
                    writer.flush().await.ok();
                    transcript.push_str(&format!("C: RCPT TO:<{}>\r\n", rcpt));
                    smtp_read!(reader, transcript);
                }

                writer.write_all(b"DATA\r\n").await.ok();
                writer.flush().await.ok();
                transcript.push_str("C: DATA\r\n");
                smtp_read!(reader, transcript);

                let mail_body = format!(
                    "From: {}\r\nTo: {}\r\nSubject: {}\r\nMIME-Version: 1.0\r\nContent-Type: text/plain; charset=utf-8\r\n\r\n{}\r\n.\r\n",
                    mail_from, to, subject, body_tpl
                );
                writer.write_all(mail_body.as_bytes()).await.ok();
                writer.flush().await.ok();
                transcript.push_str(&format!("C: [email {} bytes]\r\n", mail_body.len()));
                smtp_read!(reader, transcript);
            }

            writer.write_all(b"QUIT\r\n").await.ok();
            writer.flush().await.ok();
        } else {
            let (reader, writer) = tokio::io::split(stream);
            let mut reader = tokio::io::BufReader::new(reader);
            let mut writer = writer;

            smtp_read!(reader, transcript);

            writer.write_all(b"EHLO ironbullet\r\n").await.ok();
            writer.flush().await.ok();
            transcript.push_str("C: EHLO ironbullet\r\n");
            smtp_read!(reader, transcript);

            if !username.is_empty() {
                use base64::Engine;
                writer.write_all(b"AUTH LOGIN\r\n").await.ok();
                writer.flush().await.ok();
                transcript.push_str("C: AUTH LOGIN\r\n");
                smtp_read!(reader, transcript);

                let b64_user =
                    base64::engine::general_purpose::STANDARD.encode(username.as_bytes());
                writer
                    .write_all(format!("{}\r\n", b64_user).as_bytes())
                    .await
                    .ok();
                writer.flush().await.ok();
                transcript.push_str(&format!("C: {}\r\n", b64_user));
                smtp_read!(reader, transcript);

                let b64_pass =
                    base64::engine::general_purpose::STANDARD.encode(password.as_bytes());
                writer
                    .write_all(format!("{}\r\n", b64_pass).as_bytes())
                    .await
                    .ok();
                writer.flush().await.ok();
                transcript.push_str("C: ****\r\n");
                let code = smtp_read!(reader, transcript);
                auth_ok = code == 235;
            }

            // SEND_EMAIL or VERIFY
            if is_send && (auth_ok || username.is_empty()) {
                let mail_from = if from.is_empty() {
                    username.clone()
                } else {
                    from.clone()
                };
                writer
                    .write_all(format!("MAIL FROM:<{}>\r\n", mail_from).as_bytes())
                    .await
                    .ok();
                writer.flush().await.ok();
                transcript.push_str(&format!("C: MAIL FROM:<{}>\r\n", mail_from));
                smtp_read!(reader, transcript);

                for rcpt in to.split(',').map(|s| s.trim()).filter(|s| !s.is_empty()) {
                    writer
                        .write_all(format!("RCPT TO:<{}>\r\n", rcpt).as_bytes())
                        .await
                        .ok();
                    writer.flush().await.ok();
                    transcript.push_str(&format!("C: RCPT TO:<{}>\r\n", rcpt));
                    smtp_read!(reader, transcript);
                }

                writer.write_all(b"DATA\r\n").await.ok();
                writer.flush().await.ok();
                transcript.push_str("C: DATA\r\n");
                smtp_read!(reader, transcript);

                let mail_body = format!(
                    "From: {}\r\nTo: {}\r\nSubject: {}\r\nMIME-Version: 1.0\r\nContent-Type: text/plain; charset=utf-8\r\n\r\n{}\r\n.\r\n",
                    mail_from, to, subject, body_tpl
                );
                writer.write_all(mail_body.as_bytes()).await.ok();
                writer.flush().await.ok();
                transcript.push_str(&format!("C: [email {} bytes]\r\n", mail_body.len()));
                smtp_read!(reader, transcript);
            }

            writer.write_all(b"QUIT\r\n").await.ok();
            writer.flush().await.ok();
        }

        self.variables
            .set_user(&settings.output_var, transcript.clone(), settings.capture);

        if let Some(last) = self.block_results.last_mut() {
            last.request = Some(RequestInfo {
                method: if is_send {
                    "SMTP/SEND".into()
                } else {
                    "SMTP/VERIFY".into()
                },
                url: addr,
                headers: vec![],
                body: if is_send {
                    format!("FROM:{} TO:{} SUBJECT:{}", from, to, subject)
                } else {
                    format!("AUTH {} ****", username)
                },
            });
            last.response = Some(ResponseInfo {
                status_code: if auth_ok { 235 } else { 535 },
                headers: std::collections::HashMap::new(),
                body: transcript,
                final_url: String::new(),
                cookies: std::collections::HashMap::new(),
                timing_ms: 0,
            });
        }

        Ok(())
    }

    // ── POP3 Request ──

    pub(super) async fn execute_pop_request(
        &mut self,
        _block: &Block,
        settings: &PopRequestSettings,
    ) -> crate::error::Result<()> {
        use tokio::io::{AsyncBufReadExt, AsyncWriteExt};

        let host = self.variables.interpolate(&settings.host);
        let username = self.variables.interpolate(&settings.username);
        let password = self.variables.interpolate(&settings.password);
        let cmd_base = self.variables.interpolate(&settings.command);
        let timeout = std::time::Duration::from_millis(settings.timeout_ms);
        let addr = format!("{}:{}", host, settings.port);

        // For RETR/DELE, append the message number
        let command = if matches!(cmd_base.as_str(), "RETR" | "DELE") {
            format!("{} {}", cmd_base, settings.message_num)
        } else {
            cmd_base.clone()
        };

        let stream = tokio::time::timeout(timeout, tokio::net::TcpStream::connect(&addr))
            .await
            .map_err(|_| {
                crate::error::AppError::Pipeline(format!("POP3 connect timeout: {}", addr))
            })?
            .map_err(|e| {
                crate::error::AppError::Pipeline(format!("POP3 connect failed {}: {}", addr, e))
            })?;

        let mut transcript = String::new();
        #[allow(unused_assignments)]
        let mut auth_ok = false;

        macro_rules! pop_read_line {
            ($reader:expr, $transcript:expr) => {{
                let mut line = String::new();
                match tokio::time::timeout(
                    std::time::Duration::from_secs(5),
                    $reader.read_line(&mut line),
                )
                .await
                {
                    Ok(Ok(n)) if n > 0 => {
                        $transcript.push_str(&format!("S: {}", line));
                        line.starts_with("+OK")
                    }
                    _ => false,
                }
            }};
        }

        if settings.use_tls {
            let tls_config = {
                let mut cfg = rustls::ClientConfig::builder()
                    .with_root_certificates({
                        let mut roots = rustls::RootCertStore::empty();
                        roots.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
                        roots
                    })
                    .with_no_client_auth();
                if !settings.ssl_verify {
                    cfg.dangerous().set_certificate_verifier(Arc::new(
                        crate::pipeline::engine::protocol::NoVerify,
                    ));
                }
                Arc::new(cfg)
            };
            let connector = tokio_rustls::TlsConnector::from(tls_config);
            let sn = rustls::pki_types::ServerName::try_from(host.as_str())
                .map_err(|e| crate::error::AppError::Pipeline(format!("Invalid hostname: {}", e)))?
                .to_owned();
            let tls = connector
                .connect(sn, stream)
                .await
                .map_err(|e| crate::error::AppError::Pipeline(format!("TLS handshake: {}", e)))?;
            let (reader, writer) = tokio::io::split(tls);
            let mut reader = tokio::io::BufReader::new(reader);
            let mut writer = writer;

            pop_read_line!(reader, transcript); // Banner

            writer
                .write_all(format!("USER {}\r\n", username).as_bytes())
                .await
                .ok();
            writer.flush().await.ok();
            transcript.push_str(&format!("C: USER {}\r\n", username));
            pop_read_line!(reader, transcript);

            writer
                .write_all(format!("PASS {}\r\n", password).as_bytes())
                .await
                .ok();
            writer.flush().await.ok();
            transcript.push_str("C: PASS ****\r\n");
            auth_ok = pop_read_line!(reader, transcript);

            if auth_ok && !command.is_empty() && command != "STAT" || command == "STAT" {
                writer
                    .write_all(format!("{}\r\n", command).as_bytes())
                    .await
                    .ok();
                writer.flush().await.ok();
                transcript.push_str(&format!("C: {}\r\n", command));
                pop_read_line!(reader, transcript);
            }

            writer.write_all(b"QUIT\r\n").await.ok();
            writer.flush().await.ok();
        } else {
            let (reader, writer) = tokio::io::split(stream);
            let mut reader = tokio::io::BufReader::new(reader);
            let mut writer = writer;

            pop_read_line!(reader, transcript);

            writer
                .write_all(format!("USER {}\r\n", username).as_bytes())
                .await
                .ok();
            writer.flush().await.ok();
            transcript.push_str(&format!("C: USER {}\r\n", username));
            pop_read_line!(reader, transcript);

            writer
                .write_all(format!("PASS {}\r\n", password).as_bytes())
                .await
                .ok();
            writer.flush().await.ok();
            transcript.push_str("C: PASS ****\r\n");
            auth_ok = pop_read_line!(reader, transcript);

            if auth_ok && !command.is_empty() {
                writer
                    .write_all(format!("{}\r\n", command).as_bytes())
                    .await
                    .ok();
                writer.flush().await.ok();
                transcript.push_str(&format!("C: {}\r\n", command));
                pop_read_line!(reader, transcript);
            }

            writer.write_all(b"QUIT\r\n").await.ok();
            writer.flush().await.ok();
        }

        self.variables
            .set_user(&settings.output_var, transcript.clone(), settings.capture);

        if let Some(last) = self.block_results.last_mut() {
            last.request = Some(RequestInfo {
                method: "POP3".into(),
                url: addr,
                headers: vec![],
                body: format!("USER {} PASS ****", username),
            });
            last.response = Some(ResponseInfo {
                status_code: if auth_ok { 200 } else { 401 },
                headers: std::collections::HashMap::new(),
                body: transcript,
                final_url: String::new(),
                cookies: std::collections::HashMap::new(),
                timing_ms: 0,
            });
        }

        Ok(())
    }
}

/// Rustls verifier that skips certificate verification (used when ssl_verify=false).
#[derive(Debug)]
pub struct NoVerify;

impl rustls::client::danger::ServerCertVerifier for NoVerify {
    fn verify_server_cert(
        &self,
        _: &rustls::pki_types::CertificateDer,
        _: &[rustls::pki_types::CertificateDer],
        _: &rustls::pki_types::ServerName,
        _: &[u8],
        _: rustls::pki_types::UnixTime,
    ) -> Result<rustls::client::danger::ServerCertVerified, rustls::Error> {
        Ok(rustls::client::danger::ServerCertVerified::assertion())
    }
    fn verify_tls12_signature(
        &self,
        _: &[u8],
        _: &rustls::pki_types::CertificateDer,
        _: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
    }
    fn verify_tls13_signature(
        &self,
        _: &[u8],
        _: &rustls::pki_types::CertificateDer,
        _: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
    }
    fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
        vec![
            rustls::SignatureScheme::RSA_PKCS1_SHA256,
            rustls::SignatureScheme::RSA_PKCS1_SHA384,
            rustls::SignatureScheme::RSA_PKCS1_SHA512,
            rustls::SignatureScheme::ECDSA_NISTP256_SHA256,
            rustls::SignatureScheme::ECDSA_NISTP384_SHA384,
            rustls::SignatureScheme::RSA_PSS_SHA256,
            rustls::SignatureScheme::RSA_PSS_SHA384,
            rustls::SignatureScheme::RSA_PSS_SHA512,
            rustls::SignatureScheme::ED25519,
        ]
    }
}
