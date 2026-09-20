//! 兼容性 fixture bundle 的脱敏导出与机械复检。

use anyhow::{bail, Context, Result};
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use thiserror::Error;

const TIMELINE_FILES: &[&str] = &[
    "draft_content.json",
    "draft_info.json",
    "draft_meta_info.json",
    "template-2.tmp",
];
const MASK_ARRAY_KEYS: &[&str] = &["masks", "common_mask", "common_masks"];
const CLI_MASK_ENTRY_KEYS: &[&str] = &[
    "config",
    "category",
    "category_id",
    "category_name",
    "id",
    "name",
    "platform",
    "position_info",
    "resource_type",
    "resource_id",
    "type",
];
const KNOWN_PROPERTY_TYPES: &[&str] = &[
    "KFTypePositionX",
    "KFTypePositionY",
    "KFTypeRotation",
    "KFTypeScaleX",
    "KFTypeScaleY",
    "KFTypeAlpha",
    "KFTypeSaturation",
    "KFTypeContrast",
    "KFTypeBrightness",
    "KFTypeVolume",
];

/// 脱敏复检发现，错误响应只携带位置和类型，不回显敏感值。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RedactionFinding {
    pub file: String,
    pub line: usize,
    pub kind: String,
}

/// 已完成 bundle 的机械隐私复检结果。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RedactionCheck {
    pub ok: bool,
    pub bundle_dir: String,
    pub files_scanned: usize,
    pub findings: Vec<RedactionFinding>,
}

/// Fixture 命令稳定失败类型。
#[derive(Debug, Error)]
pub enum FixtureError {
    #[error("fixture redaction check failed; review the reported locations before sharing")]
    RedactionFailed {
        bundle_dir: String,
        files_scanned: usize,
        findings: Vec<RedactionFinding>,
    },
}

