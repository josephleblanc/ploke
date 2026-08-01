# Prototype 1 Broad Headless Slots Can Be Request-Only With No Launch Diagnostics

Status: patched in worktree; fresh doctor setup preflight passed in campaign `p1-admissionfix-g35flash-p25flash-20260606-140827`; live step/rerun validation pending. Observed in campaign `p1-admissionfix-g35flash-p25flash-20260606-090815`
Discovered: 2026-06-06

## Summary

The 090815 Prototype 1 parent published ten broad edit-harness requests and materialized ten clean candidate workspaces, then rejected every slot because no submitted result or `.headless-tui.json` diagnostic existed. The missing artifact is not itself the whole bug: the broader broken contract is that a published broad headless-TUI slot has no durable launch/exit/no-start record when the normal diagnostic path is never written.

Initial artifact-only triage could prove request publication, workspace materialization, and absence of submitted result/diagnostics, but not whether the headless worker never launched, crashed before returning a terminal run, hit provider/environment failure before diagnostics, timed out, or was interrupted.

Follow-up log correlation on 2026-06-06 found the missing terminal class: all ten broad attempts reached headless harness startup and failed during database/RAG preparation before the normal diagnostic writer ran:

```text
failed to start headless ploke-tui harness: database setup failed during 'bm25_ready': RAG service is unavailable
```

So the immediate 090815 failure class is now known: headless sparse RAG service was unavailable at `bm25_ready`. The worktree patch now addresses the per-slot artifact contract for this setup-failure class; a fresh live rerun is still required to prove broad headless-TUI slots launch and terminate with typed evidence in campaign artifacts.

## Implementation Status

Patched in the worktree on 2026-06-06T14:05:15-07:00:

- `prototype1-doctor` now has an explicit setup extra command:
  `--headless-tui-setup-preflight`.
- The extra command initializes the headless TUI sparse/BM25 runtime for the
  active parent checkout without making a model call and reports a structured
  `headless_tui_setup_preflight` result.
- Headless startup `PrepareError::DatabaseSetup` failures are converted to a
  typed `setup_unavailable` terminal diagnostic and persisted beside the slot as
  `.headless-tui.json` before the error is returned.
- Child-plan rejection joins that typed diagnostic; it no longer needs to fall
  back to "produced no submitted result or diagnostics" for this failure class.
- The TUI bridge preserves the setup `phase`/`detail` when setup fails, so the
  `bm25_ready`/`RAG service is unavailable` class remains visible in artifacts.

Validation run in `/home/brasides/code/ploke`:

- `cargo fmt --check` -> passed.
- `cargo test -p ploke-db typed_type_graph_presence_probe_handles_empty_schema_relations -- --nocapture` -> passed; the typed-graph presence probe now returns `false` for empty registered relations instead of a Cozo parser error.
- `cargo test -p ploke-tui test_runtime_headless_sparse_constructor_provides_rag_service -- --nocapture` -> passed; the sparse headless test runtime now constructs `state.rag`.
- `cargo test -p ploke-eval prototype1_doctor -- --nocapture` -> passed
  5 tests, including doctor command parse and headless setup preflight
  regressions.
- `cargo test -p ploke-eval headless_tui -- --nocapture` -> passed 7 tests, 2
  ignored live tests, including
  `rag_unavailable_headless_tui_setup_writes_typed_diagnostics`.
- `cargo build -p ploke-eval` -> passed.

Fresh campaign setup-preflight evidence from `p1-admissionfix-g35flash-p25flash-20260606-140827` on 2026-06-06T14:30:46-07:00:

```text
headless_tui_setup_preflight.outcome = passed
doctor.phase = baseline_eval
blocker_count = 0
```

Root cause for the live setup blocker was the typed-graph relation presence probe
used during `RagService::new_full(...)`: it queried Cozo with invalid syntax
(`?[present] := *type_contains, present = true :limit 1`). In the eval/headless
harness this `RagError::Db(Cozo(...))` was converted to `state.rag = None`, so
`wait_for_bm25_ready` reported only `RAG service is unavailable`. The DB probe now
uses valid relation-map syntax with `*type_contains { parent_type_id,
child_type_id @ 'NOW' } :limit 1`, returning `false` for registered-but-empty
relations and allowing type-context degradation instead of RAG construction
failure.

