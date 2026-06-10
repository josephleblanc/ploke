#![cfg(test)]

use ploke_llm::{ProviderKey, router_only::RouterVariants};
use ploke_records::agent_turn::{ModelRouteRecord, ObservedTurnEventRecord};
use serde::Serialize;
use std::{
    borrow::Cow,
    fs,
    path::{Path, PathBuf},
    process::Command,
    sync::Arc,
};
use uuid::Uuid;

use super::*;
use super::{
    Attempt, BroadAttemptError, Capture, LiveObserver, ModelSelection, next_event, run_attempt,
    run_headless, run_headless_with_model, submit_prompt, validation_command_display,
};
use crate::cli::prototype1_state::{
    backend::EditSurfaceAdmission,
    edit_surface::{
        harness_request::{
            AttachedReport, EvidenceRole, HarnessChildBudget, PublishedBroadHarnessRequest,
            RequestAdmissionBinding, contract,
        },
        harness_result::SubmittedBroadHarnessResult,
        surface,
        surface_policy::SurfacePolicy,
    },
};
use crate::loop_graph::{ArtifactId, Coordinate, OperationTarget, RuntimeId};
use crate::spec::PrepareError;

include!("test_fixtures.rs");

#[test]
fn model_selection_sets_openrouter_route() {
    let model_id = "moonshotai/kimi-k2".parse().expect("model id");
    let provider = ProviderKey::new("moonshotai").expect("provider");

    let selection = ModelSelection::openrouter(model_id, Some(provider.clone()));

    assert!(matches!(selection.router(), RouterVariants::OpenRouter(_)));
    assert_eq!(selection.provider(), Some(&provider));
    assert_eq!(
        selection.model_route_record(),
        ModelRouteRecord {
            route_source: "openrouter".to_string(),
            router: "openrouter".to_string(),
            provider_slug: Some("moonshotai".to_string()),
            endpoint_host: Some("openrouter.ai".to_string()),
        }
    );
}

#[test]
fn model_selection_sets_direct_google_route_without_provider_pin() {
    let model_id = "google/gemini-2.5-flash".parse().expect("model id");

    let selection = ModelSelection::direct_google(model_id);

    assert!(matches!(selection.router(), RouterVariants::Google(_)));
    assert!(selection.provider().is_none());
    assert_eq!(
        selection.model_route_record(),
        ModelRouteRecord {
            route_source: "direct_google".to_string(),
            router: "google".to_string(),
            provider_slug: None,
            endpoint_host: Some("aiplatform.googleapis.com".to_string()),
        }
    );
}

fn declared_cargo_command(label: &str, args: &[&str]) -> contract::Command {
    contract::Command {
        label: label.to_string(),
        program: "cargo".to_string(),
        args: args.iter().map(|arg| (*arg).to_string()).collect(),
        workdir: contract::Workdir::CandidateWorkspace,
        success: "command exits successfully".to_string(),
    }
}

#[test]
fn turn_live() {
    let session_id = Uuid::from_u128(0x1111);
    let request_id = Uuid::from_u128(0x2222);
    let parent_id = Uuid::from_u128(0x3333);
    let assistant_id = Uuid::from_u128(0x4444);
    let call_id = "call-read-1".to_string();
    let response = stop_response_record(assistant_id, 0, "chatcmpl-turn-live");
    let run = HeadlessRun {
        attempts: Vec::new(),
        events: vec![
            Event::ToolRequest {
                request_id: request_id.to_string(),
                parent_id: parent_id.to_string(),
                call_id: call_id.clone(),
                tool: "read_file".to_string(),
                arguments: r#"{"file_path":"src/lib.rs"}"#.to_string(),
            },
            Event::Turn {
                session_id: session_id.to_string(),
                request_id: request_id.to_string(),
                parent_id: parent_id.to_string(),
                assistant_message_id: assistant_id.to_string(),
                outcome: "completed".to_string(),
                error_id: None,
                attempts: 1,
                summary: "done".to_string(),
            },
        ],
        validations: Vec::new(),
        debug_relay: DebugRelay::new(),
        prompt_diagnostics: Vec::new(),
        full_response_records: vec![response],
        next_response_index: 1,
        terminal: Some(HeadlessTerminal::CompletedWithoutEdit {
            outcome: "completed".to_string(),
            summary: "done".to_string(),
        }),
        model_route: Some(ModelRouteRecord {
            route_source: "direct_google".to_string(),
            router: "google".to_string(),
            provider_slug: None,
            endpoint_host: Some("aiplatform.googleapis.com".to_string()),
        }),
    };

    let artifact = run.agent_turn_artifact_record("task-1", "test/model", "inspect src/lib.rs");
    let route = artifact
        .model_route
        .as_ref()
        .expect("turn-live artifact records model route");
    assert_eq!(route.route_source, "direct_google");
    assert_eq!(route.router, "google");
    assert_eq!(route.provider_slug, None);
    assert_eq!(
        route.endpoint_host.as_deref(),
        Some("aiplatform.googleapis.com")
    );
    let summary = run.evidence();
    assert_eq!(
        summary
            .model_route
            .as_ref()
            .map(|route| route.endpoint_host.as_deref()),
        Some(Some("aiplatform.googleapis.com"))
    );
    let turn = artifact
        .terminal_record
        .as_ref()
        .expect("turn-live artifact records terminal turn");
    assert_eq!(artifact.user_message_id, parent_id.to_string());
    assert_eq!(turn.session_id, session_id.to_string());
    assert_eq!(turn.request_id, request_id.to_string());
    assert_eq!(turn.parent_id, parent_id.to_string());
    assert_eq!(turn.assistant_message_id, assistant_id.to_string());
    assert!(matches!(
        artifact.events.first(),
        Some(ObservedTurnEventRecord::ToolRequested(record))
            if record.call_id == call_id
                && record.parent_id == parent_id.to_string()
                && record.request_id == request_id.to_string()
    ));

    let [record] = run.full_response_records() else {
        panic!("expected one captured full-response record");
    };
    assert!(record.matches_assistant_message(assistant_id));
    assert_eq!(record.response_index().get(), 0);
}

#[test]
fn classifier_rejects_absolute_path_outside_workspace() {
    let rejection = classify_paths(
        Path::new("/tmp/prototype1/workspace"),
        &SurfacePolicy::workspace_except_core(),
        &[PathBuf::from("/tmp/other/crates/ploke-tui/src/lib.rs")],
    )
    .expect("outside path should reject");

    assert!(matches!(rejection, Reject::Outside { .. }));
}

#[test]
fn classifier_rejects_non_normal_relative_path() {
    let rejection = classify_paths(
        Path::new("/tmp/prototype1/workspace"),
        &SurfacePolicy::workspace_except_core(),
        &[PathBuf::from("crates/../crates/ploke-tui/src/lib.rs")],
    )
    .expect("non-normal path should reject");

    assert!(matches!(rejection, Reject::Outside { .. }));
}

#[test]
fn classifier_uses_broad_policy_for_protected_core() {
    let rejection = classify_paths(
        Path::new("/tmp/prototype1/workspace"),
        &SurfacePolicy::workspace_except_core(),
        &[PathBuf::from("crates/ploke-eval/src/lib.rs")],
    )
    .expect("protected path should reject");

    assert!(matches!(rejection, Reject::Protected { .. }));
}

#[test]
fn tool_batch_waits_for_every_requested_call() {
    let request_id = Uuid::new_v4();
    let first = ploke_core::ArcStr::from("call-first");
    let second = ploke_core::ArcStr::from("call-second");
    let staged = StagedItem::Edit(Uuid::new_v4());
    let mut batches = HashMap::<Uuid, ToolBatch>::new();

    batches
        .entry(request_id)
        .or_default()
        .request(first.clone());
    batches
        .entry(request_id)
        .or_default()
        .request(second.clone());

    assert!(record_batch_terminal(&mut batches, request_id, first, Some(staged)).is_none());
    assert_eq!(
        record_batch_terminal(&mut batches, request_id, second, None),
        Some(vec![staged])
    );
    assert!(!batches.contains_key(&request_id));
}

#[test]
fn failed_cargo_tool_result_becomes_structured_validation_feedback() {
    let mut run = HeadlessRun::new();
    let arguments = r#"{"command":"test","package":"syn_parser"}"#;
    let content = r#"{
        "ok": false,
        "status_reason": "tests_failed_or_runtime",
        "command": "test",
        "scope": "workspace",
        "manifest_path": "/repo/Cargo.toml",
        "exit_code": 101,
        "duration_ms": 42,
        "summary": {
            "errors": 0,
            "warnings": 0,
            "notes": 0,
            "artifacts": 10,
            "other_messages": 0
        },
        "diagnostics": [],
        "stderr_tail": [],
        "non_json_stdout_tail": [],
        "json_parse_errors_tail": [],
        "raw_messages_truncated": false
    }"#;

    let observation =
        observe_cargo_validation(&mut run, "call-cargo", arguments, content).expect("cargo");

    assert!(!observation.ok);
    assert_eq!(observation.display_command, "cargo test -p syn_parser");
    assert_eq!(
        latest_failed_cargo_validation_feedback(&run).as_deref(),
        Some(
            "Cargo validation failed after applying edits: `cargo test -p syn_parser` exited Some(101) with status `tests_failed_or_runtime` (errors: 0, warnings: 0). Repair the failure before claiming success."
        )
    );
    let evidence = run.evidence();
    assert_eq!(evidence.validations.len(), 1);
    assert_eq!(
        evidence.validations[0].display_command,
        "cargo test -p syn_parser"
    );
    assert!(!evidence.validations[0].ok);
}

#[test]
fn declared_cargo_test_command_maps_to_tool_args() {
    let command = declared_cargo_command(
        "edit surface tests",
        &["test", "-p", "ploke-eval", "edit_surface"],
    );
    let args = contract_cargo_args(&command).expect("map declared cargo command");
    let parsed = serde_json::from_str::<CargoRequestArgs>(&args).expect("parse cargo args");
    let observed = display_cargo_command(&parsed, "test");

    assert_eq!(observed, "cargo test -p ploke-eval -- edit_surface");
    assert!(command_display_matches(
        &validation_command_display(&command),
        &observed
    ));
}

#[test]
fn later_successful_cargo_result_clears_latest_failure_gate() {
    let mut run = HeadlessRun::new();
    let failed = r#"{
        "ok": false,
        "status_reason": "tests_failed_or_runtime",
        "command": "test",
        "scope": "workspace",
        "manifest_path": "/repo/Cargo.toml",
        "exit_code": 101,
        "duration_ms": 42,
        "summary": {
            "errors": 0,
            "warnings": 0,
            "notes": 0,
            "artifacts": 10,
            "other_messages": 0
        },
        "diagnostics": [],
        "stderr_tail": [],
        "non_json_stdout_tail": [],
        "json_parse_errors_tail": [],
        "raw_messages_truncated": false
    }"#;
    let passed = r#"{
        "ok": true,
        "status_reason": "success",
        "command": "test",
        "scope": "workspace",
        "manifest_path": "/repo/Cargo.toml",
        "exit_code": 0,
        "duration_ms": 42,
        "summary": {
            "errors": 0,
            "warnings": 0,
            "notes": 0,
            "artifacts": 10,
            "other_messages": 0
        },
        "diagnostics": [],
        "stderr_tail": [],
        "non_json_stdout_tail": [],
        "json_parse_errors_tail": [],
        "raw_messages_truncated": false
    }"#;

    observe_cargo_validation(
        &mut run,
        "call-failed",
        r#"{"command":"test","package":"syn_parser"}"#,
        failed,
    )
    .expect("failed cargo");
    observe_cargo_validation(
        &mut run,
        "call-passed",
        r#"{"command":"test","package":"syn_parser"}"#,
        passed,
    )
    .expect("passed cargo");

    assert!(latest_failed_cargo_validation_feedback(&run).is_none());
    assert_eq!(run.evidence().validations.len(), 2);
}

#[test]
fn timeout_after_apply_terminal_preserves_applied_evidence() {
    let proposal_id = Uuid::from_u128(0x8100);
    let paths = vec![PathBuf::from("crates/ploke-eval/src/lib.rs")];
    let run = HeadlessRun::from_parts_for_test(
        vec![HeadlessAttempt::applied_for_test(
            1,
            proposal_id,
            paths.clone(),
        )],
        None,
    );

    let terminal = timeout_terminal_for_run(&run, 900);

    assert!(matches!(
        terminal,
        HeadlessTerminal::AppliedTimedOut { secs: 900, applied }
            if applied.proposal_id() == proposal_id && applied.changed_paths() == paths
    ));
}

#[test]
fn aborted_turn_after_apply_terminal_preserves_applied_evidence() {
    let proposal_id = Uuid::from_u128(0x8101);
    let applied = AppliedEdit {
        proposal_id,
        proposal_ids: vec![proposal_id],
        changed_paths: vec![PathBuf::from("crates/ploke-eval/src/lib.rs")],
    };

    let terminal = turn_aborted_after_apply_terminal(
        applied.clone(),
        "aborted".to_string(),
        "model turn aborted after tool output".to_string(),
    );

    assert!(matches!(
        terminal,
        HeadlessTerminal::AppliedTurnAborted {
            applied: observed,
            outcome,
            summary,
        } if observed == applied
            && outcome == "aborted"
            && summary.contains("after tool output")
    ));
}

