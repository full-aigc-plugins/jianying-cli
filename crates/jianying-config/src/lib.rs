mod config_document;
mod config_error;
mod config_store;
mod validation_report;

pub use config_document::ConfigDocument;
pub use config_error::ConfigError;
pub use config_store::ConfigStore;
pub use validation_report::ValidationReport;

/// 返回发布随二进制携带的配置 JSON Schema。
pub fn schema() -> serde_json::Value {
    serde_json::from_str(include_str!(
        "../../../schemas/jianying-config-v1.schema.json"
    ))
    .expect("checked-in config schema must be valid JSON")
}
