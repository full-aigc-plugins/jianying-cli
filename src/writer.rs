//! 草稿写入器：编辑操作的唯一落盘点。
//!
//! 新建路径（`project build`）与编辑路径（`timeline ...`）都必须经过这里，
//! 否则同一语义会在两条路径上写出不同 wire。当前覆盖片段级的速度、音量与
//! 不透明度；后续操作按同一形状逐步迁入。

use crate::edit_op::EditOp;
use anyhow::{bail, Result};
use serde_json::{json, Value};

/// 依据目标区间长度与速度重算源区间长度。
///
/// pyJianYingDraft 使用 Python 的 `round()`，即银行家舍入（ties-to-even）。
/// Rust 的 `f64::round()` 在 .5 上向远离零取整：`3_000_001 × 0.5 = 1_500_000.5`
/// 会得到 1_500_001，而参考实现得到 1_500_000，相差的 1µs 直接进入
/// `source_timerange`，属于 wire 级差异。
pub fn scaled_source_duration(target_duration_us: i64, speed: f64) -> i64 {
    (target_duration_us as f64 * speed).round_ties_even() as i64
}

/// 把一个操作应用到**已定位的片段**上。
///
/// - `segment`：目标片段对象（可变）。
/// - `materials`：草稿根 `materials` 对象；仅速度需要同步伴生素材，其余操作传 `None`。
///
/// 返回执行前后的对比信息，供调用方组装审计输出。
pub fn apply(segment: &mut Value, materials: Option<&mut Value>, op: EditOp) -> Result<Value> {
    op.validate()?;
    match op {
        EditOp::SetSpeed(speed) => set_speed(segment, materials, speed),
        EditOp::SetVolume(volume) => set_volume(segment, volume),
        EditOp::SetOpacity(alpha) => set_opacity(segment, alpha),
    }
}

fn set_speed(segment: &mut Value, materials: Option<&mut Value>, speed: f64) -> Result<Value> {
    let old = segment["speed"].as_f64().unwrap_or(1.0);
    let target_duration = segment["target_timerange"]["duration"]
        .as_i64()
        .unwrap_or(0);
    let source_duration = scaled_source_duration(target_duration, speed);
    segment["speed"] = json!(speed);
    segment["source_timerange"]["duration"] = json!(source_duration);

    if let Some(materials) = materials {
        let references: Vec<String> = segment["extra_material_refs"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .map(str::to_owned)
            .collect();
        for reference in references {
            if let Some(material) = materials["speeds"]
                .as_array_mut()
                .and_then(|items| items.iter_mut().find(|item| item["id"] == reference))
            {
                material["speed"] = json!(speed);
            }
        }
    }

    Ok(json!({
        "old_speed": old,
        "new_speed": speed,
        "source_duration_us": source_duration
    }))
}

fn set_volume(segment: &mut Value, volume: f64) -> Result<Value> {
    let old = segment["volume"].as_f64().unwrap_or(1.0);
    segment["volume"] = json!(volume);
    Ok(json!({"old_volume": old, "new_volume": volume}))
}

fn set_opacity(segment: &mut Value, alpha: f64) -> Result<Value> {
    if !segment["clip"].is_object() {
        bail!("segment has no clip; opacity only applies to visual segments");
    }
    let old = segment["clip"]["alpha"].as_f64().unwrap_or(1.0);
    segment["clip"]["alpha"] = json!(alpha);
    Ok(json!({"old_opacity": old, "new_opacity": alpha}))
}

#[cfg(test)]
mod tests {
    use super::scaled_source_duration;

    #[test]
    fn source_duration_rounds_half_to_even_like_the_reference() {
        // 2.5 → 2，3.5 → 4：ties-to-even，而不是 f64::round() 的远离零。
        assert_eq!(scaled_source_duration(5, 0.5), 2);
        assert_eq!(scaled_source_duration(7, 0.5), 4);
        // 整值速度与常见小数不受影响。
        assert_eq!(scaled_source_duration(2_000_000, 2.0), 4_000_000);
        assert_eq!(scaled_source_duration(2_000_000, 0.5), 1_000_000);
    }
}
