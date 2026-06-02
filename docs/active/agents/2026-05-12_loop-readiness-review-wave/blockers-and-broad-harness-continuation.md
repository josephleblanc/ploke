# Long-Loop Blockers And Broad-Harness Continuation

Date: 2026-05-12

Scope: synthesis of the loop readiness, invariants, and style reviews in this
directory, with the implementation contract needed to resolve the primary
blocker: complete live runs default to `BroadHarnessRequest`, but that generator
still stops before child planning.

## 2026-05-13 Resolution Note

Commit `bd00056b Add broad harness request fanout` resolves blocker 1 for the
current Prototype 1 run shape. Complete live mode now accepts
`BroadHarnessRequest`, publishes one request slot per child-budget slot, invokes
the headless `ploke-tui` adapter when a submitted result is absent, admits each
request-bound workspace diff through backend checks, and seals a child plan from
the admitted transactions.

This file remains useful as the design contract that the implementation was
meant to satisfy. Treat the sections below as pre-resolution context unless
they discuss the still-open durability and proof gaps: clean campaign worktree,
verified publication loading, durable grant projection, compatibility aliases,
and eventual formal `SurfaceGrant`/`CheckedProposal` coverage.

## Current Blockers

1. `BroadHarnessRequest` is the default generation posture, but complete live
   mode rejects it until a typed request-to-child continuation exists.
2. Broad-harness admission requires a clean source repository. The current
   working tree is dirty, so it is not a suitable campaign worktree.
3. `request::Request<request::Broad, request::Published>` exists, but
   publication loading and mutation are not yet fully constrained by a verified
   transition. The stored hash can be deserialized as data instead of checked as
   a publication fact.
4. `grant::Grant<grant::Checked>` and `grant::Grant<grant::Admitted>` exist,
   but the durable coordinate projection still uses an untagged record shape.
5. Compatibility aliases still keep flattened names on active paths. The
   compiler sees the structural carriers, but future patches can still follow
   the old vocabulary.

Only the first blocker prevents the broad-harness path from becoming a runnable
complete-loop generator. The other blockers should be handled before trusting
many generations of persisted evidence, but they are not the continuation seam
itself.

## Structural Carrier Map For Blocker 1

Larger semantic object:

The missing object is a bounded, hyper-agent-style candidate creation protocol.
A published broad request is not child evidence and not an admitted candidate.
It is an invitation for an external harness to operate over a target Artifact
surface, return evidence, and then let `ploke-eval` decide whether that evidence
can become loop authority.

Refused reduction:

Do not make `BroadHarnessRequest` directly satisfy child planning. Do not treat
submitted harness JSON, CLI output, or harness self-report as an admitted child.
Do not encode the transition as another compound noun that bundles request,
admission, child, plan, and surface evidence into one flat record.

Roles:

- `Parent<Ruling>` owns generation, surface policy, History admission, and child
  materialization.
- A broad harness is an executor/proposer over a target Artifact, not an
  authority source.
- `History` remains the durable authority substrate for admitted lineage facts.

States:

- `request::Request<request::Broad, request::Draft>` before publication.
- `request::Request<request::Broad, request::Published>` after the Parent binds
  the request to policy, identity, hash, and publication paths.
- Submitted harness result as evidence, not authority.
- Checked surface result after `ploke-eval` validates writes against the
  bounded surface.
- Admitted candidate/child plan only after the checked result is bound back to
  the published request and current Parent authority.

Transitions:

- publish: draft broad request plus live `EditSurfaceAdmission` becomes a
  published request and a module-owned reference/hash.
- submit: harness returns a result package that names the request reference,
  changed files, evidence roots, and return evidence.
- check: `ploke-eval` validates request binding, source cleanliness, stale base,
  protected-core containment, writable surface containment, and material patch
  evidence.
- admit: checked result becomes an admitted candidate artifact or a child plan
  that the complete loop may consume.

Durable record projections:

- Published request record: persisted request preimage, hash, identity, paths,
  policy binding, and target Artifact coordinate.
- Submitted result record: harness evidence and changed-path claims.
- Checked/admitted surface record: explicit projection of the `Grant<Checked>`
  to `Grant<Admitted>` refinement.
