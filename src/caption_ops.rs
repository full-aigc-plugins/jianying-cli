//! 字幕查询、导入导出和无损事务化编辑。

use anyhow::{bail, Context, Result};
use serde_json::{json, Value};
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};
use uuid::Uuid;

/// 文字入场/出场动画参数。
pub struct TextAnimationOptions<'a> {
    pub intro: Option<&'a str>,
    pub outro: Option<&'a str>,
    pub intro_duration_us: Option<i64>,
    pub outro_duration_us: Option<i64>,
    pub jianying: bool,
}

/// 列出全部文字轨字幕，按时间和轨道顺序稳定排序。
pub fn list(draft: &Path) -> Result<Value> {
    let timeline = crate::draft::load_timeline(draft)?;
    let mut captions = caption_values(&timeline)?;
    captions.sort_by_key(|item| {
        (
            item["start_us"].as_i64().unwrap_or(0),
            item["track_index"].as_u64().unwrap_or(0),
            item["index"].as_u64().unwrap_or(0),
        )
    });
    Ok(Value::Array(captions))
}

/// 读取单条字幕及其基础样式。
pub fn get(draft: &Path, segment_id: &str) -> Result<Value> {
    list(draft)?
        .as_array()
        .and_then(|items| {
            items
                .iter()
                .find(|item| item["segment_id"].as_str() == Some(segment_id))
        })
        .cloned()
        .ok_or_else(|| anyhow::anyhow!("caption segment not found: {segment_id}"))
}

