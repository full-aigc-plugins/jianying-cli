use serde_json::Value;
use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

#[test]
fn cloud_tts_protocol_sources_cover_every_named_adapter_without_live_calls() {
    let manifest_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../provenance/CLOUD_TTS_PROTOCOL_SOURCES.json");
    let manifest: Value =
        serde_json::from_slice(&fs::read(&manifest_path).expect("应能读取云端 TTS 协议来源清单"))
            .expect("云端 TTS 协议来源清单必须是 JSON");

    assert_eq!(
        manifest["schema_version"],
        "jianying-cloud-tts-protocol-sources/v1"
    );
    assert_eq!(manifest["policy"]["source_copied"], false);
    assert_eq!(manifest["policy"]["network_call_performed"], false);
    assert_eq!(
        manifest["policy"]["paid_canary_requires_separate_approval"],
        true
    );

    let providers = manifest["sources"]
        .as_array()
        .expect("sources 必须是数组")
        .iter()
        .map(|source| {
            assert!(source["documentation"]
                .as_str()
                .expect("documentation 必须存在")
                .starts_with("https://"));
            assert!(!source["protocol"].as_str().unwrap_or_default().is_empty());
            assert!(!source["endpoint"].as_str().unwrap_or_default().is_empty());
            source["id"].as_str().expect("id 必须存在").to_owned()
        })
        .collect::<BTreeSet<_>>();
    assert_eq!(
        providers,
        BTreeSet::from([
            "aliyun-bailian".to_owned(),
            "baidu".to_owned(),
            "minimax".to_owned(),
            "tencent-cloud".to_owned(),
            "volcengine".to_owned(),
            "xiaomi-mimo".to_owned(),
            "zhipu-glm".to_owned(),
        ])
    );
    let volcengine = manifest["sources"]
        .as_array()
        .unwrap()
        .iter()
        .find(|source| source["id"] == "volcengine")
        .unwrap();
    assert_eq!(volcengine["status"], "current-v3");
    assert_eq!(
        volcengine["endpoint"],
        "https://openspeech.bytedance.com/api/v3/tts/unidirectional/sse"
    );
    assert!(volcengine["implementation_reference"]
        .as_str()
        .unwrap()
        .starts_with("https://github.com/bytedance/"));

    let capabilities_path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../provenance/CAPABILITIES.json");
    let capabilities: Value = serde_json::from_slice(
        &fs::read(&capabilities_path).expect("应能读取 capability manifest"),
    )
    .expect("capability manifest 必须是 JSON");
    let tts = capabilities["capabilities"]
        .as_array()
        .unwrap()
        .iter()
        .find(|capability| capability["id"] == "media.tts_provider")
        .unwrap();
    assert_eq!(tts["status"], "partial");
    let evidence = tts["evidence"].as_str().unwrap();
    assert!(evidence.contains("Volcengine uses the current V3 unidirectional SSE protocol"));
    assert!(!evidence.contains("Volcengine V3 remain pending"));
}
