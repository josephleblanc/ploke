# MBE Instance Patch Run Record Locator

## Scope

Discovery/report only. This locates the smallest typed/projection path from a Prototype 1 campaign node to the child self-validation runner result, agent-turn summary, treatment run directory, and Multi-SWE-bench submission patch artifact without scanning raw logs or JSONL.

The path starts from a known `campaign_manifest_path` plus `node_id`. It treats monitor projections as the primary locator family and uses LLM-attempt inventory only to identify the typed enough agent-turn/full-response sidecars.

## Inventory Rows Used

- `prototype1.node_request_projection` from `monitor-projections`: typed `Prototype1NodeRecord` and `Prototype1RunnerRequest`; gives `node_dir`, `runner_request_path`, `runner_result_path`, `workspace_root`, `branch_id`, `instance_id`, and graph provenance ids.
- `prototype1.runner_result_projection` from `monitor-projections`: typed `Prototype1RunnerResult`; gives `treatment_campaign_id`, `evaluation_artifact_path`, status, disposition, excerpts, and `recorded_at`.
- `prototype1.agent_turn_trace` from `monitor-projections`: now backed by `AgentTurnTraceProjection` instead of raw field walking for summary counts/latency, though the UI summary type is still local to `cli_facing.rs`.
- `llm.attempt.full_response_logs` from `llm-attempts`: typed sidecar `ResponseSidecar { records: Vec<RawFullResponseRecord> }`; useful only if the UI needs response spans, not needed to find the MBE patch.

## Source Code Ranges Worth Reading

- `crates/ploke-eval/src/intervention/scheduler.rs:251-330`: typed `Prototype1NodeRecord`, `Prototype1RunnerResult`, and `Prototype1RunnerRequest`.
- `crates/ploke-eval/src/intervention/scheduler.rs:390-445`: deterministic node request/result paths under `prototype1_node_dir(...)`.
- `crates/ploke-eval/src/intervention/scheduler.rs:714-870`: passive runner-result record projection plus typed read/write helpers.
- `crates/ploke-eval/src/intervention/scheduler.rs:988-1003`: `record_runner_result` writes `runner-result.json` and updates scheduler/node status.
- `crates/ploke-eval/src/intervention/scheduler.rs:1095-1320`: treatment evaluation node/request projection construction from `ResolvedTreatmentBranch`.
- `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:6093-6140`: child treatment comparison writes the branch evaluation artifact and returns the report.
- `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:8352-8575`: treatment campaign manifest path, closure-state path, branch evaluation path, and treatment evidence construction.
- `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:8990-9118`: typed treatment evidence/evaluation report/comparison row fields, including `treatment_record_path`.
- `crates/ploke-eval/src/closure.rs:150-200` and `560-620`: `ClosureInstanceRow`/`ClosureArtifactRefs` and preferred registration lookup; gives typed `run_root`, `record_path`, `msb_submission`.
- `crates/ploke-eval/src/run_registry.rs:1-110` and `crates/ploke-eval/src/inner/registry.rs:45-145`: canonical `RunRegistration` and `RunArtifactRefs` authority for run dirs and artifact refs.
- `crates/ploke-eval/src/runner.rs:96-185`: run output allocation and registration setup, including `msb_submission` pre-registration for treatment MBE runs.
- `crates/ploke-eval/src/runner.rs:907-1088`: patch artifact snapshot and `multi-swe-bench-submission.jsonl` generation from `git diff`.
- `crates/ploke-eval/src/runner.rs:2096-2130` and `2370-2565`: treatment runner output files, `agent-turn-trace.json`, `agent-turn-summary.json`, `record.json.gz`, `multi-swe-bench-submission.jsonl`, and registration updates.
- `crates/ploke-eval/src/runner.rs:3255-3385`: `AgentTurnArtifact` is repeatedly persisted to `agent-turn-trace.json` and receives final `patch_artifact`.
- `crates/ploke-eval/src/record.rs:1238-1310`: typed `AgentTurnTraceProjection` and externally tagged event deserializer used by the monitor summary.
- `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:4679-4735`: bounded agent-turn summary discovery by exact filename and typed projection parse.

## Persisted Records / Artifact Paths Involved

