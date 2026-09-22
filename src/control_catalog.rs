//! 目标剪映版本的语义时间线控制目录。

use anyhow::{bail, Result};
use serde_json::{json, Value};
use std::collections::BTreeSet;

const CATALOG: &str = include_str!("../provenance/TIMELINE_CONTROLS.json");
const SURFACE_INVENTORY: &str = include_str!("../provenance/TIMELINE_SURFACE_INVENTORY.json");

/// 返回经结构校验的完整控制目录。
pub fn catalogue() -> Result<Value> {
    let value: Value = serde_json::from_str(CATALOG)?;
    validate(&value)?;
    Ok(value)
}

/// 按路由和状态过滤控制目录。
pub fn list(route: Option<&str>, status: Option<&str>) -> Result<Value> {
    let value = catalogue()?;
    let controls = value["controls"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|control| {
            route.is_none_or(|expected| control["route"] == expected)
                && status.is_none_or(|expected| control["status"] == expected)
        })
        .cloned()
        .collect::<Vec<_>>();
    Ok(json!({
        "schema":value["schema"],
        "editor_scope":value["editor_scope"],
        "observed_editor":value["observed_editor"],
        "route_priority":value["route_priority"],
        "controls":controls,
        "truth_rule":value["truth_rule"]
    }))
}

/// 按稳定语义 ID 读取一个控件。
pub fn get(semantic_id: &str) -> Result<Value> {
    let value = catalogue()?;
    value["controls"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|control| control["semantic_id"] == semantic_id)
        .cloned()
        .ok_or_else(|| anyhow::anyhow!("unknown timeline control: {semantic_id}"))
}

/// 返回目标版本截图中逐入口登记的完整界面表面清单。
pub fn surface_inventory() -> Result<Value> {
    let value: Value = serde_json::from_str(SURFACE_INVENTORY)?;
    validate_surface_inventory(&value, &catalogue()?)?;
    Ok(value)
}

/// 按区域和映射状态过滤界面表面清单。
pub fn surfaces(region: Option<&str>, status: Option<&str>) -> Result<Value> {
    let value = surface_inventory()?;
    let surfaces = value["surfaces"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|surface| {
            region.is_none_or(|expected| surface["region"] == expected)
                && status.is_none_or(|expected| surface["status"] == expected)
        })
        .cloned()
        .collect::<Vec<_>>();
    Ok(json!({
        "schema":value["schema"],
        "observed_editor":value["observed_editor"],
        "evidence":value["evidence"],
        "coverage":value["coverage"],
        "surfaces":surfaces,
        "expansion_rule":value["expansion_rule"],
        "truth_rule":value["truth_rule"]
    }))
}

/// 校验版本绑定的运行时控制请求；未具备状态回读的控制会结构化拒绝。
pub fn request(
    semantic_id: &str,
    requested_operation: &str,
    input: &Value,
    bundle_id: &str,
    version: &str,
    build: &str,
) -> std::result::Result<Value, jianying_runtime::RuntimeError> {
    let catalogue =
        catalogue().map_err(|error| jianying_runtime::RuntimeError::ControlUnavailable {
            control: semantic_id.to_owned(),
            reason: error.to_string(),
        })?;
    let control = catalogue["controls"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|control| control["semantic_id"] == semantic_id)
        .ok_or_else(|| jianying_runtime::RuntimeError::UnknownControl(semantic_id.to_owned()))?;
    for (field, observed) in [
        ("bundle_id", bundle_id),
        ("version", version),
        ("build", build),
    ] {
        let expected = catalogue["observed_editor"][field]
            .as_str()
            .unwrap_or_default();
        if expected != observed {
            return Err(jianying_runtime::RuntimeError::ControlProfileMismatch {
                field: field.to_owned(),
                expected: expected.to_owned(),
                observed: observed.to_owned(),
            });
        }
    }
    let expected_operation = control["state_contract"]["operation"]
        .as_str()
        .unwrap_or("direct_command");
    if expected_operation != requested_operation {
        return Err(jianying_runtime::RuntimeError::ControlOperationMismatch {
            control: semantic_id.to_owned(),
            expected: expected_operation.to_owned(),
            requested: requested_operation.to_owned(),
        });
    }
    let input_contract = control["state_contract"]["input"]
        .as_str()
        .unwrap_or_default();
    let input_valid = match input_contract {
        "boolean" => input.is_boolean(),
        "finite_number" => input.as_f64().is_some_and(f64::is_finite),
        "empty_object" => input.as_object().is_some_and(serde_json::Map::is_empty),
        "optional_step_object" | "record_action_object" => input.is_object(),
        _ => false,
    };
    if !input_valid {
        return Err(jianying_runtime::RuntimeError::ControlUnavailable {
            control: semantic_id.to_owned(),
            reason: format!("input does not satisfy {input_contract}"),
        });
    }
    Err(jianying_runtime::RuntimeError::ControlUnavailable {
        control: semantic_id.to_owned(),
        reason: format!(
            "{} readback is not implemented for this version-bound route",
            control["state_contract"]["readback"]
                .as_str()
                .unwrap_or("state")
        ),
    })
}

