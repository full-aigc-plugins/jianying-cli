#[cfg(unix)]
mod unix {
    use jianying_runtime::{
        PersistentRuntimeController, RuntimeControlStatus, RuntimeFileIdentity, RuntimePlatform,
        RuntimeProfile,
    };
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn fixture_root() -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!(
            "jianying-persistent-runtime-{}-{nonce}",
            std::process::id()
        ))
    }

    #[test]
    fn a_second_controller_can_stop_only_the_recorded_owned_process() {
        let root = fixture_root();
        fs::create_dir_all(&root).unwrap();
        let executable = PathBuf::from("/bin/sleep");

        let profile = RuntimeProfile::new(
            "fixture-editor",
            "Fixture Editor",
            "1.0",
            RuntimePlatform::MacOs,
            ["editor.control"],
        )
        .unwrap()
        .with_file_identity(RuntimeFileIdentity::from_path(&executable).unwrap());

        let first = PersistentRuntimeController::new(&root);
        let started = first.start(&profile, &executable, ["30"]).unwrap();
        assert_eq!(started.status(), RuntimeControlStatus::Running);
        assert!(started.pid() > 0);

        let second = PersistentRuntimeController::new(&root);
        assert_eq!(
            second.status(&profile).unwrap().status(),
            RuntimeControlStatus::Running
        );
        assert_eq!(
            second.stop(&profile).unwrap().status(),
            RuntimeControlStatus::Stopped
        );
        assert_eq!(
            first.status(&profile).unwrap().status(),
            RuntimeControlStatus::Stopped
        );

        fs::remove_dir_all(root).unwrap();
    }
}
