//! 已有草稿的本地素材新增、替换与重链。

use anyhow::{bail, Context, Result};
use jianying_media::{LocalCommandTtsProvider, TtsAudioFormat, TtsProvider, TtsRequest};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use uuid::Uuid;

/// 列出素材类型计数，或按类型返回稳定的素材摘要。
pub fn materials(draft: &Path, material_type: Option<&str>) -> Result<Value> {
    let timeline = crate::draft::load_timeline(draft)?;
    let buckets = timeline["materials"]
        .as_object()
        .context("materials must be an object")?;
    if let Some(material_type) = material_type {
        let items = buckets
            .get(material_type)
            .and_then(Value::as_array)
            .with_context(|| format!("unknown material type: {material_type}"))?;
        return Ok(Value::Array(
            items
                .iter()
                .map(|material| {
                    let mut summary = serde_json::Map::new();
                    summary.insert("id".to_owned(), material["id"].clone());
                    if let Some(name) = material.get("name") {
                        summary.insert("name".to_owned(), name.clone());
                    }
                    if let Some(name) = material.get("material_name") {
                        summary.insert("name".to_owned(), name.clone());
                    }
                    if let Some(path) = material.get("path") {
                        summary.insert("path".to_owned(), path.clone());
                    }
                    if let Some(duration) = material.get("duration") {
                        summary.insert("duration_us".to_owned(), duration.clone());
                    }
                    if let Some(kind) = material.get("type") {
                        summary.insert("type".to_owned(), kind.clone());
                    }
                    summary.insert(
                        "fields".to_owned(),
                        json!(material.as_object().map(serde_json::Map::len).unwrap_or(0)),
                    );
                    Value::Object(summary)
                })
                .collect(),
        ));
    }
    let mut counts = buckets
        .iter()
        .filter_map(|(kind, values)| {
            values
                .as_array()
                .map(|items| json!({"type":kind,"count":items.len()}))
        })
        .collect::<Vec<_>>();
    counts.sort_by(|left, right| {
        right["count"]
            .as_u64()
            .unwrap_or(0)
            .cmp(&left["count"].as_u64().unwrap_or(0))
    });
    Ok(Value::Array(counts))
}

/// 通过完整 ID 或大小写无关前缀返回无损素材对象。
pub fn material(draft: &Path, material_id: &str) -> Result<Value> {
    let timeline = crate::draft::load_timeline(draft)?;
    let wanted = material_id.to_lowercase();
    for (kind, items) in timeline["materials"].as_object().into_iter().flatten() {
        for material in items.as_array().into_iter().flatten() {
            let Some(candidate) = material["id"].as_str() else {
                continue;
            };
            let candidate_lower = candidate.to_lowercase();
            if candidate == material_id || candidate_lower.starts_with(&wanted) {
                let mut detail = material.clone();
                detail
                    .as_object_mut()
                    .context("material must be an object")?
                    .insert("_type".to_owned(), json!(kind));
                return Ok(detail);
            }
        }
    }
    bail!("material not found: {material_id}")
}