## Broken Contract

Every published broad headless-TUI slot must have one of these first-class, child-plan-joinable outcomes:

1. a submitted edit result;
2. a terminal `.headless-tui.json` diagnostic with enough redacted evidence to classify the terminal; or
3. a minimal launch/fallback/no-start record keyed to the request id/hash and workspace, including attempted entrypoint/argv, start time, PID/task id when available, end/exit/timeout/error class when available, and redacted provider route.

A child-plan rejection that only says the expected diagnostic file is missing is insufficient. It records the negative admission outcome but not the lifecycle cause.

## Evidence

Campaign:

```text
p1-admissionfix-g35flash-p25flash-20260606-090815
```

Primary source reports:

```text
docs/active/agents/run-reviews/2026-06-06-p1-admissionfix-g35flash-p25flash-20260606-090815-parent-request-only-rca.md
docs/active/agents/run-reviews/2026-06-06-p1-admissionfix-g35flash-p25flash-20260606-090815-slots-base-r5-incomplete.md
docs/active/agents/run-reviews/2026-06-06-p1-admissionfix-g35flash-p25flash-20260606-090815-slots-r6-r10-incomplete.md
docs/active/agents/run-reviews/2026-06-06-p1-admissionfix-g35flash-p25flash-20260606-090815-coverage-status.md
```

Artifact roots:

```text
/home/brasides/.ploke-eval/campaigns/p1-admissionfix-g35flash-p25flash-20260606-090815/prototype1/messages/edit-harness-request/
/home/brasides/.ploke-eval/campaigns/p1-admissionfix-g35flash-p25flash-20260606-090815/prototype1/messages/edit-harness-result/
/home/brasides/.ploke-eval/campaigns/p1-admissionfix-g35flash-p25flash-20260606-090815/prototype1/messages/child-plan/node-0cdf3741b09283fe.json
/home/brasides/.ploke-eval/campaigns/p1-admissionfix-g35flash-p25flash-20260606-090815/prototype1/workspaces/edit-harness/
/home/brasides/.ploke-eval/campaigns/p1-admissionfix-g35flash-p25flash-20260606-090815/prototype1/nodes/node-0cdf3741b09283fe/runner-request.json
```

Verified state from the 2026-06-06 synthesis pass:

```text
request_json_count = 10
edit_harness_result_dir_exists = false
child_plan_children = 0
child_plan_rejected = 10
node_status = failed
runner_request_exists = true
```

The request/workspace/child-plan trace is:

```text
transition-journal parent_started for node-0cdf3741b09283fe
-> parent runner-request records `loop prototype1-state --repo-root ...090815`
-> ten request JSON/Markdown pairs are published under messages/edit-harness-request
-> ten candidate workspaces exist at target commit 2bcf9ead0e2ef53db740471c40056ec76b8926cf
-> messages/edit-harness-result/ is absent
-> child-plan records children=[] and ten rejected_surface_attempts naming missing `.headless-tui.json` paths
-> node.json status is failed
```

Process log correlation:

```text
/home/brasides/.ploke-eval/logs/ploke_eval_20260606_091957_2593254.log
```

The log contains ten `broad headless-tui slot attempt did not produce an admissible edit` warnings. Each warning reports `admitted=0`, `required_min=2`, `configured_max=5`, and the same startup error:

```text
batch selection is invalid: broad headless-tui attempt failed: failed to start headless ploke-tui harness: database setup failed during 'bm25_ready': RAG service is unavailable
```

Timing shape from those warnings:

- first wave ended at 2026-06-06T09:32:59-07:00 for base/r2/r3/r4/r5;
- second wave ended at 2026-06-06T09:35:04-07:00 for r6/r7/r8/r9/r10;
- this matches an effective broad parallel cap of five slots per wave.

## Source Trace

The relevant broad-slot path is in `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs`:

