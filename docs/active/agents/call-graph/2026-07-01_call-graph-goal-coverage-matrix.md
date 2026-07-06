# 2026-07-01 Call Graph Goal Coverage Matrix

Short description: workflow guardrail and status matrix for implementing the call-graph usage-question goal without over-focusing on the latest local concern.

Related planning files:
- [`2026-07-01_call-graph-larger-plan-map.md`](2026-07-01_call-graph-larger-plan-map.md)
- [`../2026-06-30_call-graph-usage-questions.md`](../2026-06-30_call-graph-usage-questions.md)
- [`2026-06-25_call-graph-coverage-inventory.md`](2026-06-25_call-graph-coverage-inventory.md)
- [`2026-06-28_real-corpus-call-site-case-matrix.md`](2026-06-28_real-corpus-call-site-case-matrix.md)
- [`2026-06-28_real-corpus-call-site-oracle-matrices.md`](2026-06-28_real-corpus-call-site-oracle-matrices.md)

Status date: 2026-07-06
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

## Current Bucket

Current bucket: local callable parameter proof.

Exit criteria:

- Reuse the existing `PathCallCallee::ValueBinding` and local path resolver
  proof patterns rather than adding a parallel binding subsystem.
- Resolve only private function-pointer parameter calls whose local caller set
  is complete and supplies exactly one proven callable target for the parameter.
- Keep public, uncalled, opaque-argument, and multi-target function-pointer
  parameter calls targetless/unsupported.
- Regenerate registered backup fixtures only when the schema/projection contract
  has intentionally changed.

Completed evidence:

- Parser path-call payloads now record conservative argument summaries for
  exact path arguments and non-async closure literal arguments.
- The resolver can prove `call_single_function_pointer_param(f: fn() -> i32)`
  resolves `f()` to `local_target` only because the helper is private and its
  complete local caller set supplies the same exact function item argument.
- Public, opaque, missing-argument, and multi-target parameter-call shapes still
  fail closed without a fabricated edge.
- DB fixture tests prove the persisted call graph can traverse the resolved
  parameter call from `call_single_function_pointer_param` to `local_target`.
- RAG collection and `request_code_context` tests preserve the same resolved
  `f()` to `local_target` edge for the private single-caller parameter helper.
- `cargo run -p xtask --features call_graph -- fixtures regenerate --active`
  passed on 2026-07-06; it refreshed ignored checkout-local fixtures and
  shared call-graph snapshots without producing tracked backup-fixture changes.

Reason to stay in this bucket: none by default. Switch buckets unless the
matrix identifies one adjacent callable-parameter source oracle with a distinct
proof carrier.

## Coverage Matrix

