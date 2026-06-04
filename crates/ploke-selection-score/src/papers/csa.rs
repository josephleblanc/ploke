//! 2606.00251 — Capability self-assessment scoring helpers.

/// Capability self-assessment route label.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Label {
    SelfSolve,
    Delegate,
}

/// Converts an aggregation predicate result into a CSA label.
pub fn label(can_self_solve: bool) -> Label {
    if can_self_solve {
        Label::SelfSolve
    } else {
        Label::Delegate
    }
}

/// Binary reward for matching the target CSA label.
pub fn binary_reward(predicted: Label, target: Label) -> i8 {
    if predicted == target { 1 } else { -1 }
}

/// True when a rollout group contains both CSA labels.
pub fn has_label_diversity(label: &[Label]) -> bool {
    label.contains(&Label::SelfSolve) && label.contains(&Label::Delegate)
}
