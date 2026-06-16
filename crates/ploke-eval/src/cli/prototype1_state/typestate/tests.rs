use std::{cell::Cell, path::PathBuf};

use ploke_records::{identity::ParentIdentityRecord, ids::CampaignId};

use crate::cli::prototype1_state::{
    identity::{PARENT_IDENTITY_SCHEMA_VERSION, ParentIdentity},
    journal::PrototypeJournal,
    parent::{Parent, Unchecked},
};
use crate::cli::{
    InspectOutputFormat, Prototype1CandidateGenerator, Prototype1StateCommand,
    Prototype1StateStopAfter, Prototype1SuccessorSelection, Prototype1TraversalMetrics,
};

use super::{AsyncStep, R1, R2a, R3, R4a, Step, async_transition, context, transition};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct S0(u8);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct S1(u8);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct S2(u8);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct S3(u8);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TestError {
    Stop,
}

#[test]
fn transition_applies_single_typed_edge() {
    let edge = transition(|state: S0| -> Result<S1, TestError> { Ok(S1(state.0 + 1)) });

    let result = edge.apply(S0(10));

    assert_eq!(result, Ok(S1(11)));
}

#[test]
fn then_composes_adjacent_typed_edges() {
    let pipeline = transition(|state: S0| -> Result<S1, TestError> { Ok(S1(state.0 + 1)) }).then(
        transition(|state: S1| -> Result<S2, TestError> { Ok(S2(state.0 * 2)) }),
    );

    let result = pipeline.apply(S0(10));

    assert_eq!(result, Ok(S2(22)));
}

#[test]
fn composed_chain_preserves_final_output() {
    let pipeline = transition(|state: S0| -> Result<S1, TestError> { Ok(S1(state.0 + 1)) })
        .then(transition(|state: S1| -> Result<S2, TestError> {
            Ok(S2(state.0 * 2))
        }))
        .then(transition(|state: S2| -> Result<S3, TestError> {
            Ok(S3(state.0 + 3))
        }));

    let result = pipeline.apply(S0(10));

    assert_eq!(result, Ok(S3(25)));
}

#[test]
fn chain_short_circuits_on_error() {
    let ran_second = Cell::new(false);
    let pipeline = transition(|_state: S0| -> Result<S1, TestError> { Err(TestError::Stop) }).then(
        transition(|state: S1| -> Result<S2, TestError> {
            ran_second.set(true);
            Ok(S2(state.0))
        }),
    );

    let result = pipeline.apply(S0(10));

    assert_eq!(result, Err(TestError::Stop));
    assert!(!ran_second.get());
}

#[tokio::test]
async fn async_transition_applies_single_typed_edge() {
    let edge = async_transition(|state: S0| async move { Ok::<S1, TestError>(S1(state.0 + 1)) });

    let result = edge.apply(S0(10)).await;

    assert_eq!(result, Ok(S1(11)));
}

fn state_command_for_typestate_test() -> Prototype1StateCommand {
    Prototype1StateCommand {
        campaign: Some(CampaignId::from("campaign")),
        node_id: None,
        repo_root: Some(PathBuf::from("/tmp/prototype1-typestate-test/repo")),
        init_parent_identity: false,
        identity_branch: None,
        identity_instance: None,
        handoff_invocation: None,
        stop_after: Prototype1StateStopAfter::Complete,
        successor_selection: Prototype1SuccessorSelection::HistoryScoreChildProp,
        successor_selection_seed: 0,
        successor_selection_metrics: Prototype1TraversalMetrics::Operational,
        candidate_generator: Prototype1CandidateGenerator::BroadHarnessRequest,
        format: InspectOutputFormat::Table,
    }
}

fn collected_for_typestate_test() -> context::Collected<(), ()> {
    context::Collected::new(
        state_command_for_typestate_test(),
        PathBuf::from("/tmp/prototype1-typestate-test/repo"),
        CampaignId::from("campaign"),
        PathBuf::from("/tmp/prototype1-typestate-test/campaign.json"),
        (),
        (),
        PathBuf::from("/tmp/prototype1-typestate-test/journal.jsonl"),
        PrototypeJournal::new(PathBuf::from(
            "/tmp/prototype1-typestate-test/journal.jsonl",
        )),
    )
}

fn parent_identity_for_typestate_test(node_id: &str) -> ParentIdentity {
    ParentIdentity::from_record_for_test(ParentIdentityRecord {
        schema_version: PARENT_IDENTITY_SCHEMA_VERSION.to_string(),
        campaign_id: CampaignId::from("campaign"),
        parent_id: node_id.to_string(),
        node_id: node_id.to_string(),
        generation: 0,
        instance_id: Some("instance".to_string()),
        previous_parent_id: None,
        parent_node_id: None,
        branch_id: format!("branch-{node_id}"),
        artifact_branch: Some(format!("prototype1-{node_id}")),
        created_at: "2026-06-15T00:00:00Z".to_string(),
    })
}

#[test]
fn r2a_carries_initialized_parent_identity_payload() {
    let identity = parent_identity_for_typestate_test("parent-r2a");
    let r1: R1<(), ()> = R1::from_collected(collected_for_typestate_test());
    let collected = r1.into_collected();

    let r2a = R2a::from_collected_identity(collected, identity.clone());
    let parts = r2a.into_parts();

    assert_eq!(parts.identity, identity);
    assert_eq!(
        parts.collected.into_parts().campaign_id,
        CampaignId::from("campaign")
    );
}

#[test]
fn r3_carries_resolved_parent_identity_payload() {
    let parent_identity = parent_identity_for_typestate_test("parent-r3");
    let r1: R1<(), ()> = R1::from_collected(collected_for_typestate_test());
    let collected = r1.into_collected();

    let r3 = R3::from_collected_identity(collected, parent_identity.clone());
    let parts = r3.into_parts();

    assert_eq!(parts.parent_identity, parent_identity);
    assert_eq!(
        parts.collected.into_parts().campaign_id,
        CampaignId::from("campaign")
    );
}

#[test]
fn r4a_carries_existing_unchecked_parent_carrier() {
    let parent_identity = parent_identity_for_typestate_test("parent-r4a");
    let parent = Parent::<Unchecked>::load(
        &PathBuf::from("/tmp/prototype1-typestate-test/campaign.json"),
        parent_identity.clone(),
    )
    .expect("unchecked parent loads from identity projection");

    let r4a = R4a::from_collected_parent(collected_for_typestate_test(), parent);
    let parts = r4a.into_parts();

    assert_eq!(parts.parent.identity(), &parent_identity);
    assert_eq!(
        parts.collected.into_parts().campaign_id,
        CampaignId::from("campaign")
    );
}
