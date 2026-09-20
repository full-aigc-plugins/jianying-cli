use jianying_media::{
    CommercialUsePolicy, LocalTtsRuntimeKind, LocalTtsRuntimeRegistry, LocalTtsTransport, TtsError,
};

#[test]
fn registers_named_local_open_source_runtimes_without_bundling_weights() {
    let registry = LocalTtsRuntimeRegistry::built_in();

    for runtime in [
        LocalTtsRuntimeKind::Qwen3Tts,
        LocalTtsRuntimeKind::CosyVoice,
        LocalTtsRuntimeKind::GptSovits,
        LocalTtsRuntimeKind::MeloTts,
        LocalTtsRuntimeKind::EmotiVoice,
    ] {
        let profile = registry.find(runtime).expect("应注册本地开源 TTS 运行时");
        assert!(!profile.bundles_model_weights());
        assert_eq!(
            profile.commercial_use_policy(),
            CommercialUsePolicy::AllowedAfterArtifactReview
        );
        assert!(profile
            .upstream_repository()
            .starts_with("https://github.com/"));
    }
}

#[test]
fn keeps_copyleft_mars5_as_an_external_process_boundary() {
    let registry = LocalTtsRuntimeRegistry::built_in();
    let profile = registry
        .find(LocalTtsRuntimeKind::Mars5Tts)
        .expect("应注册 MARS5-TTS 来源档案");

    assert_eq!(profile.code_license(), "AGPL-3.0-only");
    assert_eq!(
        profile.commercial_use_policy(),
        CommercialUsePolicy::CopyleftComplianceRequired
    );
    assert_eq!(profile.transport(), LocalTtsTransport::Process);
    assert_eq!(profile.default_endpoint(), None);
    assert!(!profile.bundles_model_weights());
}

#[test]
fn java_audio_research_sources_are_classified_without_copying_unlicensed_code() {
    let manifest_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../provenance/AUDIO_JAVA_RESEARCH_SOURCES.json");
    let manifest: serde_json::Value = serde_json::from_slice(
        &std::fs::read(manifest_path).expect("应存在 Java 音频研究来源清单"),
    )
    .expect("Java 音频研究来源清单应为合法 JSON");
    let sources = manifest["sources"].as_array().expect("应声明 sources");

    assert_eq!(sources.len(), 6);
    assert!(sources
        .iter()
        .all(|source| source["source_copied"] == false));
    assert!(sources
        .iter()
        .all(|source| source["research_repository_license"] == "NOASSERTION"));

    let classification = |id: &str| {
        sources
            .iter()
            .find(|source| source["id"] == id)
            .and_then(|source| source["classification"].as_str())
            .expect("应记录研究模块分类")
    };
    assert_eq!(classification("chattts"), "local-http-wrapper");
    assert_eq!(classification("edgetts"), "local-client-remote-service");
    assert_eq!(classification("emotivoice"), "local-http-runtime");
    assert_eq!(
        classification("mars5tts"),
        "protocol-mismatch-reference-only"
    );
    assert_eq!(classification("unifiedtts"), "paid-remote-aggregator");
    assert_eq!(classification("whisper"), "local-asr-runtime");
}

#[test]
fn exposes_only_verified_default_transports_for_qwen_and_cosyvoice() {
    let registry = LocalTtsRuntimeRegistry::built_in();

    let qwen = registry
        .find(LocalTtsRuntimeKind::Qwen3Tts)
        .expect("应注册 Qwen3-TTS");
    assert_eq!(qwen.transport(), LocalTtsTransport::Process);
    assert_eq!(qwen.default_endpoint(), None);

    let cosyvoice = registry
        .find(LocalTtsRuntimeKind::CosyVoice)
        .expect("应注册 CosyVoice");
    assert_eq!(cosyvoice.transport(), LocalTtsTransport::CosyVoiceFastApi);
    assert_eq!(
        cosyvoice.default_endpoint(),
        Some("http://127.0.0.1:50000/inference_sft")
    );
}

#[test]
fn requires_selected_artifact_review_even_for_permissive_code() {
    let registry = LocalTtsRuntimeRegistry::built_in();
    let profile = registry
        .find(LocalTtsRuntimeKind::Qwen3Tts)
        .expect("应注册 Qwen3-TTS");

    assert!(matches!(
        profile.ensure_commercial_use(false),
        Err(TtsError::MissingModelLicenseReview { .. })
    ));
    profile
        .ensure_commercial_use(true)
        .expect("完成具体模型制品审查后才允许进入商用路径");
}

#[test]
fn denies_or_blocks_restricted_local_models_for_commercial_jobs() {
    let registry = LocalTtsRuntimeRegistry::built_in();

    for runtime in [
        LocalTtsRuntimeKind::ChatTts,
        LocalTtsRuntimeKind::F5Tts,
        LocalTtsRuntimeKind::FishSpeech,
        LocalTtsRuntimeKind::IndexTts,
    ] {
        let profile = registry.find(runtime).expect("应保留受限运行时的显式档案");
        assert!(matches!(
            profile.ensure_commercial_use(true),
            Err(TtsError::CommercialUseDenied { .. })
        ));
    }
}

#[test]
fn local_network_defaults_are_loopback_only() {
    let registry = LocalTtsRuntimeRegistry::built_in();

    for profile in registry.profiles() {
        if let Some(endpoint) = profile.default_endpoint() {
            assert!(
                endpoint.starts_with("http://127.0.0.1:"),
                "{} 不得默认暴露到局域网",
                profile.id()
            );
        }
    }
}
