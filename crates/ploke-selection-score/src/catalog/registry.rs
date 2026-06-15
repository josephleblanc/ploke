//! Static registry of source-backed mechanisms.

use super::{Exactness, MechanismKind, MechanismSpec, SourceRef};

/// Formal mechanism note used as the current source authority.
pub const FORMAL_NOTE: &str = "/home/brasides/wiki/queries/research/ploke-arxiv-cs-ai-2026-06-02/synthesis/formal-scoring-mechanisms.md";

/// SADN setup/algorithm equation note used as the source authority.
pub const SADN_ALGORITHM_NOTE: &str = "/home/brasides/wiki/queries/ploke/selection-scoring/sadn-equations/setup-and-algorithm-equations.md";

/// Archive parent-selection note used as the source authority for DGM-H / HyperAgents.
pub const ARCHIVE_PARENT_NOTE: &str =
    "/home/brasides/wiki/queries/ploke/selection-scoring/archive-parent-selection-mechanisms.md";

/// Nearby-mechanism ledger for candidates that are tracked but not executable formulas yet.
pub const NEARBY_MECHANISM_LEDGER: &str =
    "/home/brasides/wiki/queries/ploke/selection-scoring/ledgers/mechanism-ledger.md";

macro_rules! spec {
    ($id:literal, Some($paper:literal), $name:literal, $kind:expr, $exact:expr) => {
        MechanismSpec {
            id: $id,
            paper_id: Some($paper),
            name: $name,
            kind: $kind,
            exactness: $exact,
            source: [SourceRef {
                path: FORMAL_NOTE,
                section_key: $paper,
            }],
        }
    };
    ($id:literal, None, $name:literal, $kind:expr, $exact:expr) => {
        MechanismSpec {
            id: $id,
            paper_id: None,
            name: $name,
            kind: $kind,
            exactness: $exact,
            source: [SourceRef {
                path: FORMAL_NOTE,
                section_key: $id,
            }],
        }
    };
}

macro_rules! nearby_candidate {
    ($id:literal, $paper:literal, $name:literal) => {
        MechanismSpec {
            id: $id,
            paper_id: Some($paper),
            name: $name,
            kind: MechanismKind::Unresolved,
            exactness: Exactness::NotFormalizable,
            source: [SourceRef {
                path: NEARBY_MECHANISM_LEDGER,
                section_key: $paper,
            }],
        }
    };
}

