use jianying_runtime::{
    EntitlementEvidenceSource, EntitlementStatus, JianyingEdition, OfficialAssetReceipt,
    OfficialAssetTier, OfficialAssetUsage, OfficialResourceKind, RuntimeEntitlementSnapshot,
    RuntimeFileIdentity,
};
use serde_json::Value;
use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

fn fixture_root() -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!(
        "jianying-entitlement-schema-{}-{nonce}",
        std::process::id()
    ))
}

#[test]
fn checked_in_schemas_are_closed_and_cover_every_runtime_resource_kind() {
    let entitlement: serde_json::Value = serde_json::from_str(include_str!(
        "../schemas/jianying-entitlement-v1.schema.json"
    ))
    .unwrap();
    assert_eq!(entitlement["additionalProperties"], false);
    assert_eq!(
        entitlement["properties"]["schema_version"]["const"],
        "jianying-entitlement/v1"
    );
    for forbidden in ["cookie", "token", "phone", "email", "username"] {
        assert!(entitlement["properties"].get(forbidden).is_none());
    }

    let receipt: serde_json::Value = serde_json::from_str(include_str!(
        "../schemas/jianying-official-resource-receipt-v1.schema.json"
    ))
    .unwrap();
    assert_eq!(receipt["additionalProperties"], false);
    assert_eq!(
        receipt["properties"]["schema_version"]["const"],
        "jianying-official-resource-receipt/v1"
    );
    let kinds = receipt["properties"]["resource_kind"]["enum"]
        .as_array()
        .unwrap();
    for kind in [
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
        "digital_human",
        "animation",
        "sound_effect",
    ] {
        assert!(kinds.iter().any(|value| value == kind), "missing {kind}");
    }
    assert!(!kinds.iter().any(|value| value == "unknown"));
}

#[test]
fn official_resource_preflight_schema_is_closed_and_requires_exactly_one_identity_source() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("schemas/jianying-official-resource-preflight-v1.schema.json");
    let schema: Value = serde_json::from_slice(&fs::read(path).unwrap()).unwrap();
    assert_eq!(schema["additionalProperties"], false);
    assert_eq!(
        schema["properties"]["requirements"]["items"]["additionalProperties"],
        false
    );
    assert_eq!(
        schema["properties"]["requirements"]["items"]["oneOf"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
}

#[test]
fn deserialized_documents_revalidate_versions_and_invariants() {
    let snapshot = RuntimeEntitlementSnapshot::new(
        JianyingEdition::Professional,
        EntitlementStatus::Active,
        EntitlementEvidenceSource::GuiObserved,
        1_000,
        Some(2_000),
        ["official_assets.professional"],
    )
    .unwrap();
    let mut value = serde_json::to_value(snapshot).unwrap();
    value["schema_version"] = serde_json::json!("jianying-entitlement/v99");
    let decoded: RuntimeEntitlementSnapshot = serde_json::from_value(value).unwrap();
    assert!(decoded
        .validate_document()
        .unwrap_err()
        .to_string()
        .contains("unsupported schema version"));

    let root = fixture_root();
    fs::create_dir_all(&root).unwrap();
    let resource = root.join("resource.bin");
    fs::write(&resource, b"resource").unwrap();
    let receipt = OfficialAssetReceipt::new(
        "resource-001",
        "资源",
        OfficialResourceKind::Sticker,
        OfficialAssetTier::Basic,
        RuntimeFileIdentity::from_path(&resource).unwrap(),
        1_100,
        "https://example.invalid/terms",
        [OfficialAssetUsage::Personal],
        BTreeSet::new(),
        false,
    )
    .unwrap();
    let mut value = serde_json::to_value(receipt).unwrap();
    value["schema_version"] = serde_json::json!("jianying-official-resource-receipt/v99");
    let decoded: OfficialAssetReceipt = serde_json::from_value(value).unwrap();
    assert!(decoded
        .validate_document()
        .unwrap_err()
        .to_string()
        .contains("unsupported schema version"));
    fs::remove_dir_all(root).unwrap();
}
