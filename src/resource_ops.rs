//! 用户资源库：把草稿里已经用过的素材沉淀成可检索、可复用的目录。
//!
//! 剪映的音乐库、花字库、特效库都在联网接口后面（需要登录态与请求签名），
//! 安装目录里没有可直接读取的目录文件；但用户自己的草稿里带着真实用过的
//! `effect_id`、`resource_id`、`md5` 与名称快照。本模块把这些事实聚合起来，
//! 形成「个人资源库」，供计划与时间线命令按名字或 id 复用。
//!
//! 目录以「来源」为单位保存：同一次扫描重复写入是幂等的（按来源 id 覆盖），
//! 查询时再跨来源合并，避免重复扫描把使用次数越滚越大。

use anyhow::{Context, Result};
use serde_json::{json, Map, Value};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// 资源库文件 schema。
pub const SCHEMA: &str = "jianying-resource-library/v1";

/// 素材桶：这些数组里的对象才代表「库里的资源」，用户的原始媒体不在其中。
const MATERIAL_BUCKETS: &[&str] = &[
    "effects",
    "filters",
    "transitions",
    "stickers",
    "texts",
    "audio_effects",
    "video_effects",
];

/// 默认资源库路径：`JIANYING_CLI_HOME` > 平台标准配置目录。
pub fn default_catalog_path() -> PathBuf {
    if let Some(home) = std::env::var_os("JIANYING_CLI_HOME") {
        return PathBuf::from(home).join("resources.json");
    }
    #[cfg(windows)]
    if let Some(appdata) = std::env::var_os("APPDATA") {
        return PathBuf::from(appdata)
            .join("jianying-cli")
            .join("resources.json");
    }
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    home.join(".config")
        .join("jianying-cli")
        .join("resources.json")
}

/// 空目录骨架。
pub fn empty_catalog() -> Value {
    json!({
        "schema": SCHEMA,
        "sources": [],
    })
}

/// 读取资源库；文件不存在时返回空目录（不是错误）。
pub fn load(path: &Path) -> Result<Value> {
    if !path.exists() {
        return Ok(empty_catalog());
    }
    let raw = std::fs::read_to_string(path)
        .with_context(|| format!("cannot read resource catalog {}", path.display()))?;
    let value: Value = serde_json::from_str(&raw)
        .with_context(|| format!("resource catalog {} is not valid JSON", path.display()))?;
    if value["schema"].as_str() != Some(SCHEMA) {
        anyhow::bail!(
            "{} is not a {SCHEMA} file (schema={:?})",
            path.display(),
            value["schema"]
        );
    }
    Ok(value)
}

/// 写回资源库，固定 UTF-8 + LF，便于 diff 与跨平台一致。
pub fn save(path: &Path, catalog: &Value) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("cannot create {}", parent.display()))?;
    }
    let payload = serde_json::to_string_pretty(catalog)? + "\n";
    std::fs::write(path, payload)
        .with_context(|| format!("cannot write resource catalog {}", path.display()))?;
    Ok(())
}

/// 从一棵草稿目录树里抽取资源使用情况。
///
/// 只读取 `draft_content.json`，不改动任何草稿。
pub fn mine(root: &Path) -> Result<Value> {
    if !root.is_dir() {
        anyhow::bail!("draft root {} is not a directory", root.display());
    }
    let mut drafts = 0_u64;
    let mut with_resources = 0_u64;
    let mut encrypted = 0_u64;
    let mut unreadable = 0_u64;
    let mut entries: BTreeMap<String, Entry> = BTreeMap::new();
    let mut seen_ids: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let read = match std::fs::read_dir(&dir) {
            Ok(read) => read,
            Err(_) => continue,
        };
        for item in read.flatten() {
            let path = item.path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            if path.file_name().and_then(|name| name.to_str()) != Some("draft_content.json") {
                continue;
            }
            // `Timelines/<uuid>/draft_content.json` 是同一草稿的镜像副本，跳过以免重复计数。
            if path
                .components()
                .any(|component| component.as_os_str() == "Timelines")
            {
                continue;
            }
            let Ok(bytes) = std::fs::read(&path) else {
                continue;
            };
            drafts += 1;
            // 剪映 11.6 保存的草稿是加密的（首字节不是 `{`），CLI 不负责解密，
            // 这里只如实计数，避免把「读不了」伪装成「没有资源」。
            let raw = String::from_utf8_lossy(&bytes);
            let raw = raw.trim_start_matches('\u{feff}');
            let content: Value = match serde_json::from_str(raw) {
                Ok(value) => value,
                Err(_) => {
                    if looks_encrypted(raw) {
                        encrypted += 1;
                    } else {
                        unreadable += 1;
                    }
                    continue;
                }
            };
            // 同一草稿可能出现在多个位置（例如备份目录），按草稿 id 去重。
            if let Some(id) = content["id"].as_str() {
                if !seen_ids.insert(id.to_owned()) {
                    drafts -= 1;
                    continue;
                }
            }
            let draft_seen = content["update_time"]
                .as_i64()
                .or_else(|| content["create_time"].as_i64())
                .unwrap_or(0);
            let draft_path = path
                .parent()
                .map(|parent| parent.to_string_lossy().into_owned())
                .unwrap_or_default();
            let mut found = 0_u64;
            for bucket in MATERIAL_BUCKETS {
                for material in content["materials"][bucket]
                    .as_array()
                    .into_iter()
                    .flatten()
                {
                    let Some(key) = entry_key(material) else {
                        continue;
                    };
                    found += 1;
                    entries
                        .entry(key)
                        .or_insert_with(|| Entry::from_material(material))
                        .observe(draft_seen, &draft_path);
                }
            }
            if found > 0 {
                with_resources += 1;
            }
        }
    }

    let mut list: Vec<Value> = entries.into_values().map(Entry::into_value).collect();
    list.sort_by(|left, right| {
        right["uses"]
            .as_u64()
            .cmp(&left["uses"].as_u64())
            .then_with(|| left["key"].as_str().cmp(&right["key"].as_str()))
    });
    let mut kinds: BTreeMap<&str, u64> = BTreeMap::new();
    for entry in &list {
        if let Some(kind) = entry["kind"].as_str() {
            *kinds.entry(kind).or_default() += 1;
        }
    }
    Ok(json!({
        "root": root.to_string_lossy(),
        "drafts_scanned": drafts,
        "drafts_with_resources": with_resources,
        "drafts_encrypted": encrypted,
        "drafts_unreadable": unreadable,
        "entries": list,
        "kinds": kinds,
    }))
}

