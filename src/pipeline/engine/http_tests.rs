use super::*;
use crate::pipeline::block::*;
use crate::pipeline::BotStatus;
use crate::sidecar::native::create_native_backend;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::mpsc;

fn read_http_request(stream: &mut std::net::TcpStream) -> String {
    let mut request = Vec::new();
    let mut buffer = [0_u8; 1024];
    stream
        .set_read_timeout(Some(std::time::Duration::from_secs(3)))
        .unwrap();

    loop {
        let count = stream.read(&mut buffer).unwrap();
        if count == 0 {
            break;
        }
        request.extend_from_slice(&buffer[..count]);
        let text = String::from_utf8_lossy(&request);
        if let Some(headers_end) = text.find("\r\n\r\n") {
            let content_length = text[..headers_end]
                .lines()
                .find_map(|line| line.strip_prefix("Content-Length: "))
                .and_then(|value| value.trim().parse::<usize>().ok())
                .unwrap_or(0);
            if request.len() >= headers_end + 4 + content_length {
                break;
            }
        }
    }

    String::from_utf8(request).unwrap()
}

#[tokio::test]
async fn local_http_parse_keycheck_pipeline_applies_request_settings() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let (request_tx, request_rx) = mpsc::channel();
    let server = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let request = read_http_request(&mut stream);
        request_tx.send(request).unwrap();
        stream
            .write_all(
                b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 23\r\nConnection: close\r\n\r\n{\"result\":\"ok\",\"id\":42}",
            )
            .unwrap();
    });

    let mut http = Block::new(BlockType::HttpRequest);
    http.settings = BlockSettings::HttpRequest(HttpRequestSettings {
        method: "POST".into(),
        url: format!("http://{address}/fixture"),
        headers: vec![("X-Fixture".into(), "pipeline".into())],
        body: "{\"token\":\"<input.TOKEN>\"}".into(),
        body_type: BodyType::Raw,
        content_type: "application/json".into(),
        follow_redirects: true,
        max_redirects: 3,
        timeout_ms: 3_000,
        auto_redirect: true,
        basic_auth: Some(("fixture-user".into(), "fixture-pass".into())),
        http_version: "HTTP/1.1".into(),
        response_var: "RESPONSE".into(),
        custom_cookies: "fixture_cookie=present".into(),
        ssl_verify: true,
        proxy_insecure: false,
        cipher_suites: String::new(),
        tls_client: TlsClient::RustTLS,
        browser_profile: String::new(),
        ja3_override: String::new(),
        http2fp_override: String::new(),
        wreq_emulation: String::new(),
    });

    let mut parse = Block::new(BlockType::ParseRegex);
    parse.settings = BlockSettings::ParseRegex(ParseRegexSettings {
        input_var: "data.RESPONSE".into(),
        pattern: "\\\"result\\\":\\\"(\\w+)\\\"".into(),
        output_format: "$1".into(),
        output_var: "RESULT".into(),
        capture: true,
        multi_line: false,
    });

    let mut keycheck = Block::new(BlockType::KeyCheck);
    keycheck.settings = BlockSettings::KeyCheck(KeyCheckSettings {
        keychains: vec![Keychain {
            result: BotStatus::Success,
            conditions: vec![KeyCondition {
                source: "RESULT".into(),
                comparison: Comparison::EqualTo,
                value: "ok".into(),
            }],
            mode: KeychainMode::And,
        }],
        stop_on_fail: false,
    });

    let sidecar_tx = create_native_backend();
    let mut ctx = ExecutionContext::new("local-fixture".into());
    ctx.variables.set_input("TOKEN", "interpolated".into());
    ctx.execute_blocks(&[http, parse, keycheck], &sidecar_tx)
        .await
        .unwrap();

    let request = request_rx
        .recv_timeout(std::time::Duration::from_secs(3))
        .unwrap();
    server.join().unwrap();

    assert!(request.starts_with("POST /fixture HTTP/1.1\r\n"));
    let request_headers = request.to_ascii_lowercase();
    assert!(request_headers.contains("x-fixture: pipeline\r\n"));
    assert!(request_headers.contains("content-type: application/json\r\n"));
    assert!(
        request_headers.contains("authorization: basic zml4dhvyzs11c2vyomzpehr1cmutcgfzcw==\r\n")
    );
    assert!(request_headers.contains("cookie: fixture_cookie=present\r\n"));
    assert!(request.ends_with("{\"token\":\"interpolated\"}"));
    assert_eq!(ctx.variables.get("RESULT").as_deref(), Some("ok"));
    assert_eq!(
        ctx.variables.get("data.RESPONSE.STATUS").as_deref(),
        Some("200")
    );
    assert_eq!(ctx.status, BotStatus::Success);
}

