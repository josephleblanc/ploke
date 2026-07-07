# 2026-07-01 Call Graph Goal Coverage Matrix

Short description: workflow guardrail and status matrix for implementing the call-graph usage-question goal without over-focusing on the latest local concern.

Related planning files:
- [`2026-07-01_call-graph-larger-plan-map.md`](2026-07-01_call-graph-larger-plan-map.md)
- [`../2026-06-30_call-graph-usage-questions.md`](../2026-06-30_call-graph-usage-questions.md)
- [`2026-06-25_call-graph-coverage-inventory.md`](2026-06-25_call-graph-coverage-inventory.md)
- [`2026-06-28_real-corpus-call-site-case-matrix.md`](2026-06-28_real-corpus-call-site-case-matrix.md)
- [`2026-06-28_real-corpus-call-site-oracle-matrices.md`](2026-06-28_real-corpus-call-site-oracle-matrices.md)

Status date: 2026-07-07
Baseline HEAD when created: `19860cc40`

## Operating Rule

Work must be coverage-matrix driven, not attention driven. The active bucket is only the next bounded slice, not the whole goal. Once a bucket meets its "done for now" threshold, switch to the next uncovered bucket unless there is a correctness failure or broken downstream contract.

Before starting a new call-graph slice, check this document and state:

```text
Current bucket:
Exit criteria:
Status:
Next bucket:
Reason to stay in current bucket:
```

If the reason to stay is only "there is another nice improvement nearby", write it in the parking lot and switch buckets.

## Switch Criteria

A supported bucket is done for now when it has:

- one real-corpus positive proof when a real target case exists;
- one DB assertion over the persisted call graph;
- one RAG propagation assertion if the data is exposed through RAG;
- one TUI/tool assertion if the data is exposed through a tool;
- one consolidated doc note for the chunk.

An unsupported bucket is done for now when it has:

- one real-corpus or fixture-backed source oracle;
- a fail-closed assertion that no traversal edge is invented;
- a visible blocker/frontier/proof row when the model stores one;
- a short reason in the matrix explaining what implementation capability is missing.

No bucket should receive more than four consecutive commits without re-checking this matrix and either switching buckets or recording a concrete reason to stay.

## Current And Recent Buckets

Current bucket: none selected after the memchr callable trait-object
targetless propagation slice.

Latest completed bucket: downstream targetless propagation for real-corpus
callable trait-object path rows.

Completed evidence:

- DB already pins the memchr `Runner::run` rows at
  `memchr/src/tests/substring/mod.rs:94,110` as unsupported, targetless path
  rows for `fwd(...)` and `rev(...)`, with no dynamic rows and no fabricated
  edge to the boxed setter closures.
- RAG call-context tests now assert the same two owner-scoped rows preserve
  their observed paths, argument counts, unsupported status, and empty target
  sets.
- Exact `code_item_lookup` and `code_item_edges` table-drive both rows through
  the shared targetless path tool matrix using the memchr corpus proof domain,
  preserving blocked `type_resolution_missing` proof rows and zero call edges.
- This closes propagation for the stored fallback rows only. Binding from
  `Runner` fields to boxed `dyn FnMut` setter closures remains future
  callable-value/type-flow work.

Previously completed bucket: local callable-parameter alias proof for complete
private single-caller helpers.

Completed evidence:

- Added fixture-backed parser/resolver proof for
  `call_single_aliased_function_pointer_param(f: fn() -> i32) { let g = f; g() }`
  and
  `call_single_parenthesized_aliased_function_pointer_param(f: fn() -> i32) { let g = f; (g)() }`.
- The call row preserves the observed callee path `g`, while resolver proof
  follows the local alias back to parameter `f` and then through the existing
  complete private single-caller argument proof to `local_target`.
- DB context, target-centered callers, one-hop traversal, and proof projection
  now cover both path and dynamic alias forms. RAG call context covers both
  forms, and TUI lookup/edges cover the dynamic alias form through the existing
  resolved callable tool matrix.
- This remains bounded to simple local aliases of visible parameters or prior
  aliases. Arbitrary callable value flow, callable trait objects, and public or
  multi-caller parameter dispatch remain fail-closed.

Previously completed bucket: downstream targetless propagation for a real-corpus
dyn Future poll dispatch row.

Completed evidence:

- DB already pins the axum `HandleErrorFuture::poll` row at
  `axum/src/error_handling/mod.rs:251` as an unsupported, targetless
  trait-object dispatch site for `self.project().future.poll(cx)`.
- RAG call-context and proof-context tests now assert the same row remains
  visible as `CallReceiverInfo::Unsupported`, carries a blocked
  `type_resolution_missing` proof row, and has no fabricated local callee
  edge.
- Exact `code_item_lookup` and `code_item_edges` table-drive the same source
  oracle through the existing unsupported receiver tool matrix, alongside the
  axum-core request-parts turbofish receiver row. This closes downstream
  propagation only; async poll/resume semantics and runtime trait-object
  dispatch remain future modeling work.

Previously completed bucket: exact TUI lookup/edges coverage for a real-corpus
async-block executable owner.

Completed evidence:

- Exact `code_item_lookup` and `code_item_edges` now address the axum
  `Handler::call` nested `async_block` owner in
  `axum/src/handler/mod.rs:217` through the existing executable-owner lookup
  path (`node_kind=async_block`, `item_name=async_block`, `parent_name=call`).
- Both tool tests prove the nested async-block-owned `self()` path row and
  awaited `into_response()` method row remain `Unsupported`, targetless, and
  backed by projected blocker proof rows. No poll/resume or callable-binding
  edge is fabricated.
- This closes the TUI/tool surface for an already DB/RAG-documented async
  boundary without adding parser/resolver breadth.

Previously completed bucket: fixture-backed FFI external-frontier reach proof.

Completed evidence:

- DB `call_reach_for_owner` now has fixture-backed coverage for
  `call_extern_c_function`, proving `abs(value)` from the
  `unsafe extern "C"` declaration is visible in both `frontier_calls` and
  `external_frontier_calls` while producing no local traversal edge.
