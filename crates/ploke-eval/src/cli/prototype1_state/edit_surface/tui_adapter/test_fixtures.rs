//! Live-canary and historical replay test fixtures.

#![cfg(test)]

use ploke_llm::{manager::RecordedResponse, router_only::RouterVariants};
use ploke_records::{agent_turn::ModelRouteRecord, llm_response::RawFullResponseRecord};
use serde::Serialize;
use std::{
    borrow::Cow,
    fs,
    path::{Path, PathBuf},
    process::Command,
    sync::Arc,
};
use uuid::Uuid;

use super::super::super::{
    backend::EditSurfaceAdmission,
    harness_request::{
        BroadEditPolicy, HarnessChildBudget, PublishedBroadHarnessRequest, RequestAdmissionBinding,
        contract,
    },
    harness_result::SubmittedBroadHarnessResult,
};
use super::super::{ModelSelection, run_headless_with_model};
use super::{Budget, Event, HeadlessRun, HeadlessTerminal, evidence, validation_command_display};

fn recorded_replay_test_mutex() -> &'static tokio::sync::Mutex<()> {
    crate::test_support::llm_lock()
}

struct ClearRecordedTapeOnDrop;

impl Drop for ClearRecordedTapeOnDrop {
    fn drop(&mut self) {
        ploke_tui::llm::clear_recorded_response_tape();
    }
}

fn collect_request_snapshots(
    request_rx: &std::sync::mpsc::Receiver<Vec<ploke_tui::llm::RequestMessage>>,
    snapshots: &mut Vec<Vec<ploke_tui::llm::RequestMessage>>,
) {
    loop {
        match request_rx.try_recv() {
            Ok(snapshot) => snapshots.push(snapshot),
            Err(std::sync::mpsc::TryRecvError::Empty) => break,
            Err(std::sync::mpsc::TryRecvError::Disconnected) => break,
        }
    }
}

fn historical_r10_turn_live_dir() -> PathBuf {
    PathBuf::from(
        "/home/brasides/.ploke-eval/campaigns/p1-memfix-0604a/prototype1/messages/edit-harness-result/node-a212c1db6c2774de-r10.turn-live",
    )
}

fn historical_r10_completed_tail_tape(
    run_dir: &Path,
    turn_live_dir: &Path,
) -> ploke_llm::manager::RecordedResponseTape {
    const TAIL_CALLS: [(&str, &str); 5] = [
        (
            "function-call-f57da9c4-31f4-4046-9548-532a4b57f143",
            "non_semantic_patch",
        ),
        (
            "function-call-e62c3e56-4434-43be-92ef-b9d60a9bf9d8",
            "cargo",
        ),
        (
            "function-call-34662591-b7b8-4c3e-b589-f70d5b7fb2c1",
            "non_semantic_patch",
        ),
        (
            "function-call-f300448f-a4c2-487a-a610-6db96e13661c",
            "cargo",
        ),
        (
            "function-call-0c5f5c0d-cc91-4b56-b7cd-7b394493df6e",
            "cargo",
        ),
    ];

    let historical_records = read_historical_turn_live_records(turn_live_dir);
    let mut selected = TAIL_CALLS
        .iter()
        .map(|(call_id, tool)| historical_record_for_call(&historical_records, call_id, tool))
        .collect::<Vec<_>>();
    selected.push(historical_stop_record(&historical_records));

    let assistant_id = Uuid::new_v4();
    let rebased = selected
        .into_iter()
        .enumerate()
        .map(
            |(response_index, record)| ploke_records::llm_response::RawFullResponseRecord {
                assistant_message_id: assistant_id,
                recorded_response: ploke_llm::manager::RecordedResponse::new(
                    response_index,
                    record.response().clone(),
                ),
            },
        )
        .collect::<Vec<_>>();
    load_recorded_tape(run_dir, assistant_id, rebased)
}

fn read_historical_turn_live_records(
    turn_live_dir: &Path,
) -> Vec<ploke_records::llm_response::RawFullResponseRecord> {
    let path = turn_live_dir.join(ploke_records::llm_response::FULL_RESPONSE_TRACE_FILE);
    let text = fs::read_to_string(&path).unwrap_or_else(|source| {
        panic!(
            "read historical turn-live tape {}: {source}",
            path.display()
        )
    });
    text.lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| {
            serde_json::from_str::<ploke_records::llm_response::RawFullResponseRecord>(line)
                .unwrap_or_else(|source| {
                    panic!(
                        "parse historical turn-live tape {}: {source}",
                        path.display()
                    )
                })
        })
        .collect()
}

fn historical_record_for_call(
    records: &[ploke_records::llm_response::RawFullResponseRecord],
    call_id: &str,
    tool: &str,
) -> ploke_records::llm_response::RawFullResponseRecord {
    let matches = records
        .iter()
        .filter(|record| response_record_contains_tool_call(record, call_id, tool))
        .cloned()
        .collect::<Vec<_>>();
    assert_eq!(
        matches.len(),
        1,
        "expected exactly one historical provider response for {tool} call {call_id}"
    );
    matches.into_iter().next().expect("one match")
}

fn historical_stop_record(
    records: &[ploke_records::llm_response::RawFullResponseRecord],
) -> ploke_records::llm_response::RawFullResponseRecord {
    let matches = records
        .iter()
        .filter(|record| response_record_is_stop(record))
        .cloned()
        .collect::<Vec<_>>();
    assert_eq!(
        matches.len(),
        1,
        "expected exactly one historical stop response in r10 turn-live tape"
    );
    matches.into_iter().next().expect("one stop response")
}

fn response_record_contains_tool_call(
    record: &ploke_records::llm_response::RawFullResponseRecord,
    call_id: &str,
    tool: &str,
) -> bool {
    let body =
        serde_json::to_string(record.response()).expect("serialize historical provider response");
    let step =
        ploke_llm::manager::parse_chat_outcome(&body).expect("parse historical provider response");
    let ploke_llm::manager::ChatStepOutcome::ToolCalls { calls, .. } = step.outcome else {
        return false;
    };
    calls
        .iter()
        .any(|call| call.call_id.as_ref() == call_id && call.function.name.as_str() == tool)
}

fn response_record_is_stop(record: &ploke_records::llm_response::RawFullResponseRecord) -> bool {
    serde_json::to_value(record.response())
        .ok()
        .and_then(|value| {
            value
                .pointer("/choices/0/finish_reason")
                .and_then(serde_json::Value::as_str)
                .map(str::to_string)
        })
        .as_deref()
        == Some("stop")
}

