//! Draft store: root discovery, editor guard, publish + registration.
//!
//! Registration discipline follows capcut-cli (MIT): 剪映/CapCut read the
//! start page from `root_meta_info.json`, so a draft folder without a store
//! entry is invisible; writes refuse while the editor is running.

use anyhow::{bail, Context, Result};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::process::Command;
use uuid::Uuid;

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

#[cfg(unix)]
fn editors_from_process_listing(text: &str) -> Vec<String> {
    ["JianyingPro", "CapCut", "VideoFusion-macOS"]
        .into_iter()
        .filter(|name| {
            text.lines().any(|line| {
                Path::new(line.trim())
                    .file_name()
                    .and_then(|name| name.to_str())
                    == Some(*name)
            })
        })
        .map(str::to_owned)
        .collect()
}

#[cfg(all(test, unix))]
mod editor_process_tests {
    #[test]
    fn identifies_actual_editor_executables_without_matching_helpers_or_parent_paths() {
        let listing = "/Applications/VideoFusion-macOS.app/Contents/MacOS/VideoFusion-macOS\n/Applications/CapCut.app/Contents/MacOS/CapCut\nJianyingPro\n";
        let found = super::editors_from_process_listing(listing);
        assert_eq!(found.len(), 3);
        assert!(found.iter().any(|name| name == "VideoFusion-macOS"));
        assert!(super::editors_from_process_listing(
            "/Applications/CapCut.app/Contents/MacOS/helper\nVideoFusion-macOSTray\nnot-JianyingPro\n"
        ).is_empty());
    }
}