/// 为文字片段写入 CapCut/剪映入场与出场动画。
pub fn animation(
    draft: &Path,
    segment_id: &str,
    options: TextAnimationOptions<'_>,
) -> Result<Value> {
    if options.intro.is_none() && options.outro.is_none() {
        bail!("at least one of --intro or --outro is required");
    }
    let intro = options
        .intro
        .map(|slug| resolve_text_animation(slug, "intros", options.jianying))
        .transpose()?;
    let outro = options
        .outro
        .map(|slug| resolve_text_animation(slug, "outros", options.jianying))
        .transpose()?;
    crate::timeline_ops::mutate(draft, |timeline| {
        if !timeline["materials"]["material_animations"].is_array() {
            timeline["materials"]["material_animations"] = json!([]);
        }
        let animation_ids: std::collections::HashSet<String> = timeline["materials"]
            ["material_animations"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|item| item["id"].as_str().map(str::to_owned))
            .collect();
        let wanted = segment_id.to_ascii_lowercase();
        let segment = timeline["tracks"]
            .as_array_mut()
            .context("tracks must be an array")?
            .iter_mut()
            .flat_map(|track| track["segments"].as_array_mut().into_iter().flatten())
            .find(|segment| {
                segment["id"].as_str().is_some_and(|id| {
                    id == segment_id || id.to_ascii_lowercase().starts_with(&wanted)
                })
            })
            .with_context(|| format!("Segment not found: {segment_id}"))?;
        let resolved_id = segment["id"]
            .as_str()
            .context("segment id must be a string")?
            .to_owned();
        let target_duration = segment["target_timerange"]["duration"]
            .as_i64()
            .context("segment duration must be an integer")?;
        if !segment["extra_material_refs"].is_array() {
            segment["extra_material_refs"] = json!([]);
        }
        let existing = segment["extra_material_refs"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(Value::as_str)
            .find(|id| animation_ids.contains(*id))
            .map(str::to_owned);
        let container_id = existing.unwrap_or_else(|| Uuid::new_v4().to_string());
        if !segment["extra_material_refs"]
            .as_array()
            .unwrap()
            .iter()
            .any(|item| item.as_str() == Some(&container_id))
        {
            segment["extra_material_refs"]
                .as_array_mut()
                .unwrap()
                .push(json!(container_id));
        }
        let containers = timeline["materials"]["material_animations"]
            .as_array_mut()
            .unwrap();
        let container_index = if let Some(index) = containers
            .iter()
            .position(|item| item["id"].as_str() == Some(&container_id))
        {
            index
        } else {
            containers.push(json!({"animations":[],"id":container_id,"multi_language_current":"none","type":"sticker_animation"}));
            containers.len() - 1
        };
        let animations = containers[container_index]["animations"]
            .as_array_mut()
            .context("animation container animations must be an array")?;
        let mut added = Vec::new();
        if let Some(entry) = intro.as_ref() {
            add_text_animation_entry(
                animations,
                entry,
                "in",
                options.intro_duration_us,
                target_duration,
                &mut added,
            )?;
        }
        if let Some(entry) = outro.as_ref() {
            add_text_animation_entry(
                animations,
                entry,
                "out",
                options.outro_duration_us,
                target_duration,
                &mut added,
            )?;
        }
        Ok(json!({"ok":true,"segmentId":resolved_id,"added":added,"material_id":container_id}))
    })
}

fn resolve_text_animation(slug: &str, bucket: &str, jianying: bool) -> Result<Value> {
    let alias = match slug {
        "blur-text-in" => "blur",
        "zoom-in-text" => "zoom-in",
        other => other,
    };
    let catalogue = if jianying {
        crate::catalogs::capcut_jianying_text_animations()
    } else {
        crate::catalogs::capcut_text_animations()
    };
    catalogue[bucket]
        .as_array()
        .into_iter()
        .flatten()
        .find(|entry| {
            entry["slug"]
                .as_str()
                .is_some_and(|value| !value.is_empty() && value.eq_ignore_ascii_case(alias))
                || entry["member"]
                    .as_str()
                    .is_some_and(|value| value.eq_ignore_ascii_case(alias))
        })
        .cloned()
        .with_context(|| {
            format!(
                "Unknown text {}: {slug}",
                if bucket == "intros" { "intro" } else { "outro" }
            )
        })
}

fn add_text_animation_entry(
    animations: &mut Vec<Value>,
    entry: &Value,
    animation_type: &str,
    override_duration: Option<i64>,
    target_duration: i64,
    added: &mut Vec<Value>,
) -> Result<()> {
    if animations
        .iter()
        .any(|animation| animation["type"].as_str() == Some(animation_type))
    {
        bail!("segment already has a {animation_type} text animation");
    }
    let duration = override_duration.unwrap_or_else(|| {
        entry["duration"]
            .as_i64()
            .or_else(|| entry["default_duration"].as_i64())
            .unwrap_or(500_000)
    });
    if duration > target_duration {
        bail!("duration ({duration}us) exceeds segment duration ({target_duration}us)");
    }
    let start = if animation_type == "out" {
        target_duration - duration
    } else {
        0
    };
    let name = entry["title"]
        .as_str()
        .or_else(|| entry["name"].as_str())
        .unwrap_or_default();
    let category = if animation_type == "in" {
        "in_fav"
    } else {
        "out_fav"
    };
    animations.push(json!({
        "anim_adjust_params":null,"category_id":category,"category_name":category,
        "duration":duration,"id":entry["effect_id"],"material_type":"text","name":name,
        "panel":"","path":"","platform":"all","request_id":"","resource_id":entry["resource_id"],
        "source_platform":1,"start":start,"third_resource_id":"","type":animation_type
    }));
    added.push(json!({"type":animation_type,"name":name,"duration_us":duration,"start_us":start}));
    Ok(())
}

/// 新增一条字幕，使用微秒时间范围和具名文字轨。
pub fn add(
    draft: &Path,
    text: &str,
    start_us: i64,
    duration_us: i64,
    track_name: &str,
) -> Result<Value> {
    validate_interval(text, start_us, duration_us)?;
    crate::timeline_ops::mutate(draft, |timeline| {
        add_to_timeline(timeline, text, start_us, duration_us, track_name, None)
    })
}

/// 替换单条字幕文本，并按 UTF-16 码元重算样式范围。
pub fn set_text(draft: &Path, segment_id: &str, text: &str) -> Result<Value> {
    if text.is_empty() {
        bail!("caption text must not be empty");
    }
    crate::timeline_ops::mutate(draft, |timeline| {
        set_text_in_timeline(timeline, segment_id, text)?;
        Ok(json!({"ok":true,"segment_id":segment_id,"text":text,
            "utf16_length":text.encode_utf16().count()}))
    })
}

/// 修改字幕基础样式；未提供的字段保持原值。
#[allow(clippy::too_many_arguments)]
pub fn style(
    draft: &Path,
    segment_id: &str,
    size: Option<f64>,
    color: Option<&str>,
    bold: Option<bool>,
    italic: Option<bool>,
    underline: Option<bool>,
    alignment: Option<u8>,
) -> Result<Value> {
    if size.is_some_and(|value| !value.is_finite() || value <= 0.0) {
        bail!("caption size must be a positive finite number");
    }
    if alignment.is_some_and(|value| value > 2) {
        bail!("caption alignment must be 0, 1 or 2");
    }
    let rgb = color.map(parse_color).transpose()?;
    crate::timeline_ops::mutate(draft, |timeline| {
        let material_id = caption_material_id(timeline, segment_id)?;
        let material = text_material_mut(timeline, &material_id)?;
        let mut content = read_content(material)?;
        let text = content["text"].as_str().unwrap_or_default();
        let utf16_length = text.encode_utf16().count() as i64;
        let styles = content["styles"]
            .as_array_mut()
            .context("caption content styles must be an array")?;
        if styles.is_empty() {
            styles.push(default_style(utf16_length));
        }
        let base = &mut styles[0];
        base["range"] = json!([0, utf16_length]);
        if let Some(value) = size {
            base["size"] = json!(value);
            material["font_size"] = json!(value);
        }
        if let Some(value) = rgb {
            base["fill"] = json!({"alpha":1.0,"content":{"render_type":"solid",
                "solid":{"alpha":1.0,"color":value}}});
            material["text_color"] = json!(color.unwrap_or_default().to_uppercase());
        }
        for (key, value) in [("bold", bold), ("italic", italic), ("underline", underline)] {
            if let Some(value) = value {
                base[key] = json!(value);
            }
        }
        if let Some(value) = alignment {
            material["alignment"] = json!(value);
        }
        write_content(material, content)?;
        Ok(json!({"ok":true,"segment_id":segment_id,"style":style_summary(material)?}))
    })
}

/// 按 UTF-16 码元区间替换字幕的多段样式；未覆盖区间继承原首样式。
pub fn style_ranges(draft: &Path, segment_id: &str, ranges: &[Value]) -> Result<Value> {
    if ranges.is_empty() {
        bail!("at least one range required");
    }
    crate::timeline_ops::mutate(draft, |timeline| {
        let wanted = segment_id.to_ascii_lowercase();
        let segment = timeline["tracks"]
            .as_array()
            .context("tracks must be an array")?
            .iter()
            .filter(|track| track["type"].as_str() == Some("text"))
            .flat_map(|track| track["segments"].as_array().into_iter().flatten())
            .find(|segment| {
                segment["id"].as_str().is_some_and(|candidate| {
                    candidate == segment_id || candidate.to_ascii_lowercase().starts_with(&wanted)
                })
            })
            .with_context(|| format!("Segment not found: {segment_id}"))?;
        let resolved_segment_id = segment["id"]
            .as_str()
            .context("text segment requires string id")?
            .to_owned();
        let material_id = segment["material_id"]
            .as_str()
            .context("text segment requires material_id")?
            .to_owned();

        let material = text_material_mut(timeline, &material_id)?;
        let mut content = read_content(material)?;
        let text = content["text"].as_str().unwrap_or_default();
        let text_length = text.encode_utf16().count() as i64;
        let base = content["styles"]
            .as_array()
            .and_then(|styles| styles.first())
            .cloned()
            .unwrap_or_else(|| json!({}));
        let base_solid = &base["fill"]["content"]["solid"];
        let default_color = base_solid["color"]
            .as_array()
            .map(|color| Value::Array(color.clone()))
            .unwrap_or_else(|| json!([1.0, 1.0, 1.0]));
        let default_alpha = value_or(&base_solid["alpha"], json!(1.0));
        let default_size = value_or(&base["size"], json!(15.0));
        let default_bold = value_or(&base["bold"], json!(false));
        let default_italic = value_or(&base["italic"], json!(false));
        let default_underline = value_or(&base["underline"], json!(false));

        let mut sorted = Vec::with_capacity(ranges.len());
        for range in ranges {
            let object = range
                .as_object()
                .context("each style range must be a JSON object")?;
            let start = object
                .get("start")
                .and_then(Value::as_i64)
                .context("range {start,end} must be integers")?;
            let end = object
                .get("end")
                .and_then(Value::as_i64)
                .context("range {start,end} must be integers")?;
            if start < 0 || end > text_length {
                bail!("range [{start},{end}) out of bounds (text length={text_length})");
            }
            if end <= start {
                bail!("range [{start},{end}) must have end > start");
            }
            sorted.push((start, end, range));
        }
        sorted.sort_by_key(|(start, _, _)| *start);
        for pair in sorted.windows(2) {
            if pair[1].0 < pair[0].1 {
                bail!(
                    "overlapping ranges: [{},{}) and [{},{})",
                    pair[0].0,
                    pair[0].1,
                    pair[1].0,
                    pair[1].1
                );
            }
        }

        let make_style = |start: i64, end: i64, range: Option<&Value>| -> Result<Value> {
            let override_value = |key: &str, fallback: &Value| {
                range
                    .and_then(|item| item.get(key))
                    .filter(|value| !value.is_null())
                    .cloned()
                    .unwrap_or_else(|| fallback.clone())
            };
            let color = match range
                .and_then(|item| item.get("font_color"))
                .filter(|value| !value.is_null())
            {
                Some(value) => json!(parse_color(
                    value
                        .as_str()
                        .context("font_color must be a #RRGGBB string")?
                )?),
                None => default_color.clone(),
            };
            Ok(json!({
                "range":[start,end],
                "size":override_value("font_size", &default_size),
                "bold":override_value("bold", &default_bold),
                "italic":override_value("italic", &default_italic),
                "underline":override_value("underline", &default_underline),
                "fill":{"alpha":1,"content":{"render_type":"solid","solid":{
                    "alpha":override_value("font_alpha", &default_alpha),"color":color
                }}}
            }))
        };

        let mut styles = Vec::new();
        let mut cursor = 0;
        for (start, end, range) in sorted {
            if start > cursor {
                styles.push(make_style(cursor, start, None)?);
            }
            styles.push(make_style(start, end, Some(range))?);
            cursor = end;
        }
        if cursor < text_length {
            styles.push(make_style(cursor, text_length, None)?);
        }
        let style_count = styles.len();
        content["styles"] = Value::Array(styles);
        write_content(material, content)?;
        Ok(
            json!({"ok":true,"segmentId":resolved_segment_id,"material_id":material_id,
            "styles":style_count,"text_length":text_length}),
        )
    })
}

fn value_or(value: &Value, fallback: Value) -> Value {
    if value.is_null() {
        fallback
    } else {
        value.clone()
    }
}

/// 为文字片段设置气泡形状，对应 capcut-cli `setBubble`。
pub fn bubble(
    draft: &Path,
    segment_id: &str,
    slug: Option<&str>,
    effect_id: Option<&str>,
    resource_id: Option<&str>,
) -> Result<Value> {
    let (effect_id, resource_id) = resolve_bubble(slug, effect_id, resource_id)?;
    crate::timeline_ops::mutate(draft, |timeline| {
        let wanted = segment_id.to_ascii_lowercase();
        let tracks = timeline["tracks"]
            .as_array()
            .context("tracks must be an array")?;
        let (track_index, segment_index) = tracks
            .iter()
            .enumerate()
            .find_map(|(track_index, track)| {
                track["segments"]
                    .as_array()?
                    .iter()
                    .position(|segment| {
                        segment["id"].as_str().is_some_and(|candidate| {
                            candidate == segment_id
                                || candidate.to_ascii_lowercase().starts_with(&wanted)
                        })
                    })
                    .map(|segment_index| (track_index, segment_index))
            })
            .with_context(|| format!("Segment not found: {segment_id}"))?;
        let track_type = timeline["tracks"][track_index]["type"]
            .as_str()
            .unwrap_or_default();
        if track_type != "text" {
            bail!("bubble-text only applies to text segments (track type: {track_type})");
        }
        let resolved_segment_id = timeline["tracks"][track_index]["segments"][segment_index]["id"]
            .as_str()
            .context("text segment requires string id")?
            .to_owned();
        let material_id = timeline["tracks"][track_index]["segments"][segment_index]["material_id"]
            .as_str()
            .context("text segment requires material_id")?
            .to_owned();
        let old_bubbles = timeline["materials"]["filters"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|item| item["type"].as_str() == Some("text_shape"))
            .filter_map(|item| item["id"].as_str().map(str::to_owned))
            .collect::<std::collections::HashSet<_>>();
        timeline["tracks"][track_index]["segments"][segment_index]["extra_material_refs"]
            .as_array_mut()
            .context("extra_material_refs must be an array")?
            .retain(|item| {
                item.as_str()
                    .is_none_or(|reference| !old_bubbles.contains(reference))
            });
        let text = text_material_mut(timeline, &material_id)
            .with_context(|| format!("Text material not found for segment {segment_id}"))?;
        text["bubble_effect_id"] = json!(effect_id);
        text["bubble_resource_id"] = json!(resource_id);
        if !timeline["materials"]["filters"].is_array() {
            timeline["materials"]["filters"] = json!([]);
        }
        let bubble_id = Uuid::new_v4().simple().to_string();
        timeline["materials"]["filters"]
            .as_array_mut()
            .unwrap()
            .push(json!({
                "id":bubble_id,"apply_target_type":0,"effect_id":effect_id,
                "resource_id":resource_id,"type":"text_shape","value":1.0
            }));
        timeline["tracks"][track_index]["segments"][segment_index]["extra_material_refs"]
            .as_array_mut()
            .unwrap()
            .push(json!(bubble_id));
        Ok(json!({
            "ok":true,"segmentId":resolved_segment_id,"bubble_id":bubble_id,
            "effect_id":effect_id,"resource_id":resource_id
        }))
    })
}

fn resolve_bubble(
    slug: Option<&str>,
    effect_id: Option<&str>,
    resource_id: Option<&str>,
) -> Result<(String, String)> {
    let mut effect_id = effect_id.map(str::to_owned);
    let mut resource_id = resource_id.map(str::to_owned);
    if (effect_id.is_none() || resource_id.is_none()) && slug.is_some() {
        let slug = slug.unwrap_or_default();
        let entry = crate::catalogs::capcut_bubbles()
            .as_array()
            .into_iter()
            .flatten()
            .find(|entry| {
                entry["slug"]
                    .as_str()
                    .is_some_and(|candidate| candidate.eq_ignore_ascii_case(slug))
            })
            .with_context(|| {
                format!("Unknown bubble slug: {slug}. Run 'capcut enums --bubbles' or pass --effect-id / --resource-id directly.")
            })?;
        effect_id.get_or_insert_with(|| entry["effect_id"].as_str().unwrap_or_default().to_owned());
        resource_id
            .get_or_insert_with(|| entry["resource_id"].as_str().unwrap_or_default().to_owned());
    }
    match (effect_id, resource_id) {
        (Some(effect_id), Some(resource_id)) => Ok((effect_id, resource_id)),
        _ => bail!(
            "bubble-text requires either --bubble <slug> or both --effect-id and --resource-id"
        ),
    }
}

/// 将 SRT 文件导入既有草稿的具名文字轨。
#[allow(clippy::too_many_arguments)]
pub fn import_srt(
    draft: &Path,
    file: &Path,
    track_name: &str,
    offset_us: i64,
    size: Option<f64>,
    color: Option<&str>,
    bold: Option<bool>,
) -> Result<Value> {
    if let Some(value) = color {
        parse_color(value)?;
    }
    let cues = crate::srt::parse(&std::fs::read_to_string(file)?)?;
    import_cues(draft, track_name, offset_us, size, color, bold, cues)
}

/// 将标准 ASS Events/Dialogue 行导入既有草稿。
pub fn import_ass(draft: &Path, file: &Path, track_name: &str, offset_us: i64) -> Result<Value> {
    let cues = parse_ass(&std::fs::read_to_string(file)?)?;
    import_cues(draft, track_name, offset_us, None, None, None, cues)
}

/// 将全部字幕按时间顺序导出为 UTF-8 SRT。
pub fn export_srt(draft: &Path, out: &Path) -> Result<Value> {
    let items = list(draft)?;
    let mut body = String::new();
    for (index, item) in items.as_array().into_iter().flatten().enumerate() {
        body.push_str(&format!(
            "{}\n{} --> {}\n{}\n\n",
            index + 1,
            srt_time(item["start_us"].as_i64().unwrap_or(0)),
            srt_time(item["end_us"].as_i64().unwrap_or(0)),
            item["text"].as_str().unwrap_or_default()
        ));
    }
    atomic_write(out, body.as_bytes())?;
    Ok(
        json!({"ok":true,"format":"srt","out":out,"captions":items.as_array().map(Vec::len).unwrap_or(0)}),
    )
}

/// 将全部字幕导出为可直接读取的 ASS v4+ 文本。
pub fn export_ass(draft: &Path, out: &Path) -> Result<Value> {
    let items = list(draft)?;
    let mut body = String::from(
        "[Script Info]\nScriptType: v4.00+\nPlayResX: 1920\nPlayResY: 1080\n\n\
[V4+ Styles]\nFormat: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding\n\
Style: Default,Arial,48,&H00FFFFFF,&H000000FF,&H00000000,&H00000000,0,0,0,0,100,100,0,0,1,2,0,2,10,10,10,1\n\n\
[Events]\nFormat: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text\n",
    );
    for item in items.as_array().into_iter().flatten() {
        let text = item["text"]
            .as_str()
            .unwrap_or_default()
            .replace('\n', "\\N");
        body.push_str(&format!(
            "Dialogue: 0,{},{},Default,,0,0,0,,{}\n",
            ass_time(item["start_us"].as_i64().unwrap_or(0)),
            ass_time(item["end_us"].as_i64().unwrap_or(0)),
            text
        ));
    }
    atomic_write(out, body.as_bytes())?;
    Ok(
        json!({"ok":true,"format":"ass","out":out,"captions":items.as_array().map(Vec::len).unwrap_or(0)}),
    )
}

/// 应用离线翻译映射。文件格式为 `by_id` 和可选 `by_text` 两个 JSON 对象。
pub fn translate(draft: &Path, translations: &Path) -> Result<Value> {
    let mapping: Value = serde_json::from_str(&std::fs::read_to_string(translations)?)?;
    apply_translation_mapping(draft, &mapping, json!(translations), "mapping-file")
}

/// 通过绝对路径的结构化进程 provider 翻译字幕，不经过 shell。
pub fn translate_with_command(
    draft: &Path,
    executable: &Path,
    target_language: &str,
) -> Result<Value> {
    if !executable.is_absolute() || !executable.is_file() {
        bail!("translation provider must be an existing absolute executable path");
    }
    if target_language.trim().is_empty() {
        bail!("translation target language must not be empty");
    }
    let captions = list(draft)?;
    let request = json!({
        "schema":"jianying-caption-translation/v1",
        "target_language":target_language,
        "captions":captions.as_array().into_iter().flatten().map(|item| json!({
            "segment_id":item["segment_id"],"text":item["text"]
        })).collect::<Vec<_>>()
    });
    let mut child = Command::new(executable)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .with_context(|| format!("starting translation provider {}", executable.display()))?;
    child
        .stdin
        .take()
        .context("translation provider stdin unavailable")?
        .write_all(&serde_json::to_vec(&request)?)?;
    let output = child.wait_with_output()?;
    if !output.status.success() {
        bail!(
            "translation provider failed with status {}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    let mapping: Value = serde_json::from_slice(&output.stdout)
        .context("translation provider stdout must be one JSON mapping document")?;
    apply_translation_mapping(draft, &mapping, json!(executable), "structured-command")
}

fn apply_translation_mapping(
    draft: &Path,
    mapping: &Value,
    source: Value,
    provider_kind: &str,
) -> Result<Value> {
    let by_id = mapping["by_id"].as_object().cloned().unwrap_or_default();
    let by_text = mapping["by_text"].as_object().cloned().unwrap_or_default();
    if by_id.is_empty() && by_text.is_empty() {
        bail!("translation mapping requires non-empty by_id or by_text object");
    }
    crate::timeline_ops::mutate(draft, |timeline| {
        let captions = caption_values(timeline)?;
        let known: std::collections::BTreeSet<&str> = captions
            .iter()
            .filter_map(|item| item["segment_id"].as_str())
            .collect();
        if let Some(unknown) = by_id.keys().find(|id| !known.contains(id.as_str())) {
            bail!("translation references unknown caption segment: {unknown}");
        }
        let mut changes = Vec::new();
        for item in captions {
            let id = item["segment_id"].as_str().unwrap_or_default();
            let old = item["text"].as_str().unwrap_or_default();
            let replacement = by_id
                .get(id)
                .or_else(|| by_text.get(old))
                .and_then(Value::as_str);
            if let Some(new_text) = replacement {
                if new_text.is_empty() {
                    bail!("translated caption text must not be empty: {id}");
                }
                set_text_in_timeline(timeline, id, new_text)?;
                changes.push(json!({"segment_id":id,"from":old,"to":new_text}));
            }
        }
        Ok(
            json!({"ok":true,"translated":changes.len(),"changes":changes,
            "source":source,"provider_kind":provider_kind}),
        )
    })
}

fn import_cues(
    draft: &Path,
    track_name: &str,
    offset_us: i64,
    size: Option<f64>,
    color: Option<&str>,
    bold: Option<bool>,
    cues: Vec<crate::srt::Cue>,
) -> Result<Value> {
    if cues.is_empty() {
        bail!("subtitle file contains no cues");
    }
    crate::timeline_ops::mutate(draft, |timeline| {
        let mut ids = Vec::new();
        for cue in &cues {
            let start_us = (cue.start_us + offset_us).max(0);
            let output = add_to_timeline(
                timeline,
                &cue.text,
                start_us,
                cue.end_us - cue.start_us,
                track_name,
                Some((size, color, bold)),
            )?;
            ids.push(output["segment_id"].clone());
        }
        Ok(json!({"ok":true,"imported":ids.len(),"segment_ids":ids,
            "track_name":track_name,"offset_us":offset_us}))
    })
}

pub(crate) fn add_to_timeline(
    timeline: &mut Value,
    text: &str,
    start_us: i64,
    duration_us: i64,
    track_name: &str,
    style: Option<(Option<f64>, Option<&str>, Option<bool>)>,
) -> Result<Value> {
    validate_interval(text, start_us, duration_us)?;
    let material_id = Uuid::new_v4().simple().to_string();
    let segment_id = Uuid::new_v4().simple().to_string();
    let (size, color, bold) = style.unwrap_or((None, None, None));
    let size = size.unwrap_or(5.0);
    let color = color.unwrap_or("#FFFFFF");
    let rgb = parse_color(color)?;
    let utf16_length = text.encode_utf16().count() as i64;
    let content = json!({"text":text,"styles":[{
        "range":[0,utf16_length],"size":size,"bold":bold.unwrap_or(false),
        "italic":false,"underline":false,"strokes":[],
        "fill":{"alpha":1.0,"content":{"render_type":"solid",
            "solid":{"alpha":1.0,"color":rgb}}}
    }]});
    let material = json!({
        "id":material_id,"type":"subtitle","content":content.to_string(),
        "alignment":1,"font_size":size,"text_color":color.to_uppercase(),
        "typesetting":0,"letter_spacing":0,"line_spacing":0.02,"line_feed":1,
        "line_max_width":0.82,"force_apply_line_max_width":false,"check_flag":7,
        "global_alpha":1.0,"fixed_width":-1,"fixed_height":-1
    });
    timeline["materials"]["texts"]
        .as_array_mut()
        .context("materials.texts must be an array")?
        .push(material);
    let segment = json!({
        "id":segment_id,"material_id":material_id,
        "target_timerange":{"start":start_us,"duration":duration_us},
        "source_timerange":{"start":0,"duration":duration_us},
        "speed":1.0,"volume":1.0,"visible":true,"reverse":false,
        "enable_adjust":true,"enable_color_correct_adjust":false,
        "enable_color_curves":true,"enable_color_match_adjust":false,
        "enable_color_wheels":true,"enable_lut":true,"enable_smart_color_adjust":false,
        "last_nonzero_volume":1.0,"is_tone_modify":false,"dur":duration_us,
        "render_index":0,"track_render_index":0,"track_attribute":0,
        "extra_material_refs":[],"common_keyframes":[],"keyframe_refs":[],
        "clip":{"alpha":1.0,"rotation":0.0,"scale":{"x":1.0,"y":1.0},
            "transform":{"x":0.0,"y":-0.8},"flip":{"horizontal":false,"vertical":false}},
        "uniform_scale":{"on":true,"value":1.0}
    });
    let tracks = timeline["tracks"]
        .as_array_mut()
        .context("tracks must be an array")?;
    let track_index = tracks.iter().position(|track| {
        track["type"].as_str() == Some("text") && track["name"].as_str() == Some(track_name)
    });
    let track_id;
    if let Some(index) = track_index {
        track_id = tracks[index]["id"].as_str().unwrap_or_default().to_owned();
        let segments = tracks[index]["segments"]
            .as_array_mut()
            .context("text track segments must be an array")?;
        segments.push(segment);
        segments.sort_by_key(|item| item["target_timerange"]["start"].as_i64().unwrap_or(0));
    } else {
        track_id = Uuid::new_v4().simple().to_string();
        tracks.push(json!({"attribute":0,"flag":0,"id":track_id,
            "is_default_name":false,"name":track_name,"segments":[segment],"type":"text"}));
    }
    Ok(
        json!({"ok":true,"segment_id":segment_id,"material_id":material_id,
        "track_id":track_id,"track_name":track_name,"start_us":start_us,
        "duration_us":duration_us}),
    )
}

fn caption_values(timeline: &Value) -> Result<Vec<Value>> {
    let mut output = Vec::new();
    for (track_index, track) in timeline["tracks"]
        .as_array()
        .context("tracks must be an array")?
        .iter()
        .enumerate()
    {
        if track["type"].as_str() != Some("text") {
            continue;
        }
        for (index, segment) in track["segments"]
            .as_array()
            .context("text track segments must be an array")?
            .iter()
            .enumerate()
        {
            let material_id = segment["material_id"].as_str().unwrap_or_default();
            let material = text_material(timeline, material_id)?;
            let content = read_content(material)?;
            let text = content["text"].as_str().unwrap_or_default();
            let start = segment["target_timerange"]["start"].as_i64().unwrap_or(0);
            let duration = segment["target_timerange"]["duration"]
                .as_i64()
                .unwrap_or(0);
            output.push(json!({
                "segment_id":segment["id"],"material_id":material_id,
                "track_id":track["id"],"track_name":track["name"],
                "track_index":track_index,"index":index,"start_us":start,
                "duration_us":duration,"end_us":start+duration,"text":text,
                "utf16_length":text.encode_utf16().count(),"style":style_summary(material)?
            }));
        }
    }
    Ok(output)
}

fn caption_material_id(timeline: &Value, segment_id: &str) -> Result<String> {
    timeline["tracks"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|track| track["type"].as_str() == Some("text"))
        .flat_map(|track| track["segments"].as_array().into_iter().flatten())
        .find(|segment| segment["id"].as_str() == Some(segment_id))
        .and_then(|segment| segment["material_id"].as_str())
        .map(str::to_owned)
        .ok_or_else(|| anyhow::anyhow!("caption segment not found: {segment_id}"))
}

fn set_text_in_timeline(timeline: &mut Value, segment_id: &str, text: &str) -> Result<()> {
    let material_id = caption_material_id(timeline, segment_id)?;
    let material = text_material_mut(timeline, &material_id)?;
    let mut content = read_content(material)?;
    content["text"] = json!(text);
    let length = text.encode_utf16().count() as i64;
    let styles = content["styles"]
        .as_array_mut()
        .context("caption content styles must be an array")?;
    if styles.is_empty() {
        styles.push(default_style(length));
    }
    styles[0]["range"] = json!([0, length]);
    for style in styles.iter_mut().skip(1) {
        if let Some(range) = style["range"].as_array_mut() {
            if range.len() == 2 {
                let start = range[0].as_i64().unwrap_or(0).clamp(0, length);
                let end = range[1].as_i64().unwrap_or(length).clamp(start, length);
                range[0] = json!(start);
                range[1] = json!(end);
            }
        }
    }
    write_content(material, content)
}

fn text_material<'a>(timeline: &'a Value, material_id: &str) -> Result<&'a Value> {
    timeline["materials"]["texts"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|material| material["id"].as_str() == Some(material_id))
        .ok_or_else(|| anyhow::anyhow!("text material not found: {material_id}"))
}

fn text_material_mut<'a>(timeline: &'a mut Value, material_id: &str) -> Result<&'a mut Value> {
    timeline["materials"]["texts"]
        .as_array_mut()
        .into_iter()
        .flatten()
        .find(|material| material["id"].as_str() == Some(material_id))
        .ok_or_else(|| anyhow::anyhow!("text material not found: {material_id}"))
}

