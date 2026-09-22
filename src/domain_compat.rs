use anyhow::{anyhow, Result};
use jianying_domain::{
    Animation, AudioEffect, AudioEffects, BackgroundFilling, BlendMode, ChromaKey, ClipSettings,
    CropSettings, DraftProject, Fade, FrameRate, KeyframePoint, Keyframes, Mask, Material,
    MaterialId, RawIds, Segment, SegmentId, StyleRange, TextBackground, TextShadow, TextStyle,
    TimeRange, Timeline, Track, TrackKind, Transform, Transition,
};
use jianying_schema::{CompatibilityInput, JobV2};

use crate::plan::Plan;

/// 将已通过校验的 `jianying-cli-plan/v1` 转换为统一领域模型。
///
/// 该转换不写文件；当前构建器仍消费 v1，后续 Job v2 和 wire adapter 将复用
/// 此模型。转换错误会携带轨道和片段位置，避免把字段问题推迟到草稿写入阶段。
pub fn from_v1_plan(plan: &Plan) -> Result<DraftProject> {
    plan.validate()?;
    let mut materials = Vec::new();
    let mut tracks = Vec::with_capacity(plan.tracks.len());

    for (track_index, source_track) in plan.tracks.iter().enumerate() {
        let kind = track_kind(&source_track.kind)?;
        let mut segments = Vec::with_capacity(source_track.segments.len());
        for (segment_index, source_segment) in source_track.segments.iter().enumerate() {
            let location = format!("track {track_index} segment {segment_index}");
            let id = SegmentId::new(format!("segment-{track_index}-{segment_index}"))?;
            let range = TimeRange::new(source_segment.start_us, source_segment.duration_us)
                .map_err(|error| anyhow!("{location}: {error}"))?;
            let segment = match kind {
                TrackKind::Video | TrackKind::Audio => {
                    let path = source_segment
                        .source
                        .clone()
                        .ok_or_else(|| anyhow!("{location}: source is required"))?;
                    let material_id =
                        MaterialId::new(format!("material-{track_index}-{segment_index}"))?;
                    let source_duration = source_segment
                        .source_duration_us
                        .unwrap_or(source_segment.duration_us);
                    let source_range =
                        TimeRange::new(source_segment.source_start_us, source_duration)
                            .map_err(|error| anyhow!("{location}: {error}"))?;
                    let clip = ClipSettings::new(
                        source_segment.speed.unwrap_or(1.0),
                        source_segment.volume.unwrap_or(1.0),
                        source_segment.change_pitch,
                    )
                    .map_err(|error| anyhow!("{location}: {error}"))?;
                    if kind == TrackKind::Video {
                        let material = if source_segment.photo {
                            Material::image(material_id.clone(), path)
                        } else {
                            Material::video(material_id.clone(), path)
                        };
                        materials.push(material);
                        Segment::video_with_advanced_settings(
                            id,
                            range,
                            material_id,
                            source_range,
                            clip,
                            transform(source_segment)
                                .map_err(|error| anyhow!("{location}: {error}"))?,
                            source_segment
                                .crop
                                .as_ref()
                                .map(crop_settings)
                                .transpose()
                                .map_err(|error| anyhow!("{location}: {error}"))?,
                            keyframes(source_segment)
                                .map_err(|error| anyhow!("{location}: {error}"))?,
                            source_segment
                                .mask
                                .as_ref()
                                .map(mask)
                                .transpose()
                                .map_err(|error| anyhow!("{location}: {error}"))?,
                            source_segment
                                .chroma
                                .as_ref()
                                .map(chroma)
                                .transpose()
                                .map_err(|error| anyhow!("{location}: {error}"))?,
                            source_segment
                                .background_filling
                                .as_ref()
                                .map(background_filling)
                                .transpose()
                                .map_err(|error| anyhow!("{location}: {error}"))?,
                            source_segment
                                .mix_mode
                                .as_deref()
                                .map(BlendMode::new)
                                .transpose()
                                .map_err(|error| anyhow!("{location}: {error}"))?,
                            animation(source_segment.animation_in.as_ref())
                                .map_err(|error| anyhow!("{location}: {error}"))?,
                            animation(source_segment.animation_out.as_ref())
                                .map_err(|error| anyhow!("{location}: {error}"))?,
                            animation(source_segment.animation_group.as_ref())
                                .map_err(|error| anyhow!("{location}: {error}"))?,
                            transition(source_segment.transition_out.as_ref())
                                .map_err(|error| anyhow!("{location}: {error}"))?,
                            source_segment
                                .fade
                                .as_ref()
                                .map(|fade| Fade::new(fade.in_us, fade.out_us))
                                .transpose()
                                .map_err(|error| anyhow!("{location}: {error}"))?,
                        )?
                    } else {
                        materials.push(Material::audio(material_id.clone(), path));
                        Segment::audio_with_advanced_settings(
                            id,
                            range,
                            material_id,
                            source_range,
                            clip,
                            keyframes(source_segment)
                                .map_err(|error| anyhow!("{location}: {error}"))?,
                            audio_effects(source_segment)
                                .map_err(|error| anyhow!("{location}: {error}"))?,
                        )?
                    }
                }
                TrackKind::Text => Segment::text_with_settings(
                    id,
                    range,
                    source_segment.text.clone().unwrap_or_default(),
                    transform(source_segment).map_err(|error| anyhow!("{location}: {error}"))?,
                    keyframes(source_segment).map_err(|error| anyhow!("{location}: {error}"))?,
                    animation(source_segment.animation_in.as_ref())
                        .map_err(|error| anyhow!("{location}: {error}"))?,
                    animation(source_segment.animation_out.as_ref())
                        .map_err(|error| anyhow!("{location}: {error}"))?,
                    animation(source_segment.animation_group.as_ref())
                        .map_err(|error| anyhow!("{location}: {error}"))?,
                    text_style(source_segment).map_err(|error| anyhow!("{location}: {error}"))?,
                )?,
                TrackKind::Sticker => Segment::sticker_with_motion(
                    id,
                    range,
                    source_segment
                        .resource_id
                        .clone()
                        .ok_or_else(|| anyhow!("{location}: resource_id is required"))?,
                    transform(source_segment).map_err(|error| anyhow!("{location}: {error}"))?,
                    keyframes(source_segment).map_err(|error| anyhow!("{location}: {error}"))?,
                )?,
                TrackKind::Filter => {
                    let filter = source_segment
                        .filters
                        .first()
                        .ok_or_else(|| anyhow!("{location}: filters[0] is required"))?;
                    Segment::filter(
                        id,
                        range,
                        filter.name.clone(),
                        source_segment
                            .intensity
                            .or(filter.intensity)
                            .unwrap_or(100.0),
                    )?
                }
                TrackKind::Effect => {
                    let effect = source_segment
                        .effects
                        .first()
                        .ok_or_else(|| anyhow!("{location}: effects[0] is required"))?;
                    Segment::effect(
                        id,
                        range,
                        effect.name.clone(),
                        source_segment
                            .params
                            .clone()
                            .unwrap_or_else(|| effect.params.clone()),
                    )?
                }
                TrackKind::Composite => unreachable!("v1 has no composite track"),
            };
            segments.push(segment);
        }
        // v1 允许同名轨道；统一模型的内部标识追加稳定序号，避免把显示名误作唯一键。
        let track_id = source_track
            .name
            .as_deref()
            .map(|name| format!("{name}-{track_index}"))
            .unwrap_or_else(|| format!("{}-{track_index}", source_track.kind));
        tracks.push(Track::new_named(
            track_id,
            source_track.name.clone(),
            kind,
            segments,
        )?);
    }

    let timeline = Timeline::new(tracks)?;
    let width =
        u32::try_from(plan.canvas.width).map_err(|_| anyhow!("canvas width exceeds u32"))?;
    let height =
        u32::try_from(plan.canvas.height).map_err(|_| anyhow!("canvas height exceeds u32"))?;
    let fps = u32::try_from(plan.canvas.fps).map_err(|_| anyhow!("fps exceeds u32"))?;
    Ok(DraftProject::new(
        plan.name.clone(),
        width,
        height,
        FrameRate::new(fps, 1)?,
        timeline,
        materials,
    )?)
}

