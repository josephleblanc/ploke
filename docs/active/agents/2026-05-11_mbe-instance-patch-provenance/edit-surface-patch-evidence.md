# Scope

Discovery-only report for typed-persistence inventory family `edit-surface-patch-evidence`, focused on how a Prototype 1 child/TUI edit proposal can become the workspace diff exported as the Multi-SWE-bench `fix_patch`.

Questions covered: proposal staging, applied/failed status, expected file-change evidence, patch-artifact records, and whether failed proposal status can coexist with an exported MBE patch.

# Inventory Rows Used

- `edit_surface.proposal_registry`: accepted survey row in `docs/active/plans/self-improvement-loop/typed-persistence-spine/reports/2026-05-10-edit-surface-patch-evidence.survey-d-v2.jsonl`, row 10. Source: `crates/ploke-tui/src/app_state/handlers/proposals.rs:1-107`. Current type: `Vec<EditProposal> / Vec<CreateProposal>`.
- `edit_surface.patch_artifact`: same survey, row 11. Source: `crates/ploke-eval/src/runner.rs:801-986`. Current type: `PatchArtifact + ProposalSnapshotRecord`.
- `edit_surface.surface_evidence_record`: same survey, row 4. Source: `crates/ploke-records/src/history/payload.rs:274-324`. Current type: `SurfaceEvidenceRecord`.
- `edit_surface.surface_attempt_record`: same survey, row 5. Source: `crates/ploke-records/src/history/payload.rs:326-353`. Current type: `SurfaceAttemptRecord`.

The survey summary says all discovered shapes are typed; remaining work is ownership splits and replay joins.

# Source Code Ranges Worth Reading

- `crates/ploke-tui/src/app_state/core.rs:321-382`: `EditProposalStatus`, `DiffPreview`, and `EditProposal`. This is the typed status carrier for staged/applied/failed proposals.
- `crates/ploke-tui/src/app_state/handlers/proposals.rs:1-107`: proposal registry persistence to `proposals.json` and `create_proposals.json`, with typed `serde_json::from_str::<Vec<EditProposal>>()`.
- `crates/ploke-tui/src/rag/tools.rs:70-286`: semantic edit proposal staging; writes `EditProposal { status: Pending, preview, files, edits }`.
- `crates/ploke-tui/src/rag/tools.rs:1168-1229`: non-semantic `ns_patch` staging; writes `EditProposal { status: Pending, edits_ns, preview, files }`.
- `crates/ploke-tui/src/rag/editing.rs:180-203`: non-semantic apply result; if some files were written but not all, status becomes `Failed("Partially applied...")`.
- `crates/ploke-tui/src/rag/editing.rs:286-300`: non-semantic apply error path; status becomes `Failed(e.to_string())`.
- `crates/ploke-tui/src/rag/editing.rs:378-462`: semantic apply result; any applied semantic edit sets status `Applied`, otherwise `Failed`.
- `crates/ploke-records/src/history/payload.rs:274-353`: typed surface records: `SurfaceEvidenceRecord` has `check_status: Checked` and `apply_status: Applied`; `SurfaceAttemptRecord` has `outcome: Applied | Rejected`.
- `crates/ploke-eval/src/runner.rs:802-829`: `ProposalSnapshotRecord`, `PatchArtifact`, and `ExpectedFileChangeRecord`.
- `crates/ploke-eval/src/runner.rs:929-1005`: collects proposal snapshots and computes `applied`, `all_proposals_applied`, and expected file-change booleans.
- `crates/ploke-eval/src/runner.rs:1008-1068`: expected patch-file selection and MBE submission `fix_patch` collection via `git diff`.
- `crates/ploke-eval/src/runner.rs:1092-1121`: expected file-change records from before/after hashes.
- `crates/ploke-eval/src/runner.rs:2128-2136` and `2369-2382`: snapshot expected files before the turn, pass baselines into `run_benchmark_turn`, then write `agent-turn-summary.json`.
- `crates/ploke-eval/src/runner.rs:3248-3382`: `AgentTurnArtifact` starts with empty patch evidence; on timeout or terminal turn it calls `collect_patch_artifact_with_expected`.
- `crates/ploke-eval/src/runner.rs:2454-2486`: writes `multi-swe-bench-submission.jsonl` and records whether the submission patch is empty/nonempty.
- `crates/ploke-eval/src/runner.rs:4756-4848`: test proving MBE submission uses the repo diff only for a partial-change state.
- `crates/ploke-eval/src/runner.rs:4986-5100`: tests proving `PatchArtifact.applied` is true if any proposal status is `Applied`, while `all_proposals_applied` is false if another proposal is `Failed`.
- `crates/ploke-eval/src/runner.rs:5102-5121`: test proving expected-file change records detect before/after hash transitions.

# Persisted Records / Artifact Paths Involved

- TUI proposal registry:
  - Default edit path: `${config_dir}/ploke/proposals.json`.
  - Override edit path: `PLOKE_PROPOSALS_PATH`.
  - Default create path: `${config_dir}/ploke/create_proposals.json`.
  - Override create path: `PLOKE_CREATE_PROPOSALS_PATH`.
  - Types: `Vec<EditProposal>` and `Vec<CreateProposal>`.
- Turn artifacts in each eval run output directory:
  - `agent-turn-trace.json`: continuously rewritten `AgentTurnArtifact`.
  - `agent-turn-summary.json`: final `AgentTurnArtifact`.
  - `PatchArtifact` inside the turn artifact records proposal snapshots and expected file-change hashes.
