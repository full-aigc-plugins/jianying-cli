use jianying_runtime::{
    EntitlementEvidenceSource, EntitlementStatus, JianyingEdition, OfficialAssetReceipt,
    OfficialAssetTier, OfficialAssetUsage, OfficialResourceKind, RuntimeEntitlementSnapshot,
    RuntimeFileIdentity,
};
use serde_json::Value;
use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_jianying"))
        .args(args)
        .output()
        .expect("run jianying")
}

fn fixture_root(name: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!(
        "jianying-edition-cli-{name}-{}-{nonce}",
        std::process::id()
    ))
}

#[test]
fn entitlement_and_official_resource_commands_are_machine_discoverable() {
    let runtime = run(&["runtime", "--help"]);
    assert!(runtime.status.success());
    assert!(String::from_utf8(runtime.stdout)
        .unwrap()
        .contains("entitlement"));

    let media = run(&["media", "--help"]);
    assert!(media.status.success());
    assert!(String::from_utf8(media.stdout)
        .unwrap()
        .contains("official"));
}

#[test]
fn professional_entitlement_and_resource_receipt_complete_a_cli_round_trip() {
    let root = fixture_root("round-trip");
    fs::create_dir_all(&root).unwrap();
    let entitlement_path = root.join("entitlement.json");
    let receipt_path = root.join("receipt.json");
    let resource_path = root.join("music.wav");
    let store_root = root.join("receipts");
    fs::write(&resource_path, b"licensed professional music").unwrap();

    let entitlement = RuntimeEntitlementSnapshot::new(
        JianyingEdition::Professional,
        EntitlementStatus::Active,
        EntitlementEvidenceSource::GuiObserved,
        1_000,
        Some(2_000),
        ["official_assets.professional"],
    )
    .unwrap();
    fs::write(
        &entitlement_path,
        serde_json::to_vec_pretty(&entitlement).unwrap(),
    )
    .unwrap();
    let receipt = OfficialAssetReceipt::new(
        "official-music-001",
        "专业版音乐",
        OfficialResourceKind::Music,
        OfficialAssetTier::Professional,
        RuntimeFileIdentity::from_path(&resource_path).unwrap(),
        1_100,
        "https://example.invalid/jianying/resource-terms",
        [OfficialAssetUsage::Commercial],
        BTreeSet::from(["不得单独转售".to_owned()]),
        false,
    )
    .unwrap();
    fs::write(&receipt_path, serde_json::to_vec_pretty(&receipt).unwrap()).unwrap();

    let entitlement_output = run(&[
        "--json",
        "runtime",
        "entitlement",
        "verify",
        entitlement_path.to_str().unwrap(),
        "--required-edition",
        "professional",
        "--capability",
        "official_assets.professional",
        "--at",
        "1500",
    ]);
    assert!(
        entitlement_output.status.success(),
        "stdout={} stderr={}",
        String::from_utf8_lossy(&entitlement_output.stdout),
        String::from_utf8_lossy(&entitlement_output.stderr)
    );
    let envelope: Value = serde_json::from_slice(&entitlement_output.stdout).unwrap();
    assert_eq!(envelope["data"]["authorized"], true);
    assert_eq!(envelope["data"]["edition"], "professional");

    let registered = run(&[
        "--json",
        "media",
        "official",
        "register",
        receipt_path.to_str().unwrap(),
        resource_path.to_str().unwrap(),
        "--entitlement",
        entitlement_path.to_str().unwrap(),
        "--usage",
        "commercial",
        "--at",
        "1500",
        "--store-root",
        store_root.to_str().unwrap(),
    ]);
    assert!(
        registered.status.success(),
        "stdout={} stderr={}",
        String::from_utf8_lossy(&registered.stdout),
        String::from_utf8_lossy(&registered.stderr)
    );
    let envelope: Value = serde_json::from_slice(&registered.stdout).unwrap();
    assert_eq!(envelope["data"]["status"], "registered");
    assert_eq!(envelope["data"]["asset_id"], "official-music-001");

    let listed = run(&[
        "--json",
        "media",
        "official",
        "list",
        "--store-root",
        store_root.to_str().unwrap(),
    ]);
    assert!(listed.status.success());
    let envelope: Value = serde_json::from_slice(&listed.stdout).unwrap();
    assert_eq!(
        envelope["data"]["receipts"][0]["asset_id"],
        "official-music-001"
    );

    let shown = run(&[
        "--json",
        "media",
        "official",
        "show",
        "official-music-001",
        "--store-root",
        store_root.to_str().unwrap(),
    ]);
    assert!(shown.status.success());
    let envelope: Value = serde_json::from_slice(&shown.stdout).unwrap();
    assert_eq!(envelope["data"]["resource_kind"], "music");

    let requirements_path = root.join("official-requirements.json");
    fs::write(
        &requirements_path,
        serde_json::to_vec_pretty(&serde_json::json!({
            "schema_version":"jianying-official-resource-preflight/v1",
            "requirements":[{
                "asset_id":"official-music-001",
                "usage":"commercial",
                "resource":resource_path
            }]
        }))
        .unwrap(),
    )
    .unwrap();
    let preflight = run(&[
        "--json",
        "media",
        "official",
        "preflight",
        requirements_path.to_str().unwrap(),
        "--entitlement",
        entitlement_path.to_str().unwrap(),
        "--at",
        "1500",
        "--store-root",
        store_root.to_str().unwrap(),
    ]);
    assert!(
        preflight.status.success(),
        "stdout={} stderr={}",
        String::from_utf8_lossy(&preflight.stdout),
        String::from_utf8_lossy(&preflight.stderr)
    );
    let envelope: Value = serde_json::from_slice(&preflight.stdout).unwrap();
    assert_eq!(envelope["data"]["status"], "ready");
    assert_eq!(
        envelope["data"]["requirements"][0]["asset_id"],
        "official-music-001"
    );
    assert!(serde_json::to_string(&envelope)
        .unwrap()
        .find("do-not-leak")
        .is_none());

    fs::write(&resource_path, b"tampered resource").unwrap();
    let rejected = run(&[
        "--json",
        "media",
        "official",
        "verify",
        receipt_path.to_str().unwrap(),
        resource_path.to_str().unwrap(),
        "--entitlement",
        entitlement_path.to_str().unwrap(),
        "--usage",
        "commercial",
        "--at",
        "1500",
    ]);
    assert!(!rejected.status.success());
    let envelope: Value = serde_json::from_slice(&rejected.stdout).unwrap();
    assert_eq!(envelope["error"]["type"], "asset_identity_mismatch");

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn expired_professional_entitlement_fails_without_downgrade() {
    let root = fixture_root("expired");
    fs::create_dir_all(&root).unwrap();
    let entitlement_path = root.join("entitlement.json");
    let entitlement = RuntimeEntitlementSnapshot::new(
        JianyingEdition::Professional,
        EntitlementStatus::Active,
        EntitlementEvidenceSource::GuiObserved,
        1_000,
        Some(2_000),
        ["official_assets.professional"],
    )
    .unwrap();
    fs::write(
        &entitlement_path,
        serde_json::to_vec_pretty(&entitlement).unwrap(),
    )
    .unwrap();
    let output = run(&[
        "--json",
        "runtime",
        "entitlement",
        "verify",
        entitlement_path.to_str().unwrap(),
        "--required-edition",
        "professional",
        "--at",
        "2000",
    ]);
    assert!(!output.status.success());
    let envelope: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(envelope["error"]["type"], "entitlement_expired");
    assert_eq!(
        envelope["error"]["details"]["expires_at_epoch_seconds"],
        2_000
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn basic_edition_and_missing_capability_return_structured_requirements() {
    let root = fixture_root("requirements");
    fs::create_dir_all(&root).unwrap();
    let basic_path = root.join("basic.json");
    let basic = RuntimeEntitlementSnapshot::new(
        JianyingEdition::Basic,
        EntitlementStatus::Active,
        EntitlementEvidenceSource::GuiObserved,
        1_000,
        None,
        ["official_assets.basic"],
    )
    .unwrap();
    fs::write(&basic_path, serde_json::to_vec_pretty(&basic).unwrap()).unwrap();
    let edition = run(&[
        "--json",
        "runtime",
        "entitlement",
        "verify",
        basic_path.to_str().unwrap(),
        "--required-edition",
        "professional",
        "--at",
        "1500",
    ]);
    assert!(!edition.status.success());
    let envelope: Value = serde_json::from_slice(&edition.stdout).unwrap();
    assert_eq!(envelope["error"]["type"], "entitlement_denied");
    assert_eq!(
        envelope["error"]["details"]["required_edition"],
        "professional"
    );
    assert_eq!(envelope["error"]["details"]["observed_edition"], "basic");

    let capability = run(&[
        "--json",
        "runtime",
        "entitlement",
        "verify",
        basic_path.to_str().unwrap(),
        "--required-edition",
        "basic",
        "--capability",
        "effects.professional",
        "--at",
        "1500",
    ]);
    assert!(!capability.status.success());
    let envelope: Value = serde_json::from_slice(&capability.stdout).unwrap();
    assert_eq!(
        envelope["error"]["details"]["capability"],
        "effects.professional"
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn embedded_draft_resource_can_be_verified_and_registered_without_a_download_file() {
    let root = fixture_root("draft-resource");
    let draft = root.join("draft");
    let store_root = root.join("receipts");
    fs::create_dir_all(&draft).unwrap();
    fs::write(
        draft.join("draft_content.json"),
        r#"{"materials":{"video_effects":[{"id":"effect-1","name":"科技感","resource_id":"remote-effect"}]},"tracks":[]}"#,
    )
    .unwrap();
    let entitlement_path = root.join("entitlement.json");
    let entitlement = RuntimeEntitlementSnapshot::new(
        JianyingEdition::Professional,
        EntitlementStatus::Active,
        EntitlementEvidenceSource::GuiObserved,
        1_000,
        Some(2_000),
        ["official_assets.professional"],
    )
    .unwrap();
    fs::write(
        &entitlement_path,
        serde_json::to_vec_pretty(&entitlement).unwrap(),
    )
    .unwrap();
    let receipt_path = root.join("receipt.json");
    let receipt = OfficialAssetReceipt::new_draft_resource(
        "official-effect-001",
        "科技感特效",
        OfficialResourceKind::VideoEffect,
        OfficialAssetTier::Professional,
        &draft,
        "effect-1",
        1_100,
        "https://example.invalid/terms",
        [OfficialAssetUsage::Commercial],
        BTreeSet::new(),
        false,
    )
    .unwrap();
    fs::write(&receipt_path, serde_json::to_vec_pretty(&receipt).unwrap()).unwrap();
    let output = run(&[
        "--json",
        "media",
        "official",
        "register",
        receipt_path.to_str().unwrap(),
        "--draft",
        draft.to_str().unwrap(),
        "--entitlement",
        entitlement_path.to_str().unwrap(),
        "--usage",
        "commercial",
        "--at",
        "1500",
        "--store-root",
        store_root.to_str().unwrap(),
    ]);
    assert!(
        output.status.success(),
        "stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let envelope: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(envelope["data"]["status"], "registered");
    assert_eq!(envelope["data"]["draft"], draft.to_string_lossy().as_ref());
    fs::remove_dir_all(root).unwrap();
}
