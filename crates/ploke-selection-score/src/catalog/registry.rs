//! Static registry of source-backed mechanisms.

use super::{Exactness, LineSpan, MechanismKind, MechanismSpec, SourceRef};

/// Formal mechanism note used as the current source authority.
pub const FORMAL_NOTE: &str = "/home/brasides/wiki/queries/research/ploke-arxiv-cs-ai-2026-06-02/synthesis/formal-scoring-mechanisms.md";

macro_rules! spec {
    ($id:literal, $paper:expr, $name:literal, $kind:expr, $exact:expr, $start:literal, $end:literal) => {
        MechanismSpec {
            id: $id,
            paper_id: $paper,
            name: $name,
            kind: $kind,
            exactness: $exact,
            source: [SourceRef {
                path: FORMAL_NOTE,
                span: Some(LineSpan {
                    start: $start,
                    end: $end,
                }),
            }],
        }
    };
}

/// All source-backed mechanisms currently represented by this crate boundary.
pub static MECHANISMS: &[MechanismSpec] = &[
    spec!(
        "2606.00007-deliberative-curation",
        Some("2606.00007"),
        "Deliberative Curation reputation and governance metrics",
        MechanismKind::Formula,
        Exactness::ScopeLimited,
        56,
        132
    ),
    spec!(
        "2606.00251-capability-self-assessment",
        Some("2606.00251"),
        "Capability Self-Assessment labels and rewards",
        MechanismKind::Formula,
        Exactness::Faithful,
        134,
        198
    ),
    spec!(
        "2606.00424-weak-critics-opcd",
        Some("2606.00424"),
        "Weak Critics O-PCD filters and KL loss",
        MechanismKind::Formula,
        Exactness::Faithful,
        200,
        258
    ),
    spec!(
        "2606.00611-trace-risk-compression",
        Some("2606.00611"),
        "TRACE unsafe probability and BCE loss",
        MechanismKind::Formula,
        Exactness::Faithful,
        260,
        317
    ),
    spec!(
        "2606.00671-axiom-trust-routing",
        Some("2606.00671"),
        "AXIOM trust score and abstention routing",
        MechanismKind::Predicate,
        Exactness::Faithful,
        319,
        365
    ),
    spec!(
        "2606.01160-expected-value-alignment",
        Some("2606.01160"),
        "Expected Value Alignment anchor-token score",
        MechanismKind::Formula,
        Exactness::Faithful,
        367,
        418
    ),
    spec!(
        "2606.01351-entropy-dynamics",
        Some("2606.01351"),
        "Entropy dynamics orchestration diagnostics",
        MechanismKind::Formula,
        Exactness::Interpretive,
        420,
        473
    ),
    spec!(
        "2606.02438-planning-cost-partitioning",
        Some("2606.02438"),
        "Planning cost and saturated cost partitioning",
        MechanismKind::Formula,
        Exactness::Faithful,
        475,
        530
    ),
    spec!(
        "2606.02461-agentcl-gains",
        Some("2606.02461"),
        "AGENTCL plasticity stability generalization gains",
        MechanismKind::Metric,
        Exactness::Interpretive,
        532,
        575
    ),
    spec!(
        "2606.02488-raser-route-argmax",
        Some("2606.02488"),
        "RASER cost-aware route argmax",
        MechanismKind::Selector,
        Exactness::Interpretive,
        577,
        613
    ),
    spec!(
        "2606.02536-trait-vector-diff",
        Some("2606.02536"),
        "Behavioral trait-vector diff score",
        MechanismKind::Formula,
        Exactness::Faithful,
        615,
        663
    ),
    spec!(
        "2606.00103-game-benchmark-status",
        Some("2606.00103"),
        "Interactive executable-game benchmark logic",
        MechanismKind::Protocol,
        Exactness::Interpretive,
        669,
        719
    ),
    spec!(
        "2606.00384-vesta-model-selection",
        Some("2606.00384"),
        "VESTA metric-directed model selection",
        MechanismKind::Selector,
        Exactness::Faithful,
        721,
        758
    ),
    spec!(
        "2606.02373-harness1-state-eval",
        Some("2606.02373"),
        "Harness-1 reward components and state predicates",
        MechanismKind::ComponentSet,
        Exactness::Interpretive,
        760,
        803
    ),
    spec!(
        "2606.02449-hll-pass-rate",
        Some("2606.02449"),
        "HLL human-verification pass rate",
        MechanismKind::Predicate,
        Exactness::Interpretive,
        805,
        845
    ),
    spec!(
        "2606.02470-mcp-persona-task-success",
        Some("2606.02470"),
        "MCP-Persona personalized tool success",
        MechanismKind::Predicate,
        Exactness::Interpretive,
        847,
        887
    ),
    spec!(
        "2606.02484-iteris-acceptance",
        Some("2606.02484"),
        "Iteris verified output acceptance",
        MechanismKind::Predicate,
        Exactness::Interpretive,
        889,
        931
    ),
    spec!(
        "2606.01066-verifier-metrics-lead",
        Some("2606.01066"),
        "Verifier metrics candidate lead",
        MechanismKind::Unresolved,
        Exactness::NotFormalizable,
        958,
        976
    ),
];

/// Returns all catalog entries.
pub fn all() -> &'static [MechanismSpec] {
    MECHANISMS
}

/// Looks up one catalog entry by id.
pub fn by_id(id: &str) -> Option<&'static MechanismSpec> {
    MECHANISMS.iter().find(|spec| spec.id == id)
}