- MBE submission artifact:
  - `multi-swe-bench-submission.jsonl`.
  - Type: `MultiSweBenchSubmissionRecord { org, repo, number, fix_patch }`.
  - `fix_patch` is collected from `git diff --no-ext-diff --binary <base_sha-or-HEAD> --`, not from proposal preview text or proposal status.
- Prototype 1 edit-surface records:
  - `SurfaceEvidenceRecord`: typed checked-and-applied surface evidence.
  - `SurfaceAttemptRecord`: typed applied/rejected attempt evidence.
  - These are not the same artifact as the MBE `fix_patch`; they are the typed edit-surface evidence side of the provenance chain.

# Provenance Chain Contribution

The chain visible from source is:

1. Tool call stages an `EditProposal` with typed preview/files/edits and `status: Pending`.
2. Approval/apply path mutates the same proposal to `Applied` or `Failed`.
3. Eval runner snapshots the proposal registry into `PatchArtifact.edit_proposals` / `create_proposals` as `ProposalSnapshotRecord`.
4. Eval runner separately snapshots expected files before the turn and hashes them after the turn into `ExpectedFileChangeRecord`.
5. MBE submission packaging separately exports the actual workspace diff with `git diff`; this is the `fix_patch` used by Multi-SWE-bench.

Staged vs actually applied is distinguished by `EditProposalStatus` in the proposal registry. `PatchArtifact` preserves that status as strings in proposal snapshots and adds aggregate booleans. The stronger edit-surface evidence records distinguish checked/applied candidates (`SurfaceEvidenceRecord`) from applied/rejected attempts (`SurfaceAttemptRecord`), but proposal snapshots remain TUI-local registry projections.

There is code-level evidence that a failed proposal can leave a changed workspace diff that later gets exported:

- `ns_patch` can write some files, then mark the proposal `Failed` when only a partial file set applied.
- MBE submission `fix_patch` ignores proposal status and exports the repo diff.
- The partial-change MBE test proves the exporter includes the changed file and does not invent a diff for expected-but-unchanged files.

No raw run artifact was inspected here, so this report does not claim observed historical MBE data contains such a failed-partial proposal. It identifies the source-level path that permits it and the smallest ranges to verify against a named run artifact if needed.

# Smallest Verification Commands

- Inventory row lookup:
  - `rg -n 'edit_surface\\.(proposal_registry|patch_artifact|surface_evidence_record|surface_attempt_record)' docs/active/plans/self-improvement-loop/typed-persistence-spine/inventory.md`
- Accepted survey rows, width-capped:
  - `sed -n '1p;4,5p;10,11p' docs/active/plans/self-improvement-loop/typed-persistence-spine/reports/2026-05-10-edit-surface-patch-evidence.survey-d-v2.jsonl | cut -c 1-1600`
- Source ranges:
  - `nl -ba crates/ploke-tui/src/app_state/core.rs | sed -n '321,382p'`
  - `nl -ba crates/ploke-tui/src/app_state/handlers/proposals.rs | sed -n '1,107p'`
  - `nl -ba crates/ploke-tui/src/rag/tools.rs | sed -n '70,286p;1168,1229p'`
  - `nl -ba crates/ploke-tui/src/rag/editing.rs | sed -n '180,203p;286,300p;378,462p'`
  - `nl -ba crates/ploke-eval/src/runner.rs | sed -n '802,829p;929,1005p;1008,1068p;1092,1121p;3248,3382p;4756,4848p;4986,5121p'`
- Targeted tests:
  - `cargo test -p ploke-eval collect_patch_artifact_snapshots_applied_proposals 2>&1 | tail -n 40`
  - `cargo test -p ploke-eval collect_patch_artifact_marks_partial_apply_as_applied_but_not_all_applied 2>&1 | tail -n 40`
  - `cargo test -p ploke-eval write_msb_submission_artifact_uses_repo_diff_only_for_partial_apply_state 2>&1 | tail -n 40`
  - `cargo test -p ploke-eval expected_file_change_records_hash_transition 2>&1 | tail -n 40`

# Avoid Reading Wholesale

- Do not read `agent-turn-trace.json`, `agent-turn-summary.json`, `multi-swe-bench-submission.jsonl`, `transition-journal.jsonl`, `llm-full-responses.jsonl`, or any run log wholesale.
- If a specific MBE run must be checked, inspect metadata first: `ls -lh`, `wc -l`, mtimes, and artifact path names.
- For JSONL, read at most one named file and a few width-capped records, for example: `tail -n 3 <run-output>/multi-swe-bench-submission.jsonl | cut -c 1-400`.
- For JSON turn artifacts, prefer typed reader/test code or narrow key probes; do not dump the full artifact into context.

# Open Questions

- Which exact historical run output directory produced the MBE instance patch under investigation?
- Did that run's `agent-turn-summary.json` contain a `PatchArtifact` with `any_expected_file_changed: true` while all proposal snapshots were non-`Applied`, or was there at least one `Applied` proposal?
- Was the changed workspace diff caused by the non-semantic partial-apply path, the semantic apply path, or an edit outside the proposal registry?
- Should proposal snapshots remain a TUI-local projection, or should `ProposalSnapshotRecord` move into a shared replay record home for direct typed joins with MBE submission artifacts?
