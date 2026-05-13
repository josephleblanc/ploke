# Structural Invariants Review: TUI Adapter Wave

Scope: current `ploke-eval` TUI adapter implementation wave. Review used targeted reads only and applied the structural-carrier-gate / structural-naming guidance conceptually.

## Findings

### High: Broad harness request/admission binding is dropped before child-plan validation

`admit_submitted_broad_harness_result` correctly checks the submitted result against the published request and reprojects the live admission binding before admission:

- `crates/ploke-eval/src/cli/prototype1_state/backend.rs:1481`
- `crates/ploke-eval/src/cli/prototype1_state/backend.rs:1488`
- `crates/ploke-eval/src/cli/prototype1_state/backend.rs:1494`
- `crates/ploke-eval/src/cli/prototype1_state/backend.rs:1500`

But the admitted result is then projected into a child plan without carrying the request reference, request hash, admission binding, or checked surface evidence:

- `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:1395`
- `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:1433`
- `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:1471`

The later broad child validator only checks a magic producer string, presence of base/derived artifact ids, and target equality:

- `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:1790`
- `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:1792`
- `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:1800`
- `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:1808`

This loses the structural chain `Request<Broad, Published> -> SubmittedResult -> Admission -> ChildPlan`. After projection, a child can satisfy the broad harness path without a request-bound evidence carrier. The deterministic TUI path has `SurfaceEvidence`; broad harness should have an equivalent request/admission-bound evidence carrier or should attach a typed surface/admission evidence projection before `ChildFiles` is accepted.

### Medium: `Request<Broad, Published>` is typestated but not sealed against crate-local mutation

The request module has a good carrier shape:

- `crates/ploke-eval/src/cli/prototype1_state/edit_surface/harness_request.rs:310`
- `crates/ploke-eval/src/cli/prototype1_state/edit_surface/harness_request.rs:319`
- `crates/ploke-eval/src/cli/prototype1_state/edit_surface/harness_request.rs:326`
- `crates/ploke-eval/src/cli/prototype1_state/edit_surface/harness_request.rs:487`

However, the published request fields are `pub(crate)`, including `request_hash`, paths, `admission_binding`, and body:

- `crates/ploke-eval/src/cli/prototype1_state/edit_surface/harness_request.rs:488`
- `crates/ploke-eval/src/cli/prototype1_state/edit_surface/harness_request.rs:494`
- `crates/ploke-eval/src/cli/prototype1_state/edit_surface/harness_request.rs:496`

`prototype1_workspace` and `with_admission_binding` recompute the hash, but crate-local code can still mutate a published request directly and leave the hash/binding relation inconsistent. This is already exercised in tests by direct field mutation:

- `crates/ploke-eval/src/cli/prototype1_state/backend.rs:4133`

Runtime verification catches many bad submitted-result cases, but the compile-time invariant is weaker than the typestate name claims. Prefer private fields plus transition methods for all hash/binding-affecting changes.

### Low: Several new active-path names still flatten phase, authority, and mechanism

Some flattened names are acceptable durable record/projection names, especially under `harness_result.rs`:

- `SubmittedBroadHarnessResult`: `crates/ploke-eval/src/cli/prototype1_state/edit_surface/harness_result.rs:11`
- `SubmittedRequestBinding`: `crates/ploke-eval/src/cli/prototype1_state/edit_surface/harness_result.rs:27`

The more concerning active-path names are:

- `AdmittedBroadHarnessResult`: `crates/ploke-eval/src/cli/prototype1_state/backend.rs:474`
- `BroadHarnessRequestPublication`: `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:1600`
- `PendingBroadHarnessRequest`: `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:875`

These are not immediate blockers because the backend carrier is privately minted and the CLI names are mostly projection/error surface. Still, they combine generator kind, phase, authority, and artifact/result role in one identifier. If this path grows, prefer structural carriers such as request/result roles with `Published`, `Submitted`, `Admitted`, or `Awaiting` state markers behind module boundaries.

## Positive Checks

- The request state carrier preserves `Broad` and `Published` as type parameters rather than relying only on strings: `crates/ploke-eval/src/cli/prototype1_state/edit_surface/harness_request.rs:319`, `crates/ploke-eval/src/cli/prototype1_state/edit_surface/harness_request.rs:326`, `crates/ploke-eval/src/cli/prototype1_state/edit_surface/harness_request.rs:487`.
- The submitted result is a named typed durable record, not `serde_json::Value` field-walking in production: `crates/ploke-eval/src/cli/prototype1_state/edit_surface/harness_result.rs:11`, `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:1190`.
- The parent request wait state preserves the published request reference structurally: `crates/ploke-eval/src/cli/prototype1_state/parent.rs:47`, `crates/ploke-eval/src/cli/prototype1_state/parent.rs:981`, `crates/ploke-eval/src/cli/prototype1_state/parent.rs:1018`.
- Live admission rechecks the request binding before broad harness admission: `crates/ploke-eval/src/cli/prototype1_state/backend.rs:1494`, `crates/ploke-eval/src/cli/prototype1_state/backend.rs:1500`.
