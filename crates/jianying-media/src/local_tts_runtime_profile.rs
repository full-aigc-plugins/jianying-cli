use crate::{CommercialUsePolicy, LocalTtsRuntimeKind, LocalTtsTransport, TtsError};

/// 单个本地开源 TTS 运行时的来源、协议和许可证门禁档案。
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LocalTtsRuntimeProfile {
    kind: LocalTtsRuntimeKind,
    upstream_repository: &'static str,
    code_license: &'static str,
    model_license: &'static str,
    commercial_use_policy: CommercialUsePolicy,
    transport: LocalTtsTransport,
    default_endpoint: Option<&'static str>,
}

impl LocalTtsRuntimeProfile {
    /// 创建内置运行时档案；模型权重始终由用户单独提供，不随 CLI 分发。
    pub(crate) const fn new(
        kind: LocalTtsRuntimeKind,
        upstream_repository: &'static str,
        code_license: &'static str,
        model_license: &'static str,
        commercial_use_policy: CommercialUsePolicy,
        transport: LocalTtsTransport,
        default_endpoint: Option<&'static str>,
    ) -> Self {
        Self {
            kind,
            upstream_repository,
            code_license,
            model_license,
            commercial_use_policy,
            transport,
            default_endpoint,
        }
    }

    /// 返回运行时种类。
    pub fn kind(&self) -> LocalTtsRuntimeKind {
        self.kind
    }

    /// 返回稳定的机器可读标识。
    pub fn id(&self) -> &'static str {
        self.kind.as_str()
    }

    /// 返回上游官方仓库地址。
    pub fn upstream_repository(&self) -> &'static str {
        self.upstream_repository
    }

    /// 返回运行时代码许可证的 SPDX 标识或上游许可证名称。
    pub fn code_license(&self) -> &'static str {
        self.code_license
    }

    /// 返回公开模型权重的许可证结论或逐制品审查要求。
    pub fn model_license(&self) -> &'static str {
        self.model_license
    }

    /// 返回商业使用策略。
    pub fn commercial_use_policy(&self) -> CommercialUsePolicy {
        self.commercial_use_policy
    }

    /// 返回本地运行时传输协议。
    pub fn transport(&self) -> LocalTtsTransport {
        self.transport
    }

    /// 返回仅绑定回环地址的默认 endpoint；进程型运行时返回空。
    pub fn default_endpoint(&self) -> Option<&'static str> {
        self.default_endpoint
    }

    /// 返回是否随 CLI 分发模型权重；内置档案固定为否。
    pub fn bundles_model_weights(&self) -> bool {
        false
    }

    /// 校验运行时能否进入商用任务。
    ///
    /// `model_artifact_reviewed` 表示调用方已对实际选择的 checkpoint、声库和附属制品完成
    /// 来源、哈希及许可证审查，不能用代码仓库许可证替代。
    pub fn ensure_commercial_use(&self, model_artifact_reviewed: bool) -> Result<(), TtsError> {
        match self.commercial_use_policy {
            CommercialUsePolicy::AllowedAfterArtifactReview if model_artifact_reviewed => Ok(()),
            CommercialUsePolicy::AllowedAfterArtifactReview => {
                Err(TtsError::MissingModelLicenseReview {
                    runtime: self.id().to_owned(),
                })
            }
            policy => Err(TtsError::CommercialUseDenied {
                runtime: self.id().to_owned(),
                policy: policy.as_str().to_owned(),
            }),
        }
    }
}
