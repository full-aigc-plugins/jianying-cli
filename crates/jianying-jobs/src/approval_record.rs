use crate::{ApprovalBinding, ApprovalError};
use serde::{Deserialize, Serialize};

/// 可审计、带过期时间且仅可消费一次的审批记录。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApprovalRecord {
    pub approval_id: String,
    pub binding: ApprovalBinding,
    pub issued_at: u64,
    pub expires_at: u64,
    pub consumed_at: Option<u64>,
}

impl ApprovalRecord {
    /// 在给定纪元秒签发审批，便于真实运行和确定性测试共用。
    pub fn grant(
        approval_id: String,
        binding: ApprovalBinding,
        ttl_seconds: u64,
        now: u64,
    ) -> Result<Self, ApprovalError> {
        Self::validate_id(&approval_id)?;
        if ttl_seconds == 0 {
            return Err(ApprovalError::InvalidTtl);
        }
        let expires_at = now
            .checked_add(ttl_seconds)
            .ok_or(ApprovalError::TtlOverflow)?;
        Ok(Self {
            approval_id,
            binding,
            issued_at: now,
            expires_at,
            consumed_at: None,
        })
    }

    /// 校验完整绑定与有效期，并原子语义地标记为已消费。
    pub fn consume(&mut self, binding: &ApprovalBinding, now: u64) -> Result<(), ApprovalError> {
        if self.binding != *binding
            || !self.binding.is_self_consistent()
            || !binding.is_self_consistent()
        {
            return Err(ApprovalError::BindingMismatch);
        }
        if let Some(consumed_at) = self.consumed_at {
            return Err(ApprovalError::AlreadyConsumed(consumed_at));
        }
        if now >= self.expires_at {
            return Err(ApprovalError::Expired {
                expires_at: self.expires_at,
                now,
            });
        }
        self.consumed_at = Some(now);
        Ok(())
    }

    pub(crate) fn validate_id(approval_id: &str) -> Result<(), ApprovalError> {
        if approval_id.is_empty()
            || !approval_id
                .bytes()
                .all(|value| value.is_ascii_alphanumeric() || value == b'-' || value == b'_')
        {
            return Err(ApprovalError::InvalidId(approval_id.to_owned()));
        }
        Ok(())
    }
}
