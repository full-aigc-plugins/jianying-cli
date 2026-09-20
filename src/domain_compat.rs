use anyhow::{anyhow, Result};
use jianying_domain::{
    DraftProject, FrameRate, Material, MaterialId, Segment, SegmentId, TimeRange, Timeline, Track,
    TrackKind,
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
                    if kind == TrackKind::Video {
                        let material = if source_segment.photo {
                            Material::image(material_id.clone(), path)
                        } else {
                            Material::video(material_id.clone(), path)
                        };
                        materials.push(material);
                        Segment::video(id, range, material_id, source_range)?
                    } else {
                        materials.push(Material::audio(material_id.clone(), path));
                        Segment::audio(id, range, material_id, source_range)?
                    }
                }
                TrackKind::Text => {
                    Segment::text(id, range, source_segment.text.clone().unwrap_or_default())?
                }
                TrackKind::Sticker => Segment::sticker(
                    id,
                    range,
                    source_segment
                        .resource_id
                        .clone()
                        .ok_or_else(|| anyhow!("{location}: resource_id is required"))?,
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
        tracks.push(Track::new(track_id, kind, segments)?);
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
