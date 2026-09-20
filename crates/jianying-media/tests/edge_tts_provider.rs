#[cfg(unix)]
mod unix {
    use jianying_media::{EdgeTtsProvider, TtsAudioFormat, TtsProvider, TtsRequest};
    use std::os::unix::fs::PermissionsExt;

    #[test]
    fn edge_tts_uses_structured_argv_and_declares_network_dependency() {
        let root = std::env::temp_dir().join(format!("jianying-edge-tts-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        let executable = root.join("fake-edge-tts.sh");
        std::fs::write(
            &executable,
            "#!/bin/sh\nout=''\nnext=0\nfor arg in \"$@\"; do\n  if [ \"$next\" = 1 ]; then out=$arg; next=0; fi\n  if [ \"$arg\" = '--write-media' ]; then next=1; fi\ndone\nprintf 'edge-audio' > \"$out\"\n",
        )
        .unwrap();
        let mut permissions = std::fs::metadata(&executable).unwrap().permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(&executable, permissions).unwrap();

        let provider = EdgeTtsProvider::new(&executable).unwrap();
        assert!(provider.requires_network());
        assert!(!provider.capability().billed());

        let marker = root.join("must-not-exist");
        let text = format!("hello; touch {}", marker.display());
        let request = TtsRequest::new(text, TtsAudioFormat::Mp3)
            .unwrap()
            .with_voice("zh-CN-XiaoyiNeural")
            .with_speed(1.25)
            .unwrap()
            .with_volume(0.8)
            .unwrap();
        let output = root.join("speech.mp3");
        let artifact = provider.synthesize(&request, &output).unwrap();

        assert_eq!(artifact.provider_id(), "edge-tts");
        assert_eq!(std::fs::read(&output).unwrap(), b"edge-audio");
        assert!(!marker.exists());
        let _ = std::fs::remove_dir_all(root);
    }
}