fn validate(value: &Value) -> Result<()> {
    if value["schema"] != "jianying-timeline-control-catalog/v1" {
        bail!("unsupported timeline control catalog schema");
    }
    for field in ["product", "bundle_id", "version", "build", "platform"] {
        if value["observed_editor"][field]
            .as_str()
            .is_none_or(|item| item.trim().is_empty())
        {
            bail!("timeline control catalog observed_editor.{field} missing");
        }
    }
    let controls = value["controls"]
        .as_array()
        .ok_or_else(|| anyhow::anyhow!("timeline controls must be an array"))?;
    let mut ids = BTreeSet::new();
    for control in controls {
        let id = control["semantic_id"]
            .as_str()
            .filter(|id| !id.trim().is_empty())
            .ok_or_else(|| anyhow::anyhow!("timeline control semantic_id missing"))?;
        if !ids.insert(id) {
            bail!("duplicate timeline control: {id}");
        }
        if !matches!(
            control["route"].as_str(),
            Some("draft_protocol" | "runtime_native" | "accessibility_adapter" | "visual_fallback")
        ) {
            bail!("invalid timeline control route: {id}");
        }
        if !matches!(
            control["status"].as_str(),
            Some("supported" | "partial" | "unresolved")
        ) {
            bail!("invalid timeline control status: {id}");
        }
        if matches!(
            id,
            "timeline.session.undo"
                | "timeline.session.redo"
                | "timeline.audio.record"
                | "timeline.session.main_track_magnet"
                | "timeline.session.snap"
                | "timeline.session.linkage"
                | "timeline.view.fit"
                | "timeline.view.zoom_out"
                | "timeline.view.zoom"
                | "timeline.view.zoom_in"
        ) {
            let state_contract = control["state_contract"]
                .as_object()
                .ok_or_else(|| anyhow::anyhow!("runtime control state contract missing: {id}"))?;
            if !matches!(
                state_contract.get("operation").and_then(Value::as_str),
                Some("set" | "invoke")
            ) || state_contract
                .get("readback")
                .and_then(Value::as_str)
                .is_none_or(str::is_empty)
                || state_contract
                    .get("readback_required")
                    .and_then(Value::as_bool)
                    != Some(true)
            {
                bail!("invalid runtime control state contract: {id}");
            }
        }
        if !control["parameters"].is_array() {
            bail!("timeline control parameters missing: {id}");
        }
        if !matches!(
            control["fallback_route"].as_str(),
            Some("draft_protocol" | "runtime_native" | "accessibility_adapter" | "visual_fallback")
        ) {
            bail!("invalid timeline control fallback route: {id}");
        }
        if control.get("screen_coordinate").is_some() || control.get("coordinates").is_some() {
            bail!("timeline control catalog must not contain coordinates: {id}");
        }
    }
    Ok(())
}

