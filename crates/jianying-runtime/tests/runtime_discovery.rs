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
fn discovers_per_user_windows_layout_with_apps_launcher_and_version() {
    let local_app_data = root("windows-per-user");
    let _ = std::fs::remove_dir_all(&local_app_data);
    let installation = local_app_data.join("JianyingPro");
    let apps = installation.join("Apps");
    let executable = apps.join("JianyingPro.exe");
    std::fs::create_dir_all(apps.join("11.6.0.14552")).unwrap();
    std::fs::write(&executable, b"synthetic windows launcher").unwrap();
    // 真实逐用户安装器在启动器同级还有配置文件与非版本目录，解析必须忽略它们。
    std::fs::write(apps.join("Configure.ini"), b"").unwrap();
    std::fs::create_dir_all(apps.join("data")).unwrap();
    let draft_root = local_app_data.join("JianyingPro/User Data/Projects/com.lveditor.draft");
    std::fs::create_dir_all(&draft_root).unwrap();

    let installations = discover_runtime_installations(
        RuntimePlatform::Windows,
        &local_app_data.join("home"),
        std::slice::from_ref(&local_app_data),
        Some(&local_app_data),
    )
    .unwrap();
    assert_eq!(installations.len(), 1);
    assert_eq!(installations[0].product_id(), "jianying");
    assert_eq!(installations[0].version(), Some("11.6.0.14552"));
    assert_eq!(installations[0].installation_root(), installation);
    assert_eq!(installations[0].executable_identity().path(), executable);
    let value = serde_json::to_value(&installations[0]).unwrap();
    assert_eq!(value["version"], "11.6.0.14552");
    assert_eq!(value["support_status"], "unverified");
    assert_eq!(value["automatic_routing"], false);
    assert_eq!(
        value["draft_roots"][0],
        draft_root.to_string_lossy().as_ref()
    );
    std::fs::remove_dir_all(local_app_data).unwrap();
}

#[test]
fn keeps_program_files_layouts_alongside_the_per_user_layout() {
    let root = root("windows-layouts");
    let _ = std::fs::remove_dir_all(&root);
    let program_files = root.join("ProgramFiles");
    let local_app_data = root.join("LocalAppData");

    // 传统 Program Files 布局：启动器位于安装根，版本化负载目录在其 Apps/ 下。
    for (installation, version) in [
        ("JianyingPro", "11.5.0"),
        ("ByteDance/JianyingPro", "9.0.1"),
    ] {
        let installation_root = program_files.join(installation);
        let executable = installation_root.join("JianyingPro.exe");
        std::fs::create_dir_all(executable.parent().unwrap()).unwrap();
        std::fs::write(&executable, format!("synthetic {version} launcher")).unwrap();
        std::fs::create_dir_all(installation_root.join("Apps").join(version)).unwrap();
    }

    // 逐用户安装器布局：启动器位于 Apps/，与版本化负载目录同级。
    let per_user_apps = local_app_data.join("JianyingPro").join("Apps");
    std::fs::create_dir_all(per_user_apps.join("11.6.0.14552")).unwrap();
    std::fs::write(
        per_user_apps.join("JianyingPro.exe"),
        b"synthetic per-user launcher",
    )
    .unwrap();

    let installations = discover_runtime_installations(
        RuntimePlatform::Windows,
        &root.join("home"),
        &[program_files, local_app_data],
        None,
    )
    .unwrap();
    let mut versions: Vec<&str> = installations
        .iter()
        .map(|installation| installation.version().unwrap_or("missing"))
        .collect();
    versions.sort_unstable();
    assert_eq!(versions, ["11.5.0", "11.6.0.14552", "9.0.1"]);
    for installation in &installations {
        assert_eq!(installation.product_id(), "jianying");
        let value = serde_json::to_value(installation).unwrap();
        assert_eq!(value["support_status"], "unverified");
        assert_eq!(value["automatic_routing"], false);
    }
    std::fs::remove_dir_all(root).unwrap();
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
