# Date
2026-03-30

# Task Title
Live Corpus Download and Triage Runbook V2

# Task Description
Provide an operator handoff for running a long `parse debug corpus` sweep while triaging failures in parallel from persisted artifacts, with explicit downstream support for creating minimal repro tests from observed failures.

# Related Planning Files
- [2026-03-28_corpus-triage-workflow.md](/home/brasides/code/ploke/docs/active/agents/2026-03-29_corpus-triage/2026-03-28_corpus-triage-workflow.md)
- [2026-03-28_error-diagnostic-rollout-plan.md](/home/brasides/code/ploke/docs/active/agents/2026-03-28_error-diagnostic-rollout-plan.md)
- [2026-03-30_corpus-repro-report-template.json](/home/brasides/code/ploke/docs/active/agents/2026-03-29_corpus-triage/2026-03-30_corpus-repro-report-template.json)

## Intent

Use corpus download and parse time productively by triaging failures as they appear, while leaving behind a reproducible handoff that a later human or agent can use to build minimal repro tests without re-discovering basic context.

## Commands

Main corpus run:

```text
cargo xtask parse debug corpus --limit 100 --workspace-mode probe --artifact-dir xtask/debug_corpus_runs
```

Live triage watcher against the active run directory:

```text
cargo xtask parse debug corpus-triage <run-id-or-run-dir> --watch --interval-secs 15
```

One-shot refresh from an in-progress or completed run:

```text
cargo xtask parse debug corpus-triage <run-id-or-run-dir>
```

Export a machine-readable repro handoff for one cluster, target, member, or failure:

