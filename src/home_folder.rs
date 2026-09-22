//! 剪映首页本地文件夹生命周期。
//!
//! 本模块只操作已观测并冻结的 `LocalDraftFolder` JSON 协议。
//! 所有修改先进入同卷工作副本，验证后再整目录替换，以避免四个
//! 相关配置文件出现部分更新。

use anyhow::{bail, Context, Result};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use uuid::Uuid;

const FOLDER_META: &str = "folder_meta_info.json";
const DRAFT_MAPPINGS: &str = "draft_folder_mappings.json";
const RECYCLE_BIN: &str = "recycle_bin.json";
const RECYCLED_MAPPINGS: &str = "draft_mapping_recycle_bin.json";
const REQUIRED_FILES: [&str; 4] = [FOLDER_META, DRAFT_MAPPINGS, RECYCLE_BIN, RECYCLED_MAPPINGS];

/// 返回当前平台可能的首页本地文件夹配置目录。
pub fn config_root_candidates() -> Vec<(&'static str, PathBuf)> {
    let home = PathBuf::from(std::env::var("HOME").unwrap_or_default());
    let appdata = std::env::var("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|_| home.join("AppData").join("Local"));
    let mut roots = Vec::new();
    if cfg!(target_os = "macos") {
        roots.push((
            "jianying",
            home.join("Movies/JianyingPro/User Data/Config/LocalDraftFolder"),
        ));
        roots.push((
            "capcut",
            home.join("Movies/CapCut/User Data/Config/LocalDraftFolder"),
        ));
    }
    if cfg!(target_os = "windows") {
        roots.push((
            "jianying",
            appdata
                .join("JianyingPro")
                .join("User Data")
                .join("Config")
                .join("LocalDraftFolder"),
        ));
        roots.push((
            "capcut",
            appdata
                .join("CapCut")
                .join("User Data")
                .join("Config")
                .join("LocalDraftFolder"),
        ));
    }
    roots
}

/// 解析并校验首页本地文件夹配置目录。
///
/// `explicit` 为空时优先使用 `JIANYING_CLI_HOME_CONFIG_ROOT`，然后检测平台默认路径。
pub fn resolve_config_root(explicit: Option<&Path>) -> Result<PathBuf> {
    let root = explicit
        .map(Path::to_path_buf)
        .or_else(|| std::env::var_os("JIANYING_CLI_HOME_CONFIG_ROOT").map(PathBuf::from))
        .or_else(|| {
            config_root_candidates()
                .into_iter()
                .map(|(_, path)| path)
                .find(|path| path.is_dir())
        })
        .ok_or_else(|| {
            anyhow::anyhow!(
                "home_config_root_not_found: launch JianYing once, pass --config-root, or set JIANYING_CLI_HOME_CONFIG_ROOT"
            )
        })?;
    validate_config_root(&root)?;
    Ok(root)
}

/// 返回当前 UTC 时间，格式与剪映首页配置一致。
pub fn current_timestamp() -> Result<String> {
    let format = time::format_description::parse_borrowed::<2>(
        "[year]-[month]-[day]T[hour]:[minute]:[second]",
    )?;
    let now = time::OffsetDateTime::now_utc();
    let local = time::UtcOffset::current_local_offset()
        .map(|offset| now.to_offset(offset))
        .unwrap_or(now);
    Ok(local.format(&format)?)
}

/// 列出活动文件夹，保留每个条目的未知字段。
pub fn list(root: &Path) -> Result<Value> {
    let documents = load_documents(root)?;
    Ok(json!({
        "config_root": root,
        "folders": array(&documents.folder_meta, "folders")?,
        "count": array(&documents.folder_meta, "folders")?.len(),
        "timestamp": documents.folder_meta.get("timestamp").cloned().unwrap_or(Value::Null)
    }))
}

/// 列出“最近删除”条目，保留每个条目的未知字段。
pub fn list_recycled(root: &Path) -> Result<Value> {
    let documents = load_documents(root)?;
    Ok(json!({
        "config_root": root,
        "recycled_folders": array(&documents.recycle_bin, "recycled_folders")?,
        "count": array(&documents.recycle_bin, "recycled_folders")?.len(),
        "timestamp": documents.recycle_bin.get("timestamp").cloned().unwrap_or(Value::Null)
    }))
}

/// 创建一个首页本地文件夹。
///
/// 同一父节点下的同名文件夹被拒绝，避免智能体后续产生名称歧义。
pub fn create(root: &Path, name: &str, parent_id: &str, at: &str) -> Result<Value> {
    validate_name(name)?;
    ensure_safe_mutation_target(root)?;
    let mut documents = load_documents(root)?;
    let folders = array(&documents.folder_meta, "folders")?;
    if !parent_id.is_empty() && count_id(folders, parent_id) != 1 {
        bail!("home_folder_parent_not_found: expected exactly one parent id {parent_id}");
    }
    if folders.iter().any(|folder| {
        folder.get("name").and_then(Value::as_str) == Some(name)
            && folder
                .get("parentId")
                .and_then(Value::as_str)
                .unwrap_or_default()
                == parent_id
    }) {
        bail!("home_folder_name_conflict: {name} already exists under parent {parent_id}");
    }
    let id = Uuid::new_v4().hyphenated().to_string();
    let folder = json!({
        "createdTime": at,
        "id": id,
        "modifiedTime": at,
        "name": name,
        "parentId": parent_id
    });
    let before = counts(&documents)?;
    array_mut(&mut documents.folder_meta, "folders")?.push(folder.clone());
    set_timestamp(&mut documents.folder_meta, at)?;
    let after = counts(&documents)?;
    let snapshot = commit_documents(root, &documents)?;
    Ok(json!({
        "status": "created",
        "folder": folder,
        "before": before,
        "after": after,
        "snapshot_path": snapshot,
        "config_digest": digest_documents(root)?
    }))
}

/// 将精确 ID 对应的空叶子文件夹移入“最近删除”。
///
/// 当文件夹含子文件夹或草稿映射时，原生回收站 wire 格式尚未取得差分证据，
/// 因此必须失败关闭而不是猜测结构。
pub fn recycle(root: &Path, folder_id: &str, at: &str) -> Result<Value> {
    ensure_safe_mutation_target(root)?;
    let mut documents = load_documents(root)?;
    let matching: Vec<Value> = array(&documents.folder_meta, "folders")?
        .iter()
        .filter(|folder| folder.get("id").and_then(Value::as_str) == Some(folder_id))
        .cloned()
        .collect();
    if matching.len() != 1 {
        bail!(
            "home_folder_id_ambiguous: expected exactly one active folder id {folder_id}, found {}",
            matching.len()
        );
    }
    let has_children = array(&documents.folder_meta, "folders")?
        .iter()
        .any(|folder| folder.get("parentId").and_then(Value::as_str) == Some(folder_id));
    let has_drafts = array(&documents.draft_mappings, "mappings")?
        .iter()
        .any(|mapping| mapping.get("folderId").and_then(Value::as_str) == Some(folder_id));
    if has_children || has_drafts {
        bail!(
            "nonempty_folder_schema_unverified: folder {folder_id} has child folders or draft mappings; source files remain unchanged"
        );
    }
    let folder = matching.into_iter().next().expect("exact match checked");
    let before = counts(&documents)?;
    array_mut(&mut documents.folder_meta, "folders")?
        .retain(|candidate| candidate.get("id").and_then(Value::as_str) != Some(folder_id));
    let recycle_id = format!("{{{}}}", Uuid::new_v4().hyphenated());
    array_mut(&mut documents.recycle_bin, "recycled_folders")?.push(json!({
        "createdTime": at,
        "drafts": [],
        "folder_info": folder,
        "recycle_id": recycle_id,
        "sub_folders": []
    }));
    set_timestamp(&mut documents.folder_meta, at)?;
    set_timestamp(&mut documents.recycle_bin, at)?;
    let after = counts(&documents)?;
    let snapshot = commit_documents(root, &documents)?;
    Ok(json!({
        "status": "recycled",
        "folder_id": folder_id,
        "recycle_id": recycle_id,
        "before": before,
        "after": after,
        "snapshot_path": snapshot,
        "config_digest": digest_documents(root)?
    }))
}

/// 按精确回收 ID 恢复一个已观测格式的空叶子文件夹。
pub fn restore(root: &Path, recycle_id: &str, at: &str) -> Result<Value> {
    ensure_safe_mutation_target(root)?;
    let mut documents = load_documents(root)?;
    let matching: Vec<Value> = array(&documents.recycle_bin, "recycled_folders")?
        .iter()
        .filter(|entry| entry.get("recycle_id").and_then(Value::as_str) == Some(recycle_id))
        .cloned()
        .collect();
    if matching.len() != 1 {
        bail!(
            "home_recycle_id_ambiguous: expected exactly one recycle id {recycle_id}, found {}",
            matching.len()
        );
    }
    let entry = matching.into_iter().next().expect("exact match checked");
    if !is_empty_array(entry.get("drafts")) || !is_empty_array(entry.get("sub_folders")) {
        bail!(
            "nonempty_folder_schema_unverified: recycle entry {recycle_id} contains drafts or sub-folders; source files remain unchanged"
        );
    }
    let folder = entry
        .get("folder_info")
        .and_then(Value::as_object)
        .cloned()
        .map(Value::Object)
        .ok_or_else(|| anyhow::anyhow!("home_recycle_entry_invalid: folder_info is missing"))?;
    let folder_id = required_string(&folder, "id")?.to_owned();
    let parent_id = folder
        .get("parentId")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if count_id(array(&documents.folder_meta, "folders")?, &folder_id) != 0 {
        bail!("home_folder_id_conflict: active folder id {folder_id} already exists");
    }
    if !parent_id.is_empty() && count_id(array(&documents.folder_meta, "folders")?, parent_id) != 1
    {
        bail!("home_folder_parent_not_found: cannot restore under parent id {parent_id}");
    }
    let name = required_string(&folder, "name")?;
    if array(&documents.folder_meta, "folders")?
        .iter()
        .any(|candidate| {
            candidate.get("name").and_then(Value::as_str) == Some(name)
                && candidate
                    .get("parentId")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    == parent_id
        })
    {
        bail!("home_folder_name_conflict: {name} already exists under parent {parent_id}");
    }
    let before = counts(&documents)?;
    array_mut(&mut documents.folder_meta, "folders")?.push(folder);
    array_mut(&mut documents.recycle_bin, "recycled_folders")?.retain(|candidate| {
        candidate.get("recycle_id").and_then(Value::as_str) != Some(recycle_id)
    });
    set_timestamp(&mut documents.folder_meta, at)?;
    set_timestamp(&mut documents.recycle_bin, at)?;
    let after = counts(&documents)?;
    let snapshot = commit_documents(root, &documents)?;
    Ok(json!({
        "status": "restored",
        "folder_id": folder_id,
        "recycle_id": recycle_id,
        "before": before,
        "after": after,
        "snapshot_path": snapshot,
        "config_digest": digest_documents(root)?
    }))
}

struct Documents {
    folder_meta: Value,
    draft_mappings: Value,
    recycle_bin: Value,
    recycled_mappings: Value,
}

fn validate_config_root(root: &Path) -> Result<()> {
    let metadata = std::fs::symlink_metadata(root)
        .with_context(|| format!("reading home config root {}", root.display()))?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        bail!(
            "home_config_root_unsafe: {} must be a real directory",
            root.display()
        );
    }
    for name in REQUIRED_FILES {
        let path = root.join(name);
        let metadata = std::fs::symlink_metadata(&path)
            .with_context(|| format!("required home config file is missing: {}", path.display()))?;
        if metadata.file_type().is_symlink() || !metadata.is_file() {
            bail!(
                "home_config_file_unsafe: {} must be a regular file",
                path.display()
            );
        }
    }
    Ok(())
}