#[test]
fn requested_validation_missing_blocks_applied_terminal() {
    let proposal_id = Uuid::from_u128(0x8102);
    let request_id = Uuid::from_u128(0x8103);
    let applied = AppliedEdit {
        proposal_id,
        proposal_ids: vec![proposal_id],
        changed_paths: vec![PathBuf::from("crates/ploke-eval/src/lib.rs")],
    };
    let mut run = HeadlessRun::new();
    let passed = r#"{
        "ok": true,
        "status_reason": "success",
        "command": "test",
        "scope": "workspace",
        "manifest_path": "/repo/Cargo.toml",
        "exit_code": 0,
        "duration_ms": 42,
        "summary": {
            "errors": 0,
            "warnings": 0,
            "notes": 0,
            "artifacts": 10,
            "other_messages": 0
        },
        "diagnostics": [],
        "stderr_tail": [],
        "non_json_stdout_tail": [],
        "json_parse_errors_tail": [],
        "raw_messages_truncated": false
    }"#;
    observe_cargo_validation(
        &mut run,
        "call-wrong",
        r#"{"command":"test","package":"ploke-db-derive"}"#,
        passed,
    )
    .expect("wrong package validation");

    let terminal = classify_applied_terminal(
        &run,
        &[declared_cargo_command(
            "compile ploke-eval",
            &["check", "-p", "ploke-eval"],
        )],
        request_id,
        applied.clone(),
    );

    assert!(matches!(
        terminal,
        HeadlessTerminal::AppliedValidationMissing {
            applied: observed,
            missing,
        } if observed == applied && missing == vec!["cargo check -p ploke-eval"]
    ));
}

#[test]
fn requested_validation_failure_blocks_applied_terminal() {
    let proposal_id = Uuid::from_u128(0x8104);
    let request_id = Uuid::from_u128(0x8105);
    let applied = AppliedEdit {
        proposal_id,
        proposal_ids: vec![proposal_id],
        changed_paths: vec![PathBuf::from("crates/ploke-eval/src/lib.rs")],
    };
    let mut run = HeadlessRun::new();
    let failed = r#"{
        "ok": false,
        "status_reason": "tests_failed_or_runtime",
        "command": "check",
        "scope": "workspace",
        "manifest_path": "/repo/Cargo.toml",
        "exit_code": 101,
        "duration_ms": 42,
        "summary": {
            "errors": 1,
            "warnings": 0,
            "notes": 0,
            "artifacts": 10,
            "other_messages": 0
        },
        "diagnostics": [],
        "stderr_tail": [],
        "non_json_stdout_tail": [],
        "json_parse_errors_tail": [],
        "raw_messages_truncated": false
    }"#;
    observe_cargo_validation(
        &mut run,
        "call-failed",
        r#"{"command":"check","package":"ploke-eval"}"#,
        failed,
    )
    .expect("declared validation failure");

    let terminal = classify_applied_terminal(
        &run,
        &[declared_cargo_command(
            "compile ploke-eval",
            &["check", "-p", "ploke-eval"],
        )],
        request_id,
        applied.clone(),
    );

    assert!(matches!(
        terminal,
        HeadlessTerminal::AppliedValidationFailed { applied: observed, feedback }
            if observed == applied
                && feedback.contains("cargo check -p ploke-eval")
                && feedback.contains("tests_failed_or_runtime")
    ));
}

#[test]
fn requested_validation_passes_applied_terminal() {
    let proposal_id = Uuid::from_u128(0x8106);
    let request_id = Uuid::from_u128(0x8107);
    let applied = AppliedEdit {
        proposal_id,
        proposal_ids: vec![proposal_id],
        changed_paths: vec![PathBuf::from("crates/ploke-eval/src/lib.rs")],
    };
    let mut run = HeadlessRun::new();
    let passed = r#"{
        "ok": true,
        "status_reason": "success",
        "command": "check",
        "scope": "workspace",
        "manifest_path": "/repo/Cargo.toml",
        "exit_code": 0,
        "duration_ms": 42,
        "summary": {
            "errors": 0,
            "warnings": 0,
            "notes": 0,
            "artifacts": 10,
            "other_messages": 0
        },
        "diagnostics": [],
        "stderr_tail": [],
        "non_json_stdout_tail": [],
        "json_parse_errors_tail": [],
        "raw_messages_truncated": false
    }"#;
    observe_cargo_validation(
        &mut run,
        "call-passed",
        r#"{"command":"check","package":"ploke-eval"}"#,
        passed,
    )
    .expect("declared validation success");

    let terminal = classify_applied_terminal(
        &run,
        &[declared_cargo_command(
            "compile ploke-eval",
            &["check", "-p", "ploke-eval"],
        )],
        request_id,
        applied,
    );

    assert!(matches!(
        terminal,
        HeadlessTerminal::Applied {
            proposal_id: observed_proposal,
            request_id: observed_request,
            changed_paths,
            ..
        } if observed_proposal == proposal_id
            && observed_request == request_id
            && changed_paths == vec![PathBuf::from("crates/ploke-eval/src/lib.rs")]
    ));
}

#[test]
fn select_disjoint_keeps_newest_file_disjoint_candidates() {
    let workspace = Path::new("/repo");
    let newer_same_file = Candidate {
        item: StagedItem::Edit(Uuid::from_u128(2)),
        proposed_at_ms: 200,
        paths: vec![PathBuf::from("/repo/crates/ploke-tui/src/lib.rs")],
    };
    let other_file = Candidate {
        item: StagedItem::Edit(Uuid::from_u128(3)),
        proposed_at_ms: 150,
        paths: vec![PathBuf::from("crates/ploke-rag/src/lib.rs")],
    };
    let older_same_file = Candidate {
        item: StagedItem::Edit(Uuid::from_u128(1)),
        proposed_at_ms: 100,
        paths: vec![PathBuf::from("crates/ploke-tui/src/lib.rs")],
    };

    let (selected, rejected) = select_disjoint(
        vec![
            older_same_file.clone(),
            other_file.clone(),
            newer_same_file.clone(),
        ],
        workspace,
    );

    assert_eq!(
        selected
            .iter()
            .map(|candidate| candidate.item)
            .collect::<Vec<_>>(),
        vec![newer_same_file.item, other_file.item]
    );
    assert_eq!(
        rejected
            .iter()
            .map(|candidate| candidate.item)
            .collect::<Vec<_>>(),
        vec![older_same_file.item]
    );
}

#[test]
fn sparse_post_apply_refresh_gate_uses_sparse_search_config() {
    use ploke_tui::user_config::RetrievalStrategyUser;

    assert!(sparse_search_refresh_enabled(
        &RetrievalStrategyUser::Sparse { strict: true },
        false
    ));
    assert!(!sparse_search_refresh_enabled(
        &RetrievalStrategyUser::Sparse { strict: false },
        false
    ));
    assert!(sparse_search_refresh_enabled(
        &RetrievalStrategyUser::Sparse { strict: false },
        true
    ));
    assert!(!sparse_search_refresh_enabled(
        &RetrievalStrategyUser::Dense,
        true
    ));
    assert!(!sparse_search_refresh_enabled(
        &RetrievalStrategyUser::default(),
        false
    ));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn sparse_post_apply_refresh_returns_on_bm25_without_dense_index_completion() {
    let _recorded_replay_guard = recorded_replay_test_mutex().lock().await;
    let fixture = prepare_live_canary(
        "sparse-post-apply-refresh",
        "runtime refresh only; no LLM prompt is submitted",
    )
    .expect("prepare sparse refresh fixture");
    let mut runtime = crate::runner::setup_workspace_tui_runtime(&fixture.workspace)
        .await
        .expect("start sparse refresh runtime");
    runtime.app.pump_pending_events().await;

    fs::write(
        &fixture.src_file,
        r#"pub fn broad_surface_canary() -> &'static str {
"after"
}
"#,
    )
    .expect("write changed source before refresh");

    let mut pending_events = VecDeque::new();
    let refresh_timeouts = Timeouts::default();
    let refresh_deadline = std::time::Instant::now() + refresh_timeouts.post_apply_index_duration();
    tokio::time::timeout(
        Duration::from_secs(10),
        wait_for_refresh(
            &mut runtime,
            &mut pending_events,
            1,
            &LiveObserver::disabled(),
            refresh_deadline,
            &refresh_timeouts,
            std::slice::from_ref(&fixture.src_file),
        ),
    )
    .await
    .expect("sparse refresh should not wait for dense IndexingCompleted")
    .expect("sparse refresh should succeed");

    let status = runtime
        .state
        .rag
        .as_ref()
        .expect("RAG service")
        .bm25_status()
        .await
        .expect("BM25 status after sparse refresh");
    assert!(
        matches!(status, ploke_db::bm25_index::bm25_service::Bm25Status::Ready { docs } if docs > 0),
        "expected BM25 ready after sparse refresh, got {status:?}"
    );
}

#[test]
fn terminal_ids_use_last_applied_proposal_as_primary() {
    let first = AppliedItem::Edit(Uuid::from_u128(1));
    let second = AppliedItem::Edit(Uuid::from_u128(2));
    let third = AppliedItem::Edit(Uuid::from_u128(3));

    let (primary, proposal_ids) = terminal_ids(&[first, second, third]).expect("terminal ids");

    assert_eq!(primary, third.id());
    assert_eq!(proposal_ids, vec![first.id(), second.id(), third.id()]);
}

#[test]
fn attempt_prompt_preserves_minimal_request_text() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let existing_evidence = tmp.path().join("evaluations");
    let missing_evidence = tmp.path().join("missing-evaluations");
    fs::create_dir_all(&existing_evidence).expect("existing evidence dir");

    let prompt = attempt_prompt(
        Path::new("/tmp/prototype1/workspace"),
        &SurfacePolicy::workspace_except_core(),
        &[
            EvidenceRoot {
                kind: EvidenceRootKind::Evaluations,
                location: EvidenceRootLocation::Directory {
                    path: existing_evidence.clone(),
                },
                role: EvidenceRole::EvaluationPayloads,
            },
            EvidenceRoot {
                kind: EvidenceRootKind::Evaluations,
                location: EvidenceRootLocation::Directory {
                    path: missing_evidence.clone(),
                },
                role: EvidenceRole::EvaluationPayloads,
            },
        ],
        "Modify any part of the codebase at `/tmp/prototype1/workspace`.\n\nPast benchmark results live under `/tmp/prototype1/evaluations`.\n",
        Some("tool failed"),
    );

    assert!(prompt.starts_with("Modify any part of the codebase at"));
    assert!(prompt.contains("Past benchmark results live under"));
    assert!(prompt.contains("Previous attempt result:"));
    assert!(prompt.contains("tool failed"));
    assert!(!prompt.contains("Headless TUI harness boundary"));
    assert!(!prompt.contains("Read-only evidence available to tools"));
    assert!(!prompt.contains("Original broad request"));
    assert!(!prompt.contains("stage one"));
}

#[test]
fn evidence_read_roots_include_request_evidence_but_not_result_output() {
    let roots = evidence_read_roots(&[
        EvidenceRoot {
            kind: EvidenceRootKind::HistoryBlocks,
            location: EvidenceRootLocation::Directory {
                path: PathBuf::from("/tmp/prototype1/history/blocks"),
            },
            role: EvidenceRole::SealedHistory,
        },
        EvidenceRoot {
            kind: EvidenceRootKind::Evaluations,
            location: EvidenceRootLocation::Directory {
                path: PathBuf::from("/tmp/prototype1/evaluations"),
            },
            role: EvidenceRole::EvaluationPayloads,
        },
        EvidenceRoot {
            kind: EvidenceRootKind::Nodes,
            location: EvidenceRootLocation::Directory {
                path: PathBuf::from("/tmp/prototype1/nodes"),
            },
            role: EvidenceRole::RuntimeEvidence,
        },
        EvidenceRoot {
            kind: EvidenceRootKind::ProtocolArtifacts,
            location: EvidenceRootLocation::NodeScopedDirectory {
                nodes_root: PathBuf::from("/tmp/prototype1/nodes"),
                child_relpath: PathBuf::from("protocol-artifacts"),
            },
            role: EvidenceRole::GuidanceOnly,
        },
        EvidenceRoot {
            kind: EvidenceRootKind::Oracle,
            location: EvidenceRootLocation::AttachedReport {
                report: AttachedReport::FinalReportJson,
            },
            role: EvidenceRole::OracleSummary,
        },
        EvidenceRoot {
            kind: EvidenceRootKind::SubmittedResultOutput,
            location: EvidenceRootLocation::File {
                path: PathBuf::from("/tmp/prototype1/messages/result.json"),
            },
            role: EvidenceRole::OutputBox,
        },
    ]);

    assert_eq!(
        roots,
        vec![
            PathBuf::from("/tmp/prototype1"),
            PathBuf::from("/tmp/prototype1/evaluations"),
            PathBuf::from("/tmp/prototype1/history/blocks"),
            PathBuf::from("/tmp/prototype1/nodes"),
        ]
    );
}

#[test]
fn retry_feedback_summarizes_tool_json_without_replaying_payload() {
    let feedback = retry_feedback(
        r#"{"user":"read_file: read_file expects a file path, not a directory.","llm":{"ok":false}}"#,
    );

    assert!(feedback.contains("Previous attempt failed"));
    assert!(feedback.contains("read_file expects a file path"));
    assert!(!feedback.contains("stage one"));
    assert!(!feedback.contains("\"llm\""));
}

#[test]
fn retry_feedback_turns_aborted_summary_into_terse_result() {
    let feedback = retry_feedback("Request summary: [aborted] error_id=abc");

    assert!(feedback.contains("Previous attempt aborted before staging an edit"));
    assert!(!feedback.contains("stage one small concrete source edit"));
    assert!(!feedback.contains("error_id=abc"));
}

