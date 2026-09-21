use jianying_runtime::{
    EntitlementEvidenceSource, EntitlementStatus, JianyingEdition, OfficialAssetReceipt,
    OfficialAssetTier, OfficialAssetUsage, OfficialResourceKind, OfficialResourceReceiptStore,
    RuntimeEntitlementSnapshot, RuntimeFileIdentity,
};
use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

fn fixture_root(name: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!(
        "jianying-entitlement-{name}-{}-{nonce}",
        std::process::id()
    ))
}

fn professional_snapshot(expires_at: u64) -> RuntimeEntitlementSnapshot {
    RuntimeEntitlementSnapshot::new(
        JianyingEdition::Professional,
        EntitlementStatus::Active,
        EntitlementEvidenceSource::GuiObserved,
        1_000,
        Some(expires_at),
        ["official_assets.professional"],
    )
    .unwrap()
}

#[test]
fn application_name_does_not_imply_professional_entitlement() {
    let unknown = RuntimeEntitlementSnapshot::new(
        JianyingEdition::Unknown,
        EntitlementStatus::Unknown,
        EntitlementEvidenceSource::InstallationOnly,
        1_000,
        None,
        std::iter::empty::<&str>(),
    )
    .unwrap();

    let error = unknown
        .authorize(
            JianyingEdition::Professional,
            ["official_assets.professional"],
            1_001,
        )
        .unwrap_err();
    assert!(error.to_string().contains("professional"));
    assert!(error.to_string().contains("unknown"));
}

#[test]
fn professional_entitlement_expires_and_requires_declared_capabilities() {
    let snapshot = professional_snapshot(2_000);
    snapshot
        .authorize(
            JianyingEdition::Professional,
            ["official_assets.professional"],
            1_500,
        )
        .unwrap();

    assert!(snapshot
        .authorize(
            JianyingEdition::Professional,
            ["official_assets.professional"],
            2_000,
        )
        .unwrap_err()
        .to_string()
        .contains("expired"));
    assert!(snapshot
        .authorize(
            JianyingEdition::Professional,
            ["effects.professional"],
            1_500
        )
        .unwrap_err()
        .to_string()
        .contains("effects.professional"));
}

#[test]
fn entitlement_json_rejects_secret_or_unknown_fields() {
    let value = serde_json::json!({
        "schema_version": "jianying-entitlement/v1",
        "edition": "professional",
        "status": "active",
        "source": "gui_observed",
        "observed_at_epoch_seconds": 1000,
        "expires_at_epoch_seconds": 2000,
        "capabilities": ["official_assets.professional"],
        "access_token": "must-not-be-persisted"
    });
    let error = serde_json::from_value::<RuntimeEntitlementSnapshot>(value).unwrap_err();
    assert!(error.to_string().contains("unknown field"));
}

