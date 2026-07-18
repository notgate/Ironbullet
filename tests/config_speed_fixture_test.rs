use ironbullet::export::format::RfxConfig;
use ironbullet::pipeline::block::{BlockSettings, Comparison, TlsClient};
use ironbullet::pipeline::BotStatus;

#[test]
fn config_speed_fixture_matches_current_rfx_contract() {
    let fixture = RfxConfig::load_from_file("tools/config-speed-test/speed-test.rfx")
        .expect("speed-test.rfx must deserialize");

    assert_eq!(fixture.pipeline.blocks.len(), 2);
    assert_eq!(fixture.pipeline.runner_settings.threads, 100);
    assert!(!fixture.pipeline.runner_settings.start_threads_gradually);
    assert!(!fixture.pipeline.output_settings.save_to_file);

    match &fixture.pipeline.blocks[0].settings {
        BlockSettings::HttpRequest(settings) => {
            assert!(settings.url.contains("__TARGET_URL__"));
            assert!(settings.url.contains("<input.USER>"));
            assert_eq!(settings.tls_client, TlsClient::RustTLS);
        }
        other => panic!("expected HttpRequest benchmark block, got {other:?}"),
    }

    match &fixture.pipeline.blocks[1].settings {
        BlockSettings::KeyCheck(settings) => {
            assert_eq!(settings.keychains.len(), 1);
            assert_eq!(settings.keychains[0].result, BotStatus::Success);
            assert_eq!(settings.keychains[0].conditions.len(), 1);
            assert_eq!(
                settings.keychains[0].conditions[0].source,
                "data.RESPONSECODE"
            );
            assert!(matches!(
                settings.keychains[0].conditions[0].comparison,
                Comparison::EqualTo
            ));
            assert_eq!(settings.keychains[0].conditions[0].value, "200");
        }
        other => panic!("expected KeyCheck benchmark block, got {other:?}"),
    }
}