pub fn editors_running() -> Vec<String> {
    let mut found = vec![];
    #[cfg(unix)]
    {
        if let Ok(out) = Command::new("ps").args(["-axo", "comm="]).output() {
            let text = String::from_utf8_lossy(&out.stdout);
            found.extend(editors_from_process_listing(&text));
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

/// 返回平台默认草稿目录候选及可用性，供智能体显式选择。
pub fn directories() -> Value {
    Value::Array(
        draft_root_candidates()
            .into_iter()
            .map(|(namespace, path)| {
                json!({"namespace":namespace,"path":path,"exists":path.is_dir()})
            })
            .collect(),
    )
}

/// 在草稿库中安全重命名草稿目录，并同步时间线、元数据和根登记表。
pub fn rename(root: &Path, name: &str, new_name: &str) -> Result<Value> {
    validate_store_name(name)?;
    validate_store_name(new_name)?;
    let running = editors_running();
    if !running.is_empty() {
        bail!(
            "editor is running ({}); close JianYing/CapCut before renaming drafts",
            running.join(", ")
        );
    }
    let source = root.join(name);
    let target = root.join(new_name);
    if !source.is_dir() {
        bail!("draft {name} not found in {}", root.display());
    }
    if target.exists() {
        bail!("draft {new_name} already exists in {}", root.display());
    }
    crate::draft::validate_bundle(&source)?;
    let staging = root.join(format!(".jianying-rename-{}", Uuid::new_v4().simple()));
    copy_dir_recursive(&source, &staging)?;
    let mut timeline = crate::draft::load_timeline(&staging)?;
    timeline["name"] = json!(new_name);
    if let Err(error) = crate::template::save_timeline_as(&staging, &timeline, &target) {
        let _ = std::fs::remove_dir_all(&staging);
        return Err(error);
    }
    crate::draft::validate_bundle_as(&staging, &target)?;
    std::fs::rename(&staging, &target)?;

    let root_meta_path = root.join("root_meta_info.json");
    if root_meta_path.is_file() {
        let mut root_meta: Value =
            serde_json::from_str(&std::fs::read_to_string(&root_meta_path)?)?;
        for entry in root_meta
            .as_object_mut()
            .into_iter()
            .flat_map(|object| object.values_mut())
            .flat_map(|value| value.as_array_mut().into_iter().flatten())
        {
            if entry["draft_name"].as_str() == Some(name)
                || entry["draft_fold_path"].as_str() == Some(source.to_string_lossy().as_ref())
            {
                entry["draft_name"] = json!(new_name);
                entry["draft_fold_path"] = json!(target.to_string_lossy());
                entry["draft_root_path"] = json!(root.to_string_lossy());
                entry["draft_json_file"] =
                    json!(target.join("draft_content.json").to_string_lossy());
            }
        }
        if let Err(error) = write_atomic_json(&root_meta_path, &root_meta) {
            let _ = std::fs::remove_dir_all(&target);
            return Err(error);
        }
    }
    std::fs::remove_dir_all(&source)?;
    Ok(json!({"status":"renamed","from":name,"to":new_name,"draft":target}))
}

/// 规划或执行根时间线到镜像文件的单向同步。
pub fn sync_timelines(draft: &Path, apply: bool, force_newer: bool) -> Result<Value> {
    let canonical = draft.join("draft_content.json");
    let canonical_raw = std::fs::read_to_string(&canonical)
        .with_context(|| format!("reading canonical timeline {}", canonical.display()))?;
    let canonical_json: Value = serde_json::from_str(&canonical_raw)
        .context("canonical draft_content.json must be plaintext JSON")?;
    let canonical_modified = std::fs::metadata(&canonical)?.modified()?;
    let mut plans = Vec::new();
    for name in ["draft_info.json", "template-2.tmp"] {
        let path = draft.join(name);
        if !path.is_file() {
            continue;
        }
        let raw = std::fs::read_to_string(&path)?;
        let parsed: Value = serde_json::from_str(&raw)
            .with_context(|| format!("mirror {} must be plaintext JSON", path.display()))?;
        let drifted = parsed != canonical_json;
        let newer = drifted && std::fs::metadata(&path)?.modified()? > canonical_modified;
        plans.push(json!({"path":path,"drifted":drifted,"newer_than_canonical":newer}));
    }
    let newer_mirrors: Vec<String> = plans
        .iter()
        .filter(|item| item["newer_than_canonical"].as_bool() == Some(true))
        .filter_map(|item| item["path"].as_str().map(str::to_owned))
        .collect();
    if apply && !newer_mirrors.is_empty() && !force_newer {
        bail!(
            "refused [mirror-newer]: {} mirror(s) are newer than draft_content.json; inspect or pass --force-newer",
            newer_mirrors.len()
        );
    }
    let mut reconciled = Vec::new();
    let mut backups = Vec::new();
    if apply {
        let running = editors_running();
        if !running.is_empty() {
            bail!(
                "editor is running ({}); close JianYing/CapCut before syncing timelines",
                running.join(", ")
            );
        }
        for item in &plans {
            if item["drifted"].as_bool() != Some(true) {
                continue;
            }
            let path = PathBuf::from(item["path"].as_str().unwrap_or_default());
            let backup = path.with_extension(format!(
                "{}.bak",
                path.extension()
                    .and_then(|value| value.to_str())
                    .unwrap_or("json")
            ));
            std::fs::copy(&path, &backup)?;
            write_atomic(&path, canonical_raw.as_bytes())?;
            reconciled.push(path);
            backups.push(backup);
        }
        crate::draft::validate_bundle(draft)?;
    }
    Ok(json!({"canonical":canonical,"apply":apply,"targets":plans,
        "newer_mirrors":newer_mirrors,"reconciled":reconciled,"backups":backups}))
}

/// 创建一个不覆盖既有目标的完整草稿备份。
pub fn backup(draft: &Path, out: &Path) -> Result<Value> {
    crate::draft::validate_bundle(draft)?;
    if out.exists() {
        bail!("backup target already exists: {}", out.display());
    }
    copy_dir_recursive(draft, out)?;
    let timeline = crate::draft::load_timeline(out)?;
    crate::template::save_timeline_as(out, &timeline, out)?;
    crate::draft::validate_bundle(out)?;
    Ok(json!({"status":"backed_up","source":draft,"backup":out}))
}

/// 通过 MutationPlan 将完整备份恢复到既有草稿。
pub fn restore(snapshot: &Path, target: &Path) -> Result<Value> {
    if !snapshot.is_dir() {
        bail!(
            "restore snapshot is not a directory: {}",
            snapshot.display()
        );
    }
    let running = editors_running();
    if !running.is_empty() {
        bail!(
            "editor is running ({}); close JianYing/CapCut before restoring drafts",
            running.join(", ")
        );
    }
    let state_root = target
        .parent()
        .unwrap_or(target)
        .join(".jianying-transactions");
    let mut plan = jianying_store::MutationPlan::new(target.to_path_buf(), state_root)?;
    plan.stage()?;
    plan.replace_work_copy_from(snapshot)?;
    let timeline = crate::draft::load_timeline(plan.work_copy())?;
    crate::template::save_timeline_as(plan.work_copy(), &timeline, target)?;
    let identity = plan.source().to_path_buf();
    plan.validate(|work_copy| {
        crate::draft::validate_bundle_as(work_copy, &identity)
            .map_err(|error| jianying_store::StoreError::Validation(format!("{error:#}")))
    })?;
    plan.commit()?;
    Ok(json!({"target":target,"restored_from":snapshot,
        "safety_snapshot":plan.snapshot(),"audit_file":plan.audit_file()}))
}

/// 检测剪映加密、明文 JSON 或损坏 JSON；本项目明确不提供解密算法。
pub fn detect_encryption(input: &Path) -> Result<Value> {
    let path = if input.is_dir() {
        input.join("draft_content.json")
    } else {
        input.to_path_buf()
    };
    let bytes = std::fs::read(&path)?;
    let first_non_whitespace = bytes
        .iter()
        .copied()
        .find(|byte| !byte.is_ascii_whitespace());
    let parsed = serde_json::from_slice::<Value>(&bytes);
    let (encrypted, classification, reason) = match parsed {
        Ok(value) if value.is_object() => (
            false,
            "plaintext",
            "timeline parses as a plaintext JSON object",
        ),
        Ok(_) => (false, "corrupted", "timeline JSON root is not an object"),
        Err(_) if first_non_whitespace == Some(b'{') => (
            false,
            "corrupted",
            "timeline begins like JSON but parsing failed; this is not classified as encryption",
        ),
        Err(_) => (
            true,
            "encrypted",
            "timeline is non-JSON binary data consistent with JianYing 6.0+ encryption",
        ),
    };
    let leading_bytes = bytes
        .iter()
        .take(8)
        .map(|byte| format!("{byte:02x}"))
        .collect::<Vec<_>>()
        .join("");
    Ok(json!({
        "path":path,"encrypted":encrypted,"classification":classification,"reason":reason,
        "bytes":bytes.len(),"leading_bytes_hex":leading_bytes,"decrypt_supported":false,
        "workarounds":["use a plaintext app-authored draft or generated copy",
            "use CapCut International for plaintext round-trip editing",
            "keep encrypted drafts behind the native Runtime Adapter boundary"]
    }))
}

fn validate_store_name(name: &str) -> Result<()> {
    let path = Path::new(name);
    if name.trim().is_empty()
        || path.is_absolute()
        || path.components().count() != 1
        || matches!(name, "." | "..")
    {
        bail!("draft name must be one safe path component");
    }
    Ok(())
}

fn write_atomic_json(path: &Path, value: &Value) -> Result<()> {
    write_atomic(path, &serde_json::to_vec_pretty(value)?)
}

fn write_atomic(path: &Path, bytes: &[u8]) -> Result<()> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let name = path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("store");
    let temporary = parent.join(format!(".{name}.{}.tmp", Uuid::new_v4().simple()));
    std::fs::write(&temporary, bytes)?;
    std::fs::rename(&temporary, path)?;
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
    crate::draft::validate_bundle(draft_dir)
        .context("draft bundle failed integrity validation before publish")?;
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
    let mut published_timeline = crate::draft::load_timeline(&dest)?;
    crate::draft::relocate_material_paths(&mut published_timeline, draft_dir, &dest);
    crate::template::save_timeline(&dest, &published_timeline)
        .context("synchronizing the published draft bundle")?;
    meta = serde_json::from_str(&std::fs::read_to_string(dest.join("draft_meta_info.json"))?)?;

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
    // Restamp only the published copy; the source build remains a valid,
    // independently addressable draft bundle.
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