fn read_content(material: &Value) -> Result<Value> {
    match &material["content"] {
        Value::String(raw) => Ok(serde_json::from_str(raw)?),
        Value::Object(_) => Ok(material["content"].clone()),
        _ => bail!("text material content must be a JSON string or object"),
    }
}

fn write_content(material: &mut Value, content: Value) -> Result<()> {
    material["content"] = match &material["content"] {
        Value::Object(_) => content,
        _ => Value::String(serde_json::to_string(&content)?),
    };
    Ok(())
}

fn style_summary(material: &Value) -> Result<Value> {
    let content = read_content(material)?;
    let base = content["styles"]
        .as_array()
        .and_then(|styles| styles.first());
    Ok(json!({
        "size":base.and_then(|style| style["size"].as_f64()).or_else(|| material["font_size"].as_f64()),
        "color":material["text_color"],
        "bold":base.and_then(|style| style["bold"].as_bool()).unwrap_or(false),
        "italic":base.and_then(|style| style["italic"].as_bool()).unwrap_or(false),
        "underline":base.and_then(|style| style["underline"].as_bool()).unwrap_or(false),
        "alignment":material["alignment"]
    }))
}

fn default_style(length: i64) -> Value {
    json!({"range":[0,length],"size":5.0,"bold":false,"italic":false,
        "underline":false,"strokes":[],"fill":{"alpha":1.0,"content":{
            "render_type":"solid","solid":{"alpha":1.0,"color":[1.0,1.0,1.0]}}}})
}