#[test]
fn system_message_google_401_is_provider_unavailable() {
    let content = r#"Error: API error (status 401): [{
  "error": {
"code": 401,
"message": "Request had invalid authentication credentials.",
"status": "UNAUTHENTICATED",
"details": [
  {
    "@type": "type.googleapis.com/google.rpc.ErrorInfo",
    "reason": "ACCESS_TOKEN_TYPE_UNSUPPORTED"
  }
]
  }
}]
Suggested action: Verify API credentials and retry."#;
    let status = ploke_tui::chat_history::MessageStatus::Error {
        description: "API error (status 401)".to_string(),
    };

    let reason = provider_failure_from_message(
        ploke_tui::chat_history::MessageKind::System,
        &status,
        content,
    )
    .expect("system provider error should stop the headless attempt");

    assert!(reason.contains("status 401"));
    assert!(reason.contains("ACCESS_TOKEN_TYPE_UNSUPPORTED"));
}

#[test]
fn aborted_summary_google_adc_failure_is_provider_unavailable() {
    let reason = provider_unavailable_reason(
        "Request summary: [aborted] error_id=abc code=HTTP_SEND_FAILED kind=transport \
         error_summary=HTTP error while sending request: failed to resolve bearer token: \
         Var error: failed to resolve Google application default credentials",
    )
    .expect("ADC bearer-token failures should stop the headless attempt");

    assert!(reason.contains("failed to resolve bearer token"));
    assert!(reason.contains("application default credentials"));
}

#[test]
fn completed_system_message_is_not_provider_unavailable() {
    let status = ploke_tui::chat_history::MessageStatus::Completed;

    assert!(
        provider_failure_from_message(
            ploke_tui::chat_history::MessageKind::System,
            &status,
            "Error: API error (status 401)"
        )
        .is_none()
    );
}

#[test]
fn policy_repair_prompt_preserves_applied_workspace_state() {
    let prompt = policy_repair_prompt("Rejected protected paths: crates/example/Cargo.toml", true);

    assert!(prompt.contains("Previous attempt result"));
    assert!(prompt.contains("Protected core: see"));
    assert!(prompt.contains("WORKSPACE_EXCEPT_AUTHORITY_*"));
    assert!(prompt.contains("The workspace already contains allowed edits"));
    assert!(!prompt.contains("authority/runtime directories"));
    assert!(!prompt.contains("stage only allowed follow-up source edits"));
}

#[test]
fn policy_repair_prompt_handles_no_applied_edits() {
    let prompt = policy_repair_prompt("Rejected protected paths: crates/example/Cargo.toml", false);

    assert!(prompt.contains("No allowed source edit has been applied yet"));
    assert!(!prompt.contains("stage a concrete candidate"));
}

#[test]
fn evidence_applied_attempt_carries_changed_paths() {
    let proposal_id = Uuid::from_u128(1);
    let request_id = Uuid::from_u128(2);
    let changed_paths = vec![
        PathBuf::from("crates/ploke-tui/src/app.rs"),
        PathBuf::from("crates/ploke-tui/src/lib.rs"),
    ];
    let run = HeadlessRun {
        attempts: vec![HeadlessAttempt {
            turn: 1,
            proposal_id: Some(proposal_id),
            result: HeadlessAttemptResult::Applied {
                paths: changed_paths.clone(),
            },
        }],
        events: Vec::new(),
        validations: Vec::new(),
        debug_relay: DebugRelay::new(),
        prompt_diagnostics: Vec::new(),
        full_response_records: Vec::new(),
        next_response_index: 0,
        terminal: Some(HeadlessTerminal::Applied {
            proposal_id,
            applied_proposal_ids: vec![proposal_id],
            request_id,
            changed_paths: changed_paths.clone(),
        }),
        model_route: None,
    };

    let summary = run.evidence();
    let proposal_uuid = proposal_id;
    let proposal_id = proposal_id.to_string();
    let request_id = request_id.to_string();

    assert_eq!(summary.attempts.len(), 1);
    assert_eq!(summary.attempts[0].turn, 1);
    assert_eq!(
        summary.attempts[0].proposal_id.as_deref(),
        Some(proposal_id.as_str())
    );
    assert!(matches!(
        &summary.attempts[0].result,
        evidence::Result::Applied { paths } if paths == &changed_paths
    ));
    assert!(matches!(
        summary.terminal.as_ref(),
        Some(evidence::Terminal::Applied {
            proposal_id: observed_proposal,
            applied_proposal_ids,
            request_id: observed_request,
            changed_paths: observed_paths,
        }) if observed_proposal == &proposal_id
            && applied_proposal_ids.as_slice() == &[proposal_uuid]
            && observed_request == &request_id
            && observed_paths == &changed_paths
    ));
}

#[test]
fn evidence_applied_timeout_terminal_carries_post_apply_state() {
    let proposal_id = Uuid::from_u128(0x8200);
    let changed_paths = vec![PathBuf::from("crates/ploke-eval/src/lib.rs")];
    let applied = AppliedEdit {
        proposal_id,
        proposal_ids: vec![proposal_id],
        changed_paths: changed_paths.clone(),
    };
    let run = HeadlessRun::from_parts_for_test(
        vec![HeadlessAttempt::applied_for_test(
            1,
            proposal_id,
            changed_paths.clone(),
        )],
        Some(HeadlessTerminal::AppliedTimedOut { secs: 900, applied }),
    );

    let summary = run.evidence();

    assert!(matches!(
        summary.terminal.as_ref(),
        Some(evidence::Terminal::AppliedTimedOut {
            secs: 900,
            proposal_id: observed_proposal,
            applied_proposal_ids,
            changed_paths: observed_paths,
        }) if observed_proposal == &proposal_id.to_string()
            && applied_proposal_ids.as_slice() == &[proposal_id]
            && observed_paths == &changed_paths
    ));
    serde_json::to_string_pretty(&summary).expect("post-apply timeout evidence serializes");
}

#[test]
fn evidence_rejected_attempt_carries_feedback() {
    let proposal_id = Uuid::from_u128(3);
    let feedback = "Rejected protected paths: crates/ploke-eval/src/lib.rs".to_string();
    let run = HeadlessRun {
        attempts: vec![HeadlessAttempt {
            turn: 2,
            proposal_id: Some(proposal_id),
            result: HeadlessAttemptResult::Rejected {
                reason: feedback.clone(),
            },
        }],
        events: Vec::new(),
        validations: Vec::new(),
        debug_relay: DebugRelay::new(),
        prompt_diagnostics: Vec::new(),
        full_response_records: Vec::new(),
        next_response_index: 0,
        terminal: Some(HeadlessTerminal::Exhausted {
            attempts: 2,
            last: feedback.clone(),
        }),
        model_route: None,
    };

    let summary = run.evidence();

    assert_eq!(summary.attempts.len(), 1);
    assert_eq!(summary.attempts[0].turn, 2);
    assert!(matches!(
        &summary.attempts[0].result,
        evidence::Result::Rejected { feedback: observed } if observed == &feedback
    ));
    assert!(matches!(
        summary.terminal.as_ref(),
        Some(evidence::Terminal::Exhausted {
            attempts: 2,
            last_feedback,
        }) if last_feedback == &feedback
    ));
}

#[test]
fn evidence_can_preserve_observed_headless_runtime_error() {
    let proposal_id = Uuid::from_u128(4);
    let changed_paths = vec![PathBuf::from("crates/ploke-llm/src/types/meta.rs")];
    let error = observed_headless_error(Error::HeadlessEvent(
        "timed out waiting for indexing completion after applying proposal batch after 180s"
            .to_string(),
    ));
    let run = HeadlessRun::from_parts_for_test(
        vec![HeadlessAttempt::applied_for_test(
            1,
            proposal_id,
            changed_paths.clone(),
        )],
        Some(HeadlessTerminal::ToolFailed {
            error: error.clone(),
        }),
    );

    assert!(run.has_observed_activity());
    let summary = run.evidence();

    assert!(matches!(
        &summary.attempts[0].result,
        evidence::Result::Applied { paths } if paths == &changed_paths
    ));
    assert!(matches!(
        summary.terminal.as_ref(),
        Some(evidence::Terminal::ToolFailed { error: observed })
            if observed == &error
                && observed.contains("headless runtime failed after observed activity")
    ));
}

#[test]
fn post_approval_error_records_indeterminate_for_unsettled_proposal() {
    let proposal_id = Uuid::from_u128(41);
    let paths = vec![PathBuf::from("crates/ploke-tree/src/tests.rs")];
    let error = "headless ploke-tui event stream failed: scan barrier failed: channel closed";
    let selected = vec![Candidate {
        item: StagedItem::Edit(proposal_id),
        proposed_at_ms: 0,
        paths: paths.clone(),
    }];
    let mut run = HeadlessRun::new();

    record_post_approval_indeterminate(&mut run, 2, &LiveObserver::disabled(), &selected, error);

    assert!(matches!(
        run.attempts().first().map(HeadlessAttempt::result),
        Some(HeadlessAttemptResult::PostApprovalIndeterminate {
            paths: observed_paths,
            error: observed_error,
        }) if observed_paths == &paths && observed_error == error
    ));
    let summary = run.evidence();
    assert!(matches!(
        &summary.attempts[0].result,
        evidence::Result::PostApprovalIndeterminate {
            paths: observed_paths,
            error: observed_error,
        } if observed_paths == &paths && observed_error.contains("scan barrier failed")
    ));
    serde_json::to_string_pretty(&summary).expect("indeterminate result serializes");
}

#[test]
fn post_approval_error_keeps_existing_applied_record() {
    let proposal_id = Uuid::from_u128(42);
    let paths = vec![PathBuf::from("crates/ploke-tree/src/tests.rs")];
    let selected = vec![Candidate {
        item: StagedItem::Edit(proposal_id),
        proposed_at_ms: 0,
        paths: paths.clone(),
    }];
    let mut run = HeadlessRun::from_parts_for_test(
        vec![HeadlessAttempt::applied_for_test(2, proposal_id, paths)],
        None,
    );

    record_post_approval_indeterminate(
        &mut run,
        2,
        &LiveObserver::disabled(),
        &selected,
        "scan barrier failed: channel closed",
    );

    assert_eq!(
        run.attempts().len(),
        1,
        "scan-barrier failure after observed apply should preserve the applied record"
    );
    assert!(matches!(
        run.attempts()[0].result(),
        HeadlessAttemptResult::Applied { .. }
    ));
}

#[test]
fn evidence_retains_bounded_debug_relay() {
    let mut run = HeadlessRun::new();
    run.debug_relay.push("first relay command");
    run.debug_relay
        .push(&"x".repeat(MAX_DEBUG_RELAY_EVENT_CHARS + 3));
    for index in 0..MAX_DEBUG_RELAY_EVENTS {
        run.debug_relay.push(&format!("relay command {index}"));
    }

    let summary = run.evidence();

    assert_eq!(summary.debug_relay.retained.len(), MAX_DEBUG_RELAY_EVENTS);
    assert_eq!(summary.debug_relay.dropped, 2);
    assert_eq!(summary.debug_relay.truncated, 1);
    let expected_last = format!("relay command {}", MAX_DEBUG_RELAY_EVENTS - 1);
    assert_eq!(
        summary.debug_relay.retained.last().map(String::as_str),
        Some(expected_last.as_str())
    );
    assert!(
        summary
            .debug_relay
            .retained
            .iter()
            .all(|message| message.chars().count() <= MAX_DEBUG_RELAY_EVENT_CHARS)
    );
}

#[test]
fn evidence_carries_bounded_tool_event_stream() {
    let mut run = HeadlessRun::new();
    let long_args = format!(
        r#"{{"file_path":"src/lib.rs","payload":"{}"}}"#,
        "x".repeat(MAX_EVIDENCE_EVENT_CHARS + 10)
    );
    run.events.push(Event::ToolRequest {
        request_id: "request-1".to_string(),
        parent_id: "parent-1".to_string(),
        call_id: "call-1".to_string(),
        tool: "apply_code_edit".to_string(),
        arguments: long_args,
    });
    run.events.push(Event::Tool {
        call_id: "call-1".to_string(),
        result: Tool::Failed {
            error: "tool rejected invalid path".to_string(),
        },
    });

    let summary = run.evidence();

    assert_eq!(summary.events.len(), 2);
    assert!(matches!(
        &summary.events[0],
        evidence::Event::ToolRequest {
            tool,
            arguments,
            ..
        } if tool == "apply_code_edit"
            && arguments.chars > MAX_EVIDENCE_EVENT_CHARS
            && arguments.preview.chars().count() <= MAX_EVIDENCE_EVENT_CHARS + 3
    ));
    assert!(matches!(
        &summary.events[1],
        evidence::Event::ToolFailed { error, .. }
            if error.preview.contains("tool rejected invalid path")
    ));
    serde_json::to_string_pretty(&summary).expect("event diagnostics serialize");
}