fn load_documents(root: &Path) -> Result<Documents> {
    validate_config_root(root)?;
    let documents = Documents {
        folder_meta: read_json(&root.join(FOLDER_META))?,
        draft_mappings: read_json(&root.join(DRAFT_MAPPINGS))?,
        recycle_bin: read_json(&root.join(RECYCLE_BIN))?,
        recycled_mappings: read_json(&root.join(RECYCLED_MAPPINGS))?,
    };
    validate_documents(&documents)?;
    Ok(documents)
}

fn read_json(path: &Path) -> Result<Value> {
    serde_json::from_slice(&std::fs::read(path)?)
        .with_context(|| format!("parsing {}", path.display()))
}

fn validate_documents(documents: &Documents) -> Result<()> {
    validate_document(&documents.folder_meta, "folders", FOLDER_META)?;
    validate_document(&documents.draft_mappings, "mappings", DRAFT_MAPPINGS)?;
    validate_document(&documents.recycle_bin, "recycled_folders", RECYCLE_BIN)?;
    validate_document(
        &documents.recycled_mappings,
        "recycled_draft_mappings",
        RECYCLED_MAPPINGS,
    )?;
    let mut active_ids = BTreeSet::new();
    for folder in array(&documents.folder_meta, "folders")? {
        let id = required_string(folder, "id")?;
        required_string(folder, "name")?;
        if !active_ids.insert(id.to_owned()) {
            bail!("home_folder_id_conflict: duplicate active folder id {id}");
        }
    }
    let mut recycle_ids = BTreeSet::new();
    for entry in array(&documents.recycle_bin, "recycled_folders")? {
        let id = required_string(entry, "recycle_id")?;
        if !recycle_ids.insert(id.to_owned()) {
            bail!("home_recycle_id_conflict: duplicate recycle id {id}");
        }
        if entry
            .get("folder_info")
            .and_then(Value::as_object)
            .is_none()
        {
            bail!("home_recycle_entry_invalid: recycle entry {id} has no folder_info object");
        }
    }
    Ok(())
}