fn transform(segment: &crate::plan::Segment) -> Result<Transform, jianying_domain::DomainError> {
    Transform::new(
        segment.scale,
        segment.x,
        segment.y,
        segment.rotation,
        segment.opacity,
    )
}

fn crop_settings(
    crop: &crate::plan::CropSettings,
) -> Result<CropSettings, jianying_domain::DomainError> {
    CropSettings::new([
        crop.upper_left_x,
        crop.upper_left_y,
        crop.upper_right_x,
        crop.upper_right_y,
        crop.lower_left_x,
        crop.lower_left_y,
        crop.lower_right_x,
        crop.lower_right_y,
    ])
}

fn keyframes(
    segment: &crate::plan::Segment,
) -> Result<Option<Keyframes>, jianying_domain::DomainError> {
    segment
        .keyframes
        .as_ref()
        .map(|channels| {
            channels
                .iter()
                .map(|(channel, points)| {
                    points
                        .iter()
                        .map(|point| KeyframePoint::new(point.at_us, point.value))
                        .collect::<Result<Vec<_>, _>>()
                        .map(|points| (channel.clone(), points))
                })
                .collect::<Result<std::collections::BTreeMap<_, _>, _>>()
                .and_then(Keyframes::new)
        })
        .transpose()
}

fn mask(value: &crate::plan::Mask) -> Result<Mask, jianying_domain::DomainError> {
    Mask::new(
        value.name.clone(),
        value.center_x,
        value.center_y,
        value.size,
        value.rotation,
        value.feather,
        value.invert,
        value.rect_width,
        value.round_corner,
    )
}

