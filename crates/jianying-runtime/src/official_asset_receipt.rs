use crate::{
    OfficialAssetTier, OfficialAssetUsage, OfficialResourceIdentity, OfficialResourceKind,
    RuntimeEntitlementSnapshot, RuntimeError, RuntimeFileIdentity,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::path::Path;

/// 将官方素材、实际下载文件、权益层级和允许用途绑定的不可变收据。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OfficialAssetReceipt {
    schema_version: String,
    asset_id: String,
    title: String,
    resource_kind: OfficialResourceKind,
    tier: OfficialAssetTier,
    identity: OfficialResourceIdentity,
    acquired_at_epoch_seconds: u64,
    terms_reference: String,
    allowed_usages: BTreeSet<OfficialAssetUsage>,
    restrictions: BTreeSet<String>,
    preview_only: bool,
}

impl OfficialAssetReceipt {
    /// 创建官方素材收据；空标识、空条款和空用途均被拒绝。
    #[allow(clippy::too_many_arguments)]
    pub fn new<I>(
        asset_id: impl Into<String>,
        title: impl Into<String>,
        resource_kind: OfficialResourceKind,
        tier: OfficialAssetTier,
        file_identity: RuntimeFileIdentity,
        acquired_at_epoch_seconds: u64,
        terms_reference: impl Into<String>,
        allowed_usages: I,
        restrictions: BTreeSet<String>,
        preview_only: bool,
    ) -> Result<Self, RuntimeError>
    where
        I: IntoIterator<Item = OfficialAssetUsage>,
    {
        let asset_id = asset_id.into();
        let title = title.into();
        let terms_reference = terms_reference.into();
        let allowed_usages: BTreeSet<OfficialAssetUsage> = allowed_usages.into_iter().collect();
        if asset_id.trim().is_empty() || title.trim().is_empty() {
            return Err(RuntimeError::InvalidAssetReceipt(
                "asset id and title must not be empty".to_owned(),
            ));
        }
        if !resource_kind.supports_automatic_application() {
            return Err(RuntimeError::InvalidAssetReceipt(
                "unknown resource kind cannot be applied automatically".to_owned(),
            ));
        }
        if acquired_at_epoch_seconds == 0 {
            return Err(RuntimeError::InvalidAssetReceipt(
                "acquisition time must be positive".to_owned(),
            ));
        }
        if terms_reference.trim().is_empty() {
            return Err(RuntimeError::InvalidAssetReceipt(
                "terms reference must not be empty".to_owned(),
            ));
        }
        if allowed_usages.is_empty() {
            return Err(RuntimeError::InvalidAssetReceipt(
                "at least one allowed usage is required".to_owned(),
            ));
        }
        Ok(Self {
            schema_version: "jianying-official-resource-receipt/v1".to_owned(),
            asset_id,
            title,
            resource_kind,
            tier,
            identity: OfficialResourceIdentity::DownloadedFile {
                file: file_identity,
            },
            acquired_at_epoch_seconds,
            terms_reference,
            allowed_usages,
            restrictions,
            preview_only,
        })
    }

    /// 为草稿内嵌资源创建收据；资源节点必须在草稿中唯一。
    #[allow(clippy::too_many_arguments)]
    pub fn new_draft_resource<I>(
        asset_id: impl Into<String>,
        title: impl Into<String>,
        resource_kind: OfficialResourceKind,
        tier: OfficialAssetTier,
        draft: impl AsRef<Path>,
        resource_id: impl Into<String>,
        acquired_at_epoch_seconds: u64,
        terms_reference: impl Into<String>,
        allowed_usages: I,
        restrictions: BTreeSet<String>,
        preview_only: bool,
    ) -> Result<Self, RuntimeError>
    where
        I: IntoIterator<Item = OfficialAssetUsage>,
    {
        let asset_id = asset_id.into();
        let title = title.into();
        let terms_reference = terms_reference.into();
        let allowed_usages: BTreeSet<OfficialAssetUsage> = allowed_usages.into_iter().collect();
        let receipt = Self {
            schema_version: "jianying-official-resource-receipt/v1".to_owned(),
            asset_id,
            title,
            resource_kind,
            tier,
            identity: OfficialResourceIdentity::draft_resource(draft, resource_id)?,
            acquired_at_epoch_seconds,
            terms_reference,
            allowed_usages,
            restrictions,
            preview_only,
        };
        receipt.validate_document()?;
        Ok(receipt)
    }

    /// 重新计算本地文件身份并校验版型、权益能力和目标用途。
    pub fn verify(
        &self,
        path: impl AsRef<Path>,
        entitlement: &RuntimeEntitlementSnapshot,
        usage: OfficialAssetUsage,
        now_epoch_seconds: u64,
    ) -> Result<(), RuntimeError> {
        let path = path.as_ref();
        if self.preview_only {
            return Err(RuntimeError::PreviewOnlyAsset);
        }
        self.identity.verify_file(path)?;
        entitlement.authorize(
            self.tier.required_edition(),
            [self.tier.required_capability()],
            now_epoch_seconds,
        )?;
        if !self.allowed_usages.contains(&usage) {
            return Err(RuntimeError::AssetUsageNotAllowed(usage.to_string()));
        }
        Ok(())
    }

    /// 校验草稿内嵌资源身份、版型、权益能力和目标用途。
    pub fn verify_draft(
        &self,
        draft: impl AsRef<Path>,
        entitlement: &RuntimeEntitlementSnapshot,
        usage: OfficialAssetUsage,
        now_epoch_seconds: u64,
    ) -> Result<(), RuntimeError> {
        if self.preview_only {
            return Err(RuntimeError::PreviewOnlyAsset);
        }
        self.identity.verify_draft(draft)?;
        entitlement.authorize(
            self.tier.required_edition(),
            [self.tier.required_capability()],
            now_epoch_seconds,
        )?;
        if !self.allowed_usages.contains(&usage) {
            return Err(RuntimeError::AssetUsageNotAllowed(usage.to_string()));
        }
        Ok(())
    }

    /// 校验从外部 JSON 反序列化的收据版本与字段不变量。
    pub fn validate_document(&self) -> Result<(), RuntimeError> {
        if self.schema_version != "jianying-official-resource-receipt/v1" {
            return Err(RuntimeError::InvalidAssetReceipt(format!(
                "unsupported schema version: {}",
                self.schema_version
            )));
        }
        if self.asset_id.trim().is_empty() || self.title.trim().is_empty() {
            return Err(RuntimeError::InvalidAssetReceipt(
                "asset id and title must not be empty".to_owned(),
            ));
        }
        if !self.resource_kind.supports_automatic_application() {
            return Err(RuntimeError::InvalidAssetReceipt(
                "unknown resource kind cannot be applied automatically".to_owned(),
            ));
        }
        if self.acquired_at_epoch_seconds == 0
            || self.terms_reference.trim().is_empty()
            || self.allowed_usages.is_empty()
        {
            return Err(RuntimeError::InvalidAssetReceipt(
                "acquisition time, terms and allowed usages are required".to_owned(),
            ));
        }
        Ok(())
    }

    /// 返回稳定官方资源标识。
    pub fn asset_id(&self) -> &str {
        &self.asset_id
    }

    /// 返回资源类型。
    pub fn resource_kind(&self) -> OfficialResourceKind {
        self.resource_kind
    }
}