#[test]
fn official_asset_requires_matching_identity_entitlement_and_usage() {
    let root = fixture_root("asset");
    fs::create_dir_all(&root).unwrap();
    let asset = root.join("wedding.mp4");
    fs::write(&asset, b"licensed official asset").unwrap();
    let receipt = OfficialAssetReceipt::new(
        "jy-official-asset-001",
        "婚礼纪实镜头",
        OfficialResourceKind::Media,
        OfficialAssetTier::Professional,
        RuntimeFileIdentity::from_path(&asset).unwrap(),
        1_200,
        "https://www.capcut.cn/terms/content-license",
        [OfficialAssetUsage::Personal, OfficialAssetUsage::Commercial],
        BTreeSet::from(["不得单独转售素材".to_owned()]),
        false,
    )
    .unwrap();

    receipt
        .verify(
            &asset,
            &professional_snapshot(2_000),
            OfficialAssetUsage::Commercial,
            1_500,
        )
        .unwrap();

    let basic = RuntimeEntitlementSnapshot::new(
        JianyingEdition::Basic,
        EntitlementStatus::Active,
        EntitlementEvidenceSource::GuiObserved,
        1_000,
        None,
        ["official_assets.basic"],
    )
    .unwrap();
    assert!(receipt
        .verify(&asset, &basic, OfficialAssetUsage::Commercial, 1_500)
        .unwrap_err()
        .to_string()
        .contains("professional"));

    fs::write(&asset, b"changed bytes").unwrap();
    assert!(receipt
        .verify(
            &asset,
            &professional_snapshot(2_000),
            OfficialAssetUsage::Commercial,
            1_500,
        )
        .unwrap_err()
        .to_string()
        .contains("identity mismatch"));
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn preview_only_and_usage_conflicts_fail_closed() {
    let root = fixture_root("preview");
    fs::create_dir_all(&root).unwrap();
    let asset = root.join("preview.mp4");
    fs::write(&asset, b"preview asset").unwrap();
    let receipt = OfficialAssetReceipt::new(
        "jy-preview-001",
        "仅预览素材",
        OfficialResourceKind::Sticker,
        OfficialAssetTier::Basic,
        RuntimeFileIdentity::from_path(&asset).unwrap(),
        1_200,
        "https://www.capcut.cn/terms/content-license",
        [OfficialAssetUsage::Personal],
        BTreeSet::new(),
        true,
    )
    .unwrap();
    let basic = RuntimeEntitlementSnapshot::new(
        JianyingEdition::Basic,
        EntitlementStatus::Active,
        EntitlementEvidenceSource::GuiObserved,
        1_000,
        None,
        ["official_assets.basic"],
    )
    .unwrap();

    assert!(receipt
        .verify(&asset, &basic, OfficialAssetUsage::Personal, 1_500)
        .unwrap_err()
        .to_string()
        .contains("preview-only"));

    let deliverable = OfficialAssetReceipt::new(
        "jy-personal-001",
        "仅个人用途素材",
        OfficialResourceKind::Music,
        OfficialAssetTier::Basic,
        RuntimeFileIdentity::from_path(&asset).unwrap(),
        1_200,
        "https://www.capcut.cn/terms/content-license",
        [OfficialAssetUsage::Personal],
        BTreeSet::new(),
        false,
    )
    .unwrap();
    assert!(deliverable
        .verify(&asset, &basic, OfficialAssetUsage::Commercial, 1_500)
        .unwrap_err()
        .to_string()
        .contains("commercial"));
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn resource_families_are_explicit_and_unknown_resources_fail_closed() {
    let encoded = serde_json::to_value([
        OfficialResourceKind::Media,
        OfficialResourceKind::Music,
        OfficialResourceKind::TextTemplate,
        OfficialResourceKind::Sticker,
        OfficialResourceKind::VideoEffect,
        OfficialResourceKind::Transition,
        OfficialResourceKind::CaptionStyle,
        OfficialResourceKind::SmartPackage,
        OfficialResourceKind::SmartBRoll,
        OfficialResourceKind::Filter,
        OfficialResourceKind::Adjustment,
        OfficialResourceKind::Template,
        OfficialResourceKind::DigitalHuman,
    ])
    .unwrap();
    assert_eq!(
        encoded,
        serde_json::json!([
            "media",
            "music",
            "text_template",
            "sticker",
            "video_effect",
            "transition",
            "caption_style",
            "smart_package",
            "smart_b_roll",
            "filter",
            "adjustment",
            "template",
            "digital_human"
        ])
    );

    let root = fixture_root("unknown-kind");
    fs::create_dir_all(&root).unwrap();
    let asset = root.join("unknown.bin");
    fs::write(&asset, b"unknown resource").unwrap();
    let error = OfficialAssetReceipt::new(
        "jy-unknown-001",
        "未知资源",
        OfficialResourceKind::Unknown,
        OfficialAssetTier::Basic,
        RuntimeFileIdentity::from_path(&asset).unwrap(),
        1_200,
        "https://www.capcut.cn/terms/content-license",
        [OfficialAssetUsage::Personal],
        BTreeSet::new(),
        false,
    )
    .unwrap_err();
    assert!(error.to_string().contains("unknown resource kind"));
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn receipt_store_is_idempotent_sorted_and_rejects_conflicting_overwrite() {
    let root = fixture_root("receipt-store");
    let resources = root.join("resources");
    let receipts = root.join("receipts");
    fs::create_dir_all(&resources).unwrap();
    let first_path = resources.join("first.bin");
    let second_path = resources.join("second.bin");
    fs::write(&first_path, b"first").unwrap();
    fs::write(&second_path, b"second").unwrap();
    let make = |asset_id: &str, title: &str, path: &std::path::Path| {
        OfficialAssetReceipt::new(
            asset_id,
            title,
            OfficialResourceKind::Media,
            OfficialAssetTier::Basic,
            RuntimeFileIdentity::from_path(path).unwrap(),
            1_200,
            "https://example.invalid/terms",
            [OfficialAssetUsage::Personal],
            BTreeSet::new(),
            false,
        )
        .unwrap()
    };
    let second = make("resource-b", "B", &second_path);
    let first = make("resource-a", "A", &first_path);
    let store = OfficialResourceReceiptStore::new(&receipts);
    let first_store_path = store.register(&first).unwrap();
    assert_eq!(store.register(&first).unwrap(), first_store_path);
    store.register(&second).unwrap();
    let listed = store.list().unwrap();
    assert_eq!(listed[0].asset_id(), "resource-a");
    assert_eq!(listed[1].asset_id(), "resource-b");
    assert_eq!(store.show("resource-a").unwrap(), first);

    let conflicting = make("resource-a", "Changed", &second_path);
    assert!(store
        .register(&conflicting)
        .unwrap_err()
        .to_string()
        .contains("different content"));
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn draft_embedded_resource_identity_detects_missing_duplicate_and_changed_nodes() {
    let root = fixture_root("draft-resource");
    let draft = root.join("draft");
    fs::create_dir_all(&draft).unwrap();
    fs::write(
        draft.join("draft_content.json"),
        br#"{"materials":{"stickers":[{"id":"sticker-1","name":"Vlog","resource_id":"remote-1"}]},"tracks":[]}"#,
    )
    .unwrap();
    let receipt = OfficialAssetReceipt::new_draft_resource(
        "official-sticker-001",
        "Vlog 贴纸",
        OfficialResourceKind::Sticker,
        OfficialAssetTier::Professional,
        &draft,
        "sticker-1",
        1_200,
        "https://example.invalid/terms",
        [OfficialAssetUsage::Commercial],
        BTreeSet::new(),
        false,
    )
    .unwrap();
    receipt
        .verify_draft(
            &draft,
            &professional_snapshot(2_000),
            OfficialAssetUsage::Commercial,
            1_500,
        )
        .unwrap();

    fs::write(
        draft.join("draft_content.json"),
        br#"{"materials":{"stickers":[{"id":"sticker-1","name":"Changed","resource_id":"remote-1"}]},"tracks":[]}"#,
    )
    .unwrap();
    assert!(receipt
        .verify_draft(
            &draft,
            &professional_snapshot(2_000),
            OfficialAssetUsage::Commercial,
            1_500,
        )
        .unwrap_err()
        .to_string()
        .contains("identity mismatch"));

    fs::write(
        draft.join("draft_content.json"),
        br#"{"materials":{"stickers":[{"id":"same"},{"id":"same"}]},"tracks":[]}"#,
    )
    .unwrap();
    assert!(OfficialAssetReceipt::new_draft_resource(
        "ambiguous",
        "重复",
        OfficialResourceKind::Sticker,
        OfficialAssetTier::Basic,
        &draft,
        "same",
        1_200,
        "https://example.invalid/terms",
        [OfficialAssetUsage::Personal],
        BTreeSet::new(),
        false,
    )
    .unwrap_err()
    .to_string()
    .contains("matched 2 nodes"));
    fs::remove_dir_all(root).unwrap();
}
