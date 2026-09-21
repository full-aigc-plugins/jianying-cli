use jianying_runtime::{
    discover_runtime_draft_roots, discover_runtime_installations, RuntimePlatform,
};
use std::path::PathBuf;

fn root(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "jianying-runtime-discovery-{name}-{}",
        std::process::id()
    ))
}

#[test]
fn discovers_exact_macos_installations_without_enabling_automatic_routing() {
    let root = root("macos");
    let _ = std::fs::remove_dir_all(&root);
    let home = root.join("home");
    let applications = root.join("Applications");
    let bundle = applications.join("剪映专业版.app");
    let executable = bundle.join("Contents/MacOS/JianyingPro");
    std::fs::create_dir_all(executable.parent().unwrap()).unwrap();
    std::fs::write(&executable, b"synthetic editor identity").unwrap();
    std::fs::write(
        bundle.join("Contents/Info.plist"),
        br#"<?xml version="1.0"?><plist><dict><key>CFBundleShortVersionString</key><string>9.9.1</string></dict></plist>"#,
    )
    .unwrap();
    let draft_root = home.join("Movies/JianyingPro/User Data/Projects/com.lveditor.draft");
    std::fs::create_dir_all(&draft_root).unwrap();

    let installations = discover_runtime_installations(
        RuntimePlatform::MacOs,
        &home,
        &[applications.clone(), applications],
        None,
    )
    .unwrap();
    assert_eq!(installations.len(), 1);
    assert_eq!(installations[0].product_id(), "jianying");
    assert_eq!(installations[0].version(), Some("9.9.1"));
    assert_eq!(installations[0].executable_identity().path(), executable);
    let value = serde_json::to_value(&installations[0]).unwrap();
    assert_eq!(value["support_status"], "unverified");
    assert_eq!(value["automatic_routing"], false);
    assert_eq!(
        value["draft_roots"][0],
        draft_root.to_string_lossy().as_ref()
    );
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn discovers_videofusion_layout_with_current_jianying_identity() {
    for name in ["VideoFusion-macOS.app", "剪映专业版.app"] {
        let root = root(&format!("videofusion-{name}"));
        let applications = root.join("Applications");
        let bundle = applications.join(name);
        let executable = bundle.join("Contents/MacOS/VideoFusion-macOS");
        std::fs::create_dir_all(executable.parent().unwrap()).unwrap();
        std::fs::write(&executable, b"independent synthetic executable").unwrap();
        std::fs::write(bundle.join("Contents/Info.plist"),
            br#"<plist><dict><key>CFBundleShortVersionString</key><string>11.5.13239</string></dict></plist>"#).unwrap();
        let installations = discover_runtime_installations(
            RuntimePlatform::MacOs,
            &root.join("home"),
            &[applications.clone(), applications],
            None,
        )
        .unwrap();
        assert_eq!(installations.len(), 1, "missing layout {name}");
        let value = serde_json::to_value(&installations[0]).unwrap();
        assert_eq!(value["bundle_id"], "com.lemon.lvpro");
        assert_eq!(value["process_name"], "VideoFusion-macOS");
        assert_eq!(value["version"], "11.5.13239");
        assert_eq!(value["support_status"], "unverified");
        assert_eq!(value["automatic_routing"], false);
        assert_eq!(installations[0].executable_identity().path(), executable);
        std::fs::remove_dir_all(root).unwrap();
    }
}

#[test]
fn reports_existing_orphan_draft_roots_when_no_editor_is_installed() {
    let root = root("orphan");
    let _ = std::fs::remove_dir_all(&root);
    let home = root.join("home");
    let draft_root = home.join("Movies/JianyingPro/User Data/Projects/com.lveditor.draft");
    std::fs::create_dir_all(&draft_root).unwrap();

    let installations =
        discover_runtime_installations(RuntimePlatform::MacOs, &home, &[], None).unwrap();
    assert!(installations.is_empty());
    let roots = discover_runtime_draft_roots(RuntimePlatform::MacOs, &home, None);
    assert!(roots
        .iter()
        .any(|candidate| candidate.path() == draft_root && candidate.exists()));
    std::fs::remove_dir_all(root).unwrap();
}