fn validate_surface_inventory(value: &Value, catalogue: &Value) -> Result<()> {
    if value["schema"] != "jianying-timeline-surface-inventory/v1" {
        bail!("unsupported timeline surface inventory schema");
    }
    let expected = value["coverage"]["expected_surfaces"]
        .as_u64()
        .ok_or_else(|| anyhow::anyhow!("surface coverage count missing"))?
        as usize;
    let surfaces = value["surfaces"]
        .as_array()
        .ok_or_else(|| anyhow::anyhow!("timeline surfaces must be an array"))?;
    if surfaces.len() != expected {
        bail!(
            "timeline surface count mismatch: expected {expected}, observed {}",
            surfaces.len()
        );
    }
    let region_total = value["coverage"]["regions"]
        .as_object()
        .into_iter()
        .flat_map(|regions| regions.values())
        .filter_map(Value::as_u64)
        .sum::<u64>() as usize;
    if region_total != expected {
        bail!("timeline surface region totals do not equal expected count");
    }
    let control_ids = catalogue["controls"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|control| control["semantic_id"].as_str())
        .collect::<BTreeSet<_>>();
    let mut surface_ids = BTreeSet::new();
    let mut observed_regions = std::collections::BTreeMap::<&str, usize>::new();
    let mut expandable_parents = 0usize;
    let mut enumerated_parents = 0usize;
    for surface in surfaces {
        let surface_id = surface["surface_id"]
            .as_str()
            .filter(|id| !id.is_empty())
            .ok_or_else(|| anyhow::anyhow!("timeline surface_id missing"))?;
        if !surface_ids.insert(surface_id) {
            bail!("duplicate timeline surface: {surface_id}");
        }
        let semantic_id = surface["semantic_id"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("timeline surface semantic_id missing: {surface_id}"))?;
        if !control_ids.contains(semantic_id) {
            bail!("timeline surface references unknown control: {semantic_id}");
        }
        let region = surface["region"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("timeline surface region missing: {surface_id}"))?;
        *observed_regions.entry(region).or_default() += 1;
        if !matches!(surface["status"].as_str(), Some("mapped" | "unresolved")) {
            bail!("invalid timeline surface status: {surface_id}");
        }
        if !surface["parameters"].is_array()
            || surface.get("coordinates").is_some()
            || surface.get("screen_coordinate").is_some()
        {
            bail!("invalid or coordinate-bound timeline surface: {surface_id}");
        }
        if let Some(expansion) = surface.get("expansion") {
            expandable_parents += 1;
            if !matches!(
                expansion["kind"].as_str(),
                Some("menu" | "dropdown" | "popover" | "context_menu")
            ) || !matches!(
                expansion["status"].as_str(),
                Some("pending_accessibility" | "partial" | "enumerated")
            ) || !expansion["child_surface_ids"].is_array()
                || expansion["evidence"]
                    .as_str()
                    .is_none_or(|value| value.trim().is_empty())
            {
                bail!("invalid timeline surface expansion: {surface_id}");
            }
            if expansion["status"] == "enumerated" {
                enumerated_parents += 1;
                if expansion["child_surface_ids"]
                    .as_array()
                    .is_none_or(Vec::is_empty)
                {
                    bail!("enumerated timeline surface has no children: {surface_id}");
                }
            }
        }
    }
    if value["coverage"]["expandable_parents"].as_u64() != Some(expandable_parents as u64)
        || value["coverage"]["enumerated_parents"].as_u64() != Some(enumerated_parents as u64)
    {
        bail!("timeline surface expansion coverage differs from observed parents");
    }
    for surface in surfaces {
        let Some(expansion) = surface.get("expansion") else {
            continue;
        };
        let parent_id = surface["surface_id"].as_str().unwrap_or_default();
        for child_id in expansion["child_surface_ids"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
        {
            let child = surfaces
                .iter()
                .find(|candidate| candidate["surface_id"] == child_id)
                .ok_or_else(|| anyhow::anyhow!("unknown expanded child surface: {child_id}"))?;
            if child["parent_surface_id"] != parent_id {
                bail!("expanded child {child_id} does not reference parent {parent_id}");
            }
            if expansion["status"] == "enumerated"
                && child["accessibility_name"]
                    .as_str()
                    .is_none_or(|value| value.trim().is_empty())
            {
                bail!("enumerated child lacks accessibility name: {child_id}");
            }
        }
    }
    for (region, count) in value["coverage"]["regions"]
        .as_object()
        .into_iter()
        .flatten()
    {
        if observed_regions
            .get(region.as_str())
            .copied()
            .unwrap_or_default()
            != count.as_u64().unwrap_or_default() as usize
        {
            bail!("timeline surface region count mismatch: {region}");
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expanded_parent_requires_named_accessible_children() {
        let catalog: Value = serde_json::from_str(CATALOG).unwrap();
        let mut inventory: Value = serde_json::from_str(SURFACE_INVENTORY).unwrap();
        validate_surface_inventory(&inventory, &catalog).unwrap();

        inventory["coverage"]["enumerated_parents"] = 1.into();
        let surfaces = inventory["surfaces"].as_array_mut().unwrap();
        let parent = surfaces
            .iter_mut()
            .find(|surface| surface["surface_id"] == "view.more")
            .unwrap();
        parent["expansion"]["status"] = "enumerated".into();
        let error = validate_surface_inventory(&inventory, &catalog).unwrap_err();
        assert!(error.to_string().contains("lacks accessibility name"));

        for child in inventory["surfaces"].as_array_mut().unwrap().iter_mut() {
            if child["parent_surface_id"] == "view.more" {
                child["accessibility_name"] = child["label"].clone();
            }
        }
        validate_surface_inventory(&inventory, &catalog).unwrap();
    }

    #[test]
    fn expansion_coverage_count_must_match_inventory() {
        let catalog: Value = serde_json::from_str(CATALOG).unwrap();
        let mut inventory: Value = serde_json::from_str(SURFACE_INVENTORY).unwrap();
        inventory["coverage"]["expandable_parents"] = 8.into();
        let error = validate_surface_inventory(&inventory, &catalog).unwrap_err();
        assert!(error
            .to_string()
            .contains("expansion coverage differs from observed parents"));
    }
}