fn validate_document(value: &Value, array_key: &str, file: &str) -> Result<()> {
    let object = value
        .as_object()
        .ok_or_else(|| anyhow::anyhow!("home_config_invalid: {file} root must be an object"))?;
    if object.get("version").and_then(Value::as_str) != Some("1.0") {
        bail!("home_config_version_unsupported: {file} requires observed version 1.0");
    }
    if object.get(array_key).and_then(Value::as_array).is_none() {
        bail!("home_config_invalid: {file}.{array_key} must be an array");
    }
    Ok(())
}

fn array<'a>(value: &'a Value, key: &str) -> Result<&'a Vec<Value>> {
    value
        .get(key)
        .and_then(Value::as_array)
        .ok_or_else(|| anyhow::anyhow!("home_config_invalid: {key} must be an array"))
}

fn array_mut<'a>(value: &'a mut Value, key: &str) -> Result<&'a mut Vec<Value>> {
    value
        .get_mut(key)
        .and_then(Value::as_array_mut)
        .ok_or_else(|| anyhow::anyhow!("home_config_invalid: {key} must be an array"))
}

fn required_string<'a>(value: &'a Value, key: &str) -> Result<&'a str> {
    value
        .get(key)
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| anyhow::anyhow!("home_config_invalid: {key} must be a non-empty string"))
}