fn write_historical_r10_admission_request(fixture: &LiveCanaryFixture) -> PathBuf {
    let prototype_root = fixture.artifact_root.join("prototype1");
    let request_dir = prototype_root.join("messages/edit-harness-request");
    let result_dir = prototype_root.join("messages/edit-harness-result");
    fs::create_dir_all(&request_dir).expect("create historical r10 request dir");
    fs::create_dir_all(&result_dir).expect("create historical r10 result dir");

    let published = PublishedBroadHarnessRequest::prototype1_workspace(
        "node-a212c1db6c2774de-r10".to_string(),
        fixture.workspace.clone(),
        HarnessChildBudget {
            min_children: 1,
            max_children: 1,
        },
        &prototype_root,
        request_dir.join("node-a212c1db6c2774de-r10.json"),
        request_dir.join("node-a212c1db6c2774de-r10.md"),
        result_dir.join("node-a212c1db6c2774de-r10.json"),
        historical_r10_admission_binding(&fixture.workspace),
    );
    fs::write(
        published.request_path(),
        serde_json::to_vec_pretty(&published).expect("serialize historical r10 request"),
    )
    .expect("write historical r10 request");
    fs::write(published.prompt_path(), published.request().render_prompt())
        .expect("write historical r10 prompt");
    published.request_path().to_path_buf()
}

fn historical_r10_admission_binding(repo_root: &Path) -> RequestAdmissionBinding {
    let artifact_id = ArtifactId::new(format!("artifact:{}", repo_root.display()));
    RequestAdmissionBinding::from_admission(&EditSurfaceAdmission::new(
        Coordinate {
            runtime_id: RuntimeId::new(),
            target: OperationTarget::Artifact { artifact_id },
        },
        surface::SurfacePolicyId::new("workspace except ploke-eval"),
    ))
    .expect("construct historical r10 admission binding")
}

fn read_published_request(path: &Path) -> PublishedBroadHarnessRequest {
    let bytes = fs::read(path)
        .unwrap_or_else(|source| panic!("read published request {}: {source}", path.display()));
    serde_json::from_slice(&bytes)
        .unwrap_or_else(|source| panic!("parse published request {}: {source}", path.display()))
}

fn read_submitted_result(path: &Path) -> SubmittedBroadHarnessResult {
    let bytes = fs::read(path)
        .unwrap_or_else(|source| panic!("read submitted result {}: {source}", path.display()));
    serde_json::from_slice(&bytes)
        .unwrap_or_else(|source| panic!("parse submitted result {}: {source}", path.display()))
}

fn read_headless_summary(path: &Path) -> evidence::Summary {
    let bytes = fs::read(path)
        .unwrap_or_else(|source| panic!("read headless summary {}: {source}", path.display()));
    serde_json::from_slice(&bytes)
        .unwrap_or_else(|source| panic!("parse headless summary {}: {source}", path.display()))
}

fn install_historical_r10_selection_score_workspace(fixture: &LiveCanaryFixture) {
    fs::write(
        fixture.workspace.join("Cargo.toml"),
        r#"[workspace]
members = [
"crates/ploke-selection-score",
"crates/ploke-eval",
]
resolver = "2"
"#,
    )
    .expect("write historical r10 workspace Cargo.toml");
    fs::write(fixture.workspace.join(".gitignore"), "/target\n")
        .expect("write historical r10 workspace .gitignore");

    let selection_root = fixture.workspace.join("crates/ploke-selection-score");
    fs::create_dir_all(selection_root.join("src/common"))
        .expect("create historical r10 common dir");
    fs::create_dir_all(selection_root.join("src/ploke")).expect("create historical r10 ploke dir");
    fs::write(
        selection_root.join("Cargo.toml"),
        r#"[package]
name = "ploke-selection-score"
version = "0.1.0"
edition = "2024"

[lib]
path = "src/lib.rs"
"#,
    )
    .expect("write ploke-selection-score Cargo.toml");
    fs::write(
        selection_root.join("src/lib.rs"),
        r#"pub mod common {
pub mod ranking;
}

pub mod ploke {
pub mod frontier;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScoreError {
Empty,
LengthMismatch,
NonFinite,
}

pub use common::ranking::rank_quality;
pub use ploke::frontier::{frontier_weights, FrontierConfig};
"#,
    )
    .expect("write ploke-selection-score lib.rs");
    fs::write(
        selection_root.join("src/common/ranking.rs"),
        r#"use crate::ScoreError;

pub fn rank_quality(score: &[f64]) -> Result<Vec<f64>, ScoreError> {
if score.is_empty() {
    return Err(ScoreError::Empty);
}
if score.iter().any(|value| !value.is_finite()) {
    return Err(ScoreError::NonFinite);
}
if score.len() == 1 {
    return Ok(vec![0.5]);
}

let mut pair: Vec<(usize, f64)> = score.iter().copied().enumerate().collect();
pair.sort_by(|left, right| left.1.total_cmp(&right.1));

let denom = (score.len() - 1) as f64;
let mut out = vec![0.0; score.len()];
for (rank, (idx, _)) in pair.into_iter().enumerate() {
    out[idx] = rank as f64 / denom;
}
Ok(out)
}
"#,
    )
    .expect("write historical r10 ranking.rs");
    fs::write(
        selection_root.join("src/ploke/frontier.rs"),
        r#"use crate::ScoreError;

#[derive(Debug, Clone, Copy)]
pub struct FrontierConfig {
pub top_m: usize,
pub lambda: f64,
}

fn sigmoid(value: f64) -> f64 {
1.0 / (1.0 + (-value).exp())
}

pub fn frontier_weights(
qual: &[f64],
child: &[usize],
cfg: FrontierConfig,
) -> Result<Vec<f64>, ScoreError> {
if qual.is_empty() {
    return Err(ScoreError::Empty);
}
if qual.len() != child.len() {
    return Err(ScoreError::LengthMismatch);
}
if !cfg.lambda.is_finite() || qual.iter().any(|value| !value.is_finite()) {
    return Err(ScoreError::NonFinite);
}

let count = cfg.top_m.clamp(1, qual.len());
let mut top = qual.to_vec();
top.sort_by(|left, right| right.total_cmp(left));
let mid = top.iter().take(count).sum::<f64>() / count as f64;

Ok(qual
    .iter()
    .zip(child.iter())
    .map(|(quality, kids)| sigmoid(cfg.lambda * (quality - mid)) / (1.0 + *kids as f64))
    .collect())
}
"#,
    )
    .expect("write historical r10 frontier.rs");

    let eval_root = fixture.workspace.join("crates/ploke-eval");
    fs::create_dir_all(eval_root.join("src")).expect("create historical r10 eval src");
    fs::write(
        eval_root.join("Cargo.toml"),
        r#"[package]
name = "ploke-eval"
version = "0.1.0"
edition = "2024"

[dependencies]
ploke-selection-score = { path = "../ploke-selection-score" }

[lib]
path = "src/lib.rs"
"#,
    )
    .expect("write ploke-eval Cargo.toml");
    fs::write(
        eval_root.join("src/lib.rs"),
        r#"pub fn edit_surface_dependency_canary() -> usize {
let cfg = ploke_selection_score::FrontierConfig {
    top_m: 1,
    lambda: 1.0,
};
ploke_selection_score::frontier_weights(&[1.0], &[0], cfg)
    .map(|weights| weights.len())
    .unwrap_or_default()
}

#[cfg(test)]
mod tests {
#[test]
fn edit_surface() {
    assert_eq!(super::edit_surface_dependency_canary(), 1);
}
}
"#,
    )
    .expect("write ploke-eval lib.rs");

    command_output(&fixture.workspace, "cargo", &["generate-lockfile"]);
    if !fixture.workspace.join("Cargo.lock").exists() {
        fs::write(
            fixture.workspace.join("Cargo.lock"),
            r#"# This file is automatically @generated by Cargo.
# It is not intended for manual editing.
version = 4

[[package]]
name = "ploke-eval"
version = "0.1.0"
dependencies = [
 "ploke-selection-score",
]

[[package]]
name = "ploke-selection-score"
version = "0.1.0"
"#,
        )
        .expect("write historical r10 Cargo.lock");
    }
    assert!(
        fixture.workspace.join("Cargo.lock").exists(),
        "historical r10 fixture must baseline Cargo.lock before validation commands run"
    );
    command_output(
        &fixture.workspace,
        "git",
        &[
            "add",
            "Cargo.toml",
            "Cargo.lock",
            ".gitignore",
            "crates/ploke-selection-score",
            "crates/ploke-eval",
        ],
    );
    command_output(
        &fixture.workspace,
        "git",
        &[
            "-c",
            "user.email=ploke-eval-live-canary@example.invalid",
            "-c",
            "user.name=ploke eval live canary",
            "commit",
            "-m",
            "historical r10 frontier midpoint",
        ],
    );
}

