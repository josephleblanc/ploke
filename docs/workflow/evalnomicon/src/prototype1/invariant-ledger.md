# Prototype 1 Invariant Ledger

This ledger records architecture claims that should remain stable across documentation passes. Each entry should eventually include a status, code anchor, caveat, and verification/audit note.

## Status labels

- **Implemented:** enforced by current code.
- **Partially implemented:** current code enforces part of the claim but has named gaps.
- **Intended:** target architecture not yet enforced.
- **Not claimed:** a tempting stronger interpretation that Prototype 1 explicitly does not rely on.

## Ledger

### 1. History is authority; scheduler/report/registry are projections

**Status:** partially implemented / current local claim.

History is the durable authority surface over sealed lineage-local blocks. Scheduler snapshots, branch registries, CLI reports, previews, dashboards, and side tables are projections or evidence sources. They do not become History authority by being read.

**Primary anchors:**

- `crates/ploke-eval/src/cli/prototype1_state/mod.rs`
- `crates/ploke-eval/src/cli/prototype1_state/history.rs`

**Caveat:** Current History is local, lineage-scoped, and tamper-evident. It is not distributed consensus or process uniqueness.

### 2. Crown is lineage authority, not pid/path/branch

**Status:** current architecture claim.

The Crown is the capability to mutate one active lineage. It is not a process id, git branch, filesystem path, or global singleton.

**Caveat:** Future multi-parent work must make the lineage coordinate explicit so sibling parents do not share one Crown by accident.

### 3. At most one valid `Crown<Ruling>` per lineage

**Status:** partially implemented / target invariant.

For one lineage, at most one valid typestate carrier may hold `Crown<Ruling>`. During handoff there may be zero rulers.

**Caveat:** Multiple runtimes may execute around handoff. Execution is not the same as Crown authority.

### 4. Successor handoff is a cross-runtime typed contract

**Status:** partially implemented.

The outgoing parent locks succession and seals handoff material. The incoming runtime verifies the active checkout and sealed predecessor evidence before it may become the next ruling parent.

**Caveat:** The same in-memory Crown object does not cross the process boundary.

### 5. Startup admission is not just “binary ran”

**Status:** intended / partially implemented for successor handoff.

Target admission shape:

```text
ProducedBy(SelfRuntime, CurrentArtifact)
AdmittedBy(CurrentArtifact, Lineage, Policy, History)
```

**Caveat:** Bootstrap/genesis admission is currently a local configured-store absence claim; it is not a global absence proof.

### 6. Policy-bearing surface is protected

**Status:** current Prototype 1 policy.

The policy-bearing `ploke-eval` surface defines parent creation, child/successor execution, History admission, Crown transitions, and handoff. Ordinary self-improvement must not mutate this surface until an explicit protocol-upgrade transition exists.

**Caveat:** External processes can still compile incompatible code; such processes are outside the admitted transition system.

### 7. Artifact surface is partitioned

**Status:** partially implemented.

Current model:

```text
ArtifactSurface = Immutable + Mutated + Ambient
```

Current concrete partition:

- Immutable: `crates/ploke-eval`
- Mutated: tool-description text files
- Ambient: empty declared surface

**Caveat:** The partition is a current Prototype 1 policy, not the final general edit model.

### 8. Artifact identity is not worktree path

**Status:** current architecture claim.

Branch names and worktree paths are handles, not semantic identity. Durable records that identify an artifact should carry explicit artifact identity or backend recovery identity.

**Caveat:** Text-file fallback ids are surface identities, not whole-worktree artifact identities.

### 9. Dirty worktrees are provisional candidates

**Status:** current architecture claim.

A dirty worktree should not be treated as a durable graph node until it has a recoverable identity such as a git commit, tree id, content hash, or artifact manifest id.

**Caveat:** A runtime built from an uncommitted worktree may run, but its source provenance is degraded if the source artifact is later lost.

### 10. Runtime graph is richer than git ancestry

**Status:** intended / partially wired through loop graph records.

Prototype 1 needs at least three related graph views:

- artifact graph
- runtime derivation graph
- operation graph

The important operation coordinate is generator runtime plus target artifact.

**Caveat:** Most live CLI synthesis paths still have incomplete graph provenance and fallback ids.

### 11. Child self-report is evidence, not promotion

**Status:** current architecture claim.

A child runtime may self-evaluate and record evidence. It does not self-promote. Successor selection and admission require parent-side policy, History/Crown checks, and admissible evidence.

**Caveat:** LLM adjudication can be useful evidence, but must not become circular authority.

### 12. Evaluation records must name evaluator and policy

**Status:** current architecture constraint.

Scores or evaluation records must name the evaluated runtime/artifact, the oracle or eval set, and the policy used to interpret the result. A score without evaluator and policy identity is not comparable evidence.

### 13. Messages are typed cross-runtime obligations

**Status:** partially implemented.

A message is not merely a persisted JSON blob. A box is:

```text
Box = (Lock transition, Unlock transition, File schema)
```

Each mutable buffer should name its owner, allowed readers, and the typed transition that permits each write/read.

**Current concrete example:** child-plan message box under `prototype1/messages/child-plan/<parent-node-id>.json`.

### 14. Append observations and decisions rather than overwriting them

**Status:** current architecture constraint.

Later graph traversal, recovery, and cross-lineage selection need the historical sequence of generation attempts, evaluations, selections, handoffs, and failures.

**Caveat:** Some live projections still overwrite latest state for operator convenience; those projections must not become authority by accident.

## Audit rule

Prototype 1 History/Crown authority claims should be periodically compared against current implementation. If the code permits forging `Parent<Ruling>`, `Crown<Locked>`, `Block<Sealed>`, or successor admission outside the intended path, the documentation claim must be narrowed or the implementation must be fixed before relying on the stronger claim.
