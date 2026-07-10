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

Current completed checkpoint: exact binding/type-aware fixture slices through
struct-pattern receiver bindings, async-closure future alias proof, private
callable-parameter proof, and parameter-field receiver proof have DB/RAG/TUI
coverage where exposed. The 2026-07-08 fixture regeneration plus DB
`real_target_matrix`, RAG `real_corpus`, and TUI integration `real_corpus`
checkpoints passed.

Exit criteria for the first implementation slice:

- one parser-side typed proof shape is added or extended;
- one transform/DB projection assertion proves the shape is persisted exactly;
- one RAG/tool assertion is added only if the shape is surfaced downstream;
- unsupported rows remain explicit when proof is missing;
- no broad trait dispatch, arbitrary dynamic callable execution, or workspace dependency-root import resolution is introduced.

Next phase if this bucket is done: choose a newly sourced unresolved
coverage-matrix bucket, or expand binding proof by one adjacent shape only when
existing parser-owned evidence can prove it without arbitrary value flow.

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
     the parameter type is a bare function pointer and the complete local caller
     set supplies exactly one proven callable target,
     for example `call_single_function_pointer_param(f: fn() -> i32) { f() }`
     called only as `call_single_function_pointer_param(local_target)`.
     The same proof is reused for dynamic syntax forms that still name the
     same bare function-pointer parameter: the parenthesized form
     `call_single_parenthesized_function_pointer_param(f: fn() -> i32) { (f)() }`
     and the cast form
     `call_single_function_pointer_param_cast(f: fn() -> i32) { (f as fn() -> i32)() }`
     both resolve to `DynamicFunction` edges only under the same private,
     bare-function-pointer, complete-single-caller constraints.
   - The same complete-local-caller boundary now admits one generic callable
     parameter shape when the private helper's parameter is a type parameter
     with an explicit callable trait bound and every local caller supplies the
     same exact function item. The fixture-backed
     `call_single_generic_fn_once_param<F>(generic_f: F) where F: FnOnce() ->
     i32 { generic_f() }` resolves to `local_target` only because its complete
     local caller set passes that function item. The parenthesized dynamic
     form `call_single_parenthesized_generic_fn_once_param<F>(generic_f: F)
     where F: FnOnce() -> i32 { (generic_f)() }` reuses the same proof
     boundary and resolves as a `DynamicFunction` edge, not broad
     callable-trait dispatch.
   - Public helpers, unproven argument expressions, missing arguments,
     multi-target caller sets, and callable-trait values without complete
     private caller proof remain targetless; do not broaden this through API
     entrypoints, callable-trait dispatch, or arbitrary interprocedural value
     flow.
   - DB, RAG, and tool assertions now preserve the resolved private
     function-pointer single-caller edges to `local_target` for path `f()`,
     dynamic `(f)()`, and dynamic `(f as fn() -> i32)()` call forms, plus the
     bounded private single-caller generic `FnOnce` path and parenthesized
     dynamic edges. Public callable-trait, opaque, missing-argument, and
     multi-target shapes remain explicit blockers.
   - The same complete-local-caller boundary now has one adjacent constructed
     argument proof for private indexed field-parameter calls:
     `call_single_indexed_field_function_param(holder: CallbackArrayHolder)
     { holder.callbacks[0]() }` and
     `call_single_indexed_tuple_field_function_param(holder:
     TupleCallbackArrayHolder) { holder.0[0]() }` resolve only because each
     private helper has one local caller that constructs the holder with
     `local_target` in the indexed field slot. Public holder parameters and
     arbitrary constructed/value-flow cases remain targetless. Parser, DB,
     RAG, and lookup/edges tool assertions now preserve these exact indexed
     holder-parameter `DynamicFunction` edges and the wrapper helper path rows
     that make the single-caller proof auditable downstream.
   - The same complete-local-caller boundary now has one private multi-caller
     constructed-holder named-field proof:
     `call_multi_named_field_function_param(holder: CallbackHolder) {
     (holder.callback)() }` resolves only because both local callers construct
     `CallbackHolder { callback: local_target }`. Its paired conflicting
     fixture, `call_multi_conflicting_named_field_function_param`, has callers
     that pass different function items and remains targetless with a
     `dynamic_dispatch_unbounded` blocker. Parser, DB, RAG, and
     `request_code_context` assertions preserve both the exact positive edge
     and the fail-closed blocker without broad holder/value-flow dispatch.

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

6. Exact local closure-binding deref proof - completed:
   - `call_dereferenced_closure_binding` now records `(*closure)()` as a
     dereferenced local closure binding when the existing local binding proof
     already carries one closure executable owner.
   - The resolver reuses the existing `DynamicClosure` relation path for this
     exact proof shape.
   - Parser and DB tests assert the resolved closure edge, one-hop traversal,
     proof projection, and removal from targetless dynamic blocker tables.
   - Opaque dereferenced callable parameters and broader callable trait-object
     dispatch remain targetless/unsupported.