fn recorded_protected_ns_patch_tape(
    run_dir: &Path,
    call_id: &str,
    protected_rel: &Path,
) -> ploke_llm::manager::RecordedResponseTape {
    let assistant_id = Uuid::new_v4();
    let request = ns_patch_request(
        call_id,
        protected_rel.display().to_string(),
        protected_ns_patch_diff(protected_rel),
        "exercise protected-path staged proposal replay",
        Some(0.9),
    );
    load_recorded_tape(
        run_dir,
        assistant_id,
        vec![
            tool_response_record(assistant_id, 0, "recorded-protected-ns-patch", &request),
            stop_response_record(assistant_id, 1, "recorded-final"),
        ],
    )
}

fn recorded_historical_ns_patch_tape(
    run_dir: &Path,
    requests: Vec<ploke_records::agent_turn::ToolRequestRecord>,
) -> ploke_llm::manager::RecordedResponseTape {
    let assistant_id = Uuid::new_v4();
    let mut records = requests
        .iter()
        .enumerate()
        .map(|(index, request)| {
            tool_response_record(
                assistant_id,
                index,
                format!("historical-protected-cargo-{index}"),
                request,
            )
        })
        .collect::<Vec<_>>();
    records.push(stop_response_record(
        assistant_id,
        requests.len(),
        "historical-protected-cargo-final",
    ));
    load_recorded_tape(run_dir, assistant_id, records)
}

fn historical_repeated_cargo_ns_patch_requests(
    workspace: &Path,
    count: usize,
) -> Vec<ploke_records::agent_turn::ToolRequestRecord> {
    let trace_path = Path::new(
        "/home/brasides/.ploke-eval/campaigns/p1-broad-batch-admission-20260518-2/prototype1/messages/edit-harness-result/node-01c9e8fdc70e3ee8.headless-tui.json",
    );
    let trace = fs::read_to_string(trace_path).unwrap_or_else(|source| {
        panic!(
            "read historical headless trace {}: {source}",
            trace_path.display()
        )
    });
    let summary: evidence::Summary = serde_json::from_str(&trace).unwrap_or_else(|source| {
        panic!(
            "parse historical headless trace {} as evidence::Summary: {source}",
            trace_path.display()
        )
    });

    let mut requests = Vec::new();
    for event in summary.events {
        let evidence::Event::ToolRequest {
            request_id,
            parent_id,
            call_id,
            tool,
            arguments,
        } = event
        else {
            continue;
        };
        if tool != "non_semantic_patch" {
            continue;
        }
        assert_eq!(
            arguments.chars,
            arguments.preview.chars().count(),
            "historical ns_patch arguments must be untruncated for typed replay"
        );
        let mut params = historical_ns_patch_params(&tool, &arguments.preview, &call_id);
        if !is_workspace_cargo_ns_patch(&params) {
            continue;
        }
        rebase_ns_patch_workspace(&mut params, workspace);
        let encoded =
            serde_json::to_string(&params).expect("serialize rebased historical ns_patch");
        let record = ploke_records::agent_turn::ToolRequestRecord {
            request_id,
            parent_id,
            call_id,
            tool,
            arguments: ploke_records::tool_contracts::ToolArgumentsJson::from(encoded),
        };
        assert_decodes_as_ns_patch(&record);
        requests.push(record);
        if requests.len() == count {
            break;
        }
    }
    assert_eq!(
        requests.len(),
        count,
        "expected {count} historical Cargo.toml ns_patch requests in replay trace"
    );
    requests
}

#[test]
#[ignore = "historical diagnostic for successful empty read_file completions"]
fn read_diag() -> Result<(), Box<dyn std::error::Error>> {
    // Follow-up: once we have a second run demonstrating the fix, extend this
    // diagnostic with a second part that compares the historical bad trace
    // against the fixed run's read_file completions.
    let run_path = PathBuf::from(
        "/home/brasides/.ploke-eval/campaigns/p1-gemini35-flash-direct-15g2x3-par2-20260601-173956/prototype1/",
    );
    let trace_path =
        run_path.join("messages/edit-harness-result/node-552c19a55f53dbe6-r2.headless-tui.json");
    let trace = fs::read_to_string(&trace_path).unwrap_or_else(|source| {
        panic!(
            "read historical headless trace {}: {source}",
            trace_path.display()
        )
    });
    let summary: evidence::Summary = serde_json::from_str(&trace).unwrap_or_else(|source| {
        panic!(
            "parse historical headless trace {} as evidence::Summary: {source}",
            trace_path.display()
        )
    });

    let diagnostics = historical_empty_read_file_diagnostics(&summary);
    println!(
        "READ_FILE_DIAG trace={} empty_successes_with_real_lines={}",
        trace_path.display(),
        diagnostics.len()
    );
    for diagnostic in &diagnostics {
        let diagnostic_file = diagnostic.observed_file.strip_prefix(&run_path)?;
        println!(
            "\nREAD_FILE_DIAG call_id={}\n\
requested={}\n\
observed={}\n\
range={:?}-{:?}\n\
byte_len={:?}\n\
truncated={}\n\
line_count={}\n\
first_line={}\n\
target_line={}\n",
            diagnostic.call_id,
            diagnostic.requested_file,
            diagnostic_file.display(),
            diagnostic.start_line,
            diagnostic.end_line,
            diagnostic.byte_len,
            diagnostic.truncated,
            diagnostic.direct_line_count,
            diagnostic.first_direct_line,
            diagnostic
                .target_line
                .as_deref()
                .unwrap_or("<no target marker in range>"),
        );
    }

    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic.call_id
            == "function-call-cd7bb6c0-97b8-4928-9324-57aa0c08b4e5"
            && diagnostic.direct_line_count > 0),
        "expected historical r2 trace to reproduce the tests.rs empty successful read"
    );
    Ok(())
}

