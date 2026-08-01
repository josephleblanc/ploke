# MBE Instance Patch Provenance Summary

Date: 2026-05-11

## Scope

This note summarizes the first provenance pass over how a Prototype 1 child
self-validation run produces the `multi-swe-bench-submission.jsonl` instance
patch later evaluated by Multi-SWE-bench.

Terms:

- candidate: child runtime/artifact that may become next parent/ruler
- instance target: benchmark repo used for the task, such as `BurntSushi/ripgrep`
- instance patch: patch against the instance target; this is what MBE validates
- parent-generated patch: patch that modifies an Artifact to produce a candidate

## Reports

- `evaluation-oracle-targets.md`
- `edit-surface-patch-evidence.md`
- `tool-calls-results.md`
- `run-record-locator.md`

## Provenance Chain

1. Campaign/node lookup starts from typed Prototype 1 projections:
   `Prototype1NodeRecord` and `Prototype1RunnerResult`.
2. `Prototype1RunnerResult.evaluation_artifact_path` points to the typed branch
   evaluation report.
3. The branch evaluation report contains
   `compared_instances[*].treatment_record_path`.
4. The treatment record resolves through `RunRegistration` to the run root and
   artifact paths, including:
   - `agent-turn-trace.json`
   - `agent-turn-summary.json`
   - `record.json.gz`
   - `multi-swe-bench-submission.jsonl`
5. The child self-validation runner snapshots proposal state into
   `PatchArtifact`.
6. The MBE submission writer does not serialize proposal text as the patch.
   It calls `git diff --no-ext-diff --binary <base_sha> --` in the instance
   target checkout and writes that diff as `fix_patch`.

## Current Understanding

The MBE instance patch is a benchmark-base diff measured from the current
mutable instance target checkout at packaging time.

That means the patch bytes are not sourced directly from:

- tool-call result text
- proposal preview text
- History surface evidence
- parent-generated patch data

Those records explain how the workspace reached its state. The submitted MBE
patch is still the checkout diff at the moment packaging runs.

## First Concrete Examples

Campaign:

```text
p1-history-traversal-20260511-2
```

Example 1:

```text
node-e73f3c82afd7f4c3
generation 2
fix_patch_bytes = 4444
fix_patch_lines = 124
PatchArtifact.applied = false
PatchArtifact.all_proposals_applied = false
edit proposal statuses = ["Failed"]
expected file changed = crates/printer/src/util.rs
```

Patch headers/symbols include a change to `crates/printer/src/util.rs` and a
call to `crate::util::replace_all_clipped`, but no matching helper definition
was observed in the bounded patch header/symbol scan.

Example 2:

```text
node-fc481e444bb4327b
generation 2
fix_patch_bytes = 681
fix_patch_lines = 18
PatchArtifact.applied = false
PatchArtifact.all_proposals_applied = false
edit proposal statuses = []
expected file changed = crates/printer/src/util.rs
```

This exported a non-empty MBE patch even though the patch artifact has no
proposal statuses and says no proposal was applied.

Example 3:

```text
node-28b9040eae0e1e82
generation 4
fix_patch_bytes = 9067
fix_patch_lines = 218
PatchArtifact.applied = false
PatchArtifact.all_proposals_applied = false
edit proposal statuses = []
expected file changed = crates/printer/src/util.rs
```

Patch headers/symbols include changes to both:

```text
crates/printer/src/standard.rs
crates/printer/src/util.rs
```

This again exported a non-empty MBE patch while the patch artifact says no
proposal was applied.

## Strong Suspicion

The immediate evidence points more toward harness/projection failure than pure
model failure:

1. `PatchArtifact.applied=false` can coexist with a non-empty exported MBE
   `fix_patch`.
2. At least two sampled runs have non-empty exported patches while the proposal
   snapshot has no edit proposal status entries.
3. The export source is the mutable instance target checkout, not an accepted
   proposal artifact.
4. The current MBE prep path uses a shared repo cache path for the instance
   target. Concurrent child self-validation can therefore interfere unless each
   run receives an isolated checkout/worktree.

## Main Code Ranges

- `crates/ploke-eval/src/runner.rs:929-1068`
  - `PatchArtifact` snapshot and MBE `fix_patch` collection.
- `crates/ploke-eval/src/runner.rs:2448-2486`
  - packaging writes `multi-swe-bench-submission.jsonl`.
- `crates/ploke-eval/src/msb.rs:138-160`
  - prepared run currently derives `repo_root` from the shared repo cache path.
- `crates/ploke-eval/src/runner.rs:2128-2150`
  - runner checks out `prepared.repo_root` to base before the turn.
- `crates/ploke-eval/src/runner.rs:3208-3236`
  - checkout uses `git reset --hard` and detached checkout.
- `crates/ploke-tui/src/rag/tools.rs:250-323`
  - tool staging reports pending/staged and spawns async approval when enabled.
- `crates/ploke-tui/src/rag/editing.rs:180-203`
  - partial non-semantic apply can mark proposal `Failed` after some writes.

## Next Checks

1. Confirm whether child self-validation runs for the same instance target share
   one mutable checkout during concurrent fanout.
2. Add or sketch a typed provenance helper that reports, for one node:
   candidate id, treatment run root, instance target base SHA, proposal statuses,
   expected file changes, submission patch path, patch byte count, and MBE
   verdict.
3. Gate oracle evidence so a non-empty `fix_patch` is not marked usable when
   the patch artifact shows failed, pending, absent, or partial proposal state.
4. Move instance target execution to an isolated checkout/worktree per candidate
   self-validation run.