/// 新增商店贴纸片段，并写入固定上游要求的六类伴随素材。
#[allow(clippy::too_many_arguments)]
pub fn add_sticker(
    draft: &Path,
    resource_id: &str,
    start_us: i64,
    duration_us: i64,
    x: Option<f64>,
    y: Option<f64>,
    scale: Option<f64>,
    rotation: Option<f64>,
    track_name: Option<&str>,
) -> Result<Value> {
    crate::timeline_ops::mutate(draft, |timeline| {
        let segment_id = Uuid::new_v4().to_string();
        let material_id = Uuid::new_v4().to_string();
        let wanted_name = track_name.unwrap_or("sticker");
        let (track_index, track_id) = {
            let tracks = timeline["tracks"]
                .as_array_mut()
                .context("tracks must be an array")?;
            let track_index = if let Some(index) = tracks.iter().position(|track| {
                track["type"].as_str() == Some("sticker")
                    && track["name"].as_str() == Some(wanted_name)
            }) {
                index
            } else {
                let track_id = Uuid::new_v4().to_string();
                tracks.push(json!({
                    "id":track_id,"type":"sticker","name":wanted_name,"attribute":0,
                    "segments":[],"is_default_name":track_name.is_none(),"flag":0
                }));
                tracks.len() - 1
            };
            let track_id = tracks[track_index]["id"]
                .as_str()
                .context("sticker track id must be a string")?
                .to_owned();
            (track_index, track_id)
        };

        let speed_id = Uuid::new_v4().to_string();
        let placeholder_id = Uuid::new_v4().to_string();
        let channel_id = Uuid::new_v4().to_string();
        let vocal_id = Uuid::new_v4().to_string();
        let canvas_id = Uuid::new_v4().to_string();
        let color_id = Uuid::new_v4().to_string();
        push_material(
            timeline,
            "speeds",
            json!({
                "id":speed_id,"type":"speed","speed":1,"mode":0,"curve_speed":null
            }),
        )?;
        push_material(
            timeline,
            "placeholder_infos",
            json!({
                "id":placeholder_id,"type":"placeholder_info","error_path":"","error_text":"",
                "meta_type":"none","res_path":"","res_text":""
            }),
        )?;
        push_material(
            timeline,
            "sound_channel_mappings",
            json!({
                "id":channel_id,"type":"none","audio_channel_mapping":0,"is_config_open":false
            }),
        )?;
        push_material(
            timeline,
            "vocal_separations",
            json!({
                "id":vocal_id,"type":"vocal_separation","choice":0,"enter_from":"",
                "final_algorithm":"","production_path":"","removed_sounds":[],"time_range":null
            }),
        )?;
        push_material(
            timeline,
            "canvases",
            json!({
                "id":canvas_id,"type":"canvas_color","album_image":"","blur":0,"color":"",
                "image":"","image_id":"","image_name":"","source_platform":0,"team_id":""
            }),
        )?;
        push_material(
            timeline,
            "material_colors",
            json!({
                "id":color_id,"type":"material_color","gradient_angle":90,"gradient_colors":[],
                "gradient_percents":[],"height":0,"is_color_clip":false,"is_gradient":false,
                "solid_color":"","width":0
            }),
        )?;
        push_material(
            timeline,
            "stickers",
            json!({
                "id":material_id,"resource_id":resource_id,"sticker_id":resource_id,
                "source_platform":1,"type":"sticker"
            }),
        )?;

        let scale = scale.unwrap_or(1.0);
        timeline["tracks"].as_array_mut().unwrap()[track_index]["segments"]
            .as_array_mut()
            .context("sticker track segments must be an array")?
            .push(json!({
                "id":segment_id,"material_id":material_id,"raw_segment_id":track_id,
                "target_timerange":{"start":start_us,"duration":duration_us},
                "source_timerange":{"start":0,"duration":duration_us},
                "speed":1,"volume":1,"visible":true,"reverse":false,
                "clip":{"alpha":1,"rotation":rotation.unwrap_or(0.0),
                    "scale":{"x":scale,"y":scale},
                    "transform":{"x":x.unwrap_or(0.0),"y":y.unwrap_or(0.0)},
                    "flip":{"horizontal":false,"vertical":false}},
                "render_index":14000,"track_render_index":0,"track_attribute":0,
                "extra_material_refs":[speed_id,placeholder_id,channel_id,vocal_id,canvas_id,color_id],
                "common_keyframes":[],"keyframe_refs":[]
            }));
        Ok(
            json!({"ok":true,"segmentId":segment_id,"materialId":material_id,
            "trackId":track_id,"start_us":start_us,"duration_us":duration_us}),
        )
    })
}

fn push_material(timeline: &mut Value, bucket: &str, material: Value) -> Result<()> {
    if timeline["materials"].get(bucket).is_none() {
        timeline["materials"][bucket] = json!([]);
    }
    timeline["materials"][bucket]
        .as_array_mut()
        .with_context(|| format!("materials.{bucket} must be an array"))?
        .push(material);
    Ok(())
}