/// 粗略判断一段文本是否是剪映的加密草稿（不是 JSON，且整体像 base64）。
///
/// 只用于给出可读的诊断计数；解密不在本工具职责内。
fn looks_encrypted(text: &str) -> bool {
    let head: String = text.chars().take(96).collect();
    let trimmed = head.trim();
    if trimmed.is_empty() || trimmed.starts_with('{') || trimmed.starts_with('[') {
        return false;
    }
    trimmed
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '/' | '=' | '\r' | '\n'))
}

fn entry_key(material: &Value) -> Option<String> {
    let effect_id = material["effect_id"].as_str().unwrap_or("");
    let resource_id = material["resource_id"].as_str().unwrap_or("");
    if effect_id.is_empty() && resource_id.is_empty() {
        return None;
    }
    let kind = material["type"]
        .as_str()
        .or_else(|| material["panel"].as_str())
        .unwrap_or("unknown");
    Some(format!("{kind}\u{1}{effect_id}\u{1}{resource_id}"))
}

struct Entry {
    key: String,
    kind: String,
    name: String,
    effect_id: String,
    resource_id: String,
    md5: String,
    uses: u64,
    drafts: u64,
    last_seen: i64,
    sample_draft: String,
}

impl Entry {
    fn from_material(material: &Value) -> Self {
        Self {
            key: entry_key(material).unwrap_or_default(),
            kind: material["type"].as_str().unwrap_or("unknown").to_owned(),
            name: material["name"].as_str().unwrap_or("").to_owned(),
            effect_id: material["effect_id"].as_str().unwrap_or("").to_owned(),
            resource_id: material["resource_id"].as_str().unwrap_or("").to_owned(),
            md5: material["md5"].as_str().unwrap_or("").to_owned(),
            uses: 0,
            drafts: 0,
            last_seen: 0,
            sample_draft: String::new(),
        }
    }

    fn observe(&mut self, seen: i64, draft: &str) {
        self.uses += 1;
        self.drafts += 1; // 每个草稿只统计一次：调用方按草稿去重
        self.last_seen = self.last_seen.max(seen);
        if self.sample_draft.is_empty() {
            self.sample_draft = draft.to_owned();
        }
    }

    fn into_value(self) -> Value {
        json!({
            "key": self.key,
            "kind": self.kind,
            "name": self.name,
            "effect_id": self.effect_id,
            "resource_id": self.resource_id,
            "md5": self.md5,
            "uses": self.uses,
            "drafts": self.drafts,
            "last_seen": self.last_seen,
            "sample_draft": self.sample_draft,
        })
    }
}

/// 把一次扫描结果写成目录里的一个来源（同 id 覆盖，幂等）。
pub fn put_source(
    catalog: &mut Value,
    source_id: &str,
    mined: &Value,
    scanned_at: i64,
) -> Result<()> {
    let sources = catalog["sources"]
        .as_array_mut()
        .context("catalog.sources must be an array")?;
    sources.retain(|source| source["id"].as_str() != Some(source_id));
    sources.push(json!({
        "id": source_id,
        "scanned_at": scanned_at,
        "root": mined["root"],
        "drafts_scanned": mined["drafts_scanned"],
        "entries": mined["entries"],
    }));
    Ok(())
}