struct EmptyReadFileDiagnostic {
    call_id: String,
    requested_file: String,
    observed_file: PathBuf,
    start_line: Option<u32>,
    end_line: Option<u32>,
    byte_len: Option<u64>,
    truncated: bool,
    direct_line_count: usize,
    first_direct_line: String,
    target_line: Option<String>,
}

fn historical_empty_read_file_diagnostics(
    summary: &evidence::Summary,
) -> Vec<EmptyReadFileDiagnostic> {
    let mut requests = HashMap::<String, ploke_records::tool_contracts::NsReadParamsOwned>::new();
    let mut diagnostics = Vec::new();

    for event in &summary.events {
        match event {
            evidence::Event::ToolRequest {
                call_id,
                tool,
                arguments,
                ..
            } if tool == "read_file" && arguments.chars == arguments.preview.chars().count() => {
                let captured = ploke_records::tool_contracts::ToolArgumentsJson::from(
                    arguments.preview.clone(),
                );
                if let ploke_records::tool_contracts::PersistedToolCallArguments::Decoded(
                    ploke_records::tool_contracts::ToolCallArguments::NsRead(params),
                ) = captured.decode_for_tool(tool)
                {
                    requests.insert(call_id.clone(), params);
                }
            }
            evidence::Event::ToolCompleted { call_id, content } => {
                let Some(request) = requests.get(call_id) else {
                    continue;
                };
                let Ok(result) = serde_json::from_str::<ploke_records::tool_contracts::NsReadResult>(
                    &content.preview,
                ) else {
                    continue;
                };
                if !result.ok
                    || !result.exists
                    || result.content.as_deref() != Some("")
                    || !result.truncated
                {
                    continue;
                }
                let observed_file = PathBuf::from(&result.file_path);
                let Some((direct_line_count, first_direct_line, target_line)) =
                    direct_line_range_preview(&observed_file, request.start_line, request.end_line)
                else {
                    continue;
                };
                diagnostics.push(EmptyReadFileDiagnostic {
                    call_id: call_id.clone(),
                    requested_file: request.file.clone(),
                    observed_file,
                    start_line: request.start_line,
                    end_line: request.end_line,
                    byte_len: result.byte_len,
                    truncated: result.truncated,
                    direct_line_count,
                    first_direct_line,
                    target_line,
                });
            }
            _ => {}
        }
    }

    diagnostics
}

fn direct_line_range_preview(
    path: &Path,
    start_line: Option<u32>,
    end_line: Option<u32>,
) -> Option<(usize, String, Option<String>)> {
    let content = fs::read_to_string(path).ok()?;
    let start = start_line.unwrap_or(1).max(1);
    let end = end_line.unwrap_or(start).max(start);
    let mut lines = content
        .lines()
        .enumerate()
        .filter_map(|(index, line)| {
            let line_no = u32::try_from(index + 1).ok()?;
            (line_no >= start && line_no <= end).then_some(line)
        })
        .collect::<Vec<_>>();
    if lines.is_empty() {
        return None;
    }
    let target = lines
        .iter()
        .find(|line| line.contains("real_tui_resolver_touch_is_checked_before_adapter_apply"))
        .map(|line| line.trim().to_string());
    let first = lines.remove(0).trim().to_string();
    Some((1 + lines.len(), first, target))
}

fn historical_ns_patch_params(
    tool: &str,
    arguments: &str,
    call_id: &str,
) -> ploke_records::tool_contracts::NsPatchParamsOwned {
    let captured = ploke_records::tool_contracts::ToolArgumentsJson::from(arguments);
    let decoded = captured.decode_for_tool(tool);
    let ploke_records::tool_contracts::PersistedToolCallArguments::Decoded(
        ploke_records::tool_contracts::ToolCallArguments::NsPatch(params),
    ) = decoded
    else {
        panic!("historical tool request {call_id} did not decode as non_semantic_patch");
    };
    params
}

fn is_workspace_cargo_ns_patch(params: &ploke_records::tool_contracts::NsPatchParamsOwned) -> bool {
    params.patches.len() == 1
        && params.patches[0].file.ends_with("/Cargo.toml")
        && params.patches[0].diff.contains("--- a/Cargo.toml")
}

fn rebase_ns_patch_workspace(
    params: &mut ploke_records::tool_contracts::NsPatchParamsOwned,
    workspace: &Path,
) {
    for patch in &mut params.patches {
        if patch.file.ends_with("/Cargo.toml") {
            patch.file = workspace.join("Cargo.toml").display().to_string();
        }
    }
}

fn recorded_allowed_ns_patch_tape(
    run_dir: &Path,
    call_id: &str,
) -> ploke_llm::manager::RecordedResponseTape {
    let assistant_id = Uuid::new_v4();
    let request = ns_patch_request(
        call_id,
        "src/lib.rs".to_string(),
        allowed_canary_ns_patch_diff(),
        "Change broad_surface_canary from before to after",
        Some(0.95),
    );
    load_recorded_tape(
        run_dir,
        assistant_id,
        vec![
            tool_response_record(assistant_id, 0, "recorded-allowed-ns-patch", &request),
            stop_response_record(assistant_id, 1, "recorded-allowed-final"),
        ],
    )
}

fn recorded_truncating_lib_patch_tape(
    run_dir: &Path,
    call_id: &str,
) -> ploke_llm::manager::RecordedResponseTape {
    // Build the tape through the same raw full-response format used by
    // replayed live runs. The adapter should not know this came from a test
    // helper once the tape is installed.
    let assistant_id = Uuid::new_v4();
    let request = ns_patch_request(
        call_id,
        "src/lib.rs".to_string(),
        truncating_stale_snippet_ns_patch_diff(),
        "Remove stale_index_canary and shorten src/lib.rs after indexing",
        Some(0.95),
    );
    load_recorded_tape(
        run_dir,
        assistant_id,
        vec![
            tool_response_record(
                assistant_id,
                0,
                "recorded-truncating-stale-snippet",
                &request,
            ),
            stop_response_record(assistant_id, 1, "recorded-truncating-final"),
        ],
    )
}