7. Immediate awaited async-closure literal proof - completed:
   - `call_awaited_async_closure_literal_with_body_call` records
     `(async || local_target())().await` as an awaited async-closure dynamic
     call only when the closure call expression is the immediate base of
     `.await`.
   - The resolver emits `CallRelation::DynamicClosure` to the async-closure
     executable owner, so owner traversal can continue through the separately
     owned closure-body `local_target()` row.
   - The existing non-awaited `(async || local_target())()` case remains
     explicit `Unsupported` with no direct edge because constructing the future
     does not prove that it is polled.
   - Parser and DB tests assert the typed callee, non-flattened body owner, and
     two-hop outer function -> async-closure owner -> `local_target` traversal.

8. Direct and typed tuple-pattern local receiver proof - completed:
   - `call_tuple_pattern_local_instance_method` now records
     `let (value, _) = (LocalAssoc, 0); value.instance_value()` as an
     `InitializedLocalBinding` receiver by reusing existing per-element local
     binding proof for direct same-arity tuple expressions.
   - `call_typed_tuple_pattern_local_instance_method` now records
     `let (value, _): (LocalAssoc, i32) = make_local_assoc_pair();
     value.instance_value()` as a `TypedLocalBinding` receiver by using the
     explicit tuple type annotation for the destructured element. The helper
     initializer call is preserved as a separate resolved path edge.
   - Parser, DB receiver decode, DB proof projection, and RAG call-context
     assertions preserve the exact `LocalAssoc` initializer/type evidence and
     the local method target edge.
   - A bounded local method-result tuple destructuring shape is now completed:
     `call_method_tuple_return_pattern_local_instance_method` records
     `let (next, _) = value.tuple_pair(); next.instance_value()` as a
     `TupleMethodReturn` receiver carrying the initializer method name, exact
     initializer method-call span, and tuple element index. The resolver uses
     that exact initializer callsite to resolve the initializer method target,
     extracts the selected tuple element return type, and resolves the outer
     receiver method from that type.
   - Parser, transform/DB receiver projection, raw and structured DB receiver
     decode, DB proof rows, RAG call-context collection, and TUI formatter
     assertions preserve the exact method tuple-return receiver payload.
   - Arbitrary destructuring, external method-result tuple destructuring such
     as `let (parts, body) = Request::new(()).into_parts();`, nested value-flow,
     and tuple patterns without direct tuple-expression initializers or exact
     local initializer method proof remain out of scope unless an explicit
     local type annotation supplies a per-element type proof. The regenerated
     axum `request_parts.rs:164` row now preserves the
     `TupleMethodReturn(parts, into_parts, index 0)` receiver payload and
     resolves through the exact external tuple-return summary proving
     `http::Request::into_parts` returns `http::request::Parts` at tuple
     index 0.

9. External `Service`-bound self-field receiver frontier - completed:
   - Regenerated axum `self.inner.poll_ready(cx)` and `self.0.poll_ready(cx)`
     forwarding rows now classify as targetless `External` frontier rows when
     the receiver field type is either a concrete external service receiver
     type or a generic parameter with a source-visible external `Service`
     bound.
   - The resolver reuses existing self-field type proof and generic-bound
     source collection. It does not fabricate a local `tower_service::Service`
     target or broaden concrete trait dispatch.
   - DB real-corpus assertions pin seven `SelfField(inner)` rows and three
     `SelfField(0)` rows over the regenerated axum call-graph fixture.
   - Broader external trait methods, dynamic dispatch, and async poll/resume
     effect edges remain out of scope.

10. Borrowed value-parameter receiver and method-result proof - completed:
   - `call_borrowed_value_param_instance_method(value: LocalAssoc) {
     (&value).instance_value() }` reuses exact parameter receiver proof for a
     borrowed by-value parameter.
   - `call_borrowed_value_param_method_result_instance_method(value:
     LocalAssoc) { (&value).clone_assoc().instance_value() }` now uses the same
     borrowed parameter proof for the inner method and existing return-type
     proof for the outer `MethodCallResult` receiver.
   - Parser, DB owner/target-centered receiver/proof rows, and RAG
     call/proof-context assertions cover the exact fixture-backed shape.
   - Broader borrowed receiver chains without exact parameter type proof remain
     unsupported.

11. Workspace re-exported external receiver proof - completed:
   - A focused transform workspace test builds a temporary two-member
     workspace where a selected dependency re-exports an external type alias
     (`pub type Request<T = ()> = http::Request<T>`) and a dependent crate
     calls `req.extensions_mut()` through `use provider::Request`.
   - The method resolver follows the existing workspace type proof to the
     parsed dependency alias and reuses the associated-path alias external
     predicate to classify the receiver as targetless `External`.
   - This does not make dependency-root imports traversable and does not guess
     concrete external targets; unresolved generic receiver rows without exact
     workspace alias proof remain targetless.

