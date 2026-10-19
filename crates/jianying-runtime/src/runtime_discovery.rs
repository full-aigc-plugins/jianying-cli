use crate::{
    RuntimeDraftRoot, RuntimeError, RuntimeFileIdentity, RuntimeInstallation, RuntimePlatform,
};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::Command;

type RuntimeCandidate = (
    &'static str,
    &'static str,
    &'static str,
    &'static str,
    Option<&'static str>,
    &'static str,
);

/// 在显式搜索根中只读发现编辑器安装；不生成或启用 Runtime Profile。
pub fn discover_runtime_installations(
    platform: RuntimePlatform,
    home: &Path,
    search_roots: &[PathBuf],
    local_app_data: Option<&Path>,
) -> Result<Vec<RuntimeInstallation>, RuntimeError> {
    let draft_roots = discover_runtime_draft_roots(platform, home, local_app_data);
    let mut seen = BTreeSet::new();
    let mut installations = Vec::new();
    for search_root in search_roots {
        for candidate in candidates(platform) {
            let installation_root = search_root.join(candidate.0);
            let executable = installation_root.join(candidate.1);
            if !executable.is_file() {
                continue;
            }
            let canonical =
                std::fs::canonicalize(&executable).map_err(|error| RuntimeError::Io {
                    path: executable.clone(),
                    message: error.to_string(),
                })?;
            if !seen.insert(canonical) {
                continue;
            }
            let identity = RuntimeFileIdentity::from_path(&executable)?;
            let version = match platform {
                RuntimePlatform::MacOs => macos_bundle_version(&installation_root),
                RuntimePlatform::Windows => windows_application_version(&installation_root),
                RuntimePlatform::Linux => None,
            };
            let product_draft_roots = draft_roots
                .iter()
                .filter(|root| root.product_id() == candidate.2)
                .map(|root| root.path().to_path_buf())
                .collect();
            installations.push(RuntimeInstallation::new(
                candidate.2,
                candidate.3,
                candidate.4.map(str::to_owned),
                version,
                platform,
                installation_root,
                identity,
                candidate.5,
                product_draft_roots,
            ));
        }
    }
    installations.sort_by(|left, right| {
        left.product_id()
            .cmp(right.product_id())
            .then_with(|| left.installation_root().cmp(right.installation_root()))
    });
    Ok(installations)
}

/// 返回当前平台的已知草稿根候选，包含不存在的候选以支持可操作诊断。
pub fn discover_runtime_draft_roots(
    platform: RuntimePlatform,
    home: &Path,
    local_app_data: Option<&Path>,
) -> Vec<RuntimeDraftRoot> {
    match platform {
        RuntimePlatform::MacOs => vec![
            RuntimeDraftRoot::new(
                "jianying",
                home.join("Movies/JianyingPro/User Data/Projects/com.lveditor.draft"),
            ),
            RuntimeDraftRoot::new(
                "jianying",
                home.join("Movies/JianyingPro/User Data/Projects/com.lemon.lvpro"),
            ),
            RuntimeDraftRoot::new(
                "capcut",
                home.join("Movies/CapCut/User Data/Projects/com.lveditor.draft"),
            ),
        ],
        RuntimePlatform::Windows => {
            let root = local_app_data
                .map(Path::to_path_buf)
                .unwrap_or_else(|| home.join("AppData/Local"));
            vec![
                RuntimeDraftRoot::new(
                    "jianying",
                    root.join("JianyingPro/User Data/Projects/com.lveditor.draft"),
                ),
                RuntimeDraftRoot::new(
                    "capcut",
                    root.join("CapCut/User Data/Projects/com.lveditor.draft"),
                ),
            ]
        }
        RuntimePlatform::Linux => Vec::new(),
    }
}

fn candidates(platform: RuntimePlatform) -> &'static [RuntimeCandidate] {
    const MACOS: &[RuntimeCandidate] = &[
        // 新版剪映的应用包和进程使用 VideoFusion 名称，显示名称仍为剪映专业版。
        (
            "VideoFusion-macOS.app",
            "Contents/MacOS/VideoFusion-macOS",
            "jianying",
            "JianYing Pro",
            Some("com.lemon.lvpro"),
            "VideoFusion-macOS",
        ),
        (
            "剪映专业版.app",
            "Contents/MacOS/VideoFusion-macOS",
            "jianying",
            "JianYing Pro",
            Some("com.lemon.lvpro"),
            "VideoFusion-macOS",
        ),
        (
            "剪映专业版.app",
            "Contents/MacOS/JianyingPro",
            "jianying",
            "JianYing Pro",
            Some("com.lemon.lv"),
            "JianyingPro",
        ),
        (
            "JianyingPro.app",
            "Contents/MacOS/JianyingPro",
            "jianying",
            "JianYing Pro",
            Some("com.lemon.lv"),
            "JianyingPro",
        ),
        (
            "CapCut.app",
            "Contents/MacOS/CapCut",
            "capcut",
            "CapCut",
            Some("com.lemon.lvoverseas"),
            "CapCut",
        ),
    ];
    const WINDOWS: &[RuntimeCandidate] = &[
        (
            "JianyingPro",
            "JianyingPro.exe",
            "jianying",
            "JianYing Pro",
            None,
            "JianyingPro.exe",
        ),
        // 逐用户安装器布局：启动器位于 <安装根>/Apps/，与版本化负载目录同级。
        (
            "JianyingPro",
            "Apps/JianyingPro.exe",
            "jianying",
            "JianYing Pro",
            None,
            "JianyingPro.exe",
        ),
        (
            "CapCut",
            "CapCut.exe",
            "capcut",
            "CapCut",
            None,
            "CapCut.exe",
        ),
        (
            "CapCut",
            "Apps/CapCut.exe",
            "capcut",
            "CapCut",
            None,
            "CapCut.exe",
        ),
        (
            "ByteDance/JianyingPro",
            "JianyingPro.exe",
            "jianying",
            "JianYing Pro",
            None,
            "JianyingPro.exe",
        ),
    ];
    match platform {
        RuntimePlatform::MacOs => MACOS,
        RuntimePlatform::Windows => WINDOWS,
        RuntimePlatform::Linux => &[],
    }
}