fn recorded_ploke_tree_assemble_patch_tape(
    run_dir: &Path,
    call_id: &str,
) -> ploke_llm::manager::RecordedResponseTape {
    let assistant_id = Uuid::new_v4();
    let request = ns_patch_request(
        call_id,
        "crates/ploke-tree/src/lib.rs".to_string(),
        ploke_tree_assemble_run_forest_ns_patch_diff(),
        "Add a post-apply refresh canary inside assemble_run_forest",
        Some(0.95),
    );
    load_recorded_tape(
        run_dir,
        assistant_id,
        vec![
            tool_response_record(
                assistant_id,
                0,
                "recorded-ploke-tree-assemble-run-forest",
                &request,
            ),
            stop_response_record(assistant_id, 1, "recorded-ploke-tree-final"),
        ],
    )
}

fn recorded_same_file_repair_tape(
    run_dir: &Path,
    first_call_id: &str,
    stale_call_id: &str,
) -> ploke_llm::manager::RecordedResponseTape {
    let assistant_id = Uuid::new_v4();
    let first = ns_patch_request(
        first_call_id,
        "src/lib.rs".to_string(),
        allowed_canary_ns_patch_diff(),
        "First same-file edit changes broad_surface_canary from before to after",
        Some(0.95),
    );
    let stale = ns_patch_request(
        stale_call_id,
        "src/lib.rs".to_string(),
        stale_canary_repair_ns_patch_diff(),
        "Stale repair attempt generated against the pre-apply same-file content",
        Some(0.80),
    );
    load_recorded_tape(
        run_dir,
        assistant_id,
        vec![
            tool_response_record(assistant_id, 0, "recorded-same-file-first", &first),
            tool_response_record(assistant_id, 1, "recorded-same-file-stale-repair", &stale),
            stop_response_record(assistant_id, 2, "recorded-same-file-final"),
        ],
    )
}

fn repeated_protected_ns_patch_tape(
    run_dir: &Path,
    count: usize,
) -> ploke_llm::manager::RecordedResponseTape {
    let assistant_id = Uuid::new_v4();
    let protected_rel = Path::new("Cargo.toml");
    let mut records = Vec::new();
    for index in 0..count {
        let call_id = format!("call_budget_protected_{index}");
        let request = ns_patch_request(
            &call_id,
            protected_rel.display().to_string(),
            protected_ns_patch_diff(protected_rel),
            "Exercise provider-step budget with repeated protected writes",
            Some(0.50),
        );
        records.push(tool_response_record(
            assistant_id,
            index,
            format!("recorded-budget-protected-{index}"),
            &request,
        ));
    }
    records.push(stop_response_record(
        assistant_id,
        count,
        "recorded-budget-final",
    ));
    load_recorded_tape(run_dir, assistant_id, records)
}

fn ns_patch_request(
    call_id: &str,
    file: String,
    diff: String,
    reasoning: &str,
    confidence: Option<f32>,
) -> ploke_records::agent_turn::ToolRequestRecord {
    let params = ploke_records::tool_contracts::NsPatchParamsOwned {
        patches: vec![ploke_records::tool_contracts::NsPatchOwned {
            file,
            diff,
            reasoning: reasoning.to_string(),
        }],
        confidence,
    };
    let arguments = serde_json::to_string(&params).expect("serialize typed ns_patch params");
    let record = ploke_records::agent_turn::ToolRequestRecord {
        request_id: format!("{call_id}-request"),
        parent_id: "recorded-replay-parent".to_string(),
        call_id: call_id.to_string(),
        tool: "non_semantic_patch".to_string(),
        arguments: ploke_records::tool_contracts::ToolArgumentsJson::from(arguments),
    };
    assert_decodes_as_ns_patch(&record);
    record
}

fn assert_decodes_as_ns_patch(record: &ploke_records::agent_turn::ToolRequestRecord) {
    let decoded = record.arguments.decode_for_tool(&record.tool);
    let ploke_records::tool_contracts::PersistedToolCallArguments::Decoded(
        ploke_records::tool_contracts::ToolCallArguments::NsPatch(arguments),
    ) = decoded
    else {
        panic!("expected typed ns_patch arguments for {}", record.call_id);
    };
    assert_eq!(arguments.patches.len(), 1);
    assert!(
        arguments.patches[0].diff.starts_with("--- a/"),
        "historical replay should preserve a unified diff"
    );
}

fn load_recorded_tape(
    run_dir: &Path,
    assistant_id: Uuid,
    records: Vec<ploke_records::llm_response::RawFullResponseRecord>,
) -> ploke_llm::manager::RecordedResponseTape {
    fs::create_dir_all(run_dir).expect("create recorded response fixture dir");
    let path = run_dir.join(ploke_records::llm_response::FULL_RESPONSE_TRACE_FILE);
    let mut jsonl = String::new();
    for record in &records {
        assert!(record.matches_assistant_message(assistant_id));
        jsonl.push_str(&serde_json::to_string(record).expect("serialize full response record"));
        jsonl.push('\n');
    }
    fs::write(&path, jsonl).expect("write full response fixture");
    crate::replay::llm::load_recorded_response_tape(run_dir, &assistant_id.to_string())
        .expect("load recorded response tape through full-response record loader")
}

fn tool_response_record(
    assistant_id: Uuid,
    response_index: usize,
    response_id: impl Into<String>,
    request: &ploke_records::agent_turn::ToolRequestRecord,
) -> ploke_records::llm_response::RawFullResponseRecord {
    assert_decodes_as_ns_patch(request);
    let response = serde_json::from_value(serde_json::json!({
        "id": response_id.into(),
        "choices": [{
            "index": 0,
            "finish_reason": "tool_calls",
            "message": {
                "role": "assistant",
                "tool_calls": [{
                    "id": request.call_id,
                    "type": "function",
                    "function": {
                        "name": request.tool,
                        "arguments": request.arguments.as_str(),
                    }
                }]
            }
        }],
        "created": response_index,
        "model": "test/model",
        "object": "chat.completion"
    }))
    .expect("recorded ns_patch provider response should parse");
    ploke_records::llm_response::RawFullResponseRecord {
        assistant_message_id: assistant_id,
        recorded_response: ploke_llm::manager::RecordedResponse::new(response_index, response),
    }
}

fn stop_response_record(
    assistant_id: Uuid,
    response_index: usize,
    response_id: impl Into<String>,
) -> ploke_records::llm_response::RawFullResponseRecord {
    let response = serde_json::from_value(serde_json::json!({
        "id": response_id.into(),
        "choices": [{
            "index": 0,
            "finish_reason": "stop",
            "message": {
                "role": "assistant",
                "content": "done"
            }
        }],
        "created": response_index,
        "model": "test/model",
        "object": "chat.completion"
    }))
    .expect("recorded final provider response should parse");
    ploke_records::llm_response::RawFullResponseRecord {
        assistant_message_id: assistant_id,
        recorded_response: ploke_llm::manager::RecordedResponse::new(response_index, response),
    }
}

fn protected_ns_patch_diff(protected_rel: &Path) -> String {
    let path = protected_rel.display();
    format!(
        r#"--- a/{path}
+++ b/{path}
@@ -1,3 +1,3 @@
 pub fn protected_replay_canary() -> &'static str {{
-    "before"
+    "after"
 }}
