// Temporary debug test module
#[cfg(test)]
mod tests {
    use crate::export::rust_codegen;
    use crate::pipeline::block::*;
    use crate::pipeline::Pipeline;

    /// Test: Simulate adding blocks one by one and generating code each time
    /// This mimics exactly what the frontend does when user adds blocks
    #[test]
    fn test_incremental_block_addition_codegen() {
        let mut pipeline = Pipeline::default();
        println!("=== Incremental block addition test ===\n");

        // Empty pipeline
        let code = rust_codegen::generate_rust_code(&pipeline);
        println!("0 blocks: {} chars", code.len());
        assert!(code.contains("async fn main"), "Should have main function");

        // Add HTTP Request
        pipeline.blocks.push(Block::new(BlockType::HttpRequest));
        let code = rust_codegen::generate_rust_code(&pipeline);
        println!("1 block (HTTP): {} chars", code.len());
        assert!(code.contains("client.get"), "Should have HTTP request code");

        // Add ParseLR
        pipeline.blocks.push(Block::new(BlockType::ParseLR));
        let code = rust_codegen::generate_rust_code(&pipeline);
        println!("2 blocks (+ParseLR): {} chars", code.len());
        assert!(code.contains("find(left)"), "Should have ParseLR code");

        // Add TcpRequest
        pipeline.blocks.push(Block::new(BlockType::TcpRequest));
        let code = rust_codegen::generate_rust_code(&pipeline);
        println!("3 blocks (+TCP): {} chars", code.len());
        assert!(code.contains("TcpStream"), "Should have TCP code");

        // Add CaptchaSolver
        pipeline.blocks.push(Block::new(BlockType::CaptchaSolver));
        let code = rust_codegen::generate_rust_code(&pipeline);
        println!("4 blocks (+Captcha): {} chars", code.len());
        assert!(
            code.contains("capsolver") || code.contains("createTask"),
            "Should have captcha code"
        );

        // Add CloudflareBypass
        pipeline
            .blocks
            .push(Block::new(BlockType::CloudflareBypass));
        let code = rust_codegen::generate_rust_code(&pipeline);
        println!("5 blocks (+CF Bypass): {} chars", code.len());
        assert!(
            code.contains("FlareSolverr") || code.contains("flaresolverr"),
            "Should have CF bypass code"
        );

        // Add LaravelCsrf
        pipeline.blocks.push(Block::new(BlockType::LaravelCsrf));
        let code = rust_codegen::generate_rust_code(&pipeline);
        println!("6 blocks (+Laravel): {} chars", code.len());
        assert!(
            code.contains("csrf") || code.contains("CSRF"),
            "Should have CSRF code"
        );

        // Add KeyCheck
        pipeline.blocks.push(Block::new(BlockType::KeyCheck));
        let code = rust_codegen::generate_rust_code(&pipeline);
        println!("7 blocks (+KeyCheck): {} chars", code.len());

        println!("\n=== Final generated code ===");
        println!("{}", code);
    }

