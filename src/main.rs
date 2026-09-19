//! jianying — local CLI that builds verifiable, fully editable 剪映/CapCut
//! drafts from a jianying-cli-plan/v1 JSON file. No account, no upload.

use anyhow::{bail, Result};
use clap::{Parser, Subcommand};
use jianying_cli::{draft, plan, probe, render, srt, store};
use std::path::{Path, PathBuf};

#[derive(Parser)]
#[command(name = "jianying", version, about = None, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Command,
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
        Command::Build { plan, out, srt, seed } => {
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
