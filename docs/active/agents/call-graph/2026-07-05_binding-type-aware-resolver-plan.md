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

Current completed bucket: local callable parameter proof.

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

Pick one, not all. Status notes below reflect the current committed matrix and
should prevent future resumes from reselecting already-covered shapes.

1. Function pointer parameter blockers and exact private single-caller proof -
   completed:
   - Keep `f()` / `(f)()` where `f: fn(...)` is an owner parameter targetless unless an initializer is available.
   - Add or verify parser/DB/RAG proof that the callable parameter is visible as `ValueBinding` / `LocalBinding` with no edge.
   - A bounded positive subset now resolves private helper parameter calls when
     the complete local caller set supplies exactly one proven callable target,
     for example `call_single_function_pointer_param(f: fn() -> i32) { f() }`
     called only as `call_single_function_pointer_param(local_target)`.
   - Public helpers, unproven argument expressions, missing arguments, and
     multi-target caller sets remain targetless; do not broaden this through
     API entrypoints or arbitrary interprocedural value flow.
   - DB, RAG, and tool assertions now preserve the resolved private
     single-caller edge to `local_target`, while public, opaque,
     missing-argument, and multi-target shapes remain explicit blockers.

2. Direct typed local receiver alias - completed:
   - Extend one exact local alias propagation case for method receivers only if it reuses existing initializer proof.
   - Example shape: `let source = LocalAssoc; let value = source; value.instance_value()`.
   - Do not generalize through arbitrary expressions or multi-hop type inference.

3. Direct closure return proof - completed:
   - Completed for `make_closure()()` when the maker's final expression is directly a closure literal with a single recorded closure executable owner.
   - Completed for `make_bound_closure()()` when the maker's final expression returns a local binding initialized by that single recorded closure executable owner.
   - Completed for `make_alias_bound_closure()()` when the maker's final expression returns a local alias chain that reaches a local binding initialized by that single recorded closure executable owner.
   - Preserve broader returned closure values as targetless unless the returned callable is proven to a function item, direct closure literal, direct local closure binding, or direct local alias chain to a closure binding.

4. Function pointer field blocker - completed:
   - Use the existing memchr real-corpus fallback as a blocker proof target.
   - Assert owner/source-line fanout and targetless status rather than resolving callable fields.

5. Import/re-export/glob completeness with explicit workspace type proof -
   completed:
   - Regenerated axum backup data resolves the `Body::empty` re-export/import
     target-centered set through direct parsed-workspace imports, local
     re-export imports, inherited `super::*` imports, closure-owned rows, and
     local-item rows.
   - Remaining source-oracle rows stay documented rather than guessed.

6. Next adjacent candidate:
   - Select from the coverage matrix parking lot rather than adding more
     import breadth by default.
   - Likely options are the downstream proof for the private single-caller
     parameter case, one bounded adjacent local binding/type proof shape, or an
     explicit workspace proof carrier for one documented dependency-root source
     oracle.

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
