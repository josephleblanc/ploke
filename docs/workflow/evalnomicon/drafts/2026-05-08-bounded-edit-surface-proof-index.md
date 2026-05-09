# Bounded Edit Surface Proof Index

Status: working proof index for the bounded edit-surface implementation plan.

This document records what each validation test proves, where the test lives,
why the proof is non-trivial, and how the proof maps back to the formal
surface/edit language in the plan.

If a test cannot be mapped to a formal object or judgment, treat that as a
design smell:

```text
either the formal representation is incomplete,
or the test is not proving a load-bearing property.
```

Related plan:

- [`2026-05-08-bounded-edit-harness-adapter-plan.md`](2026-05-08-bounded-edit-harness-adapter-plan.md)
- [`formal-edit-surface.md`](formal-edit-surface.md)

## Formal Reference

Core formal objects:

```text
Γ_a = (V_a, E_a, μ_a)
g = (R, W, F)
ρ_a(q) = (Q_r, Q_w)
g ⊢ q  iff  Q_r ⊆ R ∧ Q_w ⊆ W ∧ Q_w ∩ F = ∅
valid_a(q)
apply_a(q) = (a', δ)
H' = H ⋅ (a, Γ_a, g, q, ρ_a(q), δ, a')
```

Observation/admission objects:

```text
event e
observation obs = observe(observer, e)
record rec = record(writer, obs)
evidence ev = admit_D(rec)
d ∈ inputs(D) iff admit_D(rec) = Some(ev)
```

where `D` is a decision domain such as `Diagnosis`, `Selection`,
`SurfaceCheck`, `HistoryAdmission`, or `OperatorProjection`.

A record can exist without being admissible evidence for a decision:

```text
record(rec) does not imply admissible_D(rec)

projection(rec) ∨ log(rec) ∨ ui_state(rec)
  does not imply admissible_D(rec)
```

unless an explicit admission rule for `D` says otherwise.

Surface-attempt admission:

```text
AttemptOutcome_a(q) =
    Applied(a', δ)
  | Rejected(reason)

H' = H ⋅ Attempt(a, Γ_a, g, q, ρ_a(q), AttemptOutcome_a(q))
```

The success-only notation
`H' = H ⋅ (a, Γ_a, g, q, ρ_a(q), δ, a')` is the `Applied(a', δ)`
case. `Rejected(reason)` produces no `a'` and no `δ`, but may still be
admitted as `AdmissibleEvidence<Diagnosis>`.

Current implementation vocabulary used by these tests:

```text
history::surface_attempt::Evidence
history::surface_attempt::Outcome
EvaluationPayload.surface_attempt
SurfaceEvidence
ArtifactDelta
ChildPlanFiles.rejected_surface_attempts
ParentSelection::current_generation_candidates()
```

The first route concerns failed or rejected proposal/application attempts that
may not produce a derived Artifact:

```text
q rejected
  => no apply_a(q)
  => no a'
  => no δ
  => H admits Attempt(..., Rejected(reason)) for the relevant decision domain
```

That rejected-attempt evidence is not the same as History admission of a
successful Artifact transition. It is parent-readable evidence used by later
diagnosis.

## Proof Flow

```mermaid
flowchart LR
    A1["A': rejected edit-surface attempt<br/>or rejected-only child plan"]
    B1["B*: persist/recover typed attempt evidence<br/>ChildPlanFiles.rejected_surface_attempts<br/>surface_attempt::Evidence"]
    C1["C': parent-readable payload projection<br/>EvaluationPayload.surface_attempt<br/>artifact = None"]

    A2["A': checked/applied surface evidence"]
    B2["B*: current generation projection"]
    C2["C': candidate payload<br/>artifact.surface + ArtifactDelta + applied attempt"]

    A3["A': projection/log-only state"]
    B3["B*: payload construction"]
    C3["C': no typed attempt evidence"]

    A1 --> B1 --> C1
    A2 --> B2 --> C2
    A3 --> B3 --> C3
```

Future request-policy receipt splice:

```text
A': parent artifact + router config + EditObjective
B*: Router-backed harness request construction
C': complete EffectiveRequestPolicyReceipt with stable client_policy_hash
```

Negative counterpart:

```text
A': proposal from a material model call with no receipt or incomplete policy
B*: proposal admission
C': rejected before SurfaceCheck / candidate admission
```

Future generator-surface provenance splice:

```text
A': parent Artifact contains generator surface version T_a and EditObjective
B*: harness/proposal construction uses T_a
C': proposal provenance cites T_a as the used GeneratorSurface
```

Later descendant-fitness analysis, tracked separately, should consume the
provenance chain:

```text
ArtifactDelta δ modifies GeneratorSurface T
  -> later proposal cites modified T'
  -> descendant evaluation score changes
```

## Test Index

### `below_min_rejected_attempts_are_persisted_and_recoverable_from_existing_child_plan`

Location:

```text
crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:8854-8916
```

Run:

