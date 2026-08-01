# General Style Review: Prototype 1 Structural-Carrier Changes

Role: general style reviewer

Scope:
- `crates/ploke-eval/src/cli/prototype1_state/backend.rs`
- `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs`
- `crates/ploke-eval/src/cli/prototype1_state/edit_surface/harness_request.rs`
- `crates/ploke-eval/src/cli/prototype1_state/edit_surface/harness_result.rs`
- `crates/ploke-eval/src/cli/prototype1_state/history.rs`
- `crates/ploke-eval/src/cli/prototype1_state/history_preview.rs`
- `crates/ploke-eval/src/cli/prototype1_state/parent.rs`
- `xtask/src/commands/check.rs`

## Findings

### High: Published request invariants are still bypassable through `pub(crate)` fields

`request::Request<K, S>` now carries `Identity<K>`, `Hash<K>`, `Binding<P>`, and a `Published` typestate, which is the right direction. However, the core fields remain `pub(crate)` at `crates/ploke-eval/src/cli/prototype1_state/edit_surface/harness_request.rs:212`, including `request_hash` at line 215 and `admission_binding` at lines 219-220.

That means crate-local code can mutate a published request without going through `with_admission_binding`, which recomputes the request hash at `crates/ploke-eval/src/cli/prototype1_state/edit_surface/harness_request.rs:1090`. The test at `crates/ploke-eval/src/cli/prototype1_state/backend.rs:3755` demonstrates the problem by assigning `published.admission_binding = ...` directly at line 3756.

This weakens the structural-carrier work: the carrier exists, but the request hash and admission binding relation is still partly a convention. Prefer private fields plus constructor/transition methods. Tests that need corrupt data can use a test-only helper, a serde fixture, or a record/projection boundary rather than direct active-carrier mutation.

### Medium: Compatibility aliases keep flattened authority names on active production paths

The aliases at `crates/ploke-eval/src/cli/prototype1_state/edit_surface/harness_request.rs:29` and `crates/ploke-eval/src/cli/prototype1_state/edit_surface/harness_request.rs:32` let the structural naming check pass, but active callers still mostly speak in the flattened names `PublishedBroadHarnessRequest` and `RequestAdmissionBinding`.

Examples:
- `crates/ploke-eval/src/cli/prototype1_state/backend.rs:23`
- `crates/ploke-eval/src/cli/prototype1_state/edit_surface/harness_result.rs:5`
- `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:1232`
- `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:1253`

As a migration shim this is understandable, but as-is it makes `structural-naming:allow` function like a broad escape hatch. The aliases should either have a concrete removal task or be replaced at call sites with `request::Request<request::Broad, request::Published>` and `request::Binding<surface::SurfacePolicyId>`, preferably imported locally so the call sites stay readable.

### Medium: `grant::Coordinate<S>` carries typestate externally but stores an erased state enum internally

The grant module introduces `Coordinate<Checked>`, `Coordinate<Admitted>`, and `Grant<S>` at `crates/ploke-eval/src/cli/prototype1_state/history.rs:2925`, which is a clear improvement over the previous flattened grant-coordinate names.

The implementation still stores `record::Coordinate` inside every `Coordinate<S>` at `crates/ploke-eval/src/cli/prototype1_state/history.rs:2935`. That requires state-specific methods to include unreachable branches, for example `runtime_id` at lines 3001-3005, `candidate` at lines 3029-3033, and serde implementations at lines 3297-3339.

Those `unreachable!` branches are a maintainability smell: the outer typestate says the state is known, but the inner field layout does not reflect it. A stronger shape would make `Coordinate<Checked>` own `record::Checked` and `Coordinate<Admitted>` own `record::Admitted`, then reserve `record::Coordinate` or `AnyCoordinate` for durable projections.

### Medium: Grant evidence uses an untagged durable coordinate projection

`grant::AnyCoordinate` is serialized with `#[serde(untagged)]` at `crates/ploke-eval/src/cli/prototype1_state/history.rs:2950`. The checked and admitted variants currently have different field shapes, so this works today, but it leaves the durable record without an explicit state discriminant.

This is fragile for a long-running loop: future fields can make the shapes ambiguous, and external/debug readers cannot tell whether the state is checked or admitted without schema inference. Prefer an explicitly tagged record projection, e.g. `#[serde(tag = "state", content = "coordinate")]`, while keeping active loop code on `Grant<Checked>` and `Grant<Admitted>`.

### Low: `request::Request<K, S>` advertises more genericity than it currently carries

`request::Request<K, S>` is generic over kind and state at `crates/ploke-eval/src/cli/prototype1_state/edit_surface/harness_request.rs:210`, but the payload is still hard-coded to `BroadHarnessRequest` at line 221 and the admission binding is fixed to `surface::SurfacePolicyId` at lines 219-220.

That is not incorrect for the current broad harness path, but it can mislead future code into treating `K` as a real request-family abstraction. Either keep the module explicitly broad-request-shaped until another kind exists, or move the payload and binding policy behind a kind-owned associated structure before adding more request kinds.

## Open Questions

- Are the compatibility aliases intended to survive the next slice, or should they be treated as temporary migration aids with a tracked removal task?
- Should submitted harness result records remain active mutable structs, or should tampering tests move to typed fixture/projection constructors so production fields can become private?
- Does the durable grant JSON need backward compatibility with existing untagged records, or can the next schema version add an explicit state tag?

## Verification

Commands run:
- `cargo check -p ploke-eval 2>&1 | tail -n 80`: passed; `ploke-eval` still reports existing warnings.
- `cargo run -p xtask -- check structural-naming --report-only 2>&1 | tail -n 80`: passed; 117 Rust files checked, threshold 3.
- `cargo check -p xtask 2>&1 | tail -n 60`: passed; inherited `syn_parser` warnings shown.
- `git diff --check -- crates/ploke-eval/src/cli/prototype1_state/backend.rs crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs crates/ploke-eval/src/cli/prototype1_state/edit_surface/harness_request.rs crates/ploke-eval/src/cli/prototype1_state/edit_surface/harness_result.rs crates/ploke-eval/src/cli/prototype1_state/history.rs crates/ploke-eval/src/cli/prototype1_state/history_preview.rs crates/ploke-eval/src/cli/prototype1_state/parent.rs xtask/src/commands/check.rs`: passed.

## Recommended Follow-Up Commands

- `cargo test -p ploke-eval broad_harness_ 2>&1 | tail -n 80`
- `cargo test -p ploke-eval edit_surface 2>&1 | tail -n 80`
- `cargo run -p xtask -- check structural-naming`
- `cargo check -p ploke-eval 2>&1 | tail -n 80`

## Changed Files

- `docs/active/agents/2026-05-12_loop-readiness-review-wave/style.md`