fn parse_color(value: &str) -> Result<[f64; 3]> {
    let hex = value.strip_prefix('#').unwrap_or(value);
    if hex.len() != 6 || !hex.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        bail!("caption color must be #RRGGBB");
    }
    Ok([
        u8::from_str_radix(&hex[0..2], 16)? as f64 / 255.0,
        u8::from_str_radix(&hex[2..4], 16)? as f64 / 255.0,
        u8::from_str_radix(&hex[4..6], 16)? as f64 / 255.0,
    ])
}

fn validate_interval(text: &str, start_us: i64, duration_us: i64) -> Result<()> {
    if text.is_empty() {
        bail!("caption text must not be empty");
    }
    if start_us < 0 || duration_us <= 0 {
        bail!("caption start must be >= 0 and duration must be > 0");
    }
    Ok(())
}

fn parse_ass(input: &str) -> Result<Vec<crate::srt::Cue>> {
    let mut cues = Vec::new();
    let mut in_events = false;
    for line in input.replace("\r\n", "\n").lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            in_events = trimmed.eq_ignore_ascii_case("[Events]");
            continue;
        }
        if !in_events || !trimmed.to_ascii_lowercase().starts_with("dialogue:") {
            continue;
        }
        let raw = trimmed
            .split_once(':')
            .map(|(_, tail)| tail)
            .unwrap_or_default();
        let fields: Vec<&str> = raw.splitn(10, ',').collect();
        if fields.len() != 10 {
            bail!("ASS Dialogue line must contain 10 fields");
        }
        let start_us = parse_ass_time(fields[1])?;
        let end_us = parse_ass_time(fields[2])?;
        if end_us <= start_us {
            bail!("ASS cue end must be after start");
        }
        let text = fields[9].replace("\\N", "\n").replace("\\n", "\n");
        if !text.is_empty() {
            cues.push(crate::srt::Cue {
                start_us,
                end_us,
                text,
            });
        }
    }
    Ok(cues)
}

