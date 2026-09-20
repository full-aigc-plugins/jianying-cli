//! jianying — local CLI that builds verifiable, fully editable 剪映/CapCut
//! drafts from a jianying-cli-plan/v1 JSON file. No account, no upload.

use anyhow::{bail, Context, Result};
use clap::{CommandFactory, Parser, Subcommand, ValueEnum};
use clap_complete::Shell;
use jianying_cli::{
    capabilities, caption_ops, draft, interchange, job_runner, media_analysis, media_ops, plan,
    probe, project_ops, render, srt, store, template, tim, timeline_ops,
};
use jianying_media::{
    AliyunBailianTtsAdapter, AliyunTtsFamily, AsrOutputFormat, AsrProvider, AsrRequest,
    AsrTimestampGranularity, BaiduTtsAdapter, CloudTtsHttpExecutor, CloudTtsProviderAdapter,
    CredentialRef, CredentialSource, EdgeTtsProvider, LocalHttpTtsProvider,
    LocalProcessTtsProvider, MacOsSayTtsProvider, MiniMaxTtsAdapter, TencentCloudTtsAdapter,
    TtsAudioFormat, TtsInputKind, TtsProvider, TtsRequest, VolcengineTtsAdapter,
    WhisperCppAsrProvider, WindowsSystemTtsProvider, XiaomiMimoTtsAdapter, ZhipuGlmTtsAdapter,
};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::Duration;

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
    /// Emit one stable success or error envelope on stdout
    #[arg(long, global = true)]
    json: bool,
    /// Select an isolated configuration profile
    #[arg(long, global = true, default_value = "default")]
    profile: String,
    /// Allow configuration reads but reject every configuration mutation
    #[arg(long, global = true)]
    host_read_only: bool,
    /// Disable ANSI colors in human-readable diagnostics
    #[arg(long, global = true)]
    no_color: bool,
    /// Diagnostic verbosity; JSON output remains stable at every level
    #[arg(long, global = true, value_enum, default_value_t = LogLevel::Info)]
    log_level: LogLevel,
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum LogLevel {
    Error,
    Warn,
    Info,
    Debug,
    Trace,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum TtsProviderArg {
    LocalCommand,
    SystemMacos,
    SystemWindows,
    LocalHttp,
    LocalProcess,
    EdgeTts,
    XiaomiMimo,
    Volcengine,
    AliyunBailian,
    Baidu,
    TencentCloud,
    Minimax,
    ZhipuGlm,
}

impl TtsProviderArg {
    fn is_cloud(self) -> bool {
        matches!(
            self,
            Self::XiaomiMimo
                | Self::Volcengine
                | Self::AliyunBailian
                | Self::Baidu
                | Self::TencentCloud
                | Self::Minimax
                | Self::ZhipuGlm
        )
    }
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum TtsFormatArg {
    Wav,
    Mp3,
    Pcm,
    Ogg,
    Aac,
    Flac,
    Aiff,
}

impl From<TtsFormatArg> for TtsAudioFormat {
    fn from(value: TtsFormatArg) -> Self {
        match value {
            TtsFormatArg::Wav => Self::Wav,
            TtsFormatArg::Mp3 => Self::Mp3,
            TtsFormatArg::Pcm => Self::Pcm,
            TtsFormatArg::Ogg => Self::Ogg,
            TtsFormatArg::Aac => Self::Aac,
            TtsFormatArg::Flac => Self::Flac,
            TtsFormatArg::Aiff => Self::Aiff,
        }
    }
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum AsrFormatArg {
    Json,
    Text,
    Srt,
    VerboseJson,
    Vtt,
}

impl From<AsrFormatArg> for AsrOutputFormat {
    fn from(value: AsrFormatArg) -> Self {
        match value {
            AsrFormatArg::Json => Self::Json,
            AsrFormatArg::Text => Self::Text,
            AsrFormatArg::Srt => Self::Srt,
            AsrFormatArg::VerboseJson => Self::VerboseJson,
            AsrFormatArg::Vtt => Self::Vtt,
        }
    }
}

#[derive(Clone, Copy)]
struct CloudTtsPlanInput<'a> {
    provider: TtsProviderArg,
    request: &'a TtsRequest,
    draft: &'a Path,
    task_id: &'a str,
    max_cost_microunits: u64,
    live_canary: bool,
    aliyun_family: Option<&'a str>,
    workspace_id: Option<&'a str>,
    app_id: Option<&'a str>,
    cluster: Option<&'a str>,
    resource_id: Option<&'a str>,
    uid: Option<&'a str>,
    cuid: Option<&'a str>,
}

fn plan_cloud_tts(input: CloudTtsPlanInput<'_>) -> Result<serde_json::Value> {
    let adapter = build_cloud_tts_adapter(input)?;
    adapter.validate_request(input.request)?;
    let provider_id = adapter.provider_id();
    let endpoint = adapter.endpoint();
    let submission = cloud_tts_submission(input, provider_id)?;
    let credential = input
        .request
        .credential()
        .ok_or_else(|| anyhow::anyhow!("cloud TTS request is missing its credential reference"))?;
    Ok(serde_json::json!({
        "state": "approval_required",
        "provider": provider_id,
        "endpoint": endpoint,
        "execution_mode": submission.execution_mode(),
        "credential_source": credential.source(),
        "credential_ref": credential.name(),
        "max_cost_microunits": input.max_cost_microunits,
        "idempotency_key": submission.idempotency_key(),
        "approval_binding": submission.approval_binding()?,
        "next": "grant the exact binding, then rerun with the resulting approval id; no network request was made"
    }))
}

fn build_cloud_tts_adapter(input: CloudTtsPlanInput<'_>) -> Result<CloudTtsProviderAdapter> {
    let adapter = match input.provider {
        TtsProviderArg::XiaomiMimo => {
            reject_cloud_config(&input, &["generic"])?;
            CloudTtsProviderAdapter::XiaomiMimo(XiaomiMimoTtsAdapter::new()?)
        }
        TtsProviderArg::Volcengine => {
            reject_cloud_config(&input, &["volcengine"])?;
            if input.app_id.is_some() || input.cluster.is_some() {
                bail!(
                    "Volcengine V1 --app-id/--cluster configuration is unsupported; use V3 --resource-id, --uid and an API key credential"
                );
            }
            CloudTtsProviderAdapter::Volcengine(VolcengineTtsAdapter::new(
                required_cloud_option("--resource-id", input.resource_id)?,
                required_cloud_option("--uid", input.uid)?,
            )?)
        }
        TtsProviderArg::AliyunBailian => {
            reject_cloud_config(&input, &["aliyun"])?;
            let family = match input.aliyun_family.unwrap_or("qwen-tts") {
                "qwen-audio" => AliyunTtsFamily::QwenAudio,
                "cosyvoice" => AliyunTtsFamily::CosyVoice,
                "qwen-tts" => AliyunTtsFamily::QwenTts,
                other => bail!(
                    "unsupported --aliyun-family {other}; use qwen-audio, cosyvoice, or qwen-tts"
                ),
            };
            CloudTtsProviderAdapter::AliyunBailian(AliyunBailianTtsAdapter::new(
                family,
                input.workspace_id,
            )?)
        }
        TtsProviderArg::Baidu => {
            reject_cloud_config(&input, &["baidu"])?;
            CloudTtsProviderAdapter::Baidu(BaiduTtsAdapter::new(required_cloud_option(
                "--cuid", input.cuid,
            )?)?)
        }
        TtsProviderArg::TencentCloud => {
            reject_cloud_config(&input, &["generic"])?;
            CloudTtsProviderAdapter::TencentCloud(TencentCloudTtsAdapter::new()?)
        }
        TtsProviderArg::Minimax => {
            reject_cloud_config(&input, &["generic"])?;
            CloudTtsProviderAdapter::MiniMax(MiniMaxTtsAdapter::new()?)
        }
        TtsProviderArg::ZhipuGlm => {
            reject_cloud_config(&input, &["generic"])?;
            CloudTtsProviderAdapter::ZhipuGlm(ZhipuGlmTtsAdapter::new()?)
        }
        _ => unreachable!("provider_id rejected local providers"),
    };
    adapter.validate_request(input.request)?;
    Ok(adapter)
}

fn cloud_tts_submission(
    input: CloudTtsPlanInput<'_>,
    provider_id: &str,
) -> Result<jianying_jobs::TtsSubmission> {
    let cwd = std::env::current_dir()?;
    let target = if input.draft.is_absolute() {
        input.draft.to_path_buf()
    } else {
        cwd.join(input.draft)
    };
    let execution_mode = if input.live_canary {
        jianying_jobs::TtsExecutionMode::LiveCanary
    } else {
        jianying_jobs::TtsExecutionMode::Production
    };
    Ok(jianying_jobs::TtsSubmission::new(
        provider_id,
        input.request,
        cwd,
        target,
        input.task_id,
        input.max_cost_microunits,
        execution_mode,
    )?)
}

fn required_cloud_option<'a>(name: &str, value: Option<&'a str>) -> Result<&'a str> {
    value
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| anyhow::anyhow!("selected cloud TTS provider requires {name}"))
}

fn reject_cloud_config(input: &CloudTtsPlanInput<'_>, allowed: &[&str]) -> Result<()> {
    let aliyun_allowed = allowed.contains(&"aliyun");
    let volcengine_allowed = allowed.contains(&"volcengine");
    let baidu_allowed = allowed.contains(&"baidu");
    if !aliyun_allowed && (input.aliyun_family.is_some() || input.workspace_id.is_some()) {
        bail!("--aliyun-family and --workspace-id require --provider aliyun-bailian");
    }
    if !volcengine_allowed
        && (input.app_id.is_some()
            || input.cluster.is_some()
            || input.resource_id.is_some()
            || input.uid.is_some())
    {
        bail!("--app-id, --cluster, --resource-id and --uid require --provider volcengine");
    }
    if !baidu_allowed && input.cuid.is_some() {
        bail!("--cuid requires --provider baidu");
    }
    Ok(())
}

impl LogLevel {
    fn as_str(self) -> &'static str {
        match self {
            Self::Error => "error",
            Self::Warn => "warn",
            Self::Info => "info",
            Self::Debug => "debug",
            Self::Trace => "trace",
        }
    }
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum RuntimePlatformArg {
    Macos,
    Windows,
    Linux,
}

impl From<RuntimePlatformArg> for jianying_runtime::RuntimePlatform {
    fn from(value: RuntimePlatformArg) -> Self {
        match value {
            RuntimePlatformArg::Macos => Self::MacOs,
            RuntimePlatformArg::Windows => Self::Windows,
            RuntimePlatformArg::Linux => Self::Linux,
        }
    }
}

#[derive(Subcommand)]
enum RuntimeOp {
    /// Discover installed editors and known draft roots without enabling native routing
    Discover {
        /// Override the platform used for deterministic discovery and diagnostics
        #[arg(long, value_enum)]
        platform: Option<RuntimePlatformArg>,
        /// Additional application directory to scan; defaults to platform-known roots
        #[arg(long = "search-root")]
        search_roots: Vec<PathBuf>,
        /// Override the home directory used only for discovery
        #[arg(long)]
        home: Option<PathBuf>,
        /// Override Windows LOCALAPPDATA used only for discovery
        #[arg(long)]
        local_app_data: Option<PathBuf>,
    },
    /// Report runtime capability state, or inspect one owned process record
    Status {
        #[arg(long)]
        runtime_profile: Option<PathBuf>,
        #[arg(long)]
        state_root: Option<PathBuf>,
    },
    /// Validate a runtime profile against observed executable, paths and capabilities
    Probe {
        #[arg(long)]
        runtime_profile: PathBuf,
        #[arg(long)]
        executable: PathBuf,
        #[arg(long)]
        draft_root: PathBuf,
        #[arg(long, value_enum)]
        platform: Option<RuntimePlatformArg>,
        #[arg(long)]
        product: Option<String>,
        #[arg(long)]
        version: Option<String>,
        #[arg(long = "running-process")]
        running_processes: Vec<String>,
        #[arg(long = "material")]
        materials: Vec<PathBuf>,
        #[arg(long = "capability")]
        capabilities: Vec<String>,
    },
    /// Start an exact, profile-bound executable and persist an ownership record
    Start {
        #[arg(long)]
        runtime_profile: PathBuf,
        #[arg(long)]
        executable: PathBuf,
        #[arg(long = "arg", allow_hyphen_values = true)]
        arguments: Vec<String>,
        #[arg(long)]
        state_root: Option<PathBuf>,
    },
    /// Stop only the exact process previously started by this CLI and profile
    Stop {
        #[arg(long)]
        runtime_profile: PathBuf,
        #[arg(long)]
        state_root: Option<PathBuf>,
    },
    /// Export the runtime-related portion of the capability manifest
    ExportCapabilities,
}

