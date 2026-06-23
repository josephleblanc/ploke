//! Source-derived Prototype 1 transition inventory.
#![cfg_attr(not(test), allow(dead_code))]
//!
//! The rows here are migration-test metadata, not runtime authority. Parent
//! rows are derived from [`WalkPhase::next_steps`] so a new walk edge must be
//! classified before the inventory can render.

use serde::Serialize;

use super::walk::phase::WalkPhase;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum AuthorityClass {
    EvidenceProjection,
    History,
    Channel,
    MessageBox,
    Bootstrap,
    Artifact,
}

impl AuthorityClass {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::EvidenceProjection => "evidence_projection",
            Self::History => "history",
            Self::Channel => "channel",
            Self::MessageBox => "message_box",
            Self::Bootstrap => "bootstrap",
            Self::Artifact => "artifact",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum LiveApi {
    No,
    DirectGoogle,
    ProviderChild,
}

impl LiveApi {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::No => "no",
            Self::DirectGoogle => "direct_google",
            Self::ProviderChild => "provider_child",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct TransitionInventoryRow {
    pub(crate) edge_id: &'static str,
    pub(crate) source_anchor: &'static str,
    pub(crate) from_phase: &'static str,
    pub(crate) to_phase: &'static str,
    pub(crate) producer_surfaces: &'static [&'static str],
    pub(crate) consumer_surfaces: &'static [&'static str],
    pub(crate) authority_classes: &'static [AuthorityClass],
    pub(crate) live_api: LiveApi,
    pub(crate) checkpoint_in: &'static str,
    pub(crate) checkpoint_out: &'static str,
    pub(crate) downstream: &'static [&'static str],
    pub(crate) authority_negative_case: &'static str,
}

pub(crate) fn prototype1_transition_inventory_rows() -> Vec<TransitionInventoryRow> {
    let mut rows = Vec::new();
    for phase in parent_source_phases() {
        for step in phase.next_steps() {
            rows.push(parent_transition_row(*phase, step.phase, step.edge));
        }
    }
    rows.extend(child_transition_rows());
    rows
}

pub(crate) fn render_transition_inventory_markdown(rows: &[TransitionInventoryRow]) -> String {
    let mut out = String::from(
        "# Prototype 1 Transition Inventory\n\n\
Generated from `prototype1_state::transition_inventory`. Update the source\n\
classification and rerender this file when a transition splits, merges, or\n\
changes its authority/checkpoint contract.\n\n",
    );
    out.push_str(&format!("Generated row count: {}\n\n", rows.len()));
    out.push_str("| edge_id | from | to | source | live_api | checkpoint | authority | producers | consumers | negative case |\n");
    out.push_str("| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |\n");
    for row in rows {
        out.push_str(&format!(
            "| `{}` | `{}` | `{}` | `{}` | `{}` | `{} -> {}` | {} | {} | {} | {} |\n",
            row.edge_id,
            row.from_phase,
            row.to_phase,
            row.source_anchor,
            row.live_api.as_str(),
            row.checkpoint_in,
            row.checkpoint_out,
            format_classes(row.authority_classes),
            format_terms(row.producer_surfaces),
            format_terms(row.consumer_surfaces),
            row.authority_negative_case,
        ));
    }
    out
}

fn parent_source_phases() -> &'static [WalkPhase] {
    &[
        WalkPhase::Empty,
        WalkPhase::R0,
        WalkPhase::R1,
        WalkPhase::R3,
        WalkPhase::R4a,
        WalkPhase::R4b,
        WalkPhase::R4c,
        WalkPhase::R5,
        WalkPhase::R6,
        WalkPhase::R7,
        WalkPhase::R8,
        WalkPhase::R9,
        WalkPhase::R10,
        WalkPhase::R11a,
        WalkPhase::R11,
        WalkPhase::R12,
        WalkPhase::R13a,
        WalkPhase::R13b,
    ]
}