12. Bounded local try-method result receiver proof - completed:
   - `call_try_method_result_instance_method()` records
     `value.try_clone_assoc()?.try_instance_value()` as a local method-result
     chain where the outer receiver is
     `TryMethodCallResult { method_name: "try_clone_assoc" }`.
   - The resolver first resolves the direct inner `try_clone_assoc` method,
     reads that method's return type, unwraps its `Result<LocalAssoc, ()>` Ok
     type through the existing result proof path, and then reuses exact local
     instance-method lookup for `try_instance_value`.
   - Parser, transform/DB receiver projection, raw and structured receiver
     decode, DB proof rows, RAG call-context collection, and
     `request_code_context` assertions cover the exact fixture-backed shape.
   - Opaque external `?` chains and method-result receivers without exact inner
     method return-type proof remain targetless or unsupported.

13. Immediate awaited async-closure binding proof - completed:
   - Parser local binding proof now records whether a visible closure binding
     is async, so `closure()` can fail closed when the returned future is not
     immediately awaited.
   - `call_async_closure_binding_without_await_with_body_call()` remains an
     unsupported, targetless path call from the outer function while the
     async-closure executable owner still owns the body `local_target()` call.
   - `call_awaited_async_closure_binding_with_body_call()` records
     `closure().await` as an awaited async-closure binding path call and emits
     a local exact `Closure` relation to the async-closure executable owner.
   - Parser, transform/DB, DB traversal, RAG call-context collection, and
     exact `request_code_context` tool assertions cover the supported and
     fail-closed shapes.
   - Direct same-block future bindings are covered by the next completed
     bucket. Returned futures, arbitrary async-callable values,
     dereferenced/cast async closures, and general poll/resume semantics remain
     unsupported.

14. Same-block awaited async-closure future binding proof - completed:
   - `call_async_closure_future_binding_without_await_with_body_call()`
     records `_future = closure()` as an unsupported targetless async-closure
     binding path call because constructing the future does not prove that it
     is polled.
   - `call_awaited_async_closure_future_binding_with_body_call()` records the
     earlier `closure()` call as awaited when the same block later executes
     `future.await`.
   - Parser extraction uses only direct same-block evidence:
     `let future = closure(); future.await;`. It does not model nested control
     flow, arbitrary future value flow, returned futures, async callable trait
     objects, or general poll/resume semantics.
   - Parser, transform/DB, DB traversal, RAG call-context collection, and
     exact `request_code_context` tool assertions cover the supported and
     fail-closed shapes.

15. Same-block awaited async-closure future alias proof - completed:
   - `call_awaited_async_closure_future_alias_with_body_call()` records the
     original `closure()` call as awaited when the same block executes
     `let future = closure(); let alias = future; alias.await;`.
   - `call_awaited_async_closure_future_block_alias_with_body_call()` records
     the same awaited edge when the alias initializer is a single-expression
     block containing the previously recorded future binding:
     `let future = closure(); let alias = { future }; alias.await;`.
   - Parser extraction uses only one-step same-block alias evidence from a
     previously recorded future binding, including the single-expression block
     form already used by nearby call extraction helpers. It does not model
     nested control flow, arbitrary future value flow, returned futures, async
     callable trait objects, or general poll/resume semantics.
   - Parser, DB traversal, RAG call-context collection, and exact
     `request_code_context` tool assertions cover the direct alias shape;
     parser, DB traversal, and RAG call-context assertions cover the
     block-alias shape.

16. Private parenthesized generic `FnOnce` single-caller proof - completed:
   - `call_single_parenthesized_generic_fn_once_param<F>(generic_f: F) where
     F: FnOnce() -> i32 { (generic_f)() }` reuses the complete private
     single-caller callable-parameter proof boundary from the regular
     `generic_f()` form.
   - The dynamic row resolves to `local_target` only because every local caller
     supplies that exact function item; public helpers, unproven argument
     expressions, multi-target caller sets, and broader callable-trait dispatch
     remain targetless.
   - Parser, DB traversal/caller queries, RAG call-context collection, and
     exact `request_code_context` tool assertions batch the regular path and
     parenthesized dynamic forms under the same fixture-backed proof.

