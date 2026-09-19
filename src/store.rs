//! Draft store: root discovery, editor guard, publish + registration.
//!
//! Registration discipline follows capcut-cli (MIT): 剪映/CapCut read the
//! start page from `root_meta_info.json`, so a draft folder without a store
//! entry is invisible; writes refuse while the editor is running.

use anyhow::{bail, Context, Result};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::process::Command;

pub fn draft_root_candidates() -> Vec<(&'static str, PathBuf)> {
    let home = PathBuf::from(std::env::var("HOME").unwrap_or_default());
    let appdata = std::env::var("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|_| home.join("AppData").join("Local"));
    let mut roots = vec![];
    if cfg!(target_os = "macos") {
        roots.push((
            "jianying",
            home.join("Movies/JianyingPro/User Data/Projects/com.lveditor.draft"),
        ));
        roots.push((
            "jianying",
            home.join("Movies/JianyingPro/User Data/Projects/com.lemon.lvpro"),
        ));
        roots.push((
            "capcut",
            home.join("Movies/CapCut/User Data/Projects/com.lveditor.draft"),
        ));
    }
    if cfg!(target_os = "windows") {
        roots.push((
            "jianying",
            appdata.join(r"JianyingPro\User Data\Projects\com.lveditor.draft"),
        ));
        roots.push((
            "capcut",
            appdata.join(r"CapCut\User Data\Projects\com.lveditor.draft"),
        ));
    }
    roots
}

pub fn resolve_root(explicit: Option<&Path>) -> Result<PathBuf> {
    if let Some(p) = explicit {
        let p = p.to_path_buf();
        if !p.is_dir() {
            bail!("draft root {} does not exist", p.display());
        }
        return Ok(p);
    }
    if let Ok(env) = std::env::var("JIANYING_CLI_DRAFT_ROOT") {
        let p = PathBuf::from(env);
        if p.is_dir() {
            return Ok(p);
        }
    }
    for (_, p) in draft_root_candidates() {
        if p.is_dir() {
            return Ok(p);
        }
    }
    bail!(
        "no JianYing draft root found; launch 剪映专业版 once, or pass --root / set JIANYING_CLI_DRAFT_ROOT"
    );
}

pub fn editors_running() -> Vec<String> {
    let mut found = vec![];
    #[cfg(unix)]
    {
        if let Ok(out) = Command::new("ps").args(["-axo", "comm="]).output() {
            let text = String::from_utf8_lossy(&out.stdout);
            for needle in ["JianyingPro", "CapCut"] {
                if text.lines().any(|l| l.contains(needle)) {
                    found.push(needle.to_string());
                }
            }
        }
    }
    #[cfg(windows)]
    {
        if let Ok(out) = Command::new("tasklist").args(["/FO", "CSV"]).output() {
            let text = String::from_utf8_lossy(&out.stdout);
            for needle in ["JianyingPro.exe", "CapCut.exe"] {
                if text.to_lowercase().contains(&needle.to_lowercase()) {
                    found.push(needle.to_string());
                }
            }
        }
    }
    found.sort();
    found.dedup();
    found
}

pub fn doctor() -> Result<Value> {
    let roots: Vec<Value> = draft_root_candidates()
        .into_iter()
        .map(|(ns, p)| json!({"namespace": ns, "path": p.to_string_lossy(), "exists": p.is_dir()}))
        .collect();
    Ok(json!({
        "version": env!("CARGO_PKG_VERSION"),
        "plan_schema": crate::plan::SCHEMA,
        "draft_roots": roots,
        "editors_running": editors_running(),
        "ffprobe": crate::probe::ffprobe_path(),
        "ffmpeg": crate::probe::ffmpeg_path(),
    }))
}

fn copy_dir_recursive(src: &Path, dest: &Path) -> Result<()> {
    std::fs::create_dir_all(dest)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        let target = dest.join(entry.file_name());
        if ty.is_dir() {
            copy_dir_recursive(&entry.path(), &target)?;
        } else {
            std::fs::copy(entry.path(), &target)?;
        }
    }
    Ok(())
}

fn find_store_key(root_meta: &Value) -> Option<String> {
    root_meta.as_object()?.iter().find_map(|(k, v)| {
        v.as_array().and_then(|a| {
            a.first().and_then(|e| {
                if e.get("draft_fold_path").is_some() && e.get("draft_id").is_some() {
                    Some(k.clone())
                } else {
                    None
                }
            })
        })
    })
}