"#
    )
}

fn allowed_canary_ns_patch_diff() -> String {
    [
        "--- a/src/lib.rs",
        "+++ b/src/lib.rs",
        "@@ -1,3 +1,3 @@",
        " pub fn broad_surface_canary() -> &'static str {",
        "-    \"before\"",
        "+    \"after\"",
        " }",
        "",
    ]
    .join("\n")
}

fn stale_snippet_initial_lib() -> &'static str {
    // Keep one function alive and delete the other. This distinguishes
    // ordinary reindexing of a changed file from the specific stale
    // descendant-row case we care about.
    r#"pub fn broad_surface_canary() -> &'static str {
"before"
}

pub fn stale_index_canary() -> &'static str {
"stale"
}
"#
}

fn install_stale_snippet_canary_source(fixture: &LiveCanaryFixture) {
    // Commit the custom source before runtime startup so the initial scan
    // treats it as the workspace baseline and records a current file hash.
    fs::write(&fixture.src_file, stale_snippet_initial_lib())
        .expect("write stale snippet canary source");
    fs::write(&fixture.initial_file, stale_snippet_initial_lib())
        .expect("write stale snippet initial artifact");
    command_output(&fixture.workspace, "git", &["add", "src/lib.rs"]);
    command_output(
        &fixture.workspace,
        "git",
        &[
            "-c",
            "user.email=ploke-eval-live-canary@example.invalid",
            "-c",
            "user.name=ploke eval live canary",
            "commit",
            "--allow-empty",
            "-m",
            "install stale snippet canary",
        ],
    );
}

fn truncating_stale_snippet_ns_patch_diff() -> String {
    // Removing the second function shortens the file. Before the retraction
    // fix, DB rows for that deleted function could still carry the old byte
    // range into snippet extraction.
    [
        "--- a/src/lib.rs",
        "+++ b/src/lib.rs",
        "@@ -1,7 +1,3 @@",
        " pub fn broad_surface_canary() -> &'static str {",
        "-    \"before\"",
        "+    \"after\"",
        " }",
        "-",
        "-pub fn stale_index_canary() -> &'static str {",
        "-    \"stale\"",
        "-}",
        "",
    ]
    .join("\n")
}

fn ploke_tree_assemble_run_forest_ns_patch_diff() -> String {
    [
        "--- a/crates/ploke-tree/src/lib.rs",
        "+++ b/crates/ploke-tree/src/lib.rs",
        "@@ -65,6 +65,7 @@ pub fn assemble_run_forest(input: RunForestInput) -> RunForest {",
        "     } = input;",
        "",
        "     let campaign_id = scheduler.campaign_id.as_str().to_owned();",
        "+    let _post_apply_refresh_canary = \"after\";",
        "     let mut diagnostics = Vec::new();",
        "     let merged_node_records = merge_node_records(&scheduler.nodes, node_records, &mut diagnostics);",
        "     let mut nodes = merged_node_records",
        "",
    ]
    .join("\n")
}

fn stale_canary_repair_ns_patch_diff() -> String {
    [
        "--- a/src/lib.rs",
        "+++ b/src/lib.rs",
        "@@ -1,3 +1,3 @@",
        " pub fn broad_surface_canary() -> &'static str {",
        "-    \"before\"",
        "+    \"repair\"",
        " }",
        "",
    ]
    .join("\n")
}

fn install_ploke_tree_assemble_target(fixture: &LiveCanaryFixture) -> PathBuf {
    let root_cargo_toml = r#"[package]
name = "ploke-eval-live-tui-canary"
version = "0.1.0"
edition = "2024"

[workspace]
members = ["crates/ploke-tree"]
resolver = "3"

[lib]
path = "src/lib.rs"
"#;
    let member_cargo_toml = r#"[package]
name = "ploke-tree"
version = "0.1.0"
edition = "2024"

[lib]
path = "src/lib.rs"
"#;
    let source_file = ploke_tree_assemble_fixture_path();
    let target_dir = fixture.workspace.join("crates/ploke-tree/src");
    let target_file = target_dir.join("lib.rs");
    fs::create_dir_all(&target_dir).expect("create ploke-tree target src dir");
    fs::write(fixture.workspace.join("Cargo.toml"), root_cargo_toml)
        .expect("write workspace cargo manifest");
    fs::write(
        fixture.workspace.join("crates/ploke-tree/Cargo.toml"),
        member_cargo_toml,
    )
    .expect("write ploke-tree member manifest");
    fs::write(
        &target_file,
        fs::read_to_string(&source_file).unwrap_or_else(|err| {
            panic!(
                "read ploke-tree target fixture '{}': {err}",
                source_file.display()
            )
        }),
    )
    .expect("write actual ploke-tree target source");

    // The copied lib.rs declares these modules. Empty files are enough for
    // parser/index traversal; the replay only needs the lib.rs target.
    for module in [
        "browser.rs",
        "graph.rs",
        "playback.rs",
        "store.rs",
        "tests.rs",
    ] {
        fs::write(target_dir.join(module), "").expect("write stub ploke-tree module");
    }

    command_output(
        &fixture.workspace,
        "git",
        &[
            "add",
            "Cargo.toml",
            "crates/ploke-tree/Cargo.toml",
            "crates/ploke-tree/src/lib.rs",
            "crates/ploke-tree/src/browser.rs",
            "crates/ploke-tree/src/graph.rs",
            "crates/ploke-tree/src/playback.rs",
            "crates/ploke-tree/src/store.rs",
            "crates/ploke-tree/src/tests.rs",
        ],
    );
    command_output(
        &fixture.workspace,
        "git",
        &[
            "-c",
            "user.email=ploke-eval-live-canary@example.invalid",
            "-c",
            "user.name=ploke eval live canary",
            "commit",
            "--allow-empty",
            "-m",
            "install ploke-tree assemble_run_forest target",
        ],
    );
    target_file
}

fn ploke_tree_assemble_fixture_path() -> PathBuf {
    ploke_workspace_root_for_test().join("tests/fixtures/prototype1/ploke-tree-src-lib.rs")
}

fn function_rows_by_name(
    runtime: &crate::runner::WorkspaceTuiRuntime,
    name: &str,
) -> ploke_db::QueryResult {
    // Query the primary function relation directly. This avoids hiding the
    // stale-row condition behind RAG retrieval or embedding behavior.
    runtime
        .state
        .db
        .raw_query(&format!(
            r#"?[id, tracking_hash, span] := *function {{ id, name, tracking_hash, span @ 'NOW' }}, name = "{}""#,
            name
        ))
        .expect("query function rows by name")
}

fn print_headless_replay_rows(stage: &str, rows: &ploke_db::QueryResult) {
    println!(
        "\n=== headless replay: {stage} ===\n  row_count: {}\n  headers:\n{:#?}\n  rows:\n{:#?}",
        rows.rows.len(),
        rows.headers,
        rows.rows
    );
}

