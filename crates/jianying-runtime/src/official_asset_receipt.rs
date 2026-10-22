use crate::{
    OfficialAssetTier, OfficialAssetUsage, OfficialResourceIdentity, OfficialResourceKind,
    RuntimeEntitlementSnapshot, RuntimeError, RuntimeFileIdentity,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

/// 条款、价格与授权文件的引用块（4.2 的契约扩展）。
///
/// 4.2 要求证明“价格文件、政府授权与用户可见信息”的拒绝路径，而原收据只表达素材本身，
/// 没有这些字段的位置。这里把三者表达为“引用 + 可选文件摘要”，从而可以校验：
/// 缺条款、价格文件与声明不符、授权文件缺失。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OfficialAssetTerms {
    /// 条款的稳定标识；不可为空，面板标注文本（例如“不可商用”）也算合法引用。
    reference: String,
    /// 价格文件路径；有路径就必须有摘要，两者一起构成可校验的声明。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    price_reference: Option<PathBuf>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    price_sha256: Option<String>,
    /// 授权文件（含政府/机构授权）路径。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    authorisation_reference: Option<PathBuf>,
    /// 界面原文，例如“不可商用”；用于把 4.1 的标注直接带进契约。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    user_visible_notice: Option<String>,
}

impl OfficialAssetTerms {
    /// 构造条款块；空引用直接拒绝，价格路径必须与摘要成对出现。
    pub fn new(reference: impl Into<String>) -> Result<Self, RuntimeError> {
        let reference = reference.into();
        if reference.trim().is_empty() {
            return Err(RuntimeError::TermsReferenceMissing);
        }
        Ok(Self {
            reference,
            price_reference: None,
            price_sha256: None,
            authorisation_reference: None,
            user_visible_notice: None,
        })
    }

    /// 绑定价格文件与其 sha256。
    pub fn with_price(mut self, path: impl Into<PathBuf>, sha256: impl Into<String>) -> Self {
        self.price_reference = Some(path.into());
        self.price_sha256 = Some(sha256.into());
        self
    }

    /// 绑定授权文件。
    pub fn with_authorisation(mut self, path: impl Into<PathBuf>) -> Self {
        self.authorisation_reference = Some(path.into());
        self
    }

    /// 记录界面提示原文。
    pub fn with_user_visible_notice(mut self, notice: impl Into<String>) -> Self {
        self.user_visible_notice = Some(notice.into());
        self
    }

    /// 返回条款引用。
    pub fn reference(&self) -> &str {
        &self.reference
    }

    /// 返回界面提示原文。
    pub fn user_visible_notice(&self) -> Option<&str> {
        self.user_visible_notice.as_deref()
    }

    /// 校验条款块：引用非空；价格文件存在且摘要一致；授权文件存在。
    pub fn verify(&self) -> Result<(), RuntimeError> {
        if self.reference.trim().is_empty() {
            return Err(RuntimeError::TermsReferenceMissing);
        }
        if let Some(path) = &self.price_reference {
            if !path.is_file() {
                return Err(RuntimeError::PriceDocumentMissing(path.clone()));
            }
            let expected = self.price_sha256.clone().ok_or_else(|| {
                RuntimeError::InvalidAssetReceipt(
                    "price document must be paired with a sha256".to_owned(),
                )
            })?;
            let observed = sha256_of_file(path)?;
            if !observed.eq_ignore_ascii_case(&expected) {
                return Err(RuntimeError::PriceDocumentMismatch {
                    path: path.clone(),
                    expected,
                    observed,
                });
            }
        }
        if let Some(path) = &self.authorisation_reference {
            if !path.is_file() {
                return Err(RuntimeError::AuthorisationDocumentMissing(path.clone()));
            }
        }
        Ok(())
    }
}

/// 计算文件 sha256；失败时带上路径，方便调用方判断是权限还是文件消失。
fn sha256_of_file(path: &Path) -> Result<String, RuntimeError> {
    use sha2::{Digest, Sha256};
    use std::io::Read;
    let mut file = std::fs::File::open(path).map_err(|error| RuntimeError::Io {
        path: path.to_path_buf(),
        message: error.to_string(),
    })?;
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let read = file.read(&mut buffer).map_err(|error| RuntimeError::Io {
            path: path.to_path_buf(),
            message: error.to_string(),
        })?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

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
    /// 条款/价格/授权引用；旧收据没有该字段，因此保持可选。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    terms: Option<OfficialAssetTerms>,
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
            terms: None,
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
            terms: None,
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

    /// 挂上条款/价格/授权引用。
    pub fn with_terms(mut self, terms: OfficialAssetTerms) -> Self {
        self.terms = Some(terms);
        self
    }

    /// 返回已挂的条款块。
    pub fn terms(&self) -> Option<&OfficialAssetTerms> {
        self.terms.as_ref()
    }

    /// 严格校验条款：没有挂条款视为缺失，价格/授权文件按声明核对。
    ///
    /// `verify` / `verify_draft` 保持不变以兼容旧收据；4.2 的拒绝路径由本方法提供。
    pub fn verify_terms(&self) -> Result<(), RuntimeError> {
        match &self.terms {
            Some(terms) => terms.verify(),
            None => Err(RuntimeError::TermsReferenceMissing),
        }
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