17. Branch-initialized local receiver proof - completed:
   - `call_if_initialized_local_instance_method()` records
     `let value = if flag { LocalAssoc } else { LocalAssoc };
     value.instance_value()` as an `InitializedLocalBinding` receiver because
     every branch arm proves the same exact local initializer path.
   - `call_match_initialized_local_instance_method()` records the equivalent
     `match` initializer shape through the same proof carrier.
   - Parser extraction reuses the existing `branch_init_path` local-binding
     proof and does not add a new receiver kind. Mixed, opaque, or unproven
     branch initializers do not become initialized local receivers.
   - Parser, DB owner/proof/target-centered rows, RAG call-context
     collection, and exact `code_item_lookup` / `code_item_edges` tool
     assertions cover the exact fixture-backed shapes.

18. Next adjacent candidate:
   - Select from the coverage matrix parking lot rather than adding more
     import breadth by default.
   - Already audited candidates should not be reselected as simple parser
     slices: real-corpus routing helper rows require macro/cfg evidence,
     axum callable fields and proc-macro callback rows require broader
     interprocedural value flow. The axum-core request-parts turbofish row is
     no longer a candidate: it now resolves through the exact
     `http::Request::into_parts` tuple-return summary.
   - Likely remaining options are a broader async poll/resume proof carrier, a
     new explicitly reviewed dependency-root source oracle, or another bounded
     local binding/type proof only if it reuses existing parser-owned evidence
     without arbitrary interprocedural value flow.

19. Post-request-parts candidate audit:
   - The focused unsupported fixture inventory still has no safe promotion
     candidate without a new proof input. Public callable parameters and
     public callable fields lack complete source-visible caller proof;
     conflicting caller sets intentionally stay blocked; non-awaited async
     callable values lack poll/resume evidence; ambiguous local callable
     initialization and missing trait visibility remain fail-closed.
   - The real-corpus routing helper rows remain a macro/generated-code
     boundary problem, not a same-module path-resolution bug: the
     `fallback_endpoint` callsites live inside `tap_inner!` input, and the
     debug-only `super::take_route_or_internal_error` test owner is not present
     in the current fixture.
   - Existing external-summary proof surfaces already cover the DB/RAG/TUI
     owner-need and admission pattern for Request::builder,
     std::mem::replace, and feature-gated serde_json. Do not add
     Body::size_hint or Route::oneshot summary rows unless there is a new
     usage-question surface gap or an explicit summary artifact plan.
   - Next code work should therefore start only after selecting one of:
     a real async poll/resume proof carrier, a reviewed dependency-root source
     oracle with missing proof payload, or a bounded local binding/type proof
     shape not already covered in parser/DB/RAG/TUI tests.

## Remaining Focused Unsupported Inventory

Status checkpoint: 2026-07-09 after the active call-graph corpus fixtures were
regenerated and `xtask --features call_graph verify-backup-dbs` passed.

The focused parser call-site suite has a small remaining set of
`ExpectedCallOutcome::Unsupported` rows. These should not be treated as the
next implementation target unless the missing proof input below is supplied.

| Group | Representative tests | Why it remains fail-closed |
| --- | --- | --- |
| Macro calls | `fixture_nodes_use_imported_items_records_documented_macro_call_site`, `fixture_macros_use_local_macro_records_local_macro_call_site`, `fixture_call_graph_assert_eq_macro_call_records_test_body_macro_call_site` | Macro expansion bodies and generated call edges are not modeled as source call graph edges. |
| Public callable parameters | `call_function_pointer_param`, `call_parenthesized_function_pointer_param`, `call_function_pointer_param_cast`, `call_generic_fn_once_value_binding`, `call_parenthesized_generic_fn_once_value_binding` | Public API callers do not give a complete source-visible argument set, so no local callee can be proven. |
| Conflicting callable caller sets | `call_multi_conflicting_function_pointer_param`, `call_multi_conflicting_generic_fn_once_param`, `call_multi_conflicting_named_field_function_param` | Complete local callers exist but pass different callable targets, so the row must stay targetless with blocker proof. |
| Public callable fields and arrays | `call_field_function_param`, `call_indexed_field_function_param`, `call_indexed_tuple_field_function_param`, `call_indexed_function_pointer` | Parameter field/index values lack exact single-caller or initializer proof at public API boundaries. |
| Non-awaited async callable values | `call_async_closure_binding_without_await_with_body_call`, `call_async_closure_future_binding_without_await_with_body_call` | Constructing an async-closure future does not prove poll/resume execution. |
| Ambiguous local callable initialization | `call_if_ambiguous_initialized_function_item_binding` | Branch proof reaches multiple possible function items, so no exact edge can be emitted. |
| Missing trait visibility | `call_unimported_trait_method` | The receiver type is local, but the trait method is not visible in the call scope. |

The nearby completed positive rows already cover private complete caller sets,
same-target multi-caller sets, branch/match same-parameter forms, typed local
function items, boxed callable initializers, returned closures, and exact local
receiver proof. The next semantic slice should therefore introduce a new proof
carrier, or add one explicitly sourced real-corpus/dependency-root oracle,
rather than reworking these fail-closed parser rows.

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
