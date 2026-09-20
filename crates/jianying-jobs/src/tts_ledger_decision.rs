use crate::TtsLedgerRecord;

/// 提交门禁决策：发起一次新请求或复用已验证制品。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TtsLedgerDecision {
    Submit(TtsLedgerRecord),
    Reuse(TtsLedgerRecord),
}
