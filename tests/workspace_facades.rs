use jianying_cli::cli_contract::{ErrorEnvelope, SuccessEnvelope};
use jianying_cli::jobs::{JobCheckpoint, JobState};
use jianying_cli::mcp::ToolRegistry;
use jianying_cli::media::{MediaAsset, MediaKind};
use jianying_cli::runtime::{RuntimePlatform, RuntimeProfile};
use jianying_cli::store_model::DraftStoreLayout;
use serde_json::json;
use std::path::PathBuf;

#[test]
fn workspace_facades_have_real_validated_contracts() {
    let store = DraftStoreLayout::new(PathBuf::from("drafts")).unwrap();
    assert_eq!(
        store.draft_path("episode-01").unwrap(),
        PathBuf::from("drafts/episode-01")
    );
    assert!(store.draft_path("../escape").is_err());

    let media = MediaAsset::new(
        PathBuf::from("a.mp4"),
        MediaKind::Video,
        1_000_000,
        Some((1920, 1080)),
    )
    .unwrap();
    assert_eq!(media.duration_us(), 1_000_000);

    let profile = RuntimeProfile::new(
        "jianying-macos-11.4",
        "JianYing Pro",
        "11.4",
        RuntimePlatform::MacOs,
        ["project.edit_isolated", "render.native"],
    )
    .unwrap();
    assert!(profile.supports("render.native"));
    assert!(!profile.supports("media.asr_ledger"));

    let checkpoint = JobCheckpoint::new("task-1", JobState::Queued).unwrap();
    let running = checkpoint.transition(JobState::Running).unwrap();
    assert!(running.transition(JobState::Succeeded).is_ok());
    assert!(checkpoint.transition(JobState::Succeeded).is_err());

    let tools = ToolRegistry::standard();
    assert!(tools.get("jianying_job_run").is_some());
    assert!(tools.get("jianying_doctor").is_some());

    assert_eq!(
        serde_json::to_value(SuccessEnvelope::new(json!({"ok": 1}))).unwrap()["ok"],
        true
    );
    assert_eq!(
        serde_json::to_value(ErrorEnvelope::new("invalid_input", "bad input")).unwrap()["ok"],
        false
    );
}