#[derive(Subcommand)]
enum CaptionsOp {
    /// List every caption in timeline order
    List { draft: PathBuf },
    /// Read one caption by segment id
    Get { draft: PathBuf, id: String },
    /// Add one caption to a named text track
    Add {
        draft: PathBuf,
        text: String,
        start: String,
        duration: String,
        #[arg(long, default_value = "字幕")]
        track: String,
    },
    /// Replace one caption's text
    Set {
        draft: PathBuf,
        id: String,
        text: String,
    },
    /// Import SRT cues into an existing draft
    ImportSrt {
        draft: PathBuf,
        file: PathBuf,
        #[arg(long, default_value = "字幕")]
        track: String,
        #[arg(long, default_value = "0us")]
        offset: String,
        #[arg(long)]
        size: Option<f64>,
        #[arg(long)]
        color: Option<String>,
        #[arg(long)]
        bold: Option<bool>,
    },
    /// Import ASS Dialogue events into an existing draft
    ImportAss {
        draft: PathBuf,
        file: PathBuf,
        #[arg(long, default_value = "字幕")]
        track: String,
        #[arg(long, default_value = "0us")]
        offset: String,
    },
    /// Export all captions as SRT
    ExportSrt {
        draft: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
    /// Export all captions as ASS
    ExportAss {
        draft: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
    /// Patch the base style of one caption
    Style {
        draft: PathBuf,
        id: String,
        #[arg(long)]
        size: Option<f64>,
        #[arg(long)]
        color: Option<String>,
        #[arg(long)]
        bold: Option<bool>,
        #[arg(long)]
        italic: Option<bool>,
        #[arg(long)]
        underline: Option<bool>,
        #[arg(long)]
        alignment: Option<u8>,
    },
    /// Replace per-range text styles using UTF-16 code-unit offsets
    StyleRanges {
        draft: PathBuf,
        id: String,
        /// Inline JSON array or @path.json
        #[arg(long)]
        styles: String,
    },
    /// Apply or replace a speech-bubble shape on one text segment
    Bubble {
        draft: PathBuf,
        id: String,
        #[arg(long)]
        bubble: Option<String>,
        #[arg(long)]
        effect_id: Option<String>,
        #[arg(long)]
        resource_id: Option<String>,
    },
    /// Apply one text intro and/or outro animation
    Animation {
        draft: PathBuf,
        id: String,
        #[arg(long)]
        intro: Option<String>,
        #[arg(long)]
        outro: Option<String>,
        #[arg(long)]
        intro_duration: Option<String>,
        #[arg(long)]
        outro_duration: Option<String>,
        #[arg(long)]
        jianying: bool,
    },
    /// Apply an offline by_id/by_text JSON translation mapping
    Translate {
        draft: PathBuf,
        #[arg(required_unless_present = "provider", conflicts_with = "provider")]
        translations: Option<PathBuf>,
        /// Absolute executable implementing jianying-caption-translation/v1
        #[arg(long)]
        provider: Option<PathBuf>,
        /// Target language passed to the structured provider
        #[arg(long, requires = "provider")]
        to: Option<String>,
    },
    /// Report caption capability state
    Status,
}

#[derive(Subcommand)]
enum TimelineOp {
    /// Show the complete timeline lane layout with computed columns
    Show {
        draft: PathBuf,
        #[arg(long, default_value_t = 60)]
        cols: u64,
    },
    /// List track summaries
    Tracks(DraftPathArgs),
    /// List segments, optionally filtering by track type
    Segments {
        draft: PathBuf,
        #[arg(long)]
        track: Option<String>,
    },
    /// Read one segment with its track and material
    Get { draft: PathBuf, id: String },
    /// Move one segment by a signed time offset
    Move {
        draft: PathBuf,
        id: String,
        offset: String,
    },
    /// Move all segments, optionally on one track type
    MoveAll {
        draft: PathBuf,
        offset: String,
        #[arg(long)]
        track: Option<String>,
    },
    /// Set one segment's playback speed
    Speed {
        draft: PathBuf,
        id: String,
        multiplier: f64,
    },
    /// Set one segment's volume
    Volume {
        draft: PathBuf,
        id: String,
        level: f64,
    },
    /// Trim one segment's source window
    Trim {
        draft: PathBuf,
        id: String,
        start: String,
        duration: String,
    },
    /// Set one visual segment's opacity
    Opacity {
        draft: PathBuf,
        id: String,
        alpha: f64,
    },
    /// Add an empty track
    AddTrack {
        draft: PathBuf,
        kind: String,
        name: String,
    },
    /// Add a lossless segment JSON object to an existing track
    AddSegment {
        draft: PathBuf,
        track_id: String,
        segment_json: PathBuf,
    },
    /// Atomically set common segment properties
    Set {
        draft: PathBuf,
        id: String,
        #[arg(long)]
        start: Option<String>,
        #[arg(long)]
        duration: Option<String>,
        #[arg(long)]
        speed: Option<f64>,
        #[arg(long)]
        volume: Option<f64>,
        #[arg(long)]
        opacity: Option<f64>,
    },
    /// Split a segment at an absolute timeline position
    Split {
        draft: PathBuf,
        id: String,
        at: String,
    },
    /// Duplicate a segment with independent material state
    Duplicate {
        draft: PathBuf,
        id: String,
        #[arg(long, conflicts_with = "new_track")]
        track: Option<String>,
        #[arg(long)]
        new_track: bool,
        #[arg(long)]
        dry_run: bool,
    },
    /// Remove a segment and optionally retain an emptied track
    Remove {
        draft: PathBuf,
        id: String,
        #[arg(long)]
        keep_track: bool,
        #[arg(long)]
        keep_materials: bool,
        #[arg(long)]
        dry_run: bool,
    },
    /// Set a visual segment's compositing mode
    Composite {
        draft: PathBuf,
        id: String,
        mode: String,
    },
    /// Enable or disable smart portrait matting on a video material
    Matting {
        draft: PathBuf,
        id: String,
        #[arg(long)]
        off: bool,
    },
    /// Add or remove a chroma-key material on a video segment
    Chroma {
        draft: PathBuf,
        id: String,
        #[arg(long, required_unless_present = "off")]
        color: Option<String>,
        #[arg(long)]
        intensity: Option<f64>,
        #[arg(long)]
        off: bool,
    },
    /// Attach or remove a geometric mask material
    Mask {
        draft: PathBuf,
        id: String,
        #[arg(required_unless_present = "off")]
        slug: Option<String>,
        #[arg(long)]
        off: bool,
        #[arg(long)]
        jianying: bool,
        #[arg(long, allow_hyphen_values = true)]
        center_x: Option<f64>,
        #[arg(long, allow_hyphen_values = true)]
        center_y: Option<f64>,
        #[arg(long)]
        size: Option<f64>,
        #[arg(long, allow_hyphen_values = true)]
        rotation: Option<f64>,
        #[arg(long)]
        feather: Option<f64>,
        #[arg(long)]
        invert: bool,
        #[arg(long)]
        rect_width: Option<f64>,
        #[arg(long)]
        round_corner: Option<f64>,
        #[arg(long)]
        mask_field: Option<String>,
    },
    /// Set one of four background blur levels, or disable canvas fill
    BgBlur {
        draft: PathBuf,
        id: String,
        #[arg(required_unless_present = "off")]
        level: Option<u8>,
        #[arg(long)]
        off: bool,
    },
    /// Set fade-in and/or fade-out durations on an audio segment
    AudioFade {
        draft: PathBuf,
        id: String,
        #[arg(long = "in")]
        fade_in: Option<f64>,
        #[arg(long)]
        fade_out: Option<f64>,
    },
    /// Add a colour filter over an explicit or whole-timeline range
    AddFilter {
        draft: PathBuf,
        slug: String,
        start: Option<String>,
        duration: Option<String>,
        #[arg(long)]
        full: bool,
        #[arg(long)]
        intensity: Option<f64>,
        #[arg(long)]
        track_name: Option<String>,
        #[arg(long)]
        jianying: bool,
        #[arg(long)]
        resource_id: Option<String>,
        #[arg(long)]
        effect_id: Option<String>,
    },
    /// Add a scene or character effect over an explicit or whole-timeline range
    AddEffect {
        draft: PathBuf,
        slug: String,
        start: Option<String>,
        duration: Option<String>,
        #[arg(long)]
        full: bool,
        #[arg(long)]
        params: Option<String>,
        #[arg(long)]
        intensity: Option<f64>,
        #[arg(long)]
        track_name: Option<String>,
        #[arg(long)]
        jianying: bool,
        #[arg(long)]
        resource_id: Option<String>,
        #[arg(long)]
        effect_id: Option<String>,
        #[arg(long = "bind")]
        bind_segment_id: Option<String>,
    },
    /// Read or set the crop rectangle on a video/photo material
    Crop {
        draft: PathBuf,
        id: String,
        #[arg(long)]
        ratio: Option<String>,
        #[arg(long, allow_hyphen_values = true)]
        rect: Option<String>,
        #[arg(long)]
        reset: bool,
        #[arg(long)]
        dry_run: bool,
    },
    /// Add one keyframe or a JSONL batch from stdin
    Keyframe {
        draft: PathBuf,
        id: String,
        property: Option<String>,
        time: Option<String>,
        value: Option<String>,
        #[arg(long)]
        batch: bool,
        #[arg(long)]
        easing: Option<String>,
    },
    /// Attach one catalogue transition to a segment
    Transition {
        draft: PathBuf,
        id: String,
        slug: String,
        #[arg(long)]
        duration: Option<String>,
        #[arg(long)]
        jianying: bool,
    },
    /// Apply video/photo intro, outro and combo animations
    ImageAnimation {
        draft: PathBuf,
        id: String,
        #[arg(long)]
        intro: Option<String>,
        #[arg(long)]
        outro: Option<String>,
        #[arg(long)]
        combo: Option<String>,
        #[arg(long)]
        intro_duration: Option<String>,
        #[arg(long)]
        outro_duration: Option<String>,
        #[arg(long)]
        combo_duration: Option<String>,
        #[arg(long)]
        jianying: bool,
    },
    /// Quantize a time range outward to a rational frame grid and report drift
    QuantizationReport {
        start: String,
        duration: String,
        #[arg(long, default_value_t = 30)]
        fps_numerator: u32,
        #[arg(long, default_value_t = 1)]
        fps_denominator: u32,
        #[arg(long, default_value = "34ms")]
        maximum_drift: String,
    },
    /// Report timeline capability state
    Status,
}

#[derive(clap::Args)]
struct RenderProxyArgs {
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
}

#[derive(clap::Args)]
struct RenderNativeArgs {
    draft: PathBuf,
    #[arg(long)]
    out: PathBuf,
    #[arg(long)]
    runtime_profile: Option<PathBuf>,
    #[arg(long)]
    executable: Option<PathBuf>,
    #[arg(long)]
    approval_id: Option<String>,
    #[arg(long)]
    task_id: Option<String>,
    #[arg(long)]
    state_root: Option<PathBuf>,
    #[arg(long)]
    approval_root: Option<PathBuf>,
    #[arg(long)]
    overwrite: bool,
    /// Print the exact approval binding without consuming an approval
    #[arg(long)]
    plan: bool,
}

#[derive(clap::Args)]
struct RenderNativeTaskArgs {
    #[command(subcommand)]
    operation: RenderNativeTaskOp,
}

#[derive(Subcommand)]
enum RenderNativeTaskOp {
    /// Show the persisted native-export task
    Show {
        task_id: String,
        #[arg(long)]
        state_root: Option<PathBuf>,
    },
    /// Mark that the approved native adapter actually started
    Start {
        task_id: String,
        #[arg(long)]
        state_root: Option<PathBuf>,
    },
    /// Persist a monotonic native-adapter progress checkpoint from 0 through 99
    Progress {
        task_id: String,
        percent: u8,
        #[arg(long)]
        state_root: Option<PathBuf>,
    },
    /// Verify the native output and record its byte length and SHA-256
    Verify {
        task_id: String,
        #[arg(long)]
        state_root: Option<PathBuf>,
    },
    /// Mark a running or verifying native export as interrupted
    Interrupt {
        task_id: String,
        #[arg(long)]
        reason: String,
        #[arg(long)]
        state_root: Option<PathBuf>,
    },
    /// Mark a running or verifying native export as failed
    Fail {
        task_id: String,
        #[arg(long)]
        reason: String,
        #[arg(long)]
        state_root: Option<PathBuf>,
    },
    /// Recompute and verify a succeeded native artifact's identity
    Result {
        task_id: String,
        #[arg(long)]
        state_root: Option<PathBuf>,
    },
}

#[derive(Subcommand)]
enum RenderOp {
    /// Render an ffmpeg proxy preview
    Proxy(RenderProxyArgs),
    /// Request JianYing/CapCut native export; fails closed without a supported live profile
    Native(RenderNativeArgs),
    /// Drive and audit an already-approved native-export task
    NativeTask(RenderNativeTaskArgs),
    /// Render multiple proxy previews from a manifest
    Batch {
        manifest: PathBuf,
        #[arg(long)]
        out_dir: PathBuf,
        #[arg(long, default_value_t = 0.5)]
        scale: f64,
        #[arg(long)]
        burn_captions: bool,
        #[arg(long, default_value_t = 28)]
        crf: i32,
        #[arg(long)]
        continue_on_error: bool,
        #[arg(long)]
        overwrite: bool,
    },
    /// Report render capability state
    Status,
}

#[derive(Subcommand)]
enum JobOp {
    /// Execute one validated jianying-job/v2 document
    Run {
        /// Job JSON file
        job: PathBuf,
        /// Output draft directory; required by create
        #[arg(long)]
        out: Option<PathBuf>,
        /// Persistent task journal root
        #[arg(long)]
        state_root: Option<PathBuf>,
    },
    /// Execute a validated jianying-job/v2 batch document
    Batch {
        job: PathBuf,
        #[arg(long)]
        out: Option<PathBuf>,
        #[arg(long)]
        state_root: Option<PathBuf>,
    },
    /// List persistent tasks
    List {
        #[arg(long)]
        state_root: Option<PathBuf>,
    },
    /// Show one persistent task
    Show {
        task_id: String,
        #[arg(long)]
        state_root: Option<PathBuf>,
    },
    /// Cancel a queued or running task
    Cancel {
        task_id: String,
        #[arg(long)]
        state_root: Option<PathBuf>,
    },
    /// Retry a failed task from its saved input/output paths
    Retry {
        task_id: String,
        #[arg(long)]
        state_root: Option<PathBuf>,
    },
    /// Show the append-only audit history for one task
    Audit {
        task_id: String,
        #[arg(long)]
        state_root: Option<PathBuf>,
    },
    /// Summarize journal health without mutating it
    Maintenance {
        #[arg(long)]
        state_root: Option<PathBuf>,
    },
    /// Run a JSON Lines stdio worker using the same persistent handler
    Serve {
        #[arg(long)]
        state_root: Option<PathBuf>,
    },
}

#[derive(Subcommand)]
enum ApprovalsOp {
    /// Grant one time-limited approval bound to the complete invocation context
    Grant {
        #[arg(long)]
        command: String,
        #[arg(long = "arg", allow_hyphen_values = true)]
        arguments: Vec<String>,
        #[arg(long)]
        cwd: PathBuf,
        #[arg(long)]
        target: PathBuf,
        #[arg(long)]
        task_id: String,
        #[arg(long)]
        ttl_seconds: u64,
        #[arg(long)]
        state_root: Option<PathBuf>,
    },
    /// Check and consume an approval for an exactly matching invocation
    Check {
        approval_id: String,
        #[arg(long)]
        command: String,
        #[arg(long = "arg", allow_hyphen_values = true)]
        arguments: Vec<String>,
        #[arg(long)]
        cwd: PathBuf,
        #[arg(long)]
        target: PathBuf,
        #[arg(long)]
        task_id: String,
        #[arg(long)]
        state_root: Option<PathBuf>,
    },
    /// List local approval records for audit
    List {
        #[arg(long)]
        state_root: Option<PathBuf>,
    },
}

#[derive(Subcommand)]
enum ConfigOp {
    /// Show the current profile file without creating it
    File,
    /// Print the machine-readable configuration schema
    Schema,
    /// Validate the current profile document
    Validate,
    /// Get the complete document or one dotted key
    Get { key: Option<String> },
    /// Set one dotted key from a JSON value
    Set { key: String, value_json: String },
    /// Apply an RFC 7396 JSON Merge Patch to profile values
    Patch { patch_json: String },
    /// Remove one dotted key
    Unset { key: String },
}

#[derive(Subcommand)]
enum McpOp {
    /// Run the MCP server over stdio or Streamable HTTP
    Serve {
        #[arg(long)]
        state_root: Option<PathBuf>,
        /// Transport profile; sse forces Streamable HTTP event-stream responses
        #[arg(long, value_enum, default_value_t = McpTransport::Stdio)]
        transport: McpTransport,
        /// HTTP listen address (ignored by stdio)
        #[arg(long, default_value = "127.0.0.1:8765")]
        bind: std::net::SocketAddr,
        /// Single Streamable HTTP MCP endpoint
        #[arg(long, default_value = "/mcp")]
        endpoint: String,
        /// Environment variable containing the Bearer token
        #[arg(long, default_value = "JIANYING_MCP_TOKEN")]
        token_env: String,
        /// Accepted HTTP Host authority; repeat for aliases
        #[arg(long)]
        allowed_host: Vec<String>,
        /// Accepted browser Origin; repeat for Pad/web frontends
        #[arg(long)]
        allowed_origin: Vec<String>,
    },
    /// Print the MCP tool catalogue and schemas
    Tools,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum McpTransport {
    Stdio,
    StreamableHttp,
    Sse,
}

#[derive(Subcommand)]
enum TemplateOp {
    /// List registered templates in a template library
    List {
        #[arg(long)]
        root: PathBuf,
    },
    /// Save an independent draft copy as a registered template
    Save {
        draft: PathBuf,
        name: String,
        #[arg(long)]
        root: PathBuf,
        #[arg(long)]
        description: Option<String>,
    },
    /// Apply a registered template as a new independent draft
    Apply {
        template: PathBuf,
        new_name: String,
        #[arg(long)]
        root: PathBuf,
    },
    /// Extract a reusable text-style preset from a draft
    MakePreset {
        draft: PathBuf,
        name: String,
        #[arg(long)]
        root: PathBuf,
    },
    /// Apply a text-style preset to every caption in a draft
    ApplyPreset { draft: PathBuf, preset: PathBuf },
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

#[derive(clap::Args)]
struct BuildArgs {
    /// Plan file (jianying-cli-plan/v1)
    plan: PathBuf,
    /// New output directory for the draft
    #[arg(long)]
    out: PathBuf,
    /// SRT file to append as a subtitle track
    #[arg(long)]
    srt: Option<PathBuf>,
    /// SRT cue offset (tim() strings accepted)
    #[arg(long)]
    srt_offset: Option<String>,
    /// SRT font size
    #[arg(long)]
    srt_size: Option<f64>,
    /// SRT alignment 0/1/2
    #[arg(long)]
    srt_align: Option<u8>,
    /// SRT text color #RRGGBB
    #[arg(long)]
    srt_color: Option<String>,
    /// SRT stroke width 0-100
    #[arg(long)]
    srt_border: Option<f64>,
    /// SRT vertical position
    #[arg(long)]
    srt_y: Option<f64>,
    /// Seed schema markers from the newest app-written draft in this store
    #[arg(long)]
    seed: Option<PathBuf>,
    /// Build on top of an existing template draft
    #[arg(long)]
    template: Option<PathBuf>,
}

#[derive(clap::Args)]
struct DraftPathArgs {
    /// Draft directory
    draft: PathBuf,
}

#[derive(clap::Args)]
struct ProbeArgs {
    /// Media file to measure
    media: PathBuf,
}

#[derive(clap::Args)]
struct CatalogArgs {
    /// Limit to one catalogue domain
    #[arg(long)]
    domain: Option<String>,
    /// Search display names by substring
    #[arg(long)]
    search: Option<String>,
    /// Include VIP entries
    #[arg(long)]
    include_vip: bool,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum EnumNamespace {
    Capcut,
    Jianying,
}

impl EnumNamespace {
    fn as_str(self) -> &'static str {
        match self {
            Self::Capcut => "capcut",
            Self::Jianying => "jianying",
        }
    }
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum EnumCategory {
    Transitions,
    Masks,
    ImageIntros,
    ImageOutros,
    ImageCombos,
    TextIntros,
    TextOutros,
    TextLoopAnims,
    SceneEffects,
    CharacterEffects,
    AudioEffects,
    Fonts,
    Filters,
    Bubbles,
}

impl EnumCategory {
    fn as_str(self) -> &'static str {
        match self {
            Self::Transitions => "transitions",
            Self::Masks => "masks",
            Self::ImageIntros => "image_intros",
            Self::ImageOutros => "image_outros",
            Self::ImageCombos => "image_combos",
            Self::TextIntros => "text_intros",
            Self::TextOutros => "text_outros",
            Self::TextLoopAnims => "text_loop_anims",
            Self::SceneEffects => "scene_effects",
            Self::CharacterEffects => "character_effects",
            Self::AudioEffects => "audio_effects",
            Self::Fonts => "fonts",
            Self::Filters => "filters",
            Self::Bubbles => "bubbles",
        }
    }
}

#[derive(clap::Args)]
struct EnumArgs {
    /// Enum category to list
    #[arg(value_enum)]
    category: EnumCategory,
    /// Resource namespace
    #[arg(long, value_enum, default_value = "capcut")]
    namespace: EnumNamespace,
    /// Render a compact table instead of the machine-readable object
    #[arg(long)]
    human: bool,
}

#[derive(Subcommand)]
enum HarvestEnumsOp {
    /// Scan one draft; plan by default and write only with --apply
    Scan {
        draft: PathBuf,
        #[arg(long)]
        catalogue: Option<PathBuf>,
        #[arg(long)]
        apply: bool,
    },
    /// Scan every direct child draft in one library root
    Sync {
        #[arg(long)]
        drafts: Option<PathBuf>,
        #[arg(long)]
        catalogue: Option<PathBuf>,
        #[arg(long)]
        apply: bool,
    },
    /// Plan or register one manually witnessed resource id
    Add {
        kind: String,
        slug: String,
        resource_id: String,
        #[arg(long)]
        effect_id: Option<String>,
        #[arg(long)]
        catalogue: Option<PathBuf>,
        #[arg(long)]
        apply: bool,
    },
}

#[derive(clap::Args)]
struct PublishArgs {
    /// Draft directory
    draft: PathBuf,
    /// Draft store root
    #[arg(long)]
    root: Option<PathBuf>,
    /// Publish even if JianYing/CapCut is running
    #[arg(long)]
    force: bool,
}

#[derive(clap::Args)]
struct ProjectInitArgs {
    /// Draft display name
    name: String,
    /// New draft directory
    #[arg(long)]
    out: PathBuf,
    /// Canvas width
    #[arg(long, default_value_t = 1920)]
    width: u64,
    /// Canvas height
    #[arg(long, default_value_t = 1080)]
    height: u64,
    /// Timeline frames per second
    #[arg(long, default_value_t = 30)]
    fps: u64,
    /// Seed schema markers from the newest app-written draft in this store
    #[arg(long)]
    seed: Option<PathBuf>,
}

#[derive(clap::Args)]
struct ProjectQuickstartArgs {
    #[command(flatten)]
    init: ProjectInitArgs,
    /// Add one video at timeline zero
    #[arg(long)]
    video: Option<PathBuf>,
    /// Add one audio file at timeline zero
    #[arg(long)]
    audio: Option<PathBuf>,
    /// Import one SRT file as a subtitle track
    #[arg(long)]
    srt: Option<PathBuf>,
}

#[derive(Subcommand)]
enum ProjectOp {
    /// Create a new empty native draft
    Init(ProjectInitArgs),
    /// Create a new draft from one or more media/subtitle inputs
    Quickstart(ProjectQuickstartArgs),
    /// Build a native draft from a plan
    Build(BuildArgs),
    /// Verify draft structure and bundle integrity
    Verify(DraftPathArgs),
    /// Inspect draft metadata and tracks
    Inspect(DraftPathArgs),
    /// Diagnose canonical sibling files, divergence and write safety
    Diagnose {
        draft: PathBuf,
        #[arg(long)]
        bundle: Option<PathBuf>,
        #[arg(long)]
        human: bool,
    },
    /// Build or mechanically verify a redacted, media-free compatibility bundle
    Fixture {
        draft_or_bundle: PathBuf,
        #[arg(long)]
        out: Option<PathBuf>,
        #[arg(long)]
        check: bool,
    },
    /// Compile a complete draft from the declarative capcut-cli-compatible specification
    Compile {
        spec: PathBuf,
        #[arg(long)]
        out: Option<PathBuf>,
        #[arg(long)]
        drafts: Option<PathBuf>,
        #[arg(long)]
        data: Option<String>,
        #[arg(long)]
        check: bool,
        #[arg(long)]
        plan: bool,
        #[arg(long)]
        continue_on_error: bool,
    },
    /// Read a stable structural project summary
    Info(DraftPathArgs),
    /// Detect draft wire-version markers
    Version(DraftPathArgs),
    /// Emit the complete command surface as an agent tool specification
    Describe,
    /// Compare segments, materials and track counts in two drafts
    Diff { left: PathBuf, right: PathBuf },
    /// Migrate known wire-schema boundaries through a validated transaction
    Migrate {
        draft: PathBuf,
        #[arg(long)]
        from: Option<String>,
        #[arg(long)]
        to: Option<String>,
        /// Copy schema markers from another app-written draft
        #[arg(long)]
        like: Option<PathBuf>,
        /// Copy markers from the newest other app-written draft in the store
        #[arg(long)]
        from_store: bool,
    },
    /// Append the second draft after the first with collision-safe ids
    Concat {
        left: PathBuf,
        right: PathBuf,
        #[arg(long)]
        out: Option<PathBuf>,
    },
    /// Extract one time range as a standalone timeline JSON
    Cut {
        draft: PathBuf,
        start: String,
        end: String,
        #[arg(long)]
        out: PathBuf,
    },
    /// Export video/audio tracks and optional caption markers as OpenTimelineIO
    ExportTimeline {
        draft: PathBuf,
        #[arg(long)]
        out: Option<PathBuf>,
        #[arg(long, default_value = "skip")]
        captions: String,
        #[arg(long)]
        quiet: bool,
    },
    /// Import an OpenTimelineIO cut into a new or existing draft
    ImportTimeline {
        file: PathBuf,
        #[arg(long, conflicts_with = "into")]
        out: Option<PathBuf>,
        #[arg(long, conflicts_with = "out")]
        into: Option<PathBuf>,
        #[arg(long)]
        dry_run: bool,
        #[arg(long)]
        quiet: bool,
    },
    /// Remove material entries that no surviving segment references
    Prune {
        draft: PathBuf,
        #[arg(long)]
        dry_run: bool,
    },
    /// Set or replace the project cover image
    AddCover {
        draft: PathBuf,
        image: PathBuf,
        /// Cover time point in milliseconds
        #[arg(long, default_value_t = 0)]
        time: i64,
    },
    /// Report project capability state
    Status,
}

#[derive(clap::Args)]
struct MediaTtsArgs {
    draft: PathBuf,
    start: Option<String>,
    duration: Option<String>,
    #[arg(long, conflicts_with = "text_file")]
    text: Option<String>,
    #[arg(long, conflicts_with = "text")]
    text_file: Option<PathBuf>,
    #[arg(long, value_enum, default_value_t = TtsProviderArg::LocalCommand)]
    provider: TtsProviderArg,
    /// Command template for local-command; executed without a shell and must contain {out}
    #[arg(long)]
    tts_cmd: Option<String>,
    /// Loopback URL for local-http
    #[arg(long)]
    endpoint: Option<String>,
    /// Absolute executable path for local-process or edge-tts
    #[arg(long)]
    executable: Option<PathBuf>,
    /// Repeatable structured argv template for local-process
    #[arg(long = "provider-arg")]
    provider_args: Vec<String>,
    #[arg(long, value_enum)]
    format: Option<TtsFormatArg>,
    #[arg(long)]
    model: Option<String>,
    #[arg(long)]
    voice: Option<String>,
    #[arg(long)]
    language: Option<String>,
    #[arg(long)]
    emotion: Option<String>,
    #[arg(long)]
    ssml: bool,
    #[arg(long, default_value_t = 1.0)]
    speed: f32,
    #[arg(long = "provider-volume", default_value_t = 1.0)]
    provider_volume: f32,
    #[arg(long, default_value_t = 0.0)]
    pitch: f32,
    #[arg(long, default_value_t = 1.0)]
    volume: f64,
    #[arg(long)]
    track_name: Option<String>,
    /// Environment variable name containing the cloud credential; never the secret value
    #[arg(long)]
    credential_env: Option<String>,
    /// Stable task identity included in the exact approval binding
    #[arg(long)]
    task_id: Option<String>,
    /// Maximum approved provider cost in account-defined micro-units
    #[arg(long)]
    max_cost_microunits: Option<u64>,
    /// Generate a separately-bound paid live-canary approval
    #[arg(long)]
    live_canary: bool,
    /// Validate the cloud request and print its exact approval binding without networking
    #[arg(long, conflicts_with = "approval_id")]
    plan: bool,
    /// Exact approval to consume before the first paid network submission
    #[arg(long, conflicts_with = "plan")]
    approval_id: Option<String>,
    /// Approval record directory; defaults to JIANYING_APPROVAL_ROOT or .jianying-approvals
    #[arg(long)]
    approval_root: Option<PathBuf>,
    /// Cloud TTS ledger and durable artifact directory
    #[arg(long)]
    tts_state_root: Option<PathBuf>,
    /// Loopback-only endpoint override for offline protocol tests
    #[arg(long, hide = true)]
    cloud_endpoint_override: Option<String>,
    /// Aliyun protocol family: qwen-audio, cosyvoice, or qwen-tts
    #[arg(long)]
    aliyun_family: Option<String>,
    /// Aliyun workspace ID for Qwen-Audio or CosyVoice
    #[arg(long)]
    workspace_id: Option<String>,
    /// Legacy Volcengine V1 application ID; rejected by the V3 adapter
    #[arg(long)]
    app_id: Option<String>,
    /// Legacy Volcengine V1 cluster; rejected by the V3 adapter
    #[arg(long)]
    cluster: Option<String>,
    /// Volcengine V3 resource ID written to X-Api-Resource-Id
    #[arg(long)]
    resource_id: Option<String>,
    /// Volcengine user identity
    #[arg(long)]
    uid: Option<String>,
    /// Baidu client identity
    #[arg(long)]
    cuid: Option<String>,
    #[arg(long)]
    sample_rate: Option<u32>,
    #[arg(long)]
    streaming: bool,
    #[arg(long)]
    timestamps: bool,
}

#[derive(clap::Args)]
struct MediaTranscribeArgs {
    /// Existing local audio/video file accepted by whisper-cli
    source: PathBuf,
    /// New transcript artifact path
    #[arg(long)]
    out: PathBuf,
    /// Absolute path to the official whisper.cpp whisper-cli executable
    #[arg(long)]
    executable: PathBuf,
    /// Absolute path to one locally reviewed GGML/GGUF Whisper model
    #[arg(long)]
    model: PathBuf,
    /// Stable logical model identity included in the idempotency request
    #[arg(long)]
    model_id: String,
    /// Transcript output format
    #[arg(long, value_enum, default_value_t = AsrFormatArg::VerboseJson)]
    format: AsrFormatArg,
    /// Spoken language passed to whisper-cli; omit to use its configured default
    #[arg(long)]
    language: Option<String>,
    /// Ask whisper.cpp to translate speech into English
    #[arg(long)]
    translate: bool,
    /// Require segment timestamps; valid only with verbose-json
    #[arg(long)]
    segment_timestamps: bool,
    /// Stable task identity recorded by the ASR ledger
    #[arg(long)]
    task_id: String,
    /// ASR ledger directory; defaults to JIANYING_ASR_STATE_ROOT or .jianying-asr
    #[arg(long)]
    state_root: Option<PathBuf>,
    /// Validate identities and print the idempotent execution plan without running whisper-cli
    #[arg(long, conflicts_with = "retry")]
    plan: bool,
    /// Explicitly retry a ledger entry in failed state
    #[arg(long, conflicts_with = "plan")]
    retry: bool,
}

#[derive(Subcommand)]
enum MediaOp {
    /// Probe one local media file
    Probe(ProbeArgs),
    /// Search the embedded resource catalogue
    Catalog(CatalogArgs),
    /// List one fixed CapCut or JianYing enum category
    Enums(EnumArgs),
    /// Learn app-authored resource ids into a separate user catalogue
    HarvestEnums {
        #[command(subcommand)]
        op: HarvestEnumsOp,
    },
    /// List material type counts or summaries for one type
    Materials {
        draft: PathBuf,
        #[arg(long = "type")]
        material_type: Option<String>,
    },
    /// Read one lossless material by full id or case-insensitive prefix
    Material { draft: PathBuf, id: String },
    /// Add a probed local video or image to an editable draft
    AddVideo {
        draft: PathBuf,
        source: PathBuf,
        start: String,
        duration: Option<String>,
        #[arg(long)]
        track_name: Option<String>,
    },
    /// Add a probed local audio file to an editable draft
    AddAudio {
        draft: PathBuf,
        source: PathBuf,
        start: String,
        duration: Option<String>,
        #[arg(long, default_value_t = 1.0)]
        volume: f64,
        #[arg(long)]
        track_name: Option<String>,
    },
    /// Add a store sticker on a sticker track
    AddSticker {
        draft: PathBuf,
        resource_id: String,
        start: String,
        duration: String,
        #[arg(long, allow_hyphen_values = true)]
        x: Option<f64>,
        #[arg(long, allow_hyphen_values = true)]
        y: Option<f64>,
        #[arg(long)]
        scale: Option<f64>,
        #[arg(long, allow_hyphen_values = true)]
        rotation: Option<f64>,
        #[arg(long)]
        track_name: Option<String>,
    },
    /// Replace the source material behind one segment
    Replace {
        draft: PathBuf,
        segment_id: String,
        source: PathBuf,
        #[arg(long)]
        retime: bool,
    },
    /// Repair media paths by prefix or basename and optionally stage assets
    Relink {
        draft: PathBuf,
        #[arg(long)]
        dir: Option<PathBuf>,
        #[arg(long)]
        from: Option<PathBuf>,
        #[arg(long)]
        to: Option<PathBuf>,
        #[arg(long)]
        stage: bool,
    },
    /// Detect content-aware scene cut points with ffmpeg
    Scenes {
        media: PathBuf,
        #[arg(long, default_value_t = 0.4)]
        threshold: f64,
        #[arg(long, default_value_t = 2.0)]
        min_gap: f64,
        #[arg(long)]
        limit: Option<usize>,
        #[arg(long)]
        ffmpeg_cmd: Option<PathBuf>,
    },
    /// Detect silence and complementary keep spans with ffmpeg
    Silence {
        media: PathBuf,
        #[arg(long, default_value_t = -30.0)]
        threshold_db: f64,
        #[arg(long, default_value_t = 0.5)]
        min_silence: f64,
        #[arg(long, default_value_t = 0.1)]
        pad: f64,
        #[arg(long)]
        limit: Option<usize>,
        #[arg(long)]
        ffmpeg_cmd: Option<PathBuf>,
    },
    /// Detect repeated takes from an SRT transcript
    Retakes {
        /// Draft directory; omit when using --srt
        draft: Option<PathBuf>,
        #[arg(long)]
        srt: Option<PathBuf>,
        #[arg(long)]
        track_name: Option<String>,
        #[arg(long, default_value_t = 60.0)]
        window: f64,
        #[arg(long, default_value_t = 0.8)]
        similarity: f64,
        #[arg(long, default_value_t = 4)]
        min_words: usize,
    },
    /// Synthesize a voiceover through a capability-checked local, system, or cloud Provider
    Tts(Box<MediaTtsArgs>),
    /// Transcribe local media through a fixed whisper.cpp executable and model
    Transcribe(MediaTranscribeArgs),
    /// Add a bundled CapCut sound effect by slug
    Sfx {
        draft: PathBuf,
        slug: String,
        start: String,
        duration: String,
        #[arg(long, default_value_t = 1.0)]
        volume: f64,
        #[arg(long)]
        track_name: Option<String>,
    },
    /// Report media capability state
    Status,
}

#[derive(Subcommand)]
enum StoreOp {
    /// Publish and register a built draft
    Publish(PublishArgs),
    /// Compatibility name for publishing and registering a built draft
    Register(PublishArgs),
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
    /// Rename one registered draft without changing its draft id
    Rename {
        name: String,
        new_name: String,
        #[arg(long)]
        root: Option<PathBuf>,
    },
    /// Plan or apply canonical-to-mirror timeline synchronization
    Sync {
        draft: PathBuf,
        #[arg(long)]
        apply: bool,
        #[arg(long, requires = "apply")]
        force_newer: bool,
    },
    /// Create a complete, validated draft backup
    Backup {
        draft: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
    /// Restore a complete backup through a validated transaction
    Restore {
        backup: PathBuf,
        #[arg(long)]
        target: PathBuf,
    },
    /// Detect JianYing encryption; intentionally does not decrypt
    Decrypt { input: PathBuf },
    /// List platform draft-root candidates
    Directories,
    /// Restore a transaction snapshot through validation and atomic commit
    RestoreSnapshot {
        #[arg(long)]
        snapshot: PathBuf,
        #[arg(long)]
        target: PathBuf,
    },
}

#[derive(Subcommand)]
enum Command {
    /// Project lifecycle and draft-level operations
    Project {
        #[command(subcommand)]
        op: ProjectOp,
    },
    /// Timeline, track, segment and editing operations
    Timeline {
        #[command(subcommand)]
        op: TimelineOp,
    },
    /// Media probing, registration and analysis operations
    Media {
        #[command(subcommand)]
        op: MediaOp,
    },
    /// Caption import, export, styling and translation operations
    Captions {
        #[command(subcommand)]
        op: CaptionsOp,
    },
    /// Native editor discovery, control and export operations
    Runtime {
        #[command(subcommand)]
        op: RuntimeOp,
    },
    /// 环境预检：草稿根、运行中的剪映/CapCut、ffprobe/ffmpeg（只读，恒 JSON）
    Doctor,
    /// Summarize editor, dependency, capability and persistent-job health
    Status {
        #[arg(long)]
        state_root: Option<PathBuf>,
    },
    /// Read the append-only audit view for one task or the whole local journal
    Audit {
        task_id: Option<String>,
        #[arg(long)]
        state_root: Option<PathBuf>,
    },
    /// Generate shell completion without installing or modifying shell files
    Completion { shell: Shell },
    /// Print the embedded schema and capability manifest (read-only)
    Capabilities,
    /// Print the live machine-readable command catalogue
    Commands,
    /// 实测媒体（时长/分辨率/流/是否图片）；所有计划时长应以它为准
    Probe(ProbeArgs),
    /// 计划 → 草稿目录（构建后自动跑结构校验，失败即报错不产出）
    /// 例：jianying build plan.json --out d1 --srt subs.srt --seed <草稿根>
    Build(BuildArgs),
    /// 模板模式（对已有草稿）：inspect/duplicate/replace-text/replace-material/import-track
    Template {
        #[command(subcommand)]
        op: TemplateOp,
    },
    /// 能力目录检索：列 16 个域或按名搜索（转场/滤镜/字体/特效/动画/蒙版…）
    /// 例：jianying catalog --domain transitions --search 叠化
    Catalog(CatalogArgs),
    /// 草稿库管理：list/has/remove（remove 需 --yes）
    Store {
        #[command(subcommand)]
        op: StoreOp,
    },
    /// 结构 lint：引用完整/主轨连续/时长一致（只读）
    Verify(DraftPathArgs),
    /// 草稿摘要：轨道/段数/画布/平台（只读）
    Inspect(DraftPathArgs),
    /// 拷入草稿库并注册 root_meta_info.json（剪映运行中拒绝；同名拒绝）
    Publish(PublishArgs),
    /// ffmpeg 代理渲染（预览级：平铺主轨+混音+可选烧字幕；不含转场/特效/蒙版）
    Render {
        /// Preferred grouped render operation; omit only for the deprecated legacy form
        #[command(subcommand)]
        op: Option<RenderOp>,
        /// Draft directory
        draft: Option<PathBuf>,
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
    /// Versioned job execution for agents and runtime adapters
    Job {
        #[command(subcommand)]
        op: JobOp,
    },
    /// Time-limited, exact-binding approvals for mutating operations
    Approvals {
        #[command(subcommand)]
        op: ApprovalsOp,
    },
    /// Isolated, versioned configuration profiles
    Config {
        #[command(subcommand)]
        op: ConfigOp,
    },
    /// Model Context Protocol server and tool discovery
    Mcp {
        #[command(subcommand)]
        op: McpOp,
    },
}

fn run(cmd: Command, json: bool, profile: &str, host_read_only: bool) -> Result<()> {
    match cmd {
        Command::Project { op } => match op {
            ProjectOp::Init(args) => run_project_init(args, json)?,
            ProjectOp::Quickstart(args) => run_project_quickstart(args, json)?,
            ProjectOp::Build(args) => run_build(args, json)?,
            ProjectOp::Verify(args) => run_verify(args, json)?,
            ProjectOp::Inspect(args) => run_inspect(args, json)?,
            ProjectOp::Diagnose {
                draft,
                bundle,
                human,
            } => {
                if human && json {
                    bail!("--human and --json cannot be used together");
                }
                let report = project_ops::diagnose(&draft, bundle.as_deref())?;
                if human {
                    println!(
                        "Canonical: {}",
                        report["canonical"].as_str().unwrap_or("unknown")
                    );
                    println!(
                        "Layout:    {}",
                        report["layout"].as_str().unwrap_or("unknown")
                    );
                    println!(
                        "Version:   {}",
                        report["version"].as_str().unwrap_or("unknown")
                    );
                    println!(
                        "Diverged:  {}",
                        if report["diverged"].as_bool() == Some(true) {
                            "YES"
                        } else {
                            "no"
                        }
                    );
                    println!(
                        "Editor:    {}",
                        report["editor_running"]
                            .as_array()
                            .into_iter()
                            .flatten()
                            .filter_map(serde_json::Value::as_str)
                            .collect::<Vec<_>>()
                            .join(", ")
                    );
                    for candidate in report["candidates"].as_array().into_iter().flatten() {
                        let state = if candidate["exists"].as_bool() != Some(true) {
                            "missing"
                        } else if candidate["parseable_timeline"].as_bool() == Some(true) {
                            "timeline"
                        } else {
                            "unreadable"
                        };
                        println!(
                            "{:<24} {:<10} {:>9} bytes",
                            candidate["file"].as_str().unwrap_or_default(),
                            state,
                            candidate["size"].as_u64().unwrap_or(0)
                        );
                    }
                } else {
                    print_output(report, json);
                }
            }
            ProjectOp::Fixture {
                draft_or_bundle,
                out,
                check,
            } => {
                if let Some(out) = out {
                    let mut report =
                        jianying_cli::fixture_ops::build_bundle(&draft_or_bundle, &out)?;
                    if check {
                        let redaction_check =
                            jianying_cli::fixture_ops::verify_bundle_redaction(&out)?;
                        if !redaction_check.ok {
                            return Err(jianying_cli::fixture_ops::FixtureError::RedactionFailed {
                                bundle_dir: redaction_check.bundle_dir,
                                files_scanned: redaction_check.files_scanned,
                                findings: redaction_check.findings,
                            }
                            .into());
                        }
                        report["redaction_check"] = serde_json::to_value(redaction_check)?;
                    }
                    print_output(report, json);
                } else if check && draft_or_bundle.join("SANITIZE_REPORT.json").is_file() {
                    let redaction_check =
                        jianying_cli::fixture_ops::verify_bundle_redaction(&draft_or_bundle)?;
                    if !redaction_check.ok {
                        return Err(jianying_cli::fixture_ops::FixtureError::RedactionFailed {
                            bundle_dir: redaction_check.bundle_dir,
                            files_scanned: redaction_check.files_scanned,
                            findings: redaction_check.findings,
                        }
                        .into());
                    }
                    print_output(serde_json::to_value(redaction_check)?, json);
                } else {
                    bail!("fixture requires --out <dir>, or an existing bundle with --check");
                }
            }
            ProjectOp::Compile {
                spec,
                out,
                drafts,
                data,
                check,
                plan,
                continue_on_error,
            } => print_output(
                jianying_cli::compile_ops::run_cli(
                    &spec,
                    out.as_deref(),
                    drafts.as_deref(),
                    data.as_deref(),
                    check || plan,
                    continue_on_error,
                )?,
                json,
            ),
            ProjectOp::Info(args) => print_output(project_ops::info(&args.draft)?, json),
            ProjectOp::Version(args) => print_output(project_ops::version(&args.draft)?, json),
            ProjectOp::Describe => print_output(machine_command_catalog()?, json),
            ProjectOp::Diff { left, right } => {
                print_output(project_ops::diff(&left, &right)?, json)
            }
            ProjectOp::Migrate {
                draft,
                from,
                to,
                like,
                from_store,
            } => {
                let result = match (from, to, like, from_store) {
                    (Some(from), Some(to), None, false) => {
                        project_ops::migrate(&draft, &from, &to)?
                    }
                    (None, None, Some(donor), false) => {
                        project_ops::restamp(&draft, &donor)?
                    }
                    (None, None, None, true) => project_ops::restamp_from_store(&draft)?,
                    _ => bail!("use exactly one migration mode: --from <ver> --to <ver>, --like <draft>, or --from-store"),
                };
                print_output(result, json)
            }
            ProjectOp::Concat { left, right, out } => {
                print_output(project_ops::concat(&left, &right, out.as_deref())?, json)
            }
            ProjectOp::Cut {
                draft,
                start,
                end,
                out,
            } => print_output(
                project_ops::cut(&draft, tim::parse(&start)?, tim::parse(&end)?, &out)?,
                json,
            ),
            ProjectOp::ExportTimeline {
                draft,
                out,
                captions,
                quiet,
            } => print_output(
                interchange::export_timeline(&draft, out.as_deref(), &captions, quiet)?,
                json,
            ),
            ProjectOp::ImportTimeline {
                file,
                out,
                into,
                dry_run,
                quiet,
            } => print_output(
                interchange::import_timeline(
                    &file,
                    out.as_deref(),
                    into.as_deref(),
                    dry_run,
                    quiet,
                )?,
                json,
            ),
            ProjectOp::Prune { draft, dry_run } => {
                print_output(timeline_ops::prune(&draft, dry_run)?, json)
            }
            ProjectOp::AddCover { draft, image, time } => {
                print_output(project_ops::add_cover(&draft, &image, time)?, json)
            }
            ProjectOp::Status => print_domain_status("project", json)?,
        },
        Command::Timeline { op } => match op {
            TimelineOp::Show { draft, cols } => {
                print_output(timeline_ops::show(&draft, cols)?, json)
            }
            TimelineOp::Tracks(args) => print_output(timeline_ops::tracks(&args.draft)?, json),
            TimelineOp::Segments { draft, track } => {
                print_output(timeline_ops::segments(&draft, track.as_deref())?, json)
            }
            TimelineOp::Get { draft, id } => {
                print_output(timeline_ops::segment(&draft, &id)?, json)
            }
            TimelineOp::Move { draft, id, offset } => print_output(
                timeline_ops::move_segment(&draft, &id, tim::parse(&offset)?)?,
                json,
            ),
            TimelineOp::MoveAll {
                draft,
                offset,
                track,
            } => print_output(
                timeline_ops::move_all(&draft, tim::parse(&offset)?, track.as_deref())?,
                json,
            ),
            TimelineOp::Speed {
                draft,
                id,
                multiplier,
            } => print_output(timeline_ops::speed(&draft, &id, multiplier)?, json),
            TimelineOp::Volume { draft, id, level } => {
                print_output(timeline_ops::volume(&draft, &id, level)?, json)
            }
            TimelineOp::Trim {
                draft,
                id,
                start,
                duration,
            } => print_output(
                timeline_ops::trim(&draft, &id, tim::parse(&start)?, tim::parse(&duration)?)?,
                json,
            ),
            TimelineOp::Opacity { draft, id, alpha } => {
                print_output(timeline_ops::opacity(&draft, &id, alpha)?, json)
            }
            TimelineOp::AddTrack { draft, kind, name } => {
                print_output(timeline_ops::add_track(&draft, &kind, &name)?, json)
            }
            TimelineOp::AddSegment {
                draft,
                track_id,
                segment_json,
            } => print_output(
                timeline_ops::add_segment(&draft, &track_id, &segment_json)?,
                json,
            ),
            TimelineOp::Set {
                draft,
                id,
                start,
                duration,
                speed,
                volume,
                opacity,
            } => print_output(
                timeline_ops::set_segment(
                    &draft,
                    &id,
                    start.as_deref().map(tim::parse).transpose()?,
                    duration.as_deref().map(tim::parse).transpose()?,
                    speed,
                    volume,
                    opacity,
                )?,
                json,
            ),
            TimelineOp::Split { draft, id, at } => {
                print_output(timeline_ops::split(&draft, &id, tim::parse(&at)?)?, json)
            }
            TimelineOp::Duplicate {
                draft,
                id,
                track,
                new_track: _,
                dry_run,
            } => print_output(
                timeline_ops::duplicate(&draft, &id, track.as_deref(), dry_run)?,
                json,
            ),
            TimelineOp::Remove {
                draft,
                id,
                keep_track,
                keep_materials,
                dry_run,
            } => print_output(
                timeline_ops::remove(&draft, &id, keep_track, keep_materials, dry_run)?,
                json,
            ),
            TimelineOp::Composite { draft, id, mode } => {
                print_output(timeline_ops::composite(&draft, &id, &mode)?, json)
            }
            TimelineOp::Matting { draft, id, off } => {
                print_output(timeline_ops::matting(&draft, &id, off)?, json)
            }
            TimelineOp::Chroma {
                draft,
                id,
                color,
                intensity,
                off,
            } => print_output(
                timeline_ops::chroma(&draft, &id, color.as_deref(), intensity, off)?,
                json,
            ),
            TimelineOp::Mask {
                draft,
                id,
                slug,
                off,
                jianying,
                center_x,
                center_y,
                size,
                rotation,
                feather,
                invert,
                rect_width,
                round_corner,
                mask_field,
            } => print_output(
                timeline_ops::mask(
                    &draft,
                    &id,
                    slug.as_deref(),
                    off,
                    jianying,
                    center_x,
                    center_y,
                    size,
                    rotation,
                    feather,
                    invert,
                    rect_width,
                    round_corner,
                    mask_field.as_deref(),
                )?,
                json,
            ),
            TimelineOp::BgBlur {
                draft,
                id,
                level,
                off,
            } => print_output(
                timeline_ops::background_blur(&draft, &id, level, off)?,
                json,
            ),
            TimelineOp::AudioFade {
                draft,
                id,
                fade_in,
                fade_out,
            } => {
                let to_microseconds = |seconds: Option<f64>| -> Result<i64> {
                    let seconds = seconds.unwrap_or(0.0);
                    if !seconds.is_finite() {
                        bail!("audio-fade durations must be finite");
                    }
                    Ok((seconds * 1_000_000.0).round() as i64)
                };
                print_output(
                    timeline_ops::audio_fade(
                        &draft,
                        &id,
                        to_microseconds(fade_in)?,
                        to_microseconds(fade_out)?,
                    )?,
                    json,
                )
            }
            TimelineOp::AddFilter {
                draft,
                slug,
                start,
                duration,
                full,
                intensity,
                track_name,
                jianying,
                resource_id,
                effect_id,
            } => {
                let (start_us, duration_us) = if full {
                    (None, None)
                } else {
                    let start =
                        start.context("add-filter requires <start> <duration> or --full")?;
                    let duration =
                        duration.context("add-filter requires <start> <duration> or --full")?;
                    (Some(tim::parse(&start)?), Some(tim::parse(&duration)?))
                };
                print_output(
                    timeline_ops::add_filter(
                        &draft,
                        &slug,
                        start_us,
                        duration_us,
                        full,
                        intensity,
                        track_name.as_deref(),
                        jianying,
                        resource_id.as_deref(),
                        effect_id.as_deref(),
                    )?,
                    json,
                )
            }
            TimelineOp::AddEffect {
                draft,
                slug,
                start,
                duration,
                full,
                params,
                intensity,
                track_name,
                jianying,
                resource_id,
                effect_id,
                bind_segment_id,
            } => {
                let (start_us, duration_us) = if full {
                    (None, None)
                } else {
                    let start =
                        start.context("add-effect requires <start> <duration> or --full")?;
                    let duration =
                        duration.context("add-effect requires <start> <duration> or --full")?;
                    (Some(tim::parse(&start)?), Some(tim::parse(&duration)?))
                };
                let params = params
                    .map(|raw| serde_json::from_str::<Vec<f64>>(&raw))
                    .transpose()
                    .context("--params must be a JSON array of numbers")?;
                print_output(
                    timeline_ops::add_effect(
                        &draft,
                        &slug,
                        start_us,
                        duration_us,
                        full,
                        params.as_deref(),
                        intensity,
                        track_name.as_deref(),
                        jianying,
                        resource_id.as_deref(),
                        effect_id.as_deref(),
                        bind_segment_id.as_deref(),
                    )?,
                    json,
                )
            }
            TimelineOp::Crop {
                draft,
                id,
                ratio,
                rect,
                reset,
                dry_run,
            } => print_output(
                timeline_ops::crop(
                    &draft,
                    &id,
                    rect.as_deref(),
                    ratio.as_deref(),
                    reset,
                    dry_run,
                )?,
                json,
            ),
            TimelineOp::Keyframe {
                draft,
                id,
                property,
                time,
                value,
                batch,
                easing,
            } => {
                let inputs = if batch {
                    if property.is_some() || time.is_some() || value.is_some() {
                        bail!("--batch reads JSONL from stdin and does not accept property/time/value positionals");
                    }
                    let mut raw = String::new();
                    std::io::stdin().read_to_string(&mut raw)?;
                    let raw = raw.trim_start_matches('\u{feff}').trim();
                    if raw.is_empty() {
                        bail!("No input on stdin for --batch");
                    }
                    raw.lines()
                        .filter(|line| !line.trim().is_empty())
                        .map(|line| {
                            let item: serde_json::Value = serde_json::from_str(line.trim())?;
                            let property = item["property"]
                                .as_str()
                                .context("batch keyframe requires property")?;
                            let time_us =
                                if let Some(value) = item["time"].as_i64() {
                                    value
                                } else {
                                    tim::parse(item["time"].as_str().context(
                                        "batch keyframe requires string or integer time",
                                    )?)?
                                };
                            let value_raw = item["value"]
                                .as_str()
                                .map(str::to_owned)
                                .unwrap_or_else(|| item["value"].to_string());
                            let (_, value) =
                                timeline_ops::parse_keyframe_value(property, &value_raw)?;
                            Ok(timeline_ops::KeyframeInput {
                                property: property.to_owned(),
                                time_us,
                                value,
                                easing: item["easing"].as_str().map(str::to_owned),
                            })
                        })
                        .collect::<Result<Vec<_>>>()?
                } else {
                    let property = property
                        .context("keyframe requires <property> <time> <value> or --batch")?;
                    let time_us = tim::parse(&time.context("keyframe requires <time>")?)?;
                    let (_, value) = timeline_ops::parse_keyframe_value(
                        &property,
                        &value.context("keyframe requires <value>")?,
                    )?;
                    vec![timeline_ops::KeyframeInput {
                        property,
                        time_us,
                        value,
                        easing: None,
                    }]
                };
                print_output(
                    timeline_ops::keyframe(&draft, &id, &inputs, easing.as_deref())?,
                    json,
                )
            }
            TimelineOp::Transition {
                draft,
                id,
                slug,
                duration,
                jianying,
            } => print_output(
                timeline_ops::transition(
                    &draft,
                    &id,
                    &slug,
                    duration.as_deref().map(tim::parse).transpose()?,
                    jianying,
                )?,
                json,
            ),
            TimelineOp::ImageAnimation {
                draft,
                id,
                intro,
                outro,
                combo,
                intro_duration,
                outro_duration,
                combo_duration,
                jianying,
            } => print_output(
                timeline_ops::image_animation(
                    &draft,
                    &id,
                    timeline_ops::ImageAnimationOptions {
                        intro: intro.as_deref(),
                        outro: outro.as_deref(),
                        combo: combo.as_deref(),
                        intro_duration_us: intro_duration.as_deref().map(tim::parse).transpose()?,
                        outro_duration_us: outro_duration.as_deref().map(tim::parse).transpose()?,
                        combo_duration_us: combo_duration.as_deref().map(tim::parse).transpose()?,
                        jianying,
                    },
                )?,
                json,
            ),
            TimelineOp::QuantizationReport {
                start,
                duration,
                fps_numerator,
                fps_denominator,
                maximum_drift,
            } => {
                let source = jianying_cli::domain::TimeRange::new(
                    tim::parse(&start)?,
                    tim::parse(&duration)?,
                )?;
                let frame_rate =
                    jianying_cli::domain::FrameRate::new(fps_numerator, fps_denominator)?;
                let report = frame_rate.quantize_containing(source, tim::parse(&maximum_drift)?)?;
                print_output(serde_json::to_value(report)?, json);
            }
            TimelineOp::Status => print_domain_status("timeline", json)?,
        },
        Command::Media { op } => match op {
            MediaOp::Probe(args) => run_probe(args, json)?,
            MediaOp::Catalog(args) => run_catalog(args, json)?,
            MediaOp::Enums(args) => run_enums(args, json)?,
            MediaOp::HarvestEnums { op } => match op {
                HarvestEnumsOp::Scan {
                    draft,
                    catalogue,
                    apply,
                } => run_harvest_enum_scan(&draft, catalogue.as_deref(), apply, json)?,
                HarvestEnumsOp::Sync {
                    drafts,
                    catalogue,
                    apply,
                } => run_harvest_enum_sync(drafts.as_deref(), catalogue.as_deref(), apply, json)?,
                HarvestEnumsOp::Add {
                    kind,
                    slug,
                    resource_id,
                    effect_id,
                    catalogue,
                    apply,
                } => run_harvest_enum_add(
                    &kind,
                    &slug,
                    &resource_id,
                    effect_id.as_deref(),
                    catalogue.as_deref(),
                    apply,
                    json,
                )?,
            },
            MediaOp::Materials {
                draft,
                material_type,
            } => print_output(
                media_ops::materials(&draft, material_type.as_deref())?,
                json,
            ),
            MediaOp::Material { draft, id } => {
                print_output(media_ops::material(&draft, &id)?, json)
            }
            MediaOp::AddVideo {
                draft,
                source,
                start,
                duration,
                track_name,
            } => print_output(
                media_ops::add(
                    &draft,
                    &source,
                    "video",
                    tim::parse(&start)?,
                    duration.as_deref().map(tim::parse).transpose()?,
                    track_name.as_deref(),
                    1.0,
                )?,
                json,
            ),
            MediaOp::AddAudio {
                draft,
                source,
                start,
                duration,
                volume,
                track_name,
            } => print_output(
                media_ops::add(
                    &draft,
                    &source,
                    "audio",
                    tim::parse(&start)?,
                    duration.as_deref().map(tim::parse).transpose()?,
                    track_name.as_deref(),
                    volume,
                )?,
                json,
            ),
            MediaOp::AddSticker {
                draft,
                resource_id,
                start,
                duration,
                x,
                y,
                scale,
                rotation,
                track_name,
            } => print_output(
                media_ops::add_sticker(
                    &draft,
                    &resource_id,
                    tim::parse(&start)?,
                    tim::parse(&duration)?,
                    x,
                    y,
                    scale,
                    rotation,
                    track_name.as_deref(),
                )?,
                json,
            ),
            MediaOp::Replace {
                draft,
                segment_id,
                source,
                retime,
            } => print_output(
                media_ops::replace(&draft, &segment_id, &source, retime)?,
                json,
            ),
            MediaOp::Relink {
                draft,
                dir,
                from,
                to,
                stage,
            } => print_output(
                media_ops::relink(
                    &draft,
                    dir.as_deref(),
                    from.as_deref(),
                    to.as_deref(),
                    stage,
                )?,
                json,
            ),
            MediaOp::Scenes {
                media,
                threshold,
                min_gap,
                limit,
                ffmpeg_cmd,
            } => print_output(
                media_analysis::scenes(&media, threshold, min_gap, limit, ffmpeg_cmd.as_deref())?,
                json,
            ),
            MediaOp::Silence {
                media,
                threshold_db,
                min_silence,
                pad,
                limit,
                ffmpeg_cmd,
            } => print_output(
                media_analysis::silence(
                    &media,
                    threshold_db,
                    min_silence,
                    pad,
                    limit,
                    ffmpeg_cmd.as_deref(),
                )?,
                json,
            ),
            MediaOp::Retakes {
                draft,
                srt,
                track_name,
                window,
                similarity,
                min_words,
            } => {
                let result = match (draft.as_deref(), srt.as_deref()) {
                    (Some(_), Some(_)) => bail!("draft and --srt are mutually exclusive"),
                    (None, None) => bail!("pass a draft directory or --srt <file>"),
                    (Some(draft), None) => media_analysis::retakes_from_draft(
                        draft,
                        track_name.as_deref(),
                        window,
                        similarity,
                        min_words,
                    )?,
                    (None, Some(srt)) => {
                        media_analysis::retakes(srt, window, similarity, min_words)?
                    }
                };
                print_output(result, json)
            }
            MediaOp::Transcribe(args) => run_media_transcribe(args, json)?,
            MediaOp::Tts(args) => {
                let MediaTtsArgs {
                    draft,
                    start,
                    duration,
                    text,
                    text_file,
                    provider,
                    tts_cmd,
                    endpoint,
                    executable,
                    provider_args,
                    format,
                    model,
                    voice,
                    language,
                    emotion,
                    ssml,
                    speed,
                    provider_volume,
                    pitch,
                    volume,
                    track_name,
                    credential_env,
                    task_id,
                    max_cost_microunits,
                    live_canary,
                    plan,
                    approval_id,
                    approval_root,
                    tts_state_root,
                    cloud_endpoint_override,
                    aliyun_family,
                    workspace_id,
                    app_id,
                    cluster,
                    resource_id,
                    uid,
                    cuid,
                    sample_rate,
                    streaming,
                    timestamps,
                } = *args;
                let content = match (text, text_file) {
                    (Some(text), None) => text,
                    (None, Some(path)) => std::fs::read_to_string(path)?,
                    (None, None) => bail!("pass --text or --text-file"),
                    (Some(_), Some(_)) => unreachable!("clap rejects conflicting text sources"),
                };
                match provider {
                    TtsProviderArg::LocalCommand
                        if endpoint.is_some()
                            || executable.is_some()
                            || !provider_args.is_empty() =>
                    {
                        bail!(
                            "local-command accepts --tts-cmd, not --endpoint, --executable or --provider-arg"
                        )
                    }
                    TtsProviderArg::SystemMacos | TtsProviderArg::SystemWindows
                        if tts_cmd.is_some()
                            || endpoint.is_some()
                            || executable.is_some()
                            || !provider_args.is_empty() =>
                    {
                        bail!("system TTS providers do not accept provider transport options")
                    }
                    TtsProviderArg::LocalHttp
                        if tts_cmd.is_some()
                            || executable.is_some()
                            || !provider_args.is_empty() =>
                    {
                        bail!("local-http accepts only --endpoint as its transport option")
                    }
                    TtsProviderArg::LocalProcess if tts_cmd.is_some() || endpoint.is_some() => {
                        bail!(
                            "local-process accepts --executable and --provider-arg, not --tts-cmd or --endpoint"
                        )
                    }
                    TtsProviderArg::EdgeTts
                        if tts_cmd.is_some() || endpoint.is_some() || !provider_args.is_empty() =>
                    {
                        bail!("edge-tts accepts only --executable as its transport option")
                    }
                    selected
                        if selected.is_cloud()
                            && (tts_cmd.is_some()
                                || endpoint.is_some()
                                || executable.is_some()
                                || !provider_args.is_empty()) =>
                    {
                        bail!("cloud TTS providers do not accept local transport options")
                    }
                    _ => {}
                }
                let format: TtsAudioFormat = format.map(Into::into).unwrap_or(match provider {
                    TtsProviderArg::SystemMacos => TtsAudioFormat::Aiff,
                    TtsProviderArg::EdgeTts | TtsProviderArg::Baidu | TtsProviderArg::Minimax => {
                        TtsAudioFormat::Mp3
                    }
                    _ => TtsAudioFormat::Wav,
                });
                let mut request = TtsRequest::new(content.trim(), format)?
                    .with_input_kind(if ssml {
                        TtsInputKind::Ssml
                    } else {
                        TtsInputKind::Text
                    })
                    .with_speed(speed)?
                    .with_volume(provider_volume)?
                    .with_pitch(pitch)?;
                if let Some(model) = model {
                    request = request.with_model(model);
                }
                if let Some(voice) = voice {
                    request = request.with_voice(voice);
                }
                if let Some(language) = language {
                    request = request.with_language(language);
                }
                if let Some(emotion) = emotion {
                    request = request.with_emotion(emotion);
                }
                if let Some(sample_rate) = sample_rate {
                    request = request.with_sample_rate_hz(sample_rate)?;
                }
                request = request
                    .with_streaming(streaming)
                    .with_timestamps(timestamps);
                let start_us = start.as_deref().map(tim::parse).transpose()?.unwrap_or(0);
                let duration_us = duration.as_deref().map(tim::parse).transpose()?;
                if provider.is_cloud() {
                    let credential_env = credential_env.as_deref().ok_or_else(|| {
                        anyhow::anyhow!(
                            "cloud TTS requires --credential-env with an environment variable name"
                        )
                    })?;
                    request = request.with_credential(CredentialRef::new(
                        CredentialSource::Environment,
                        credential_env,
                    )?);
                    let cloud_input = CloudTtsPlanInput {
                        provider,
                        request: &request,
                        draft: &draft,
                        task_id: task_id.as_deref().ok_or_else(|| {
                            anyhow::anyhow!("cloud TTS requires a stable --task-id")
                        })?,
                        max_cost_microunits: max_cost_microunits.ok_or_else(|| {
                            anyhow::anyhow!("cloud TTS requires --max-cost-microunits")
                        })?,
                        live_canary,
                        aliyun_family: aliyun_family.as_deref(),
                        workspace_id: workspace_id.as_deref(),
                        app_id: app_id.as_deref(),
                        cluster: cluster.as_deref(),
                        resource_id: resource_id.as_deref(),
                        uid: uid.as_deref(),
                        cuid: cuid.as_deref(),
                    };
                    if plan {
                        if approval_root.is_some()
                            || tts_state_root.is_some()
                            || cloud_endpoint_override.is_some()
                        {
                            bail!("cloud execution state options cannot be used with --plan");
                        }
                        print_output(plan_cloud_tts(cloud_input)?, json);
                        return Ok(());
                    }
                    let adapter = build_cloud_tts_adapter(cloud_input)?;
                    let submission = cloud_tts_submission(cloud_input, adapter.provider_id())?;
                    let executor = match cloud_endpoint_override {
                        Some(endpoint) => {
                            if std::env::var("JIANYING_ALLOW_TTS_ENDPOINT_OVERRIDE").as_deref()
                                != Ok("1")
                            {
                                bail!(
                                    "--cloud-endpoint-override requires JIANYING_ALLOW_TTS_ENDPOINT_OVERRIDE=1"
                                );
                            }
                            CloudTtsHttpExecutor::with_loopback_endpoint(
                                endpoint,
                                Duration::from_secs(30),
                            )?
                        }
                        None => CloudTtsHttpExecutor::new(Duration::from_secs(60)),
                    };
                    let approval_id = approval_id.as_deref().ok_or_else(|| {
                        anyhow::anyhow!(
                            "cloud TTS submission requires --approval-id; run the same command with --plan first"
                        )
                    })?;
                    executor.preflight(&adapter, &request)?;
                    let state_root = resolve_tts_state_root(tts_state_root);
                    let ledger = jianying_jobs::TtsLedgerStore::new(state_root.join("ledger"));
                    let approvals = jianying_jobs::ApprovalStore::new(resolve_approval_state_root(
                        approval_root,
                    ));
                    let decision =
                        ledger.prepare(&submission, &approvals, approval_id, epoch_seconds())?;
                    let (record, reused) = match decision {
                        jianying_jobs::TtsLedgerDecision::Reuse(record) => (record, true),
                        jianying_jobs::TtsLedgerDecision::Submit(_) => {
                            ledger.mark_running(submission.idempotency_key(), epoch_seconds())?;
                            let artifacts = state_root.join("artifacts");
                            let artifact_path = artifacts.join(format!(
                                "{}.{}",
                                submission.idempotency_key(),
                                cloud_audio_extension(request.format())
                            ));
                            let record = match executor.execute(&adapter, &request, &artifact_path)
                            {
                                Ok(artifact) => ledger.mark_succeeded(
                                    submission.idempotency_key(),
                                    artifact.path(),
                                    artifact.request_id(),
                                    epoch_seconds(),
                                )?,
                                Err(error) => {
                                    if error.is_ambiguous() {
                                        ledger.mark_ambiguous(
                                            submission.idempotency_key(),
                                            epoch_seconds(),
                                        )?;
                                    } else {
                                        ledger.mark_failed(
                                            submission.idempotency_key(),
                                            epoch_seconds(),
                                        )?;
                                    }
                                    return Err(error.into());
                                }
                            };
                            (record, false)
                        }
                    };
                    let artifact_path = record.artifact_path().ok_or_else(|| {
                        anyhow::anyhow!("succeeded TTS ledger record is missing its artifact path")
                    })?;
                    let mut result = media_ops::tts_existing_artifact(
                        &draft,
                        artifact_path,
                        adapter.provider_id(),
                        &request,
                        start_us,
                        duration_us,
                        track_name.as_deref(),
                        volume,
                    )?;
                    let object = result
                        .as_object_mut()
                        .context("cloud TTS result must be an object")?;
                    object.insert(
                        "idempotency_key".into(),
                        serde_json::json!(record.idempotency_key()),
                    );
                    object.insert("ledger_state".into(), serde_json::json!(record.state()));
                    object.insert("reused".into(), serde_json::json!(reused));
                    object.insert(
                        "external_request_id".into(),
                        serde_json::json!(record.external_request_id()),
                    );
                    print_output(result, json);
                    return Ok(());
                }
                if credential_env.is_some()
                    || task_id.is_some()
                    || max_cost_microunits.is_some()
                    || live_canary
                    || plan
                    || approval_id.is_some()
                    || approval_root.is_some()
                    || tts_state_root.is_some()
                    || cloud_endpoint_override.is_some()
                    || aliyun_family.is_some()
                    || workspace_id.is_some()
                    || app_id.is_some()
                    || cluster.is_some()
                    || resource_id.is_some()
                    || uid.is_some()
                    || cuid.is_some()
                {
                    bail!("cloud approval and provider configuration options require a cloud TTS provider");
                }
                let result = match provider {
                    TtsProviderArg::LocalCommand => {
                        let command = tts_cmd
                            .as_deref()
                            .ok_or_else(|| anyhow::anyhow!("local-command requires --tts-cmd"))?;
                        media_ops::tts_local_command(
                            &draft,
                            command,
                            &request,
                            start_us,
                            duration_us,
                            track_name.as_deref(),
                            volume,
                        )?
                    }
                    selected => {
                        if tts_cmd.is_some() {
                            bail!("--tts-cmd is valid only with --provider local-command");
                        }
                        let provider: Box<dyn TtsProvider> = match selected {
                            TtsProviderArg::SystemMacos => Box::new(MacOsSayTtsProvider::new()?),
                            TtsProviderArg::SystemWindows => {
                                Box::new(WindowsSystemTtsProvider::new()?)
                            }
                            TtsProviderArg::LocalHttp => Box::new(LocalHttpTtsProvider::new(
                                endpoint.ok_or_else(|| {
                                    anyhow::anyhow!("local-http requires --endpoint")
                                })?,
                                format,
                            )?),
                            TtsProviderArg::LocalProcess => Box::new(LocalProcessTtsProvider::new(
                                executable.ok_or_else(|| {
                                    anyhow::anyhow!("local-process requires --executable")
                                })?,
                                provider_args,
                                format,
                            )?),
                            TtsProviderArg::EdgeTts => {
                                Box::new(EdgeTtsProvider::new(executable.ok_or_else(|| {
                                    anyhow::anyhow!("edge-tts requires --executable")
                                })?)?)
                            }
                            TtsProviderArg::LocalCommand
                            | TtsProviderArg::XiaomiMimo
                            | TtsProviderArg::Volcengine
                            | TtsProviderArg::AliyunBailian
                            | TtsProviderArg::Baidu
                            | TtsProviderArg::TencentCloud
                            | TtsProviderArg::Minimax
                            | TtsProviderArg::ZhipuGlm => unreachable!(),
                        };
                        media_ops::tts_provider(
                            &draft,
                            provider.as_ref(),
                            &request,
                            start_us,
                            duration_us,
                            track_name.as_deref(),
                            volume,
                        )?
                    }
                };
                print_output(result, json)
            }
            MediaOp::Sfx {
                draft,
                slug,
                start,
                duration,
                volume,
                track_name,
            } => print_output(
                media_ops::add_sfx(
                    &draft,
                    &slug,
                    tim::parse(&start)?,
                    tim::parse(&duration)?,
                    track_name.as_deref(),
                    volume,
                )?,
                json,
            ),
            MediaOp::Status => print_domain_status("media", json)?,
        },
        Command::Captions { op } => match op {
            CaptionsOp::List { draft } => print_output(caption_ops::list(&draft)?, json),
            CaptionsOp::Get { draft, id } => print_output(caption_ops::get(&draft, &id)?, json),
            CaptionsOp::Add {
                draft,
                text,
                start,
                duration,
                track,
            } => print_output(
                caption_ops::add(
                    &draft,
                    &text,
                    tim::parse(&start)?,
                    tim::parse(&duration)?,
                    &track,
                )?,
                json,
            ),
            CaptionsOp::Set { draft, id, text } => {
                print_output(caption_ops::set_text(&draft, &id, &text)?, json)
            }
            CaptionsOp::ImportSrt {
                draft,
                file,
                track,
                offset,
                size,
                color,
                bold,
            } => print_output(
                caption_ops::import_srt(
                    &draft,
                    &file,
                    &track,
                    tim::parse(&offset)?,
                    size,
                    color.as_deref(),
                    bold,
                )?,
                json,
            ),
            CaptionsOp::ImportAss {
                draft,
                file,
                track,
                offset,
            } => print_output(
                caption_ops::import_ass(&draft, &file, &track, tim::parse(&offset)?)?,
                json,
            ),
            CaptionsOp::ExportSrt { draft, out } => {
                print_output(caption_ops::export_srt(&draft, &out)?, json)
            }
            CaptionsOp::ExportAss { draft, out } => {
                print_output(caption_ops::export_ass(&draft, &out)?, json)
            }
            CaptionsOp::Style {
                draft,
                id,
                size,
                color,
                bold,
                italic,
                underline,
                alignment,
            } => print_output(
                caption_ops::style(
                    &draft,
                    &id,
                    size,
                    color.as_deref(),
                    bold,
                    italic,
                    underline,
                    alignment,
                )?,
                json,
            ),
            CaptionsOp::StyleRanges { draft, id, styles } => {
                let raw = if let Some(path) = styles.strip_prefix('@') {
                    std::fs::read_to_string(path)
                        .with_context(|| format!("failed to read styles file: {path}"))?
                        .trim_start_matches('\u{feff}')
                        .to_owned()
                } else {
                    styles
                };
                let parsed: serde_json::Value =
                    serde_json::from_str(&raw).context("--styles is not valid JSON")?;
                let ranges = parsed
                    .as_array()
                    .context("--styles must be a JSON array of {start,end,...} ranges")?;
                print_output(caption_ops::style_ranges(&draft, &id, ranges)?, json)
            }
            CaptionsOp::Bubble {
                draft,
                id,
                bubble,
                effect_id,
                resource_id,
            } => print_output(
                caption_ops::bubble(
                    &draft,
                    &id,
                    bubble.as_deref(),
                    effect_id.as_deref(),
                    resource_id.as_deref(),
                )?,
                json,
            ),
            CaptionsOp::Animation {
                draft,
                id,
                intro,
                outro,
                intro_duration,
                outro_duration,
                jianying,
            } => print_output(
                caption_ops::animation(
                    &draft,
                    &id,
                    caption_ops::TextAnimationOptions {
                        intro: intro.as_deref(),
                        outro: outro.as_deref(),
                        intro_duration_us: intro_duration.as_deref().map(tim::parse).transpose()?,
                        outro_duration_us: outro_duration.as_deref().map(tim::parse).transpose()?,
                        jianying,
                    },
                )?,
                json,
            ),
            CaptionsOp::Translate {
                draft,
                translations,
                provider,
                to,
            } => {
                let output = match (translations, provider) {
                    (Some(path), None) => caption_ops::translate(&draft, &path)?,
                    (None, Some(executable)) => caption_ops::translate_with_command(
                        &draft,
                        &executable,
                        to.as_deref().unwrap_or_default(),
                    )?,
                    _ => unreachable!("clap enforces one translation source"),
                };
                print_output(output, json)
            }
            CaptionsOp::Status => print_domain_status("captions", json)?,
        },
        Command::Runtime { op } => run_runtime(op, json)?,
        Command::Doctor => print_output(redact_diagnostics(store::doctor()?), json),
        Command::Status { state_root } => run_status(state_root, json)?,
        Command::Audit {
            task_id,
            state_root,
        } => run_audit(task_id.as_deref(), state_root, json)?,
        Command::Completion { shell } => run_completion(shell, json)?,
        Command::Capabilities => {
            let mut manifest = capabilities::manifest()?;
            manifest["command_catalog"] = machine_command_catalog()?;
            print_output(manifest, json)
        }
        Command::Commands => print_output(machine_command_catalog()?, json),
        Command::Probe(args) => {
            deprecated_alias("jianying probe", "jianying media probe");
            run_probe(args, json)?;
        }
        Command::Build(args) => {
            deprecated_alias("jianying build", "jianying project build");
            run_build(args, json)?;
        }
        Command::Verify(args) => {
            deprecated_alias("jianying verify", "jianying project verify");
            run_verify(args, json)?;
        }
        Command::Inspect(args) => {
            deprecated_alias("jianying inspect", "jianying project inspect");
            run_inspect(args, json)?;
        }
        Command::Publish(args) => {
            deprecated_alias("jianying publish", "jianying store publish");
            run_publish(args, json)?;
        }
        Command::Render {
            op,
            draft,
            out,
            scale,
            burn_captions,
            crf,
        } => match op {
            Some(RenderOp::Proxy(args)) => run_render_proxy(args, json),
            Some(RenderOp::Native(args)) => run_render_native(args, json),
            Some(RenderOp::NativeTask(args)) => run_render_native_task(args, json),
            Some(RenderOp::Batch {
                manifest,
                out_dir,
                scale,
                burn_captions,
                crf,
                continue_on_error,
                overwrite,
            }) => {
                print_output(
                    render::render_batch(
                        &manifest,
                        &out_dir,
                        scale,
                        burn_captions,
                        crf,
                        continue_on_error,
                        overwrite,
                    )?,
                    json,
                );
                Ok(())
            }
            Some(RenderOp::Status) => print_domain_status("render", json),
            None => {
                deprecated_alias("jianying render <draft>", "jianying render proxy <draft>");
                let draft = draft.ok_or_else(|| {
                    anyhow::anyhow!("render requires a subcommand or legacy draft path")
                })?;
                run_render_proxy(
                    RenderProxyArgs {
                        draft,
                        out,
                        scale,
                        burn_captions,
                        crf,
                    },
                    json,
                )
            }
        }?,
        Command::Template { op } => match op {
            TemplateOp::List { root } => print_output(template::list_templates(&root)?, json),
            TemplateOp::Save {
                draft,
                name,
                root,
                description,
            } => print_output(
                template::save_template(&draft, &name, &root, description.as_deref())?,
                json,
            ),
            TemplateOp::Apply {
                template: template_path,
                new_name,
                root,
            } => print_output(
                template::apply_template(&template_path, &new_name, &root)?,
                json,
            ),
            TemplateOp::MakePreset { draft, name, root } => {
                print_output(template::make_preset(&draft, &name, &root)?, json)
            }
            TemplateOp::ApplyPreset { draft, preset } => {
                print_output(template::apply_preset(&draft, &preset)?, json)
            }
            TemplateOp::Inspect { draft } => {
                print_output(template::inspect_materials(&draft)?, json)
            }
            TemplateOp::Duplicate {
                draft,
                new_name,
                root,
            } => print_output(
                template::duplicate(&draft, &new_name, root.as_deref())?,
                json,
            ),
            TemplateOp::ReplaceText {
                draft,
                track,
                index,
                text,
            } => print_output(template::replace_text(&draft, &track, index, &text)?, json),
            TemplateOp::ReplaceMaterial {
                draft,
                source,
                name,
                track,
                index,
            } => print_output(
                template::replace_material(
                    &draft,
                    name.as_deref(),
                    track.as_deref(),
                    index,
                    &source,
                )?,
                json,
            ),
            TemplateOp::ImportTrack {
                draft,
                source,
                track,
                before,
            } => print_output(
                template::import_track_at(&draft, &source, &track, before.as_deref())?,
                json,
            ),
        },
        Command::Catalog(args) => {
            deprecated_alias("jianying catalog", "jianying media catalog");
            run_catalog(args, json)?;
        }
        Command::Store { op } => match op {
            StoreOp::Publish(args) => run_publish(args, json)?,
            StoreOp::Register(args) => run_publish(args, json)?,
            StoreOp::List { root } => {
                let root = store::resolve_root(root.as_deref())?;
                print_output(store::list(&root)?, json)
            }
            StoreOp::Has { name, root } => {
                let root = store::resolve_root(root.as_deref())?;
                print_output(store::has(&root, &name)?, json)
            }
            StoreOp::Remove { name, root, yes } => {
                if !yes {
                    bail!("refusing to remove without --yes");
                }
                let root = store::resolve_root(root.as_deref())?;
                print_output(store::remove(&root, &name)?, json)
            }
            StoreOp::Rename {
                name,
                new_name,
                root,
            } => {
                let root = store::resolve_root(root.as_deref())?;
                print_output(store::rename(&root, &name, &new_name)?, json)
            }
            StoreOp::Sync {
                draft,
                apply,
                force_newer,
            } => print_output(store::sync_timelines(&draft, apply, force_newer)?, json),
            StoreOp::Backup { draft, out } => print_output(store::backup(&draft, &out)?, json),
            StoreOp::Restore { backup, target } => {
                print_output(store::restore(&backup, &target)?, json)
            }
            StoreOp::Decrypt { input } => print_output(store::detect_encryption(&input)?, json),
            StoreOp::Directories => print_output(store::directories(), json),
            StoreOp::RestoreSnapshot { snapshot, target } => {
                print_output(store::restore(&snapshot, &target)?, json)
            }
        },
        Command::Job { op } => match op {
            JobOp::Run {
                job,
                out,
                state_root,
            } => {
                let root = job_runner::state_root(&job, state_root.as_deref());
                print_output(
                    job_runner::run_persisted(&job, out.as_deref(), &root)?,
                    json,
                )
            }
            JobOp::Batch {
                job,
                out,
                state_root,
            } => {
                let root = job_runner::state_root(&job, state_root.as_deref());
                print_output(
                    job_runner::run_persisted(&job, out.as_deref(), &root)?,
                    json,
                )
            }
            JobOp::List { state_root } => {
                let root = resolve_job_state_root(state_root);
                print_output(job_runner::list_persisted(&root)?, json)
            }
            JobOp::Show {
                task_id,
                state_root,
            } => {
                let root = resolve_job_state_root(state_root);
                print_output(job_runner::show_persisted(&task_id, &root)?, json)
            }
            JobOp::Cancel {
                task_id,
                state_root,
            } => {
                let root = resolve_job_state_root(state_root);
                print_output(job_runner::cancel_persisted(&task_id, &root)?, json)
            }
            JobOp::Retry {
                task_id,
                state_root,
            } => {
                let root = resolve_job_state_root(state_root);
                print_output(job_runner::retry_persisted(&task_id, &root)?, json)
            }
            JobOp::Audit {
                task_id,
                state_root,
            } => {
                let root = resolve_job_state_root(state_root);
                print_output(job_runner::audit_persisted(&task_id, &root)?, json)
            }
            JobOp::Maintenance { state_root } => {
                let root = resolve_job_state_root(state_root);
                let store = jianying_jobs::SqliteJobStore::new(job_runner::database_path(&root));
                let records = store.list()?;
                let failed = records
                    .iter()
                    .filter(|record| record.state == jianying_jobs::JobState::Failed)
                    .count();
                print_output(
                    serde_json::json!({"records":records.len(),"failed":failed,"database":store.path()}),
                    json,
                )
            }
            JobOp::Serve { state_root } => {
                let root = resolve_job_state_root(state_root);
                job_runner::serve(&root)?;
            }
        },
        Command::Approvals { op } => match op {
            ApprovalsOp::Grant {
                command,
                arguments,
                cwd,
                target,
                task_id,
                ttl_seconds,
                state_root,
            } => {
                let binding = approval_binding(command, arguments, cwd, target, task_id)?;
                let record = jianying_jobs::ApprovalRecord::grant(
                    format!("ap-{}", uuid::Uuid::new_v4().simple()),
                    binding,
                    ttl_seconds,
                    epoch_seconds(),
                )?;
                let store =
                    jianying_jobs::ApprovalStore::new(resolve_approval_state_root(state_root));
                store.save(&record)?;
                print_output(serde_json::to_value(record)?, json)
            }
            ApprovalsOp::Check {
                approval_id,
                command,
                arguments,
                cwd,
                target,
                task_id,
                state_root,
            } => {
                let binding = approval_binding(command, arguments, cwd, target, task_id)?;
                let store =
                    jianying_jobs::ApprovalStore::new(resolve_approval_state_root(state_root));
                print_output(
                    serde_json::to_value(store.consume(
                        &approval_id,
                        &binding,
                        epoch_seconds(),
                    )?)?,
                    json,
                )
            }
            ApprovalsOp::List { state_root } => {
                let store =
                    jianying_jobs::ApprovalStore::new(resolve_approval_state_root(state_root));
                print_output(serde_json::to_value(store.list()?)?, json)
            }
        },
        Command::Config { op } => {
            let read_only = host_read_only || host_read_only_from_env();
            let store = jianying_config::ConfigStore::new(config_root(), profile, read_only)?;
            match op {
                ConfigOp::File => print_output(
                    serde_json::json!({
                        "profile":profile,
                        "path":store.file(),
                        "exists":store.file().is_file(),
                        "read_only":read_only
                    }),
                    json,
                ),
                ConfigOp::Schema => print_output(jianying_config::schema(), json),
                ConfigOp::Validate => print_output(serde_json::to_value(store.validate()?)?, json),
                ConfigOp::Get { key } => print_output(store.get(key.as_deref())?, json),
                ConfigOp::Set { key, value_json } => {
                    let value = serde_json::from_str(&value_json)?;
                    print_output(serde_json::to_value(store.set(&key, value)?)?, json)
                }
                ConfigOp::Patch { patch_json } => {
                    let patch = serde_json::from_str(&patch_json)?;
                    print_output(serde_json::to_value(store.patch(patch)?)?, json)
                }
                ConfigOp::Unset { key } => {
                    print_output(serde_json::to_value(store.unset(&key)?)?, json)
                }
            }
        }
        Command::Mcp { op } => match op {
            McpOp::Serve {
                state_root,
                transport,
                bind,
                endpoint,
                token_env,
                allowed_host,
                allowed_origin,
            } => {
                let root = resolve_job_state_root(state_root);
                match transport {
                    McpTransport::Stdio => jianying_cli::mcp_server::serve(&root)?,
                    McpTransport::StreamableHttp | McpTransport::Sse => {
                        let bearer_token = std::env::var(&token_env)
                            .ok()
                            .filter(|token| !token.is_empty());
                        let response_mode = match transport {
                            McpTransport::StreamableHttp => {
                                jianying_cli::mcp_server::HttpResponseMode::JsonPreferred
                            }
                            McpTransport::Sse => jianying_cli::mcp_server::HttpResponseMode::Sse,
                            McpTransport::Stdio => unreachable!(),
                        };
                        jianying_cli::mcp_server::serve_http(
                            &root,
                            jianying_cli::mcp_server::HttpServeOptions {
                                bind,
                                endpoint,
                                bearer_token,
                                allowed_hosts: allowed_host,
                                allowed_origins: allowed_origin,
                                response_mode,
                            },
                        )?;
                    }
                }
            }
            McpOp::Tools => print_output(jianying_cli::mcp_server::tools()?, json),
        },
    }
    Ok(())
}

fn resolve_job_state_root(requested: Option<PathBuf>) -> PathBuf {
    requested
        .or_else(|| std::env::var_os("JIANYING_STATE_ROOT").map(PathBuf::from))
        .unwrap_or_else(|| PathBuf::from(".jianying-jobs"))
}

fn resolve_approval_state_root(requested: Option<PathBuf>) -> PathBuf {
    requested
        .or_else(|| std::env::var_os("JIANYING_APPROVAL_ROOT").map(PathBuf::from))
        .unwrap_or_else(|| PathBuf::from(".jianying-approvals"))
}

fn resolve_tts_state_root(requested: Option<PathBuf>) -> PathBuf {
    requested
        .or_else(|| std::env::var_os("JIANYING_TTS_STATE_ROOT").map(PathBuf::from))
        .unwrap_or_else(|| PathBuf::from(".jianying-tts"))
}

fn resolve_asr_state_root(requested: Option<PathBuf>) -> PathBuf {
    requested
        .or_else(|| std::env::var_os("JIANYING_ASR_STATE_ROOT").map(PathBuf::from))
        .unwrap_or_else(|| PathBuf::from(".jianying-asr"))
}

fn run_media_transcribe(args: MediaTranscribeArgs, json: bool) -> Result<()> {
    let provider = WhisperCppAsrProvider::new(&args.executable, &args.model, &args.model_id)?;
    let source_sha256 = AsrRequest::hash_source(&args.source)?;
    let mut request = AsrRequest::new(source_sha256, args.format.into())?
        .with_model(&args.model_id)
        .with_translate_to_english(args.translate);
    if let Some(language) = args.language.as_deref() {
        request = request.with_language(language);
    }
    if args.segment_timestamps {
        request = request.with_timestamps([AsrTimestampGranularity::Segment]);
    }
    provider.capability().validate(&request)?;

    let cwd = std::env::current_dir()?;
    let target = if args.out.is_absolute() {
        args.out.clone()
    } else {
        cwd.join(&args.out)
    };
    let submission = jianying_jobs::AsrSubmission::new(
        provider.capability().provider_id(),
        provider.capability().executor_identity(),
        &request,
        cwd,
        target.clone(),
        &args.task_id,
        false,
        0,
    )?;
    if args.plan {
        print_output(
            serde_json::json!({
                "state":"ready",
                "provider":provider.capability().provider_id(),
                "executor_identity":provider.capability().executor_identity(),
                "model_id":provider.model_id(),
                "source_sha256":request.source_sha256(),
                "output":target,
                "output_format":request.output_format(),
                "idempotency_key":submission.idempotency_key(),
                "paid":false,
                "network":false,
                "next":"rerun without --plan to execute the same local request"
            }),
            json,
        );
        return Ok(());
    }

    let ledger = jianying_jobs::AsrLedgerStore::new(resolve_asr_state_root(args.state_root));
    let decision = if args.retry {
        jianying_jobs::AsrLedgerDecision::Submit(ledger.retry_local(&submission, epoch_seconds())?)
    } else {
        ledger.prepare_local(&submission, epoch_seconds())?
    };
    let (record, reused) = match decision {
        jianying_jobs::AsrLedgerDecision::Reuse(record) => (record, true),
        jianying_jobs::AsrLedgerDecision::Submit(_) => {
            ledger.mark_running(submission.idempotency_key(), epoch_seconds())?;
            match provider.transcribe(&request, &args.source, &target) {
                Ok(_) => (
                    ledger.mark_succeeded(
                        submission.idempotency_key(),
                        &target,
                        None,
                        epoch_seconds(),
                    )?,
                    false,
                ),
                Err(error) => {
                    ledger.mark_failed(submission.idempotency_key(), epoch_seconds())?;
                    return Err(error.into());
                }
            }
        }
    };
    print_output(
        serde_json::json!({
            "state":record.state(),
            "provider":provider.capability().provider_id(),
            "executor_identity":provider.capability().executor_identity(),
            "idempotency_key":record.idempotency_key(),
            "attempts":record.attempts(),
            "reused":reused,
            "artifact":record.artifact_path(),
            "artifact_sha256":record.artifact_sha256(),
            "output_format":request.output_format()
        }),
        json,
    );
    Ok(())
}

fn cloud_audio_extension(format: TtsAudioFormat) -> &'static str {
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

fn config_root() -> PathBuf {
    std::env::var_os("JIANYING_CONFIG_ROOT")
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var_os("XDG_CONFIG_HOME")
                .map(PathBuf::from)
                .map(|root| root.join("jianying-cli"))
        })
        .or_else(|| {
            std::env::var_os("HOME")
                .map(PathBuf::from)
                .map(|root| root.join(".config/jianying-cli"))
        })
        .unwrap_or_else(|| PathBuf::from(".jianying-config"))
}

fn host_read_only_from_env() -> bool {
    std::env::var("JIANYING_HOST_READ_ONLY")
        .map(|value| value == "1" || value.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
}

fn approval_binding(
    command: String,
    arguments: Vec<String>,
    cwd: PathBuf,
    target: PathBuf,
    task_id: String,
) -> Result<jianying_jobs::ApprovalBinding> {
    Ok(jianying_jobs::ApprovalBinding::new(
        command, arguments, cwd, target, task_id,
    )?)
}

fn epoch_seconds() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn deprecated_alias(legacy: &str, replacement: &str) {
    eprintln!("warning: deprecated `{legacy}`; use `{replacement}`");
}

fn run_probe(args: ProbeArgs, json: bool) -> Result<()> {
    print_output(serde_json::to_value(probe::probe(&args.media)?)?, json);
    Ok(())
}

fn run_catalog(args: CatalogArgs, json: bool) -> Result<()> {
    catalog_query(
        args.domain.as_deref(),
        args.search.as_deref(),
        args.include_vip,
        json,
    )
}

fn run_enums(args: EnumArgs, json: bool) -> Result<()> {
    if args.human && json {
        bail!("--human and --json cannot be used together");
    }
    let namespace = args.namespace.as_str();
    let category = args.category.as_str();
    let mut entries = match args.category {
        EnumCategory::Bubbles => jianying_cli::catalogs::capcut_bubbles()
            .as_array()
            .cloned()
            .unwrap_or_default(),
        EnumCategory::Filters if matches!(args.namespace, EnumNamespace::Capcut) => {
            jianying_cli::catalogs::capcut_filters()
                .as_array()
                .cloned()
                .unwrap_or_default()
        }
        _ => jianying_cli::catalogs::capcut_enums()[namespace][category]
            .as_array()
            .cloned()
            .unwrap_or_default(),
    };
    if matches!(args.category, EnumCategory::Bubbles)
        || matches!(
            (args.category, args.namespace),
            (EnumCategory::Filters, EnumNamespace::Capcut)
        )
    {
        for entry in &mut entries {
            if entry["member"].is_null() {
                entry["member"] = entry["name"].clone();
            }
        }
    }
    if args.human {
        if entries.is_empty() {
            println!("No {category} in {namespace} namespace.");
            return Ok(());
        }
        println!("Slug                              Name                             Member");
        for entry in &entries {
            let slug = entry["slug"]
                .as_str()
                .filter(|value| !value.is_empty())
                .unwrap_or("(non-ascii)");
            let display = entry["name"]
                .as_str()
                .or_else(|| entry["title"].as_str())
                .unwrap_or_default();
            let member = entry["member"].as_str().unwrap_or_default();
            println!(
                "{slug:<33} {:<32} {member}",
                display.chars().take(32).collect::<String>()
            );
        }
        eprintln!("\n{} {category} ({namespace})", entries.len());
    } else {
        print_output(
            serde_json::json!({
                "namespace":namespace,
                "category":category,
                "count":entries.len(),
                "entries":entries
            }),
            json,
        );
    }
    Ok(())
}

fn run_harvest_enum_add(
    kind: &str,
    slug: &str,
    resource_id: &str,
    effect_id: Option<&str>,
    catalogue: Option<&Path>,
    apply: bool,
    json: bool,
) -> Result<()> {
    const WRITABLE_KINDS: &[&str] = &[
        "video_effects",
        "filters",
        "transitions",
        "masks",
        "audio_effects",
    ];
    if !WRITABLE_KINDS.contains(&kind) {
        bail!(
            "unknown or id-only kind {kind:?}; writable kinds: {}",
            WRITABLE_KINDS.join(", ")
        );
    }
    let normalized = ascii_slug(slug);
    if normalized.is_empty() || normalized != slug {
        bail!("slug {slug:?} must be canonical lowercase ASCII kebab-case; use {normalized:?}");
    }
    if resource_id.is_empty() {
        bail!("resource_id must not be empty");
    }
    let path = catalogue
        .map(Path::to_path_buf)
        .unwrap_or_else(default_user_enum_catalogue);
    let mut document = if path.exists() {
        let raw = std::fs::read_to_string(&path)
            .with_context(|| format!("failed to read user enum catalogue {}", path.display()))?;
        serde_json::from_str::<serde_json::Value>(raw.trim_start_matches('\u{feff}')).with_context(
            || format!("refusing to rewrite malformed catalogue {}", path.display()),
        )?
    } else {
        serde_json::json!({"version":1,"entries":[]})
    };
    if document["version"].as_u64() != Some(1) || !document["entries"].is_array() {
        bail!("refusing to rewrite catalogue without version=1 and entries[]");
    }
    let ids = [Some(resource_id), effect_id]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>();
    if let Some(existing) = document["entries"]
        .as_array()
        .unwrap()
        .iter()
        .find(|entry| {
            ids.iter().any(|id| {
                entry["effect_id"].as_str() == Some(id) || entry["resource_id"].as_str() == Some(id)
            })
        })
    {
        bail!(
            "resource id is already registered to {}/{}",
            existing["kind"].as_str().unwrap_or("unknown"),
            existing["slug"].as_str().unwrap_or("(id-only)")
        );
    }
    let mut entry = serde_json::json!({
        "kind":kind,"slug":slug,"name":slug,"resource_id":resource_id,
        "harvested_from":"manual"
    });
    if let Some(value) = effect_id {
        entry["effect_id"] = serde_json::json!(value);
    }
    if apply {
        let mut stored = entry.clone();
        stored["harvested_at"] = serde_json::json!(format!(
            "unix:{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .context("system clock precedes Unix epoch")?
                .as_secs()
        ));
        document["entries"].as_array_mut().unwrap().push(stored);
        write_user_enum_catalogue(&path, &document)?;
    }
    print_output(
        serde_json::json!({
            "ok":true,"applied":apply,"catalogue":path,"entry":entry,
            "added":if apply { 1 } else { 0 },
            "total":document["entries"].as_array().unwrap().len()
        }),
        json,
    );
    Ok(())
}

fn run_harvest_enum_scan(
    draft_path: &Path,
    catalogue: Option<&Path>,
    apply: bool,
    json: bool,
) -> Result<()> {
    let timeline = draft::load_timeline(draft_path)?;
    let path = catalogue
        .map(Path::to_path_buf)
        .unwrap_or_else(default_user_enum_catalogue);
    let mut document = load_user_enum_document(&path)?;
    let mut known_ids = known_enum_ids(&document);
    let source = timeline["name"]
        .as_str()
        .or_else(|| timeline["id"].as_str())
        .unwrap_or("unknown-draft");
    let (found, known, candidates) = harvest_enum_candidates(&timeline, &known_ids, source);
    for candidate in &candidates {
        insert_candidate_ids(candidate, &mut known_ids);
    }
    let merge = merge_user_enum_candidates(&mut document, &candidates, apply)?;
    if apply && merge.0 > 0 {
        write_user_enum_catalogue(&path, &document)?;
    }
    let writable = candidates
        .iter()
        .filter(|entry| entry["slug"].as_str().is_some_and(|slug| !slug.is_empty()))
        .count();
    print_output(
        serde_json::json!({"ok":true,"applied":apply,"catalogue":path,
            "found":found,"known":known,"new":candidates,
            "writable_slugs":writable,"id_only":candidates.len()-writable,
            "added":merge.0,"duplicates":merge.1,"total":merge.2}),
        json,
    );
    Ok(())
}

fn run_harvest_enum_sync(
    drafts: Option<&Path>,
    catalogue: Option<&Path>,
    apply: bool,
    json: bool,
) -> Result<()> {
    let path = catalogue
        .map(Path::to_path_buf)
        .unwrap_or_else(default_user_enum_catalogue);
    let mut document = load_user_enum_document(&path)?;
    let mut known_ids = known_enum_ids(&document);
    let roots: Vec<PathBuf> = drafts
        .map(|path| vec![path.to_path_buf()])
        .unwrap_or_else(|| {
            store::draft_root_candidates()
                .into_iter()
                .map(|(_, path)| path)
                .collect()
        });
    let mut folders = Vec::new();
    for root in &roots {
        if !root.is_dir() {
            continue;
        }
        let mut children: Vec<PathBuf> = std::fs::read_dir(root)?
            .filter_map(|entry| entry.ok().map(|entry| entry.path()))
            .filter(|path| path.is_dir())
            .collect();
        children.sort();
        folders.extend(children);
    }
    let mut scanned = 0usize;
    let mut found = 0usize;
    let mut known = 0usize;
    let mut skipped = Vec::new();
    let mut candidates = Vec::new();
    for folder in folders {
        if !folder.join("draft_content.json").is_file() && !folder.join("draft_info.json").is_file()
        {
            continue;
        }
        match draft::load_timeline(&folder) {
            Ok(timeline) => {
                let source = timeline["name"]
                    .as_str()
                    .or_else(|| timeline["id"].as_str())
                    .or_else(|| folder.file_name().and_then(|name| name.to_str()))
                    .unwrap_or("unknown-draft");
                let result = harvest_enum_candidates(&timeline, &known_ids, source);
                found += result.0;
                known += result.1;
                for candidate in result.2 {
                    insert_candidate_ids(&candidate, &mut known_ids);
                    candidates.push(candidate);
                }
                scanned += 1;
            }
            Err(error) => skipped.push(serde_json::json!({
                "draft":folder.file_name().and_then(|name| name.to_str()).unwrap_or_default(),
                "reason":error.to_string()
            })),
        }
    }
    let merge = merge_user_enum_candidates(&mut document, &candidates, apply)?;
    if apply && merge.0 > 0 {
        write_user_enum_catalogue(&path, &document)?;
    }
    let mut by_kind = serde_json::Map::new();
    for candidate in &candidates {
        let kind = candidate["kind"].as_str().unwrap_or("unknown");
        let count = by_kind
            .get(kind)
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(0)
            + 1;
        by_kind.insert(kind.to_owned(), serde_json::json!(count));
    }
    let writable = candidates
        .iter()
        .filter(|entry| entry["slug"].as_str().is_some_and(|slug| !slug.is_empty()))
        .count();
    print_output(
        serde_json::json!({"ok":true,"applied":apply,"catalogue":path,
        "drafts_scanned":scanned,"drafts_skipped":skipped,"found":found,"known":known,
        "new":candidates,"new_by_kind":by_kind,"writable_slugs":writable,
        "id_only":candidates.len()-writable,"added":merge.0,"duplicates":merge.1,"total":merge.2}),
        json,
    );
    Ok(())
}

fn load_user_enum_document(path: &Path) -> Result<serde_json::Value> {
    if !path.exists() {
        return Ok(serde_json::json!({"version":1,"entries":[]}));
    }
    let raw = std::fs::read_to_string(path)?;
    let value: serde_json::Value = serde_json::from_str(raw.trim_start_matches('\u{feff}'))
        .with_context(|| format!("refusing to rewrite malformed catalogue {}", path.display()))?;
    if value["version"].as_u64() != Some(1) || !value["entries"].is_array() {
        bail!("refusing to rewrite catalogue without version=1 and entries[]");
    }
    Ok(value)
}

fn known_enum_ids(document: &serde_json::Value) -> std::collections::HashSet<String> {
    fn collect(value: &serde_json::Value, output: &mut std::collections::HashSet<String>) {
        match value {
            serde_json::Value::Array(items) => items.iter().for_each(|item| collect(item, output)),
            serde_json::Value::Object(object) => {
                for key in ["effect_id", "resource_id"] {
                    if let Some(id) = object
                        .get(key)
                        .and_then(serde_json::Value::as_str)
                        .filter(|id| !id.is_empty())
                    {
                        output.insert(id.to_owned());
                    }
                }
                object.values().for_each(|item| collect(item, output));
            }
            _ => {}
        }
    }
    let mut output = std::collections::HashSet::new();
    collect(jianying_cli::catalogs::capcut_enums(), &mut output);
    for catalogue in [
        jianying_cli::catalogs::capcut_filters(),
        jianying_cli::catalogs::capcut_bubbles(),
    ] {
        collect(catalogue, &mut output);
    }
    collect(document, &mut output);
    output
}

fn harvest_enum_candidates(
    timeline: &serde_json::Value,
    known_ids: &std::collections::HashSet<String>,
    source: &str,
) -> (usize, usize, Vec<serde_json::Value>) {
    let mut found = 0usize;
    let mut known = 0usize;
    let mut seen = std::collections::HashSet::new();
    let mut output = Vec::new();
    let materials = &timeline["materials"];
    for (bucket, kind, slugless) in [
        ("video_effects", "video_effects", false),
        ("transitions", "transitions", false),
        ("audio_effects", "audio_effects", false),
        ("masks", "masks", false),
        ("common_mask", "masks", false),
        ("common_masks", "masks", false),
    ] {
        for raw in materials[bucket].as_array().into_iter().flatten() {
            consider_harvest_entry(
                raw,
                kind,
                slugless,
                source,
                known_ids,
                &mut seen,
                &mut found,
                &mut known,
                &mut output,
            );
        }
    }
    for raw in materials["filters"].as_array().into_iter().flatten() {
        let bubble = raw["type"].as_str() == Some("text_shape");
        consider_harvest_entry(
            raw,
            if bubble { "bubbles" } else { "filters" },
            bubble,
            source,
            known_ids,
            &mut seen,
            &mut found,
            &mut known,
            &mut output,
        );
    }
    for container in materials["material_animations"]
        .as_array()
        .into_iter()
        .flatten()
    {
        for animation in container["animations"].as_array().into_iter().flatten() {
            let mut raw = animation.clone();
            if raw["effect_id"].is_null() {
                raw["effect_id"] = raw["id"].clone();
            }
            consider_harvest_entry(
                &raw,
                "animations",
                true,
                source,
                known_ids,
                &mut seen,
                &mut found,
                &mut known,
                &mut output,
            );
        }
    }
    for text in materials["texts"].as_array().into_iter().flatten() {
        for key in ["font_id", "font_resource_id"] {
            if let Some(id) = text[key].as_str().filter(|id| !id.is_empty()) {
                let raw = serde_json::json!({"resource_id":id});
                consider_harvest_entry(
                    &raw,
                    "fonts",
                    true,
                    source,
                    known_ids,
                    &mut seen,
                    &mut found,
                    &mut known,
                    &mut output,
                );
            }
        }
    }
    (found, known, output)
}

#[allow(clippy::too_many_arguments)]
fn consider_harvest_entry(
    raw: &serde_json::Value,
    kind: &str,
    slugless: bool,
    source: &str,
    known_ids: &std::collections::HashSet<String>,
    seen: &mut std::collections::HashSet<String>,
    found: &mut usize,
    known: &mut usize,
    output: &mut Vec<serde_json::Value>,
) {
    let effect = raw["effect_id"].as_str().unwrap_or_default();
    let resource = raw["resource_id"].as_str().unwrap_or_default();
    if effect.is_empty() && resource.is_empty() {
        return;
    }
    *found += 1;
    if known_ids.contains(effect) || known_ids.contains(resource) {
        *known += 1;
        return;
    }
    if !seen.insert(format!("{effect}|{resource}")) {
        return;
    }
    let name = raw["name"].as_str().unwrap_or_default();
    let mut entry = serde_json::json!({"kind":kind,"slug":if slugless { String::new() } else { ascii_slug(name) },"name":name,"harvested_from":source});
    if !effect.is_empty() {
        entry["effect_id"] = serde_json::json!(effect);
    }
    if !resource.is_empty() {
        entry["resource_id"] = serde_json::json!(resource);
    }
    if let Some(value) = raw["resource_type"]
        .as_str()
        .filter(|value| !value.is_empty())
    {
        entry["resource_type"] = serde_json::json!(value);
    }
    output.push(entry);
}

fn insert_candidate_ids(entry: &serde_json::Value, ids: &mut std::collections::HashSet<String>) {
    for key in ["effect_id", "resource_id"] {
        if let Some(value) = entry[key].as_str().filter(|value| !value.is_empty()) {
            ids.insert(value.to_owned());
        }
    }
}

fn merge_user_enum_candidates(
    document: &mut serde_json::Value,
    candidates: &[serde_json::Value],
    apply: bool,
) -> Result<(usize, usize, usize)> {
    let entries = document["entries"]
        .as_array_mut()
        .context("user enum entries must be an array")?;
    let mut existing = std::collections::HashSet::new();
    for entry in entries.iter() {
        existing.insert(format!(
            "{}|{}",
            entry["effect_id"].as_str().unwrap_or_default(),
            entry["resource_id"].as_str().unwrap_or_default()
        ));
    }
    let mut added = 0;
    let mut duplicates = 0;
    for candidate in candidates {
        let key = format!(
            "{}|{}",
            candidate["effect_id"].as_str().unwrap_or_default(),
            candidate["resource_id"].as_str().unwrap_or_default()
        );
        if !existing.insert(key) {
            duplicates += 1;
            continue;
        }
        if apply {
            let mut stored = candidate.clone();
            stored["harvested_at"] = serde_json::json!(format!(
                "unix:{}",
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)?
                    .as_secs()
            ));
            entries.push(stored);
        }
        added += 1;
    }
    Ok((
        if apply { added } else { 0 },
        duplicates,
        entries.len() + if apply { 0 } else { added },
    ))
}

fn ascii_slug(value: &str) -> String {
    let mut output = String::new();
    let mut separator = false;
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() {
            if separator && !output.is_empty() {
                output.push('-');
            }
            output.push((byte as char).to_ascii_lowercase());
            separator = false;
        } else {
            separator = true;
        }
    }
    output
}

fn default_user_enum_catalogue() -> PathBuf {
    if let Some(path) = std::env::var_os("JIANYING_CLI_USER_ENUMS") {
        return PathBuf::from(path);
    }
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))
        .unwrap_or_else(|| PathBuf::from("."));
    base.join("jianying-cli/user-enums.json")
}

fn write_user_enum_catalogue(path: &Path, document: &serde_json::Value) -> Result<()> {
    let parent = path
        .parent()
        .context("user enum catalogue requires a parent directory")?;
    std::fs::create_dir_all(parent)?;
    let temporary = parent.join(format!(
        ".{}.{}.tmp",
        path.file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("user-enums"),
        uuid::Uuid::new_v4().simple()
    ));
    let bytes = serde_json::to_vec_pretty(document)?;
    std::fs::write(&temporary, [&bytes[..], b"\n"].concat())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&temporary, std::fs::Permissions::from_mode(0o600))?;
    }
    std::fs::rename(&temporary, path)
        .with_context(|| format!("failed to atomically replace {}", path.display()))?;
    Ok(())
}