#[test]
fn evidence_carries_prompt_context_diagnostics() {
    let mut run = HeadlessRun::new();
    run.prompt_diagnostics.push(PromptDiagnostic {
        parent_id: Uuid::from_u128(7).to_string(),
        workspace: WorkspaceDiagnostic {
            loaded: true,
            root: Some(PathBuf::from("/tmp/candidate")),
            member_count: 3,
            focused_root: Some(PathBuf::from("/tmp/candidate/crates/ploke-eval")),
        },
        bm25: Some(Bm25Diagnostic {
            status: "ready".to_string(),
            docs: Some(42),
            error: None,
        }),
        context_mode: "Light".to_string(),
        max_leased_tokens: 2400,
        estimated_total_tokens: 900,
        message_count: 2,
        message_previews: vec![MessagePreview {
            role: "System".to_string(),
            chars: 11,
            preview: "RAG context".to_string(),
        }],
        included_rag_parts: 1,
        rag_part_previews: vec![RagPartPreview {
            file_path: "src/lib.rs".to_string(),
            kind: "Code".to_string(),
            estimated_tokens: 100,
            score: 0.75,
        }],
        rag_stats: Some(ContextStatsDiagnostic {
            total_tokens: 100,
            files: 1,
            parts: 1,
            truncated_parts: 0,
            dedup_removed: 0,
        }),
        fallback_notice: None,
    });

    let summary = run.evidence();

    assert_eq!(summary.prompt_diagnostics.len(), 1);
    let diagnostic = &summary.prompt_diagnostics[0];
    assert!(diagnostic.workspace.loaded);
    assert_eq!(diagnostic.workspace.member_count, 3);
    assert!(matches!(
        diagnostic.bm25.as_ref(),
        Some(evidence::Bm25 { status, docs: Some(42), .. }) if status == "ready"
    ));
    assert_eq!(diagnostic.included_rag_parts, 1);
    serde_json::to_string_pretty(&summary).expect("prompt diagnostics serialize");
}

#[test]
fn fallback_prompt_becomes_context_unavailable_terminal() {
    let fallback = "No workspace context loaded; proceeding without code context. Index or load a workspace to enable RAG.";
    let diagnostic = PromptDiagnostic {
        parent_id: Uuid::from_u128(8).to_string(),
        workspace: WorkspaceDiagnostic {
            loaded: false,
            root: None,
            member_count: 0,
            focused_root: None,
        },
        bm25: None,
        context_mode: "Light".to_string(),
        max_leased_tokens: 2400,
        estimated_total_tokens: 26,
        message_count: 1,
        message_previews: vec![MessagePreview {
            role: "System".to_string(),
            chars: fallback.chars().count(),
            preview: fallback.to_string(),
        }],
        included_rag_parts: 0,
        rag_part_previews: Vec::new(),
        rag_stats: None,
        fallback_notice: Some(fallback.to_string()),
    };

    let reason = diagnostic
        .context_unavailable_reason()
        .expect("fallback should be hard failure");
    let run = HeadlessRun {
        attempts: Vec::new(),
        events: Vec::new(),
        validations: Vec::new(),
        debug_relay: DebugRelay::new(),
        prompt_diagnostics: vec![diagnostic],
        full_response_records: Vec::new(),
        next_response_index: 0,
        terminal: Some(HeadlessTerminal::ContextUnavailable {
            reason: reason.clone(),
        }),
        model_route: None,
    };

    let summary = run.evidence();

    assert!(matches!(
        summary.terminal.as_ref(),
        Some(evidence::Terminal::ContextUnavailable { reason: observed }) if observed == &reason
    ));
    assert_eq!(
        summary.prompt_diagnostics[0].fallback_notice.as_deref(),
        Some(fallback)
    );
}

#[test]
fn off_context_prompt_is_not_context_unavailable() {
    let fallback = "Context mode is Off: Context will not automatically be attached to the user message.; Context search via request_code_context is still available. workspace loaded /tmp/candidate.";
    let diagnostic = PromptDiagnostic {
        parent_id: Uuid::from_u128(9).to_string(),
        workspace: WorkspaceDiagnostic {
            loaded: true,
            root: Some(PathBuf::from("/tmp/candidate")),
            member_count: 1,
            focused_root: Some(PathBuf::from("/tmp/candidate")),
        },
        bm25: Some(Bm25Diagnostic {
            status: "ready".to_string(),
            docs: Some(12),
            error: None,
        }),
        context_mode: "Off".to_string(),
        max_leased_tokens: 2400,
        estimated_total_tokens: 20,
        message_count: 2,
        message_previews: vec![MessagePreview {
            role: "System".to_string(),
            chars: fallback.chars().count(),
            preview: fallback.to_string(),
        }],
        included_rag_parts: 0,
        rag_part_previews: Vec::new(),
        rag_stats: None,
        fallback_notice: Some(fallback.to_string()),
    };

    assert!(
        diagnostic.context_unavailable_reason().is_none(),
        "broad harness intentionally disables automatic prompt context"
    );
}

// regr:protectedstaged:19-05-26_15-43
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn recorded_replay_rejects_protected_ns_patch_before_staged_success_reaches_model() {
    let _recorded_replay_guard = recorded_replay_test_mutex().lock().await;
    let fixture = prepare_live_canary(
        "recorded-protected-ns-patch-replay",
        "Use non_semantic_patch to edit crates/ploke-eval/src/lib.rs.",
    )
    .expect("prepare recorded replay fixture");
    let protected_rel = Path::new("crates/ploke-eval/src/lib.rs");
    let protected_abs = fixture.workspace.join(protected_rel);
    fs::create_dir_all(protected_abs.parent().expect("protected file parent"))
        .expect("create protected file parent");
    fs::write(
        &protected_abs,
        r#"pub fn protected_replay_canary() -> &'static str {
"before"
}
"#,
    )
    .expect("write protected file");

    let call_id = "call_protected_ns_patch";
    let tape = recorded_protected_ns_patch_tape(&fixture.artifact_root, call_id, protected_rel);
    ploke_tui::llm::install_recorded_response_tape(tape);
    let _clear_tape = ClearRecordedTapeOnDrop;

    let (request_tx, request_rx) = std::sync::mpsc::channel();
    let _tap_guard = ploke_tui::llm::install_request_tap(request_tx);
    let (mut runtime, parent_id) = start_attempt_runtime(
        &fixture.workspace,
        &[],
        fixture.prompt.clone(),
        &SurfacePolicy::workspace_except_core(),
        None,
        &Timeouts::default(),
    )
    .await
    .expect("start recorded replay runtime");

    let mut snapshots = Vec::new();
    let mut failed_tool = None;
    let deadline = tokio::time::Instant::now() + Duration::from_secs(15);
    while tokio::time::Instant::now() < deadline {
        runtime.app.pump_pending_events().await;
        collect_request_snapshots(&request_rx, &mut snapshots);
        if snapshots.len() >= 2 && failed_tool.is_some() {
            break;
        }

        let event = match tokio::time::timeout(Duration::from_millis(100), next_event(&mut runtime))
            .await
        {
            Ok(Ok(event)) => event,
            Ok(Err(err)) => panic!("recorded replay event stream failed: {err}"),
            Err(_) => continue,
        };

        if let ploke_tui::AppEvent::System(
            ploke_tui::app_state::events::SystemEvent::ToolCallFailed {
                parent_id: event_parent_id,
                call_id: event_call_id,
                error,
                ..
            },
        ) = event
            && event_parent_id == parent_id
            && event_call_id.as_ref() == call_id
        {
            let wire = ploke_tui::tools::ToolErrorWire::parse(&error)
                .expect("protected ns_patch failure should use tool error wire");
            assert!(!wire.llm.ok);
            assert!(
                wire.llm.message.contains("protected"),
                "expected protected-path failure, got {error}"
            );
            failed_tool = Some(error);
        }
    }
    collect_request_snapshots(&request_rx, &mut snapshots);
    let proposals = runtime.state.proposals.read().await;
    assert!(
        proposals.is_empty(),
        "protected ns_patch should fail before staging a proposal, got {:?}",
        proposals.keys().collect::<Vec<_>>()
    );
    drop(proposals);

    let second_request = snapshots.get(1).unwrap_or_else(|| {
        panic!(
            "expected replay to capture the second provider request; captured {} requests",
            snapshots.len()
        )
    });
    assert!(
        model_request_contains_tool_rejection(second_request, call_id),
        "expected second request to contain a tool rejection for protected call {call_id}; request={second_request:#?}"
    );
    assert!(
        !model_request_contains_staged_success(second_request, call_id),
        "protected tool failure must not be replayed as staged success; request={second_request:#?}"
    );
    assert!(
        failed_tool.is_some(),
        "expected protected tool failure before assertion"
    );
}

/// Regression test for the protected-manifest retry loop observed in the
/// `node-01c9e8fdc70e3ee8` headless TUI trace.
///
/// This is fixed-contract regression coverage tracked as resolved, not an
/// expected-failing case. It replays only the relevant failure shape instead
/// of the full trace.
///
/// The historical run repeatedly attempted the same `non_semantic_patch`
/// against workspace `Cargo.toml`, and the old tool response gave the
/// model another generic path hint instead of recording that this was a
/// repeated protected write. This test reads the historical headless trace
/// through the typed `evidence::Summary` projection, extracts the first two
/// real provider-emitted `non_semantic_patch` requests, decodes them as
/// typed `NsPatchParamsOwned`, rebases only the old workspace root onto this
/// test's isolated workspace, and then wraps the recovered model output as
/// `RawFullResponseRecord` lines loaded through `load_recorded_response_tape`.
///
/// The assertions pin the fixed contract: both protected attempts fail before
/// staging, no success/completion is emitted for either call, the second
/// denial is marked `retry_context.repeated = true`, and the next model
/// requests receive structured rejection messages rather than staged-success
/// payloads.
// regr:protectedrepeat:19-05-26_15-43
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn historical_trace_replay_marks_repeated_protected_ns_patch_before_staging() {
    let _recorded_replay_guard = recorded_replay_test_mutex().lock().await;
    let fixture = prepare_live_canary(
        "historical-trace-protected-cargo-repeat",
        "Replay historical repeated Cargo.toml protected edit attempts.",
    )
    .expect("prepare recorded replay fixture");

    let historical_requests = historical_repeated_cargo_ns_patch_requests(&fixture.workspace, 2);
    let call_ids = historical_requests
        .iter()
        .map(|request| request.call_id.as_str())
        .collect::<Vec<_>>();
    let tape =
        recorded_historical_ns_patch_tape(&fixture.artifact_root, historical_requests.clone());
    ploke_tui::llm::install_recorded_response_tape(tape);
    let _clear_tape = ClearRecordedTapeOnDrop;

    let (request_tx, request_rx) = std::sync::mpsc::channel();
    let _tap_guard = ploke_tui::llm::install_request_tap(request_tx);
    let (mut runtime, parent_id) = start_attempt_runtime(
        &fixture.workspace,
        &[],
        fixture.prompt.clone(),
        &SurfacePolicy::workspace_except_core(),
        None,
        &Timeouts::default(),
    )
    .await
    .expect("start recorded replay runtime");

    let mut snapshots = Vec::new();
    let mut failures = Vec::new();
    let mut completions = Vec::new();
    let deadline = tokio::time::Instant::now() + Duration::from_secs(15);
    while tokio::time::Instant::now() < deadline {
        runtime.app.pump_pending_events().await;
        collect_request_snapshots(&request_rx, &mut snapshots);
        if failures.len() >= 2 && snapshots.len() >= 3 {
            break;
        }

        let event = match tokio::time::timeout(Duration::from_millis(100), next_event(&mut runtime))
            .await
        {
            Ok(Ok(event)) => event,
            Ok(Err(err)) => panic!("recorded replay event stream failed: {err}"),
            Err(_) => continue,
        };

        match event {
            ploke_tui::AppEvent::System(
                ploke_tui::app_state::events::SystemEvent::ToolCallFailed {
                    parent_id: event_parent_id,
                    call_id: event_call_id,
                    error,
                    ..
                },
            ) if event_parent_id == parent_id
                && call_ids
                    .iter()
                    .any(|expected| event_call_id.as_ref() == *expected) =>
            {
                failures.push((event_call_id.to_string(), error));
            }
            ploke_tui::AppEvent::System(
                ploke_tui::app_state::events::SystemEvent::ToolCallCompleted {
                    parent_id: event_parent_id,
                    call_id: event_call_id,
                    ..
                },
            ) if event_parent_id == parent_id
                && call_ids
                    .iter()
                    .any(|expected| event_call_id.as_ref() == *expected) =>
            {
                completions.push(event_call_id.to_string());
            }
            _ => {}
        }
    }
    collect_request_snapshots(&request_rx, &mut snapshots);

    assert_eq!(
        failures.len(),
        2,
        "both historical protected attempts should fail before staging"
    );
    assert!(
        completions.is_empty(),
        "protected preflight should not emit completions for historical calls: {completions:?}"
    );
    let proposals = runtime.state.proposals.read().await;
    assert!(
        proposals.is_empty(),
        "historical protected Cargo.toml replay should not stage proposals, got {:?}",
        proposals.keys().collect::<Vec<_>>()
    );
    drop(proposals);

    let first = ploke_tui::tools::ToolErrorWire::parse(&failures[0].1)
        .expect("first historical protected failure should use tool error wire");
    let second = ploke_tui::tools::ToolErrorWire::parse(&failures[1].1)
        .expect("second historical protected failure should use tool error wire");
    assert_eq!(
        first.llm.code,
        ploke_tui::tools::ToolErrorCode::InvalidFormat
    );
    assert_eq!(
        second.llm.code,
        ploke_tui::tools::ToolErrorCode::InvalidFormat
    );
    assert_eq!(retry_context_bool(&first, "repeated"), Some(false));
    assert_eq!(retry_context_bool(&second, "repeated"), Some(true));
    assert!(
        second
            .llm
            .retry_hint
            .as_deref()
            .is_some_and(|hint| hint.contains("already denied")),
        "repeat denial should tell the model the target was already denied: {:?}",
        second.llm.retry_hint
    );

    let second_request = snapshots.get(1).unwrap_or_else(|| {
        panic!(
            "expected second provider request after first rejection; captured {} requests",
            snapshots.len()
        )
    });
    assert!(
        model_request_contains_tool_rejection(second_request, call_ids[0]),
        "expected second request to carry first protected rejection; request={second_request:#?}"
    );
    assert!(
        !model_request_contains_staged_success(second_request, call_ids[0]),
        "first protected rejection must not be converted to staged success; request={second_request:#?}"
    );

    let third_request = snapshots.get(2).unwrap_or_else(|| {
        panic!(
            "expected third provider request after repeated rejection; captured {} requests",
            snapshots.len()
        )
    });
    assert!(
        model_request_contains_tool_rejection(third_request, call_ids[1]),
        "expected third request to carry repeated protected rejection; request={third_request:#?}"
    );
    assert!(
        !model_request_contains_staged_success(third_request, call_ids[1]),
        "repeated protected rejection must not be converted to staged success; request={third_request:#?}"
    );
}

