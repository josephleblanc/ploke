//! 2606.02373 — Harness-1 state-externalizing search harness.

/// Source-supported reward component names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RewardComponent {
    SetRecall,
    AnswerDocRecall,
    TrajRelevantRecall,
    TrajAnswerRecall,
    ToolDiversity,
    AnswerFoundBonus,
    TurnPenalty,
}

/// Keep predicate for retrieved documents.
pub fn keep_doc(relevant: bool, evidence_linked: bool) -> bool {
    relevant && evidence_linked
}

/// Stop predicate for a search trajectory.
pub fn should_stop(answer_found: bool, budget_exhausted: bool) -> bool {
    answer_found || budget_exhausted
}
