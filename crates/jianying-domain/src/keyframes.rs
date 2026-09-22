use crate::{DomainError, TrackKind};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// 单个关键帧采样点。对应 pyJianYingDraft 的 KeyframeProperty 时间和值语义。
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct KeyframePoint {
    at_us: i64,
    value: f64,
}

impl KeyframePoint {
    /// 创建非负时间、有限数值的关键帧点。
    pub fn new(at_us: i64, value: f64) -> Result<Self, DomainError> {
        if at_us < 0 || !value.is_finite() {
            return Err(DomainError::InvalidField {
                field: "keyframe_point",
                reason: "at_us must be non-negative and value must be finite".to_owned(),
            });
        }
        Ok(Self { at_us, value })
    }

    /// 返回片段内相对时间。
    pub fn at_us(&self) -> i64 {
        self.at_us
    }

    /// 返回关键帧值。
    pub fn value(&self) -> f64 {
        self.value
    }
}

/// 按属性名称组织的关键帧集合。对应 pyJianYingDraft 的关键帧属性映射。
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Keyframes(BTreeMap<String, Vec<KeyframePoint>>);

impl Keyframes {
    /// 创建关键帧集合并校验通用时序约束。
    pub fn new(channels: BTreeMap<String, Vec<KeyframePoint>>) -> Result<Self, DomainError> {
        let keyframes = Self(channels);
        keyframes.validate_structure()?;
        Ok(keyframes)
    }

    /// 返回按稳定名称排序的关键帧通道。
    pub fn channels(&self) -> &BTreeMap<String, Vec<KeyframePoint>> {
        &self.0
    }

    pub(crate) fn validate_for(
        &self,
        track_kind: TrackKind,
        duration_us: i64,
    ) -> Result<(), DomainError> {
        self.validate_structure()?;
        let allowed: &[(&str, f64, f64)] = match track_kind {
            TrackKind::Video | TrackKind::Sticker => &[
                ("scale", 0.01, 10.0),
                ("x", -2.0, 2.0),
                ("y", -2.0, 2.0),
                ("rotation", -360.0, 360.0),
                ("opacity", 0.0, 1.0),
            ],
            TrackKind::Text => &[
                ("scale", 0.01, 10.0),
                ("x", -2.0, 2.0),
                ("y", -2.0, 2.0),
                ("rotation", -360.0, 360.0),
            ],
            TrackKind::Audio => &[("volume", 0.0, 4.0)],
            _ => &[],
        };
        for (channel, points) in &self.0 {
            let (_, minimum, maximum) = allowed
                .iter()
                .find(|(candidate, _, _)| candidate == channel)
                .ok_or_else(|| DomainError::InvalidField {
                    field: "keyframes.channel",
                    reason: format!("{channel} is not supported on {}", track_kind.as_str()),
                })?;
            if points.iter().any(|point| {
                point.at_us > duration_us || !(*minimum..=*maximum).contains(&point.value)
            }) {
                return Err(DomainError::InvalidField {
                    field: "keyframes",
                    reason: format!(
                        "channel {channel} must stay within the segment and {minimum}..{maximum}"
                    ),
                });
            }
        }
        Ok(())
    }

    fn validate_structure(&self) -> Result<(), DomainError> {
        for (channel, points) in &self.0 {
            if channel.trim().is_empty() || points.len() < 2 || points[0].at_us != 0 {
                return Err(DomainError::InvalidField {
                    field: "keyframes",
                    reason: "each named channel needs at least two points starting at 0".to_owned(),
                });
            }
            if points.windows(2).any(|pair| pair[0].at_us >= pair[1].at_us)
                || points
                    .iter()
                    .any(|point| point.at_us < 0 || !point.value.is_finite())
            {
                return Err(DomainError::InvalidField {
                    field: "keyframes",
                    reason: "point times must strictly ascend and values must be finite".to_owned(),
                });
            }
        }
        Ok(())
    }
}