fn count_id(folders: &[Value], id: &str) -> usize {
    folders
        .iter()
        .filter(|folder| folder.get("id").and_then(Value::as_str) == Some(id))
        .count()
}

fn validate_name(name: &str) -> Result<()> {
    if name.trim().is_empty() || name.chars().any(char::is_control) {
        bail!("home_folder_name_invalid: name must be non-empty and contain no control characters");
    }
    Ok(())
}

fn is_empty_array(value: Option<&Value>) -> bool {
    value.and_then(Value::as_array).is_some_and(Vec::is_empty)
}

fn set_timestamp(value: &mut Value, at: &str) -> Result<()> {
    let object = value
        .as_object_mut()
        .ok_or_else(|| anyhow::anyhow!("home_config_invalid: document root must be an object"))?;
    object.insert("timestamp".to_owned(), Value::String(at.to_owned()));
    Ok(())
}

fn counts(documents: &Documents) -> Result<Value> {
    Ok(json!({
        "active_folders": array(&documents.folder_meta, "folders")?.len(),
        "draft_mappings": array(&documents.draft_mappings, "mappings")?.len(),
        "recycled_folders": array(&documents.recycle_bin, "recycled_folders")?.len(),
        "recycled_draft_mappings": array(&documents.recycled_mappings, "recycled_draft_mappings")?.len()
    }))
}

