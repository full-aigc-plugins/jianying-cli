use jianying_runtime::DraftCopySession;
use serde_json::{json, Value};
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

fn fixture_root() -> PathBuf {
    static NEXT_ID: AtomicU64 = AtomicU64::new(1);
    let nonce = NEXT_ID.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!(
        "jianying-draft-copy-{}-{nonce}",
        std::process::id()
    ))
}

#[test]
fn existing_draft_is_edited_only_in_an_independent_copy() {
    let root = fixture_root();
    let source = root.join("source");
    let copy = root.join("copy");
    fs::create_dir_all(source.join("Resources")).unwrap();
    let original = json!({
        "tracks": [{"id": "track-1", "segments": [{"text": "before", "volume": 1.0}]}],
        "future_field": {"preserved": true}
    });
    fs::write(
        source.join("draft_content.json"),
        serde_json::to_vec_pretty(&original).unwrap(),
    )
    .unwrap();
    fs::write(source.join("Resources/clip.bin"), b"clip bytes").unwrap();

    let session = DraftCopySession::prepare(&source, &copy).unwrap();
    let mut edited: Value =
        serde_json::from_slice(&fs::read(copy.join("draft_content.json")).unwrap()).unwrap();
    edited["tracks"][0]["segments"][0]["text"] = json!("after");
    edited["tracks"][0]["segments"][0]["volume"] = json!(0.5);
    fs::write(
        copy.join("draft_content.json"),
        serde_json::to_vec_pretty(&edited).unwrap(),
    )
    .unwrap();

    let report = session.verify().unwrap();
    assert!(report.source_unchanged());
    assert_eq!(report.source_before(), report.source_after());
    assert_ne!(report.source_after(), report.copy_after());
    assert_eq!(
        serde_json::from_slice::<Value>(&fs::read(source.join("draft_content.json")).unwrap())
            .unwrap(),
        original
    );
    assert_eq!(
        fs::read(source.join("Resources/clip.bin")).unwrap(),
        b"clip bytes"
    );
    assert_eq!(
        report.changed_copy_files(),
        &[PathBuf::from("draft_content.json")]
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn copy_destination_must_be_new_and_source_must_be_directory() {
    let root = fixture_root();
    let source = root.join("source");
    let copy = root.join("copy");
    fs::create_dir_all(&source).unwrap();
    fs::create_dir_all(&copy).unwrap();
    assert!(DraftCopySession::prepare(&source, &copy).is_err());
    assert!(DraftCopySession::prepare(root.join("missing"), root.join("new-copy")).is_err());
    fs::remove_dir_all(root).unwrap();
}
