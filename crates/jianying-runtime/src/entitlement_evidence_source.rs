use serde::{Deserialize, Serialize};

/// 权益证据的最小来源类型，不包含账号标识或认证材料。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EntitlementEvidenceSource {
    InstallationOnly,
    GuiObserved,
    ProviderVerified,
}
