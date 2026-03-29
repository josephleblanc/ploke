# Date
2026-03-28

# Task Title
Live Corpus Download and Triage Runbook

# Task Description
Provide a compact operator handoff for running a long `parse debug corpus` sweep while triaging failures in parallel from persisted artifacts and assigning sub-agents as soon as failures appear.

# Related Planning Files
- [2026-03-28_corpus-triage-workflow.md](/home/brasides/code/ploke/docs/active/agents/2026-03-28_corpus-triage-workflow.md)
- [2026-03-28_error-diagnostic-rollout-plan.md](/home/brasides/code/ploke/docs/active/agents/2026-03-28_error-diagnostic-rollout-plan.md)

## Intent

Use corpus download/parse time productively by triaging failures as they appear, instead of waiting for the full run to finish.

## Commands

Main corpus run:

```text
cargo xtask parse debug corpus --limit 100 --workspace-mode probe
```

Live triage watcher against the active run directory:

```text
cargo xtask parse debug corpus-triage <run-id-or-run-dir> --watch --interval-secs 15
```

One-shot refresh from an in-progress or completed run:

```text
cargo xtask parse debug corpus-triage <run-id-or-run-dir>
```

Watch until the top-level run summary appears, then stop:

```text
cargo xtask parse debug corpus-triage <run-id-or-run-dir> --watch --interval-secs 15 --exit-when-complete
```

## Expected Behavior

- If `<run-dir>/summary.json` exists, triage uses the completed run summary.
- If the top-level summary does not exist yet, triage falls back to scanning per-target `summary.json` files and in-progress workspace `workspace_probe/workspace_summary.json` files already written under the run directory.
- Triage outputs are refreshed under `<run-dir>/triage/`.
- Pending report stubs are still generated per clustered failure signature, but cluster formation is for dedupe and later consolidation, not a prerequisite for dispatch.

## Artifacts

Under `<run-dir>/triage/`:
- `index.json`
- `failures.jsonl`
- `clusters.json`
- `reports/_report_template.json`
- `reports/pending/*.json`

## Main Agent Workflow

1. Start the corpus run.
2. Identify the active run ID from the startup output or the newly created directory under `target/debug_corpus_runs/`.
3. Start the triage watcher for that run.
4. Query `index.json` and `clusters.json`.
5. Dispatch a sub-agent as soon as a new failure appears.
6. Use `clusters.json` to avoid duplicate investigations and to fold matching failures into existing work.
7. Keep implementation work out of parser internals unless explicitly discussed with the user, especially for:
- `code_visitor.rs`
- merge functions
- pruning functions
- other complex pipeline steps

## Sub-Agent Expectations

Each sub-agent should update the relevant pending report with:
- suspected root cause
- confidence
- fix-vs-document recommendation
- whether the issue touches sensitive pipeline areas
- recommended next step
- relevant artifact paths
- relevant code paths

## Query Snippets

```text
jq -c '.clusters[] | {count, stage, failure_kind, error_signature, pending_report_path}' <run-dir>/triage/clusters.json
jq -c '.failures[] | {id, normalized_repo, member_label, stage, error_signature}' <run-dir>/triage/index.json
rg -n '"status": "pending"' <run-dir>/triage/reports/pending
```
