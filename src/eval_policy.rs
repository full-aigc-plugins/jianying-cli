//! 评估策略（eval policy）：把评分权重、扣分与阈值从代码里拿出来。
//!
//! 默认值等价于历史行为，因此不传策略时分数不变。策略文件让"平台化调优"成为
//! 可审查的输入，而不是改代码：例如抖音要求更响、B 站容忍更长的静音。

use anyhow::{bail, Context, Result};
use serde_json::{json, Value};
use std::path::Path;

/// 评分权重：四项之和即归一化分母。
#[derive(Debug, Clone, PartialEq)]
pub struct Weights {
    pub main: f64,
    pub continuity: f64,
    pub caption: f64,
    pub audio: f64,
}

/// 扣分表：每个问题代码对应的分数惩罚。
#[derive(Debug, Clone, PartialEq)]
pub struct Penalties {
    pub media_missing: i64,
    pub source_exceeds_media: i64,
    pub video_black: i64,
    pub audio_silent: i64,
    pub audio_mostly_silent: i64,
    pub audio_loudness: i64,
}

/// 判定阈值；比例一律用千分比，避免浮点比较。
#[derive(Debug, Clone, PartialEq)]
pub struct Thresholds {
    pub black_ratio_permille: i64,
    pub silence_ratio_permille: i64,
    pub mostly_silent_permille: i64,
    pub loudness_min_lufs: f64,
    pub loudness_max_lufs: f64,
}

/// 一份完整的评估策略。
#[derive(Debug, Clone, PartialEq)]
pub struct Policy {
    pub name: String,
    pub weights: Weights,
    pub penalties: Penalties,
    pub thresholds: Thresholds,
}

impl Default for Policy {
    fn default() -> Self {
        Self {
            name: "default".to_owned(),
            weights: Weights {
                main: 40.0,
                continuity: 20.0,
                caption: 20.0,
                audio: 20.0,
            },
            penalties: Penalties {
                media_missing: 30,
                source_exceeds_media: 15,
                video_black: 25,
                audio_silent: 20,
                audio_mostly_silent: 10,
                audio_loudness: 10,
            },
            thresholds: Thresholds {
                black_ratio_permille: 800,
                silence_ratio_permille: 950,
                mostly_silent_permille: 800,
                loudness_min_lufs: -40.0,
                loudness_max_lufs: -5.0,
            },
        }
    }
}

impl Policy {
    /// 从策略文件加载；缺省字段沿用默认值。
    pub fn load(path: &Path) -> Result<Self> {
        let raw = std::fs::read_to_string(path)
            .with_context(|| format!("cannot read policy {}", path.display()))?;
        let value: Value = serde_json::from_str(raw.trim_start_matches('\u{feff}'))
            .with_context(|| format!("policy {} is not valid JSON", path.display()))?;
        if value["schema"].as_str() != Some("jianying-eval-policy/v1") {
            bail!(
                "unsupported policy schema {:?} (expected jianying-eval-policy/v1)",
                value["schema"]
            );
        }
        let mut policy = Self::default();
        if let Some(name) = value["name"].as_str() {
            policy.name = name.to_owned();
        }
        let weights = &value["weights"];
        policy.weights.main = weights["main"].as_f64().unwrap_or(policy.weights.main);
        policy.weights.continuity = weights["continuity"]
            .as_f64()
            .unwrap_or(policy.weights.continuity);
        policy.weights.caption = weights["caption"]
            .as_f64()
            .unwrap_or(policy.weights.caption);
        policy.weights.audio = weights["audio"].as_f64().unwrap_or(policy.weights.audio);

        let penalties = &value["penalties"];
        policy.penalties.media_missing = penalties["media-missing"]
            .as_i64()
            .unwrap_or(policy.penalties.media_missing);
        policy.penalties.source_exceeds_media = penalties["source-exceeds-media"]
            .as_i64()
            .unwrap_or(policy.penalties.source_exceeds_media);
        policy.penalties.video_black = penalties["video-black"]
            .as_i64()
            .unwrap_or(policy.penalties.video_black);
        policy.penalties.audio_silent = penalties["audio-silent"]
            .as_i64()
            .unwrap_or(policy.penalties.audio_silent);
        policy.penalties.audio_mostly_silent = penalties["audio-mostly-silent"]
            .as_i64()
            .unwrap_or(policy.penalties.audio_mostly_silent);
        policy.penalties.audio_loudness = penalties["audio-loudness"]
            .as_i64()
            .unwrap_or(policy.penalties.audio_loudness);

        let thresholds = &value["thresholds"];
        policy.thresholds.black_ratio_permille = thresholds["black_ratio_permille"]
            .as_i64()
            .unwrap_or(policy.thresholds.black_ratio_permille);
        policy.thresholds.silence_ratio_permille = thresholds["silence_ratio_permille"]
            .as_i64()
            .unwrap_or(policy.thresholds.silence_ratio_permille);
        policy.thresholds.mostly_silent_permille = thresholds["mostly_silent_permille"]
            .as_i64()
            .unwrap_or(policy.thresholds.mostly_silent_permille);
        policy.thresholds.loudness_min_lufs = thresholds["loudness_min_lufs"]
            .as_f64()
            .unwrap_or(policy.thresholds.loudness_min_lufs);
        policy.thresholds.loudness_max_lufs = thresholds["loudness_max_lufs"]
            .as_f64()
            .unwrap_or(policy.thresholds.loudness_max_lufs);
        policy.validate()?;
        Ok(policy)
    }