fn run_verify(args: DraftPathArgs, json: bool) -> Result<()> {
    print_output(draft::verify(&args.draft)?, json);
    Ok(())
}

fn run_inspect(args: DraftPathArgs, json: bool) -> Result<()> {
    print_output(draft::inspect(&args.draft)?, json);
    Ok(())
}

fn run_publish(args: PublishArgs, json: bool) -> Result<()> {
    let root = store::resolve_root(args.root.as_deref())?;
    print_output(store::publish(&args.draft, &root, args.force)?, json);
    Ok(())
}

fn run_build(args: BuildArgs, json: bool) -> Result<()> {
    let mut plan = plan::Plan::load(&args.plan)?;
    if let Some(srt_path) = args.srt {
        let cues = srt::parse(&std::fs::read_to_string(&srt_path)?)?;
        let opts = srt::SrtOptions {
            offset_us: args
                .srt_offset
                .as_deref()
                .map(jianying_cli::tim::parse)
                .transpose()?
                .unwrap_or(0),
            size: args.srt_size,
            align: args.srt_align,
            color: args.srt_color,
            border_width: args.srt_border,
            y: args.srt_y,
            ..Default::default()
        };
        plan.tracks.push(srt::cues_to_text_track(cues, &opts));
        plan.validate()?;
    }
    let plan_dir = plan.parent.clone().unwrap_or_else(|| PathBuf::from("."));
    let report = draft::build(&plan, &plan_dir, &args.out, args.seed.as_deref(), &|path| {
        probe::probe(path)
    })?;
    if let Some(template_path) = args.template {
        let output = Path::new(&report.out_dir);
        let mut timeline: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(output.join("draft_content.json"))?)?;
        template::build_on_template(&template_path, output, &mut timeline, &report.name)?;
        template::save_timeline(output, &timeline)?;
    }
    let verdict = draft::verify(Path::new(&report.out_dir))?;
    if verdict["ok"] != serde_json::json!(true) {
        bail!(
            "built draft failed self-verification: {}",
            verdict["issues"]
        );
    }
    print_output(serde_json::to_value(report)?, json);
    Ok(())
}

