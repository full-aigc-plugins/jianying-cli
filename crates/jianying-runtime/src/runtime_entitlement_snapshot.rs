use crate::{EntitlementEvidenceSource, EntitlementStatus, JianyingEdition, RuntimeError};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

/// 不含账号秘密、可过期的剪映权益观察快照。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeEntitlementSnapshot {
    schema_version: String,
    edition: JianyingEdition,
    status: EntitlementStatus,
    source: EntitlementEvidenceSource,
    observed_at_epoch_seconds: u64,
    expires_at_epoch_seconds: Option<u64>,
    capabilities: BTreeSet<String>,
}

impl RuntimeEntitlementSnapshot {
    /// 创建并校验最小化权益快照。
    pub fn new<I, S>(
        edition: JianyingEdition,
        status: EntitlementStatus,
        source: EntitlementEvidenceSource,
        observed_at_epoch_seconds: u64,
        expires_at_epoch_seconds: Option<u64>,
        capabilities: I,
    ) -> Result<Self, RuntimeError>
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        if observed_at_epoch_seconds == 0 {
            return Err(RuntimeError::InvalidEntitlement(
                "observed time must be positive".to_owned(),
            ));
        }
        if expires_at_epoch_seconds.is_some_and(|value| value <= observed_at_epoch_seconds) {
            return Err(RuntimeError::InvalidEntitlement(
                "expiry must be later than observation".to_owned(),
            ));
        }
        let capabilities: BTreeSet<String> = capabilities.into_iter().map(Into::into).collect();
        if capabilities.iter().any(|value| value.trim().is_empty()) {
            return Err(RuntimeError::InvalidEntitlement(
                "capability must not be empty".to_owned(),
            ));
        }
        Ok(Self {
            schema_version: "jianying-entitlement/v1".to_owned(),
            edition,
            status,
            source,
            observed_at_epoch_seconds,
            expires_at_epoch_seconds,
            capabilities,
        })
    }

    /// 校验当前快照是否满足版型、能力和时效要求。
    pub fn authorize<I, S>(
        &self,
        required_edition: JianyingEdition,
        required_capabilities: I,
        now_epoch_seconds: u64,
    ) -> Result<(), RuntimeError>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        if !self.edition.satisfies(required_edition) {
            return Err(RuntimeError::InsufficientEdition {
                required: required_edition.to_string(),
                observed: self.edition.to_string(),
            });
        }
        if self.status != EntitlementStatus::Active {
            return Err(RuntimeError::EntitlementNotActive(self.status.to_string()));
        }
        if let Some(expires_at) = self.expires_at_epoch_seconds {
            if now_epoch_seconds >= expires_at {
                return Err(RuntimeError::EntitlementExpired(expires_at));
            }
        }
        for capability in required_capabilities {
            let capability = capability.as_ref();
            let inherited = capability == "official_assets.basic"
                && self.capabilities.contains("official_assets.professional");
            if !self.capabilities.contains(capability) && !inherited {
                return Err(RuntimeError::EntitlementCapabilityMissing(
                    capability.to_owned(),
                ));
            }
        }
        Ok(())
    }

    /// 校验从外部 JSON 反序列化的快照版本与字段不变量。
    pub fn validate_document(&self) -> Result<(), RuntimeError> {
        if self.schema_version != "jianying-entitlement/v1" {
            return Err(RuntimeError::InvalidEntitlement(format!(
                "unsupported schema version: {}",
                self.schema_version
            )));
        }
        if self.observed_at_epoch_seconds == 0 {
            return Err(RuntimeError::InvalidEntitlement(
                "observed time must be positive".to_owned(),
            ));
        }
        if self
            .expires_at_epoch_seconds
            .is_some_and(|value| value <= self.observed_at_epoch_seconds)
        {
            return Err(RuntimeError::InvalidEntitlement(
                "expiry must be later than observation".to_owned(),
            ));
        }
        if self
            .capabilities
            .iter()
            .any(|value| value.trim().is_empty())
        {
            return Err(RuntimeError::InvalidEntitlement(
                "capability must not be empty".to_owned(),
            ));
        }
        Ok(())
    }

    /// 返回证据中的账号版型。
    pub fn edition(&self) -> JianyingEdition {
        self.edition
    }
}
