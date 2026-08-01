//! 2606.02484 — Iteris computational-math loop.

/// Source-supported accepted output classes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputKind {
    PhaseDiagram,
    QrcpCounterexample,
    VerifiedProof,
}

/// Accepted output predicate.
pub fn accept(output: Option<OutputKind>, human_verified: bool) -> bool {
    output.is_some() && human_verified
}