fn run_project_init(args: ProjectInitArgs, json: bool) -> Result<()> {
    let plan: plan::Plan = serde_json::from_value(serde_json::json!({
        "schema": plan::SCHEMA,
        "name": args.name,
        "canvas": {"width":args.width,"height":args.height,"fps":args.fps},
        "tracks": [{"type":"text","name":"text","segments":[]}]
    }))?;
    let report = draft::build(
        &plan,
        Path::new("."),
        &args.out,
        args.seed.as_deref(),
        &|path| probe::probe(path),
    )?;
    draft::validate_bundle(&args.out)?;
    print_output(
        serde_json::json!({
            "ok":true,
            "name":report.name,
            "draft_path":report.out_dir,
            "file_path":args.out.join("draft_content.json"),
            "registered":false,
            "canvas":{"width":args.width,"height":args.height,"fps":args.fps},
            "seed_donor":report.seed_donor
        }),
        json,
    );
    Ok(())
}

fn run_project_quickstart(args: ProjectQuickstartArgs, json: bool) -> Result<()> {
    if args.video.is_none() && args.audio.is_none() && args.srt.is_none() {
        bail!("quickstart needs at least one input: --video, --audio, or --srt");
    }
    let mut tracks = Vec::new();
    if let Some(video) = &args.video {
        let measured = probe::probe(video)?;
        tracks.push(serde_json::json!({
            "type":"video","name":"video","segments":[{
                "start_us":0,"duration_us":measured.duration_us,"source":video,
                "photo":measured.is_image
            }]
        }));
    }
    if let Some(audio) = &args.audio {
        let measured = probe::probe(audio)?;
        tracks.push(serde_json::json!({
            "type":"audio","name":"audio","segments":[{
                "start_us":0,"duration_us":measured.duration_us,"source":audio
            }]
        }));
    }
    let mut plan: plan::Plan = serde_json::from_value(serde_json::json!({
        "schema": plan::SCHEMA,
        "name": args.init.name,
        "canvas": {"width":args.init.width,"height":args.init.height,"fps":args.init.fps},
        "tracks": tracks
    }))?;
    let mut captions = 0usize;
    if let Some(srt_path) = &args.srt {
        let cues = srt::parse(&std::fs::read_to_string(srt_path)?)?;
        if cues.is_empty() {
            bail!("SRT produced 0 cues: {}", srt_path.display());
        }
        captions = cues.len();
        plan.tracks
            .push(srt::cues_to_text_track(cues, &srt::SrtOptions::default()));
    }
    plan.validate()?;
    let report = draft::build(
        &plan,
        Path::new("."),
        &args.init.out,
        args.init.seed.as_deref(),
        &|path| probe::probe(path),
    )?;
    let verdict = draft::verify(&args.init.out)?;
    let clean = verdict["ok"] == serde_json::json!(true);
    print_output(
        serde_json::json!({
            "ok":clean,
            "name":report.name,
            "draft_path":report.out_dir,
            "file_path":args.init.out.join("draft_content.json"),
            "registered":false,
            "canvas":{"width":args.init.width,"height":args.init.height,"fps":args.init.fps},
            "added":{"video":args.video.is_some(),"audio":args.audio.is_some(),"captions":captions},
            "lint":{"errors":verdict["issues"].as_array().map(Vec::len).unwrap_or(0)},
            "open_hint":[format!("Open or publish {} in JianYing/CapCut", args.init.out.display())]
        }),
        json,
    );
    Ok(())
}

