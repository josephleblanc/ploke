# 2026-07-05 Binding/Type-Aware Resolver Plan

Short description: bounded implementation plan for the next call-graph semantic-expansion phase: prove more callable-value and receiver calls from local binding/type evidence without weakening targetless unsupported rows.

Related planning files:
- [`2026-07-01_call-graph-larger-plan-map.md`](2026-07-01_call-graph-larger-plan-map.md)
- [`2026-07-01_call-graph-goal-coverage-matrix.md`](2026-07-01_call-graph-goal-coverage-matrix.md)
- [`2026-06-22_call-site-coverage-matrix.md`](2026-06-22_call-site-coverage-matrix.md)
- [`2026-06-25_call-graph-quality-recovery-tracker.md`](2026-06-25_call-graph-quality-recovery-tracker.md)
- [`../../../../.hermes/plans/2026-06-15_225532-ploke-call-graph-typed-plan.md`](../../../../.hermes/plans/2026-06-15_225532-ploke-call-graph-typed-plan.md)

## Position

Root plan: `.hermes/plans/2026-06-15_225532-ploke-call-graph-typed-plan.md`

Current phase: binding/type-aware semantic resolution.

Current bucket: local binding tracking for callable values and typed receivers.

Exit criteria for the first implementation slice:

- one parser-side typed proof shape is added or extended;
- one transform/DB projection assertion proves the shape is persisted exactly;
- one RAG/tool assertion is added only if the shape is surfaced downstream;
- unsupported rows remain explicit when proof is missing;
- no broad trait dispatch, arbitrary dynamic callable execution, or workspace dependency-root import resolution is introduced.

Next phase if this bucket is done: choose the next unresolved coverage-matrix bucket, or expand binding proof by one adjacent shape.

## Existing Pattern To Reuse

Do not add a separate binding subsystem before proving that the existing one cannot carry the case.

Current parser pattern:

- `crates/ingest/syn_parser/src/parser/visitor/call_extraction.rs`
  classifies local syntax into typed call payloads.
- `LocalBindingProof` is already the local evidence source during extraction.
- `PathCallCallee` records item-path, value-binding, closure-binding, and initialized-value-binding path calls.
- `DynamicCallCallee` records closure and callable binding cases.
- `MethodCallReceiver` records local, typed-local, initialized-local, borrowed, dereferenced, field, branch, result, await, try, literal, and unsupported receiver shapes.

Current resolver pattern:

- `crates/ingest/syn_parser/src/resolve/call_resolution/path.rs`
  resolves only `PathCallCallee` shapes that carry exact proof.
- `crates/ingest/syn_parser/src/resolve/call_resolution/dynamic.rs`
  resolves only exact dynamic callable/closure shapes.
- `crates/ingest/syn_parser/src/resolve/call_resolution/method.rs`
  resolves only receiver shapes with exact local type, initializer, field, branch, or result proof.
- Unknown, mixed-target, external, opaque, or unsupported cases must produce explicit `CallResolutionStatus::{Unsupported, External, Unresolved, Ambiguous}` rather than a guessed edge.

Current DB/RAG pattern:

- `crates/ploke-db/src/call_graph/receiver.rs` and `call_graph/receiver/decode.rs`
  mirror receiver payloads in small typed variants and validate row shape during decode.
- RAG maps DB `CallReceiver` into `CallReceiverInfo`; it should not infer proof that DB did not persist.
- Tool surfaces read RAG/DB summaries; they should not create resolver semantics.

## First Candidate Slices

Pick one, not all.

1. Function pointer parameter blockers:
   - Keep `f()` / `(f)()` where `f: fn(...)` is an owner parameter targetless unless an initializer is available.
   - Add or verify parser/DB/RAG proof that the callable parameter is visible as `ValueBinding` / `LocalBinding` with no edge.
   - This is a blocker-visibility slice, not a positive edge slice.

2. Direct typed local receiver alias:
   - Extend one exact local alias propagation case for method receivers only if it reuses existing initializer proof.
   - Example shape: `let source = LocalAssoc; let value = source; value.instance_value()`.
   - Do not generalize through arbitrary expressions or multi-hop type inference.

3. Direct closure return blocker:
   - Preserve `make_closure()()` or returned closure values as targetless unless the returned path is already proven to a function item.
   - Add source-oracle and proof rows that explain the missing return-value callable proof.

4. Function pointer field blocker:
   - Use the existing memchr real-corpus fallback as a blocker proof target.
   - Assert owner/source-line fanout and targetless status rather than resolving callable fields.

## Implementation Order

1. Select one source shape from the candidate list and record why it is the next bucket.
2. Find or add the smallest fixture/real-corpus source oracle for that shape.
3. Extend parser extraction only if the current typed call payload cannot represent the oracle.
4. Extend resolver only when the parser payload carries exact proof.
5. Extend DB/RAG/TUI tests only after parser/transform facts exist.
6. Update the coverage matrix once for the completed chunk.

## Non-Goals

- No broad trait dispatch.
- No workspace dependency-root import resolution without a workspace-level proof carrier.
- No silent conversion of unsupported targetless rows into local edges.
- No new catchall fixture files or duplicated helper stacks.
- No downstream-only workaround in DB/RAG/TUI for missing parser proof.
