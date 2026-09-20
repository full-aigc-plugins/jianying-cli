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
                RuntimePlatform::Windows | RuntimePlatform::Linux => None,
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
        (
            "CapCut",
            "CapCut.exe",
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

fn xml_string_after_key(text: &str, key: &str) -> Option<String> {
    let key = format!("<key>{key}</key>");
    let tail = text.split_once(&key)?.1;
    let start = tail.find("<string>")? + "<string>".len();
    let end = tail[start..].find("</string>")? + start;
    let value = tail[start..end].trim();
    (!value.is_empty()).then(|| value.to_owned())
}