```text
cargo xtask --format json parse debug corpus-repro <run-id-or-run-dir>
cargo xtask --format json parse debug corpus-repro <run-id-or-run-dir> --cluster <cluster-key-or-slug>
cargo xtask --format json parse debug corpus-repro <run-id-or-run-dir> --target <owner/repo> --member <member-label>
cargo xtask --format json parse debug corpus-repro <run-id-or-run-dir> --failure <failure-id>
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

If a non-default artifact or checkout directory is used, repeat that path in the final writeup so later work does not assume `xtask/debug_corpus_runs/` or `tests/fixture_github_clones/corpus/`.
The default artifact root is `xtask/debug_corpus_runs/`.

## Expected Behavior

- If `<run-dir>/summary.json` exists, triage uses the completed run summary.
- If the top-level summary does not exist yet, triage falls back to scanning per-target `summary.json` files and in-progress workspace `workspace_probe/workspace_summary.json` files already written under the run directory.
- Triage outputs are refreshed under `<run-dir>/triage/`.
- Pending report stubs are still generated per clustered failure signature, but cluster formation is for dedupe and later consolidation, not a prerequisite for dispatch.
- `corpus-repro` can be run against a completed or partial snapshot and returns failure-derived `selected_examples` plus cluster-level `canonical_examples`.

## Current Limitations

- Treat `index.json` as the live failure feed and `clusters.json` as a dedupe aid only. Do not wait for clusters before dispatching work.
- Pending cluster reports are preserved once created, which avoids watcher overwrite, but also means stub metadata such as occurrence counts can become stale during a long watch session.
- Use one writer per pending report file. If two sub-agents write structured output back to the same cluster report, the later write can overwrite the earlier one.
- Pending report filenames are derived from a sanitized cluster key. Distinct cluster keys can theoretically collapse to the same filename, so if a report path looks unexpectedly reused, stop and inspect before continuing.
- A later repro task may be blocked if triage leaves behind only cluster-level judgments and not exact target/member/revision evidence for at least one canonical example per cluster.
- `--watch` now retries through transient partial-write snapshot parse errors, but persistent malformed JSON or missing template files are still hard failures and should be treated as workflow issues.

## Artifacts

Under `<run-dir>/triage/`:
- `index.json`
- `failures.jsonl`
- `clusters.json`
- `reports/_report_template.json`
- `reports/pending/*.json`

Required sub-agent handoff template:
- [2026-03-30_corpus-repro-report-template.json](/home/brasides/code/ploke/docs/active/agents/2026-03-29_corpus-triage/2026-03-30_corpus-repro-report-template.json)

From `cargo xtask --format json parse debug corpus-repro ...`:
- `schema_version`
- `snapshot_mode`
- `summary_path`
- `run_dir`
- `triage_dir`
- `triage_present`
- `triage_index_path`
- `triage_clusters_path`
- `failures`
- `clusters`
- `canonical_examples`
- `selected_examples`

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
9. For each active cluster, ensure at least one canonical failure example is documented with exact target, member if applicable, stage, artifact paths, and a matching `corpus-repro` export.
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

Each structured sub-agent report should conform to:
- [2026-03-30_corpus-repro-report-template.json](/home/brasides/code/ploke/docs/active/agents/2026-03-29_corpus-triage/2026-03-30_corpus-repro-report-template.json)

Each sub-agent should also capture enough concrete evidence for later repro work:

- one canonical failure example for the cluster
- the exact `corpus-repro` selector used, such as `--failure <id>` or `--cluster <slug>`
- exact `normalized_repo`
- exact failing stage
- exact member label or member path component for workspace failures
- exact artifact paths for the canonical example
- exact saved error signature
- concise backtrace or diagnostic summary if available
- exact commit SHA if it is present in the persisted target summary
- whether the checkout still exists and at what path
- one inspection command and one narrow repro command for follow-up investigation, if available
- a short note on what a future repro test should assert

Before treating a failure as novel, check `/home/brasides/code/ploke/docs/design/known_limitations/` for an existing documented limitation that matches the signature or parser behavior. If there is a plausible match, note the matching limitation ID in the report and frame the recommendation as confirm or document unless the live artifact clearly shows a new failure mode.

If a cluster contains many occurrences, it is acceptable to keep the report cluster-oriented, but do not omit the canonical example block. The cluster report should still be actionable by someone who has not seen the original live session.

When structured report writes are used:

- assign only one sub-agent to a given pending report path at a time
- if two failures look similar but map to different report paths, keep them separate until a human confirms they are duplicates
- if a pending report already contains a canonical example, append new evidence instead of replacing the original example unless the original was wrong or incomplete

## Canonical Example Block

Each pending report should contain a canonical example block copied or derived from one `selected_examples[]` entry from `cargo xtask --format json parse debug corpus-repro ...`.

The preferred shape is the `canonical_example` object from:
- [2026-03-30_corpus-repro-report-template.json](/home/brasides/code/ploke/docs/active/agents/2026-03-29_corpus-triage/2026-03-30_corpus-repro-report-template.json)

Required fields:

- `failure_id`
- `run_id`
- `cluster_key`
- `cluster_slug`
- `normalized_repo`
- `repository_kind`
- `commit_sha` if known
- `source`
- `member_label` or `member_path` if applicable
- `stage`
- `failure_kind`
- `panic`
- `error_signature`
- `error_excerpt`
- `checkout_path`
- `checkout_present`
- `artifact_path`
- `artifact_present`
- `failure_artifact_path`
- `failure_artifact_present`
- `target_summary_path`
- `target_summary_present`
- `workspace_summary_path`
- `workspace_summary_present`
- `suggested_inspection_command`
- `suggested_repro_command`
- `suggested_test_assertion`
- `relevant_artifacts`
- `relevant_code_paths`

If any field is unavailable, say so explicitly rather than leaving it ambiguous.

Use `selected_examples` for failure-specific handoff. Use `canonical_examples` only as the cluster-level summary example.

## Query Snippets

```text
jq -c '.clusters[] | {count, stage, failure_kind, error_signature, pending_report_path}' <run-dir>/triage/clusters.json
jq -c '.failures[] | {id, normalized_repo, member_label, stage, error_signature}' <run-dir>/triage/index.json
rg -n '"status": "pending"' <run-dir>/triage/reports/pending
cargo xtask --format json parse debug corpus-repro <run-id-or-run-dir> --failure <failure-id>
```

Useful follow-up inspection commands:

```text
cargo xtask parse debug corpus-show <run-id-or-run-dir> --target <owner/repo> --backtrace
cargo xtask parse debug corpus-show <run-id-or-run-dir> --target <owner/repo> --member <member-label> --backtrace
```

Useful repro-handoff extraction with `jq`:

```text
cargo xtask --format json parse debug corpus-repro <run-id-or-run-dir> --failure <failure-id> | jq '.selected_examples[0]'
cargo xtask --format json parse debug corpus-repro <run-id-or-run-dir> --cluster <cluster-slug> | jq '.canonical_examples'
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
- which `corpus-repro --failure ...` or `--cluster ...` selectors should be used first for follow-up work
- any operational issues encountered during the run, such as:
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
- what inspection command should be run next
- what narrow repro command should be run next
- what a minimal repro test should eventually assert