/// 为真实草稿生成仅含时间线文本的脱敏兼容性 bundle。
pub fn build_bundle(draft: &Path, out: &Path) -> Result<Value> {
    if !draft.is_dir() {
        bail!("no draft found at {}", draft.display());
    }
    let diagnose = crate::project_ops::diagnose(draft, None)?;
    std::fs::create_dir_all(out)
        .with_context(|| format!("create fixture output {}", out.display()))?;
    if std::fs::canonicalize(draft)? == std::fs::canonicalize(out)? {
        bail!("fixture output must not be the source draft directory");
    }

    let mut tally = BTreeMap::<String, usize>::new();
    let mut file_reports = Vec::new();
    let mut bundled = Vec::<(String, String)>::new();
    let mut paths: Vec<(String, PathBuf)> = TIMELINE_FILES
        .iter()
        .map(|name| ((*name).to_owned(), draft.join(name)))
        .collect();
    for relative in crate::project_ops::nested_timeline_paths(draft)? {
        paths.push((relative.clone(), draft.join(&relative)));
    }
    for (relative, source) in paths {
        if !source.is_file() {
            continue;
        }
        let raw_bytes = std::fs::read(&source)?;
        let raw = String::from_utf8(raw_bytes)
            .with_context(|| format!("fixture accepts UTF-8 timeline text only: {relative}"))?;
        let raw = raw.trim_start_matches('\u{feff}');
        let (safe, redactions) = redact(raw, &mut tally)?;
        write_private(&out.join(&relative), safe.as_bytes())?;
        file_reports.push(json!({
            "file": relative,
            "bytes_in": raw.len(),
            "bytes_out": safe.len(),
            "redactions": redactions
        }));
        bundled.push((relative, safe));
    }
    if bundled.is_empty() {
        bail!("no timeline files found to bundle in: {}", draft.display());
    }

    let mask_report = build_mask_keyframe_report(&bundled);
    write_json_private(&out.join("mask-keyframe-report.json"), &mask_report)?;
    let mut diagnose_bundle = diagnose.clone();
    diagnose_bundle
        .as_object_mut()
        .map(|value| value.remove("bundle"));
    write_json_private(&out.join("diagnose.json"), &diagnose_bundle)?;

    let mask_summary = mask_report["summary"].clone();
    let mask_evidence = mask_summary["verdict"] == "mask-keyframe-evidence-found";
    let version = diagnose["version"].as_str();
    let modern_storage = diagnose["modern_storage"].as_bool().unwrap_or(false);
    let nested: Vec<String> = diagnose["nested_timelines"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(str::to_owned)
        .collect();
    write_private(
        &out.join("README.md"),
        reporter_readme(version, modern_storage, &nested, mask_evidence).as_bytes(),
    )?;

    let source_dir = redact_display_path(draft, &mut tally)?;
    let out_dir = redact_display_path(out, &mut tally)?;
    let notes = vec![
        "Binary media under assets/ was intentionally excluded — only timeline JSON files are bundled.",
        "User home paths, email addresses and device identifiers were redacted; review before sharing.",
        "mask-keyframe-report.json maps mask and keyframe structures without inventing an encoding.",
    ];
    let report = json!({
        "ok": true,
        "source_dir": source_dir,
        "out_dir": out_dir,
        "version": version,
        "modern_storage": modern_storage,
        "files": file_reports,
        "redaction_kinds": tally,
        "media_excluded": true,
        "mask_keyframe_evidence": mask_summary,
        "notes": notes
    });
    write_json_private(&out.join("SANITIZE_REPORT.json"), &report)?;
    Ok(report)
}

/// 扫描 bundle 内的文本文件；发现敏感形态时仅报告文件、行号和类型。
pub fn verify_bundle_redaction(bundle: &Path) -> Result<RedactionCheck> {
    if !bundle.is_dir() {
        bail!("fixture bundle is not a directory: {}", bundle.display());
    }
    let mut files = Vec::new();
    collect_text_files(bundle, bundle, &mut files)?;
    files.sort();
    let home = Regex::new(r#"(?:/Users/|/home/|[A-Za-z]:\\+Users\\+)([A-Za-z0-9._-]{2,})"#)?;
    let email = Regex::new(r#"[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,}"#)?;
    let device = Regex::new(
        r#"\\?\"(?:device_id|mac_address|hard_disk_id)\\?\"\s*:\s*\\?\"([^\"\\]{4,})\\?\""#,
    )?;
    let username = std::env::var("USER")
        .ok()
        .filter(|name| name.chars().count() >= 5);
    let username_pattern = username
        .as_deref()
        .map(regex::escape)
        .map(|name| Regex::new(&format!(r#"(?i)(^|[^A-Za-z0-9_]){name}([^A-Za-z0-9_]|$)"#)))
        .transpose()?;
    let mut findings = Vec::new();
    for path in &files {
        let relative = path
            .strip_prefix(bundle)
            .unwrap_or(path)
            .to_string_lossy()
            .replace('\\', "/");
        let text = std::fs::read_to_string(path)?;
        for (index, line) in text.lines().enumerate() {
            if home
                .captures(line)
                .and_then(|capture| capture.get(1))
                .is_some_and(|account| account.as_str() != "USER")
            {
                push_finding(&mut findings, &relative, index + 1, "home-path");
            } else if username_pattern
                .as_ref()
                .is_some_and(|pattern| pattern.is_match(line))
            {
                push_finding(&mut findings, &relative, index + 1, "username");
            }
            if email
                .find_iter(line)
                .any(|value| !value.as_str().eq_ignore_ascii_case("redacted@example.com"))
            {
                push_finding(&mut findings, &relative, index + 1, "email");
            }
            if device
                .captures_iter(line)
                .filter_map(|capture| capture.get(1))
                .any(|value| value.as_str() != "redacted")
            {
                push_finding(&mut findings, &relative, index + 1, "device-key");
            }
        }
    }
    let mut tally = BTreeMap::new();
    let bundle_dir = redact_display_path(bundle, &mut tally)?;
    Ok(RedactionCheck {
        ok: findings.is_empty(),
        bundle_dir,
        files_scanned: files.len(),
        findings,
    })
}

fn push_finding(findings: &mut Vec<RedactionFinding>, file: &str, line: usize, kind: &str) {
    let finding = RedactionFinding {
        file: file.to_owned(),
        line,
        kind: kind.to_owned(),
    };
    if !findings.contains(&finding) {
        findings.push(finding);
    }
}

fn redact(raw: &str, tally: &mut BTreeMap<String, usize>) -> Result<(String, usize)> {
    let patterns = [
        (
            "windows_user",
            r#"([A-Za-z]:\\+Users\\+)[^\\/\"<>:|?*]+"#,
            "${1}USER",
        ),
        (
            "windows_user_fwd",
            r#"([A-Za-z]:/Users/)[^/\"<>:|?*]+"#,
            "${1}USER",
        ),
        ("macos_user", r#"(/Users/)[^/\"]+"#, "${1}USER"),
        ("linux_user", r#"(/home/)[^/\"]+"#, "${1}USER"),
        (
            "email",
            r#"[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,}"#,
            "redacted@example.com",
        ),
    ];
    let mut text = raw.to_owned();
    let mut total = 0;
    for (kind, pattern, replacement) in patterns {
        let expression = Regex::new(pattern)?;
        let count = expression.find_iter(&text).count();
        if count > 0 {
            *tally.entry(kind.to_owned()).or_default() += count;
            total += count;
            text = expression.replace_all(&text, replacement).into_owned();
        }
    }
    for pattern in [
        r#"(\"(?:device_id|mac_address|hard_disk_id)\"\s*:\s*\")[^\"]+(\")"#,
        r#"(\\\"(?:device_id|mac_address|hard_disk_id)\\\"\s*:\s*\\\")[^\"\\]+(\\\")"#,
    ] {
        let expression = Regex::new(pattern)?;
        let count = expression.find_iter(&text).count();
        if count > 0 {
            *tally.entry("device_ids".to_owned()).or_default() += count;
            total += count;
            text = expression.replace_all(&text, "${1}${2}").into_owned();
        }
    }
    Ok((text, total))
}

fn redact_display_path(path: &Path, tally: &mut BTreeMap<String, usize>) -> Result<String> {
    Ok(redact(&path.to_string_lossy(), tally)?.0)
}

fn collect_text_files(root: &Path, current: &Path, output: &mut Vec<PathBuf>) -> Result<()> {
    let mut entries: Vec<_> = std::fs::read_dir(current)?.collect::<std::io::Result<_>>()?;
    entries.sort_by_key(|entry| entry.file_name());
    for entry in entries {
        let path = entry.path();
        let metadata = std::fs::symlink_metadata(&path)?;
        if metadata.file_type().is_symlink() {
            continue;
        }
        if metadata.is_dir() {
            collect_text_files(root, &path, output)?;
        } else if path
            .extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| {
                matches!(
                    extension.to_ascii_lowercase().as_str(),
                    "json" | "md" | "txt" | "tmp"
                )
            })
        {
            output.push(path);
        }
    }
    let _ = root;
    Ok(())
}

fn build_mask_keyframe_report(files: &[(String, String)]) -> Value {
    let known: HashSet<&str> = KNOWN_PROPERTY_TYPES.iter().copied().collect();
    let evidence: Vec<Value> = files
        .iter()
        .map(|(file, text)| harvest_file_evidence(file, text, &known))
        .collect();
    let masks_found: usize = evidence
        .iter()
        .map(|file| file["masks"].as_array().map(Vec::len).unwrap_or(0))
        .sum();
    let mut unknown_on_masked = BTreeSet::new();
    let mut shaped = false;
    for file in &evidence {
        for mask in file["masks"].as_array().into_iter().flatten() {
            shaped |= mask["keyframe_shaped_nodes"]
                .as_array()
                .is_some_and(|nodes| !nodes.is_empty());
        }
        for segment in file["segments_with_mask_and_keyframes"]
            .as_array()
            .into_iter()
            .flatten()
        {
            for property in segment["property_types"].as_array().into_iter().flatten() {
                if let Some(property) = property
                    .as_str()
                    .filter(|property| !known.contains(*property))
                {
                    unknown_on_masked.insert(property.to_owned());
                }
            }
        }
    }
    let verdict = if shaped || !unknown_on_masked.is_empty() {
        "mask-keyframe-evidence-found"
    } else {
        "no-mask-keyframe-evidence"
    };
    json!({
        "issue":"https://github.com/renezander030/capcut-cli/issues/44",
        "looking_for":[
            "mask geometry property_type identifiers in segment.common_keyframes",
            "or a keyframe container inside masks/common_mask/common_masks"
        ],
        "verdict":verdict,
        "summary":{
            "verdict":verdict,
            "masks_found":masks_found,
            "unknown_property_types_on_masked_segments":unknown_on_masked
        },
        "files":evidence
    })
}

#[derive(Default)]
struct Harvest {
    masks: Vec<Value>,
    mask_ids: HashSet<String>,
    property_types: BTreeSet<String>,
    segments: Vec<(Option<String>, Vec<String>, Vec<String>)>,
}

fn harvest_file_evidence(file: &str, text: &str, known: &HashSet<&str>) -> Value {
    let Ok(root) = serde_json::from_str::<Value>(text) else {
        return json!({"file":file,"parsed":false,"masks":[],
            "property_types":{"known":[],"unknown":[]},"segments_with_mask_and_keyframes":[]});
    };
    let mut harvest = Harvest::default();
    visit_evidence(&root, "$", None, false, &mut harvest);
    let found: Vec<String> = harvest.property_types.iter().cloned().collect();
    let segments: Vec<Value> = harvest
        .segments
        .iter()
        .filter_map(|(id, refs, properties)| {
            let mask_refs: Vec<&String> = refs
                .iter()
                .filter(|reference| harvest.mask_ids.contains(*reference))
                .collect();
            (!mask_refs.is_empty()).then(|| {
                json!({
                    "segment_id":id,
                    "mask_material_ids":mask_refs,
                    "property_types":properties.iter().collect::<BTreeSet<_>>()
                })
            })
        })
        .collect();
    json!({
        "file":file,
        "parsed":true,
        "masks":harvest.masks,
        "property_types":{
            "known":found.iter().filter(|property| known.contains(property.as_str())).collect::<Vec<_>>(),
            "unknown":found.iter().filter(|property| !known.contains(property.as_str())).collect::<Vec<_>>()
        },
        "segments_with_mask_and_keyframes":segments
    })
}

fn visit_evidence(
    node: &Value,
    path: &str,
    mask_index: Option<usize>,
    mask_element: bool,
    harvest: &mut Harvest,
) {
    if let Some(values) = node.as_array() {
        for (index, value) in values.iter().enumerate() {
            visit_evidence(
                value,
                &format!("{path}[{index}]"),
                mask_index,
                false,
                harvest,
            );
        }
        return;
    }
    if let Some(embedded) = node
        .as_str()
        .filter(|raw| raw.trim_start().starts_with(['{', '[']))
        .and_then(|raw| serde_json::from_str::<Value>(raw).ok())
    {
        visit_evidence(
            &embedded,
            &format!("{path}<embedded-json>"),
            mask_index,
            false,
            harvest,
        );
        return;
    }
    let Some(object) = node.as_object() else {
        return;
    };
    let mut current_mask = mask_index;
    if mask_element || object.get("type").and_then(Value::as_str) == Some("mask") {
        let config_keys: Vec<String> = object
            .get("config")
            .and_then(Value::as_object)
            .map(|config| config.keys().cloned().collect())
            .unwrap_or_default();
        let unrecognized: Vec<String> = object
            .keys()
            .filter(|key| !CLI_MASK_ENTRY_KEYS.contains(&key.as_str()))
            .cloned()
            .collect();
        if let Some(id) = object.get("id").and_then(Value::as_str) {
            harvest.mask_ids.insert(id.to_owned());
        }
        harvest.masks.push(json!({
            "json_path":path,
            "id":object.get("id").and_then(Value::as_str),
            "name":object.get("name").and_then(Value::as_str),
            "resource_type":object.get("resource_type").and_then(Value::as_str),
            "config_keys":config_keys,
            "unrecognized_keys":unrecognized,
            "keyframe_shaped_nodes":[]
        }));
        current_mask = Some(harvest.masks.len() - 1);
    }
    if let Some(property) = object.get("property_type").and_then(Value::as_str) {
        harvest.property_types.insert(property.to_owned());
    }
    if let Some(index) = current_mask {
        if object.contains_key("time_offset")
            || object.contains_key("keyframe_list")
            || object.get("property_type").is_some_and(Value::is_string)
        {
            if let Some(nodes) = harvest.masks[index]["keyframe_shaped_nodes"].as_array_mut() {
                nodes.push(Value::String(path.to_owned()));
            }
        }
    }
    if let (Some(keyframes), Some(references)) = (
        object.get("common_keyframes").and_then(Value::as_array),
        object.get("extra_material_refs").and_then(Value::as_array),
    ) {
        let properties: Vec<String> = keyframes
            .iter()
            .filter_map(|keyframe| keyframe["property_type"].as_str().map(str::to_owned))
            .collect();
        if !properties.is_empty() {
            harvest.segments.push((
                object.get("id").and_then(Value::as_str).map(str::to_owned),
                references
                    .iter()
                    .filter_map(Value::as_str)
                    .map(str::to_owned)
                    .collect(),
                properties,
            ));
        }
    }
    for (key, value) in object {
        if MASK_ARRAY_KEYS.contains(&key.as_str()) {
            if let Some(values) = value.as_array() {
                for (index, element) in values.iter().enumerate() {
                    visit_evidence(
                        element,
                        &format!("{path}.{key}[{index}]"),
                        current_mask,
                        true,
                        harvest,
                    );
                }
            }
        } else {
            visit_evidence(
                value,
                &format!("{path}.{key}"),
                current_mask,
                false,
                harvest,
            );
        }
    }
}

fn reporter_readme(
    version: Option<&str>,
    modern_storage: bool,
    nested: &[String],
    mask_evidence: bool,
) -> String {
    format!(
        "# Sanitized JianYing/CapCut draft bundle\n\n\
This folder contains only redacted timeline text. No media from `assets/` was copied.\n\n\
- Detected app version: {}\n\
- Modern storage layout (>= 8.7): {}\n\
- Nested Timelines files: {}\n\n\
Review every file before sharing. `mask-keyframe-report.json` {} mask-keyframe evidence.\n\
This bundle proves an on-disk shape only; it does not replace a real desktop open/play/export canary.\n",
        version.unwrap_or("unknown"),
        if modern_storage { "yes" } else { "no" },
        if nested.is_empty() { "none".to_owned() } else { nested.join(", ") },
        if mask_evidence { "contains" } else { "does not contain" }
    )
}

fn write_json_private(path: &Path, value: &Value) -> Result<()> {
    let mut encoded = serde_json::to_vec_pretty(value)?;
    encoded.push(b'\n');
    write_private(path, &encoded)
}

fn write_private(path: &Path, bytes: &[u8]) -> Result<()> {
    let parent = path
        .parent()
        .context("fixture output requires a parent directory")?;
    std::fs::create_dir_all(parent)?;
    let temporary = parent.join(format!(".fixture-{}.tmp", uuid::Uuid::new_v4().simple()));
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file: File = options.open(&temporary)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    drop(file);
    if let Err(error) = std::fs::rename(&temporary, path) {
        if error.kind() == std::io::ErrorKind::AlreadyExists && path.is_file() {
            std::fs::remove_file(path)?;
            std::fs::rename(&temporary, path)?;
        } else {
            let _ = std::fs::remove_file(&temporary);
            return Err(error.into());
        }
    }
    Ok(())
}
