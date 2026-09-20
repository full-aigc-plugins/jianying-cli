use serde_json::Value;
use std::fs;
use std::path::Path;

#[test]
fn whisper_cpp_protocol_source_is_official_and_bundles_no_runtime_or_model() {
    let manifest_path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../provenance/ASR_PROTOCOL_SOURCES.json");
    let manifest: Value =
        serde_json::from_slice(&fs::read(&manifest_path).expect("应能读取 ASR 协议来源清单"))
            .expect("ASR 协议来源清单必须是 JSON");

    assert_eq!(
        manifest["schema_version"],
        "jianying-asr-protocol-sources/v1"
    );
    assert_eq!(manifest["policy"]["source_copied"], false);
    assert_eq!(manifest["policy"]["network_call_performed"], false);
    assert_eq!(manifest["policy"]["model_download_performed"], false);
    assert_eq!(manifest["policy"]["model_weights_bundled"], false);
    assert_eq!(manifest["policy"]["runtime_binary_bundled"], false);

    let source = &manifest["sources"][0];
    assert_eq!(source["id"], "whisper-cpp-cli");
    assert_eq!(source["project"], "ggml-org/whisper.cpp");
    assert_eq!(source["license"], "MIT");
    assert!(source["documentation"]
        .as_str()
        .unwrap()
        .starts_with("https://github.com/ggml-org/whisper.cpp/"));
    assert!(source["implementation_reference"]
        .as_str()
        .unwrap()
        .starts_with("https://github.com/ggml-org/whisper.cpp/"));
    let flags = source["supported_output_flags"].as_array().unwrap();
    for flag in [
        "--output-json",
        "--output-json-full",
        "--output-srt",
        "--output-txt",
        "--output-vtt",
    ] {
        assert!(flags.iter().any(|candidate| candidate == flag));
    }
}
