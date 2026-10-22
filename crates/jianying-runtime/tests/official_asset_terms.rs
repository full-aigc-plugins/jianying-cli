//! 4.2 的拒绝路径：条款缺失、价格文件不符、授权文件缺失。
//!
//! 收据主体（素材身份、权益、用途）由 `edition_entitlements.rs` 覆盖；这里只钉条款块。

use jianying_runtime::{
    OfficialAssetReceipt, OfficialAssetTerms, OfficialAssetTier, OfficialAssetUsage,
    OfficialResourceKind, RuntimeError, RuntimeFileIdentity,
};
use std::collections::BTreeSet;

fn receipt() -> OfficialAssetReceipt {
    let file = std::env::temp_dir().join(format!("terms-file-{}.bin", std::process::id()));
    std::fs::write(&file, b"asset-bytes").unwrap();
    let identity = RuntimeFileIdentity::from_path(&file).unwrap();
    OfficialAssetReceipt::new(
        "terms-asset",
        "条款拒绝路径探针",
        OfficialResourceKind::Media,
        OfficialAssetTier::Professional,
        identity,
        1_760_000_000,
        "不可商用（预览）",
        [OfficialAssetUsage::Personal],
        BTreeSet::new(),
        false,
    )
    .unwrap()
}

#[test]
fn a_receipt_without_terms_is_refused() {
    let error = receipt().verify_terms().unwrap_err();
    assert!(
        matches!(error, RuntimeError::TermsReferenceMissing),
        "{error}"
    );
}

#[test]
fn an_empty_terms_reference_is_refused() {
    assert!(matches!(
        OfficialAssetTerms::new("   ").unwrap_err(),
        RuntimeError::TermsReferenceMissing
    ));
}

#[test]
fn a_price_document_whose_hash_differs_is_refused() {
    let dir = std::env::temp_dir().join(format!("terms-price-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let price = dir.join("price.json");
    std::fs::write(&price, b"price").unwrap();
    let terms = OfficialAssetTerms::new("价格表 v1")
        .unwrap()
        .with_price(&price, "0".repeat(64));
    let error = receipt().with_terms(terms).verify_terms().unwrap_err();
    assert!(
        matches!(error, RuntimeError::PriceDocumentMismatch { .. }),
        "{error}"
    );
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn a_missing_authorisation_document_is_refused() {
    let terms = OfficialAssetTerms::new("授权书 v1")
        .unwrap()
        .with_authorisation(std::env::temp_dir().join("does-not-exist-authorisation.pdf"));
    let error = receipt().with_terms(terms).verify_terms().unwrap_err();
    assert!(
        matches!(error, RuntimeError::AuthorisationDocumentMissing(_)),
        "{error}"
    );
}

#[test]
fn complete_terms_verify_and_keep_the_notice() {
    let dir = std::env::temp_dir().join(format!("terms-ok-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let price = dir.join("price.json");
    std::fs::write(&price, b"price").unwrap();
    let authorisation = dir.join("authorisation.pdf");
    std::fs::write(&authorisation, b"grant").unwrap();
    let digest = {
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(b"price");
        format!("{:x}", hasher.finalize())
    };
    let terms = OfficialAssetTerms::new("价格表 v1")
        .unwrap()
        .with_price(&price, digest)
        .with_authorisation(&authorisation)
        .with_user_visible_notice("不可商用");
    let receipt = receipt().with_terms(terms);
    receipt.validate_document().unwrap();
    receipt.verify_terms().unwrap();
    assert_eq!(
        receipt.terms().unwrap().user_visible_notice(),
        Some("不可商用")
    );

    // 序列化后仍是同一份收据（terms 参与 JSON 往返）
    let json = serde_json::to_string(&receipt).unwrap();
    let back: OfficialAssetReceipt = serde_json::from_str(&json).unwrap();
    assert_eq!(back, receipt);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn the_4_1_label_alone_satisfies_terms() {
    // 4.1 以“不可商用标注即许可证据”收口；这里证明该标注能原样进入契约并通过严格校验，
    // 价格与授权文件在缺失时都是可选的（业主已确认可为空）。
    let terms = OfficialAssetTerms::new("不可商用")
        .unwrap()
        .with_user_visible_notice("不可商用");
    let receipt = receipt().with_terms(terms);
    receipt.validate_document().unwrap();
    receipt.verify_terms().unwrap();
    let terms = receipt.terms().unwrap();
    assert_eq!(terms.reference(), "不可商用");
    assert_eq!(terms.user_visible_notice(), Some("不可商用"));
}