fn parse_ass_time(value: &str) -> Result<i64> {
    let parts: Vec<&str> = value.trim().split(':').collect();
    if parts.len() != 3 {
        bail!("invalid ASS timestamp: {value}");
    }
    let hours: i64 = parts[0].parse()?;
    let minutes: i64 = parts[1].parse()?;
    let (seconds, fraction) = parts[2]
        .split_once('.')
        .ok_or_else(|| anyhow::anyhow!("invalid ASS timestamp: {value}"))?;
    let seconds: i64 = seconds.parse()?;
    let centiseconds: i64 = fraction.parse()?;
    Ok(((hours * 60 + minutes) * 60 + seconds) * 1_000_000 + centiseconds * 10_000)
}

fn srt_time(value: i64) -> String {
    let millis = value.max(0) / 1_000;
    format!(
        "{:02}:{:02}:{:02},{:03}",
        millis / 3_600_000,
        millis / 60_000 % 60,
        millis / 1_000 % 60,
        millis % 1_000
    )
}

fn ass_time(value: i64) -> String {
    let centiseconds = value.max(0) / 10_000;
    format!(
        "{}:{:02}:{:02}.{:02}",
        centiseconds / 360_000,
        centiseconds / 6_000 % 60,
        centiseconds / 100 % 60,
        centiseconds % 100
    )
}

fn atomic_write(out: &Path, bytes: &[u8]) -> Result<()> {
    let parent = out.parent().unwrap_or_else(|| Path::new("."));
    std::fs::create_dir_all(parent)?;
    let name = out
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("captions");
    let temporary = parent.join(format!(".{name}.{}.tmp", Uuid::new_v4().simple()));
    std::fs::write(&temporary, bytes)?;
    std::fs::rename(&temporary, out).with_context(|| {
        format!(
            "atomically replacing {} with {}",
            out.display(),
            temporary.display()
        )
    })?;
    Ok(())
}