fn chroma(value: &crate::plan::Chroma) -> Result<ChromaKey, jianying_domain::DomainError> {
    ChromaKey::new(
        value.color.clone(),
        value.intensity,
        value.shadow,
        value.edge_smooth,
        value.spill,
    )
}

fn background_filling(
    value: &crate::plan::BackgroundFilling,
) -> Result<BackgroundFilling, jianying_domain::DomainError> {
    BackgroundFilling::new(value.fill_type.clone(), value.blur, value.color.clone())
}

fn animation(
    value: Option<&crate::plan::Animation>,
) -> Result<Option<Animation>, jianying_domain::DomainError> {
    value
        .map(|animation| Animation::new(animation.name.clone(), animation.duration_us))
        .transpose()
}

fn transition(
    value: Option<&crate::plan::TransitionOut>,
) -> Result<Option<Transition>, jianying_domain::DomainError> {
    value
        .map(|transition| Transition::new(transition.name.clone(), transition.duration_us))
        .transpose()
}

fn audio_effects(
    segment: &crate::plan::Segment,
) -> Result<AudioEffects, jianying_domain::DomainError> {
    let fade = segment
        .fade
        .as_ref()
        .map(|fade| Fade::new(fade.in_us, fade.out_us))
        .transpose()?;
    let effects = segment
        .audio_effects
        .iter()
        .map(|effect| AudioEffect::new(effect.name.clone(), effect.params.clone()))
        .collect::<Result<Vec<_>, _>>()?;
    AudioEffects::new(fade, effects)
}

fn text_style(segment: &crate::plan::Segment) -> Result<TextStyle, jianying_domain::DomainError> {
    let background = segment
        .background
        .as_ref()
        .map(|background| {
            TextBackground::new(
                background.color.clone(),
                background.style,
                background.alpha,
                background.round_radius,
                background.height,
                background.width,
                background.horizontal_offset,
                background.vertical_offset,
            )
        })
        .transpose()?;
    let shadow = segment
        .shadow
        .as_ref()
        .map(|shadow| {
            TextShadow::new(
                shadow.color.clone(),
                shadow.alpha,
                shadow.angle,
                shadow.distance,
                shadow.diffuse,
            )
        })
        .transpose()?;
    let styles = segment
        .styles
        .iter()
        .map(|style| {
            StyleRange::new(
                style.range,
                style.size,
                style.bold,
                style.italic,
                style.underline,
                style.color.clone(),
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    let raw_ids = |value: &crate::plan::RawIds| {
        RawIds::new(value.effect_id.clone(), value.resource_id.clone())
    };
    TextStyle::new(
        segment.size,
        segment.color.clone(),
        segment.border_color.clone(),
        segment.border_width,
        segment.bold,
        segment.italic,
        segment.underline,
        segment.alignment,
        segment.font.clone(),
        background,
        shadow,
        styles,
        segment.text_effect.as_ref().map(raw_ids).transpose()?,
        segment.bubble.as_ref().map(raw_ids).transpose()?,
    )
}

/// 将兼容的 v1 plan 包装为通过语义校验的 `jianying-job/v2` create 作业。
pub fn job_from_v1_plan(plan: &Plan) -> Result<JobV2> {
    let compatibility = CompatibilityInput::plan_v1(serde_json::to_value(plan)?)?;
    Ok(JobV2::create_compatible(
        from_v1_plan(plan)?,
        compatibility,
    )?)
}

fn track_kind(value: &str) -> Result<TrackKind> {
    match value {
        "video" => Ok(TrackKind::Video),
        "audio" => Ok(TrackKind::Audio),
        "text" => Ok(TrackKind::Text),
        "sticker" => Ok(TrackKind::Sticker),
        "filter" => Ok(TrackKind::Filter),
        "effect" => Ok(TrackKind::Effect),
        other => Err(anyhow!("unsupported v1 track kind {other}")),
    }
}