- RAG `exact_call_reach_for_owner` preserves the same external targetless
  frontier row, argument count, and fixture source-file metadata.
- This chunk does not add unsafe-function metadata or parser/schema breadth.
  Standalone `FunctionNode` / `MethodNode` unsafe qualifiers are still not
  stored in the primary node schema, so true "paths to unsafe function items"
  remains a future metadata/modeling bucket rather than something inferred
  from call graph edges.

Previously completed bucket: downstream DB/RAG proof coverage for
parser-supported fixture call shapes.

Completed evidence:

- Added DB context/proof assertions for existing parser-resolved fixture rows
  that previously had less downstream coverage: `crate::file_mod::file_module_target()`,
  `super::qualified_assoc_scope::NestedAssoc::make()`,
  `TraitMethodPath::handle(value)`, and generic-bound associated paths
  `T::make(...)` in inline and `where`-bound forms.
- Extended the shared constructor proof table with
  `EnumWithInherentImpl::Case(value)` and the type-alias variant constructor
  `AliasConstructorType::Case(value)`, so constructor context, target
  expansion, target-centered proof, and node proof tests all cover those
  endpoint shapes without duplicated helper logic.
- Added RAG call-context coverage for the same two fixture-backed enum
  variant constructor rows.
- This chunk did not add parser/resolver breadth. It closes downstream proof
  gaps for semantics already proven by the parser call-site suite and keeps
  unsupported rows unchanged.

Previously completed bucket: untyped tuple-pattern receiver from local function
tuple return.

Completed evidence:

- `call_tuple_return_pattern_local_instance_method()` now proves
  `let (value, _) = make_local_assoc_pair(); value.instance_value()` by using
  the local helper function's tuple return type for element `0`.
- Parser extraction records the receiver as
  `TupleReturnBinding { name: "value", path: ["make_local_assoc_pair"], index: 0 }`,
  while still preserving the initializer `make_local_assoc_pair()` path call
  as a separate resolved function edge.
- Transform/DB projection, raw and structured DB receiver decode, resolved
  proof rows, RAG call-context collection, and TUI call-context formatting
  preserve the tuple-return receiver payload.
- The existing `request_code_context` method-caller table now includes the
  tuple-return owner, proving tool payload propagation for the same receiver
  while preserving the separate initializer helper path row.
- Focused parser, DB context/proof/decode, RAG, and TUI tests passed, and
  `cargo run -p xtask --features call_graph -- fixtures regenerate --active`
  round-tripped all active registered fixtures with no tracked fixture drift.
- This remains bounded to local parsed functions with source-visible tuple
  return types. External function calls, method calls returning tuples, and
  arbitrary expression-produced tuple destructuring remain unsupported until
  broader binding/type-flow proof exists.

Previously completed bucket: TUI tool surface for immediately awaited
async-closure literal traversal.

Previous evidence:

- `request_code_context` asserts
  `call_awaited_async_closure_literal_with_body_call()` surfaces the outer
  dynamic call to the `async_closure` executable owner and then the
  closure-owned `local_target()` body call, matching the existing DB/RAG
  two-hop traversal contract for `(async || local_target())().await`.
- This is a downstream propagation proof over existing parser/DB/RAG facts,
  not a new resolver branch. Non-immediate async-closure futures and broader
  poll/resume value flow remain targetless/unsupported until there is an
  explicit future-binding/effect model.
- Focused TUI verification passed with
  `cargo test -p ploke-tui request_code_context_returns_awaited_async_closure_literal_owner_call_context -- --nocapture`.

Previously completed bucket: bounded local try-method result receiver proof.

Previous evidence:

- `call_try_method_result_instance_method()` now proves
  `value.try_clone_assoc()?.try_instance_value()` as two local method edges:
  the inner `try_clone_assoc` call resolves from the typed local `value:
  LocalAssoc`, and the outer `try_instance_value` call resolves from the
  inner method's `Result<LocalAssoc, ()>` Ok return type.
- Parser extraction records the outer receiver as
  `TryMethodCallResult { method_name: "try_clone_assoc" }`; transform/DB
  projection, raw and structured receiver decode, RAG call-context collection,
  and `request_code_context` all preserve that exact receiver payload.
- Focused parser, DB proof/query, RAG, and TUI tests passed, and
  `cargo run -p xtask --features call_graph -- fixtures regenerate --active`
  round-tripped all active registered fixtures.
- This remains a local proof shape only. Opaque external `?` chains and
  method-result receivers without exact inner method return-type proof stay
  targetless/unsupported rather than being guessed.

Previously completed bucket: TUI tool surface for same-target branch receiver
proof.

Previous evidence:

- `code_item_lookup` and `code_item_edges` now assert both
  `call_if_expression_receiver_method(flag)` and
  `call_match_expression_receiver_method(flag)` deserialize resolved local
  method call context plus target-centered `call_site`, `call_edge`, and
  `call_resolution` proof rows for `LocalAssoc::instance_value`.
- This closes the previous parser/DB/RAG-only receiver caveat without adding a
  new resolver branch.

Earlier completed bucket: same-target match-expression receiver proof.

Earlier evidence:

- `call_match_expression_receiver_method(flag)` now proves
  `(match flag { true => LocalAssoc, false => LocalAssoc }).instance_value()`
  as an exact local method edge to `LocalAssoc::instance_value`.
- Parser extraction reuses the existing branch-path receiver carrier, DB
  method context/proof rows preserve the decoded branch paths, and RAG
  call/proof context exposes the same resolved target-centered method row.

Earlier completed bucket: private named-field callable-parameter proof.

Earlier evidence:

- `call_single_named_field_function_param(holder: CallbackHolder)` proves
  `(holder.callback)()` as a private complete-single-caller parameter field
  edge to `local_target` when its only local caller constructs
  `CallbackHolder { callback: local_target }`.
