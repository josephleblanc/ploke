# Date
2026-03-30

# Task Title
Live Corpus Download and Triage Runbook V2

# Task Description
Provide an operator handoff for running a long `parse debug corpus` sweep while triaging failures in parallel from persisted artifacts, with explicit downstream support for creating minimal repro tests from observed failures.

# Related Planning Files
- [2026-03-28_corpus-triage-workflow.md](/home/brasides/code/ploke/docs/active/agents/2026-03-29_corpus-triage/2026-03-28_corpus-triage-workflow.md)
- [2026-03-28_error-diagnostic-rollout-plan.md](/home/brasides/code/ploke/docs/active/agents/2026-03-28_error-diagnostic-rollout-plan.md)

## Intent

Use corpus download and parse time productively by triaging failures as they appear, while leaving behind a reproducible handoff that a later human or agent can use to build minimal repro tests without re-discovering basic context.

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

Inspect one saved target:

```text
cargo xtask parse debug corpus-show <run-id-or-run-dir> --target <owner/repo>
```

Inspect one saved target with persisted backtrace:

```text
cargo xtask parse debug corpus-show <run-id-or-run-dir> --target <owner/repo> --backtrace
```

Watch until the top-level run summary appears, then stop:

```text
cargo xtask parse debug corpus-triage <run-id-or-run-dir> --watch --interval-secs 15 --exit-when-complete
```

## Required Run Identity

Record these at the start of the session document or writeup:

- exact corpus command line
- run id
- run directory
- whether the run is using the default artifact root or an explicit `--artifact-dir`
- whether the run is using the default checkout root or an explicit `--checkout-dir`
- timeout settings if they differ from defaults
- whether the run completed, was interrupted, or appears stalled

If a non-default artifact or checkout directory is used, repeat that path in the final writeup so later work does not assume `target/debug_corpus_runs/` or `tests/fixture_github_clones/corpus/`.

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
- A later repro task may be blocked if triage leaves behind only cluster-level judgments and not exact target/member/revision evidence for at least one canonical example per cluster.

## Artifacts

Under `<run-dir>/triage/`:
- `index.json`
- `failures.jsonl`
- `clusters.json`
- `reports/_report_template.json`
- `reports/pending/*.json`

Under each failed target artifact directory:
- `summary.json`
- stage subdirectories such as `discovery/`, `resolve/`, `merge/`
- `stage_summary.json`
- `failure.json` for failed or panicked stages
- stage payloads such as `discovery.json`, `resolve.json`, or `merged_graph.json`

## Main Agent Workflow

1. Start the corpus run.
2. Identify the active run id from startup output or the newly created run directory.
3. Record the exact run command, run directory, artifact root, and checkout root immediately.
4. Start the triage watcher for that run.
5. Keep monitoring until the corpus run terminates or the user explicitly stops the session. Poll the live artifact files at least every 5 minutes even if the terminal sessions are quiet, and poll more frequently while the run is actively changing or while sub-agent work is in flight.
6. Query `index.json` and `clusters.json`.
7. Dispatch a sub-agent as soon as a new failure appears.
8. Use `clusters.json` to avoid duplicate investigations and to fold matching failures into existing work.
9. For each active cluster, ensure at least one canonical failure example is documented with exact target, member if applicable, stage, and artifact paths.
10. Keep implementation work out of parser internals unless explicitly discussed with the user, especially for:
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

Each sub-agent should also capture enough concrete evidence for later repro work:

- one canonical failure example for the cluster
- exact `normalized_repo`
- exact failing stage
- exact member label or member path component for workspace failures
- exact artifact paths for the canonical example
- exact saved error signature
- concise backtrace or diagnostic summary if available
- exact commit SHA if it is present in the persisted target summary
- whether the checkout still exists and at what path
- one recommended narrow rerun command for follow-up investigation
- a short note on what a future repro test should assert

Before treating a failure as novel, check `/home/brasides/code/ploke/docs/design/known_limitations/` for an existing documented limitation that matches the signature or parser behavior. If there is a plausible match, note the matching limitation ID in the report and frame the recommendation as confirm or document unless the live artifact clearly shows a new failure mode.

If a cluster contains many occurrences, it is acceptable to keep the report cluster-oriented, but do not omit the canonical example block. The cluster report should still be actionable by someone who has not seen the original live session.

When structured report writes are used:

- assign only one sub-agent to a given pending report path at a time
- if two failures look similar but map to different report paths, keep them separate until a human confirms they are duplicates
- if a pending report already contains a canonical example, append new evidence instead of replacing the original example unless the original was wrong or incomplete

## Canonical Example Block

Each pending report should contain a compact canonical example block with:

- `failure_id`
- `normalized_repo`
- `commit_sha` if known
- `member_label` or member path if applicable
- `stage`
- `error_signature`
- `artifact_dir`
- `summary_path`
- `failure_json_path`
- `stage_summary_path`
- `checkout_path`
- `checkout_present`
- `suggested_rerun_command`
- `suggested_test_assertion`

If any field is unavailable, say so explicitly rather than leaving it ambiguous.

## Query Snippets

```text
jq -c '.clusters[] | {count, stage, failure_kind, error_signature, pending_report_path}' <run-dir>/triage/clusters.json
jq -c '.failures[] | {id, normalized_repo, member_label, stage, error_signature}' <run-dir>/triage/index.json
rg -n '"status": "pending"' <run-dir>/triage/reports/pending
```

Useful follow-up inspection commands:

```text
cargo xtask parse debug corpus-show <run-id-or-run-dir> --target <owner/repo> --backtrace
cargo xtask parse debug corpus-show <run-id-or-run-dir> --target <owner/repo> --member <member-label> --backtrace
```

## Post-Run Writeup

After the corpus process exits, is terminated, or is judged stalled, leave a short operator writeup that captures:

- run id and run directory
- exact run command
- artifact root and checkout root
- whether the run completed cleanly, was interrupted by an operator, or appears to have stalled
- whether the writeup is based on a completed top-level `summary.json` or only partial persisted artifacts
- total failure count and the main failure clusters or report paths
- which failures matched documented known limitations
- which failures still look novel or need follow-up
- which pending reports are canonical starting points for minimal repro creation
- any operational issues encountered during the run, such as:
- watcher crashes on partially written artifacts
- checkout stalls or clone hangs before stage artifacts are written
- repeated timeout-heavy targets
- missing or cleaned-up artifact/checkouts that would block later repro work

If the run did not produce a top-level `summary.json`, explicitly say that the writeup is based on partial persisted artifacts under `<run-dir>/` rather than a completed corpus summary.

## Success Criteria

The workflow is successful only if a later human or agent can pick one pending report or one post-run writeup entry and answer, without rerunning the full corpus:

- what failed
- where it failed
- which saved artifacts prove it
- which repo revision and member were involved
- what narrow command should be run next
- what a minimal repro test should eventually assert
