use jianying_config::{ConfigError, ConfigStore};
use serde_json::json;

#[test]
fn set_patch_get_unset_are_profile_scoped() {
    let root = std::env::temp_dir().join(format!("jyc-config-store-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let alpha = ConfigStore::new(&root, "alpha", false).unwrap();
    let beta = ConfigStore::new(&root, "beta", false).unwrap();

    alpha.set("render.quality", json!("high")).unwrap();
    alpha
        .patch(json!({"render":{"threads":4},"enabled":true}))
        .unwrap();
    assert_eq!(alpha.get(Some("render.quality")).unwrap(), json!("high"));
    assert_eq!(alpha.get(Some("render.threads")).unwrap(), json!(4));
    assert_eq!(beta.get(None).unwrap()["values"], json!({}));

    alpha.unset("render.quality").unwrap();
    assert!(matches!(
        alpha.get(Some("render.quality")),
        Err(ConfigError::KeyNotFound(_))
    ));
    assert!(alpha.validate().unwrap().valid);
}

#[test]
fn read_only_and_invalid_profiles_fail_closed() {
    let root = std::env::temp_dir().join(format!("jyc-config-readonly-{}", std::process::id()));
    let read_only = ConfigStore::new(&root, "managed", true).unwrap();
    assert!(matches!(
        read_only.set("render.quality", json!("high")),
        Err(ConfigError::ReadOnly)
    ));
    assert!(!root.exists());
    assert!(matches!(
        ConfigStore::new(&root, "../escape", false),
        Err(ConfigError::InvalidProfile(_))
    ));
}