fn parent_transition_row(
    from: WalkPhase,
    to: WalkPhase,
    edge: &'static str,
) -> TransitionInventoryRow {
    match (from, to, edge) {
        (WalkPhase::Empty, WalkPhase::R0, "start") => TransitionInventoryRow {
            edge_id: "start_to_r0",
            source_anchor: "walk/phase.rs:EMPTY_NEXT",
            from_phase: "empty",
            to_phase: "r0",
            producer_surfaces: &["command carrier"],
            consumer_surfaces: &["r0_to_r1"],
            authority_classes: &[AuthorityClass::Bootstrap],
            live_api: LiveApi::No,
            checkpoint_in: "F0_setup",
            checkpoint_out: "F0_setup",
            downstream: &["startup resolution"],
            authority_negative_case: "command carrier alone cannot admit a parent",
        },
        (WalkPhase::R0, WalkPhase::R1, "r0_to_r1") => TransitionInventoryRow {
            edge_id: "r0_to_r1",
            source_anchor: "live_edges.rs:prototype1_live_edge_r0_to_r1",
            from_phase: "r0",
            to_phase: "r1",
            producer_surfaces: &[
                "campaign manifest path",
                "run shape",
                "transition journal handle",
                "active monitor target",
            ],
            consumer_surfaces: &["parent identity resolution", "profile validation"],
            authority_classes: &[
                AuthorityClass::Bootstrap,
                AuthorityClass::EvidenceProjection,
            ],
            live_api: LiveApi::No,
            checkpoint_in: "F0_setup",
            checkpoint_out: "F0_setup",
            downstream: &["r1_to_r2a_or_r3", "policy/profile consumers"],
            authority_negative_case: "projection rows cannot replace campaign/profile commitment",
        },
        (WalkPhase::R1, WalkPhase::R2a, "r1_to_r2a_or_r3") => TransitionInventoryRow {
            edge_id: "r1_to_r2a",
            source_anchor: "live_edges.rs:prototype1_live_edge_r1_to_r2a_or_r3",
            from_phase: "r1",
            to_phase: "r2a",
            producer_surfaces: &["parent_identity.json", "active checkout commit"],
            consumer_surfaces: &["operator setup report"],
            authority_classes: &[AuthorityClass::Bootstrap, AuthorityClass::Artifact],
            live_api: LiveApi::No,
            checkpoint_in: "F0_setup",
            checkpoint_out: "F0_setup",
            downstream: &["normal parent startup after setup"],
            authority_negative_case: "DB identity row cannot replace committed parent identity file",
        },
        (WalkPhase::R1, WalkPhase::R3, "r1_to_r2a_or_r3") => TransitionInventoryRow {
            edge_id: "r1_to_r3",
            source_anchor: "live_edges.rs:prototype1_live_edge_r1_to_r2a_or_r3",
            from_phase: "r1",
            to_phase: "r3",
            producer_surfaces: &["resolved parent identity fact"],
            consumer_surfaces: &["r3_to_r4a"],
            authority_classes: &[AuthorityClass::Bootstrap, AuthorityClass::Artifact],
            live_api: LiveApi::No,
            checkpoint_in: "F0_setup",
            checkpoint_out: "F0_setup",
            downstream: &["startup validation"],
            authority_negative_case: "DB identity row cannot bypass active checkout or invocation validation",
        },
        (WalkPhase::R3, WalkPhase::R4a, "r3_to_r4a") => TransitionInventoryRow {
            edge_id: "r3_to_r4a",
            source_anchor: "live_edges.rs:prototype1_live_edge_r3_to_r4a",
            from_phase: "r3",
            to_phase: "r4a",
            producer_surfaces: &["Parent<Unchecked> carrier"],
            consumer_surfaces: &["r4a_to_r4b_or_r4c"],
            authority_classes: &[AuthorityClass::Bootstrap, AuthorityClass::Artifact],
            live_api: LiveApi::No,
            checkpoint_in: "F0_setup",
            checkpoint_out: "F0_setup",
            downstream: &["genesis/predecessor startup branch"],
            authority_negative_case: "unchecked parent cannot run child planning",
        },
        (WalkPhase::R4a, WalkPhase::R4b, "r4a_to_r4b_or_r4c") => TransitionInventoryRow {
            edge_id: "r4a_to_r4b",
            source_anchor: "live_edges.rs:prototype1_live_edge_r4a_to_r4b_or_r4c",
            from_phase: "r4a",
            to_phase: "r4b",
            producer_surfaces: &["genesis startup proof"],
            consumer_surfaces: &["r4b_to_r4c_genesis"],
            authority_classes: &[
                AuthorityClass::Bootstrap,
                AuthorityClass::Artifact,
                AuthorityClass::History,
            ],
            live_api: LiveApi::No,
            checkpoint_in: "F0_setup",
            checkpoint_out: "F1_ready_parent",
            downstream: &["ready parent convergence"],
            authority_negative_case: "parent-start DB row cannot make genesis startup valid",
        },
        (WalkPhase::R4a, WalkPhase::R4c, "r4a_to_r4b_or_r4c") => TransitionInventoryRow {
            edge_id: "r4a_to_r4c",
            source_anchor: "live_edges.rs:prototype1_live_edge_r4a_to_r4b_or_r4c",
            from_phase: "r4a",
            to_phase: "r4c",
            producer_surfaces: &["successor-ready record", "predecessor startup proof"],
            consumer_surfaces: &["r4c_to_r5"],
            authority_classes: &[
                AuthorityClass::History,
                AuthorityClass::Artifact,
                AuthorityClass::Bootstrap,
            ],
            live_api: LiveApi::No,
            checkpoint_in: "F8_handoff_committed",
            checkpoint_out: "F1_ready_parent",
            downstream: &["successor parent-start evidence"],
            authority_negative_case: "DB row cannot replace successor invocation or sealed History proof",
        },
        (WalkPhase::R4b, WalkPhase::R4c, "r4b_to_r4c_genesis") => TransitionInventoryRow {
            edge_id: "r4b_to_r4c",
            source_anchor: "live_edges.rs:prototype1_live_edge_r4b_to_r4c_genesis",
            from_phase: "r4b",
            to_phase: "r4c",
            producer_surfaces: &["Parent<Ready> carrier"],
            consumer_surfaces: &["r4c_to_r5"],
            authority_classes: &[
                AuthorityClass::Bootstrap,
                AuthorityClass::History,
                AuthorityClass::Artifact,
            ],
            live_api: LiveApi::No,
            checkpoint_in: "F0_setup",
            checkpoint_out: "F1_ready_parent",
            downstream: &["parent-start evidence"],
            authority_negative_case: "ready carrier cannot be reconstructed from DB evidence alone",
        },
        (WalkPhase::R4c, WalkPhase::R5, "r4c_to_r5") => TransitionInventoryRow {
            edge_id: "r4c_to_r5",
            source_anchor: "live_edges.rs:prototype1_live_edge_r4c_to_r5",
            from_phase: "r4c",
            to_phase: "r5",
            producer_surfaces: &["JournalEntry::ParentStarted", "Resource(parent_start)"],
            consumer_surfaces: &["transition replay", "metrics", "history preview"],
            authority_classes: &[AuthorityClass::EvidenceProjection],
            live_api: LiveApi::No,
            checkpoint_in: "F1_ready_parent",
            checkpoint_out: "post_r5_parent_started",
            downstream: &["r5_to_r6", "parent-start replay consumers"],
            authority_negative_case: "parent-start evidence cannot bypass R4a/R4b startup checks",
        },
        (WalkPhase::R5, WalkPhase::R6, "r5_to_r6") => TransitionInventoryRow {
            edge_id: "r5_to_r6",
            source_anchor: "live_edges.rs:prototype1_live_edge_r5_to_r6",
            from_phase: "r5",
            to_phase: "r6",
            producer_surfaces: &["parent baseline facts"],
            consumer_surfaces: &["policy budget", "child planning"],
            authority_classes: &[AuthorityClass::EvidenceProjection],
            live_api: LiveApi::No,
            checkpoint_in: "post_r5_parent_started",
            checkpoint_out: "F2_baseline_complete",
            downstream: &["r6_to_r7", "baseline metrics"],
            authority_negative_case: "baseline projection cannot replace artifact/history startup proof",
        },
        (WalkPhase::R6, WalkPhase::R7, "r6_to_r7") => TransitionInventoryRow {
            edge_id: "r6_to_r7",
            source_anchor: "live_edges.rs:prototype1_live_edge_r6_to_r7",
            from_phase: "r6",
            to_phase: "r7",
            producer_surfaces: &["policy budget fact"],
            consumer_surfaces: &["r7_to_r8"],
            authority_classes: &[AuthorityClass::EvidenceProjection],
            live_api: LiveApi::No,
            checkpoint_in: "F2_baseline_complete",
            checkpoint_out: "F2_baseline_complete",
            downstream: &["child-plan producer"],
            authority_negative_case: "scheduler fallback cannot override admitted profile policy",
        },
        (WalkPhase::R7, WalkPhase::R8, "r7_to_r8 --watch") => TransitionInventoryRow {
            edge_id: "r7_to_r8",
            source_anchor: "live_edges.rs:prototype1_live_edge_r7_to_r8",
            from_phase: "r7",
            to_phase: "r8",
            producer_surfaces: &[
                "child-plan MessageBox",
                "edit harness request",
                "planned child refs",
            ],
            consumer_surfaces: &["r8_to_r9", "C1 materialization"],
            authority_classes: &[
                AuthorityClass::MessageBox,
                AuthorityClass::EvidenceProjection,
            ],
            live_api: LiveApi::DirectGoogle,
            checkpoint_in: "F2_baseline_complete",
            checkpoint_out: "F3_child_plan_received",
            downstream: &["schedule shaping", "child materialization"],
            authority_negative_case: "DB child-plan row cannot replace MessageBox body/hash",
        },
        (WalkPhase::R8, WalkPhase::R9, "r8_to_r9") => TransitionInventoryRow {
            edge_id: "r8_to_r9",
            source_anchor: "live_edges.rs:prototype1_live_edge_r8_to_r9",
            from_phase: "r8",
            to_phase: "r9",
            producer_surfaces: &["child schedule fact", "planned child count"],
            consumer_surfaces: &["selection strategy", "child fanout"],
            authority_classes: &[AuthorityClass::EvidenceProjection],
            live_api: LiveApi::No,
            checkpoint_in: "F3_child_plan_received",
            checkpoint_out: "F3_child_plan_received",
            downstream: &["r9_to_r10", "r10_to_r11"],
            authority_negative_case: "schedule projection cannot create missing child-plan authority",
        },
        (WalkPhase::R9, WalkPhase::R10, "r9_to_r10") => TransitionInventoryRow {
            edge_id: "r9_to_r10",
            source_anchor: "live_edges.rs:prototype1_live_edge_r9_to_r10",
            from_phase: "r9",
            to_phase: "r10",
            producer_surfaces: &["successor-selection strategy fact"],
            consumer_surfaces: &["r10_to_r11"],
            authority_classes: &[AuthorityClass::EvidenceProjection],
            live_api: LiveApi::No,
            checkpoint_in: "F3_child_plan_received",
            checkpoint_out: "F3_child_plan_received",
            downstream: &["rejected-only branch", "child fanout branch"],
            authority_negative_case: "strategy projection cannot select without candidate evidence",
        },
        (WalkPhase::R10, WalkPhase::R11a, "r10_to_r11 --watch") => TransitionInventoryRow {
            edge_id: "r10_to_r11a",
            source_anchor: "live_edges.rs:prototype1_live_edge_r10_to_r11",
            from_phase: "r10",
            to_phase: "r11a",
            producer_surfaces: &["rejected-attempt evidence", "selection-ready projection"],
            consumer_surfaces: &["r11_to_r12"],
            authority_classes: &[AuthorityClass::EvidenceProjection],
            live_api: LiveApi::No,
            checkpoint_in: "F3_child_plan_received",
            checkpoint_out: "F7_selection_ready",
            downstream: &["report projection", "stopped continuation"],
            authority_negative_case: "rejected-only projection cannot prove child terminal success",
        },
        (WalkPhase::R10, WalkPhase::R11, "r10_to_r11 --watch") => TransitionInventoryRow {
            edge_id: "r10_to_r11",
            source_anchor: "live_edges.rs:prototype1_live_edge_r10_to_r11",
            from_phase: "r10",
            to_phase: "r11",
            producer_surfaces: &[
                "child runtime records",
                "runner result",
                "channel result",
                "evaluation refs",
            ],
            consumer_surfaces: &["r11_to_r12", "selection audit"],
            authority_classes: &[
                AuthorityClass::Channel,
                AuthorityClass::MessageBox,
                AuthorityClass::EvidenceProjection,
            ],
            live_api: LiveApi::ProviderChild,
            checkpoint_in: "F3_child_plan_received",
            checkpoint_out: "F5_child_terminal_result",
            downstream: &["comparison", "selection", "continuation"],
            authority_negative_case: "DB result row cannot replace channel terminal result/body hash",
        },
        (WalkPhase::R11a, WalkPhase::R12, "r11_to_r12") => TransitionInventoryRow {
            edge_id: "r11a_to_r12",
            source_anchor: "live_edges.rs:prototype1_live_edge_r11_to_r12",
            from_phase: "r11a",
            to_phase: "r12",
            producer_surfaces: &["report facts", "no-candidate evidence"],
            consumer_surfaces: &["r12_to_r13"],
            authority_classes: &[AuthorityClass::EvidenceProjection],
            live_api: LiveApi::No,
            checkpoint_in: "F7_selection_ready",
            checkpoint_out: "F7_selection_ready",
            downstream: &["stopped continuation"],
            authority_negative_case: "report facts cannot seal History without continuation decision",
        },
        (WalkPhase::R11, WalkPhase::R12, "r11_to_r12") => TransitionInventoryRow {
            edge_id: "r11_to_r12",
            source_anchor: "live_edges.rs:prototype1_live_edge_r11_to_r12",
            from_phase: "r11",
            to_phase: "r12",
            producer_surfaces: &["report facts", "candidate selection evidence"],
            consumer_surfaces: &["r12_to_r13"],
            authority_classes: &[AuthorityClass::EvidenceProjection],
            live_api: LiveApi::No,
            checkpoint_in: "F6_child_compared",
            checkpoint_out: "F7_selection_ready",
            downstream: &["continuation decision", "handoff branch"],
            authority_negative_case: "selection projection cannot advance active checkout alone",
        },
        (WalkPhase::R12, WalkPhase::R13a, "r12_to_r13") => TransitionInventoryRow {
            edge_id: "r12_to_r13a",
            source_anchor: "live_edges.rs:prototype1_live_edge_r12_to_r13",
            from_phase: "r12",
            to_phase: "r13a",
            producer_surfaces: &["stopped continuation journal", "final report inputs"],
            consumer_surfaces: &["r13_to_r14"],
            authority_classes: &[AuthorityClass::EvidenceProjection, AuthorityClass::History],
            live_api: LiveApi::No,
            checkpoint_in: "F7_selection_ready",
            checkpoint_out: "stopped_continuation",
            downstream: &["final stopped report"],
            authority_negative_case: "DB continuation row cannot replace sealed stopped decision evidence",
        },
        (WalkPhase::R12, WalkPhase::R13b, "r12_to_r13 --watch --allow git-changes") => {
            TransitionInventoryRow {
                edge_id: "r12_to_r13b",
                source_anchor: "live_edges.rs:prototype1_live_edge_r12_to_r13",
                from_phase: "r12",
                to_phase: "r13b",
                producer_surfaces: &[
                    "sealed History block",
                    "selected artifact install",
                    "successor invocation",
                    "successor ready",
                ],
                consumer_surfaces: &["successor startup", "handoff final report"],
                authority_classes: &[
                    AuthorityClass::History,
                    AuthorityClass::Artifact,
                    AuthorityClass::Channel,
                    AuthorityClass::Bootstrap,
                ],
                live_api: LiveApi::No,
                checkpoint_in: "F7_selection_ready",
                checkpoint_out: "F8_handoff_committed",
                downstream: &["successor R4a/R4c startup", "r13_to_r14"],
                authority_negative_case: "DB handoff row cannot replace sealed History or selected artifact install",
            }
        }
        (WalkPhase::R13a, WalkPhase::R14a, "r13_to_r14") => TransitionInventoryRow {
            edge_id: "r13a_to_r14a",
            source_anchor: "live_edges.rs:prototype1_live_edge_r13_to_r14",
            from_phase: "r13a",
            to_phase: "r14a",
            producer_surfaces: &["final stopped report"],
            consumer_surfaces: &["operator review"],
            authority_classes: &[AuthorityClass::EvidenceProjection],
            live_api: LiveApi::No,
            checkpoint_in: "stopped_continuation",
            checkpoint_out: "stopped_report_emitted",
            downstream: &["report queries"],
            authority_negative_case: "report file cannot retrofit earlier authority failures",
        },
        (WalkPhase::R13b, WalkPhase::R14b, "r13_to_r14") => TransitionInventoryRow {
            edge_id: "r13b_to_r14b",
            source_anchor: "live_edges.rs:prototype1_live_edge_r13_to_r14",
            from_phase: "r13b",
            to_phase: "r14b",
            producer_surfaces: &["final handoff report"],
            consumer_surfaces: &["operator review", "successor audit"],
            authority_classes: &[AuthorityClass::EvidenceProjection],
            live_api: LiveApi::No,
            checkpoint_in: "F8_handoff_committed",
            checkpoint_out: "handoff_report_emitted",
            downstream: &["successor startup diagnostics"],
            authority_negative_case: "final report cannot replace successor ready/invocation authority",
        },
        _ => panic!(
            "unclassified Prototype 1 walk transition: {} -> {} via {}",
            from.as_str(),
            to.as_str(),
            edge
        ),
    }
}