| Bucket | Current status | DB proof | RAG proof | TUI/tool proof | Real-corpus target | Next action |
| --- | --- | --- | --- | --- | --- | --- |
| Method / trait-method multi-hop | Met for now | `RequestExt::extract -> extract_with_state -> FromRequest::from_request` two-hop paths and summaries | Exact call paths, expansion, impact, reach | `code_item_lookup` two-hop payload and UI counts | axum | Do not deepen by default; switch buckets unless a regression appears. |
| Regular free-function one-hop | Covered | `parse_attrs`, `run_ui_tests`, and related target fanout assertions | Exact call-context tests for current real-corpus function callers | `code_item_lookup` function caller regressions | axum | Use as source pool for finding a free-function multi-hop chain. |
| Regular free-function multi-hop | Met for now | `from_request::expand -> impl_struct_by_extracting_each_field -> extract_fields` ordered two-hop traversal through `call_paths_between`, owner, and target path APIs | Exact call paths expose the same ordered function chain and source-node metadata | `code_item_call_path` returns the same real-corpus function reachability path | axum | Switch buckets; do not add more free-function breadth by default. |
| Inherent method one-hop | Covered/partial | Examples include `Json::from_bytes` and related method/associated-function rows | Exact call-context propagation exists | Exact lookup regressions exist | axum | Revisit only after broader buckets have at least one proof. |
| Exact local external-trait impl receiver methods | Met for now | 13 `Router::clone` typed-local/self-field receiver rows resolve to `impl<S> Clone for Router<S>::clone` | Exact call context preserves the same caller-site identities and receiver buckets | Remaining real-corpus TUI matrix includes `RouterClone` | axum | Switch buckets; do not broaden to arbitrary external trait dispatch by default. |
| Associated-function path calls | Met for now | `Self::from_bytes`, `E::from_request`, `MethodRouter::new` style rows; `RequestExt::extract -> extract_with_state -> FromRequest::from_request` proves a two-hop path whose terminal edge is `AssociatedFunction` | Exact paths, expansion, impact, and reach preserve associated-function relation/kind | `request_code_context`, `code_item_call_path`, and lookup/edges regressions carry associated-function path rows and callsite buckets | axum | Switch buckets; do not add a separate associated-function multi-hop row unless this exact contract regresses. |
| Constructors | Stronger one-hop | Tuple struct / enum variant constructor rows are asserted in real-corpus matrix, including chrono alias constructor rows; real-corpus `BoxedIntoRoute` `Self(...)` tuple constructors and fixture-backed method-owned `Self(...)` tuple constructors now resolve through the enclosing impl self type | Exact context propagation exists for direct, alias, and `Self(...)` constructor rows | Tool regressions include direct, alias, variant, and `Self(...)` constructor rows | axum, chrono plus local fixture fallback | Switch buckets; add constructor breadth only when a new source shape is represented in the matrix. |
| External dependency frontier | Covered as frontier | External rows are exposed but not traversed; regenerated axum DB fixture classifies `Body::size_hint` `self.0.size_hint()` as an external targetless frontier through tuple-field receiver and local type-alias evidence for `Body(BoxBody)`, and classifies fourteen `Request::builder` rows as external frontiers when `Request` resolves through the axum-core `Request = http::Request` alias | Reach summaries expose external frontier calls; RAG call/proof context preserves the `Body::size_hint` and `Request::builder` external frontier rows without local targets | Tool summaries count/display frontier rows; lookup/edges preserve the `Body::size_hint` and `Request::builder` external frontier call/proof rows | axum | Keep fail-closed; do not convert to traversal without external-summary semantics. |
| Unsupported receiver / trait-object shapes | Stronger blocker visibility plus bounded receiver positives | Targetless/unsupported rows remain for generic field/result/await/local receiver gaps; exact local `Router::clone` receiver rows traverse; direct and single-reference external parameter receivers such as `call_external_param_vec_len(value: Vec<i32>) { value.len() }`, `call_external_borrowed_param_vec_len(value: &Vec<i32>) { value.len() }`, and axum `req: &mut Request<_>` / `mut req: Request<_>` `req.extensions_mut()` rows classify as external targetless frontiers; borrowed initialized local receivers such as `let value = LocalAssoc; (&value).instance_value()` now carry initializer proof and resolve in fixture-backed DB/proof rows; same-target if-expression receivers such as `(if flag { LocalAssoc } else { LocalAssoc }).instance_value()` now carry branch-path proof and resolve in fixture-backed DB/proof rows; self-field method-result chains such as `self.value.clone_assoc().instance_value()` reuse self-field proof for the inner call and return-type proof for the outer call in fixture-backed parser/DB/proof rows; tuple self-field receivers can classify external alias-backed frontiers such as axum `Body(BoxBody)::size_hint`; initialized `Request::new` local receiver rows classify as external frontiers; qualified trait-object qself calls such as `<dyn std::any::Any>::downcast_mut::<T>(...)` project as external targetless path rows in fixture-backed tests and regenerated axum real-corpus rows; remaining generic `req.extensions_mut()` rows and unknown receiver expressions persist as `Unresolved` or `Unsupported` receiver rows instead of being dropped | RAG preserves blocker/frontier rows, the exact positive subset, fixture-backed unsupported receiver rows, borrowed-initialized, if-branch, self-field method-result receiver payloads, qualified dyn Any external path rows, and axum `Body::size_hint` tuple self-field external frontier payloads | Tool tests surface unsupported rows/counts, `RouterClone` positives, `&value = LocalAssoc` receiver formatting, if-branch receiver formatting, self-field method-result call context/proof payloads, qualified dyn Any external proof rows, and axum `Body::size_hint` tuple self-field external frontier rows | axum plus local fixture fallback | Switch buckets; future implementation should add exact receiver proof by shape, not weaken unsupported rows. |
| Dynamic callable values | Stronger fixture-backed subset plus real-corpus targetless fallback; private single-caller parameter proof met for now | Fixture-backed direct, alias, same-target branch/match including guarded same-target match arms, single-expression block initialized callable values, exact `Box<dyn Fn()> = Box::new(local_target)` path/dynamic calls, named local closure binding `(closure)()` calls, local closure-binding `as fn` casts, pathless non-async closure literal calls, direct returned-path calls such as `make_fn()()`, direct returned closure-literal calls such as `make_closure()()`, returned local closure binding calls such as `make_bound_closure()()`, returned local aliases of closure bindings such as `make_alias_bound_closure()()`, and the private single-caller parameter proof `call_single_function_pointer_param(f: fn() -> i32) { f() }` resolve when every local caller supplies the same exact callable target; public/unproven function-pointer parameter calls such as `call_function_pointer_param(f: fn() -> i32) { f() }` and real-corpus captured callback parameter calls such as axum `f(attr, input)` remain visible targetless path rows with no fabricated edge; async closure invocations remain targetless unsupported dynamic rows while their bodies are owned separately; mixed-target branches remain targetless; memchr function-pointer field rows remain unsupported and targetless by owner/source-line fanout | RAG preserves resolved direct/branch/block/guarded-match initialized rows, exact boxed dyn Fn initializer rows, direct returned-path dynamic rows, direct returned-closure rows including local closure binding and local alias returns, named and literal dynamic-closure rows, blocker rows including function-pointer parameter path blockers and axum closure-owned captured callback parameter blockers, async-closure unsupported dynamic rows, the two memchr function-pointer field blockers with argument counts 4 and 2, and the resolved private single-caller function-pointer parameter edge to `local_target` | `code_item_lookup` covers resolved dynamic-function callable rows, including block-initialized and guarded same-target forms; DB/RAG owner/path queries now cover resolved `DynamicClosure` rows including closure literals, closure-binding `as fn` casts, direct returned closure literals, direct returned local closure bindings, returned local aliases of closure bindings, and real-corpus closure-owned captured callback parameter blockers; lookup and edges preserve fixture-backed function-pointer parameter `type_resolution_missing` blocker proof rows and targetless axum callable-field plus memchr function-pointer field blockers; `request_code_context` preserves the resolved private single-caller function-pointer parameter edge to `local_target` | local fixture; axum and memchr real-corpus fallback | Switch buckets; broader callable trait objects without exact initializer proof, arbitrary interprocedural callable-parameter value flow, returned closure values beyond direct closure literals, direct local closure bindings, or direct local aliases of closure bindings, and poll/resume edges for async closure futures still need binding/body/effect work. |
| Proc-macro entrypoint body owners | Met for now | `CallBodyOwnerId::Macro` owners project through axum `expand_with` and `expand_attr_with` real-corpus helper calls; regenerated axum closure callback rows traverse to `debug_handler::expand`; regenerated axum IIFE rows resolve to closure-owner dynamic targets while callable-parameter invocations remain fail-closed | Exact impact summaries expose public proc-macro callers and direct helper callsites | `code_item_lookup` surfaces four public macro callers for `expand_with` | axum | Switch buckets; callback argument value-flow remains fail-closed until interprocedural callable proof exists. |
| Closures / executable-local body ownership | Named closure-binding, non-async closure-literal, real-corpus closure body rows, fixture-backed async-closure body ownership, fixture-backed async-block traversal, and function-local const/static/local-`fn`/local impl method ownership met for now | Ordinary, `move`, and async closure body calls plus async block body calls and function-local const/static initializer, local `fn`, and local impl method body calls are parser-owned by `CallBodyOwnerId::Executable`; `call_body_owner` projects closure, async-closure label, async block metadata, and local-item `local_const` / `local_static` / `local_fn` / `local_impl_method:<name>` metadata, validates them as call owners, keeps nested body calls off enclosing function owners, exposes `callers_for_target(...)` and one-hop paths from executable owners, resolves named closure binding calls and non-async closure literal dynamic calls to closure owners, proves outer function -> closure body -> `local_target` two-hop paths for non-async closure invocation, preserves async closure invocation as targetless until future poll/resume modeling, resolves local impl where-bound associated path calls and async-block-owned `Self::...` paths through parent owner bounds, and snippet-materializes fixture-backed executable owners for RAG expansion | RAG collection, incoming expansion, and projected proof-context tests preserve fixture-backed closure, async-closure, async block, and local-item owners as callers for nested body rows, including function-local const/static initializer, local `fn`, local impl method `assoc_const_value()` rows, the axum `local_impl_method:from_request_parts` resolved `Secret::from_ref` row, and the axum-core async-block-owned resolved `Self::from_request_parts` row; RAG also preserves real-corpus `Handler::call` async-block owner targetless `self()` / `into_response()` rows and outer closure binding/literal calls to closure owners | `request_code_context` tests materialize closure, async-closure, async block, and local-item owners with call context and projected proof rows, including move closure literal, local `fn` body traversal, and local impl method body traversal; exact TUI lookup/edges tests assert the enclosing axum `test_from_extractor` function does not flatten the nested `Secret::from_ref` local-item row because those exact item tools do not accept executable body owners as `node_kind` values; dedicated real-corpus lookup/edges tests include the closure-owned `expand_field` caller row; outer functions are not fabricated as direct nested-body callers | axum `HeaderValue::from_static` rows in route local const and JSON nested local `fn` bodies are projected as external targetless `LocalItem` owners; axum `Secret::from_ref` in a function-local impl method is projected as a resolved `LocalItem` owner through executable where-bound resolution; axum-core `Self::from_request_parts` in the ViaParts blanket async block is projected as a resolved `AsyncBlock` owner through parent impl bounds; async-block rows remain pinned targetless for selected `post`, `Handler::call` `self()` / `into_response()`, and awaited `unwrap` cases; fixture-backed ordinary closure, non-async closure literal, async closure body, async block, local const/static initializer, local `fn`, and local impl method body cases are positive | Switch buckets unless the next planned slice explicitly targets async poll/resume effect edges or broader local-item semantics beyond body ownership. |
| Import / re-export / glob completeness | Partial, workspace dependency glob subset improved | Explicit and some imported path calls work; chrono alias constructor path calls resolve through typed alias evidence; fixture-backed `use super::*` inherited parent glob imports now resolve associated-function calls; regenerated axum backup data resolves 168 projected `TestClient::new` rows, including nested/direct re-export import rows, routing child-module inherited glob rows, and the axum-core `request_parts.rs:193` workspace dependency glob row; regenerated axum backup data also resolves twenty-three `Body::empty` caller edges through axum-core direct rows, direct parsed-workspace imports, local re-export imports, inherited glob imports, closure-owned rows, and local-item rows; axum `Request::new` and same-crate/imported/inherited-glob `Request::builder` rows through `Request = http::Request` are classified external and targetless; generated `IntoServiceFuture::new` remains a visible unresolved frontier until macro-expanded inherent items are modeled; broader import completeness remains partial | RAG exact context/proof preserves alias rows, the regenerated 168 `TestClient::new` resolved caller rows, the twenty-three `Body::empty` caller rows, and the targetless generated `IntoServiceFuture::new` frontier | Tool coverage includes chrono alias rows, dedicated high-fanout lookup/edges tests for `AxumRemainingTarget::TestClientNew`, exact lookup/edges tests for `Body::empty` incoming callers, exact lookup/edges targetless path rows for `Request::builder`, and exact lookup/edges targetless path rows for generated `IntoServiceFuture::new` via the trait-impl owner qualifier `owner_trait=Service<Request>` plus `owner_type=HandlerService` | axum, chrono plus local fixture fallback | Keep strict source oracles for the remaining source-text/projection gaps; broaden import resolution by proof shape, not by weakening targetless rows. |
| Dead-code / private zero-incoming | Met for now | `private_uncalled_nodes` lists axum `error_handling::traits`, excludes called `parse_attrs`, and exact impact reports no incoming source calls | `exact_private_uncalled_nodes` preserves the same DB dead-code list through RAG, and exact impact reports the same private target with empty caller/path/bucket sets | `code_private_uncalled` lists the same private zero-caller axum target directly; `code_item_lookup` reports zero incoming paths and zero impact caller counts | axum | Switch buckets; do not widen to generated/dynamic reachability here. |
| DB usage summaries | Strong current surface | `call_impact_for_target`, `call_reach_for_owner`, paths, source files/modules, buckets, boundary/frontier rows; real-corpus usage-question tests now prove impact, navigation, dead-code, public/test caller bucketing, architecture boundary edges, external/unsupported frontier rows, proc-macro entrypoint impact, and Body::empty component/source-module impact over the regenerated axum fixture | N/A | N/A | axum | Add fields only when they answer a matrix question, not opportunistically. |
| RAG usage summaries | Strong current surface | N/A | Exact call paths, impact, reach, source metadata; real-corpus impact summary now preserves the regenerated `Body::empty` component/source-module impact, callsite buckets, path-shape counts, and test/non-test caller partition from the DB usage-query surface | N/A | axum | Add only when DB bucket already has proof. |
| TUI/tool usage summaries | Strong current surface | N/A | N/A | `code_item_lookup`, `code_item_edges`, and exact call-path tool coverage; `code_item_edges` now carries the same existing impact/reach summaries in `node_info` that lookup exposes, with real-corpus assertions for paths, boundary edges, direct callsites, callsite buckets, source files/modules, and UI counts; lookup/edges `Body::empty` tool tests now assert the regenerated component-impact callsite buckets, path-shape counts, source file/module carriers, and test/non-test impact partition | axum | Keep tool changes thin; do not invent semantics outside RAG/DB. |

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
