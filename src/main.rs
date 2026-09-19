//! jianying — local CLI that builds verifiable, fully editable 剪映/CapCut
//! drafts from a jianying-cli-plan/v1 JSON file. No account, no upload.

use anyhow::{bail, Result};
use clap::{Parser, Subcommand};
use jianying_cli::{draft, plan, probe, render, srt, store, template};
use std::path::{Path, PathBuf};

#[derive(Parser)]
#[command(
    name = "jianying",
    version,
    about = "Local JianYing/CapCut draft engine — build verifiable native drafts from jianying-cli-plan/v1",
    long_about = "jianying — 本地剪映/CapCut 草稿引擎（Rust）\n\n\
把一份 jianying-cli-plan/v1 JSON 计划构建为剪映专业版/CapCut 可直接打开的原生草稿；\n\
构建即结构校验，可发布进草稿库。无账号、无上传、确定性输出。\n\n\
I/O 契约（智能体请严格遵守）:\n  \
· 成功：stdout 恰好一个 pretty JSON 对象，退出码 0\n  \
· 运行错误：stderr 一行 `error: <原因链>`，退出码 1\n  \
· 用法错误：clap 用法文本到 stderr，退出码 2\n  \
· 时间字段（*_us）接受微秒整数或 tim() 字符串（\"1h2m3s\"/\"0.5s\"/\"500ms\"）\n  \
· 能力名大小写敏感，须命中内置目录（用 `jianying catalog` 检索）\n\n\
示例:\n  \
jianying doctor\n  \
jianying build plan.json --out draft-v1 --srt subs.srt\n  \
jianying publish draft-v1            # 剪映需完全关闭\n  \
jianying catalog --domain transitions --search 叠化"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum TemplateOp {
    /// List tracks and the material inventory (inspect_material parity)
    Inspect {
        /// Draft directory
        draft: PathBuf,
    },
    /// duplicate_as_template parity: copy under a new name and restamp
    Duplicate {
        /// Source draft directory
        draft: PathBuf,
        /// New draft name
        new_name: String,
        /// Destination root (default: source draft's parent)
        #[arg(long)]
        root: Option<PathBuf>,
    },
    /// replace_text parity
    ReplaceText {
        /// Draft directory
        draft: PathBuf,
        /// Text track name
        #[arg(long)]
        track: String,
        /// Segment index on the track (0-based)
        #[arg(long)]
        index: usize,
        /// Replacement text
        text: String,
    },
    /// replace_material_by_name / by_seg parity: swap the source file
    ReplaceMaterial {
        /// Draft directory
        draft: PathBuf,
        /// New source media file
        source: PathBuf,
        /// Replace by material name
        #[arg(long)]
        name: Option<String>,
        /// Replace by track name + segment index
        #[arg(long)]
        track: Option<String>,
        #[arg(long)]
        index: Option<usize>,
    },
    /// import_track parity: copy a track from another draft
    ImportTrack {
        /// Target draft directory
        draft: PathBuf,
        /// Source draft directory
        source: PathBuf,
        /// Track name (or type) to import
        track: String,
        /// Insert before this track name (pyJYD insert_track; default append)
        #[arg(long)]
        before: Option<String>,
    },
}

#[derive(Subcommand)]
enum StoreOp {
    /// list_drafts parity
    List {
        #[arg(long)]
        root: Option<PathBuf>,
    },
    /// has_draft parity
    Has {
        name: String,
        #[arg(long)]
        root: Option<PathBuf>,
    },
    /// remove parity (deletes the draft folder and unregisters it)
    Remove {
        name: String,
        #[arg(long)]
        root: Option<PathBuf>,
        /// Confirm deletion
        #[arg(long)]
        yes: bool,
    },
}