fn run_status(state_root: Option<PathBuf>, json: bool) -> Result<()> {
    let root = resolve_job_state_root(state_root);
    let jobs = jianying_jobs::SqliteJobStore::new(job_runner::database_path(&root)).list()?;
    let failed = jobs
        .iter()
        .filter(|record| record.state == jianying_jobs::JobState::Failed)
        .count();
    let running = jobs
        .iter()
        .filter(|record| record.state == jianying_jobs::JobState::Running)
        .count();
    let manifest = capabilities::manifest()?;
    let capabilities = manifest["capabilities"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    let supported = capabilities
        .iter()
        .filter(|capability| capability["status"] == "supported")
        .count();
    let doctor = store::doctor()?;
    let status = if failed == 0 && doctor["ffprobe"].is_string() {
        "ready"
    } else {
        "degraded"
    };
    print_output(
        redact_diagnostics(serde_json::json!({
            "status": status,
            "version": env!("CARGO_PKG_VERSION"),
            "doctor": doctor,
            "capabilities": {
                "total": capabilities.len(),
                "supported": supported,
                "other": capabilities.len().saturating_sub(supported)
            },
            "jobs": {
                "total": jobs.len(),
                "running": running,
                "failed": failed,
                "database": job_runner::database_path(&root)
            },
            "security": {
                "remote_auth": std::env::var_os("JIANYING_MCP_TOKEN").map(|_| "configured").unwrap_or("missing")
            }
        })),
        json,
    );
    Ok(())
}

fn run_audit(task_id: Option<&str>, state_root: Option<PathBuf>, json: bool) -> Result<()> {
    let root = resolve_job_state_root(state_root);
    let store = jianying_jobs::SqliteJobStore::new(job_runner::database_path(&root));
    let value = if let Some(task_id) = task_id {
        let record = store.load(task_id)?;
        serde_json::json!({"task_id":task_id,"history":record.history})
    } else {
        let records = store.list()?;
        let records: Vec<serde_json::Value> = records
            .iter()
            .map(|record| {
                serde_json::json!({
                    "task_id": record.task_id,
                    "state": record.state,
                    "revision": record.revision,
                    "attempts": record.attempts,
                    "events": record.history.len()
                })
            })
            .collect();
        serde_json::json!({
            "database": store.path(),
            "records": records,
            "append_only": true
        })
    };
    print_output(redact_diagnostics(value), json);
    Ok(())
}

fn run_completion(shell: Shell, json: bool) -> Result<()> {
    let mut command = Cli::command();
    let mut script = Vec::new();
    clap_complete::generate(shell, &mut command, "jianying", &mut script);
    let script = String::from_utf8(script)?;
    if json {
        print_output(
            serde_json::json!({"shell":shell.to_string(),"script":script}),
            true,
        );
    } else {
        print!("{script}");
    }
    Ok(())
}

fn redact_diagnostics(mut value: serde_json::Value) -> serde_json::Value {
    redact_value(None, &mut value);
    value
}

fn redact_value(parent_key: Option<&str>, value: &mut serde_json::Value) {
    match value {
        serde_json::Value::Object(object) => {
            for (key, child) in object {
                if is_credential_key(key) {
                    *child = serde_json::Value::String("[REDACTED]".to_owned());
                } else {
                    redact_value(Some(key), child);
                }
            }
        }
        serde_json::Value::Array(items) => {
            for item in items {
                redact_value(parent_key, item);
            }
        }
        serde_json::Value::String(text) => {
            if parent_key.is_some_and(is_path_key) {
                *text = redact_path_text(text);
            } else if let Some(home) =
                std::env::var_os("HOME").and_then(|home| home.into_string().ok())
            {
                if !home.is_empty() {
                    *text = text.replace(&home, "$HOME");
                }
            }
        }
        _ => {}
    }
}

fn is_credential_key(key: &str) -> bool {
    let key = key.to_ascii_lowercase().replace(['-', '_'], "");
    [
        "token",
        "secret",
        "password",
        "credential",
        "authorization",
        "apikey",
    ]
    .iter()
    .any(|needle| key.contains(needle))
        && key != "configured"
}

fn is_path_key(key: &str) -> bool {
    let key = key.to_ascii_lowercase();
    key.contains("path") || key.contains("root") || key == "database" || key == "cwd"
}

fn redact_path_text(text: &str) -> String {
    if let Some(home) = std::env::var_os("HOME").and_then(|home| home.into_string().ok()) {
        if !home.is_empty() && text.starts_with(&home) {
            return text.replacen(&home, "$HOME", 1);
        }
    }
    // 诊断可能包含另一平台生成的路径，不能只依赖当前宿主的 Path 语义。
    let has_windows_drive = text.as_bytes().get(1) == Some(&b':')
        && text
            .as_bytes()
            .get(2)
            .is_some_and(|separator| matches!(separator, b'/' | b'\\'));
    let is_cross_platform_absolute = text.starts_with('/')
        || text.starts_with('\\')
        || text.starts_with("//")
        || has_windows_drive;
    if is_cross_platform_absolute {
        let name = text
            .rsplit(['/', '\\'])
            .find(|component| !component.is_empty())
            .unwrap_or_default();
        return format!("<redacted-path>/{name}");
    }
    text.to_owned()
}

fn machine_command_catalog() -> Result<serde_json::Value> {
    let capability_manifest = capabilities::manifest()?;
    let mut commands = Vec::new();
    collect_commands(&Cli::command(), &[], &capability_manifest, &mut commands);
    commands.sort_by(|left, right| {
        left["path"]
            .as_str()
            .unwrap_or_default()
            .cmp(right["path"].as_str().unwrap_or_default())
    });
    Ok(serde_json::json!({
        "schema": "jianying-command-catalog/v1",
        "cli_version": env!("CARGO_PKG_VERSION"),
        "global_options": {
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "json": {"type":"boolean","default":false},
                "profile": {"type":"string","default":"default"},
                "no-color": {"type":"boolean","default":false},
                "log-level": {"type":"string","enum":["error","warn","info","debug","trace"],"default":"info"},
                "host-read-only": {"type":"boolean","default":false}
            }
        },
        "commands": commands,
        "output_schemas": {
            "success": {"$id":"jianying-output-success/v1","type":"object","required":["ok","data"]},
            "failure": {"$id":"jianying-output-failure/v1","type":"object","required":["ok","error"]}
        }
    }))
}