fn child_transition_rows() -> Vec<TransitionInventoryRow> {
    vec![
        TransitionInventoryRow {
            edge_id: "c1_to_c2",
            source_anchor: "c1.rs:prototype1_c1_to_c2",
            from_phase: "c1",
            to_phase: "c2",
            producer_surfaces: &[
                "child workspace",
                "materialization journal",
                "runner workspace root",
            ],
            consumer_surfaces: &["c2_to_c3", "build/artifact checks"],
            authority_classes: &[AuthorityClass::Artifact, AuthorityClass::EvidenceProjection],
            live_api: LiveApi::No,
            checkpoint_in: "F3_child_plan_received",
            checkpoint_out: "materialized_child",
            downstream: &["child build", "artifact provenance"],
            authority_negative_case: "DB artifact ref cannot prove workspace materialization",
        },
        TransitionInventoryRow {
            edge_id: "c2_to_c3",
            source_anchor: "c2.rs:C2->C3",
            from_phase: "c2",
            to_phase: "c3",
            producer_surfaces: &["child binary", "artifact commit", "build journal"],
            consumer_surfaces: &["c3_to_c4", "spawn checks"],
            authority_classes: &[AuthorityClass::Artifact, AuthorityClass::EvidenceProjection],
            live_api: LiveApi::No,
            checkpoint_in: "materialized_child",
            checkpoint_out: "F4_child_materialized_built",
            downstream: &["child invocation", "binary provenance"],
            authority_negative_case: "DB build row cannot replace binary existence or artifact commit",
        },
        TransitionInventoryRow {
            edge_id: "c3_to_c4",
            source_anchor: "c3.rs:C3->C4",
            from_phase: "c3",
            to_phase: "c4",
            producer_surfaces: &[
                "child invocation file",
                "ready channel envelope",
                "spawn record",
            ],
            consumer_surfaces: &["c4_to_c5", "parent observe"],
            authority_classes: &[
                AuthorityClass::Channel,
                AuthorityClass::Bootstrap,
                AuthorityClass::Artifact,
            ],
            live_api: LiveApi::No,
            checkpoint_in: "F4_child_materialized_built",
            checkpoint_out: "child_spawned_ready",
            downstream: &["terminal child observation"],
            authority_negative_case: "DB invocation row cannot replace executable invocation file",
        },
        TransitionInventoryRow {
            edge_id: "c4_to_c5",
            source_anchor: "c4.rs:C4->C5",
            from_phase: "c4",
            to_phase: "c5",
            producer_surfaces: &[
                "terminal channel result",
                "runner result",
                "treatment evidence",
            ],
            consumer_surfaces: &["parent comparison", "selection"],
            authority_classes: &[AuthorityClass::Channel, AuthorityClass::EvidenceProjection],
            live_api: LiveApi::ProviderChild,
            checkpoint_in: "child_spawned_ready",
            checkpoint_out: "F5_child_terminal_result",
            downstream: &["C5 -> ParentCompared", "R10/R11 selection"],
            authority_negative_case: "DB terminal row cannot replace channel body/hash validation",
        },
    ]
}