#[derive(Subcommand)]
enum Command {
    /// 环境预检：草稿根、运行中的剪映/CapCut、ffprobe/ffmpeg（只读，恒 JSON）
    Doctor,
    /// 实测媒体（时长/分辨率/流/是否图片）；所有计划时长应以它为准
    Probe {
        /// Media file to measure
        media: PathBuf,
    },
    /// 计划 → 草稿目录（构建后自动跑结构校验，失败即报错不产出）
    /// 例：jianying build plan.json --out d1 --srt subs.srt --seed <草稿根>
    Build {
        /// Plan file (jianying-cli-plan/v1)
        plan: PathBuf,
        /// New output directory for the draft
        #[arg(long)]
        out: PathBuf,
        /// SRT file to append as a subtitle track
        #[arg(long)]
        srt: Option<PathBuf>,
        /// SRT cue offset (tim() strings accepted) — pyJYD import_srt time_offset
        #[arg(long)]
        srt_offset: Option<String>,
        /// SRT font size (pyJYD default 5)
        #[arg(long)]
        srt_size: Option<f64>,
        /// SRT alignment 0/1/2 (pyJYD default 1)
        #[arg(long)]
        srt_align: Option<u8>,
        /// SRT text color #RRGGBB
        #[arg(long)]
        srt_color: Option<String>,
        /// SRT stroke width 0-100
        #[arg(long)]
        srt_border: Option<f64>,
        /// SRT vertical position (pyJYD default -0.8)
        #[arg(long)]
        srt_y: Option<f64>,
        /// Seed schema markers from the newest app-written draft in this store
        #[arg(long)]
        seed: Option<PathBuf>,
        /// Build on top of an existing template draft (load_template parity)
        #[arg(long)]
        template: Option<PathBuf>,
    },
    /// 模板模式（对已有草稿）：inspect/duplicate/replace-text/replace-material/import-track
    Template {
        #[command(subcommand)]
        op: TemplateOp,
    },
    /// 能力目录检索：列 16 个域或按名搜索（转场/滤镜/字体/特效/动画/蒙版…）
    /// 例：jianying catalog --domain transitions --search 叠化
    Catalog {
        /// 限定域（省略则列出全部域与规模）
        #[arg(long)]
        domain: Option<String>,
        /// 按显示名子串搜索（大小写敏感）
        #[arg(long)]
        search: Option<String>,
        /// 包含 VIP 条目（默认过滤；计划需 allow_vip=true 才可用）
        #[arg(long)]
        include_vip: bool,
    },
    /// 草稿库管理：list/has/remove（remove 需 --yes）
    Store {
        #[command(subcommand)]
        op: StoreOp,
    },
    /// 结构 lint：引用完整/主轨连续/时长一致（只读）
    Verify {
        /// Draft directory
        draft: PathBuf,
    },
    /// 草稿摘要：轨道/段数/画布/平台（只读）
    Inspect {
        /// Draft directory
        draft: PathBuf,
    },
    /// 拷入草稿库并注册 root_meta_info.json（剪映运行中拒绝；同名拒绝）
    Publish {
        /// Draft directory (as produced by build)
        draft: PathBuf,
        /// Draft store root (default: detect, or JIANYING_CLI_DRAFT_ROOT)
        #[arg(long)]
        root: Option<PathBuf>,
        /// Publish even if 剪映/CapCut is running
        #[arg(long)]
        force: bool,
    },
    /// ffmpeg 代理渲染（预览级：平铺主轨+混音+可选烧字幕；不含转场/特效/蒙版）
    Render {
        /// Draft directory
        draft: PathBuf,
        /// Output MP4 path
        #[arg(long, default_value = "preview.mp4")]
        out: PathBuf,
        /// Output scale relative to canvas (default half)
        #[arg(long, default_value_t = 0.5)]
        scale: f64,
        /// Burn text segments as captions
        #[arg(long)]
        burn_captions: bool,
        /// x264 CRF (higher = smaller/worse)
        #[arg(long, default_value_t = 28)]
        crf: i32,
    },
}

fn run(cmd: Command) -> Result<()> {
    match cmd {
        Command::Doctor => print_json(store::doctor()?),
        Command::Probe { media } => print_json(serde_json::to_value(probe::probe(&media)?)?),
        Command::Build {
            plan,
            out,
            srt,
            srt_offset,
            srt_size,
            srt_align,
            srt_color,
            srt_border,
            srt_y,
            seed,
            template,
        } => {
            let mut plan = plan::Plan::load(&plan)?;
            if let Some(srt_path) = srt {
                let cues = srt::parse(&std::fs::read_to_string(&srt_path)?)?;
                let opts = srt::SrtOptions {
                    offset_us: srt_offset
                        .as_deref()
                        .map(jianying_cli::tim::parse)
                        .transpose()?
                        .unwrap_or(0),
                    size: srt_size,
                    align: srt_align,
                    color: srt_color,
                    border_width: srt_border,
                    y: srt_y,
                    ..Default::default()
                };
                plan.tracks.push(srt::cues_to_text_track(cues, &opts));
                plan.validate()?;
            }
            let plan_dir = plan.parent.clone().unwrap_or_else(|| PathBuf::from("."));
            let report = draft::build(&plan, &plan_dir, &out, seed.as_deref(), &|p| {
                probe::probe(p)
            })?;
            if let Some(tpl) = template {
                // load_template parity: overlay the plan tracks onto the template timeline
                let mut tl: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(
                    Path::new(&report.out_dir).join("draft_content.json"),
                )?)?;
                template::build_on_template(&tpl, &mut tl, &report.name)?;
                let s = serde_json::to_string_pretty(&tl)?;
                std::fs::write(Path::new(&report.out_dir).join("draft_content.json"), &s)?;
                std::fs::write(Path::new(&report.out_dir).join("draft_info.json"), &s)?;
            }
            let verdict = draft::verify(Path::new(&report.out_dir))?;
            if verdict["ok"] != serde_json::json!(true) {
                bail!(
                    "built draft failed self-verification: {}",
                    verdict["issues"]
                );
            }
            print_json(serde_json::to_value(report)?)
        }
        Command::Verify { draft } => print_json(draft::verify(&draft)?),
        Command::Inspect { draft } => print_json(draft::inspect(&draft)?),
        Command::Publish { draft, root, force } => {
            let root = store::resolve_root(root.as_deref())?;
            print_json(store::publish(&draft, &root, force)?)
        }
        Command::Render {
            draft,
            out,
            scale,
            burn_captions,
            crf,
        } => print_json(render::render(&draft, &out, scale, burn_captions, crf)?),
        Command::Template { op } => match op {
            TemplateOp::Inspect { draft } => print_json(template::inspect_materials(&draft)?),
            TemplateOp::Duplicate {
                draft,
                new_name,
                root,
            } => print_json(template::duplicate(&draft, &new_name, root.as_deref())?),
            TemplateOp::ReplaceText {
                draft,
                track,
                index,
                text,
            } => print_json(template::replace_text(&draft, &track, index, &text)?),
            TemplateOp::ReplaceMaterial {
                draft,
                source,
                name,
                track,
                index,
            } => print_json(template::replace_material(
                &draft,
                name.as_deref(),
                track.as_deref(),
                index,
                &source,
            )?),
            TemplateOp::ImportTrack {
                draft,
                source,
                track,
                before,
            } => print_json(template::import_track_at(
                &draft,
                &source,
                &track,
                before.as_deref(),
            )?),
        },
        Command::Catalog {
            domain,
            search,
            include_vip,
        } => catalog_query(domain.as_deref(), search.as_deref(), include_vip)?,
        Command::Store { op } => match op {
            StoreOp::List { root } => {
                let root = store::resolve_root(root.as_deref())?;
                print_json(store::list(&root)?)
            }
            StoreOp::Has { name, root } => {
                let root = store::resolve_root(root.as_deref())?;
                print_json(store::has(&root, &name)?)
            }
            StoreOp::Remove { name, root, yes } => {
                if !yes {
                    bail!("refusing to remove without --yes");
                }
                let root = store::resolve_root(root.as_deref())?;
                print_json(store::remove(&root, &name)?)
            }
        },
    }
    Ok(())
}

