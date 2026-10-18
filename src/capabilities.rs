use anyhow::{Context, Result};
use serde_json::Value;

const MANIFEST: &str = include_str!("../provenance/CAPABILITIES.json");

/// 返回编译进二进制的机器可读 capability manifest。
pub fn manifest() -> Result<Value> {
    let mut value: Value =
        serde_json::from_str(MANIFEST).context("embedded capability manifest is invalid")?;
    value["cli_version"] = Value::String(env!("CARGO_PKG_VERSION").to_owned());
    if let Some(contract_state) = option_env!("JIANYING_BUILD_CONTRACT_STATE") {
        if !contract_state.is_empty() {
            value["contract_state"] = Value::String(contract_state.to_owned());
        }
    }
    if let Some(release_ref) = option_env!("JIANYING_BUILD_RELEASE_REF") {
        value["release_ref"] = if release_ref.is_empty() {
            Value::Null
        } else {
            Value::String(release_ref.to_owned())
        };
    }
    if let Some(source_commit) = option_env!("JIANYING_BUILD_SOURCE_COMMIT") {
        value["source_commit"] = if source_commit.is_empty() {
            Value::Null
        } else {
            Value::String(source_commit.to_owned())
        };
    }
    Ok(value)
}

/// ??????????????????????
pub const DRAFT_SCHEMAS: [&str; 3] = ["draft_info.json", "draft_content.json", "*"];

/// ?? `5.9.0` ????????????? None?
fn parse_version(raw: &str) -> Option<(u32, u32, u32)> {
    let mut parts = raw.split('.');
    let major = parts.next()?.parse().ok()?;
    let minor = parts.next().unwrap_or("0").parse().ok()?;
    let patch = parts.next().unwrap_or("0").parse().ok()?;
    if parts.next().is_some() {
        return None;
    }
    Some((major, minor, patch))
}

/// ???? `applies_to`????????????
fn validate_applies_to(entry: &Value) -> Vec<String> {
    let mut problems = Vec::new();
    let Some(applies) = entry.get("applies_to") else {
        return problems;
    };
    let id = entry["id"].as_str().unwrap_or("<unknown>");
    if !applies.is_object() {
        problems.push(format!("{id}: applies_to must be an object"));
        return problems;
    }
    if let Some(schema) = applies.get("draft_schema") {
        let declared: Vec<&str> = match schema {
            Value::String(single) => vec![single.as_str()],
            Value::Array(items) => items.iter().filter_map(Value::as_str).collect(),
            _ => {
                problems.push(format!(
                    "{id}: applies_to.draft_schema must be a string or array"
                ));
                vec![]
            }
        };
        if declared.is_empty() {
            problems.push(format!("{id}: applies_to.draft_schema must not be empty"));
        }
        for item in declared {
            if !DRAFT_SCHEMAS.contains(&item) {
                problems.push(format!(
                    "{id}: unknown draft_schema {item:?} (expected one of {DRAFT_SCHEMAS:?})"
                ));
            }
        }
    }
    let since = applies.get("app_since").and_then(Value::as_str);
    let until = applies.get("app_until").and_then(Value::as_str);
    let mut parsed_since = None;
    if let Some(raw) = since {
        match parse_version(raw) {
            Some(version) => parsed_since = Some(version),
            None => problems.push(format!(
                "{id}: applies_to.app_since is not a dotted version: {raw:?}"
            )),
        }
    }
    if let Some(raw) = until {
        match parse_version(raw) {
            Some(version) => {
                if let Some(lower) = parsed_since {
                    if version < lower {
                        problems.push(format!(
                            "{id}: applies_to.app_until {raw:?} is lower than app_since"
                        ));
                    }
                }
            }
            None => problems.push(format!(
                "{id}: applies_to.app_until is not a dotted version: {raw:?}"
            )),
        }
    }
    problems
}

/// ??????????????????? applies_to ?????
fn applies_to_schema(entry: &Value, schema: &str) -> bool {
    let Some(applies) = entry.get("applies_to") else {
        return true;
    };
    let Some(declared) = applies.get("draft_schema") else {
        return true;
    };
    match declared {
        Value::String(single) => single == schema || single == "*",
        Value::Array(items) => items.iter().any(|item| {
            item.as_str()
                .map(|value| value == schema || value == "*")
                .unwrap_or(false)
        }),
        _ => true,
    }
}

/// ?????????????
///
/// `draft_schema` ??????? `applicable` ????????????
/// ????????? `applies_to` ?????????????????
pub fn manifest_for(draft_schema: Option<&str>) -> Result<Value> {
    let mut value = manifest()?;
    if let Some(schema) = draft_schema {
        if !DRAFT_SCHEMAS.contains(&schema) {
            anyhow::bail!("unknown draft schema {schema:?} (expected one of {DRAFT_SCHEMAS:?})");
        }
    }
    let entries = value["capabilities"]
        .as_array_mut()
        .context("manifest.capabilities must be an array")?;
    let mut problems: Vec<String> = Vec::new();
    let mut applicable = 0_i64;
    let mut not_applicable = 0_i64;
    let mut unspecified = 0_i64;
    for entry in entries.iter_mut() {
        problems.extend(validate_applies_to(entry));
        if entry.get("applies_to").is_none() {
            unspecified += 1;
        }
        if let Some(schema) = draft_schema {
            let applies = applies_to_schema(entry, schema);
            entry["applicable"] = Value::Bool(applies);
            if applies {
                applicable += 1;
            } else {
                not_applicable += 1;
            }
        }
    }
    if !problems.is_empty() {
        anyhow::bail!("capability applies_to is invalid: {}", problems.join("; "));
    }
    if let Some(schema) = draft_schema {
        value["draft_schema"] = Value::String(schema.to_owned());
        value["applicability"] = serde_json::json!({
            "applicable": applicable,
            "not_applicable": not_applicable,
            "unspecified": unspecified,
            "known_draft_schemas": DRAFT_SCHEMAS,
        });
    }
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::{applies_to_schema, parse_version, validate_applies_to};
    use serde_json::json;

    #[test]
    fn versions_parse_and_reject_junk() {
        assert_eq!(parse_version("5.9.0"), Some((5, 9, 0)));
        assert_eq!(parse_version("11.6"), Some((11, 6, 0)));
        assert_eq!(parse_version("5.9.0.1"), None);
        assert_eq!(parse_version("v5"), None);
    }

    #[test]
    fn applies_to_is_validated_and_evaluated() {
        let ok = json!({"id": "x", "applies_to": {"draft_schema": ["draft_info.json"], "app_since": "5.9.0"}});
        assert!(validate_applies_to(&ok).is_empty());
        assert!(applies_to_schema(&ok, "draft_info.json"));
        assert!(!applies_to_schema(&ok, "draft_content.json"));

        let bad_schema = json!({"id": "y", "applies_to": {"draft_schema": "draft_v9.json"}});
        assert!(!validate_applies_to(&bad_schema).is_empty());

        let inverted =
            json!({"id": "z", "applies_to": {"app_since": "11.0.0", "app_until": "9.0.0"}});
        assert!(!validate_applies_to(&inverted).is_empty());

        let wildcard = json!({"id": "w", "applies_to": {"draft_schema": "*"}});
        assert!(applies_to_schema(&wildcard, "draft_content.json"));
    }
}