/// 探测并追加本地视频/图片或音频。
pub fn add(
    draft: &Path,
    source: &Path,
    kind: &str,
    start_us: i64,
    duration_us: Option<i64>,
    track_name: Option<&str>,
    volume: f64,
) -> Result<Value> {
    let info = crate::probe::probe(source)?;
    if kind == "video" && !info.has_video && !info.is_image {
        bail!("{} has no video stream", source.display());
    }
    if kind == "audio" && !info.has_audio {
        bail!("{} has no audio stream", source.display());
    }
    let duration = duration_us.unwrap_or(info.duration_us);
    if !info.is_image && duration > info.duration_us + 10_000 {
        bail!("requested duration exceeds probed media duration");
    }
    crate::timeline_ops::mutate_with_path(draft, |timeline, work_copy, identity| {
        crate::draft::append_media(
            timeline,
            work_copy,
            identity,
            crate::draft::MediaAppend {
                source,
                info: &info,
                kind,
                start_us,
                duration_us: duration,
                track_name,
                volume,
            },
        )
    })
}

/// 替换指定片段的视频或音频素材，保留效果和时间线位置。
pub fn replace(draft: &Path, segment_id: &str, source: &Path, retime: bool) -> Result<Value> {
    let info = crate::probe::probe(source)?;
    crate::timeline_ops::mutate_with_path(draft, |timeline, work_copy, identity| {
        let material_id = timeline["tracks"]
            .as_array()
            .into_iter()
            .flatten()
            .flat_map(|track| track["segments"].as_array().into_iter().flatten())
            .find(|segment| segment["id"].as_str() == Some(segment_id))
            .and_then(|segment| segment["material_id"].as_str())
            .with_context(|| format!("segment not found or has no material: {segment_id}"))?
            .to_owned();
        let (bucket, material_index) = find_media_material(timeline, &material_id)
            .with_context(|| format!("media material not found: {material_id}"))?;
        let kind = if bucket == "audios" { "audio" } else { "video" };
        let staged = PathBuf::from(crate::draft::copy_asset(work_copy, kind, source)?);
        let stored = identity
            .join(staged.strip_prefix(work_copy).unwrap_or(&staged))
            .to_string_lossy()
            .into_owned();
        let material = &mut timeline["materials"][bucket][material_index];
        let old_path = material["path"].as_str().unwrap_or_default().to_owned();
        let old_duration = material["duration"].as_i64();
        material["path"] = json!(stored);
        material["duration"] = json!(info.duration_us);
        let filename = source
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned();
        if material.get("material_name").is_some() {
            material["material_name"] = json!(filename);
        }
        if material.get("name").is_some() {
            material["name"] = json!(filename);
        }
        if bucket == "videos" {
            material["width"] = json!(info.width);
            material["height"] = json!(info.height);
        }
        let mut shared = 0usize;
        let mut source_used = 0i64;
        for track in timeline["tracks"].as_array_mut().into_iter().flatten() {
            for segment in track["segments"].as_array_mut().into_iter().flatten() {
                if segment["material_id"].as_str() == Some(&material_id) {
                    if segment["id"].as_str() != Some(segment_id) {
                        shared += 1;
                    }
                    let start = segment["source_timerange"]["start"].as_i64().unwrap_or(0);
                    let duration = segment["source_timerange"]["duration"]
                        .as_i64()
                        .unwrap_or(0);
                    source_used = source_used.max(start + duration);
                    if retime && segment["id"].as_str() == Some(segment_id) {
                        segment["source_timerange"]["start"] = json!(0);
                        segment["source_timerange"]["duration"] = json!(info.duration_us);
                    }
                }
            }
        }
        let warning = (!retime && info.duration_us < source_used).then(|| {
            "replacement is shorter than the source range; use --retime to fit".to_owned()
        });
        Ok(
            json!({"ok":true,"segment_id":segment_id,"material_id":material_id,
            "material_type":bucket,"old_path":old_path,"new_path":stored,
            "shared_with_segments":shared,"old_duration_us":old_duration,
            "new_duration_us":info.duration_us,"source_used_us":source_used,
            "retimed":retime,"warning":warning}),
        )
    })
}