- Child-plan/admitted-candidate record: the loop-consumable continuation fact
  that names the request, grant, surface check, resulting Artifact, and History
  admission context.

Module boundary:

The request module should own request identity, hash, publication, binding, and
receipt/load verification. Surface/grant code should own checked/admitted grant
refinement. Parent/complete-loop code should consume only the advanced state,
not the submitted harness evidence directly.

## Correct Implementation Shape

The broad-harness continuation should be implemented as a vertical transition,
not as a CLI special case:

```text
request::Request<Broad, Published>
  + submitted harness result
  + live EditSurfaceAdmission
  + clean target/source Artifact checks
  + grant::Grant<Checked>
  -> grant::Grant<Admitted>
  -> loop-consumable child plan or admitted candidate Artifact
```

The exact Rust names can vary, but the active code should preserve these axes:

- request kind: broad request, not deterministic TUI target selection;
- request phase: draft vs published vs verified-loaded publication;
- authority source: Parent/History/admission policy, not the harness;
- provenance: submitted result came from a specific published request and target
  Artifact coordinate;
- bounded edit surface: all writes are outside protected core and inside the
  admitted writable surface;
- loop consumer: complete mode may only consume the admitted child-plan or
  candidate Artifact state.

The compile-time constraint should be:

Complete live runs cannot materialize a child from a submitted broad-harness
result unless the value has passed through the request-bound admission
transition. The function that continues the loop should require the advanced
state, not a raw submitted result, a bare request id, or a broad generator enum
variant.

## Relation To Bounded Edits

The bounded edit model says `ploke-eval` owns grants, checks, candidate
admission, History evidence, runtime hydration, and selection. Harnesses may
propose/apply only as executors under Parent grant.

For this blocker, that means:

- the request may give the harness broad freedom over the target repository
  outside protected core;
- the harness may choose the intervention based on evidence roots and budget;
- the harness return package is evidence about what it changed and why;
- `ploke-eval` must independently check the resulting material changes against
  the granted Artifact surface;
- only after that check may the result become an admitted candidate, child plan,
  or child Artifact.

This keeps broad editing from regressing into a deterministic target-file
picker while also preventing the hyper-agent from becoming authority over the
loop.

## Relation To Hyper-Agents Prompt Posture

The hyper-agent direction is intentionally broad:

```text
Inspect the repository and evidence. Choose and implement the improvement most
likely to improve future evaluations. You may edit outside protected core.
```

That posture belongs in the prompt and request payload, not in History
authority. The Parent should expose evidence roots, budget, evaluation context,
return-evidence requirements, and the protected-core pointer. It should not turn
protocol diagnoses into hard edit targets.

The continuation must therefore preserve this split:

- request/prompt: broad, evidence-directed, intervention-choosing;
- submission: descriptive evidence from the harness;
- admission: bounded, policy-checked, Parent-owned;
- child planning: typed continuation consumed by the complete loop.

## Minimal Slice To Resolve Blocker 1

1. Add or tighten a verified load/publication path for
   `request::Request<request::Broad, request::Published>` so stored request
   hashes are recomputed before active use.
2. Define the request-bound continuation carrier that complete mode consumes.
   It should bind the published request reference, submitted result, admitted
   grant/surface evidence, resulting Artifact identity, and History admission
   context.
3. Wire submitted broad-harness result admission to mint that continuation
   carrier.
4. Change complete mode so `BroadHarnessRequest` no longer errors when the
   continuation carrier is present, and still errors when only a raw request or
   submitted result is present.
5. Add tests that fail if a raw submitted result, stale request hash, mismatched
   request binding, dirty source repo, protected-core edit, or unbound child
   plan can enter the complete-loop child materialization path.

## Run Readiness Gate After Implementation

Before attempting 15 generations / 128 max nodes:

1. Use a clean or isolated campaign worktree.
2. Run focused broad-harness admission tests.
3. Run a one-generation complete loop with a low node cap.
4. Run a two-generation complete loop with the intended generator and a smaller
   total-node cap.
5. Only then attempt the longer loop.
