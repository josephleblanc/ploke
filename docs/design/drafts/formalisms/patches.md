# Patch Formalism Draft

## How this connects to the larger framework

### Theoretical Anchor Points
- Context
  The index relative to which things are interpreted.
- Fiber / Indexed Layer
  Objects and admissible transitions over a given context.
- Artifact Strata
  Source, IR, binary, config, ledger, prompts, schemas.
- Interpretation / Compilation Maps
  Maps between strata, not to be conflated.
- Proposal vs Commit
  Candidate change vs realized state transition.
- Anchoring Context
  What a proposal actually depends on.
- Transport Existence
  Cross-context use should be stated as ∃ T ..., not assumed.
- Phase Barrier
  Current-generation execution vs next-generation effect.
- Generation Index
  The spiral/time-series structure you were describing.
- Validation Boundary
  External constraints that modified procedures do not get to rewrite implicitly.
- Partiality / Effects
  Failure, nondeterminism, later uncertainty kernels.
- Provenance
  Needed if later inferences are meant to be compositional rather than narrative.

### Coarse relationship
This document is anchored to the larger project for auto-transformation as roughly:

  source syntax  --compile-->  executable artifact  --execute-->  runtime behavior
        ^                                                        |
        |--------------------- propose / commit -----------------|

### Additional sources to consider
Bénabou 1985
  (in french) SGA 1 (https://arxiv.org/abs/math/0206203), 
  (https://www.cambridge.org/core/journals/journal-of-symbolic-logic/article/fibered-categories-and-the-foundations-of-naive-category-theory/5BE99DB6F7BAE699D81D27BC5C0A3D80),
Mac Lane 
  (https://link.springer.com/book/10.1007/978-1-4757-4721-8), 
Jacobs review
  (https://www.math.mcgill.ca/rags/jacobs.html).

## Patch Semantics
Draft for architecture on staged patch proposals, user approval, and
same-file reconciliation under live file change.

> for treating patches as version-anchored deferred transitions rather than
> raw diffs or immediate writes

A patch is not only a diff string. In the user-facing system, a patch is
staged, displayed, and later approved or rejected against a live workspace
whose target files may have changed in the meantime.

The formal object therefore is not merely a patch artifact. It is a staged
proposal anchored to a required input version and maintained under subsequent
confirmed file changes.

## 1. Universes And Basic Sorts

```text
R        = set of target resources/files
Doc      = set of document states
Σ        = set of workspace states
ID       = set of proposal identifiers
H        = set of ordered hunk sequences
Patch    = set of single-file patch artifacts
Δ        = set of deferred patch effects
Preview  = set of staged user-facing renderings
Status   = {pending, applied, rejected, stale, conflicted, failed}
Cause    = set of confirmed file-change causes
V        = set of file versions
Anchor   = set of admissible anchoring contexts
P(A)     = power set of A
```

```text
target : Σ × R -> Doc
ver    : Σ × R -> V
render : Doc ⇀ Preview
write  : Σ × R × Doc -> Σ
dom    : (A ⇀ B) -> P(A)
```

Interpretation:

- `target(σ, r)` is the live document for file `r` in workspace state `σ`.
- `ver(σ, r)` is the live version/hash for file `r` in workspace state `σ`.
- `render(d)` is the staged preview derived from document state `d`, when such
  a preview exists.
- `write(σ, r, d)` is the workspace state obtained by replacing the live
  document at `r` with `d`.

## 2. Single-File Patch Artifacts

A primitive single-file patch artifact is an element of:

```text
Patch = R × H
```

Its semantics are given by a partial application function:

```text
apply : (Patch, Doc) ⇀ Doc
```

Interpretation:

- a patch artifact is target-specific
- patch application is partial because a hunk sequence may fail to apply to a
  given document state
- this is still not the user-facing object in the TUI

## 3. Staged Patch Proposals

The user-facing object is a staged proposal:

```text
Π_item = ID × R × V × Δ × Preview × Status
```

For `π ∈ Π_item`, write:

```text
id(π)      ∈ ID
target(π)  ∈ R
req(π)     ∈ V
effect(π)  ∈ Δ
view(π)    ∈ Preview
status(π)  ∈ Status
```

The deferred effect is version-anchored:

```text
effect(π) : Doc[req(π)] ⇀ Doc
```

The present formalism also exposes a minimal anchoring projection:

```text
anchor_min : Π_item -> R × V
anchor_min(π) = (target(π), req(π))
```

Interpretation:

- `req(π)` is the required input version for approval of `π`
- a staged proposal is a deferred right to attempt a file transition later
- the user acts on staged proposals, not directly on raw patch artifacts
- `anchor_min(π)` is the minimal anchor made explicit in this document;
  richer anchoring contexts are left open for later extension

## 4. Proposal Registries

Let `Π_reg` be the set of finite proposal registries:

```text
Π_reg ⊆ (ID ⇀ Π_item)
Pending(Reg) := { i ∈ dom(Reg) | status(Reg(i)) = pending }
```

For registry update, write:

```text
Reg[id ↦ π']
```

Interpretation:

- `Reg ∈ Π_reg` denotes one proposal registry
- `Reg[id ↦ π']` denotes the registry obtained by replacing the entry for `id`
  with `π'`
- this is record/map update notation, not codebase-specific pseudocode

## 5. Staging

Staging does not mutate the workspace. It constructs a pending proposal from
the current live file state.

Define:

```text
stage : Σ × Patch ⇀ Π_item
```

For `P = (r, h) ∈ Patch`, the domain of `stage` is:

```text
dom(stage) = {
  (σ, P) ∈ Σ × Patch |
  ∃ v ∈ V, ∃ d' ∈ Doc, ∃ p ∈ Preview .
    ver(σ, r) = v
    ∧ apply(P, target(σ, r)) = d'
    ∧ render(d') = p
}
```

If `(σ, P) ∈ dom(stage)` and `stage(σ, P) = π`, then:

```text
target(π) = r
∃ d' ∈ Doc .
  apply(P, target(σ, r)) = d'
  ∧ effect(π)(target(σ, r)) = d'
  ∧ req(π) = ver(σ, r)
  ∧ view(π) = render(d')
status(π) = pending
```

Interpretation:

- staging observes the confirmed live file version at staging time
- staging produces a pending proposal anchored to that observed version
- staging is not a file mutation

## 6. Approval

Approval is a partial state transition on a workspace state and a proposal
registry.

For proposal identifier `i ∈ ID`, define:

```text
approve_i : (Σ × Π_reg) ⇀ (Σ × Π_reg)
```

Its domain is:

```text
dom(approve_i) = {
  (σ, Reg) ∈ Σ × Π_reg |
  i ∈ Pending(Reg)
}
```

### Successful Approval

If:

```text
π = Reg(i)
target(π) = r
req(π) = v
ver(σ, r) = v
effect(π)(target(σ, r)) = d'
```

then:

```text
σ'   = write(σ, r, d')
Reg' = Reg[i ↦ π[status ↦ applied]]
```

and:

```text
approve_i(σ, Reg) = (σ', Reg')
```

### Failed Approval By Version Mismatch

If:

```text
π = Reg(i)
target(π) = r
req(π) = v
ver(σ, r) ≠ v
```

then:

```text
σ'   = σ
Reg' = Reg[i ↦ π[status ↦ stale]]
approve_i(σ, Reg) = (σ', Reg')
```

### Failed Approval By Patch Failure

If:

```text
π = Reg(i)
target(π) = r
req(π) = v
ver(σ, r) = v
target(σ, r) ∉ dom(effect(π))
```

then:

```text
σ'   = σ
Reg' = Reg[i ↦ π[status ↦ failed]]
approve_i(σ, Reg) = (σ', Reg')
```

Interpretation:

- approval is valid only relative to the required input version
- only confirmed successful approval advances live file state
- version mismatch and patch failure are distinct failure shapes

## 7. Rejection

Rejection is non-mutating with respect to workspace state.

For proposal identifier `i ∈ ID`, define:

```text
reject_i : Σ × Π_reg -> Σ × Π_reg
```

For all `(σ, Reg) ∈ Σ × Π_reg`:

```text
i ∈ Pending(Reg)   ⇒ reject_i(σ, Reg) = (σ, Reg[i ↦ Reg(i)[status ↦ rejected]])
i ∉ Pending(Reg)   ⇒ reject_i(σ, Reg) = (σ, Reg)
```

Interpretation:

- rejection changes proposal lifecycle state only
- rejection does not change the live file

## 8. User-Facing Action Surface

The user-facing action set is:

```text
Action = { Approve(i) | i ∈ ID } ∪ { Reject(i) | i ∈ ID } ∪ { ApproveAll, RejectAll }
```

The system does not expose:

```text
{ ApproveSubset(J) | J ⊆ ID } ∪ { RejectSubset(J) | J ⊆ ID }
```

for arbitrary proposal subsets `J`.

Interpretation:

- the semantics need only support single-proposal actions and global actions
- they do not need to support arbitrary subset scheduling as a first-class UI
  operation

## 9. Bulk Actions

### RejectAll

`RejectAll` is extensional and non-mutating:

```text
RejectAll : Σ × Π_reg -> Σ × Π_reg
```

For `(σ, Reg) ∈ Σ × Π_reg`, define:

```text
RejectAll(σ, Reg) = (σ, Reg')
```

where:

```text
∀ i ∈ dom(Reg) .
  i ∈ Pending(Reg)  ⇒ Reg'(i) = Reg(i)[status ↦ rejected]
∧ i ∉ Pending(Reg) ⇒ Reg'(i) = Reg(i)
```

### ApproveAll

`ApproveAll` is not a set action. It is an ordered composition of
single-proposal approval transitions.

For an implementation-defined ordering of pending identifiers
`(i_1, ..., i_k)`, define:

```text
(i_1, ..., i_k) is a permutation of Pending(Reg)
ApproveAll = approve_(i_k) ∘ ... ∘ approve_(i_2) ∘ approve_(i_1)
```

Interpretation:

- `ApproveAll` is sequential
- later approvals observe the live workspace state produced by earlier ones
- this is the source of same-file order sensitivity

## 10. Same-File Pending Proposals

Let:

```text
π_a = (i_a, r, v_0, δ_a, p_a, pending)
π_b = (i_b, r, v_0, δ_b, p_b, pending)
```

Then:

```text
target(π_a) = target(π_b) = r
req(π_a) = req(π_b) = v_0
```

In general:

```text
approve_(i_a) ∘ approve_(i_b) ≠ approve_(i_b) ∘ approve_(i_a)
```

More specifically, if:

```text
π_b = Reg(i_b)
approve_(i_a)(σ, Reg) = (σ', Reg')
ver(σ', r) ≠ v_0
```

then:

```text
ver(σ', r) ≠ req(π_b)
```

Interpretation:

- same-file pending proposals are not merely "multiple edits"
- they are multiple deferred transitions anchored to one live file history
- once one same-file proposal is successfully applied, the other is no longer
  guaranteed to remain valid relative to its prior required version

## 11. Confirmed File Change And Reconciliation

The present document does not fix a unique construction of reconciliation.
Instead it specifies an open semantic slot constrained by domain and codomain
laws.

Define a confirmed file-change event:

```text
FileChanged = R × V × V × Cause
```

Write:

```text
fc = (r, v_old, v_new, c)
```

For every pending proposal targeting `r`, the system must perform a
reconciliation step:

```text
reconcile_fc : Π_item -> Π_item
```

with the following constrained specification:

```text
dom(reconcile_fc) = {
  π ∈ Π_item |
  target(π) = r ∧ req(π) = v_old ∧ status(π) = pending
}
```

For all `π ∈ dom(reconcile_fc)`, if `reconcile_fc(π) = π'`, then:

```text
id(π') = id(π)
target(π') = target(π)
```

and:

```text
status(π') ∈ {pending, stale, conflicted}
```

and:

```text
status(π') = pending ⇒ req(π') = v_new
```

Interpretation:

- a confirmed file change may be caused by prior approval, external user edit,
  or any other confirmed mutation
- every still-pending same-file proposal must be reconciled against the new
  live version
- the relevant semantic object is not only a patch, but a maintained staged
  proposal under file-version change
- the construction of `reconcile_fc` is intentionally left open beyond the
  constraints stated above

## 12. Reanchoring Constraint

Define the bare version-substitution operator:

```text
reanchor_v : Π_item × V -> Π_item
reanchor_v(π, v_new) = π[req ↦ v_new]
```

The following implication is not admissible in general:

```text
∀ π ∈ dom(reconcile_fc) .
  reconcile_fc(π) = reanchor_v(π, v_new)
```

Interpretation:

- a change in required input version is not a metadata substitution
- if `reconcile_fc(π) = π'` and `status(π') = pending`, then `π'` is a
  revalidated proposal rather than a bare version-substituted copy of `π`
- the system must not treat unchanged patch payload plus updated version tag as
  sufficient by default

## 13. Live-State Discipline

If:

```text
stage(σ, P) = π
target(π) = r
```

then staging yields no workspace successor state:

```text
¬∃σ'. stage(σ, P) = (π, σ')
```

If:

```text
approve_i(σ, Reg) = (σ', Reg')
π = Reg(i)
target(π) = r
```

by the successful-approval branch, then:

```text
fc = (r, ver(σ, r), ver(σ', r), approve(i)) ∈ FileChanged
```

Interpretation:

- speculative successor information may exist as advisory metadata
- speculative successor information does not replace post-apply reconciliation
- dependent staged proposals should update only after confirmed file change