/// 按路径前缀或同名文件修复草稿中的断链素材。
pub fn relink(
    draft: &Path,
    directory: Option<&Path>,
    from: Option<&Path>,
    to: Option<&Path>,
    stage: bool,
) -> Result<Value> {
    if directory.is_none() && !(from.is_some() && to.is_some()) {
        bail!("use --dir or both --from and --to");
    }
    if directory.is_some_and(|path| !path.is_dir()) {
        bail!("relink directory does not exist");
    }
    crate::timeline_ops::mutate_with_path(draft, |timeline, work_copy, identity| {
        let mut changes = Vec::new();
        let mut missing = 0usize;
        let mut ok = 0usize;
        let mut staged = 0usize;
        let materials = timeline["materials"]
            .as_object_mut()
            .context("materials must be an object")?;
        for (bucket, entries) in materials {
            for material in entries.as_array_mut().into_iter().flatten() {
                let Some(original) = material["path"]
                    .as_str()
                    .filter(|path| !path.is_empty())
                    .map(str::to_owned)
                else {
                    continue;
                };
                let mut candidate = PathBuf::from(&original);
                let mut changed = false;
                if let (Some(from), Some(to)) = (from, to) {
                    if let Ok(suffix) = candidate.strip_prefix(from) {
                        candidate = to.join(suffix);
                        changed = true;
                    }
                }
                if !candidate.is_file() {
                    if let (Some(directory), Some(name)) = (directory, candidate.file_name()) {
                        let hit = directory.join(name);
                        if hit.is_file() {
                            candidate = hit;
                            changed = true;
                        }
                    }
                }
                let mut did_stage = false;
                if changed
                    && stage
                    && candidate.is_file()
                    && matches!(bucket.as_str(), "videos" | "audios")
                {
                    let kind = if bucket == "audios" { "audio" } else { "video" };
                    let staged_path =
                        PathBuf::from(crate::draft::copy_asset(work_copy, kind, &candidate)?);
                    candidate =
                        identity.join(staged_path.strip_prefix(work_copy).unwrap_or(&staged_path));
                    did_stage = true;
                    staged += 1;
                }
                if changed && candidate != Path::new(&original) {
                    material["path"] = json!(candidate.to_string_lossy());
                    changes.push(json!({"id":material["id"],"from":original,
                        "to":candidate,"staged":did_stage}));
                }
                if candidate.is_file() {
                    ok += 1;
                } else {
                    missing += 1;
                }
            }
        }
        Ok(
            json!({"ok":missing==0,"relinked":changes,"relinked_count":changes.len(),
            "staged":staged,"existing":ok,"missing":missing}),
        )
    })
}

/// 通过用户显式提供的本地命令合成语音，并在同一事务中加入音频轨道。
pub fn tts(
    draft: &Path,
    text: &str,
    command_template: &str,
    start_us: i64,
    duration_us: Option<i64>,
    track_name: Option<&str>,
    volume: f64,
) -> Result<Value> {
    let request = TtsRequest::new(text.trim(), TtsAudioFormat::Wav)?;
    tts_local_command(
        draft,
        command_template,
        &request,
        start_us,
        duration_us,
        track_name,
        volume,
    )
}

/// 通过无 shell 本地命令 Provider 合成语音并保留文本交付方式证据。
pub fn tts_local_command(
    draft: &Path,
    command_template: &str,
    request: &TtsRequest,
    start_us: i64,
    duration_us: Option<i64>,
    track_name: Option<&str>,
    volume: f64,
) -> Result<Value> {
    let provider = LocalCommandTtsProvider::new(command_template, request.format())?;
    let delivery = provider.text_delivery();
    let mut result = tts_provider(
        draft,
        &provider,
        request,
        start_us,
        duration_us,
        track_name,
        volume,
    )?;
    result
        .as_object_mut()
        .context("TTS result must be an object")?
        .insert("text_delivery".into(), json!(delivery));
    Ok(result)
}

