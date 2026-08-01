# 2026-05-11 MBE Oracle Calibration Plan

Implementation plan for fixing Prototype 1 Multi-SWE-bench child self-eval
patch provenance.

## Summary

Prototype 1 child self-eval must not use the shared benchmark repo cache as the
mutable MBE target checkout. Each child gets a child-owned MBE instance target
cache under its node directory, outside the child Artifact worktree. MBE
`fix_patch` export records the exact checkout cwd and a typed patch-projection
check, and child success evidence must not treat a nonempty submission as
oracle-positive unless that check passed.

The v1 priority is correctness over disk use. Disk optimization is deferred,
but cleanup is required.

## Runtime Changes

- Create the per-child MBE repo cache at:

```text
nodes/<node-id>/instance-targets/<treatment-campaign-id>/<org>/<repo>
```

- Populate that cache from the shared benchmark repo cache using local git
  clones with independent working trees.
- Pass this child-owned repo cache into eval closure preparation so
  `PreparedSingleRun.repo_root` resolves under the child node directory.
- Keep this path separate from:
  - the child Artifact worktree at `nodes/<node-id>/worktree`
  - the child build target at `nodes/<node-id>/target`
  - the shared benchmark repo cache under `~/.ploke-eval/repos`
- Extend child cleanup to remove `nodes/<node-id>/instance-targets/**` with the
  same node-child path guard used for build-product cleanup.

## Provenance And Gate

- Write `benchmark-patch-projection.json` beside each MBE submission artifact.
- Persist the passive typed record through `ploke-records`, including:
  - benchmark identity
  - run root, run manifest, and compressed run record path
  - checkout cwd and head SHA
  - submission path, SHA-256, byte count, line count, and diff base
  - patch-projection check state
- Store coarse projection state on the run packaging phase so operational
  metrics can derive oracle eligibility without ad hoc JSON reads.
- Require `PatchProjectionCheckState::Passed` before nonempty MBE submissions
  count as oracle-eligible.
- In the child execution path, fail the treatment evidence step if a nonempty
  submission lacks a passed projection check.

## Tests And Acceptance

- Runner packaging writes both:
  - `multi-swe-bench-submission.jsonl`
  - `benchmark-patch-projection.json`
- `OperationalRunMetrics::oracle_eligible` is false for a nonempty submission
  when the projection check is missing or failed.
- The passive projection DTO roundtrips and rejects unknown fields.
- Child cleanup removes `instance-targets` and never treats it as an Artifact
  worktree.
- MBE campaign candidate loading skips diagnostic-only submissions when
  `nonempty_only` is requested.

## Linked Evidence

- Active bug:
  [`../bugs/2026-05-11-prototype1-mbe-shared-instance-patch-provenance.md`](../bugs/2026-05-11-prototype1-mbe-shared-instance-patch-provenance.md)
- Provenance summary:
  [`2026-05-11_mbe-instance-patch-provenance/summary.md`](2026-05-11_mbe-instance-patch-provenance/summary.md)
- Synthesis:
  [`2026-05-11_mbe-instance-patch-provenance/evidence/synthesis.md`](2026-05-11_mbe-instance-patch-provenance/evidence/synthesis.md)
