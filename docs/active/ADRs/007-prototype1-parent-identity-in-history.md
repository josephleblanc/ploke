# ADR 007: Store Prototype 1 Parent Identity in History, Not Checkout Files

## Status
Accepted (2026-06-15; implementation pending)

## Context

Prototype 1 currently writes `.ploke/prototype1/parent_identity.json` into each parent-capable checkout. The file carries the logical parent coordinate: campaign, parent/node id, generation, branch/artifact branch, optional predecessor links, and a benchmark instance id.

That file is doing two jobs:

1. Runtime admission input: `prototype1-state` reads it to infer campaign/parent identity before History startup validation.
2. Artifact-carried evidence/projection: the same identity is committed into the checkout tree and therefore affects artifact hashes/surfaces.

This splits authority across two surfaces. Successor History blocks already seal the selected successor parent identity, while the successor checkout also contains a JSON file claiming the same identity. Generation 0 is the main reason the file still exists: current setup writes the file before any History head exists, and the first live successor handoff later creates the first sealed block.

As Prototype 1 moves toward multi-instance and multi-benchmark evaluation, parent identity should also carry a cohort of benchmark targets instead of one `instance_id`. That scope is policy/evaluation admission material, not source checkout content.

## Decision

Keep the `ParentIdentity` concept/data type, but remove `.ploke/prototype1/parent_identity.json` as an authoritative checkout file.

Parent identity shall be admitted in sealed History blocks:

- `prototype1-setup` creates/seals/appends a genesis History block that admits the generation-0 parent identity.
- `prototype1-state` starts by reading configured History, deriving the active parent identity from the sealed head, and validating the active checkout artifact/tree/surface against that admission.
- Successor handoff blocks continue to admit the selected child as the next parent identity.
- The first real successor handoff after setup therefore appends a successor block after the setup-created genesis block, instead of creating genesis itself.

The parent target scope changes from a single optional `instance_id` to a benchmark-family map:

```json
"benchmark_instances": {
  "multi_swe_bench_rust": [
    "BurntSushi__ripgrep-2209"
  ]
}
```

Use a deterministic serialized order for the map and instance lists when hashing/sealing History material.

## Consequences

### Positive

- History becomes the single authority surface for parent admission.
- Artifact trees no longer change merely to carry control metadata.
- Genesis is represented as History bootstrap authority instead of a checkout file plus absence check.
- Successor startup no longer has to reconcile two full copies of parent identity.
- Multi-instance and multi-benchmark target scope becomes admitted policy/evaluation material.

### Negative

- This is a breaking change for existing Prototype 1 campaigns and sealed block schemas.
- Setup, startup, git checkout validation, history preview, graph ingestion, and tests all need updates.
- Operators lose the convenience of opening `.ploke/prototype1/parent_identity.json` to identify a checkout unless a non-authoritative projection or CLI display replaces it.

### Neutral

- `ParentIdentity` remains a real record/type; only its storage authority moves.
- A small checkout marker/projection may be added later for operator ergonomics, but it must not be an admission input and must not be trusted over History.
- Existing legacy artifacts may require explicit migration/regeneration. Do not silently fall back to `parent_identity.json` as authority.

## Implementation Notes

Likely code areas:

- `crates/ploke-eval/src/cli/prototype1_state/identity.rs`
- `crates/ploke-eval/src/cli/prototype1_state/parent.rs`
- `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs`
- `crates/ploke-eval/src/cli/prototype1_process.rs`
- `crates/ploke-eval/src/cli/prototype1_state/backend/git_worktree.rs`
- `crates/ploke-eval/src/cli/prototype1_state/history/seal/mod.rs`
- `crates/ploke-records/src/identity.rs`
- `crates/ploke-tree` History/identity ingestion paths

The sealed block field currently named `selected_parent_identity` should be reviewed. Under this ADR, the semantic role is broader: the sealed head admits the parent identity that may start from that head. For genesis, that is parent0; for successor handoff, that is the selected child/next parent.