fn ensure_safe_mutation_target(root: &Path) -> Result<()> {
    if is_platform_config_root(root) {
        let running = crate::store::editors_running();
        if !running.is_empty() {
            bail!(
                "editor_running: close JianYing/CapCut before modifying the real homepage config ({})",
                running.join(", ")
            );
        }
    }
    Ok(())
}

fn is_platform_config_root(root: &Path) -> bool {
    let actual = std::fs::canonicalize(root).ok();
    config_root_candidates().into_iter().any(|(_, candidate)| {
        candidate.is_dir()
            && std::fs::canonicalize(candidate)
                .ok()
                .is_some_and(|candidate| actual.as_ref().is_some_and(|actual| *actual == candidate))
    })
}

fn commit_documents(root: &Path, documents: &Documents) -> Result<PathBuf> {
    commit_documents_with_activation(root, documents, || Ok(()))
}

fn commit_documents_with_activation<F>(
    root: &Path,
    documents: &Documents,
    before_activation: F,
) -> Result<PathBuf>
where
    F: FnOnce() -> Result<()>,
{
    validate_documents(documents)?;
    let parent = root
        .parent()
        .ok_or_else(|| anyhow::anyhow!("home_config_root_unsafe: root has no parent"))?;
    let root_name = root
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| anyhow::anyhow!("home_config_root_unsafe: root has no UTF-8 name"))?;
    let transaction_id = Uuid::new_v4().simple().to_string();
    let transaction_root = parent
        .join(".jianying-home-folder-transactions")
        .join(&transaction_id);
    let snapshot = transaction_root.join("snapshot");
    let work_copy = transaction_root.join("work-copy");
    std::fs::create_dir_all(&transaction_root)?;
    copy_directory(root, &snapshot)?;
    copy_directory(root, &work_copy)?;
    write_json(&work_copy.join(FOLDER_META), &documents.folder_meta)?;
    write_json(&work_copy.join(DRAFT_MAPPINGS), &documents.draft_mappings)?;
    write_json(&work_copy.join(RECYCLE_BIN), &documents.recycle_bin)?;
    write_json(
        &work_copy.join(RECYCLED_MAPPINGS),
        &documents.recycled_mappings,
    )?;
    load_documents(&work_copy).context("validating staged homepage config")?;

    let rollback = parent.join(format!(
        ".{root_name}.jianying-home-folder-rollback-{transaction_id}"
    ));
    std::fs::rename(root, &rollback).with_context(|| {
        format!(
            "home_config_commit_failed: moving {} to rollback",
            root.display()
        )
    })?;
    let activation = before_activation().and_then(|_| {
        std::fs::rename(&work_copy, root).context("renaming staged homepage config into place")
    });
    if let Err(error) = activation {
        let rollback_result = std::fs::rename(&rollback, root);
        match rollback_result {
            Ok(()) => {
                bail!("home_config_commit_failed: activating work copy: {error}; source restored")
            }
            Err(rollback_error) => bail!(
                "home_config_commit_failed: activating work copy: {error}; automatic rollback failed: {rollback_error}; snapshot={}",
                snapshot.display()
            ),
        }
    }
    if let Err(error) = load_documents(root) {
        let failed = parent.join(format!(
            ".{root_name}.jianying-home-folder-invalid-{transaction_id}"
        ));
        let _ = std::fs::rename(root, &failed);
        let restore_result = std::fs::rename(&rollback, root);
        match restore_result {
            Ok(()) => {
                let _ = std::fs::remove_dir_all(&failed);
                bail!("home_config_readback_failed: {error}; source restored")
            }
            Err(rollback_error) => bail!(
                "home_config_readback_failed: {error}; automatic rollback failed: {rollback_error}; snapshot={}",
                snapshot.display()
            ),
        }
    }
    std::fs::remove_dir_all(&rollback)?;
    Ok(snapshot)
}