    /// 校验取值范围，避免"看起来能跑但分数没有意义"的配置。
    pub fn validate(&self) -> Result<()> {
        let total =
            self.weights.main + self.weights.continuity + self.weights.caption + self.weights.audio;
        if total <= 0.0 {
            bail!("policy weights must sum to a positive number");
        }
        for (label, weight) in [
            ("main", self.weights.main),
            ("continuity", self.weights.continuity),
            ("caption", self.weights.caption),
            ("audio", self.weights.audio),
        ] {
            if !weight.is_finite() || weight < 0.0 {
                bail!("policy weight {label} must be a finite non-negative number");
            }
        }
        for (label, permille) in [
            ("black_ratio_permille", self.thresholds.black_ratio_permille),
            (
                "silence_ratio_permille",
                self.thresholds.silence_ratio_permille,
            ),
            (
                "mostly_silent_permille",
                self.thresholds.mostly_silent_permille,
            ),
        ] {
            if !(0..=1000).contains(&permille) {
                bail!("policy threshold {label} must be within 0..=1000");
            }
        }
        if self.thresholds.silence_ratio_permille < self.thresholds.mostly_silent_permille {
            bail!("silence_ratio_permille must be >= mostly_silent_permille");
        }
        for penalty in [
            self.penalties.media_missing,
            self.penalties.source_exceeds_media,
            self.penalties.video_black,
            self.penalties.audio_silent,
            self.penalties.audio_mostly_silent,
            self.penalties.audio_loudness,
        ] {
            if !(0..=100).contains(&penalty) {
                bail!("policy penalties must be within 0..=100");
            }
        }
        if !self.thresholds.loudness_min_lufs.is_finite()
            || !self.thresholds.loudness_max_lufs.is_finite()
            || self.thresholds.loudness_min_lufs >= self.thresholds.loudness_max_lufs
        {
            bail!("loudness_min_lufs must be a finite value smaller than loudness_max_lufs");
        }
        Ok(())
    }

    /// 报告里回显生效策略，便于复现"为什么是这个分"。
    pub fn to_json(&self) -> Value {
        json!({
            "name": self.name,
            "weights": {
                "main": self.weights.main,
                "continuity": self.weights.continuity,
                "caption": self.weights.caption,
                "audio": self.weights.audio,
            },
            "penalties": {
                "media-missing": self.penalties.media_missing,
                "source-exceeds-media": self.penalties.source_exceeds_media,
                "video-black": self.penalties.video_black,
                "audio-silent": self.penalties.audio_silent,
                "audio-mostly-silent": self.penalties.audio_mostly_silent,
                "audio-loudness": self.penalties.audio_loudness,
            },
            "thresholds": {
                "black_ratio_permille": self.thresholds.black_ratio_permille,
                "silence_ratio_permille": self.thresholds.silence_ratio_permille,
                "mostly_silent_permille": self.thresholds.mostly_silent_permille,
                "loudness_min_lufs": self.thresholds.loudness_min_lufs,
                "loudness_max_lufs": self.thresholds.loudness_max_lufs,
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::Policy;

    #[test]
    fn defaults_are_valid_and_reject_inverted_thresholds() {
        let policy = Policy::default();
        assert!(policy.validate().is_ok());
        let mut broken = policy.clone();
        broken.thresholds.silence_ratio_permille = 500;
        assert!(broken.validate().is_err());
        let mut negative = policy;
        negative.weights.main = -1.0;
        assert!(negative.validate().is_err());
    }
}