fn collect_commands(
    command: &clap::Command,
    parent: &[String],
    capability_manifest: &serde_json::Value,
    output: &mut Vec<serde_json::Value>,
) {
    for subcommand in command.get_subcommands() {
        if subcommand.get_name() == "help" {
            continue;
        }
        let mut path = parent.to_vec();
        path.push(subcommand.get_name().to_owned());
        let has_children = subcommand.get_subcommands().next().is_some();
        let has_arguments = subcommand
            .get_arguments()
            .any(|argument| argument.get_id().as_str() != "help");
        if !has_children || has_arguments {
            output.push(command_entry(subcommand, &path, capability_manifest));
        }
        collect_commands(subcommand, &path, capability_manifest, output);
    }
}

fn command_entry(
    command: &clap::Command,
    path: &[String],
    capability_manifest: &serde_json::Value,
) -> serde_json::Value {
    let command_path = path.join(" ");
    let group = command_group(path);
    let mut properties = serde_json::Map::new();
    let mut required = Vec::new();
    for argument in command.get_arguments() {
        let id = argument.get_id().as_str();
        if matches!(id, "help" | "version") {
            continue;
        }
        let name = argument.get_long().unwrap_or(id).replace('_', "-");
        let mut schema = serde_json::Map::new();
        let schema_type = match argument.get_action() {
            clap::ArgAction::SetTrue | clap::ArgAction::SetFalse => "boolean",
            clap::ArgAction::Append | clap::ArgAction::Count => "array",
            _ => "string",
        };
        schema.insert("type".to_owned(), serde_json::json!(schema_type));
        if let Some(help) = argument.get_help() {
            schema.insert(
                "description".to_owned(),
                serde_json::json!(help.to_string()),
            );
        }
        let defaults: Vec<String> = argument
            .get_default_values()
            .iter()
            .map(|value| value.to_string_lossy().into_owned())
            .collect();
        if defaults.len() == 1 {
            schema.insert("default".to_owned(), serde_json::json!(defaults[0]));
        } else if !defaults.is_empty() {
            schema.insert("default".to_owned(), serde_json::json!(defaults));
        }
        if argument.is_required_set() {
            required.push(name.clone());
        }
        properties.insert(name, serde_json::Value::Object(schema));
    }
    let status = command_status(&group, capability_manifest);
    let replacement = legacy_replacement(&command_path);
    serde_json::json!({
        "path": command_path,
        "group": group,
        "summary": command.get_about().map(ToString::to_string).unwrap_or_default(),
        "deprecated": replacement.is_some(),
        "replacement": replacement,
        "input_schema": {
            "type": "object",
            "additionalProperties": false,
            "properties": properties,
            "required": required
        },
        "access": command_access(&command_path),
        "platforms": command_platforms(&group),
        "capability_prefix": format!("{group}."),
        "status": status,
        "output_schema": {"$ref":"#/output_schemas/success"},
        "error_schema": {"$ref":"#/output_schemas/failure"}
    })
}