fn print_headless_replay_span_content(stage: &str, source: &str, rows: &ploke_db::QueryResult) {
    let span = first_function_span(rows);
    let content = first_function_span_content(source, rows);
    println!(
        "\n=== headless replay: {stage} span content ===\n  span: {:?}\n  content:\n{}",
        span,
        content.unwrap_or("<span does not resolve in source>")
    );
}

fn first_function_span_content<'a>(
    source: &'a str,
    rows: &ploke_db::QueryResult,
) -> Option<&'a str> {
    let (start, end) = first_function_span(rows)?;
    source.get(start..end)
}

fn first_function_span(rows: &ploke_db::QueryResult) -> Option<(usize, usize)> {
    let span_index = rows.headers.iter().position(|header| header == "span")?;
    let span = rows.rows.first()?.get(span_index)?;
    let cozo::DataValue::List(parts) = span else {
        return None;
    };
    let [start, end] = parts.as_slice() else {
        return None;
    };
    Some((cozo_usize(start)?, cozo_usize(end)?))
}

fn cozo_usize(value: &cozo::DataValue) -> Option<usize> {
    let cozo::DataValue::Num(cozo::Num::Int(value)) = value else {
        return None;
    };
    usize::try_from(*value).ok()
}

fn retry_context_bool(wire: &ploke_tui::tools::ToolErrorWire, field: &str) -> Option<bool> {
    match wire.llm["retry_context"].as_object()?.get(field)? {
        ploke_tui::tools::ToolLlmErrorValue::Bool(value) => Some(*value),
        _ => None,
    }
}