- Parser, DB dynamic context/proof, DB path traversal, RAG call/proof context,
  and exact TUI lookup/edges tests cover the shape without adding a parallel
  resolver branch.

Next candidate bucket: choose the next bounded semantic-expansion source oracle
from the matrix. Do not return to dynamic callable values or branch receiver
polishing unless these proofs regress.

Reason to stay in the current receiver bucket: none after focused parser, DB,
RAG, and TUI verification. Switch buckets.

Exit criteria:

- Reuse the existing parameter receiver proof path rather than adding a parallel
  resolver branch.
- Resolve `(&value).clone_assoc().instance_value()` when the inner
  `clone_assoc` call uses a borrowed by-value local parameter whose type
  exactly names a local receiver type.
- Prove parser status, DB receiver/proof/query rows, target-centered caller
  traversal, and RAG call-context propagation.

Completed evidence:

- `MethodCallReceiver::BorrowedLocalBinding` now flows through the same
  exact parameter method resolver used by direct local parameter receivers.
- The fixture case
  `call_borrowed_value_param_instance_method(value: LocalAssoc)` with
  `(&value).instance_value()` resolves to `LocalAssoc::instance_value`.
- Parser paranoid call-site tests, DB receiver/proof/target-centered caller
  tests, and RAG call-context expansion tests cover the new shape.
- `resolve_method_call_target` now mirrors the top-level receiver handling for
  `BorrowedLocalBinding`, so method-result receiver chains can reuse exact
  borrowed parameter proof instead of falling back to unsupported.
- The fixture case
  `call_borrowed_value_param_method_result_instance_method(value: LocalAssoc)`
  with `(&value).clone_assoc().instance_value()` resolves both
  `clone_assoc` and the final `instance_value` edge.
- Active fixture regeneration also exposed the chrono
  `src/offset/local/unix.rs:159` `MappedLocalTime::Single(offset)` row; the
  real-corpus oracle now expects 12 alias constructor rows.

Additional completed bucket: workspace re-exported external receiver proof.

Completed evidence:

- A focused transform workspace test now builds a temporary two-member
  workspace where `consumer` imports `provider::Request`, `provider` defines
  `pub type Request<T = ()> = http::Request<T>`, and the consumer calls
  `req.extensions_mut()`.
- The method resolver follows the selected workspace type proof to the parsed
  provider type alias and classifies the receiver method as targetless
  `External`, without turning dependency-root imports into local traversal
  edges.
- Active fixture regeneration after the resolver change round-tripped every
  active snapshot and produced no tracked fixture drift; current axum
  real-corpus external-frontier tests remain stable.

Additional completed bucket: std-root external path frontier propagation.

Completed evidence:

- The DB real-corpus matrix already pins the currently projected
  `std::mem::replace` rows as targetless `External` path frontiers at
  `axum/src/error_handling/mod.rs:138` and
  `axum/src/response/sse.rs:449`.
- RAG call-context collection now asserts the `EventDataWriter::write_buf`
  owner preserves the `std::mem::replace(&mut self.data_written, true)` row as
  an external targetless frontier with two value arguments.
- Exact TUI `code_item_lookup` and `code_item_edges` external path-frontier
  tests now table-drive both `Request::builder` and `std::mem::replace`,
  preserving call-context and proof-context payloads without creating local
  traversal edges.

Additional completed bucket: component source-crate usage summaries.

Completed evidence:

- `CallImpactReport` and `CallReachReport` now carry `source_crates` alongside
  existing `source_files` and `source_modules`, derived from
  `file_mod.namespace -> crate_context.name` for every node participating in
  the bounded traversal summary.
- The real-corpus axum `Body::empty` component-impact DB test now proves the
  report identifies both selected workspace crates involved in the impact set:
  `axum-core` and `axum`.
- RAG exact impact propagation and the `code_item_lookup` /
  `code_item_edges` TUI surfaces preserve the same source-crate payload and UI
  count, directly supporting build/deployment and component-impact questions
  without changing resolved-only traversal or frontier semantics.

Additional completed bucket: feature/platform cfg usage summaries.

Completed evidence:

- `CallImpactReport` and `CallReachReport` now carry `source_cfgs` alongside
  existing source-file/module/crate usage-summary metadata, derived from the
  callsite rows that participate in direct call context, frontier rows, and
  bounded resolved paths.
- The axum real-corpus listener reach test proves the two
  `axum/src/serve/listener.rs` `Self::accept(self).await` owners stay
  targetless external frontiers while exactly the `#[cfg(unix)]`
  `UnixListener` owner reports the `unix` cfg.
- DB call-context rows now enrich callsite cfgs with same-namespace declaration
  module cfgs for file-based module contents, so `axum/src/json.rs`
  `Json::from_bytes` carries the parent `#[cfg(feature = "json")] mod json;`
  feature gate without changing parser callsite IDs.
- The axum real-corpus `Json::from_bytes` external-frontier reach test proves
  `serde_json::Deserializer::from_slice(bytes)` remains targetless external
  while the reach summary and backing DB callsite rows preserve
  `feature = "json"`.
- RAG exact reach propagation and the `code_item_lookup` / `code_item_edges`
  TUI surfaces preserve source-cfg payload/count without inventing traversal
  edges for external frontiers; RAG exact reach now also preserves the
  `Json::from_bytes` feature-gated external frontier summary, and lookup/edges
  tests assert the same feature-cfg reach payload/count.

Additional completed bucket: owner-scoped architecture boundary query.

Completed evidence:

- `module_boundary_edges_from_owner(owner, options)` lists resolved bounded
  call-path edges whose caller and callee module paths differ, reusing the same
  resolved-only traversal semantics as `call_paths_from_owner`.
- The axum real-corpus architecture-review test proves
  `RequestExt::extract -> extract_with_state -> FromRequest::from_request`
  reports the cross-module `extract_with_state -> from_request` edge while not
  reporting the same-module `extract -> extract_with_state` edge.