/// 跨来源合并后的视图：uses/drafts 求和，last_seen 取最大。
pub fn merged_entries(catalog: &Value) -> Vec<Value> {
    let mut merged: BTreeMap<String, Value> = BTreeMap::new();
    for source in catalog["sources"].as_array().into_iter().flatten() {
        for entry in source["entries"].as_array().into_iter().flatten() {
            let Some(key) = entry["key"].as_str() else {
                continue;
            };
            let slot = merged.entry(key.to_owned()).or_insert_with(|| {
                let mut clone = entry.clone();
                clone["uses"] = json!(0);
                clone["drafts"] = json!(0);
                clone["sources"] = json!([]);
                clone
            });
            let uses = slot["uses"].as_u64().unwrap_or(0) + entry["uses"].as_u64().unwrap_or(0);
            let drafts =
                slot["drafts"].as_u64().unwrap_or(0) + entry["drafts"].as_u64().unwrap_or(0);
            slot["uses"] = json!(uses);
            slot["drafts"] = json!(drafts);
            if entry["last_seen"].as_i64().unwrap_or(0) > slot["last_seen"].as_i64().unwrap_or(0) {
                slot["last_seen"] = entry["last_seen"].clone();
            }
            if let Some(id) = source["id"].as_str() {
                if let Some(list) = slot["sources"].as_array_mut() {
                    if !list.iter().any(|value| value.as_str() == Some(id)) {
                        list.push(json!(id));
                    }
                }
            }
        }
    }
    let mut list: Vec<Value> = merged.into_values().collect();
    list.sort_by(|left, right| {
        right["uses"]
            .as_u64()
            .cmp(&left["uses"].as_u64())
            .then_with(|| left["kind"].as_str().cmp(&right["kind"].as_str()))
            .then_with(|| left["name"].as_str().cmp(&right["name"].as_str()))
    });
    list
}

/// 过滤：按种类与名称/id 子串匹配（大小写不敏感）。
pub fn filter<'a>(
    entries: &'a [Value],
    kind: Option<&str>,
    needle: Option<&str>,
) -> Vec<&'a Value> {
    let needle = needle.map(str::to_lowercase);
    entries
        .iter()
        .filter(|entry| {
            if let Some(kind) = kind {
                if entry["kind"].as_str() != Some(kind) {
                    return false;
                }
            }
            match &needle {
                None => true,
                Some(needle) => ["name", "effect_id", "resource_id", "kind"]
                    .iter()
                    .any(|field| {
                        entry[*field]
                            .as_str()
                            .map(|value| value.to_lowercase().contains(needle))
                            .unwrap_or(false)
                    }),
            }
        })
        .collect()
}

/// 解析唯一匹配；0 个或多个匹配都报错，并给出可读的候选列表。
pub fn resolve_one<'a>(
    entries: &'a [Value],
    needle: &str,
    kind: Option<&str>,
) -> Result<&'a Value> {
    let found = filter(entries, kind, Some(needle));
    match found.len() {
        0 => anyhow::bail!("no resource matches {needle:?}"),
        1 => Ok(found[0]),
        count => {
            let candidates: Vec<String> = found
                .iter()
                .take(6)
                .map(|entry| {
                    format!(
                        "{}:{}",
                        entry["kind"].as_str().unwrap_or("?"),
                        entry["name"].as_str().unwrap_or("(no name)")
                    )
                })
                .collect();
            anyhow::bail!(
                "{needle:?} is ambiguous ({count} matches: {}); narrow it with --kind",
                candidates.join(", ")
            )
        }
    }
}

/// 把外部 bundle（`{entries:[...]}` 或完整目录）并进资源库。
pub fn import_bundle(
    catalog: &mut Value,
    bundle: &Value,
    source_id: &str,
    now: i64,
) -> Result<usize> {
    let entries = bundle["entries"]
        .as_array()
        .context("bundle must contain an entries array")?;
    let mut kept = Vec::new();
    for entry in entries {
        if entry["type"].is_string() || entry["kind"].is_string() {
            let mut normalized = entry.clone();
            if normalized["kind"].is_null() {
                normalized["kind"] = normalized["type"].clone();
            }
            if normalized["key"].is_null() {
                let key = entry_key(&normalized).context("bundle entry has no resource ids")?;
                normalized["key"] = json!(key);
            }
            normalized["uses"] = normalized["uses"]
                .clone()
                .as_u64()
                .map_or(json!(0), |v| json!(v));
            normalized["drafts"] = json!(0);
            kept.push(normalized);
        }
    }
    put_source(
        catalog,
        source_id,
        &json!({"root": source_id, "drafts_scanned": 0, "entries": kept}),
        now,
    )?;
    Ok(kept.len())
}

/// 供 CLI 输出的目录摘要。
pub fn summarize(catalog: &Value) -> Value {
    let merged = merged_entries(catalog);
    let mut kinds: Map<String, Value> = Map::new();
    for entry in &merged {
        if let Some(kind) = entry["kind"].as_str() {
            let count = kinds.get(kind).and_then(Value::as_u64).unwrap_or(0) + 1;
            kinds.insert(kind.to_owned(), json!(count));
        }
    }
    json!({
        "sources": catalog["sources"].as_array().map(|s| s.len()).unwrap_or(0),
        "entries": merged.len(),
        "kinds": kinds,
    })
}