fn macos_bundle_version(bundle: &Path) -> Option<String> {
    let info = bundle.join("Contents/Info.plist");
    let bytes = std::fs::read(&info).ok()?;
    if let Ok(text) = std::str::from_utf8(&bytes) {
        if let Some(version) = xml_string_after_key(text, "CFBundleShortVersionString") {
            return Some(version);
        }
    }
    let output = Command::new("/usr/bin/plutil")
        .args(["-extract", "CFBundleShortVersionString", "raw", "-o", "-"])
        .arg(&info)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let version = String::from_utf8(output.stdout).ok()?;
    let version = version.trim();
    (!version.is_empty()).then(|| version.to_owned())
}

/// 读取 Windows 安装根下 `Apps` 中版本化负载目录名；纯函数，可在任意主机上用临时目录验证。
///
/// 逐用户布局为 `<安装根>/Apps/<版本>/`，启动器为 `<安装根>/Apps/JianyingPro.exe`；
/// 只接受形如 `11.6.0.14552` 的纯数字点分名，并按数值分量比较（`11.10.0` 高于 `11.9.0`）。
/// 没有版本化目录、目录不可读或名称不是版本时返回 `None`，不读取注册表、环境变量或运行中进程。
fn windows_application_version(installation_root: &Path) -> Option<String> {
    let apps = installation_root.join("Apps");
    let mut highest: Option<(Vec<u64>, String)> = None;
    for entry in std::fs::read_dir(&apps).ok()?.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        let Some(components) = version_components(name) else {
            continue;
        };
        // 先比数值分量，再比名称，保证同名数值（如 `011.6` 与 `11.6`）下结果稳定。
        let replaces = match highest.as_ref() {
            None => true,
            Some((current_components, current_name)) => {
                (&components, name) > (current_components, current_name.as_str())
            }
        };
        if replaces {
            highest = Some((components, name.to_owned()));
        }
    }
    highest.map(|(_, name)| name)
}

/// 把目录名解析为版本数值分量；至少两段且每段均为十进制数字时才视为版本。
fn version_components(name: &str) -> Option<Vec<u64>> {
    let parts: Vec<&str> = name.split('.').collect();
    if parts.len() < 2 {
        return None;
    }
    parts
        .iter()
        .map(|part| {
            if part.is_empty() || !part.bytes().all(|byte| byte.is_ascii_digit()) {
                return None;
            }
            part.parse::<u64>().ok()
        })
        .collect()
}

fn xml_string_after_key(text: &str, key: &str) -> Option<String> {
    let key = format!("<key>{key}</key>");
    let tail = text.split_once(&key)?.1;
    let start = tail.find("<string>")? + "<string>".len();
    let end = tail[start..].find("</string>")? + start;
    let value = tail[start..end].trim();
    (!value.is_empty()).then(|| value.to_owned())
}

#[cfg(test)]
mod tests {
    use super::windows_application_version;
    use std::path::{Path, PathBuf};

    /// 建立最小 Windows 逐用户安装夹具：`Apps/JianyingPro.exe` 与空的 `Apps/` 目录。
    fn fixture(name: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!(
            "jianying-windows-version-{name}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("Apps")).unwrap();
        std::fs::write(root.join("Apps/JianyingPro.exe"), b"synthetic launcher").unwrap();
        root
    }

    fn add_directory(root: &Path, name: &str) {
        std::fs::create_dir_all(root.join("Apps").join(name)).unwrap();
    }

    #[test]
    fn reads_version_from_the_single_versioned_directory() {
        let root = fixture("single");
        add_directory(&root, "11.6.0.14552");
        assert_eq!(
            windows_application_version(&root).as_deref(),
            Some("11.6.0.14552")
        );
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn picks_highest_version_by_numeric_components_not_lexicographically() {
        let root = fixture("several");
        add_directory(&root, "11.9.0");
        add_directory(&root, "11.10.0");
        add_directory(&root, "9.99.99999");
        assert_eq!(
            windows_application_version(&root).as_deref(),
            Some("11.10.0")
        );
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn reports_no_version_when_apps_has_no_versioned_directory() {
        let root = fixture("none");
        assert_eq!(windows_application_version(&root), None);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn ignores_directories_that_are_not_version_shaped() {
        let root = fixture("non-version");
        add_directory(&root, "data");
        add_directory(&root, "cef");
        add_directory(&root, "11.6.0.14552");
        assert_eq!(
            windows_application_version(&root).as_deref(),
            Some("11.6.0.14552")
        );
        std::fs::remove_dir_all(root).unwrap();
    }
}
