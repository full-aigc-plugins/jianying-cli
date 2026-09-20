/// 本地 TTS 运行时进入商用任务前的许可证策略。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CommercialUsePolicy {
    /// 代码许可证允许集成，但必须逐一审查用户选择的模型权重和附属制品。
    AllowedAfterArtifactReview,
    /// 已知公开模型条款限制为非商业用途。
    NonCommercial,
    /// 商业使用需要上游书面许可。
    PermissionRequired,
    /// 外部运行时允许商用，但必须先确认并履行强 copyleft 网络服务义务。
    CopyleftComplianceRequired,
    /// 仅允许研究或评估用途。
    ResearchOnly,
    /// 尚未取得足够许可证证据，默认拒绝商用。
    Unknown,
}

impl CommercialUsePolicy {
    /// 返回稳定的机器可读策略标识。
    pub fn as_str(self) -> &'static str {
        match self {
            Self::AllowedAfterArtifactReview => "allowed_after_artifact_review",
            Self::NonCommercial => "non_commercial",
            Self::PermissionRequired => "permission_required",
            Self::CopyleftComplianceRequired => "copyleft_compliance_required",
            Self::ResearchOnly => "research_only",
            Self::Unknown => "unknown",
        }
    }
}