/// Copy a built draft into the draft root and register it in
/// `root_meta_info.json`. Non-destructive: refuses existing destinations.
pub fn publish(draft_dir: &Path, root: &Path, force: bool) -> Result<Value> {
    let running = editors_running();
    if !running.is_empty() && !force {
        bail!(
            "editor is running ({}); close 剪映/CapCut first, or pass --force",
            running.join(", ")
        );
    }
    let meta_path = draft_dir.join("draft_meta_info.json");
    if !meta_path.is_file() {
        bail!(
            "{} is not a jycut draft (missing draft_meta_info.json)",
            draft_dir.display()
        );
    }
    let mut meta: Value = serde_json::from_str(&std::fs::read_to_string(&meta_path)?)?;
    let name = meta["draft_name"]
        .as_str()
        .map(str::to_owned)
        .unwrap_or_else(|| {
            draft_dir
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default()
        });
    let dest = root.join(&name);
    if dest.exists() {
        bail!(
            "draft {} already exists in the store; rename and rebuild",
            name
        );
    }
    copy_dir_recursive(draft_dir, &dest)?;

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_micros() as i64)
        .unwrap_or(0);
    let root_str = root.to_string_lossy().into_owned();
    let dest_str = dest.to_string_lossy().into_owned();
    meta["draft_fold_path"] = json!(dest_str);
    meta["draft_root_path"] = json!(root_str);
    meta["draft_json_file"] = json!(dest.join("draft_content.json").to_string_lossy());
    meta["tm_draft_modified"] = json!(now);
    std::fs::write(
        meta_path.parent().unwrap().join("draft_meta_info.json"),
        serde_json::to_string_pretty(&meta)?,
    )?;

    // Also restamp the copy's own meta sidecar.
    let dest_meta_path = dest.join("draft_meta_info.json");
    std::fs::write(&dest_meta_path, serde_json::to_string_pretty(&meta)?)?;

    // Register in root_meta_info.json (visible in the start page).
    let root_meta_path = root.join("root_meta_info.json");
    let mut root_meta: Value = if root_meta_path.is_file() {
        let raw = std::fs::read_to_string(&root_meta_path)?;
        serde_json::from_str(&raw)
            .with_context(|| format!("parsing {}", root_meta_path.display()))?
    } else {
        json!({})
    };
    let store_key = find_store_key(&root_meta).unwrap_or_else(|| "all_draft_store".into());
    if !root_meta[&store_key].is_array() {
        root_meta[&store_key] = json!([]);
    }
    let store = root_meta[&store_key].as_array().unwrap();
    let fold_clash = store
        .iter()
        .any(|e| e["draft_fold_path"].as_str() == Some(dest_str.as_str()));
    let name_clash = store
        .iter()
        .any(|e| e["draft_name"].as_str() == Some(name.as_str()));
    if fold_clash || name_clash {
        // roll back the copy
        std::fs::remove_dir_all(&dest).ok();
        bail!("store already has a draft named {name}; rename the draft and rebuild");
    }
    let entry = json!({
        "draft_cover": meta.get("draft_cover").cloned().unwrap_or(json!("draft_cover.jpg")),
        "draft_fold_path": dest_str,
        "draft_id": meta["draft_id"],
        "draft_is_ai_shorts": false,
        "draft_is_invisible": false,
        "draft_json_file": meta["draft_json_file"],
        "draft_name": name,
        "draft_new_version": meta.get("draft_new_version").cloned().unwrap_or(json!("")),
        "draft_root_path": root_str,
        "draft_timeline_materials_size": 0,
        "tm_draft_create": now,
        "tm_draft_modified": now,
        "tm_draft_removed": 0,
        "tm_duration": meta.get("tm_duration").cloned().unwrap_or(json!(0)),
    });
    let mut new_store = store.clone();
    new_store.push(entry);
    // backup then write
    if root_meta_path.is_file() {
        std::fs::copy(&root_meta_path, root_meta_path.with_extension("json.bak"))?;
    }
    root_meta[&store_key] = json!(new_store);
    std::fs::write(&root_meta_path, serde_json::to_string_pretty(&root_meta)?)?;

    Ok(json!({
        "status": "published",
        "name": name,
        "draft": dest_str,
        "store_key": store_key,
        "editor_running": running,
    }))
}

/// `DraftFolder::list_drafts` parity: draft names known to the store.
pub fn list(root: &Path) -> Result<Value> {
    let mut names: Vec<String> = Vec::new();
    if let Ok(dirs) = std::fs::read_dir(root) {
        for d in dirs.flatten() {
            let p = d.path();
            if p.is_dir() && p.join("draft_content.json").is_file()
                || p.is_dir() && p.join("draft_info.json").is_file()
            {
                names.push(d.file_name().to_string_lossy().into_owned());
            }
        }
    }
    names.sort();
    Ok(json!({"root": root.to_string_lossy(), "drafts": names}))
}

/// `DraftFolder::has_draft` parity.
pub fn has(root: &Path, name: &str) -> Result<Value> {
    Ok(json!({"name": name, "exists": root.join(name).is_dir()}))
}

/// `DraftFolder::remove` parity: delete the draft folder and unregister it.
pub fn remove(root: &Path, name: &str) -> Result<Value> {
    let dir = root.join(name);
    if !dir.is_dir() {
        bail!("draft {name} not found in {}", root.display());
    }
    std::fs::remove_dir_all(&dir)?;
    let root_meta_path = root.join("root_meta_info.json");
    if root_meta_path.is_file() {
        let mut root_meta: Value =
            serde_json::from_str(&std::fs::read_to_string(&root_meta_path)?)?;
        if let Some(obj) = root_meta.as_object_mut() {
            for (_k, v) in obj.iter_mut() {
                if let Some(arr) = v.as_array_mut() {
                    arr.retain(|e| e["draft_name"].as_str() != Some(name));
                }
            }
        }
        std::fs::write(&root_meta_path, serde_json::to_string_pretty(&root_meta)?)?;
    }
    Ok(json!({"status": "removed", "name": name}))
}