fn legacy_replacement(path: &str) -> Option<&'static str> {
    match path {
        "build" => Some("project build"),
        "verify" => Some("project verify"),
        "inspect" => Some("project inspect"),
        "probe" => Some("media probe"),
        "catalog" => Some("media catalog"),
        "publish" => Some("store publish"),
        "render" => Some("render proxy"),
        _ => None,
    }
}

fn command_group(path: &[String]) -> String {
    match path.first().map(String::as_str).unwrap_or_default() {
        "build" | "verify" | "inspect" => "project",
        "probe" | "catalog" => "media",
        "publish" => "store",
        group => group,
    }
    .to_owned()
}

fn command_status(group: &str, manifest: &serde_json::Value) -> &'static str {
    let prefix = format!("{group}.");
    let statuses: Vec<&str> = manifest["capabilities"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|capability| {
            capability["id"]
                .as_str()
                .filter(|id| id.starts_with(&prefix))
                .and_then(|_| capability["status"].as_str())
        })
        .collect();
    if statuses.is_empty() || statuses.iter().all(|status| *status == "supported") {
        "supported"
    } else if statuses.contains(&"partial") {
        "partial"
    } else {
        "external_dependency"
    }
}

fn command_access(path: &str) -> &'static str {
    let leaf = path.split_whitespace().last().unwrap_or_default();
    if matches!(leaf, "serve") || path == "mcp serve" {
        "service"
    } else if matches!(
        leaf,
        "build"
            | "publish"
            | "remove"
            | "duplicate"
            | "replace-text"
            | "replace-material"
            | "import-track"
            | "add"
            | "import-srt"
            | "import-ass"
            | "export-srt"
            | "export-ass"
            | "style"
            | "style-ranges"
            | "harvest-enums"
            | "diagnose"
            | "fixture"
            | "compile"
            | "translate"
            | "save"
            | "apply"
            | "make-preset"
            | "apply-preset"
            | "register"
            | "rename"
            | "sync"
            | "backup"
            | "restore"
            | "restore-snapshot"
            | "batch"
            | "run"
            | "cancel"
            | "retry"
            | "grant"
            | "check"
            | "set"
            | "patch"
            | "unset"
            | "proxy"
            | "native"
            | "start"
            | "stop"
            | "prune"
            | "add-cover"
            | "cut"
            | "matting"
            | "chroma"
            | "mask"
            | "bg-blur"
            | "audio-fade"
            | "add-filter"
            | "bubble"
            | "add-effect"
            | "crop"
            | "keyframe"
            | "transition"
            | "image-animation"
            | "animation"
    ) || path == "render"
    {
        "write"
    } else {
        "read"
    }
}