/// 通过统一 Provider 合成语音并在同一 MutationPlan 中加入草稿音频轨。
#[allow(clippy::too_many_arguments)]
pub fn tts_provider(
    draft: &Path,
    provider: &dyn TtsProvider,
    request: &TtsRequest,
    start_us: i64,
    duration_us: Option<i64>,
    track_name: Option<&str>,
    volume: f64,
) -> Result<Value> {
    crate::timeline_ops::mutate_with_path(draft, |timeline, work_copy, identity| {
        let assets = work_copy.join("assets/audio");
        std::fs::create_dir_all(&assets)?;
        let output = collision_safe_path(&assets, "voiceover", audio_extension(request.format()));
        let artifact = provider.synthesize(request, &output)?;
        let bytes = artifact.byte_length();
        let info = crate::probe::probe(&output)?;
        if !info.has_audio {
            bail!("TTS command did not create a file with an audio stream");
        }
        let duration = duration_us.unwrap_or(info.duration_us);
        if duration <= 0 || duration > info.duration_us + 10_000 {
            bail!("requested duration exceeds synthesized audio duration");
        }
        let mut result = crate::draft::append_media(
            timeline,
            work_copy,
            identity,
            crate::draft::MediaAppend {
                source: &output,
                info: &info,
                kind: "audio",
                start_us,
                duration_us: duration,
                track_name,
                volume,
            },
        )?;
        let object = result
            .as_object_mut()
            .context("TTS result must be an object")?;
        object.insert("bytes".into(), json!(bytes));
        object.insert("text_chars".into(), json!(request.text().chars().count()));
        object.insert(
            "provider".into(),
            json!(provider.capability().provider_id()),
        );
        object.insert("format".into(), serde_json::to_value(request.format())?);
        object.insert("model".into(), json!(request.model()));
        object.insert("voice".into(), json!(request.voice()));
        object.insert(
            "duration_source".into(),
            json!(if duration_us.is_some() {
                "argument"
            } else {
                "ffprobe"
            }),
        );
        object.insert("media_probe".into(), serde_json::to_value(info)?);
        Ok(result)
    })
}

/// 将已由云端执行器验证并持久化的音频制品纳入同一草稿事务。
#[allow(clippy::too_many_arguments)]
pub fn tts_existing_artifact(
    draft: &Path,
    artifact_path: &Path,
    provider_id: &str,
    request: &TtsRequest,
    start_us: i64,
    duration_us: Option<i64>,
    track_name: Option<&str>,
    volume: f64,
) -> Result<Value> {
    let bytes = std::fs::metadata(artifact_path)?.len();
    if bytes == 0 {
        bail!("cloud TTS artifact is empty");
    }
    crate::timeline_ops::mutate_with_path(draft, |timeline, work_copy, identity| {
        let assets = work_copy.join("assets/audio");
        std::fs::create_dir_all(&assets)?;
        let output = collision_safe_path(&assets, "voiceover", audio_extension(request.format()));
        std::fs::copy(artifact_path, &output)?;
        let info = crate::probe::probe(&output)?;
        if !info.has_audio {
            bail!("cloud TTS artifact does not contain an audio stream");
        }
        let duration = duration_us.unwrap_or(info.duration_us);
        if duration <= 0 || duration > info.duration_us + 10_000 {
            bail!("requested duration exceeds synthesized audio duration");
        }
        let mut result = crate::draft::append_media(
            timeline,
            work_copy,
            identity,
            crate::draft::MediaAppend {
                source: &output,
                info: &info,
                kind: "audio",
                start_us,
                duration_us: duration,
                track_name,
                volume,
            },
        )?;
        let object = result
            .as_object_mut()
            .context("TTS result must be an object")?;
        object.insert("bytes".into(), json!(bytes));
        object.insert("text_chars".into(), json!(request.text().chars().count()));
        object.insert("provider".into(), json!(provider_id));
        object.insert("format".into(), serde_json::to_value(request.format())?);
        object.insert("model".into(), json!(request.model()));
        object.insert("voice".into(), json!(request.voice()));
        object.insert(
            "duration_source".into(),
            json!(if duration_us.is_some() {
                "argument"
            } else {
                "ffprobe"
            }),
        );
        object.insert("media_probe".into(), serde_json::to_value(info)?);
        Ok(result)
    })
}

