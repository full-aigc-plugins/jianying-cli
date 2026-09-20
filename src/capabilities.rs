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