- The same test cross-checks the new owner-scoped helper against the existing
  `call_reach_for_owner(...).boundary_edges` payload.

Additional completed bucket: direct recursion path traversal.

Completed evidence:

- Bounded path traversal now records a resolved edge before applying cycle
  expansion guards, so direct self-recursive source calls are visible as
  one-edge paths instead of being dropped.
- The fixture-backed `recursive_fixture_call(depth)` oracle proves
  `recursive_fixture_call(depth - 1)` resolves to the same function, appears
  in `call_paths_from_owner`, `call_paths_to_target`, `call_paths_between`,
  `call_reach_for_owner`, and `call_impact_for_target`, and does not expand
  beyond the direct cycle edge.
- RAG exact path collection preserves the same one-edge recursive path through
  `exact_call_paths_from_owner`, `exact_call_paths_to_target`, and
  `exact_call_paths_between` without adding its own traversal semantics.
- The traversal remains resolved-edge-only; targetless frontier rows are not
  promoted into cycle paths.

Additional completed bucket: owner-recursion cycle query.

Completed evidence:

- `call_cycles_from_owner(owner, options)` now exposes the bounded resolved
  paths that start at an owner and return to that same owner by filtering the
  existing `call_paths_from_owner` result instead of adding a second traversal
  algorithm.
- The fixture-backed `recursive_fixture_call(depth)` oracle proves the cycle
  helper returns the same one-edge recursive path as the owner, target, and
  between traversal APIs.
- `RagService::exact_call_cycles_from_owner(...)` preserves the DB result as a
  thin exact wrapper, including the same path node metadata, so RAG consumers
  can answer recursion/cycle questions without recomputing graph traversal.
- Exact `code_item_lookup` and `code_item_edges` payloads expose the same
  owner-recursion cycle paths and UI counts for
  `recursive_fixture_call(depth)`, so tool callers can answer the recursion
  question directly.
- The helper remains resolved-edge-only and does not convert targetless
  frontier rows into cycle evidence.

Reason to stay in this bucket: none after focused verification. Switch buckets
unless the workspace re-export alias proof regresses.

Completed bucket, 2026-07-07: direct array-parameter callable proof.

- Fixture-backed `call_single_indexed_function_pointer_param(funcs: [fn() -> i32; 1])`
  now resolves `funcs[0]()` to `local_target` when its only local caller
  supplies `[local_target]`.
- Parser extraction records the direct array argument as path-valued element
  proof, while the existing public `call_indexed_function_pointer(funcs)`
  remains fail-closed and targetless.
- DB owner context, DB proof projection, RAG call/proof context, and exact
  `code_item_lookup` / `code_item_edges` tool tests preserve the same resolved
  `DynamicFunction` edge.

Completed bucket, 2026-07-07: nested self-field receiver proof.

- Fixture-backed
  `NestedSelfFieldAssocOwner::call_nested_self_field_instance_method(&self)`
  now resolves `self.inner.value.instance_value()` to
  `LocalAssoc::instance_value`.
- The parser already records the full receiver path as
  `SelfField { ["inner", "value"] }`; the resolver now walks each segment
  through exact local struct-field type evidence and stays unsupported if any
  step cannot be proven as a single local struct field.
- DB owner context, target-centered proof projection, RAG call/proof context,
  and exact `code_item_lookup` / `code_item_edges` tool tests preserve the same
  resolved `Method` edge.

Completed bucket, 2026-07-07: same-parameter branch/match callable proof.

- Fixture-backed private helpers
  `call_single_if_function_pointer_param_branch(flag, f)` and
  `call_single_match_function_pointer_param_arm(flag, f)` now resolve
  `(if flag { f } else { f })()` and
  `(match flag { true => f, false => f })()` to `local_target` when their only
  local callers supply `local_target`.
- Parser extraction records these as parameter-specific dynamic callee carriers
  instead of item-path branch carriers, so the existing public
  `call_if_function_pointer_param_branch` and
  `call_match_function_pointer_param_arm` rows remain path-aware unsupported
  blockers with no fabricated target.
- Transform projection, DB traversal/proof rows, RAG call-context collection,
  and `request_code_context` all preserve the exact `DynamicFunction` edge.

## Coverage Matrix