    /// Test: Simulate frontend JSON exactly as it would arrive via IPC
    /// with the _tab_id field and verify full roundtrip
    #[test]
    fn test_full_ipc_roundtrip_simulation() {
        println!("=== Full IPC roundtrip simulation ===\n");

        // Step 1: Create a pipeline from Rust (like add_block does)
        let mut pipeline = Pipeline::default();
        pipeline.blocks.push(Block::new(BlockType::HttpRequest));
        pipeline.blocks.push(Block::new(BlockType::FtpRequest));
        pipeline.blocks.push(Block::new(BlockType::CaptchaSolver));
        pipeline
            .blocks
            .push(Block::new(BlockType::CloudflareBypass));

        // Step 2: Serialize the pipeline (like frontend would receive from pipeline_loaded)
        let pipeline_json = serde_json::to_value(&pipeline).unwrap();
        println!("Serialized pipeline: {} blocks", pipeline.blocks.len());

        // Step 3: Wrap in IPC data (like frontend's send() function does)
        let ipc_data = serde_json::json!({
            "pipeline": pipeline_json,
            "_tab_id": "test-tab-123"
        });

        // Step 4: Extract and deserialize (like the generate_code handler does)
        let p = ipc_data.get("pipeline").unwrap();
        let deserialized = serde_json::from_value::<Pipeline>(p.clone());
        match &deserialized {
            Ok(p) => {
                println!("Deserialized OK: {} blocks", p.blocks.len());
                for (i, b) in p.blocks.iter().enumerate() {
                    println!("  block[{}]: {:?} '{}'", i, b.block_type, b.label);
                }
            }
            Err(e) => {
                panic!("Deserialization FAILED: {}", e);
            }
        }

        // Step 5: Generate code
        let code = rust_codegen::generate_rust_code(&deserialized.unwrap());
        println!("\nGenerated {} chars of code", code.len());
        println!("{}", code);

        // Verify all block types are represented
        assert!(
            code.contains("client.get"),
            "HTTP Request should generate wreq code"
        );
        assert!(
            code.contains("TcpStream::connect") || code.contains("FTP"),
            "FTP should generate code"
        );
        assert!(
            code.contains("capsolver") || code.contains("createTask"),
            "Captcha should generate code"
        );
        assert!(
            code.contains("FlareSolverr") || code.contains("flaresolverr"),
            "CF should generate code"
        );
    }

    /// Test: Verify Block serialization format matches frontend expectations
    #[test]
    fn test_block_serialization_matches_frontend() {
        println!("=== Block serialization format check ===\n");

        // Create blocks from Rust side (like add_block handler does)
        let test_blocks = vec![
            BlockType::HttpRequest,
            BlockType::TcpRequest,
            BlockType::UdpRequest,
            BlockType::FtpRequest,
            BlockType::SshRequest,
            BlockType::ImapRequest,
            BlockType::SmtpRequest,
            BlockType::PopRequest,
            BlockType::CaptchaSolver,
            BlockType::CloudflareBypass,
            BlockType::LaravelCsrf,
            BlockType::KeyCheck,
            BlockType::ParseLR,
            BlockType::ParseRegex,
            BlockType::ParseJSON,
            BlockType::ParseCSS,
            BlockType::ParseXPath,
            BlockType::StringFunction,
            BlockType::CryptoFunction,
            BlockType::DateFunction,
            BlockType::RandomData,
            BlockType::Group,
        ];

        for bt in test_blocks {
            let block = Block::new(bt);
            let json = serde_json::to_value(&block).unwrap();

            // Check that the settings have a "type" field (serde tag)
            let settings = json.get("settings").unwrap();
            let type_tag = settings.get("type").and_then(|v| v.as_str());
            println!("{:?}: type tag = {:?}", bt, type_tag);
            assert!(type_tag.is_some(), "{:?} settings missing 'type' tag", bt);

            // The type tag should match the enum variant name
            let expected_tag = format!("{:?}", bt);
            assert_eq!(
                type_tag.unwrap(),
                expected_tag,
                "{:?} type tag mismatch: got '{}', expected '{}'",
                bt,
                type_tag.unwrap(),
                expected_tag
            );

            // Verify the block roundtrips through JSON
            let json_str = serde_json::to_string(&block).unwrap();
            let back: Result<Block, _> = serde_json::from_str(&json_str);
            assert!(
                back.is_ok(),
                "{:?} failed roundtrip: {}",
                bt,
                back.unwrap_err()
            );
        }
    }