- Scheduler node projection: `prototype1_node_record_path(campaign_manifest_path, node_id)`, typed as `Prototype1NodeRecord`.
- Runner request projection: `prototype1_runner_request_path(campaign_manifest_path, node_id)`, typed as `Prototype1RunnerRequest`.
- Runner result projection: `prototype1_runner_result_path(campaign_manifest_path, node_id)`, typed as `Prototype1RunnerResult`.
- Branch evaluation artifact: `prototype1/evaluations/{branch_id}.json` beside the campaign manifest parent, typed as `Prototype1BranchEvaluationReport`.
- Treatment campaign manifest: `Prototype1TreatmentEvidence.treatment_campaign_manifest`.
- Treatment closure state: `Prototype1TreatmentEvidence.treatment_closure_state_path`; rows carry per-instance artifacts.
- Per-instance treatment record: `Prototype1ComparedInstanceReport.treatment_record_path` and `ClosureArtifactRefs.record_path`, normally `<run_root>/record.json.gz`.
- Treatment run directory: `RunRegistration.artifacts.run_root`, or `ClosureArtifactRefs.run_root` when rebuilding closure evidence.
- Agent-turn trace: `<run_root>/agent-turn-trace.json`, parsed through `AgentTurnTraceProjection`.
- Agent-turn summary: `<run_root>/agent-turn-summary.json`, the full `AgentTurnArtifact` including `patch_artifact`.
- MBE patch artifact: `<run_root>/multi-swe-bench-submission.jsonl`, typed as `MultiSweBenchSubmissionRecord { fix_patch, org, repo, number }`.
- Compressed run record: `<run_root>/record.json.gz`, typed as `RunRecord`; useful for replay, but not the smallest locator for the submission patch path.

## Provenance Chain Contribution

1. From `(campaign_manifest_path, node_id)`, load `Prototype1NodeRecord` and `Prototype1RunnerResult`. The node gives `branch_id`, `instance_id`, and `runner_result_path`; the result gives `evaluation_artifact_path` and `treatment_campaign_id`.
2. Load `Prototype1BranchEvaluationReport` from `evaluation_artifact_path`. Its `compared_instances[*].treatment_record_path` identifies the successful/failed treatment record for each MBE instance without reading execution logs.
3. Resolve the treatment run directory from the treatment record path via `load_registration_for_record_path` / `RunRegistration.artifacts.run_root`. If registration is unavailable, the parent directory of `record.json.gz` is a compatibility fallback, but the typed authority is the registration.
4. Locate agent-turn evidence at `RunRegistration.artifacts.turn_trace` / `turn_summary`, or by `<run_root>/agent-turn-trace.json` and `<run_root>/agent-turn-summary.json`. The trace path is enough for counts/latency; the summary contains the full `AgentTurnArtifact.patch_artifact`.
5. Locate the MBE instance patch used by Multi-SWE-bench at `RunRegistration.artifacts.msb_submission` / `<run_root>/multi-swe-bench-submission.jsonl`. The runner writes `fix_patch` from `git diff` for treatment MBE runs.

## Smallest Verification Commands

- Locate typed node/result paths from source:
  `rg -n "Prototype1RunnerResult|prototype1_runner_result_path|record_runner_result" crates/ploke-eval/src/intervention/scheduler.rs`
- Confirm branch evaluation carries treatment record paths:
  `rg -n "Prototype1BranchEvaluationReport|treatment_record_path|build_prototype1_branch_evaluation_report" crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs`
- Confirm run registration is the typed run-dir/artifact authority:
  `rg -n "RunArtifactRefs|RunRegistration|load_registration_for_record_path" crates/ploke-eval/src/inner/registry.rs crates/ploke-eval/src/run_registry.rs`
- Confirm MBE patch artifact production:
  `rg -n "multi-swe-bench-submission|MultiSweBenchSubmissionRecord|collect_submission_fix_patch" crates/ploke-eval/src/runner.rs`
- Confirm typed agent-turn summary projection:
  `rg -n "AgentTurnTraceProjection|parse_turn_trace|agent-turn-summary" crates/ploke-eval/src/record.rs crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs crates/ploke-eval/src/runner.rs`

## Avoid Reading Wholesale

- Do not scan `transition-journal.jsonl`, observation streams, execution logs, or `llm-full-responses.jsonl` to answer this provenance question.
- Do not broad-`rg` raw observation JSONL for campaign/node ids during routine provenance lookup. Use scheduler projections, branch evaluation artifacts, run registrations, and closure rows first.
- Do not read `multi-swe-bench-submission.jsonl` wholesale if the patch may be large. For diagnostics, inspect one named file with width cap, for example `sed -n '1p' <path> | cut -c 1-400`.
- Do not open `agent-turn-summary.json` wholesale unless the UI needs the actual `PatchArtifact` body; first locate it through typed registration or the run dir.

## Open Questions

- `Prototype1BranchEvaluationReport.compared_instances[*].treatment_record_path` is enough to locate the run, but it does not directly carry `run_root` or `msb_submission`. A future UI helper should join through `RunRegistration` instead of deriving sibling paths by string manipulation.
- `agent-turn-summary.json` appears to persist the full `AgentTurnArtifact`, but the accepted inventory row names only `agent-turn-trace.json` as the typed projection. The next thread should confirm whether a named typed reader exists for summary, or add one before UI drilldown relies on `patch_artifact`.
- `Prototype1RunnerResult.evaluation_artifact_path` is the smallest node-to-evaluation bridge, but the report file itself is not listed in the monitor-projections family. It is typed in code and adjacent to evaluation-oracle/edit-surface rows; future inventory may need a cross-family join row.