fn audio_extension(format: TtsAudioFormat) -> &'static str {
    match format {
        TtsAudioFormat::Wav => "wav",
        TtsAudioFormat::Mp3 => "mp3",
        TtsAudioFormat::Pcm => "pcm",
        TtsAudioFormat::Ogg => "ogg",
        TtsAudioFormat::Aac => "aac",
        TtsAudioFormat::Flac => "flac",
        TtsAudioFormat::Aiff => "aiff",
    }
}

/// 新增一个由固定资源 ID 描述的 CapCut 内置音效片段。
pub fn add_sfx(
    draft: &Path,
    slug: &str,
    start_us: i64,
    duration_us: i64,
    track_name: Option<&str>,
    volume: f64,
) -> Result<Value> {
    if start_us < 0 || duration_us <= 0 || !volume.is_finite() || volume < 0.0 {
        bail!("invalid SFX timing or volume");
    }
    let entry = crate::catalogs::capcut_audio_effects()
        .as_array()
        .into_iter()
        .flatten()
        .find(|entry| {
            entry["slug"]
                .as_str()
                .is_some_and(|value| value.eq_ignore_ascii_case(slug))
                || entry["member"]
                    .as_str()
                    .is_some_and(|value| value.eq_ignore_ascii_case(slug))
        })
        .with_context(|| format!("unknown SFX slug: {slug}"))?
        .clone();
    crate::timeline_ops::mutate(draft, |timeline| {
        let material_id = Uuid::new_v4().to_string();
        let segment_id = Uuid::new_v4().to_string();
        let wanted_name = track_name.unwrap_or("sfx");
        timeline["materials"]["audio_effects"]
            .as_array_mut()
            .context("materials.audio_effects must be an array")?
            .push(json!({
                "id":material_id,"name":entry["name"],"effect_id":entry["effect_id"],
                "resource_id":entry["resource_id"],"formula_id":"","is_vip":entry["is_vip"],
                "md5":entry["md5"],"type":"sound_effect","category_id":"",
                "category_name":"","path":"","platform":"all","source_platform":0,"version":""
            }));
        let segment = json!({
            "id":segment_id,"material_id":material_id,
            "target_timerange":{"start":start_us,"duration":duration_us},
            "source_timerange":{"start":0,"duration":duration_us},
            "speed":1.0,"volume":volume,"visible":true,"clip":null,
            "extra_material_refs":[],"render_index":0
        });
        let tracks = timeline["tracks"]
            .as_array_mut()
            .context("tracks must be an array")?;
        let track_id;
        if let Some(track) = tracks.iter_mut().find(|track| {
            track["type"].as_str() == Some("audio") && track["name"].as_str() == Some(wanted_name)
        }) {
            track_id = track["id"].as_str().unwrap_or_default().to_owned();
            track["segments"]
                .as_array_mut()
                .context("track segments must be an array")?
                .push(segment);
        } else {
            track_id = Uuid::new_v4().to_string();
            tracks.push(json!({"attribute":0,"flag":0,"id":track_id,
                "is_default_name":track_name.is_none(),"name":wanted_name,
                "segments":[segment],"type":"audio"}));
        }
        Ok(
            json!({"ok":true,"segmentId":segment_id,"materialId":material_id,
            "trackId":track_id,"name":entry["name"],"slug":slug,
            "start_us":start_us,"duration_us":duration_us}),
        )
    })
}

fn collision_safe_path(directory: &Path, stem: &str, extension: &str) -> PathBuf {
    for index in 1.. {
        let name = if index == 1 {
            format!("{stem}.{extension}")
        } else {
            format!("{stem}-{index}.{extension}")
        };
        let candidate = directory.join(name);
        if !candidate.exists() {
            return candidate;
        }
    }
    unreachable!()
}

fn find_media_material(timeline: &Value, id: &str) -> Option<(&'static str, usize)> {
    for bucket in ["videos", "audios"] {
        if let Some(index) = timeline["materials"][bucket]
            .as_array()?
            .iter()
            .position(|material| material["id"].as_str() == Some(id))
        {
            return Some((bucket, index));
        }
    }
    None
}
