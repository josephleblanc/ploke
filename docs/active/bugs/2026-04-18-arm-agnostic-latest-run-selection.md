# Latest-Run Selection Is Still Arm-Agnostic On Read Paths

## Summary

`ploke-eval` now writes run artifacts into nested per-run directories, which
fixes the direct write-side collision between setup-only and treatment runs.
But read-side resolution still picks the latest run directory for an instance
without considering run arm. That means a newer setup-only/control run can hide
an older treatment run's record or submission artifact.

## Code references

The current selector only returns the most recently modified candidate run dir:

- [crates/ploke-eval/src/run_history.rs](/home/brasides/code/ploke/crates/ploke-eval/src/run_history.rs:99)
  `latest_run_dir_for_instance_root()`
- [crates/ploke-eval/src/run_history.rs](/home/brasides/code/ploke/crates/ploke-eval/src/run_history.rs:116)
  `latest_run_dir_for_instance()`

That selector is then reused by several read paths:

- campaign submission export:
  [crates/ploke-eval/src/cli.rs](/home/brasides/code/ploke/crates/ploke-eval/src/cli.rs:2986)
- `--instance` record resolution:
  [crates/ploke-eval/src/cli.rs](/home/brasides/code/ploke/crates/ploke-eval/src/cli.rs:7834)
- closure row construction:
  [crates/ploke-eval/src/closure.rs](/home/brasides/code/ploke/crates/ploke-eval/src/closure.rs:567)

## Why this is still a bug after nested run dirs

The earlier fix changed the write layout:

- each run now has its own nested run dir
- setup-only runs no longer emit new submission files

But the selector still asks only:

- which run dir is newest?

It does not ask:

- is this the treatment run?
- does this run arm even produce a submission artifact?
- does this caller want the latest record or the latest treatment submission?

So write isolation improved, but read attribution is still lossy.

## Impact

### `--instance`

Commands that resolve `--instance <id>` through the latest run dir can read the
wrong record when both control and treatment runs exist under the same instance
root.

### Closure

Closure can classify an instance from the newest run regardless of whether that
run is the relevant arm for the surface being summarized.

### Campaign export

Campaign submission export can select the newest run dir, miss the older
treatment submission, and either:

- report an empty/missing patch when a treatment patch exists, or
- attach the wrong run's context to the instance.

## Expected vs actual behavior

Expected:

- read paths that care about benchmark patch outcome should prefer the newest
  treatment/agent run with the relevant artifact
- generic record inspection should be explicit about which arm it selected

Actual:

- latest-run resolution is arm-agnostic and artifact-agnostic

## Relationship to the earlier patch-artifact bug

This is the remaining read-side half of the integrity problem tracked in:

- [2026-04-18-eval-patch-artifact-collision-and-empty-diff.md](./2026-04-18-eval-patch-artifact-collision-and-empty-diff.md)

The nested-run-dir change fixed the direct overwrite path. This bug remains
because selection still collapses distinct run arms back into one “latest”
answer on read.