fn command_platforms(group: &str) -> Vec<&'static str> {
    if group == "runtime" {
        vec!["macos", "windows"]
    } else {
        vec!["macos", "windows", "linux"]
    }
}

fn print_domain_status(group: &str, json: bool) -> Result<()> {
    let manifest = capabilities::manifest()?;
    let prefix = format!("{group}.");
    let capabilities: Vec<serde_json::Value> = manifest["capabilities"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|capability| {
            capability["id"]
                .as_str()
                .is_some_and(|id| id.starts_with(&prefix))
        })
        .cloned()
        .collect();
    let status = if capabilities.is_empty() {
        "skeleton"
    } else if capabilities
        .iter()
        .all(|capability| capability["status"] == "supported")
    {
        "supported"
    } else {
        "partial"
    };
    print_output(
        serde_json::json!({
            "group": group,
            "status": status,
            "capabilities": capabilities,
        }),
        json,
    );
    Ok(())
}

fn resolve_runtime_state_root(requested: Option<PathBuf>) -> PathBuf {
    requested
        .or_else(|| std::env::var_os("JIANYING_RUNTIME_ROOT").map(PathBuf::from))
        .unwrap_or_else(|| PathBuf::from(".jianying-runtime"))
}

fn load_runtime_profile(path: &Path) -> Result<jianying_runtime::RuntimeProfile> {
    Ok(serde_json::from_slice(&std::fs::read(path)?)?)
}

fn run_runtime(op: RuntimeOp, json: bool) -> Result<()> {
    match op {
        RuntimeOp::Discover {
            platform,
            mut search_roots,
            home,
            local_app_data,
        } => {
            let platform = platform
                .map(jianying_runtime::RuntimePlatform::from)
                .unwrap_or_else(current_runtime_platform);
            let home = home
                .or_else(|| std::env::var_os("HOME").map(PathBuf::from))
                .unwrap_or_else(|| PathBuf::from("."));
            let local_app_data =
                local_app_data.or_else(|| std::env::var_os("LOCALAPPDATA").map(PathBuf::from));
            if search_roots.is_empty() {
                search_roots =
                    default_runtime_search_roots(platform, &home, local_app_data.as_deref());
            }
            let installations = jianying_runtime::discover_runtime_installations(
                platform,
                &home,
                &search_roots,
                local_app_data.as_deref(),
            )?;
            let draft_roots = jianying_runtime::discover_runtime_draft_roots(
                platform,
                &home,
                local_app_data.as_deref(),
            );
            let existing_draft_roots = draft_roots.iter().filter(|root| root.exists()).count();
            let state = if !installations.is_empty() {
                "found_unverified"
            } else if existing_draft_roots > 0 {
                "drafts_without_editor"
            } else {
                "not_installed"
            };
            print_output(
                serde_json::json!({
                    "state":state,
                    "platform":platform,
                    "installations":installations,
                    "draft_roots":draft_roots,
                    "automatic_routing":false,
                    "support_status":"unverified",
                    "next":"create and validate an exact Runtime Profile, then run an authorized host canary"
                }),
                json,
            );
            Ok(())
        }
        RuntimeOp::Status {
            runtime_profile,
            state_root,
        } => {
            if let Some(path) = runtime_profile {
                let profile = load_runtime_profile(&path)?;
                let controller = jianying_runtime::PersistentRuntimeController::new(
                    resolve_runtime_state_root(state_root),
                );
                print_output(serde_json::to_value(controller.status(&profile)?)?, json);
                Ok(())
            } else {
                print_domain_status("runtime", json)
            }
        }
        RuntimeOp::Probe {
            runtime_profile,
            executable,
            draft_root,
            platform,
            product,
            version,
            running_processes,
            materials,
            capabilities,
        } => {
            let profile = load_runtime_profile(&runtime_profile)?;
            let input = jianying_runtime::RuntimeProbeInput::new(
                product.unwrap_or_else(|| profile.product().to_owned()),
                version.unwrap_or_else(|| profile.version().to_owned()),
                platform
                    .map(Into::into)
                    .unwrap_or_else(|| profile.platform()),
                executable,
                draft_root,
            )
            .with_running_processes(running_processes)
            .with_material_paths(materials)
            .with_requested_capabilities(capabilities);
            print_output(serde_json::to_value(profile.probe(input)?)?, json);
            Ok(())
        }
        RuntimeOp::Start {
            runtime_profile,
            executable,
            arguments,
            state_root,
        } => {
            let profile = load_runtime_profile(&runtime_profile)?;
            let controller = jianying_runtime::PersistentRuntimeController::new(
                resolve_runtime_state_root(state_root),
            );
            print_output(
                serde_json::to_value(controller.start(&profile, &executable, arguments)?)?,
                json,
            );
            Ok(())
        }
        RuntimeOp::Stop {
            runtime_profile,
            state_root,
        } => {
            let profile = load_runtime_profile(&runtime_profile)?;
            let controller = jianying_runtime::PersistentRuntimeController::new(
                resolve_runtime_state_root(state_root),
            );
            print_output(serde_json::to_value(controller.stop(&profile)?)?, json);
            Ok(())
        }
        RuntimeOp::ExportCapabilities => print_domain_status("runtime", json),
    }
}

fn current_runtime_platform() -> jianying_runtime::RuntimePlatform {
    #[cfg(target_os = "macos")]
    {
        jianying_runtime::RuntimePlatform::MacOs
    }
    #[cfg(target_os = "windows")]
    {
        jianying_runtime::RuntimePlatform::Windows
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        jianying_runtime::RuntimePlatform::Linux
    }
}

fn default_runtime_search_roots(
    platform: jianying_runtime::RuntimePlatform,
    home: &Path,
    local_app_data: Option<&Path>,
) -> Vec<PathBuf> {
    match platform {
        jianying_runtime::RuntimePlatform::MacOs => {
            vec![PathBuf::from("/Applications"), home.join("Applications")]
        }
        jianying_runtime::RuntimePlatform::Windows => {
            let mut roots = Vec::new();
            if let Some(value) = std::env::var_os("PROGRAMFILES") {
                roots.push(PathBuf::from(value));
            }
            if let Some(value) = std::env::var_os("PROGRAMFILES(X86)") {
                roots.push(PathBuf::from(value));
            }
            if let Some(value) = local_app_data {
                roots.push(value.to_path_buf());
            }
            roots
        }
        jianying_runtime::RuntimePlatform::Linux => Vec::new(),
    }
}

fn run_render_native(args: RenderNativeArgs, json: bool) -> Result<()> {
    let (Some(runtime_profile), Some(executable)) =
        (args.runtime_profile.as_deref(), args.executable.as_deref())
    else {
        return Err(
            jianying_cli::job_runner::JobRunError::IncompatibleCapability {
                capability: "render.native",
            }
            .into(),
        );
    };
    let profile = load_runtime_profile(runtime_profile)?;
    let cwd = std::env::current_dir()?;
    let draft = if args.draft.is_absolute() {
        args.draft
    } else {
        cwd.join(args.draft)
    };
    let executable = if executable.is_absolute() {
        executable.to_path_buf()
    } else {
        cwd.join(executable)
    };
    let output = if args.out.is_absolute() {
        args.out
    } else {
        cwd.join(args.out)
    };
    let report = profile.probe(
        jianying_runtime::RuntimeProbeInput::new(
            profile.product(),
            profile.version(),
            profile.platform(),
            executable,
            draft.clone(),
        )
        .with_requested_capabilities(["render.native"]),
    )?;
    let request = jianying_schema::ExportRequest::new(
        jianying_schema::ExportKind::Native,
        output,
        args.overwrite,
    )?;
    let task_id = args
        .task_id
        .unwrap_or_else(|| format!("native-{}", uuid::Uuid::new_v4().simple()));
    let submission = jianying_jobs::NativeExportSubmission::new(
        request, &profile, &report, draft, cwd, task_id,
    )?;
    if args.plan {
        print_output(
            serde_json::json!({
                "task_id": submission.task_id(),
                "state": "approval_required",
                "approval_binding": submission.approval_binding()?,
                "next": "grant the exact binding, then rerun without --plan and with --approval-id"
            }),
            json,
        );
        return Ok(());
    }
    let approval_id = args.approval_id.as_deref().ok_or_else(|| {
        anyhow::anyhow!(
            "native export requires --approval-id; run the same command with --plan first"
        )
    })?;
    let store = jianying_jobs::NativeExportStore::new(
        args.state_root
            .unwrap_or_else(|| PathBuf::from(".jianying-native-exports")),
    );
    let approvals =
        jianying_jobs::ApprovalStore::new(resolve_approval_state_root(args.approval_root));
    let record = if store.path(submission.task_id()).is_file() {
        store.retry(&submission, &approvals, approval_id, epoch_seconds())?
    } else {
        store.prepare(&submission, &approvals, approval_id, epoch_seconds())?
    };
    print_output(serde_json::to_value(record)?, json);
    Ok(())
}

fn resolve_native_export_state_root(requested: Option<PathBuf>) -> PathBuf {
    requested
        .or_else(|| std::env::var_os("JIANYING_NATIVE_EXPORT_ROOT").map(PathBuf::from))
        .unwrap_or_else(|| PathBuf::from(".jianying-native-exports"))
}

fn run_render_native_task(args: RenderNativeTaskArgs, json: bool) -> Result<()> {
    use jianying_jobs::NativeExportState;

    let (task_id, state_root) = match &args.operation {
        RenderNativeTaskOp::Show {
            task_id,
            state_root,
        }
        | RenderNativeTaskOp::Start {
            task_id,
            state_root,
        }
        | RenderNativeTaskOp::Progress {
            task_id,
            state_root,
            ..
        }
        | RenderNativeTaskOp::Verify {
            task_id,
            state_root,
        }
        | RenderNativeTaskOp::Interrupt {
            task_id,
            state_root,
            ..
        }
        | RenderNativeTaskOp::Fail {
            task_id,
            state_root,
            ..
        }
        | RenderNativeTaskOp::Result {
            task_id,
            state_root,
        } => (task_id.clone(), state_root.clone()),
    };
    let store = jianying_jobs::NativeExportStore::new(resolve_native_export_state_root(state_root));
    let output = match args.operation {
        RenderNativeTaskOp::Show { .. } => serde_json::to_value(store.load(&task_id)?)?,
        RenderNativeTaskOp::Start { .. } => {
            serde_json::to_value(store.mark_running(&task_id, epoch_seconds())?)?
        }
        RenderNativeTaskOp::Progress { percent, .. } => {
            serde_json::to_value(store.update_progress(&task_id, percent, epoch_seconds())?)?
        }
        RenderNativeTaskOp::Verify { .. } => {
            let current = store.load(&task_id)?;
            if current.state() == NativeExportState::Running {
                store.begin_verification(&task_id, epoch_seconds())?;
            } else if current.state() != NativeExportState::Verifying {
                return Err(jianying_jobs::NativeExportError::InvalidTransition {
                    from: current.state(),
                    to: NativeExportState::Verifying,
                }
                .into());
            }
            serde_json::to_value(store.mark_succeeded(&task_id, epoch_seconds())?)?
        }
        RenderNativeTaskOp::Interrupt { reason, .. } => {
            serde_json::to_value(store.mark_interrupted(&task_id, reason, epoch_seconds())?)?
        }
        RenderNativeTaskOp::Fail { reason, .. } => {
            serde_json::to_value(store.mark_failed(&task_id, reason, epoch_seconds())?)?
        }
        RenderNativeTaskOp::Result { .. } => serde_json::to_value(store.verify_result(&task_id)?)?,
    };
    print_output(output, json);
    Ok(())
}

fn run_render_proxy(args: RenderProxyArgs, json: bool) -> Result<()> {
    print_output(
        render::render(
            &args.draft,
            &args.out,
            args.scale,
            args.burn_captions,
            args.crf,
        )?,
        json,
    );
    Ok(())
}

fn catalog_query(
    domain: Option<&str>,
    search: Option<&str>,
    include_vip: bool,
    json: bool,
) -> Result<()> {
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
        print_output(
            serde_json::json!({"domains": domains,
                "note": "use --domain <name> [--search <substring>] to list entries"}),
            json,
        );
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
    print_output(
        serde_json::json!({"results": out, "count": out.len()}),
        json,
    );
    Ok(())
}

fn print_output(value: serde_json::Value, json: bool) {
    let value = if json {
        serde_json::to_value(jianying_cli::cli_contract::SuccessEnvelope::new(value))
            .unwrap_or_default()
    } else {
        value
    };
    println!(
        "{}",
        serde_json::to_string_pretty(&value).unwrap_or_default()
    );
}

fn run_main() {
    let cli = Cli::parse();
    if cli.no_color {
        std::env::set_var("NO_COLOR", "1");
    }
    std::env::set_var("JIANYING_LOG_LEVEL", cli.log_level.as_str());
    if let Err(error) = run(cli.command, cli.json, &cli.profile, cli.host_read_only) {
        if cli.json {
            let envelope = jianying_cli::error_contract::error_envelope(&error);
            println!(
                "{}",
                serde_json::to_string_pretty(&envelope).unwrap_or_default()
            );
        } else {
            eprintln!("error: {error:#}");
        }
        std::process::exit(1);
    }
}

fn main() {
    #[cfg(windows)]
    {
        // Windows 可执行文件的默认主线程栈较小；完整 Clap 命令树需要显式栈预算。
        let outcome = std::thread::Builder::new()
            .name("jianying-main".to_owned())
            .stack_size(16 * 1024 * 1024)
            .spawn(run_main)
            .expect("failed to start jianying main thread")
            .join();
        if let Err(panic) = outcome {
            std::panic::resume_unwind(panic);
        }
    }
    #[cfg(not(windows))]
    run_main();
}

#[cfg(test)]
mod tests {
    use super::redact_diagnostics;
    use serde_json::json;

    #[test]
    fn diagnostics_redact_credentials_and_absolute_paths() {
        let redacted = redact_diagnostics(json!({
            "token": "top-secret",
            "nested": {"api_key": "also-secret"},
            "database": "/private/customer/jobs.sqlite3",
            "windows_path": "C:\\Users\\customer\\jobs.sqlite3",
            "unc_root": "\\\\server\\share\\draft.json"
        }));
        assert_eq!(redacted["token"], "[REDACTED]");
        assert_eq!(redacted["nested"]["api_key"], "[REDACTED]");
        assert_eq!(redacted["database"], "<redacted-path>/jobs.sqlite3");
        assert_eq!(redacted["windows_path"], "<redacted-path>/jobs.sqlite3");
        assert_eq!(redacted["unc_root"], "<redacted-path>/draft.json");
        assert!(!redacted.to_string().contains("secret"));
        assert!(!redacted.to_string().contains("/private/customer"));
    }
}