| Bucket | Current status | DB proof | RAG proof | TUI/tool proof | Real-corpus target | Next action |
| --- | --- | --- | --- | --- | --- | --- |
| Method / trait-method multi-hop | Met for now | `RequestExt::extract -> extract_with_state -> FromRequest::from_request` two-hop paths and summaries | Exact call paths, expansion, impact, reach | `code_item_lookup` two-hop payload and UI counts | axum | Do not deepen by default; switch buckets unless a regression appears. |
| Regular free-function one-hop | Covered | `parse_attrs`, `run_ui_tests`, and related target fanout assertions | Exact call-context tests for current real-corpus function callers | `code_item_lookup` function caller regressions | axum | Use as source pool for finding a free-function multi-hop chain. |
| Regular free-function multi-hop | Met for now | `from_request::expand -> impl_struct_by_extracting_each_field -> extract_fields` ordered two-hop traversal through `call_paths_between`, owner, and target path APIs | Exact call paths expose the same ordered function chain and source-node metadata | `code_item_call_path` returns the same real-corpus function reachability path | axum | Switch buckets; do not add more free-function breadth by default. |
| Inherent method one-hop | Covered/partial | Examples include `Json::from_bytes` and related method/associated-function rows | Exact call-context propagation exists | Exact lookup regressions exist | axum | Revisit only after broader buckets have at least one proof. |
| Exact local external-trait impl receiver methods | Met for now | 13 `Router::clone` typed-local/self-field receiver rows resolve to `impl<S> Clone for Router<S>::clone`; axum-core `parts.extract_with_state(state)` resolves through imported `http::request::Parts` receiver proof to `RequestPartsExt for Parts::extract_with_state` | Exact call context preserves the same caller-site identities and receiver buckets | Remaining real-corpus TUI matrix includes `RouterClone` and the focused `RequestPartsExt` local receiver checks | axum | Switch buckets; do not broaden to arbitrary external trait dispatch by default. |
| Associated-function path calls | Met for now | `Self::from_bytes`, `E::from_request`, `MethodRouter::new` style rows; `RequestExt::extract -> extract_with_state -> FromRequest::from_request` proves a two-hop path whose terminal edge is `AssociatedFunction` | Exact paths, expansion, impact, and reach preserve associated-function relation/kind | `request_code_context`, `code_item_call_path`, and lookup/edges regressions carry associated-function path rows and callsite buckets | axum | Switch buckets; do not add a separate associated-function multi-hop row unless this exact contract regresses. |
| Constructors | Stronger one-hop | Tuple struct / enum variant constructor rows are asserted in real-corpus matrix, including chrono alias constructor rows; real-corpus `BoxedIntoRoute` `Self(...)` tuple constructors and fixture-backed method-owned `Self(...)` tuple constructors now resolve through the enclosing impl self type | Exact context propagation exists for direct, alias, and `Self(...)` constructor rows | Tool regressions include direct, alias, variant, and `Self(...)` constructor rows; lookup and edges both preserve the chrono `MappedLocalTime::Single` alias constructor caller identities | axum, chrono plus local fixture fallback | Switch buckets; add constructor breadth only when a new source shape is represented in the matrix. |
| External dependency frontier | Covered as frontier | External rows are exposed but not traversed; regenerated axum DB fixture classifies `Body::size_hint` `self.0.size_hint()` as an external targetless frontier through tuple-field receiver and local type-alias evidence for `Body(BoxBody)`, classifies fourteen `Request::builder` rows as external frontiers when `Request` resolves through the axum-core `Request = http::Request` alias, classifies selected `self.inner.poll_ready(cx)` / `self.0.poll_ready(cx)` forwarding rows as external frontiers when the receiver field type is either a concrete external service type or a generic parameter bounded by an external `Service` trait, classifies the two axum `Route::oneshot` receiver rows as external frontiers when `tower::ServiceExt` import evidence is source-visible, classifies chrono `StrftimeItems::queue.is_empty()` as an external targetless slice receiver frontier through the source-visible `&'static [Item<'static>]` field type, pins current `std::mem::replace` std-root path rows as targetless external frontiers, and fixture-backed DB reach now preserves the `unsafe extern "C"` `abs(value)` call in `external_frontier_calls` without fabricating a local edge | Reach summaries expose external frontier calls; RAG call/proof context preserves the `Body::size_hint`, `Request::builder`, Route `oneshot`, and `std::mem::replace` external frontier rows without local targets, exact RAG call context preserves the chrono `queue.is_empty` slice frontier without local targets, and exact RAG reach preserves fixture-backed `abs(value)` as an external targetless FFI frontier | Tool summaries count/display frontier rows; lookup/edges preserve the `Body::size_hint`, `Request::builder`, `std::mem::replace`, and Route `oneshot` external frontier call/proof rows; `request_code_context` already preserves the fixture-backed `abs(value)` targetless call/proof payload | axum, chrono plus local fixture fallback | Keep fail-closed; do not convert to traversal without external-summary semantics. |
| Unsupported receiver / trait-object shapes | Stronger blocker visibility plus bounded receiver positives | Targetless/unsupported rows remain for generic field/result/await/local receiver gaps; exact local `Router::clone` and axum-core `parts.extract_with_state(state)` receiver rows traverse; direct and single-reference external parameter receivers such as `call_external_param_vec_len(value: Vec<i32>) { value.len() }`, `call_external_borrowed_param_vec_len(value: &Vec<i32>) { value.len() }`, and axum `req: &mut Request<_>` / `mut req: Request<_>` `req.extensions_mut()` rows classify as external targetless frontiers; workspace re-exported external receiver aliases are now covered by a transform-level workspace proof where `provider::Request` aliases `http::Request` and `consumer` calls `req.extensions_mut()`; borrowed initialized local receivers such as `let value = LocalAssoc; (&value).instance_value()`, borrowed value-parameter receivers such as `call_borrowed_value_param_instance_method(value: LocalAssoc) { (&value).instance_value() }`, and borrowed value-parameter method-result chains such as `call_borrowed_value_param_method_result_instance_method(value: LocalAssoc) { (&value).clone_assoc().instance_value() }` now carry exact local receiver proof and resolve in fixture-backed DB/proof rows; same-target if-expression receivers such as `(if flag { LocalAssoc } else { LocalAssoc }).instance_value()` now carry branch-path proof and resolve in fixture-backed DB/proof rows; direct same-arity tuple-pattern local receivers such as `let (value, _) = (LocalAssoc, 0); value.instance_value()` now carry initializer proof and resolve in parser/DB/proof rows; explicitly typed tuple-pattern local receivers such as `let (value, _): (LocalAssoc, i32) = make_local_assoc_pair(); value.instance_value()` now carry per-element type proof and preserve the initializer helper call as a separate resolved path edge; self-field method-result chains such as `self.value.clone_assoc().instance_value()` reuse self-field proof for the inner call and return-type proof for the outer call in fixture-backed parser/DB/proof rows; tuple and named self-field receivers can classify external frontiers such as axum `Body(BoxBody)::size_hint`, Route `self.0.oneshot(req)`, and chrono `StrftimeItems::queue.is_empty()` when the source-visible field type proves the external/slice receiver shape; `Service`-bounded self-field `poll_ready` forwarding rows now classify as external targetless frontiers; initialized `Request::new` local receiver rows classify as external frontiers; qualified trait-object qself calls such as `<dyn std::any::Any>::downcast_mut::<T>(...)` project as external targetless path rows in fixture-backed tests and regenerated axum real-corpus rows; axum `HandleErrorFuture::poll` dyn Future dispatch is pinned as an unsupported, targetless method row; remaining generic `req.extensions_mut()` rows without exact workspace alias proof, turbofish method-chain receiver rows such as `request_parts.rs:164`, arbitrary untyped method-result tuple destructuring, and unknown receiver expressions persist as `Unresolved` or `Unsupported` receiver rows instead of being dropped | RAG preserves blocker/frontier rows, the exact positive subset, fixture-backed unsupported receiver rows, borrowed-initialized, borrowed value-parameter, borrowed value-parameter method-result, if-branch, tuple-pattern local binding, self-field method-result receiver payloads, qualified dyn Any external path rows, axum `Body::size_hint`, request-parts turbofish `generic_arg_count = 2`, dyn Future `poll`, Route `oneshot`, plus chrono `queue.is_empty` external frontier payloads | Tool tests surface unsupported rows/counts, `RouterClone` positives, focused `RequestPartsExt` local receiver checks, `&value = LocalAssoc` receiver formatting, if/match branch receiver formatting, tuple-pattern receiver request-code-context caller/proof expansion, self-field method-result call context/proof payloads, qualified dyn Any external proof rows, and axum `Body::size_hint`, request-parts turbofish unsupported receiver rows, dyn Future `poll` unsupported receiver rows, plus Route `oneshot` external frontier rows | axum, chrono plus local fixture fallback | Switch buckets; future implementation should add exact receiver proof by shape, not weaken unsupported rows. |
| Dynamic callable values | Stronger fixture-backed subset plus real-corpus targetless fallback; private single-caller parameter proof met for now | Fixture-backed direct, alias, same-target branch/match including guarded same-target match arms, single-expression block initialized callable values, exact `Box<dyn Fn()> = Box::new(local_target)` path/dynamic calls, named local closure binding `(closure)()` calls, local closure-binding `as fn` casts, exact local closure-binding deref calls such as `(*closure)()`, pathless non-async closure literal calls, immediately awaited async closure literal calls such as `(async || local_target())().await`, direct returned-path calls such as `make_fn()()`, direct returned closure-literal calls such as `make_closure()()`, returned local closure binding calls such as `make_bound_closure()()`, returned local aliases of closure bindings such as `make_alias_bound_closure()()`, private bare function-pointer single-caller parameter proof for `call_single_function_pointer_param(f: fn() -> i32) { f() }`, `call_single_parenthesized_function_pointer_param(f: fn() -> i32) { (f)() }`, `call_single_if_function_pointer_param_branch(flag, f) { (if flag { f } else { f })() }`, `call_single_match_function_pointer_param_arm(flag, f) { (match flag { true => f, false => f })() }`, and `call_single_function_pointer_param_cast(f: fn() -> i32) { (f as fn() -> i32)() }`, private generic callable-bound single-caller proof for `call_single_generic_fn_once_param<F>(generic_f: F) where F: FnOnce() -> i32 { generic_f() }`, and private constructed-holder single-caller field proof for `call_single_named_field_function_param(holder: CallbackHolder) { (holder.callback)() }`, `call_single_indexed_field_function_param(holder: CallbackArrayHolder) { holder.callbacks[0]() }`, and `call_single_indexed_tuple_field_function_param(holder: TupleCallbackArrayHolder) { holder.0[0]() }` when every local caller supplies `local_target` in the callable field slot; public/unproven function-pointer parameter calls such as `call_function_pointer_param(f: fn() -> i32) { f() }`, `call_parenthesized_function_pointer_param(f: fn() -> i32) { (f)() }`, `call_if_function_pointer_param_branch(flag, f)`, `call_match_function_pointer_param_arm(flag, f)`, and `call_function_pointer_param_cast(f: fn() -> i32) { (f as fn() -> i32)() }`, public holder-parameter calls such as `call_indexed_field_function_param(holder) { holder.callbacks[0]() }`, public/unproven generic callable-trait parameter calls, and real-corpus captured callback parameter calls such as axum `f(attr, input)` remain visible targetless path/dynamic rows with no fabricated edge; non-awaited async closure invocations remain targetless unsupported dynamic rows while their bodies are owned separately; mixed-target branches remain targetless; memchr function-pointer field rows remain unsupported and targetless by owner/source-line fanout; memchr callable trait-object field calls `fwd(...)` and `rev(...)` remain unsupported targetless path rows with dynamic-row absence pinned | RAG preserves resolved direct/branch/block/guarded-match initialized rows, exact boxed dyn Fn initializer rows, direct returned-path dynamic rows, direct returned-closure rows including local closure binding and local alias returns, named and literal dynamic-closure rows, dereferenced local closure-binding dynamic rows, blocker rows including function-pointer parameter path/dynamic blockers and axum closure-owned captured callback parameter blockers, non-awaited async-closure unsupported dynamic rows, the two memchr function-pointer field blockers with argument counts 4 and 2, the two memchr callable trait-object path blockers with argument count 2, the resolved private single-caller function-pointer parameter edges to `local_target` for `f()`, `(f)()`, `(if flag { f } else { f })()`, `(match flag { true => f, false => f })()`, and `(f as fn() -> i32)()`, the resolved private single-caller generic `FnOnce` `generic_f()` edge, and the resolved private holder-parameter edges for `(holder.callback)()`, `holder.callbacks[0]()` / `holder.0[0]()` plus their incoming wrapper helper rows | `code_item_lookup` covers resolved dynamic-function callable rows, including block-initialized and guarded same-target forms; DB/RAG owner/path queries now cover resolved `DynamicClosure` rows including closure literals, immediately awaited async closure literals, closure-binding `as fn` casts, exact local closure-binding derefs, direct returned closure literals, direct returned local closure bindings, returned local aliases of closure bindings, and real-corpus closure-owned captured callback parameter blockers; DB proof batches, RAG collection, and `code_item_lookup`/`code_item_edges` now cover the private named/indexed holder-parameter `DynamicFunction` edges; lookup and edges preserve fixture-backed function-pointer parameter `type_resolution_missing` blocker proof rows, targetless axum callable-field and memchr function-pointer field blockers, and memchr callable trait-object path blockers with `type_resolution_missing` proof rows; `request_code_context` preserves the resolved private single-caller function-pointer parameter edges to `local_target` for path, parenthesized dynamic, branch/match dynamic, cast dynamic, and bounded generic `FnOnce` call forms | local fixture; axum and memchr real-corpus fallback | Switch buckets; broader callable trait objects without exact initializer proof, arbitrary interprocedural callable-parameter value flow beyond complete private single-caller cases, returned closure values beyond direct closure literals, direct local closure bindings, or direct local aliases of closure bindings, and non-immediate poll/resume edges for async closure futures still need binding/body/effect work. |
| Proc-macro entrypoint body owners | Met for now | `CallBodyOwnerId::Macro` owners project through axum `expand_with` and `expand_attr_with` real-corpus helper calls; regenerated axum closure callback rows traverse to `debug_handler::expand`; regenerated axum IIFE rows resolve to closure-owner dynamic targets while callable-parameter invocations remain fail-closed | Exact impact summaries expose public proc-macro callers and direct helper callsites | `code_item_lookup` surfaces four public macro callers for `expand_with` | axum | Switch buckets; callback argument value-flow remains fail-closed until interprocedural callable proof exists. |
| Closures / executable-local body ownership | Named closure-binding, non-async closure-literal, immediately awaited async-closure literal, real-corpus closure body rows, fixture-backed async-closure body ownership, fixture-backed async-block traversal, function-local const/static/local-`fn`/local impl method ownership, local `fn` item call targets including call-before-declaration, and exact executable-owner tool lookup met for now | Ordinary, `move`, and async closure body calls plus async block body calls and function-local const/static initializer, local `fn`, and local impl method body calls are parser-owned by `CallBodyOwnerId::Executable`; `call_body_owner` projects closure, async-closure label, async block metadata, and local-item `local_const` / `local_static` / `local_fn:<name>` / `local_impl_method:<name>` metadata, validates them as call owners, keeps nested body calls off enclosing function owners, exposes `callers_for_target(...)` and one-hop paths from executable owners, resolves named closure binding calls, block-local `fn` item calls as `LocalFunction -> LocalItem` even when the call precedes the local item declaration, non-async closure literal dynamic calls, and immediate `.await` async-closure literal calls to closure owners, proves outer function -> local `fn` item -> `assoc_const_value` and outer function -> closure body -> `local_target` paths, preserves non-awaited async closure invocation as targetless until broader poll/resume modeling exists, resolves local impl where-bound associated path calls and async-block-owned `Self::...` paths through parent owner bounds, and snippet-materializes fixture-backed executable owners for RAG expansion | RAG collection, incoming expansion, and projected proof-context tests preserve fixture-backed closure, async-closure, async block, and local-item owners as callers for nested body rows, including function-local const/static initializer, local `fn`, local impl method `assoc_const_value()` rows, the outer `inner()` caller of `local_fn:inner`, the axum `local_impl_method:from_request_parts` resolved `Secret::from_ref` row, and the axum-core async-block-owned resolved `Self::from_request_parts` row; RAG also preserves real-corpus `Handler::call` async-block owner targetless `self()` / `into_response()` rows and outer closure binding/literal calls to closure owners | `request_code_context` tests materialize closure, async-closure, async block, and local-item owners with call context and projected proof rows, including move closure literal, local `fn` body traversal, the outer `inner()` `LocalFunction` edge to `local_fn:inner`, and local impl method body traversal; exact `code_item_lookup`/`code_item_edges` now accept `node_kind=local_item` for executable owners, prove the real-corpus axum `local_impl_method:deserialize` owner exposes external/unsupported call rows without local traversal paths, and parent-qualify the repeated axum `local_impl_method:from_request_parts` label with `parent_name=test_from_extractor` to expose the resolved one-hop `Secret::from_ref` associated-function edge; exact `code_item_lookup`/`code_item_edges` also address the real-corpus axum `Handler::call` nested `node_kind=async_block` owner with `parent_name=call` and preserve its `self()` plus awaited `into_response()` rows as unsupported, targetless blocker-proof rows; exact `code_item_call_path` accepts the same local-item endpoint and does not fabricate a zero-length self path; exact tests still assert enclosing outer functions do not flatten nested local-item rows unless the outer function is a proven caller of the local item; dedicated real-corpus lookup/edges tests include the closure-owned `expand_field` caller row | axum `HeaderValue::from_static` rows in route local const and JSON nested local `fn` bodies are projected as external targetless `LocalItem` owners; axum `Secret::from_ref` in a function-local impl method is projected as a resolved `LocalItem` owner through executable where-bound resolution and exact-addressable through `parent_name`; axum `local_impl_method:deserialize` in `axum/src/extract/path/mod.rs` is exact-addressable as a local-item tool target with external/unsupported rows; axum-core `Self::from_request_parts` in the ViaParts blanket async block is projected as a resolved `AsyncBlock` owner through parent impl bounds; async-block rows remain pinned targetless for selected `post`, `Handler::call` `self()` / `into_response()`, and awaited `unwrap` cases; fixture-backed ordinary closure, non-async closure literal, immediately awaited async-closure literal, async closure body, async block, local const/static initializer, local `fn`, local `fn` call target, and local impl method body cases are positive | Switch buckets unless the next planned slice explicitly targets broader async poll/resume effect edges, macro-expanded/generated source bodies, or broader local-item semantics beyond scoped local `fn` call targets. |
| Import / re-export / glob completeness | Partial, workspace dependency glob subset improved | Explicit and some imported path calls work; chrono alias constructor path calls resolve through typed alias evidence; fixture-backed `use super::*` inherited parent glob imports now resolve associated-function calls; regenerated axum backup data resolves 168 projected `TestClient::new` rows, including nested/direct re-export import rows, routing child-module inherited glob rows, and the axum-core `request_parts.rs:193` workspace dependency glob row; regenerated axum backup data also resolves twenty-three `Body::empty` caller edges through axum-core direct rows, direct parsed-workspace imports, local re-export imports, inherited glob imports, closure-owned rows, and local-item rows; axum `Request::new` and same-crate/imported/inherited-glob `Request::builder` rows through `Request = http::Request` are classified external and targetless; generated `IntoServiceFuture::new` remains a visible unresolved frontier until macro-expanded inherent items are modeled; broader import completeness remains partial | RAG exact context/proof preserves alias rows, the regenerated 168 `TestClient::new` resolved caller rows, the twenty-three `Body::empty` caller rows, and the targetless generated `IntoServiceFuture::new` frontier | Tool coverage includes chrono alias rows, dedicated high-fanout lookup/edges tests for `AxumRemainingTarget::TestClientNew`, exact lookup/edges tests for `Body::empty` incoming callers, exact lookup/edges targetless path rows for `Request::builder`, and exact lookup/edges targetless path rows for generated `IntoServiceFuture::new` via the trait-impl owner qualifier `owner_trait=Service<Request>` plus `owner_type=HandlerService` | axum, chrono plus local fixture fallback | Keep strict source oracles for the remaining source-text/projection gaps; broaden import resolution by proof shape, not by weakening targetless rows. |
| Dead-code / private zero-incoming | Met for now | `private_uncalled_nodes` lists axum `error_handling::traits`, excludes called `parse_attrs`, and exact impact reports no incoming source calls | `exact_private_uncalled_nodes` preserves the same DB dead-code list through RAG, and exact impact reports the same private target with empty caller/path/bucket sets | `code_private_uncalled` lists the same private zero-caller axum target directly; `code_item_lookup` reports zero incoming paths and zero impact caller counts | axum | Switch buckets; do not widen to generated/dynamic reachability here. |
| DB usage summaries | Strong current surface | `call_impact_for_target`, `call_reach_for_owner`, paths, owner-scoped module-boundary edges, owner-recursion cycle paths, source files/modules/crates/cfgs, buckets, boundary/frontier rows; real-corpus usage-question tests now prove impact, navigation, dead-code, public/test caller bucketing, architecture boundary edges, external/unsupported/unresolved frontier rows, proc-macro entrypoint impact, Body::empty component/source-module/source-crate impact, axum listener `#[cfg(unix)]` reach source-cfg preservation, axum `Json::from_bytes` `#[cfg(feature = "json")]` reach/callsite cfg preservation through parent module declarations, and `std::any::type_name::<K>()` API argument-shape preservation over the regenerated axum fixture; fixture-backed usage-question coverage now proves FFI `abs(value)` remains an external frontier in owner reach summaries; reach summaries split full nonresolved frontier rows into external, unsupported, unresolved, and ambiguous subsets without promoting them into traversal edges | N/A | N/A | axum plus local fixture fallback | Add fields only when they answer a matrix question, not opportunistically. |
| RAG usage summaries | Strong current surface | N/A | Exact call paths, owner-recursion cycle paths, impact, reach, source metadata; real-corpus reach summaries now preserve external, unsupported, unresolved, and ambiguous frontier subsets from DB, including the axum generated `IntoServiceFuture::new` unresolved frontier, the regenerated `Body::empty` component/source-module/source-crate impact, the axum listener `#[cfg(unix)]` source-cfg reach summary, the axum `Json::from_bytes` `feature = "json"` reach summary, the axum `type_name::<K>()` generic-argument call shape, the axum-core `request_parts.rs:164` unsupported turbofish method-receiver shape, and fixture-backed FFI `abs(value)` external-frontier reach | N/A | axum plus local fixture fallback | Add only when DB bucket already has proof. |
| TUI/tool usage summaries | Strong current surface | N/A | N/A | `code_item_lookup`, `code_item_edges`, and exact call-path tool coverage; `code_item_edges` now carries the same existing impact/reach summaries in `node_info` that lookup exposes, with real-corpus assertions for paths, owner-recursion cycle paths, boundary edges, direct callsites, callsite buckets, source files/modules/crates/cfgs, frontier status counts, and UI counts; lookup/edges `Body::empty` tool tests assert the regenerated component-impact callsite buckets, path-shape counts, source file/module/crate carriers, and test/non-test impact partition; lookup/edges generated-constructor tests assert unresolved frontier payload/count propagation; lookup/edges `Json::from_bytes` tests assert the inherited `feature = "json"` reach source-cfg payload/count; lookup coverage asserts the axum `type_name::<K>()` generic-argument call shape; lookup/edges targetless matrix tests assert the axum-core `request_parts.rs:164` unsupported turbofish receiver row; lookup/edges executable-owner tests assert the real-corpus axum `Handler::call` async-block owner targetless rows and blocker proof payloads | axum plus local fixture fallback | Keep tool changes thin; do not invent semantics outside RAG/DB. |

## Parking Lot

- Add binding tracking plan that ties syntax body ownership, local bindings, and type graph edges before attempting receiver/dynamic traversal.
- Add a workspace-level dependency-root proof carrier before resolving selected
  workspace member imports such as axum-core test code importing
  `axum::{test_helpers::*, Router}`. The current parser resolver works inside a
  single crate `ModuleTree`; DB projection must not turn those dependency-root
  imports into local edges without typed workspace evidence.
- Split future resolver work by capability: local binding, field receiver, closure body owner, import/re-export/glob, trait dispatch.
- Consider adding a small status table to `README.md` only after this matrix has stabilized.

## Workflow Checks

Before implementation:

- Pick one matrix row as the bucket.
- State its exit criteria.
- Confirm whether DB-only proof is enough or whether RAG/TUI exposure is part of the row.

During implementation:

- Batch related assertions in table-driven tests where practical.
- Avoid adding parser breadth without a matching DB/RAG/tool proof or an explicit unsupported-row assertion.
- Commit at logical boundaries.

After implementation:

- Mark the bucket status in this document or in the coverage inventory.
- Move to the next uncovered bucket unless the current bucket still fails its stated exit criteria.
