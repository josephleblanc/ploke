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

## Current Limitations

- Treat `index.json` as the live failure feed and `clusters.json` as a dedupe aid only. Do not wait for clusters before dispatching work.
- Pending cluster reports are preserved once created, which avoids watcher overwrite, but also means stub metadata such as occurrence counts can become stale during a long watch session.
- Use one writer per pending report file. If two sub-agents write structured output back to the same cluster report, the later write can overwrite the earlier one.
- Pending report filenames are derived from a sanitized cluster key. Distinct cluster keys can theoretically collapse to the same filename, so if a report path looks unexpectedly reused, stop and inspect before continuing.

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
4. Keep monitoring until the corpus run terminates or the user explicitly stops the session. Poll the live artifact files at least every 5 minutes even if the terminal sessions are quiet, and poll more frequently while the run is actively changing or while sub-agent work is in flight.
5. Query `index.json` and `clusters.json`.
6. Dispatch a sub-agent as soon as a new failure appears.
7. Use `clusters.json` to avoid duplicate investigations and to fold matching failures into existing work.
8. Keep implementation work out of parser internals unless explicitly discussed with the user, especially for:
- `code_visitor.rs`
- merge functions
- pruning functions
- other complex pipeline steps

## Sub-Agent Expectations

Each sub-agent should update the relevant pending report with:
- suspected root cause
- confidence
- whether the failure appears to match an existing entry under `/home/brasides/code/ploke/docs/design/known_limitations/`
- fix-vs-document recommendation
- whether the issue touches sensitive pipeline areas
- recommended next step
- relevant artifact paths
- relevant code paths

Before treating a failure as novel, check `/home/brasides/code/ploke/docs/design/known_limitations/` for an existing documented limitation that matches the signature or parser behavior. If there is a plausible match, note the matching limitation ID in the report and frame the recommendation as confirm/document unless the live artifact clearly shows a new failure mode.

When structured report writes are used:
- Assign only one sub-agent to a given pending report path at a time.
- If two failures look similar but map to different report paths, keep them separate until a human confirms they are duplicates.

## Query Snippets

```text
jq -c '.clusters[] | {count, stage, failure_kind, error_signature, pending_report_path}' <run-dir>/triage/clusters.json
jq -c '.failures[] | {id, normalized_repo, member_label, stage, error_signature}' <run-dir>/triage/index.json
rg -n '"status": "pending"' <run-dir>/triage/reports/pending
```

## Post-Run Writeup

After the corpus process exits, is terminated, or is judged stalled, leave a short operator writeup that captures:

- run id and run directory
- whether the run completed cleanly, was interrupted by an operator, or appears to have stalled
- total failure count and the main failure clusters or report paths
- which failures matched documented known limitations
- which failures still look novel or need follow-up
- any operational issues encountered during the run, such as:
- watcher crashes on partially written artifacts
- checkout stalls or clone hangs before stage artifacts are written
- repeated timeout-heavy targets

If the run did not produce a top-level `summary.json`, explicitly say that the writeup is based on partial persisted artifacts under `<run-dir>/` rather than a completed corpus summary.