```bash
cargo test -p ploke-eval below_min_rejected_attempts_are_persisted_and_recoverable_from_existing_child_plan -- --nocapture
```

Splice:

```text
A': rejected-only / below-min child plan
B*: ChildPlanFiles persists and receive_existing_child_plan recovers attempt evidence
C': ParentSelection::current_generation_candidates projects EvaluationPayload.surface_attempt
```

Formal meaning:

```text
q rejected
  => no apply_a(q)
  => no a'
  => no δ
  => Attempt(..., Rejected(reason)) survives persistence/recovery
  => admit_Diagnosis(record) = Some(surface_attempt::Evidence)
```

Why non-trivial:

- Crosses persistence, resume, and parent projection.
- Proves the rejected attempt is not only local runtime state.
- Proves no child Artifact is fabricated for a below-min failure.

Formal gap:

- The core formal model now names rejected-attempt admission with
  `AttemptOutcome_a(q)`.
- The remaining gap is to make the code's admission rule explicit:

```text
admit_Diagnosis(record) = Some(surface_attempt::Evidence)
```

only for records from the authorized evaluation/payload path.

### `current_generation_candidates_include_edit_surface_evidence`

Location:

```text
crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:9282-9337
```

Run:

```bash
cargo test -p ploke-eval current_generation_candidates_include_edit_surface_evidence -- --nocapture
```

Splice:

```text
A': completed child outcome with checked surface evidence
B*: ParentSelection::current_generation_candidates
C': EvaluationPayload carries CandidateArtifact.surface, delta digest, and applied attempt evidence
```

Formal meaning:

```text
g ⊢ q
valid_a(q)
apply_a(q) = (a', δ)
H' includes evidence(a, Γ_a, g, q, ρ_a(q), δ, a')
```

Why non-trivial:

- Proves successful checked/apply evidence survives into the
  candidate/History-facing projection.
- Connects `SurfaceEvidence` and `ArtifactDelta` to the parent selection
  payload instead of leaving them in local edit code.

Formal gap:

- None for the positive transition path.

### `current_generation_candidates_include_rejected_edit_surface_attempt_payload`

Location:

```text
crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:9339-9388
```

Run:

```bash
cargo test -p ploke-eval current_generation_candidates_include_rejected_edit_surface_attempt_payload -- --nocapture
```

Splice:

```text
A': rejected surface attempt attached to current generation evidence
B*: ParentSelection::current_generation_candidates
C': rejected payload is parent-readable and has artifact = None
```

Formal meaning:

```text
q rejected
  => no apply_a(q)
  => artifact = None
  => Attempt(..., Rejected(reason)) enters parent-readable payload projection
  => admitted as Diagnosis input
```

Why non-trivial:

- Proves rejected edit-surface attempts are visible to the parent even though
  they do not produce candidate Artifacts.
- Prevents the system from equating "no child" with "no useful evidence."

Formal gap:

- Same admission-rule gap as above. The formal plan now names the rejected
  attempt event; implementation still needs explicit admission rules as more
  decision domains consume records.

### `payload_surface_attempt_rejected_is_parent_readable_without_artifact`

Location:

```text
crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:9390-9425
```

Run:

```bash
cargo test -p ploke-eval payload_surface_attempt_rejected_is_parent_readable_without_artifact -- --nocapture
```

Splice:

```text
A': standalone rejected attempt evidence
B*: EvaluationPayload construction
C': has_parent_readable_surface_attempt() is true and artifact is absent
```

Formal meaning:

```text
Attempt(..., Rejected(reason)) is not Attempt(..., Applied(a', δ))
Attempt(..., Rejected(reason)) does not imply ∃ a'. apply_a(q) = (a', δ)
```

Why non-trivial:

- Isolates payload semantics from runtime fanout and selection machinery.
- Proves parent-readable attempt evidence is not coupled to Artifact creation.

Formal gap:

- The formal representation now explicitly distinguishes:

```text
AttemptOutcome_a(q) = Applied(a', δ) | Rejected(reason)
```

The implementation still needs to preserve that distinction as more consumers
start reading the evidence.

### `payload_without_surface_attempt_is_not_parent_readable_attempt_evidence`

Location:

```text
crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:9428-9450
```

Run:

```bash
cargo test -p ploke-eval payload_without_surface_attempt_is_not_parent_readable_attempt_evidence -- --nocapture
```

Splice:

```text
A': payload has projection/log-like text but no typed attempt evidence
B*: EvaluationPayload construction
C': has_parent_readable_surface_attempt() is false
```

Formal meaning:

```text
projection/log/diagnostic text does not imply admissible_Diagnosis(record)
```

Why non-trivial:

- Guards the authority boundary.
- Prevents CLI output, logs, mutable reports, TUI-local state, or prose failure
  descriptions from becoming source truth.

Formal gap:

- The observation/admission model now names the predicate:

```text
admissible_D(record)
```

and the negative rule:

```text
projection(rec) ∨ log(rec) ∨ ui_state(rec)
  does not imply admissible_Diagnosis(rec)
```