#[tokio::test]
async fn legacy_auto_redirect_false_prevents_redirect_following() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let (request_tx, request_rx) = mpsc::channel();
    let server = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        request_tx.send(read_http_request(&mut stream)).unwrap();
        stream
            .write_all(
                b"HTTP/1.1 302 Found\r\nLocation: /final\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
            )
            .unwrap();
    });

    let mut http = Block::new(BlockType::HttpRequest);
    if let BlockSettings::HttpRequest(settings) = &mut http.settings {
        settings.url = format!("http://{address}/redirect");
        settings.tls_client = TlsClient::RustTLS;
        settings.follow_redirects = true;
        settings.auto_redirect = false;
        settings.response_var = "RESPONSE".into();
    }

    let sidecar_tx = create_native_backend();
    let mut ctx = ExecutionContext::new("redirect-fixture".into());
    ctx.execute_blocks(&[http], &sidecar_tx).await.unwrap();
    let request = request_rx
        .recv_timeout(std::time::Duration::from_secs(3))
        .unwrap();
    server.join().unwrap();

    assert!(request.starts_with("GET /redirect HTTP/1.1\r\n"));
    assert_eq!(
        ctx.variables.get("data.RESPONSE.STATUS").as_deref(),
        Some("302")
    );
    assert!(ctx
        .variables
        .get("data.RESPONSE.URL")
        .is_some_and(|url| url.ends_with("/redirect")));
}

#[tokio::test]
async fn script_block_fails_instead_of_succeeding_without_execution() {
    let script = Block::new(BlockType::Script);
    let sidecar_tx = create_native_backend();
    let mut ctx = ExecutionContext::new("script-fixture".into());

    let error = ctx
        .execute_blocks(&[script], &sidecar_tx)
        .await
        .unwrap_err();

    assert!(error.to_string().contains("not supported"));
    assert_eq!(ctx.status, BotStatus::Error);
}

#[tokio::test]
async fn multipart_block_fails_instead_of_sending_an_invalid_raw_body() {
    let mut http = Block::new(BlockType::HttpRequest);
    if let BlockSettings::HttpRequest(settings) = &mut http.settings {
        settings.body_type = BodyType::Multipart;
        settings.body = "file=data".into();
    }
    let sidecar_tx = create_native_backend();
    let mut ctx = ExecutionContext::new("multipart-fixture".into());

    let error = ctx.execute_blocks(&[http], &sidecar_tx).await.unwrap_err();

    assert!(error.to_string().contains("Multipart HTTP bodies"));
    assert_eq!(ctx.status, BotStatus::Error);
}

#[tokio::test]
async fn safe_mode_http_failure_clears_prior_response_before_keycheck() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let _ = read_http_request(&mut stream);
        stream
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok")
            .unwrap();
    });
    let failed_address = {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        drop(listener);
        address
    };

    let mut successful_request = Block::new(BlockType::HttpRequest);
    if let BlockSettings::HttpRequest(settings) = &mut successful_request.settings {
        settings.url = format!("http://{address}/first");
        settings.tls_client = TlsClient::RustTLS;
        settings.response_var = "RESPONSE".into();
    }
    let mut failed_request = Block::new(BlockType::HttpRequest);
    failed_request.safe_mode = true;
    if let BlockSettings::HttpRequest(settings) = &mut failed_request.settings {
        settings.url = format!("http://{failed_address}/closed");
        settings.tls_client = TlsClient::RustTLS;
        settings.timeout_ms = 500;
        settings.response_var = "RESPONSE".into();
    }
    let mut stale_status_check = Block::new(BlockType::KeyCheck);
    stale_status_check.settings = BlockSettings::KeyCheck(KeyCheckSettings {
        keychains: vec![Keychain {
            result: BotStatus::Success,
            conditions: vec![KeyCondition {
                source: "data.RESPONSECODE".into(),
                comparison: Comparison::EqualTo,
                value: "200".into(),
            }],
            mode: KeychainMode::And,
        }],
        stop_on_fail: false,
    });

    let sidecar_tx = create_native_backend();
    let mut ctx = ExecutionContext::new("safe-mode-fixture".into());
    ctx.execute_blocks(
        &[successful_request, failed_request, stale_status_check],
        &sidecar_tx,
    )
    .await
    .unwrap();
    server.join().unwrap();

    assert_eq!(ctx.variables.get("data.RESPONSE"), None);
    assert_eq!(ctx.variables.get("data.RESPONSE.STATUS"), None);
    assert_eq!(ctx.variables.get("data.RESPONSECODE").as_deref(), Some(""));
    assert!(ctx
        .variables
        .get("data.RESPONSE.ERROR")
        .is_some_and(|error| !error.is_empty()));
    assert_eq!(ctx.status, BotStatus::None);
}

#[test]
fn empty_and_keychain_does_not_classify_every_input() {
    let mut ctx = ExecutionContext::new("keycheck-fixture".into());
    ctx.execute_keycheck(&KeyCheckSettings {
        keychains: vec![Keychain {
            result: BotStatus::Success,
            conditions: Vec::new(),
            mode: KeychainMode::And,
        }],
        stop_on_fail: false,
    })
    .unwrap();

    assert_eq!(ctx.status, BotStatus::None);
}

#[test]
fn invalid_numeric_keycheck_values_do_not_coerce_to_zero() {
    let mut ctx = ExecutionContext::new("keycheck-fixture".into());
    ctx.variables
        .set_user("VALUE", "not-a-number".into(), false);

    assert!(!ctx.evaluate_condition(&KeyCondition {
        source: "VALUE".into(),
        comparison: Comparison::LessThan,
        value: "1".into(),
    }));
    assert!(!ctx.evaluate_condition(&KeyCondition {
        source: "MISSING".into(),
        comparison: Comparison::GreaterThan,
        value: "-1".into(),
    }));
}