fn model_request_contains_staged_success(
    messages: &[ploke_tui::llm::RequestMessage],
    call_id: &str,
) -> bool {
    messages.iter().any(|message| {
        message.role == ploke_llm::manager::Role::Tool
            && message
                .tool_call_id
                .as_ref()
                .is_some_and(|tool_call_id| tool_call_id.as_ref() == call_id)
            && message.content.contains(r#""ok":true"#)
            && message.content.contains(r#""staged":1"#)
            && message.content.contains(r#""applied":0"#)
    })
}

fn model_request_contains_ns_patch_failure(
    messages: &[ploke_tui::llm::RequestMessage],
    call_id: &str,
) -> bool {
    messages.iter().any(|message| {
        message.role == ploke_llm::manager::Role::Tool
            && message
                .tool_call_id
                .as_ref()
                .is_some_and(|tool_call_id| tool_call_id.as_ref() == call_id)
            && message.content.contains(r#""ok":false"#)
            && message.content.contains(r#""tool":"non_semantic_patch""#)
    })
}

fn model_request_contains_tool_rejection(
    messages: &[ploke_tui::llm::RequestMessage],
    call_id: &str,
) -> bool {
    messages.iter().any(|message| {
        message.role == ploke_llm::manager::Role::Tool
            && message
                .tool_call_id
                .as_ref()
                .is_some_and(|tool_call_id| tool_call_id.as_ref() == call_id)
            && message.content.contains(r#""ok":false"#)
            && message.content.contains(r#""tool":"non_semantic_patch""#)
            && message.content.contains("protected")
    })
}

fn model_request_contains_applied_success(
    messages: &[ploke_tui::llm::RequestMessage],
    call_id: &str,
) -> bool {
    messages.iter().any(|message| {
        message.role == ploke_llm::manager::Role::Tool
            && message
                .tool_call_id
                .as_ref()
                .is_some_and(|tool_call_id| tool_call_id.as_ref() == call_id)
            && message.content.contains(r#""ok":true"#)
            && message.content.contains(r#""applied":1"#)
    })
}

fn ploke_workspace_root_for_test() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("ploke-eval crate lives under <workspace>/crates/ploke-eval")
        .to_path_buf()
}

fn assert_live_canary_applied(fixture: &LiveCanaryFixture, run: &HeadlessRun, final_lib: &str) {
    assert!(
        matches!(run.terminal(), Some(HeadlessTerminal::Applied { .. })),
        "expected applied terminal; artifacts at {}",
        fixture.artifact_root.display()
    );
    assert!(
        final_lib.contains("\"after\"") && !final_lib.contains("\"before\""),
        "expected sentinel change in src/lib.rs; artifacts at {}",
        fixture.artifact_root.display()
    );
    assert!(
        run.events().iter().any(|event| matches!(
            event,
            Event::Proposal { paths, .. }
                if paths.iter().any(|path| path.ends_with("src/lib.rs"))
        )),
        "expected proposal evidence for src/lib.rs; artifacts at {}",
        fixture.artifact_root.display()
    );
}

fn assert_workspace_index_ready(fixture: &LiveCanaryFixture, run: &HeadlessRun) {
    let diagnostic = run.prompt_diagnostics().first().unwrap_or_else(|| {
        panic!(
            "expected prompt diagnostics; artifacts at {}",
            fixture.artifact_root.display()
        )
    });
    assert!(
        diagnostic.workspace.loaded,
        "expected loaded workspace in prompt diagnostics; artifacts at {}",
        fixture.artifact_root.display()
    );
    assert!(
        diagnostic.workspace.member_count > 0,
        "expected loaded workspace members in prompt diagnostics; artifacts at {}",
        fixture.artifact_root.display()
    );
    assert!(
        matches!(diagnostic.bm25.as_ref(), Some(Bm25Diagnostic { status, docs: Some(docs), .. }) if status == "ready" && *docs > 0),
        "expected ready BM25 in prompt diagnostics, got {:?}; artifacts at {}",
        diagnostic.bm25,
        fixture.artifact_root.display()
    );
}

fn assert_auto_context_off(fixture: &LiveCanaryFixture, run: &HeadlessRun) {
    let diagnostic = run.prompt_diagnostics().first().unwrap_or_else(|| {
        panic!(
            "expected prompt diagnostics; artifacts at {}",
            fixture.artifact_root.display()
        )
    });
    assert!(
        diagnostic.context_mode == "Off",
        "expected automatic prompt context off, got {}; artifacts at {}",
        diagnostic.context_mode,
        fixture.artifact_root.display()
    );
    assert_eq!(
        diagnostic.included_rag_parts,
        0,
        "expected no automatic prompt RAG parts; artifacts at {}",
        fixture.artifact_root.display()
    );
    assert!(
        matches!(
            diagnostic.fallback_notice.as_deref(),
            Some(notice)
                if notice.starts_with("Context mode is Off:")
                    && notice.contains("request_code_context is still available")
                    && notice.contains("workspace loaded ")
        ),
        "expected intentional Off fallback notice, got {:?}; artifacts at {}",
        diagnostic.fallback_notice,
        fixture.artifact_root.display()
    );
}

async fn run_live_canary(fixture: &LiveCanaryFixture) -> HeadlessRun {
    let budget = Budget::new(2, 600).expect("valid live canary budget");
    match run_headless(
        &fixture.workspace,
        &fixture.prompt,
        budget,
        BroadEditPolicy::WorkspaceExceptPlokeEval,
        &[],
    )
    .await
    {
        Ok(run) => run,
        Err(err) => {
            fs::write(fixture.artifact_root.join("run-error.txt"), err.to_string())
                .expect("write run error");
            panic!(
                "live TUI adapter canary failed before returning HeadlessRun; artifacts at {}",
                fixture.artifact_root.display()
            );
        }
    }
}

fn write_live_canary_artifacts(fixture: &LiveCanaryFixture, run: &HeadlessRun) -> String {
    let final_lib = fs::read_to_string(&fixture.src_file).expect("read final lib");
    fs::write(&fixture.final_file, &final_lib).expect("write final artifact");
    fs::write(
        &fixture.evidence_path,
        serde_json::to_string_pretty(&run.evidence()).expect("serialize evidence"),
    )
    .expect("write evidence");
    fs::write(
        &fixture.events_path,
        serde_json::to_string_pretty(run.events()).expect("serialize events"),
    )
    .expect("write events");
    let diff = command_output(&fixture.workspace, "git", &["diff", "--", "src/lib.rs"]);
    fs::write(&fixture.diff_path, &diff).expect("write diff");
    fs::write(
        &fixture.report_path,
        serde_json::to_string_pretty(&LiveCanaryReport {
            artifact_root: fixture.artifact_root.clone(),
            workspace: fixture.workspace.clone(),
            prompt_path: fixture.prompt_path.clone(),
            initial_file: fixture.initial_file.clone(),
            final_file: fixture.final_file.clone(),
            evidence_path: fixture.evidence_path.clone(),
            events_path: fixture.events_path.clone(),
            diff_path: fixture.diff_path.clone(),
            terminal: run.evidence().terminal,
            requested_tools: requested_tools(run),
            final_contains_after: final_lib.contains("\"after\""),
            final_contains_before: final_lib.contains("\"before\""),
        })
        .expect("serialize report"),
    )
    .expect("write report");
    print_live_canary_summary(fixture, run, &diff);
    final_lib
}

fn prepare_live_canary(name: &str, prompt: &str) -> std::io::Result<LiveCanaryFixture> {
    let base = std::env::var_os("PLOKE_EVAL_LIVE_TUI_CANARY_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            std::env::temp_dir().join(format!(
                "ploke-eval-live-tui-canary-{}",
                Uuid::new_v4().simple()
            ))
        });
    let artifact_root = base.join(name);
    fs::create_dir_all(&artifact_root)?;
    println!(
        "live TUI adapter canary artifacts: {}",
        artifact_root.display()
    );

    let workspace = artifact_root.join("workspace");
    let src_dir = workspace.join("src");
    fs::create_dir_all(&src_dir)?;

    let cargo_toml = r#"[package]
name = "ploke-eval-live-tui-canary"
version = "0.1.0"
edition = "2024"

[lib]
path = "src/lib.rs"
"#;
    let initial_lib = r#"pub fn broad_surface_canary() -> &'static str {
"before"
}
"#;
    let src_file = src_dir.join("lib.rs");
    let prompt_path = artifact_root.join("prompt.txt");
    let initial_file = artifact_root.join("initial-src-lib.rs");
    let final_file = artifact_root.join("final-src-lib.rs");
    let evidence_path = artifact_root.join("headless-evidence.json");
    let events_path = artifact_root.join("headless-events.json");
    let diff_path = artifact_root.join("workspace.diff");
    let report_path = artifact_root.join("report.json");

    fs::write(workspace.join("Cargo.toml"), cargo_toml)?;
    fs::write(&src_file, initial_lib)?;
    fs::write(&prompt_path, prompt)?;
    fs::write(&initial_file, initial_lib)?;

    command_output(&workspace, "git", &["init"]);
    command_output(&workspace, "git", &["add", "Cargo.toml", "src/lib.rs"]);
    command_output(
        &workspace,
        "git",
        &[
            "-c",
            "user.email=ploke-eval-live-canary@example.invalid",
            "-c",
            "user.name=ploke eval live canary",
            "commit",
            "--allow-empty",
            "-m",
            "initial canary",
        ],
    );

    Ok(LiveCanaryFixture {
        artifact_root,
        workspace,
        src_file,
        prompt: prompt.to_string(),
        prompt_path,
        initial_file,
        final_file,
        evidence_path,
        events_path,
        diff_path,
        report_path,
    })
}

fn tool_request_position(run: &HeadlessRun, tool: &str) -> Option<usize> {
    run.events().iter().position(
        |event| matches!(event, Event::ToolRequest { tool: observed, .. } if observed == tool),
    )
}

fn requested_tools(run: &HeadlessRun) -> Vec<String> {
    run.events()
        .iter()
        .filter_map(|event| match event {
            Event::ToolRequest { tool, .. } => Some(tool.clone()),
            _ => None,
        })
        .collect()
}

fn print_live_canary_summary(fixture: &LiveCanaryFixture, run: &HeadlessRun, diff: &str) {
    println!(
        "live TUI adapter canary report: {}",
        fixture.report_path.display()
    );
    println!("terminal: {:?}", run.terminal());
    println!("requested tools: {}", requested_tools(run).join(", "));
    for event in run.events() {
        if let Event::ToolRequest {
            tool,
            call_id,
            arguments,
            ..
        } = event
        {
            println!("tool request {tool} {call_id}: {arguments}");
        }
    }
    println!("workspace diff:\n{diff}");
}

#[derive(Debug)]
struct LiveCanaryFixture {
    artifact_root: PathBuf,
    workspace: PathBuf,
    src_file: PathBuf,
    prompt: String,
    prompt_path: PathBuf,
    initial_file: PathBuf,
    final_file: PathBuf,
    evidence_path: PathBuf,
    events_path: PathBuf,
    diff_path: PathBuf,
    report_path: PathBuf,
}

#[derive(Debug, Serialize)]
struct LiveCanaryReport {
    artifact_root: PathBuf,
    workspace: PathBuf,
    prompt_path: PathBuf,
    initial_file: PathBuf,
    final_file: PathBuf,
    evidence_path: PathBuf,
    events_path: PathBuf,
    diff_path: PathBuf,
    terminal: Option<evidence::Terminal>,
    requested_tools: Vec<String>,
    final_contains_after: bool,
    final_contains_before: bool,
}

fn command_output(cwd: &Path, program: &str, args: &[&str]) -> String {
    let output = Command::new(program)
        .current_dir(cwd)
        .args(args)
        .output()
        .unwrap_or_else(|err| panic!("failed to run {program} {args:?}: {err}"));
    let mut rendered = String::new();
    rendered.push_str(&String::from_utf8_lossy(&output.stdout));
    rendered.push_str(&String::from_utf8_lossy(&output.stderr));
    if !output.status.success() {
        panic!(
            "{program} {args:?} failed with status {:?}: {rendered}",
            output.status.code()
        );
    }
    rendered
}