fn catalog_query(domain: Option<&str>, search: Option<&str>, include_vip: bool) -> Result<()> {
    use jianying_cli::catalogs;
    type CatalogFn = fn() -> &'static serde_json::Value;
    const DOMAINS: &[(&str, CatalogFn)] = &[
        ("transitions", catalogs::transitions),
        ("filters", catalogs::filters),
        ("fonts", catalogs::fonts),
        ("video_scene_effects", catalogs::video_scene_effects),
        ("video_character_effects", catalogs::video_character_effects),
        ("audio_scene_effects", catalogs::audio_scene_effects),
        ("tone_effects", catalogs::tone_effects),
        ("speech_to_songs", catalogs::speech_to_songs),
        ("video_animations_in", catalogs::video_animations_in),
        ("video_animations_out", catalogs::video_animations_out),
        ("video_animations_group", catalogs::video_animations_group),
        ("text_animations_in", catalogs::text_animations_in),
        ("text_animations_out", catalogs::text_animations_out),
        ("text_animations_loop", catalogs::text_animations_loop),
        ("masks", catalogs::masks),
        ("mix_modes", catalogs::mix_modes),
    ];
    let selected: Vec<&str> = match domain {
        Some(d) => {
            if !DOMAINS.iter().any(|(name, _)| *name == d) {
                bail!(
                    "unknown domain {d:?}; available: {}",
                    DOMAINS
                        .iter()
                        .map(|(n, _)| *n)
                        .collect::<Vec<_>>()
                        .join(", ")
                );
            }
            vec![d]
        }
        None => DOMAINS.iter().map(|(n, _)| *n).collect(),
    };
    if domain.is_none() && search.is_none() {
        let domains: Vec<serde_json::Value> = DOMAINS
            .iter()
            .map(|(name, load)| {
                let all = load().as_array().map(|a| a.len()).unwrap_or(0);
                let non_vip = load()
                    .as_array()
                    .map(|a| {
                        a.iter()
                            .filter(|e| !e["vip"].as_bool().unwrap_or(false))
                            .count()
                    })
                    .unwrap_or(0);
                serde_json::json!({"domain": name, "entries": all, "non_vip": non_vip})
            })
            .collect();
        print_json(serde_json::json!({"domains": domains,
            "note": "use --domain <name> [--search <substring>] to list entries"}));
        return Ok(());
    }
    let mut out = Vec::new();
    for d in selected {
        let (_, load) = DOMAINS.iter().find(|(n, _)| *n == d).unwrap();
        for e in load().as_array().unwrap_or(&Vec::new()) {
            if !include_vip && e["vip"].as_bool().unwrap_or(false) {
                continue;
            }
            if let Some(s) = search {
                let name = e["name"].as_str().unwrap_or_default();
                if !name.contains(s) {
                    continue;
                }
            }
            out.push(e.clone());
        }
    }
    print_json(serde_json::json!({"results": out, "count": out.len()}));
    Ok(())
}

fn print_json(v: serde_json::Value) {
    println!("{}", serde_json::to_string_pretty(&v).unwrap_or_default());
}

fn main() {
    if let Err(e) = run(Cli::parse().command) {
        eprintln!("error: {e:#}");
        std::process::exit(1);
    }
}