fn format_terms(terms: &[&str]) -> String {
    if terms.is_empty() {
        "-".to_string()
    } else {
        terms.join("<br>")
    }
}

fn format_classes(classes: &[AuthorityClass]) -> String {
    classes
        .iter()
        .map(|class| class.as_str())
        .collect::<Vec<_>>()
        .join("<br>")
}

#[cfg(test)]
mod tests {
    use std::{collections::BTreeSet, fs, path::Path};

    use super::*;

    const EXPECTED_ROWS: usize = 26;
    const DOC_PATH: &str = "../../docs/active/agents/2026-06-22_prototype1-eval-store-data-model/transition-inventory.generated.md";

    #[test]
    fn prototype1_transition_inventory_covers_source_edges() {
        let rows = prototype1_transition_inventory_rows();
        let parent_count: usize = parent_source_phases()
            .iter()
            .map(|phase| phase.next_steps().len())
            .sum();
        let child_count = child_transition_rows().len();

        assert_eq!(rows.len(), parent_count + child_count);
        assert_eq!(rows.len(), EXPECTED_ROWS);

        let mut ids = BTreeSet::new();
        for row in &rows {
            assert!(ids.insert(row.edge_id), "duplicate edge id {}", row.edge_id);
            assert!(!row.source_anchor.is_empty(), "missing source anchor");
            assert!(!row.authority_classes.is_empty(), "missing authority class");
            assert!(
                !row.authority_negative_case.is_empty(),
                "missing negative case"
            );
        }
    }

    #[test]
    fn prototype1_transition_inventory_names_live_api_edges() {
        let rows = prototype1_transition_inventory_rows();
        let live = rows
            .iter()
            .filter(|row| row.live_api != LiveApi::No)
            .map(|row| row.edge_id)
            .collect::<Vec<_>>();

        assert_eq!(live, vec!["r7_to_r8", "r10_to_r11", "c4_to_c5"]);
    }

    #[test]
    fn prototype1_transition_inventory_generated_doc_matches_source() {
        let rows = prototype1_transition_inventory_rows();
        let actual = render_transition_inventory_markdown(&rows);
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(DOC_PATH);
        if std::env::var_os("PLOKE_UPDATE_TRANSITION_INVENTORY").is_some() {
            fs::write(&path, &actual).unwrap_or_else(|source| {
                panic!(
                    "write generated transition inventory '{}': {source}",
                    path.display()
                )
            });
        }
        let expected = fs::read_to_string(&path).unwrap_or_else(|source| {
            panic!(
                "read generated transition inventory '{}': {source}",
                path.display()
            )
        });

        assert_eq!(actual, expected);
    }
}
