//! jianying — local CLI that builds verifiable, fully editable 剪映/CapCut
//! drafts from a jianying-cli-plan/v1 JSON file. No account, no upload.

use anyhow::{bail, Result};
use clap::{Parser, Subcommand};
use jianying_cli::{draft, plan, probe, render, srt, store, template};
use std::path::{Path, PathBuf};

#[derive(Parser)]
#[command(name = "jianying", version, about = None, long_about = None)]
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
    /// Environment preflight: draft roots, running editors, ffprobe
    Doctor,
    /// Measure a media file (duration/size/streams) via ffprobe
    Probe {
        /// Media file to measure
        media: PathBuf,
    },
    /// Build a draft directory from a plan JSON file
    Build {
        /// Plan file (jianying-cli-plan/v1)
        plan: PathBuf,
        /// New output directory for the draft
        #[arg(long)]
        out: PathBuf,
        /// SRT file to append as a subtitle track
        #[arg(long)]
        srt: Option<PathBuf>,
        /// Seed schema markers from the newest app-written draft in this store
        #[arg(long)]
        seed: Option<PathBuf>,
        /// Build on top of an existing template draft (load_template parity)
        #[arg(long)]
        template: Option<PathBuf>,
    },
    /// Template-mode operations on an existing draft
    Template {
        #[command(subcommand)]
        op: TemplateOp,
    },
    /// Draft store administration (DraftFolder parity)
    Store {
        #[command(subcommand)]
        op: StoreOp,
    },
    /// Structural lint over a built or published draft
    Verify {
        /// Draft directory
        draft: PathBuf,
    },
    /// Summarize a draft (tracks/segments/canvas/platform)
    Inspect {
        /// Draft directory
        draft: PathBuf,
    },
    /// Copy a built draft into the draft store and register it
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
    /// FFmpeg proxy render of a built draft (preview quality)
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
        Command::Build { plan, out, srt, seed, template } => {
            let mut plan = plan::Plan::load(&plan)?;
            if let Some(srt_path) = srt {
                let cues = srt::parse(&std::fs::read_to_string(&srt_path)?)?;
                plan.tracks.push(srt::cues_to_text_track(cues));
                plan.validate()?;
            }
            let plan_dir = plan.parent.clone().unwrap_or_else(|| PathBuf::from("."));
            let report = draft::build(&plan, &plan_dir, &out, seed.as_deref(), &|p| {
                probe::probe(p)
            })?;
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
        Command::Render { draft, out, scale, burn_captions, crf } => {
            print_json(render::render(&draft, &out, scale, burn_captions, crf)?)
        }
        Command::Template { op } => match op {
            TemplateOp::Inspect { draft } => print_json(template::inspect_materials(&draft)?),
            TemplateOp::Duplicate { draft, new_name, root } => {
                print_json(template::duplicate(&draft, &new_name, root.as_deref())?)
            }
            TemplateOp::ReplaceText { draft, track, index, text } => {
                print_json(template::replace_text(&draft, &track, index, &text)?)
            }
            TemplateOp::ReplaceMaterial { draft, source, name, track, index } => {
                print_json(template::replace_material(&draft, name.as_deref(), track.as_deref(), index, &source)?)
            }
            TemplateOp::ImportTrack { draft, source, track } => {
                print_json(template::import_track(&draft, &source, &track)?)
            }
        },
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

fn print_json(v: serde_json::Value) {
    println!("{}", serde_json::to_string_pretty(&v).unwrap_or_default());
}

fn main() {
    if let Err(e) = run(Cli::parse().command) {
        eprintln!("error: {e:#}");
        std::process::exit(1);
    }
}