/// All source-backed mechanisms currently represented by this crate boundary.
pub static MECHANISMS: &[MechanismSpec] = &[
    MechanismSpec {
        id: "2603.19461-dgmh-parent-selection",
        paper_id: Some("2603.19461"),
        name: "DGM-H score-child-prop parent selection",
        kind: MechanismKind::Selector,
        exactness: Exactness::Faithful,
        source: [SourceRef {
            path: ARCHIVE_PARENT_NOTE,
            section_key: "2603.19461",
        }],
    },
    MechanismSpec {
        id: "2603.19461-dgmh-archive-admission",
        paper_id: Some("2603.19461"),
        name: "DGM-H valid-child archive admission",
        kind: MechanismKind::Predicate,
        exactness: Exactness::Faithful,
        source: [SourceRef {
            path: ARCHIVE_PARENT_NOTE,
            section_key: "2603.19461",
        }],
    },
    MechanismSpec {
        id: "2603.19461-dgmh-staged-cross-domain",
        paper_id: Some("2603.19461"),
        name: "DGM-H staged evaluation and cross-domain average",
        kind: MechanismKind::Metric,
        exactness: Exactness::Faithful,
        source: [SourceRef {
            path: ARCHIVE_PARENT_NOTE,
            section_key: "2603.19461",
        }],
    },
    MechanismSpec {
        id: "2603.19461-dgmh-modifiable-parent-selection",
        paper_id: Some("2603.19461"),
        name: "DGM-H modifiable parent-selection sketches",
        kind: MechanismKind::Selector,
        exactness: Exactness::Interpretive,
        source: [SourceRef {
            path: ARCHIVE_PARENT_NOTE,
            section_key: "2603.19461",
        }],
    },
    spec!(
        "2606.00007-deliberative-curation",
        Some("2606.00007"),
        "Deliberative Curation reputation and governance metrics",
        MechanismKind::Formula,
        Exactness::ScopeLimited
    ),
    spec!(
        "2606.00251-capability-self-assessment",
        Some("2606.00251"),
        "Capability Self-Assessment labels and rewards",
        MechanismKind::Formula,
        Exactness::Faithful
    ),
    spec!(
        "2606.00424-weak-critics-opcd",
        Some("2606.00424"),
        "Weak Critics O-PCD filters and KL loss",
        MechanismKind::Formula,
        Exactness::Faithful
    ),
    spec!(
        "2606.00611-trace-risk-compression",
        Some("2606.00611"),
        "TRACE unsafe probability and BCE loss",
        MechanismKind::Formula,
        Exactness::Faithful
    ),
    spec!(
        "2606.00671-axiom-trust-routing",
        Some("2606.00671"),
        "AXIOM trust score and abstention routing",
        MechanismKind::Predicate,
        Exactness::Faithful
    ),
    spec!(
        "2606.01160-expected-value-alignment",
        Some("2606.01160"),
        "Expected Value Alignment anchor-token score",
        MechanismKind::Formula,
        Exactness::Faithful
    ),
    spec!(
        "2606.01351-entropy-dynamics",
        Some("2606.01351"),
        "Entropy dynamics orchestration diagnostics",
        MechanismKind::Formula,
        Exactness::Interpretive
    ),
    spec!(
        "2606.02438-planning-cost-partitioning",
        Some("2606.02438"),
        "Planning cost and saturated cost partitioning",
        MechanismKind::Formula,
        Exactness::Faithful
    ),
    spec!(
        "2606.02461-agentcl-gains",
        Some("2606.02461"),
        "AGENTCL plasticity stability generalization gains",
        MechanismKind::Metric,
        Exactness::Interpretive
    ),
    spec!(
        "2606.02488-raser-route-argmax",
        Some("2606.02488"),
        "RASER cost-aware route argmax",
        MechanismKind::Selector,
        Exactness::Interpretive
    ),
    spec!(
        "2606.02536-trait-vector-diff",
        Some("2606.02536"),
        "Behavioral trait-vector diff score",
        MechanismKind::Formula,
        Exactness::Faithful
    ),
    MechanismSpec {
        id: "2510.23535-sadn-sequential-advantage",
        paper_id: Some("2510.23535"),
        name: "SADN sequential advantage decomposition and greedy IGM",
        kind: MechanismKind::ComponentSet,
        exactness: Exactness::ScopeLimited,
        source: [SourceRef {
            path: SADN_ALGORITHM_NOTE,
            section_key: "2510.23535",
        }],
    },
    spec!(
        "2606.00103-game-benchmark-status",
        Some("2606.00103"),
        "Interactive executable-game benchmark logic",
        MechanismKind::Protocol,
        Exactness::Interpretive
    ),
    spec!(
        "2606.00384-vesta-model-selection",
        Some("2606.00384"),
        "VESTA metric-directed model selection",
        MechanismKind::Selector,
        Exactness::Faithful
    ),
    spec!(
        "2606.02373-harness1-state-eval",
        Some("2606.02373"),
        "Harness-1 reward components and state predicates",
        MechanismKind::ComponentSet,
        Exactness::Interpretive
    ),
    spec!(
        "2606.02449-hll-pass-rate",
        Some("2606.02449"),
        "HLL human-verification pass rate",
        MechanismKind::Predicate,
        Exactness::Interpretive
    ),
    spec!(
        "2606.02470-mcp-persona-task-success",
        Some("2606.02470"),
        "MCP-Persona personalized tool success",
        MechanismKind::Predicate,
        Exactness::Interpretive
    ),
    spec!(
        "2606.02484-iteris-acceptance",
        Some("2606.02484"),
        "Iteris verified output acceptance",
        MechanismKind::Predicate,
        Exactness::Interpretive
    ),
    spec!(
        "2606.01066-verifier-metrics-lead",
        Some("2606.01066"),
        "Verifier metrics candidate lead",
        MechanismKind::Unresolved,
        Exactness::NotFormalizable
    ),
    nearby_candidate!(
        "2606.03056-skilldag-typed-skill-graph",
        "2606.03056",
        "SkillDAG typed skill-graph admission candidate"
    ),
    nearby_candidate!(
        "2606.03083-deltamem-residual-tree-write-rule",
        "2606.03083",
        "DELTAMEM residual-tree write-rule candidate"
    ),
    nearby_candidate!(
        "2606.03467-stepfinder-step-attribution",
        "2606.03467",
        "StepFinder step-attribution candidate"
    ),
    nearby_candidate!(
        "2606.02875-handoff-debt-rediscovery-cost",
        "2606.02875",
        "Handoff Debt rediscovery-cost candidate"
    ),
    nearby_candidate!(
        "2606.02994-reasoning-primitive-induction",
        "2606.02994",
        "Reasoning primitive induction candidate"
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