fn copy_directory(source: &Path, destination: &Path) -> Result<()> {
    std::fs::create_dir_all(destination)?;
    for entry in std::fs::read_dir(source)? {
        let entry = entry?;
        let metadata = entry.metadata()?;
        let file_type = entry.file_type()?;
        if file_type.is_symlink() {
            bail!("home_config_symlink_rejected: {}", entry.path().display());
        }
        let target = destination.join(entry.file_name());
        if metadata.is_dir() {
            copy_directory(&entry.path(), &target)?;
        } else if metadata.is_file() {
            std::fs::copy(entry.path(), target)?;
        } else {
            bail!(
                "home_config_special_file_rejected: {}",
                entry.path().display()
            );
        }
    }
    Ok(())
}

fn write_json(path: &Path, value: &Value) -> Result<()> {
    std::fs::write(path, serde_json::to_vec_pretty(value)?)?;
    Ok(())
}

fn digest_documents(root: &Path) -> Result<String> {
    let mut digest = Sha256::new();
    for name in REQUIRED_FILES {
        digest.update(name.as_bytes());
        digest.update([0]);
        digest.update(std::fs::read(root.join(name))?);
        digest.update([0]);
    }
    Ok(format!("{:x}", digest.finalize()))
}

#[cfg(test)]
mod tests {
    use super::{commit_documents_with_activation, load_documents, REQUIRED_FILES};
    use anyhow::bail;
    use serde_json::json;

    #[test]
    fn activation_failure_restores_all_original_files() {
        let parent = std::env::temp_dir().join(format!(
            "jianying-home-folder-rollback-{}-{}",
            std::process::id(),
            uuid::Uuid::new_v4().simple()
        ));
        let root = parent.join("LocalDraftFolder");
        std::fs::create_dir_all(&root).unwrap();
        let fixtures = [
            (
                "folder_meta_info.json",
                json!({"version":"1.0","timestamp":"before","folders":[],"unknown":1}),
            ),
            (
                "draft_folder_mappings.json",
                json!({"version":"1.0","timestamp":"before","mappings":[],"unknown":2}),
            ),
            (
                "recycle_bin.json",
                json!({"version":"1.0","timestamp":"before","recycled_folders":[],"unknown":3}),
            ),
            (
                "draft_mapping_recycle_bin.json",
                json!({"version":"1.0","timestamp":"before","recycled_draft_mappings":[],"unknown":4}),
            ),
        ];
        for (name, value) in fixtures {
            std::fs::write(root.join(name), serde_json::to_vec_pretty(&value).unwrap()).unwrap();
        }
        let before: Vec<Vec<u8>> = REQUIRED_FILES
            .iter()
            .map(|name| std::fs::read(root.join(name)).unwrap())
            .collect();
        let mut documents = load_documents(&root).unwrap();
        documents.folder_meta["timestamp"] = json!("after");

        let error = commit_documents_with_activation(&root, &documents, || {
            bail!("injected activation failure")
        })
        .unwrap_err();
        assert!(error.to_string().contains("source restored"));
        for (index, name) in REQUIRED_FILES.iter().enumerate() {
            assert_eq!(std::fs::read(root.join(name)).unwrap(), before[index]);
        }
        let _ = std::fs::remove_dir_all(parent);
    }
}
