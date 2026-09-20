use crate::AsrLedgerRecord;

/// ASR 幂等门禁决策：提交一次或复用已验证转录。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AsrLedgerDecision {
    Submit(AsrLedgerRecord),
    Reuse(AsrLedgerRecord),
}