    /// Test: Simulate pipeline with frontend-style UUIDs (crypto.randomUUID format)
    #[test]
    fn test_pipeline_with_crypto_random_uuid() {
        // crypto.randomUUID() produces lowercase UUIDs like:
        // "3b241101-e2bb-4d7a-8613-e4d4e2e57e1c"
        let json = r#"{
            "id": "3b241101-e2bb-4d7a-8613-e4d4e2e57e1c",
            "name": "Test Config",
            "author": "",
            "created": "2026-02-16T00:00:00.000Z",
            "modified": "2026-02-16T00:00:00.000Z",
            "blocks": [{
                "id": "7c9e6679-7425-40de-944b-e07fc1f90ae7",
                "block_type": "HttpRequest",
                "label": "HTTP Request",
                "disabled": false,
                "safe_mode": false,
                "settings": {
                    "type": "HttpRequest",
                    "method": "GET",
                    "url": "https://example.com",
                    "headers": [["User-Agent", "Mozilla/5.0"]],
                    "body": "",
                    "body_type": "None",
                    "content_type": "",
                    "follow_redirects": true,
                    "max_redirects": 5,
                    "timeout_ms": 10000,
                    "auto_redirect": true,
                    "basic_auth": null,
                    "http_version": "2",
                    "response_var": "SOURCE",
                    "custom_cookies": ""
                }
            },{
                "id": "a1b2c3d4-e5f6-7890-abcd-ef1234567890",
                "block_type": "CloudflareBypass",
                "label": "Cloudflare Bypass",
                "disabled": false,
                "safe_mode": false,
                "settings": {
                    "type": "CloudflareBypass",
                    "url": "https://target.com",
                    "flaresolverr_url": "http://localhost:8191/v1",
                    "max_timeout_ms": 60000,
                    "output_var": "CF_COOKIES",
                    "capture": false
                }
            }],
            "startup_blocks": [],
            "data_settings": {"wordlist_type": "Credentials", "separator": ":", "slices": ["USER","PASS"]},
            "proxy_settings": {"proxy_mode": "None", "proxy_sources": [], "ban_duration_secs": 300, "max_retries_before_ban": 3, "cpm_per_proxy": 0, "proxy_groups": [], "active_group": ""},
            "browser_settings": {"browser": "chrome", "ja3": null, "http2_fingerprint": null, "user_agent": null},
            "runner_settings": {"threads": 100, "skip": 0, "take": 0, "continue_statuses": ["Retry"], "custom_status_name": "CUSTOM", "max_retries": 3, "concurrent_per_proxy": 0, "start_threads_gradually": true, "gradual_delay_ms": 100, "automatic_thread_count": false, "lower_threads_on_retry": false, "retry_thread_reduction_pct": 25, "pause_on_ratelimit": false, "only_proxyless": false},
            "output_settings": {"save_to_file": true, "save_to_database": false, "include_response": false, "output_directory": "results", "output_format": "{data} | {captures}", "database_path": "", "output_format_type": "Txt", "capture_filters": []}
        }"#;

