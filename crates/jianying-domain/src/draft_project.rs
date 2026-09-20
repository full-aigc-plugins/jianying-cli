use crate::{DomainError, FrameRate, Material, Timeline};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

/// 与具体剪映文件布局解耦的统一草稿项目聚合根。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DraftProject {
    name: String,
    width: u32,
    height: u32,
    frame_rate: FrameRate,
    timeline: Timeline,
    materials: Vec<Material>,
}

impl DraftProject {
    /// 创建项目并校验名称、画布、素材唯一性和片段素材引用。
    pub fn new(
        name: impl Into<String>,
        width: u32,
        height: u32,
        frame_rate: FrameRate,
        timeline: Timeline,
        materials: Vec<Material>,
    ) -> Result<Self, DomainError> {
        let name = name.into();
        if name.trim().is_empty()
            || name
                .chars()
                .any(|character| "/\\\n\r\0".contains(character))
        {
            return Err(DomainError::InvalidField {
                field: "name",
                reason: "must be a visible path component".to_owned(),
            });
        }
        if !(16..=8192).contains(&width) || !(16..=8192).contains(&height) {
            return Err(DomainError::InvalidField {
                field: "canvas",
                reason: "width and height must be 16..8192".to_owned(),
            });
        }
        let mut material_ids = BTreeSet::new();
        for material in &materials {
            material.validate()?;
            if !material_ids.insert(material.id().as_str()) {
                return Err(DomainError::InvalidField {
                    field: "material_id",
                    reason: format!("duplicate material id {}", material.id().as_str()),
                });
            }
        }
        for track in timeline.tracks() {
            for segment in track.segments() {
                if let Some(material_id) = segment.material_id() {
                    if !material_ids.contains(material_id.as_str()) {
                        return Err(DomainError::MissingMaterial(
                            material_id.as_str().to_owned(),
                        ));
                    }
                }
            }
        }
        Ok(Self {
            name,
            width,
            height,
            frame_rate,
            timeline,
            materials,
        })
    }

    /// 重新校验反序列化项目的全部构造不变量。
    pub fn validate(&self) -> Result<(), DomainError> {
        let frame_rate =
            FrameRate::new(self.frame_rate.numerator(), self.frame_rate.denominator())?;
        let tracks = self
            .timeline
            .tracks()
            .iter()
            .map(|track| crate::Track::new(track.id(), track.kind(), track.segments().to_vec()))
            .collect::<Result<Vec<_>, _>>()?;
        let timeline = Timeline::new(tracks)?;
        Self::new(
            self.name.clone(),
            self.width,
            self.height,
            frame_rate,
            timeline,
            self.materials.clone(),
        )?;
        Ok(())
    }

    /// 返回项目时间线。
    pub fn timeline(&self) -> &Timeline {
        &self.timeline
    }

    /// 返回项目名称。
    pub fn name(&self) -> &str {
        &self.name
    }

    /// 返回画布宽度。
    pub fn width(&self) -> u32 {
        self.width
    }

    /// 返回画布高度。
    pub fn height(&self) -> u32 {
        self.height
    }

    /// 返回项目帧率。
    pub fn frame_rate(&self) -> FrameRate {
        self.frame_rate
    }

    /// 返回项目素材表。
    pub fn materials(&self) -> &[Material] {
        &self.materials
    }
}
