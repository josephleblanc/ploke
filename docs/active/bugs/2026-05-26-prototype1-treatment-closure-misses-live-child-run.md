# 2026-05-26 Prototype 1 Treatment Closure Misses Live Child Run

Status: open; loop blocker for the focused live child self-eval proof.

## Symptom

The focused live test for `execute_prototype1_runner_invocation` produced a
valid child patch and run artifacts, but the treatment closure recompute still
classified the single treatment instance as `missing`.

The stricter completeness gate now fails the child runner instead of allowing a
false success:

```text
Prototype1RunnerResult {
  status: Failed,
  disposition: TreatmentFailed,
  detail: Some("batch selection is invalid: treatment 'live-child-runner-success-treatment-branch-live-child-1779784648646' instance 'ploke-live__child-target-1' did not produce complete run metrics (status=missing)")
}
```

## Evidence

Command:

```text
RUSTFLAGS=-Awarnings cargo test -q -p ploke-eval --features live_api_tests live_google_child_runner_success -- --ignored --nocapture
```

Preserved eval home:

```text
target/tmp/live-api-tests/prototype1-child-runner-success-EEaL2s/eval-home
```

Treatment closure state:

```text
campaigns/live-child-runner-success-treatment-branch-live-child-1779784648646/closure-state.json
```

Key closure summary:

```text
expected_total=1
complete_total=0
missing_total=1
status=missing
```

The run artifacts do exist:

```text
instances/live-child-runner-success/treatments/branch-live-child/instances/ploke-live__child-target-1/runs/run-1779784649538-structured-current-policy-7fcdf78c/
```

That directory contains, among other files:

```text
agent-turn-summary.json
agent-turn-trace.json
benchmark-patch-projection.json
execution-log.json
record.json.gz
multi-swe-bench-submission.jsonl
validation-audit.json
```

The model/tool run appears semantically successful:

- `agent-turn-summary.json` reports terminal outcome `completed`;
- `patch_artifact.applied=true`;
- `all_proposals_applied=true`;
- `expected_file_changes[0].path="src/lib.rs"`;
- the submission and repo diff show `answer()` changed from `"wrong"` to
  `"fixed"`.

The root cause is a lexical path mismatch in closure registration selection,
not missing registration content.

The registration exists:

```text
registries/runs/run-1779784649538-structured-current-policy-7fcdf78c.json
```

Its `intent` / `frozen_spec` fields are populated and its lifecycle is
`completed`. The relevant values include:

```text
frozen_spec.task_id=ploke-live__child-target-1
frozen_spec.storage_roots.runs_dir=/home/brasides/code/ploke/target/tmp/live-api-tests/prototype1-child-runner-success-EEaL2s/eval-home/instances/live-child-runner-success/treatments/branch-live-child/instances/ploke-live__child-target-1/runs
artifacts.record_path=/home/brasides/code/ploke/target/tmp/live-api-tests/prototype1-child-runner-success-EEaL2s/eval-home/instances/live-child-runner-success/treatments/branch-live-child/instances/ploke-live__child-target-1/runs/run-1779784649538-structured-current-policy-7fcdf78c/record.json.gz
```

Closure computes its expected run directory from the treatment campaign
`instances_root`:

```text
/home/brasides/code/ploke/crates/ploke-eval/../../target/tmp/live-api-tests/prototype1-child-runner-success-EEaL2s/eval-home/instances/live-child-runner-success/treatments/branch-live-child/instances/ploke-live__child-target-1/runs
```

Those two paths name the same directory after normalization, but they are not
lexically equal as `PathBuf`s. `list_registrations_for_instance` currently
requires exact `PathBuf` equality:

```rust
if registration.frozen_spec.storage_roots.runs_dir != expected_runs_dir {
    continue;
}
```

That rejects the completed registration. The resulting closure row has:

```text
registration_path=null
record_path=null
run_root=<instance_root>
eval_status=missing
```

The code then falls back to checking `<instance_root>/record.json.gz`, not the
actual `<instance_root>/runs/<run-id>/record.json.gz`, so metrics are not read.

## Broken Contract

A child runner must not report successful treatment evidence unless the
treatment closure state can classify the corresponding instance run as
complete. The run registry, per-instance run manifest, run artifacts, and
treatment `closure-state.json` must agree enough for
`build_prototype1_treatment_evidence` to produce metrics.

The live run shows that the patch-producing agent path can succeed while the
closure view fails to find the run because one side stored a path with
`crates/ploke-eval/../../target/...` and the other side stored the normalized
`target/...` spelling.

This is another instance of a path-authority bug. Related prior reports:

- `docs/active/bugs/2026-03-21-indexworkspace-relative-target-regression.md`;
- `docs/active/bugs/2026-05-11-prototype1-mbe-shared-instance-patch-provenance.md`.

## Current Guard

`run_prototype1_resolved_branch_treatment` now checks treatment completeness
after `build_prototype1_treatment_evidence`. If any treatment instance lacks
metrics, it returns `TreatmentFailed` instead of sending a terminal successful
child result with incomplete evidence.

Focused local guard:

```text
RUSTFLAGS=-Awarnings cargo test -q -p ploke-eval child_success_requires_complete_treatment_metrics -- --nocapture
```

## Next Fix

Fix registration selection so it does not reject equivalent paths solely
because one side contains `..` components.

Relevant code:

- `crates/ploke-eval/src/run_registry.rs::list_registrations_for_instance`;
- `crates/ploke-eval/src/closure.rs::build_instance_row`;
- `crates/ploke-eval/src/runner.rs::register_run_attempt`;
- `crates/ploke-eval/src/inner/core.rs::RunStorageRoots`.

The fix should normalize or canonicalize the stored and expected roots at the
authority boundary, then add a regression that constructs the two path spellings
shown above and proves closure selects the completed registration. After that,
rerun the same live test and require the treatment closure to report one
complete instance before the child runner can succeed.