/// Fixed-contract replay coverage for RF-05: repeated same-file repair
/// attempts must settle through the real tool/proposal loop without leaving
/// a malformed intermediate artifact.
///
/// The provider tape asks for one valid `non_semantic_patch` edit and then a
/// stale same-file repair against the pre-apply content. The fixed behavior
/// is: the first edit applies, the stale repair fails before staging a
/// second proposal, the next provider request receives a rejection for the
/// stale call, and the workspace remains at the first valid edit.
///
/// Related RF-05 bug report:
/// docs/active/bugs/2026-05-19-rf-05-edit-composition-same-file-repair.md.
// regr:samefile:19-05-26_06-42
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn recorded_replay_rejects_stale_same_file_repair_after_first_apply() {
    let _recorded_replay_guard = recorded_replay_test_mutex().lock().await;
    let fixture = prepare_live_canary(
        "recorded-same-file-stale-repair",
        "Replay repeated same-file non_semantic_patch repair attempts.",
    )
    .expect("prepare recorded same-file replay fixture");

    let first_call_id = "call_same_file_first_apply";
    let stale_call_id = "call_same_file_stale_repair";
    let tape = recorded_same_file_repair_tape(&fixture.artifact_root, first_call_id, stale_call_id);
    ploke_tui::llm::install_recorded_response_tape(tape);
    let _clear_tape = ClearRecordedTapeOnDrop;

    let (request_tx, request_rx) = std::sync::mpsc::channel();
    let _tap_guard = ploke_tui::llm::install_request_tap(request_tx);
    let (mut runtime, parent_id) = start_attempt_runtime(
        &fixture.workspace,
        &[],
        fixture.prompt.clone(),
        &SurfacePolicy::workspace_except_core(),
        None,
        &Timeouts::default(),
    )
    .await
    .expect("start same-file recorded replay runtime");

    let mut run = HeadlessRun::new();
    let (outcome, runtime) = run_attempt(
        runtime,
        parent_id,
        &fixture.workspace,
        &SurfacePolicy::workspace_except_core(),
        1,
        &mut run,
        &LiveObserver::disabled(),
        &[],
        None,
        Timeouts::default(),
    )
    .await
    .expect("same-file recorded replay should finish");

    let AttemptEnd::Terminal(HeadlessTerminal::Applied {
        applied_proposal_ids,
        changed_paths,
        ..
    }) = outcome
    else {
        panic!("same-file replay should finish with one applied edit, got {outcome:?}");
    };
    assert_eq!(
        applied_proposal_ids.len(),
        1,
        "stale same-file repair must not become a second applied proposal"
    );
    assert_eq!(
        changed_paths,
        vec![fixture.src_file.clone()],
        "only src/lib.rs should change"
    );

    let stale_failed = run.events().iter().any(|event| {
        matches!(
            event,
            Event::Tool {
                call_id,
                result: Tool::Failed { error },
            } if call_id == stale_call_id
                && (error.contains("failed to patch")
                    || error.contains("No non-semantic edits were applied")
                    || error.contains("No patches were found")
                    || error.contains("Patch applied partially"))
        )
    });
    assert!(
        stale_failed,
        "stale same-file repair should fail before staging; events={:#?}",
        run.events()
    );
    assert!(
        !run.events().iter().any(|event| {
            matches!(
                event,
                Event::Tool {
                    call_id,
                    result: Tool::Completed { .. },
                } if call_id == stale_call_id
            )
        }),
        "stale same-file repair must not produce a ToolCallCompleted success"
    );

    let proposals = runtime.state.proposals.read().await;
    assert_eq!(
        proposals.len(),
        1,
        "only the first same-file proposal should remain recorded, got {:?}",
        proposals.keys().collect::<Vec<_>>()
    );
    drop(proposals);

    let final_src =
        fs::read_to_string(&fixture.src_file).expect("read final same-file replay source");
    assert!(
        final_src.contains(r#""after""#),
        "first same-file edit should apply, got:\n{final_src}"
    );
    assert!(
        !final_src.contains("repair") && !final_src.contains("START RESTORE"),
        "stale repair artifacts must not be written, got:\n{final_src}"
    );

    let mut snapshots = Vec::new();
    collect_request_snapshots(&request_rx, &mut snapshots);
    let second_request = snapshots.get(1).unwrap_or_else(|| {
        panic!(
            "expected second provider request after first apply; captured {} requests",
            snapshots.len()
        )
    });
    assert!(
        model_request_contains_applied_success(second_request, first_call_id),
        "expected second request to contain settled applied result; request={second_request:#?}"
    );
    assert!(
        !model_request_contains_staged_success(second_request, first_call_id),
        "first same-file edit must not be replayed as staged-only success; request={second_request:#?}"
    );

    let third_request = snapshots.get(2).unwrap_or_else(|| {
        panic!(
            "expected third provider request after stale repair rejection; captured {} requests",
            snapshots.len()
        )
    });
    assert!(
        model_request_contains_ns_patch_failure(third_request, stale_call_id),
        "expected third request to carry stale same-file rejection; request={third_request:#?}"
    );
}

/// Replay regression for stale snippet rows seen as ploke-embed warnings
/// after a tape-driven edit truncated a file.
///
/// The recorded provider tape removes `stale_index_canary` from `src/lib.rs`
/// and shortens the file. Before the fix, post-apply refresh only retracted
/// embeddings, so the stale function row could survive and later snippet
/// extraction would report `ContentMismatch` or an out-of-range byte span.
///
/// Related bug report:
/// docs/active/bugs/2026-06-04-prototype1-post-apply-stale-snippet-indexing.md.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn recorded_replay_truncating_patch_removes_stale_snippet_rows() {
    let _recorded_replay_guard = recorded_replay_test_mutex().lock().await;

    // Start from the same live-canary workspace builder used by the other
    // headless TUI replay tests, then replace its source with a two-function
    // file. `stale_index_canary` is the row this test expects indexing to
    // retract after the recorded patch deletes it.
    let fixture = prepare_live_canary(
        "recorded-truncating-patch-stale-snippet",
        "Replay a truncating non_semantic_patch that removes stale_index_canary.",
    )
    .expect("prepare recorded stale snippet fixture");
    install_stale_snippet_canary_source(&fixture);

    let call_id = "call_truncate_stale_snippet_rows";
    let patch_diff = truncating_stale_snippet_ns_patch_diff();
    let initial_lib = fs::read_to_string(&fixture.src_file).expect("read initial stale fixture");
    println!(
        "\n=== stale snippet replay: setup ===\n  workspace: {}\n  src_file: {}\n  artifact_root: {}\n  call_id: {}\n  initial_bytes: {}\n  initial_contains_stale_index_canary: {}\n  recorded_patch:\n{}",
        fixture.workspace.display(),
        fixture.src_file.display(),
        fixture.artifact_root.display(),
        call_id,
        initial_lib.len(),
        initial_lib.contains("stale_index_canary"),
        patch_diff
    );

    // The tape is intentionally just one `non_semantic_patch` tool call
    // followed by a stop response. That keeps the replay focused on the
    // production apply/refresh path, not on provider behavior.
    let tape = recorded_truncating_lib_patch_tape(&fixture.artifact_root, call_id);
    ploke_tui::llm::install_recorded_response_tape(tape);
    let _clear_tape = ClearRecordedTapeOnDrop;

    // Runtime startup performs the initial workspace scan. If this fails to
    // index `stale_index_canary`, the test is not reproducing the stale-row
    // condition seen in the live warning.
    let (mut runtime, parent_id) = start_attempt_runtime(
        &fixture.workspace,
        &[],
        fixture.prompt.clone(),
        &SurfacePolicy::workspace_except_core(),
        None,
        &Timeouts::default(),
    )
    .await
    .expect("start stale snippet replay runtime");

    let stale_rows_before = function_rows_by_name(&runtime, "stale_index_canary");
    print_headless_replay_rows("indexed fixture before replay", &stale_rows_before);
    assert!(
        !stale_rows_before.rows.is_empty(),
        "fixture must index stale_index_canary before the truncating replay"
    );

    // `run_attempt` is the same adapter path used by broad headless TUI
    // parent patch generation: consume model output, stage the tool edit,
    // apply it, and wait for the post-apply refresh barrier.
    let mut run = HeadlessRun::new();
    let (outcome, runtime) = run_attempt(
        runtime,
        parent_id,
        &fixture.workspace,
        &SurfacePolicy::workspace_except_core(),
        1,
        &mut run,
        &LiveObserver::disabled(),
        &[],
        None,
        Timeouts::default(),
    )
    .await
    .expect("truncating recorded replay should finish");
    println!(
        "\n=== stale snippet replay: run_attempt outcome ===\n  outcome:\n{:#?}",
        outcome
    );

    // Require a real applied terminal so the final DB assertion is about the
    // post-apply refresh contract, not about a failed or skipped patch.
    assert!(
        matches!(
            outcome,
            AttemptEnd::Terminal(HeadlessTerminal::Applied { .. })
        ),
        "truncating replay should apply one patch, got {outcome:?}"
    );

    // The file-level assertion proves the recorded patch produced the
    // truncated workspace state that would make old byte spans invalid.
    let final_lib = fs::read_to_string(&fixture.src_file).expect("read final stale fixture");
    assert!(
        !final_lib.contains("stale_index_canary"),
        "recorded truncating patch should remove stale_index_canary"
    );

    // This is the contract check for the original warning. If this row
    // survives, later embedding/indexing work can ask IO to read a snippet
    // whose hash or byte span belongs to the pre-apply file.
    let stale_rows_after = function_rows_by_name(&runtime, "stale_index_canary");
    println!(
        "\n=== stale snippet replay: final file ===\n  final_bytes: {}\n  final_contains_stale_index_canary: {}",
        final_lib.len(),
        final_lib.contains("stale_index_canary")
    );

    // Also prove the refresh did not only delete stale rows. The surviving
    // function should still be queryable, and its refreshed span should
    // point at actual post-edit source text.
    let surviving_rows_after = function_rows_by_name(&runtime, "broad_surface_canary");
    print_headless_replay_rows(
        "post-apply refreshed surviving function",
        &surviving_rows_after,
    );
    print_headless_replay_span_content(
        "post-apply refreshed surviving function",
        &final_lib,
        &surviving_rows_after,
    );
    let surviving_content =
        first_function_span_content(&final_lib, &surviving_rows_after).unwrap_or_else(|| {
            panic!(
                "expected refreshed broad_surface_canary span to resolve in final source; rows={surviving_rows_after:#?}"
            )
        });
    assert!(
        surviving_content.contains("broad_surface_canary")
            && surviving_content.contains("\"after\""),
        "refreshed span should point at post-edit broad_surface_canary content; content={surviving_content:?}"
    );

    print_headless_replay_rows("post-apply refresh after replay", &stale_rows_after);
    assert!(
        stale_rows_after.rows.is_empty(),
        "post-apply refresh must retract stale function rows before indexing can request stale snippets; rows={stale_rows_after:#?}"
    );
}