        let result: Result<Pipeline, _> = serde_json::from_str(json);
        match &result {
            Ok(p) => {
                println!(
                    "Pipeline with crypto.randomUUID format: {} blocks",
                    p.blocks.len()
                );
                let code = rust_codegen::generate_rust_code(p);
                println!("Generated {} chars:\n{}", code.len(), code);
                assert!(code.contains("client.get"), "HTTP should produce code");
                assert!(
                    code.contains("flaresolverr") || code.contains("FlareSolverr"),
                    "CF bypass should produce code"
                );
            }
            Err(e) => panic!("FAIL: {}", e),
        }
    }

    /// Test: What happens with an empty blocks pipeline (this is the default state)
    #[test]
    fn test_empty_pipeline_generates_boilerplate() {
        let pipeline = Pipeline::default();
        let code = rust_codegen::generate_rust_code(&pipeline);
        println!("Empty pipeline code ({} chars):\n{}", code.len(), code);
        assert!(code.contains("async fn main"), "Should have main function");
        assert!(code.contains("Ok(())"), "Should have Ok(())");
        assert!(code.len() > 100, "Should have at least boilerplate code");
    }

    /// Test: KeyCheck stop_on_fail — validates early-exit optimization
    ///
    /// Pipeline:
    ///   SetVariable("MARKER1", "set")   ← must always run
    ///   KeyCheck(STATUS == "FAIL" → Fail, stop_on_fail=true)
    ///   SetVariable("MARKER2", "set")   ← must NOT run when stop_on_fail=true & status=Fail
    ///
    /// We run this twice:
    ///  (A) stop_on_fail=true, STATUS="FAIL"  → MARKER2 should be unset
    ///  (B) stop_on_fail=false, STATUS="FAIL" → MARKER2 should be set (old behaviour)
    ///  (C) stop_on_fail=true, STATUS="OK"    → MARKER2 should be set (Success path, no early exit)
    #[tokio::test]
    async fn test_keycheck_stop_on_fail() {
        // All block types are re-exported via `pub use settings_*::*` in block/mod.rs
        use crate::pipeline::block::*;
        use crate::pipeline::engine::ExecutionContext;
        use crate::pipeline::BotStatus;
        use crate::sidecar::native::create_native_backend;
        use uuid::Uuid;

        let sidecar_tx = create_native_backend();

        // Helper: build a 3-block pipeline
        let make_pipeline = |stop_on_fail: bool| -> Vec<Block> {
            // Block 1: SetVariable — set STATUS = "FAIL" (pipeline input)
            let mut b1 = Block::new(BlockType::SetVariable);
            b1.settings = BlockSettings::SetVariable(SetVariableSettings {
                name: "STATUS".into(),
                value: "FAIL".into(),
                capture: false,
            });

            // Block 2: KeyCheck — STATUS == "FAIL" → Fail
            let mut b2 = Block::new(BlockType::KeyCheck);
            b2.settings = BlockSettings::KeyCheck(KeyCheckSettings {
                keychains: vec![Keychain {
                    result: BotStatus::Fail,
                    conditions: vec![KeyCondition {
                        source: "STATUS".into(),
                        comparison: Comparison::EqualTo,
                        value: "FAIL".into(),
                    }],
                    mode: KeychainMode::And,
                }],
                stop_on_fail,
            });

            // Block 3: SetVariable("MARKER2", "set") — should be skipped when stop_on_fail
            let mut b3 = Block::new(BlockType::SetVariable);
            b3.settings = BlockSettings::SetVariable(SetVariableSettings {
                name: "MARKER2".into(),
                value: "set".into(),
                capture: false,
            });

            vec![b1, b2, b3]
        };

        // ── Case A: stop_on_fail=true, STATUS="FAIL" ─────────────────────────
        {
            let blocks = make_pipeline(true);
            let mut ctx = ExecutionContext::new(Uuid::new_v4().to_string());
            ctx.execute_blocks(&blocks, &sidecar_tx).await.unwrap();

            assert_eq!(ctx.status, BotStatus::Fail, "A: status should be Fail");
            assert_eq!(
                ctx.variables.get("STATUS").as_deref(),
                Some("FAIL"),
                "A: STATUS should be set (block ran before KeyCheck)"
            );
            assert_eq!(
                ctx.variables.get("MARKER2"),
                None,
                "A: MARKER2 should NOT be set — stop_on_fail skipped block 3"
            );
            println!(
                "[A] stop_on_fail=true  + Fail → MARKER2={:?} (expected None) ✓",
                ctx.variables.get("MARKER2")
            );
        }

        // ── Case B: stop_on_fail=false, STATUS="FAIL" ────────────────────────
        {
            let blocks = make_pipeline(false);
            let mut ctx = ExecutionContext::new(Uuid::new_v4().to_string());
            ctx.execute_blocks(&blocks, &sidecar_tx).await.unwrap();

            assert_eq!(ctx.status, BotStatus::Fail, "B: status should be Fail");
            assert_eq!(
                ctx.variables.get("MARKER2").as_deref(),
                Some("set"),
                "B: MARKER2 should be set — old behaviour continues after Fail"
            );
            println!(
                "[B] stop_on_fail=false + Fail → MARKER2={:?} (expected Some(\"set\")) ✓",
                ctx.variables.get("MARKER2")
            );
        }

        // ── Case C: stop_on_fail=true, but KeyCheck doesn't fire (STATUS != "FAIL") ──
        // Override STATUS to "OK" so keychain doesn't match → status stays None → block 3 runs
        {
            let mut blocks = make_pipeline(true);
            // Change block 1 to set STATUS = "OK"
            if let BlockSettings::SetVariable(ref mut sv) = blocks[0].settings {
                sv.value = "OK".into();
            }
            // Change keycheck condition to Fail if STATUS == "FAIL" (won't match "OK")
            let mut ctx = ExecutionContext::new(Uuid::new_v4().to_string());
            ctx.execute_blocks(&blocks, &sidecar_tx).await.unwrap();

            // KeyCheck doesn't match → status stays None → block 3 runs
            assert_ne!(ctx.status, BotStatus::Fail, "C: status should not be Fail");
            assert_eq!(
                ctx.variables.get("MARKER2").as_deref(),
                Some("set"),
                "C: MARKER2 should be set — KeyCheck didn't match (no early exit)"
            );
            println!(
                "[C] stop_on_fail=true  + no match → MARKER2={:?} (expected Some(\"set\")) ✓",
                ctx.variables.get("MARKER2")
            );
        }

        println!("\n=== stop_on_fail test: all 3 cases passed ✓ ===");
    }

    /// Trace all variables set by HTTP request to verify KeyCheck sources work
    #[tokio::test]
    async fn test_keycheck_variable_trace() {
        use crate::pipeline::block::*;
        use crate::pipeline::engine::ExecutionContext;
        use crate::pipeline::BotStatus;
        use crate::sidecar::native::create_native_backend;
        use uuid::Uuid;

        let sidecar_tx = create_native_backend();

        // Block 1: HttpRequest to httpbin
        let mut http_block = Block::new(BlockType::HttpRequest);
        if let BlockSettings::HttpRequest(ref mut s) = http_block.settings {
            s.url = "https://httpbin.org/get".into();
            s.method = "GET".into();
            s.tls_client = crate::pipeline::block::TlsClient::RustTLS; // use native backend
        }

        // Block 2: KeyCheck — data.RESPONSECODE == 200 → Success
        let mut kc = Block::new(BlockType::KeyCheck);
        kc.settings = BlockSettings::KeyCheck(KeyCheckSettings {
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

        let blocks = vec![http_block, kc];
        let mut ctx = ExecutionContext::new(Uuid::new_v4().to_string());
        let result = ctx.execute_blocks(&blocks, &sidecar_tx).await;

        println!("\n=== VARIABLE DUMP AFTER HTTP + KEYCHECK ===");
        let snap = ctx.variables.snapshot();
        let mut kv: Vec<_> = snap.iter().collect();
        kv.sort_by_key(|(k, _)| *k);
        for (k, v) in &kv {
            let display = if v.len() > 80 {
                format!("{}...", &v[..80])
            } else {
                v.to_string()
            };
            println!("  {:40} = {}", k, display);
        }
        println!("\nfinal status  : {:?}", ctx.status);
        println!(
            "pipeline result: {:?}",
            result.map(|_| "Ok").map_err(|e| e.to_string())
        );

        // Verify backward-compat variables are present
        assert!(
            snap.contains_key("data.RESPONSECODE"),
            "data.RESPONSECODE missing! Got keys: {:?}",
            snap.keys().collect::<Vec<_>>()
        );
        assert!(snap.contains_key("data.ADDRESS"), "data.ADDRESS missing!");
        assert!(snap.contains_key("data.SOURCE"), "data.SOURCE missing!");

        // Verify KeyCheck classified correctly
        assert_eq!(
            ctx.status,
            BotStatus::Success,
            "KeyCheck data.RESPONSECODE=200 → Success FAILED. data.RESPONSECODE={:?}",
            snap.get("data.RESPONSECODE")
        );

        println!("=== VARIABLE TRACE PASSED ✓ ===");
    }

    /// Test: JSONPath complex path extraction
    /// Verifies the new tokeniser/traversal handles:
    ///   • simple dot notation
    ///   • array indexing [n]
    ///   • array wildcards []
    ///   • nested wildcards (the user-reported case)
    #[test]
    fn test_jsonpath_complex_extraction() {
        use crate::pipeline::engine::parsers::evaluate_json_path;

        let json_str = r#"{
            "status": "ok",
            "user": { "name": "Alice", "age": 30 },
            "items": [
                { "id": 1, "label": "first" },
                { "id": 2, "label": "second" }
            ],
            "data": {
                "servers": [
                    {
                        "region": "us-east",
                        "servers": [
                            { "host": "s1.example.com", "is_trial": true },
                            { "host": "s2.example.com", "is_trial": false }
                        ]
                    },
                    {
                        "region": "eu-west",
                        "servers": [
                            { "host": "s3.example.com", "is_trial": true }
                        ]
                    }
                ]
            }
        }"#;

        let root: serde_json::Value = serde_json::from_str(json_str).unwrap();

        // Simple dot notation
        assert_eq!(evaluate_json_path(&root, "status"), "ok");
        assert_eq!(evaluate_json_path(&root, "user.name"), "Alice");

        // Array indexing
        assert_eq!(evaluate_json_path(&root, "items[0].label"), "first");
        assert_eq!(evaluate_json_path(&root, "items[1].id"), "2");

        // Array wildcard — collect all
        let ids = evaluate_json_path(&root, "items[].id");
        assert_eq!(ids, "1, 2");

        // The user-reported case: nested wildcards
        // data.servers[].servers[].is_trial → true, false, true
        let trials = evaluate_json_path(&root, "data.servers[].servers[].is_trial");
        assert_eq!(
            trials, "true, false, true",
            "nested wildcard extraction failed: got '{}'",
            trials
        );

        // Nested wildcards — host extraction
        let hosts = evaluate_json_path(&root, "data.servers[].servers[].host");
        assert_eq!(hosts, "s1.example.com, s2.example.com, s3.example.com");

        // Collect just first-level field across wildcard
        let regions = evaluate_json_path(&root, "data.servers[].region");
        assert_eq!(regions, "us-east, eu-west");

        println!("=== JSONPATH COMPLEX EXTRACTION PASSED ✓ ===");
    }

    /// Verify that a KeyCheck with result=ToCheck sets status=ToCheck on the context.
    #[tokio::test]
    async fn test_keycheck_to_check_status() {
        use crate::pipeline::block::*;
        use crate::pipeline::engine::ExecutionContext;
        use crate::pipeline::BotStatus;
        use crate::sidecar::native::create_native_backend;
        use uuid::Uuid;

        let sidecar_tx = create_native_backend();

        // Block 1: SetVariable STATUS="AMBIGUOUS"
        let mut b1 = Block::new(BlockType::SetVariable);
        b1.settings = BlockSettings::SetVariable(SetVariableSettings {
            name: "STATUS".into(),
            value: "AMBIGUOUS".into(),
            capture: false,
        });

        // Block 2: KeyCheck — STATUS contains "AMBIGUOUS" → ToCheck
        let mut b2 = Block::new(BlockType::KeyCheck);
        b2.settings = BlockSettings::KeyCheck(KeyCheckSettings {
            keychains: vec![Keychain {
                result: BotStatus::ToCheck,
                conditions: vec![KeyCondition {
                    source: "STATUS".into(),
                    comparison: Comparison::EqualTo,
                    value: "AMBIGUOUS".into(),
                }],
                mode: KeychainMode::And,
            }],
            stop_on_fail: false,
        });

        let blocks = vec![b1, b2];
        let mut ctx = ExecutionContext::new(Uuid::new_v4().to_string());
        ctx.execute_blocks(&blocks, &sidecar_tx).await.unwrap();

        assert_eq!(
            ctx.status,
            BotStatus::ToCheck,
            "KeyCheck result=ToCheck should set context status to ToCheck, got {:?}",
            ctx.status
        );

        println!("=== KEYCHECK TO_CHECK STATUS PASSED ✓ ===");
    }
}
