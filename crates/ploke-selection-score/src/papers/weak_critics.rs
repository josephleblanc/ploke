//! 2606.00424 — Weak Critics / O-PCD filtering helpers.

/// Product filter `h_i = r_out * r_rub` as a boolean predicate.
pub fn retain_example(outcome_correct: bool, rubric_useful: bool) -> bool {
    outcome_correct && rubric_useful
}