/// Replay regression for the concrete target that produced the live
/// `ContentMismatch`: `crates/ploke-tree/src/lib.rs::assemble_run_forest`.
///
/// This copies a fixture snapshot of ploke-tree lib source into a temp workspace at
/// the same relative path, applies a recorded edit to `assemble_run_forest`,
/// then prints and asserts the refreshed DB span resolves to the edited
/// post-apply function body.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn recorded_replay_actual_ploke_tree_target_refreshes_assemble_run_forest_span() {
    let _recorded_replay_guard = recorded_replay_test_mutex().lock().await;
    let fixture = prepare_live_canary(
        "recorded-ploke-tree-assemble-run-forest",
        "Replay a non_semantic_patch against crates/ploke-tree/src/lib.rs.",
    )
    .expect("prepare recorded ploke-tree target fixture");
    let target_file = install_ploke_tree_assemble_target(&fixture);

    let call_id = "call_patch_assemble_run_forest";
    let patch_diff = ploke_tree_assemble_run_forest_ns_patch_diff();
    let initial_lib = fs::read_to_string(&target_file).expect("read initial ploke-tree target");
    println!(
        "\n=== ploke-tree target replay: setup ===\n  workspace: {}\n  target_file: {}\n  artifact_root: {}\n  call_id: {}\n  initial_bytes: {}\n  initial_contains_assemble_run_forest: {}\n  recorded_patch:\n{}",
        fixture.workspace.display(),
        target_file.display(),
        fixture.artifact_root.display(),
        call_id,
        initial_lib.len(),
        initial_lib.contains("assemble_run_forest"),
        patch_diff
    );

    let tape = recorded_ploke_tree_assemble_patch_tape(&fixture.artifact_root, call_id);
    ploke_tui::llm::install_recorded_response_tape(tape);
    let _clear_tape = ClearRecordedTapeOnDrop;

    let (mut runtime, parent_id) = start_attempt_runtime(
        &fixture.workspace,
        &[],
        fixture.prompt.clone(),
        &SurfacePolicy::workspace_except_core(),
        None,
        &Timeouts::default(),
    )
    .await
    .expect("start ploke-tree target replay runtime");

    let target_rows_before = function_rows_by_name(&runtime, "assemble_run_forest");
    print_headless_replay_rows("ploke-tree target before replay", &target_rows_before);
    print_headless_replay_span_content(
        "ploke-tree target before replay",
        &initial_lib,
        &target_rows_before,
    );
    assert!(
        !target_rows_before.rows.is_empty(),
        "fixture must index assemble_run_forest before replay"
    );

    let mut run = HeadlessRun::new();
    let (outcome, runtime) = run_attempt(
        runtime,
        parent_id,
        &fixture.workspace,
        &SurfacePolicy::workspace_except_core(),
        1,
        &mut run,
        &LiveObserver::disabled(),
        &[],
        None,
        Timeouts::default(),
    )
    .await
    .expect("ploke-tree target recorded replay should finish");
    println!(
        "\n=== ploke-tree target replay: run_attempt outcome ===\n  outcome:\n{:#?}",
        outcome
    );

    assert!(
        matches!(
            outcome,
            AttemptEnd::Terminal(HeadlessTerminal::Applied { .. })
        ),
        "ploke-tree target replay should apply one patch, got {outcome:?}"
    );

    let final_lib = fs::read_to_string(&target_file).expect("read final ploke-tree target");
    assert!(
        final_lib.contains("_post_apply_refresh_canary"),
        "recorded patch should add the post-apply refresh canary to assemble_run_forest"
    );

    let target_rows_after = function_rows_by_name(&runtime, "assemble_run_forest");
    println!(
        "\n=== ploke-tree target replay: final file ===\n  final_bytes: {}\n  final_contains_post_apply_refresh_canary: {}",
        final_lib.len(),
        final_lib.contains("_post_apply_refresh_canary")
    );
    print_headless_replay_rows("post-apply refreshed ploke-tree target", &target_rows_after);
    print_headless_replay_span_content(
        "post-apply refreshed ploke-tree target",
        &final_lib,
        &target_rows_after,
    );

    let refreshed_content =
        first_function_span_content(&final_lib, &target_rows_after).unwrap_or_else(|| {
            panic!(
                "expected refreshed assemble_run_forest span to resolve in final source; rows={target_rows_after:#?}"
            )
        });
    assert!(
        refreshed_content.contains("assemble_run_forest")
            && refreshed_content.contains("_post_apply_refresh_canary")
            && refreshed_content.contains("\"after\""),
        "refreshed span should point at post-edit assemble_run_forest content; content={refreshed_content:?}"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn gated_replay_sends_applied_ns_patch_instead_of_staged_success() {
    let _recorded_replay_guard = recorded_replay_test_mutex().lock().await;
    let fixture = prepare_live_canary(
        "recorded-gated-applied-ns-patch",
        "Use non_semantic_patch to update src/lib.rs.",
    )
    .expect("prepare recorded gated fixture");

    let call_id = "call_gated_allowed_ns_patch";
    let tape = recorded_allowed_ns_patch_tape(&fixture.artifact_root, call_id);
    ploke_tui::llm::install_recorded_response_tape(tape);
    let _clear_tape = ClearRecordedTapeOnDrop;

    let (request_tx, request_rx) = std::sync::mpsc::channel();
    let _tap_guard = ploke_tui::llm::install_request_tap(request_tx);
    let (mut runtime, parent_id) = start_attempt_runtime(
        &fixture.workspace,
        &[],
        fixture.prompt.clone(),
        &SurfacePolicy::workspace_except_core(),
        None,
        &Timeouts::default(),
    )
    .await
    .expect("start gated recorded replay runtime");

    let mut run = HeadlessRun::new();
    let (outcome, runtime) = run_attempt(
        runtime,
        parent_id,
        &fixture.workspace,
        &SurfacePolicy::workspace_except_core(),
        1,
        &mut run,
        &LiveObserver::disabled(),
        &[],
        None,
        Timeouts::default(),
    )
    .await
    .expect("gated recorded replay should finish");
    assert!(
        matches!(
            outcome,
            AttemptEnd::Terminal(HeadlessTerminal::Applied { .. })
        ),
        "allowed ns_patch replay should apply, got {outcome:?}"
    );

    let mut snapshots = Vec::new();
    collect_request_snapshots(&request_rx, &mut snapshots);
    let second_request = snapshots.get(1).unwrap_or_else(|| {
        panic!(
            "expected second provider request after settled apply; captured {} requests",
            snapshots.len()
        )
    });
    assert!(
        model_request_contains_applied_success(second_request, call_id),
        "expected second request to contain settled applied result; request={second_request:#?}"
    );
    assert!(
        !model_request_contains_staged_success(second_request, call_id),
        "gated tool loop must not replay staged success before eval admission; request={second_request:#?}"
    );
    let final_src = fs::read_to_string(&fixture.src_file).expect("read final gated replay source");
    assert!(
        final_src.contains(r#""after""#),
        "gated replay should update source after admission, got:\n{final_src}"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn recorded_replay_runs_declared_validation_after_applied_edit() {
    let _recorded_replay_guard = recorded_replay_test_mutex().lock().await;
    let fixture = prepare_live_canary(
        "recorded-declared-validation-after-apply",
        "Use non_semantic_patch to update src/lib.rs, then stop.",
    )
    .expect("prepare recorded validation fixture");

    let call_id = "call_validation_allowed_ns_patch";
    let tape = recorded_allowed_ns_patch_tape(&fixture.artifact_root, call_id);
    ploke_tui::llm::install_recorded_response_tape(tape);
    let _clear_tape = ClearRecordedTapeOnDrop;
    let (mut runtime, parent_id) = start_attempt_runtime(
        &fixture.workspace,
        &[],
        fixture.prompt.clone(),
        &SurfacePolicy::workspace_except_core(),
        None,
        &Timeouts::default(),
    )
    .await
    .expect("start declared validation replay runtime");

    let mut run = HeadlessRun::new();
    let validations = vec![declared_cargo_command("check canary crate", &["check"])];
    let (outcome, runtime) = run_attempt(
        runtime,
        parent_id,
        &fixture.workspace,
        &SurfacePolicy::workspace_except_core(),
        1,
        &mut run,
        &LiveObserver::disabled(),
        &validations,
        None,
        Timeouts::default(),
    )
    .await
    .expect("declared validation replay should finish");

    assert!(
        matches!(
            outcome,
            AttemptEnd::Terminal(HeadlessTerminal::Applied { .. })
        ),
        "adapter should run request-declared validation after apply, got {outcome:?}; validations={:#?}",
        run.validations()
    );
    assert!(
        run.validations()
            .iter()
            .any(|validation| validation.display_command == "cargo check" && validation.ok),
        "expected successful harness-owned `cargo check`, got {:#?}",
        run.validations()
    );

    let final_src =
        fs::read_to_string(&fixture.src_file).expect("read final validated replay source");
    assert!(
        final_src.contains(r#""after""#),
        "validated replay should update source after admission, got:\n{final_src}"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn applied_batch_finalizes_before_completed_turn() {
    let _recorded_replay_guard = recorded_replay_test_mutex().lock().await;
    let fixture = prepare_live_canary(
        "finalize-on-applied-batch-no-completed-turn",
        "Use non_semantic_patch to update src/lib.rs, then stop.",
    )
    .expect("prepare finalize fixture");

    let call_id = "call_finalize_on_applied_batch";
    let tape = recorded_allowed_ns_patch_tape(&fixture.artifact_root, call_id);
    ploke_tui::llm::install_recorded_response_tape(tape);
    let _clear_tape = ClearRecordedTapeOnDrop;
    let (mut runtime, parent_id) = start_attempt_runtime(
        &fixture.workspace,
        &[],
        fixture.prompt.clone(),
        &SurfacePolicy::workspace_except_core(),
        None,
        &Timeouts::default(),
    )
    .await
    .expect("start finalize replay runtime");

    let mut run = HeadlessRun::new();
    let validations = vec![declared_cargo_command("check canary crate", &["check"])];
    let (outcome, runtime) = run_attempt(
        runtime,
        parent_id,
        &fixture.workspace,
        &SurfacePolicy::workspace_except_core(),
        1,
        &mut run,
        &LiveObserver::disabled(),
        &validations,
        None,
        Timeouts::default(),
    )
    .await
    .expect("finalize replay should finish");

    assert!(
        matches!(
            outcome,
            AttemptEnd::Terminal(HeadlessTerminal::Applied { .. })
        ),
        "passing applied batch should classify Applied without waiting for a completed turn, got {outcome:?}; validations={:#?}",
        run.validations()
    );
    assert!(
        !run.events()
            .iter()
            .any(|event| matches!(event, Event::Turn { .. })),
        "finalize must classify at the applied batch, before any completed chat turn is observed; events={:#?}",
        run.events()
    );
    assert!(
        run.validations()
            .iter()
            .any(|validation| validation.display_command == "cargo check" && validation.ok),
        "expected harness-owned `cargo check` to run at finalize, got {:#?}",
        run.validations()
    );
}

// C1 regression: `Attempt::run` with `Capture::Responses` must keep the
// process-global response-tap guard installed across the whole attempt await.
// If the guard is dropped early, `clear_response_tap` fires before the session
// runs and no provider envelopes are captured, leaving `full_response_records`
// empty even though the model produced responses.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn attempt_capture_responses_keeps_tap_installed_across_run() {
    let _recorded_replay_guard = recorded_replay_test_mutex().lock().await;
    let fixture = prepare_live_canary(
        "attempt-capture-responses-tap-lifetime",
        "Use non_semantic_patch to update src/lib.rs, then stop.",
    )
    .expect("prepare capture-responses fixture");

    let call_id = "call_capture_responses_allowed_ns_patch";
    let tape = recorded_allowed_ns_patch_tape(&fixture.artifact_root, call_id);
    ploke_tui::llm::install_recorded_response_tape(tape);
    let _clear_tape = ClearRecordedTapeOnDrop;

    let attempt = Attempt {
        workspace: fixture.workspace.clone(),
        prompt: fixture.prompt.clone(),
        budget: Budget::new(1, 120).expect("valid one-attempt budget"),
        surface: SurfacePolicy::workspace_except_core(),
        evidence: Vec::new(),
        validation: Vec::new(),
        model: None,
        capture: Capture::Responses,
    };
    let outcome = attempt
        .run()
        .await
        .expect("capture-responses attempt should return an outcome");

    assert!(
        !outcome.run.full_response_records().is_empty(),
        "Capture::Responses must drain at least one provider response; the tap \
         guard was dropped before the run if this is empty. terminal={:?}",
        outcome.run.terminal()
    );
}

#[test]
fn response_tap_drain_rebases_session_local_indices_to_run_tape() {
    let assistant_id = Uuid::from_u128(0xaaaaaaaa_aaaa_aaaa_aaaa_aaaaaaaaaaaa);
    let (tx, rx) = std::sync::mpsc::channel();
    let rx = Mutex::new(rx);
    let mut run = HeadlessRun::new();

    tx.send(stop_response_record(assistant_id, 0, "first-local-zero").recorded_response)
        .expect("send first local response");
    drain_response_records(&mut run, assistant_id, Some(&rx));

    tx.send(stop_response_record(assistant_id, 0, "second-local-zero").recorded_response)
        .expect("send second local response");
    drain_response_records(&mut run, assistant_id, Some(&rx));

    let indices = run
        .full_response_records()
        .iter()
        .map(|record| record.response_index().get())
        .collect::<Vec<_>>();
    assert_eq!(
        indices,
        vec![0, 1],
        "turn-live sidecars need monotonic replay indices even when each chat session reports local chain_index=0"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn historical_r10_near_tail_turn_live_tape_applies_ns_patch_through_tool_loop() {
    const FINAL_EVENT_INDEX: usize = 95;
    const HISTORICAL_STOP_RESPONSE_INDEX: usize = 34;

    let _env = crate::test_support::env_guard_os(vec![]);
    assert!(
        std::env::var_os("PLOKE_EVAL_BROAD_TUI_SUMMARY_FIXTURE").is_none(),
        "historical r10 replay must not run with the broad TUI summary fixture hook enabled"
    );
    let _recorded_replay_guard = recorded_replay_test_mutex().lock().await;
    let turn_live_dir = historical_r10_turn_live_dir();
    assert!(
        turn_live_dir.exists(),
        "expected historical r10 turn-live bundle at {}",
        turn_live_dir.display()
    );

    let cursor = ploke_tree::TurnCursor {
        artifact_kind: ploke_tree::TurnArtifactKind::Trace,
        artifact_path: "agent-turn-trace.json".to_string(),
        event_index: FINAL_EVENT_INDEX,
    };
    let (prefix, selected) = crate::replay::turn::resolve_replay_prefix_at(
        &turn_live_dir,
        &cursor,
        crate::replay::turn::ReplayPrefixSelector::ThroughEvent {
            cursor: cursor.clone(),
        },
        crate::replay::turn::ReplayTail::Stop,
    )
    .expect("historical r10 completed turn cursor should resolve through turn-live tape");
    let selected_record_count = selected.records().len();
    let mut selected_response_indices = selected
        .records()
        .iter()
        .map(|record| record.response_index().get())
        .collect::<Vec<_>>();
    selected_response_indices.sort_unstable();
    selected_response_indices.dedup();
    let selected_duplicate_count =
        selected_record_count.saturating_sub(selected_response_indices.len());
    println!(
        "\n=== historical r10 replay: resolved prefix ===\n  turn_live_dir: {}\n  trace_event_index: {}\n  through_response_index: {:#?}\n  unique_selected_response_indices: {:#?}\n  selected_record_count: {}\n  duplicate_sidecar_records: {}",
        turn_live_dir.display(),
        FINAL_EVENT_INDEX,
        prefix.through_response_index,
        selected_response_indices,
        selected_record_count,
        selected_duplicate_count
    );
    assert_eq!(prefix.anchor.call_id, None);
    assert_eq!(
        prefix.through_response_index,
        Some(HISTORICAL_STOP_RESPONSE_INDEX),
        "turn-live completed cursor should map to the final historical stop response"
    );
    assert!(
        selected
            .records()
            .iter()
            .any(|record| response_record_contains_tool_call(
                record,
                "function-call-34662591-b7b8-4c3e-b589-f70d5b7fb2c1",
                "non_semantic_patch"
            )),
        "selected prefix should include the historical repair ns_patch response"
    );

    let fixture = prepare_live_canary(
        "historical-r10-post-stop-admission",
        "Replay historical r10 through broad headless-TUI admission.",
    )
    .expect("prepare historical r10 admission fixture");
    // This fixture preserves the historical ordering requirement:
    // the first r10 patch introduces `+ nth` (where `nth: &mut f64`, so
    // `f64 + &mut f64` does not compile), and the repair patch changes it to
    // `+ *nth`, which compiles. `ploke-eval` depends on `ploke-selection-score`,
    // so the buildability gate (`cargo check -p ploke-eval`) fails on the first
    // patch and passes only after the repair. The harness validates each applied
    // batch and finalizes as soon as the declared validation passes, so
    // admission happens at the repaired batch rather than at a later completed
    // chat turn: the first patch fails to compile, the attempt keeps running
    // (continue-on-fail), the repair lands, and the build check then passes.
    install_historical_r10_selection_score_workspace(&fixture);
    let request_path = write_historical_r10_admission_request(&fixture);
    let tape = historical_r10_completed_tail_tape(&fixture.artifact_root, &turn_live_dir);
    println!(
        "\n=== historical r10 replay: fixture and request ===\n  workspace: {}\n  artifact_root: {}\n  request_path: {}",
        fixture.workspace.display(),
        fixture.artifact_root.display(),
        request_path.display()
    );
    ploke_tui::llm::install_recorded_response_tape(tape);
    let _clear_tape = ClearRecordedTapeOnDrop;

    let projection =
        crate::cli::prototype1_state::cli_facing::run_broad_harness_attempt_from_request_path(
            request_path.clone(),
            crate::cli::prototype1_state::cli_facing::BroadTuiAttemptOptions::default(),
        )
        .await
        .expect("historical r10 completed turn should validate and publish a submitted result");

    println!(
        "\n=== historical r10 replay: broad attempt projection ===\n  status: {}\n  changed_paths: {:#?}\n  diagnostics_path: {}\n  submitted_result_path: {}",
        projection.status,
        projection.changed_paths,
        projection.diagnostics_path.display(),
        projection.submitted_result_path.display()
    );
    assert_eq!(projection.status, "applied");
    assert!(
        projection.submitted_result_path.exists(),
        "post-stop validation should publish submitted result at {}",
        projection.submitted_result_path.display()
    );
    assert!(
        projection.changed_paths.iter().any(|path| path.as_path()
            == Path::new("crates/ploke-selection-score/src/ploke/frontier.rs")),
        "historical r10 admission should report the repaired frontier change, got {:#?}",
        projection.changed_paths
    );

    let published = read_published_request(&request_path);
    let declared_commands = published
        .request()
        .contract
        .validation
        .commands
        .iter()
        .map(validation_command_display)
        .collect::<Vec<_>>();
    println!(
        "\n=== historical r10 replay: declared validation contract ===\n  commands: {:#?}",
        declared_commands
    );
    assert_eq!(
        declared_commands,
        vec!["cargo check -p ploke-eval".to_string()],
        "test must exercise the buildability admission gate (immutability and quality are enforced elsewhere)"
    );
    let submitted = read_submitted_result(&projection.submitted_result_path);
    submitted
        .verify_request(&published)
        .expect("submitted result should be request-bound and admissible");

    let diagnostics = read_headless_summary(&projection.diagnostics_path);
    let completed_turn_count = diagnostics
        .events
        .iter()
        .filter(|event| {
            matches!(
                event,
                evidence::Event::Turn { outcome, .. } if outcome == "completed"
            )
        })
        .count();
    let validation_receipts = diagnostics
        .validations
        .iter()
        .map(|validation| {
            format!(
                "{} ok={} command={}",
                validation.call_id, validation.ok, validation.display_command
            )
        })
        .collect::<Vec<_>>();
    println!(
        "\n=== historical r10 replay: diagnostics after admission ===\n  terminal: {:#?}\n  completed_turn_count: {}\n  validations: {:#?}",
        diagnostics.terminal, completed_turn_count, validation_receipts
    );
    assert!(
        matches!(
            diagnostics.terminal,
            Some(evidence::Terminal::Applied { .. })
        ),
        "diagnostics should record an applied terminal after post-stop validation, got {:#?}",
        diagnostics.terminal
    );
    // The harness now finalizes at the validated repair batch instead of
    // depending on the model emitting a completed chat turn first. Admission
    // must therefore not require a completed turn; the repaired-content and
    // passing-declared-validation guarantees below are what gate admission.
    assert_eq!(
        completed_turn_count, 0,
        "post-apply finalize should admit at the validated repair batch, before any completed chat turn; events={:#?}",
        diagnostics.events
    );
    assert!(
        diagnostics.validations.iter().any(|validation| {
            validation.call_id == "declared_validation_1_0"
                && validation.display_command == "cargo check -p ploke-eval"
                && validation.ok
        }),
        "adapter should run the request-declared buildability validation and pass it at the repaired batch; got {:#?}",
        diagnostics.validations
    );
    assert!(
        diagnostics.validations.iter().any(|validation| {
            validation.call_id == "declared_validation_1_0"
                && validation.display_command == "cargo check -p ploke-eval"
                && !validation.ok
        }),
        "the broken first patch must fail the buildability gate before the repair lands; got {:#?}",
        diagnostics.validations
    );

    let target_file = published
        .workspace_path()
        .join("crates/ploke-selection-score/src/ploke/frontier.rs");
    let final_src = fs::read_to_string(&target_file).expect("read final historical r10 target");
    println!(
        "\n=== historical r10 replay: final source check ===\n  target_file: {}\n  contains_repaired_deref: {}\n  contains_unrepaired_nth: {}",
        target_file.display(),
        final_src.contains("(left.iter().sum::<f64>() + *nth) / count as f64"),
        final_src.contains("(left.iter().sum::<f64>() + nth) / count as f64")
    );
    assert!(
        final_src.contains("(left.iter().sum::<f64>() + *nth) / count as f64"),
        "historical r10 patch should dereference nth, got:\n{final_src}"
    );
    assert!(
        !final_src.contains("(left.iter().sum::<f64>() + nth) / count as f64"),
        "historical r10 pre-patch expression should be gone, got:\n{final_src}"
    );
}

// RED regression for the 2026-06-02 direct-Google broad-headless run:
// `Budget::max_attempts == 1` bounded only the outer harness turn while the
// inner TUI tool loop could keep making provider-step requests. Run with
// `--ignored` until a provider-step cap is wired into this adapter path.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "RED until broad headless TUI enforces a provider-step cap"]
async fn xfail_broad_headless_caps_provider_steps() {
    let _recorded_replay_guard = recorded_replay_test_mutex().lock().await;
    let fixture = prepare_live_canary(
        "recorded-provider-step-budget",
        "Replay repeated protected edits to exercise the inner provider-step budget.",
    )
    .expect("prepare provider-step budget fixture");

    let expected_cap = 15_usize;
    let replay_steps = expected_cap + 5;
    let tape = repeated_protected_ns_patch_tape(&fixture.artifact_root, replay_steps);
    ploke_tui::llm::install_recorded_response_tape(tape);
    let _clear_tape = ClearRecordedTapeOnDrop;

    let (request_tx, request_rx) = std::sync::mpsc::channel();
    let _tap_guard = ploke_tui::llm::install_request_tap(request_tx);
    let budget = Budget::new(1, 120).expect("valid one-attempt budget");
    let run = run_headless_with_model(
        &fixture.workspace,
        &fixture.prompt,
        budget,
        &SurfacePolicy::workspace_except_core(),
        &[],
        None,
    )
    .await
    .expect("recorded provider-step budget run should return evidence");

    let mut snapshots = Vec::new();
    collect_request_snapshots(&request_rx, &mut snapshots);
    let turn_attempts = run.events().iter().rev().find_map(|event| match event {
        Event::Turn { attempts, .. } => Some(*attempts),
        _ => None,
    });

    assert!(
        snapshots.len() <= expected_cap,
        "broad headless run must cap provider steps separately from outer max_attempts; \
         outer max_attempts=1, expected provider requests <= {expected_cap}, \
         observed {}, turn_attempts={turn_attempts:?}, terminal={:?}",
        snapshots.len(),
        run.terminal()
    );
    assert!(
        matches!(run.terminal(), Some(HeadlessTerminal::Exhausted { last, .. }) if last.contains("tool call chain limit")),
        "provider-step cap should surface as a budget/chain-limit terminal, got {:?}",
        run.terminal()
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "live provider test for the ploke-eval/ploke-tui broad edit surface"]
async fn live_tui_adapter_canary_shows_inputs_outputs_and_applied_edit() {
    let prompt = r#"Use the available edit tools to stage exactly one code edit in src/lib.rs.

Change broad_surface_canary so it returns "after" instead of "before".
Do not edit Cargo.toml. Do not create report, result, control, or bookkeeping files.
"#;
    let fixture =
        prepare_live_canary("live-tui-adapter-direct-file", prompt).expect("prepare canary");

    let run = run_live_canary(&fixture).await;
    let final_lib = write_live_canary_artifacts(&fixture, &run);
    assert_live_canary_applied(&fixture, &run, &final_lib);
    assert_workspace_index_ready(&fixture, &run);
    assert_auto_context_off(&fixture, &run);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "live provider test for indexed ploke-tui retrieval plus broad edit application"]
async fn live_tui_adapter_canary_uses_indexed_context_before_applied_edit() {
    let prompt = r#"Use request_code_context to locate the Rust function named broad_surface_canary, then use the available edit tools to stage exactly one code edit.

Change broad_surface_canary so it returns "after" instead of "before".
Do not edit Cargo.toml. Do not create report, result, control, or bookkeeping files.
"#;
    let fixture =
        prepare_live_canary("live-tui-adapter-indexed-context", prompt).expect("prepare canary");

    let run = run_live_canary(&fixture).await;
    let final_lib = write_live_canary_artifacts(&fixture, &run);
    let context_request = tool_request_position(&run, "request_code_context");
    let edit_request = tool_request_position(&run, "apply_code_edit");
    assert!(
        matches!((context_request, edit_request), (Some(context), Some(edit)) if context < edit),
        "expected request_code_context before edit; artifacts at {}",
        fixture.artifact_root.display()
    );
    assert_live_canary_applied(&fixture, &run, &final_lib);
    assert_workspace_index_ready(&fixture, &run);
    assert_auto_context_off(&fixture, &run);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "runtime setup canary for child-local parse/transform DB plus BM25 readiness"]
async fn live_tui_runtime_setup_uses_sparse_child_db() {
    let fixture = prepare_live_canary(
        "live-tui-adapter-sparse-child-db",
        "runtime setup only; no LLM prompt is submitted",
    )
    .expect("prepare canary");

    let mut runtime = crate::runner::setup_workspace_tui_runtime(&fixture.workspace)
        .await
        .unwrap_or_else(|err| {
            fs::write(
                fixture.artifact_root.join("setup-error.txt"),
                err.to_string(),
            )
            .expect("write setup error");
            panic!(
                "runtime setup failed; artifacts at {}",
                fixture.artifact_root.display()
            );
        });
    runtime.app.pump_pending_events().await;

    let cfg = runtime.state.config.read().await;
    assert!(
        matches!(
            cfg.rag.strategy,
            ploke_tui::user_config::RetrievalStrategyUser::Sparse { strict: true }
        ),
        "expected sparse-strict retrieval; artifacts at {}",
        fixture.artifact_root.display()
    );
    assert!(cfg.rag.strict_bm25_by_default);
    drop(cfg);

    let Some(rag) = runtime.state.rag.as_ref() else {
        panic!(
            "expected RAG service; artifacts at {}",
            fixture.artifact_root.display()
        );
    };
    let status = rag.bm25_status().await.expect("bm25 status");
    assert!(
        matches!(status, ploke_db::bm25_index::bm25_service::Bm25Status::Ready { docs } if docs > 0),
        "expected BM25 ready with documents, got {status:?}; artifacts at {}",
        fixture.artifact_root.display()
    );

    assert!(
        runtime.state.indexing_state.read().await.is_none(),
        "expected no dense indexing status from /index; artifacts at {}",
        fixture.artifact_root.display()
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "operator canary for an existing broad harness workspace"]
async fn live_tui_runtime_setup_existing_workspace_from_env() {
    let Some(workspace) = std::env::var_os("PLOKE_EVAL_EXISTING_TUI_WORKSPACE").map(PathBuf::from)
    else {
        println!("skipping: set PLOKE_EVAL_EXISTING_TUI_WORKSPACE to a workspace root");
        return;
    };

    let mut runtime = crate::runner::setup_workspace_tui_prompt_runtime(&workspace)
        .await
        .unwrap_or_else(|err| {
            panic!(
                "runtime setup failed for existing workspace '{}': {err}",
                workspace.display()
            );
        });
    runtime.app.pump_pending_events().await;

    let cfg = runtime.state.config.read().await;
    assert!(
        matches!(
            cfg.rag.strategy,
            ploke_tui::user_config::RetrievalStrategyUser::Sparse { strict: true }
        ),
        "expected sparse-strict retrieval for '{}'",
        workspace.display()
    );
    drop(cfg);

    let Some(rag) = runtime.state.rag.as_ref() else {
        panic!("expected RAG service for '{}'", workspace.display());
    };
    let status = rag
        .bm25_status_with_timeout(Duration::from_secs(5))
        .await
        .expect("bm25 status");
    assert!(
        matches!(status, ploke_db::bm25_index::bm25_service::Bm25Status::Ready { docs } if docs > 0),
        "expected BM25 ready with documents for '{}', got {status:?}",
        workspace.display()
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "operator canary for initial prompt RAG against the ploke workspace"]
async fn live_tui_initial_prompt_ploke_workspace_includes_rag_parts() {
    let workspace = std::env::var_os("PLOKE_EVAL_EXISTING_TUI_WORKSPACE")
        .map(PathBuf::from)
        .unwrap_or_else(ploke_workspace_root_for_test);
    let mut runtime = crate::runner::setup_workspace_tui_prompt_runtime(&workspace)
        .await
        .unwrap_or_else(|err| {
            panic!(
                "runtime setup failed for initial prompt RAG canary '{}': {err}",
                workspace.display()
            );
        });
    runtime.app.pump_pending_events().await;

    let prompt = r#"Inspect the indexed workspace context for setup_workspace_tui_runtime and run_broad_headless_tui_attempt.
Do not call tools. Do not propose edits. This canary only checks initial prompt context assembly."#;
    let parent_id = submit_prompt(&runtime.app, prompt.to_string())
        .await
        .expect("submit canary prompt");

    let diagnostic = tokio::time::timeout(Duration::from_secs(30), async {
        loop {
            runtime.app.pump_pending_events().await;
            match next_event(&mut runtime).await.expect("next app event") {
                ploke_tui::AppEvent::Llm(ploke_tui::llm::LlmEvent::ChatCompletion(
                    ploke_tui::llm::ChatEvt::PromptConstructed {
                        parent_id: observed,
                        formatted_prompt,
                        context_plan,
                    },
                )) if observed == parent_id => {
                    break PromptDiagnostic::capture(
                        &runtime.state,
                        observed,
                        &formatted_prompt,
                        &context_plan,
                    )
                    .await;
                }
                _ => {}
            }
        }
    })
    .await
    .unwrap_or_else(|_| {
        panic!(
            "timed out waiting for initial PromptConstructed event in '{}'",
            workspace.display()
        )
    });

    println!(
        "initial prompt RAG workspace={} bm25={:?} included_rag_parts={} fallback={:?}",
        workspace.display(),
        diagnostic.bm25,
        diagnostic.included_rag_parts,
        diagnostic.fallback_notice
    );
    assert!(
        diagnostic.workspace.loaded,
        "expected loaded workspace for '{}'",
        workspace.display()
    );
    assert!(
        matches!(diagnostic.bm25.as_ref(), Some(Bm25Diagnostic { status, docs: Some(docs), .. }) if status == "ready" && *docs > 0),
        "expected ready BM25 with documents for '{}', got {:?}",
        workspace.display(),
        diagnostic.bm25
    );
    assert!(
        diagnostic.fallback_notice.is_none(),
        "initial prompt fell back without code context for '{}': {:?}",
        workspace.display(),
        diagnostic.fallback_notice
    );
    assert!(
        diagnostic.included_rag_parts > 0,
        "initial prompt included no RAG parts for '{}': {:?}",
        workspace.display(),
        diagnostic.rag_stats
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "operator canary for ploke workspace prompt construction with context mode Off"]
async fn live_tui_initial_prompt_off_skips_rag_parts() {
    let workspace = std::env::var_os("PLOKE_EVAL_EXISTING_TUI_WORKSPACE")
        .map(PathBuf::from)
        .unwrap_or_else(ploke_workspace_root_for_test);
    let mut runtime = crate::runner::setup_workspace_tui_prompt_runtime(&workspace)
        .await
        .unwrap_or_else(|err| {
            panic!(
                "runtime setup failed for initial prompt Off canary '{}': {err}",
                workspace.display()
            );
        });
    {
        let mut cfg = runtime.state.config.write().await;
        cfg.context_management.mode = ploke_tui::user_config::CtxMode::Off;
    }
    runtime.app.pump_pending_events().await;

    let prompt = r#"Inspect the indexed workspace context for setup_workspace_tui_runtime.
Do not call tools. Do not propose edits. This canary only checks context mode Off prompt assembly."#;
    let parent_id = submit_prompt(&runtime.app, prompt.to_string())
        .await
        .expect("submit canary prompt");

    let diagnostic = tokio::time::timeout(Duration::from_secs(30), async {
        loop {
            runtime.app.pump_pending_events().await;
            match next_event(&mut runtime).await.expect("next app event") {
                ploke_tui::AppEvent::Llm(ploke_tui::llm::LlmEvent::ChatCompletion(
                    ploke_tui::llm::ChatEvt::PromptConstructed {
                        parent_id: observed,
                        formatted_prompt,
                        context_plan,
                    },
                )) if observed == parent_id => {
                    break PromptDiagnostic::capture(
                        &runtime.state,
                        observed,
                        &formatted_prompt,
                        &context_plan,
                    )
                    .await;
                }
                _ => {}
            }
        }
    })
    .await
    .unwrap_or_else(|_| {
        panic!(
            "timed out waiting for initial PromptConstructed event in '{}'",
            workspace.display()
        )
    });

    println!(
        "initial prompt Off workspace={} bm25={:?} included_rag_parts={} fallback={:?}",
        workspace.display(),
        diagnostic.bm25,
        diagnostic.included_rag_parts,
        diagnostic.fallback_notice
    );
    assert!(
        diagnostic.workspace.loaded,
        "expected loaded workspace for '{}'",
        workspace.display()
    );
    assert!(
        matches!(diagnostic.bm25.as_ref(), Some(Bm25Diagnostic { status, docs: Some(docs), .. }) if status == "ready" && *docs > 0),
        "expected ready BM25 with documents for '{}', got {:?}",
        workspace.display(),
        diagnostic.bm25
    );
    assert_eq!(diagnostic.context_mode, "Off");
    assert_eq!(diagnostic.included_rag_parts, 0);
    assert!(diagnostic.rag_part_previews.is_empty());
    assert!(diagnostic.rag_stats.is_none());
    assert!(
        matches!(
            diagnostic.fallback_notice.as_deref(),
            Some(notice)
                if notice.starts_with("Context mode is Off:")
                    && notice.contains("request_code_context is still available")
                    && notice.contains("workspace loaded ")
        ),
        "expected Off fallback notice, got {:?}",
        diagnostic.fallback_notice
    );
    assert!(
        diagnostic.context_unavailable_reason().is_none(),
        "Off mode should not terminate a broad harness attempt"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "operator canary for request_code_context against the ploke workspace"]
async fn live_tui_request_code_context_ploke_workspace_returns_results() {
    let workspace = std::env::var_os("PLOKE_EVAL_EXISTING_TUI_WORKSPACE")
        .map(PathBuf::from)
        .unwrap_or_else(ploke_workspace_root_for_test);
    let terms = std::env::var("PLOKE_EVAL_TUI_SEARCH_TERMS")
        .ok()
        .map(|raw| {
            raw.split(',')
                .map(str::trim)
                .filter(|term| !term.is_empty())
                .map(str::to_string)
                .collect::<Vec<_>>()
        })
        .filter(|terms| !terms.is_empty())
        .unwrap_or_else(|| {
            vec![
                "setup_workspace_tui_runtime".to_string(),
                "RequestCodeContextGat".to_string(),
                "run_broad_headless_tui_attempt".to_string(),
            ]
        });

    let mut runtime = crate::runner::setup_workspace_tui_runtime(&workspace)
        .await
        .unwrap_or_else(|err| {
            panic!(
                "runtime setup failed for request_code_context canary '{}': {err}",
                workspace.display()
            );
        });
    runtime.app.pump_pending_events().await;

    let ctx = ploke_tui::tools::Ctx {
        state: Arc::clone(&runtime.state),
        event_bus: Arc::new(ploke_tui::EventBus::new(ploke_tui::EventBusCaps::default())),
        request_id: Uuid::new_v4(),
        parent_id: Uuid::new_v4(),
        call_id: ploke_core::ArcStr::from("ploke-workspace-context-canary"),
    };
    let rag = runtime
        .state
        .rag
        .as_ref()
        .expect("RAG service must be configured")
        .clone();

    let mut misses = Vec::new();
    for term in terms {
        let raw_hits = rag
            .search_bm25_strict(&term, 6, ploke_core::RetrievalScope::LoadedWorkspace)
            .await
            .unwrap_or_else(|err| {
                panic!(
                    "raw BM25 search failed for term '{term}' in '{}': {err}",
                    workspace.display()
                )
            });
        let raw_ids = raw_hits.iter().map(|(id, _score)| *id).collect::<Vec<_>>();
        let raw_nodes = runtime
            .state
            .db
            .get_snippet_nodes_ordered(raw_ids)
            .unwrap_or_else(|err| panic!("failed to resolve raw BM25 nodes: {err}"));
        let snippet_checks = runtime
            .state
            .io_handle
            .get_snippets_batch(raw_nodes.clone())
            .await
            .unwrap_or_else(|err| panic!("snippet batch request failed: {err}"));
        let snippet_ok = snippet_checks.iter().filter(|res| res.is_ok()).count();
        let result = <ploke_tui::tools::request_code_context::RequestCodeContextGat as ploke_tui::tools::Tool>::execute(
            ploke_tui::tools::request_code_context::RequestCodeContextParams {
                token_budget_per_result: Some(300),
                token_budget_total: Some(1_200),
                search_term: Some(Cow::Owned(term.clone())),
            },
            ctx.clone(),
        )
        .await
        .unwrap_or_else(|err| {
            panic!(
                "request_code_context failed for term '{term}' in '{}': {err}",
                workspace.display()
            );
        });

        let payload: ploke_core::rag_types::RequestCodeContextResult =
            serde_json::from_str(&result.content).unwrap_or_else(|err| {
                panic!("request_code_context returned invalid JSON for term '{term}': {err}")
            });
        println!(
            "request_code_context term={term:?} raw_bm25_hits={} snippet_ok={} first_node_path={:?} top_k={} returned={} first_path={:?}",
            raw_hits.len(),
            snippet_ok,
            raw_nodes
                .first()
                .map(|node| node.file_path.to_string_lossy().into_owned()),
            payload.top_k,
            payload.context.len(),
            payload
                .context
                .first()
                .map(|context| context.file_path.0.as_str())
        );
        if payload.context.is_empty() {
            misses.push(format!("{term}: {:?}", payload.note));
        }
    }
    assert!(
        misses.is_empty(),
        "expected request_code_context to return snippets for all terms in '{}'; misses: {}",
        workspace.display(),
        misses.join("; ")
    );
}

#[test]
fn broad_attempt_error_maps_setup_to_prepare_error() {
    let source = BroadAttemptError::Setup {
        phase: "bm25_ready",
        detail: "RAG service is unavailable".to_string(),
    };
    let mapped = PrepareError::from(source);
    let PrepareError::DatabaseSetup { phase, detail } = mapped else {
        panic!("expected database setup mapping, got {mapped:?}");
    };
    assert_eq!(phase, "bm25_ready");
    assert_eq!(detail, "RAG service is unavailable");
}

#[test]
fn broad_attempt_error_adapter_preserves_setup_failure() {
    let source = BroadAttemptError::Adapter(Error::HeadlessSetup {
        phase: "database_open",
        detail: "locked".to_string(),
    });
    assert_eq!(source.setup_phase(), Some(("database_open", "locked")));
}

#[test]
fn broad_attempt_error_adapter_non_setup_maps_to_invalid_batch() {
    let source = BroadAttemptError::Adapter(Error::EmptyBudget);
    let mapped = PrepareError::from(source);
    assert!(matches!(mapped, PrepareError::InvalidBatchSelection { .. }));
}

#[cfg(feature = "live_api_tests")]
#[tokio::test]
#[ignore = "requires Google ADC and OPENROUTER_API_KEY"]
async fn live_attempt_run_google_direct_with_embeddings() {
    if !crate::test_support::live_google_env_or_skip(
        "live_attempt_run_google_direct_with_embeddings",
    )
    .await
    {
        return;
    }

    let prompt = r#"Use the available edit tools to stage exactly one code edit in src/lib.rs.

Change broad_surface_canary so it returns "after" instead of "before".
Do not edit Cargo.toml. Do not create report, result, control, or bookkeeping files.
"#;
    let fixture =
        prepare_live_canary("live-attempt-google-direct", prompt).expect("prepare canary");
    let budget = Budget::new(2, 600).expect("valid live budget");
    let outcome = Attempt {
        workspace: fixture.workspace.clone(),
        prompt: fixture.prompt.clone(),
        budget,
        surface: SurfacePolicy::workspace_except_core(),
        evidence: Vec::new(),
        validation: Vec::new(),
        model: Some(ModelSelection::direct_google(
            "google/gemini-2.5-flash".parse().expect("model id"),
        )),
        capture: Capture::Responses,
    }
    .run()
    .await
    .expect("live attempt should return typed outcome");
    assert!(
        outcome.terminal.live_summary().contains("applied")
            || outcome.terminal.live_summary().contains("exhausted")
            || outcome.terminal.live_summary().contains("no_edit"),
        "unexpected terminal: {}",
        outcome.terminal.live_summary()
    );
}
