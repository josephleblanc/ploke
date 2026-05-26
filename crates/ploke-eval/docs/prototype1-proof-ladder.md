# Prototype 1 Proof Ladder

Status: active proof index for making Prototype 1 credible before trusting long
live runs.

This document records the focused checks that prove each loop rung. Prefer
updating this file when a new rung is proven, instead of relying on terminal
history or chat memory.

## Rule

Do not use a long live loop run as the first proof of a stage. Prove the stage
with the smallest faithful local, historical, or `live_api_tests`-gated live
test that exercises the same authority-bearing path.

## Current Rungs

| Rung | Status | Main proof | What it proves |
| --- | --- | --- | --- |
| Child-plan request publication and zero-admission persistence | Proven | `step_persists_zero_admission_plan`, `zero_admission_batch_is_persisted` | A parent that spends a broad-harness batch still persists a durable `ChildPlan` message, even when no child is admitted. Retry must recover from the message box instead of minting fresh slots from projections. |
| Parent patch-generation slot concurrency | Proven | `live_google_parallel_slots` | `prototype1-step` can run two live Google broad-harness patch-generation slots concurrently, bounded by `search.children.parallel_targets`, and persist exactly one outcome per published request. |
| Child runner terminal failure evidence | Proven | `child_runner_failure_records_terminal_channel` | `execute_prototype1_runner_invocation` drives the real child runner through materialization failure, writes attempt/latest runner results, updates node projection, and sends channel `Ready`, `Evaluating`, terminal `Result { treatment: None }`. |
| Historical successful treatment evidence rebuild | Proven | `historical_node_150_channel_treatment_reaches_current_generation_handoff` | The real node-150 treatment `closure-state.json` plus run `record.json.gz` rebuilds treatment evidence matching the terminal child channel payload. |
| Live child self-eval success | Blocked by registration/closure gap | `live_google_child_runner_success` | The focused live child runner can produce a valid patch and run artifacts, but stricter treatment-completeness gating exposed that closure recompute still marks the treatment instance `missing`. |

## Latest Live Child Success Attempt

Command:

```bash
RUSTFLAGS=-Awarnings cargo test -q -p ploke-eval --features live_api_tests live_google_child_runner_success -- --ignored --nocapture
```

Initial observed result on 2026-05-26, before the treatment-completeness gate
was tightened:

```text
[prototype1-child-live] phase=google_auth_checked delta_ms=232 total_ms=232
[prototype1-child-live] phase=model_config_written delta_ms=0 total_ms=232
[prototype1-child-live] phase=campaign_written delta_ms=6 total_ms=239
[prototype1-child-live] phase=invocation_written delta_ms=0 total_ms=239
loop.prototype1_branch.evaluate.branch-live-child.start
loop.prototype1_branch.evaluate.branch-live-child.end +111.355s
[prototype1-child-live] phase=runner_returned delta_ms=111355 total_ms=111595
[prototype1-child-live] phase=assertions_complete delta_ms=0 total_ms=111595
test result: ok. 1 passed; finished in 111.60s
```

That initial pass was too weak. It trusted the returned runner result before
proving that the treatment closure state could see a complete run. After adding
the production completeness gate, the same live rung exposed the current
blocker:

```text
Prototype1RunnerResult {
  status: Failed,
  disposition: TreatmentFailed,
  detail: Some("batch selection is invalid: treatment 'live-child-runner-success-treatment-branch-live-child-1779784648646' instance 'ploke-live__child-target-1' did not produce complete run metrics (status=missing)")
}
```

Preserved evidence root:

```text
target/tmp/live-api-tests/prototype1-child-runner-success-EEaL2s/eval-home
```

Important facts from that root:

- treatment `closure-state.json` reports `expected_total=1`,
  `complete_total=0`, and `missing_total=1`;
- run artifacts exist under the treatment instance run root, including
  `record.json.gz`, `agent-turn-summary.json`,
  `benchmark-patch-projection.json`, and
  `multi-swe-bench-submission.jsonl`;
- `agent-turn-summary.json` reports terminal outcome `completed`,
  `patch_artifact.applied=true`, and `all_proposals_applied=true`;
- the submission and repo diff show the intended edit from `"wrong"` to
  `"fixed"`;
- the completed run registration exists, but closure rejects it because
  `list_registrations_for_instance` compares two equivalent roots lexically:
  `.../crates/ploke-eval/../../target/.../runs` versus
  `.../target/.../runs`;
- after rejecting the registration, closure falls back to the instance root and
  never sees the actual `runs/<run-id>/record.json.gz`.

This proves the child model/tool run can produce the intended patch, but does
not yet prove a successful child self-eval rung. The next fix should normalize
the registration-selection path comparison, then rerun the same live test.

The test constructs a local Multi-SWE-Bench-style source repo, writes a
one-instance dataset, admits a direct Google model registry row, bootstraps a
real `ChildInvocation`, and calls `execute_prototype1_runner_invocation`.

The assertions are intentionally channel-authority-oriented:

- returned runner result is `Succeeded` only after treatment closure reports
  complete metrics;
- treatment campaign id is present;
- treatment closure state has one complete instance;
- child-to-parent channel messages are `Ready`, `Evaluating`, and terminal
  `Result`;
- terminal `Result` carries `treatment: Some(...)`;
- treatment evidence points at an existing `record.json.gz`;
- treatment metrics report a non-empty valid patch;
- child eval target checkout contains the intended `"fixed"` content.

## Remaining Rungs

These are not proven enough to trust a long live run:

- parent `observe_child` over a fresh live successful child result;
- compare/selection over live treatment evidence;
- successor/Crown/History handoff after a live selected child;
- cleanup and disk-pressure behavior across a multi-child generation.

For these, add focused proofs before restarting a long live loop.