```text
run_broad_headless_tui_attempt
-> run_broad_headless_tui_attempt_with_options
-> prepare_broad_harness_workspace
-> read prompt and construct budget
-> tui_adapter::run_headless_with_model_capture_responses
-> require terminal outcome
-> write_broad_headless_tui_diagnostics
-> write_broad_headless_tui_turn_live_bundle
-> finish_broad_headless_tui_attempt
```

The current source writes the normal diagnostics only after `run_headless_with_model_capture_responses` returns a run with a terminal outcome (`cli_facing.rs:1610-1630`). If the attempt exits before that boundary, the campaign can reach child-plan rejection with no per-slot launch/terminal evidence. The child-plan rejection path (`cli_facing.rs:2412-2422`) then reports only that the expected diagnostics file is absent.

The 090815 log points at this earlier setup boundary:

```text
crates/ploke-eval/src/runner/mod.rs::wait_for_bm25_ready
```

`wait_for_bm25_ready` fails immediately when `state.rag` is absent:

```text
phase = "bm25_ready"
detail = "RAG service is unavailable"
```

The eval/headless test runtime currently constructs RAG as optional in:

```text
crates/ploke-tui/src/app/commands/unit_tests/harness.rs::new_with_embedding_processor_and_rag_config
```

There, `RagService::new_full(...)` errors are converted to `None` rather than propagated. In a path where headless sparse/BM25 retrieval is mandatory, this turns a deterministic setup failure into ten broad-slot attempts that all fail later at `bm25_ready`.

## Not Covered By Adjacent Bugs

- `2026-05-25-prototype1-child-plan-zero-admission-timeout.md` covers the controller contract that below-minimum admission must persist rejected child-plan evidence. In 090815, that fixed behavior is working: a rejected-attempt-only child-plan exists.
- `2026-05-25-prototype1-broad-headless-google-401-slot-thrash.md` covers typed provider-unavailable handling when diagnostics exist. In 090815, no diagnostic or raw provider sidecar exists, so provider failure is not proven.
- `2026-06-04-prototype1-headless-tui-runtime-actor-leak.md` covers runtime actor retention/OOM when attempts produce result sidecars or runtime evidence. In 090815, no OOM/process/sidecar evidence was found.

## Expected Behavior

- Persist a pre-launch record before invoking each broad headless attempt.
- Persist a terminal or fallback record on every early-exit class: workspace prep failure, prompt read failure, budget construction failure, adapter error, no-terminal outcome, task join failure, timeout, panic/kill/interruption, and provider/environment blocker when observable.
- Treat mandatory RAG/BM25 setup absence as a typed preflight/setup failure. Do not silently convert `RagService::new_full(...)` failure into `state.rag = None` for eval/headless paths that require sparse retrieval.
- Include that record path/status in child-plan `rejected_surface_attempts`.
- Redact endpoint auth, provider project ids, credential values, and credential paths in any persisted diagnostic.

## Regression / Validation Direction

- Add a fixture or test hook that forces `run_headless_with_model_capture_responses` to error before returning a terminal run and assert that a minimal per-slot fallback record is written and joined into child-plan rejection.
- Add a fixture that forces workspace-prep or prompt-read failure and asserts the same fallback contract.
- Add a setup-level regression that forces `RagService::new_full(...)` failure or `state.rag = None` in the broad/headless runtime and asserts a typed `bm25_ready`/RAG-unavailable diagnostic instead of a missing-file-only child-plan rejection.
- Re-run a broad-harness campaign after the fix and require every published slot to have either submitted result, `.headless-tui.json`, or fallback/no-start record before the parent can be reviewed as diagnosable.

## Rerun Gate

A fresh campaign is needed after per-slot diagnostics are implemented to determine whether broad headless-TUI attempts actually launch and terminate with typed evidence. Before spending the next live step, run:

```bash
./target/debug/ploke-eval loop prototype1-doctor --repo-root . --headless-tui-setup-preflight --format json
```

If it reports `headless_tui_setup_preflight.outcome = failed`, stop and fix that setup blocker before publishing broad slots.