### `classify_rejected_surface_attempt_without_artifact_as_semantic_edit_resolution`

Location:

```text
crates/ploke-eval/src/cli/prototype1_state/edit_surface/diagnosis.rs
```

Run:

```bash
cargo test -p ploke-eval classify_rejected_surface_attempt_without_artifact_as_semantic_edit_resolution -- --nocapture
```

Splice:

```text
A': payload with typed surface_attempt::Evidence::Rejected and artifact = None
B*: edit_surface::diagnosis::classify(&EvaluationPayload)
C': Diagnosis { limiter = invalid_candidate_generation, failure_kind = semantic_edit_resolution }
```

### `payload_without_surface_attempt_is_not_semantic_edit_resolution_diagnosis`

Location:

```text
crates/ploke-eval/src/cli/prototype1_state/edit_surface/diagnosis.rs
```

Run:

```bash
cargo test -p ploke-eval payload_without_surface_attempt_is_not_semantic_edit_resolution_diagnosis -- --nocapture
```

Splice:

```text
A': projection/log-like payload detail without typed surface_attempt evidence
B*: edit_surface::diagnosis::classify(&EvaluationPayload)
C': no semantic_edit_resolution diagnosis
```

### `tui_apply_evidence_is_all_applied_or_rejected`

Location:

```text
crates/ploke-eval/src/cli/prototype1_state/edit_surface/tests.rs:631-718
```

Run:

```bash
cargo test -p ploke-eval tui_apply_evidence_is_all_applied_or_rejected -- --nocapture
```

Splice:

```text
A': one applied write vs two applied writes after grant.check and proposal staging
B*: tui::Apply::from_results
C': full application reports applied; partial application reports rejected
```

Formal meaning:

```text
apply_a(q) is defined only for all checked writes
partial_apply(q) is not apply_a(q)
partial_apply(q) => rejected attempt evidence, not ArtifactDelta success
```

Why non-trivial:

- Distinguishes checked apply from outcome classification.
- Proves partial application is not admitted as success.
- Protects `ArtifactDelta` from being minted for incomplete writes.

Formal gap:

- The plan specifies "all-or-rejected semantics," but it should name the
  apply-outcome classification:

```text
ApplyOutcome(q) = Applied(a', δ) | Rejected(reason)
```

with:

```text
Applied(a', δ) iff apply_a(q) = (a', δ)
Rejected(reason) iff apply_a(q) is undefined or not total
```

## Current Formal Additions Needed

The tests are meaningful, but they expose that the formal notation should grow
slightly. The missing concepts are:

### Rejected Attempt Evidence

Current success-only admission notation:

```text
H' = H ⋅ (a, Γ_a, g, q, ρ_a(q), δ, a')
```

Needed rejected-attempt notation:

```text
AttemptOutcome_a(q) =
    Applied(a', δ)
  | Rejected(reason)

H' = H ⋅ Attempt(a, Γ_a, g, q, ρ_a(q), AttemptOutcome_a(q))
```

with `Applied(a', δ)` linked to the successful Artifact transition and
`Rejected(reason)` explicitly not producing `a'`.

### Observation And Admission

Needed authority chain:

```text
event e
observation obs = observe(observer, e)
record rec = record(writer, obs)
evidence ev = admit_D(rec)
d ∈ inputs(D) iff admit_D(rec) = Some(ev)
```

Decision domains include:

```text
Diagnosis
Selection
SurfaceCheck
HistoryAdmission
OperatorProjection
```

The authority predicate is decision-relative:

```text
admissible_D(rec)
```

with:

```text
projection(rec) ∨ log(rec) ∨ ui_state(rec)
  does not imply admissible_D(rec)
```

This is the formal counterpart of the operational rule that CLI output, logs,
mutable reports, monitor views, and TUI-local proposal state may be records,
but they are not automatically source truth for diagnosis, selection, or
checked apply.

### Apply Outcome Classification

Needed outcome judgment:

```text
ApplyOutcome(q) =
  Applied(a', δ)
  | Rejected(reason)
```

with:

```text
Applied(a', δ) iff all checked writes apply and apply_a(q) = (a', δ)
Rejected(reason) iff no successful total apply exists
```

This is the formal counterpart of the all-or-rejected semantics used by the
TUI apply evidence test.

### Effective Request Policy Receipt

Needed Router/proposal provenance judgment:

```text
AdmitProposal(q) =>
  ∀ call ∈ material_model_calls(q).
    ∃ receipt(call).
      complete_effective_policy(receipt)
```

`complete_effective_policy(receipt)` means the receipt reconstructs the
client-side policy used to build the request, including explicit/defaulted
parameter values, config source digests, prompt/schema/tool digests,
retry/fallback policy, request/response payload digests, and available external
provider receipt metadata.

The proof target is not deterministic model output. The proof target is:

```text
same parent code + same admitted inputs + same client-side request policy
```

External provider nondeterminism is allowed, but it must be represented as
external receipt metadata or explicit `unknown`, not silently dropped.
