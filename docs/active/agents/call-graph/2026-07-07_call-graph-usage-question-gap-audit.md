# 2026-07-07 Call Graph Usage Question Gap Audit

Short description: maps the usage-question document to the current DB, RAG, and
TUI call-graph surfaces so the next implementation bucket is selected from a
real gap rather than recent local attention.

Related planning files:
- [`../2026-06-30_call-graph-usage-questions.md`](../2026-06-30_call-graph-usage-questions.md)
- [`2026-07-01_call-graph-goal-coverage-matrix.md`](2026-07-01_call-graph-goal-coverage-matrix.md)
- [`2026-07-01_call-graph-larger-plan-map.md`](2026-07-01_call-graph-larger-plan-map.md)

## Summary

The core query surface is now substantially implemented. DB tests cover real
axum usage questions for paths, impact, reach, module-boundary edges,
frontiers, source files/modules/crates/cfgs, argument shape, test/non-test
caller buckets, public caller buckets, private zero-incoming nodes, generated
entrypoint-summary proof context, proc-macro entrypoint impact, and callable
item metadata such as unsafe and async function qualifiers. RAG and tool tests
preserve the same major surfaces through exact APIs and tool payloads.

The remaining gaps are mostly not missing query helpers. They are missing proof
inputs:

- value-flow and binding proof for unsupported receiver/dynamic callsites;
- external dependency summaries for frontier rows that should become trusted
  external effects;
- broader source/sink, cost, and policy annotations for
  security/performance/refactoring questions that need domain semantics beyond
  current `effect_seed`, admitted owner `effect_policy` allowlists, and
  caller-supplied effect guard or module-boundary policy reports;
- generated harness/build-entrypoint execution policy for full binary/test/CI
  reachability beyond admitted proof-only entrypoint summaries.

## Usage Question Coverage

| Section | Current evidence | Status |
| --- | --- | --- |
| Impact analysis | `call_impact_for_target`, RAG `exact_call_impact_for_target`, `code_item_lookup`, and `code_item_edges` cover eventual callers, public callers, test/non-test buckets, direct callsites, source files/modules/crates, and callsite buckets. | Strong current surface. |
| Dead code detection | `private_uncalled_nodes`, RAG `exact_private_uncalled_nodes`, and `code_private_uncalled` cover private zero-incoming nodes and cross-check empty impact reports. `code_private_uncalled` now also exposes admitted generated-entrypoint summaries, typed `call_test_entrypoints`, and linked build-domain rows for returned nodes. | Strong for stored source call graph; generated harness reach remains proof-only and does not become a local call edge. |
| Navigation | `callers_for_target`, `call_sites_for_target`, `call_context_for_owner`, `call_paths_*`, RAG exact paths, and `code_item_call_path` cover direct and multi-hop traversal. | Strong current surface. |
| Security analysis | Reach/path/frontier queries can answer "can A reach B?" and expose unsafe/FFI/external frontier rows, including `abs(value)` and unsafe-block metadata. Async callable item metadata is also exposed on function/method nodes so target-centered summaries can distinguish async and sync callable definitions without inferring poll/resume behavior. Reachable `effect_seed` facts plus caller-supplied or admitted-owner `effect_policy` allowlists can report policy violations such as the axum `tokio::spawn` `async_task_spawn` sink without fabricating local edges. DB `call_effect_guard_report_for_owner`, RAG `exact_call_effect_guard_report_for_owner`, and exact TUI `code_item_effect_guard` now classify whether resolved paths to a reachable effect owner pass through a caller-supplied guard, proving the axum `deserialize_error_status_codes -> TestClient::new -> spawn_service -> tokio::spawn` effect is guarded by `TestClient::new` and violated by an unrelated guard. Owner-scoped `call_proof_invariant_findings` exposes blocked detached-process proof obligations for reachable process-effect callsites through DB, RAG, and exact tool payloads. | Partial: broader source/sink classification and domain-specific security semantics are not modeled; async item metadata is a qualifier only and does not model poll/resume or future value-flow; effect guard reports currently cover explicit guard nodes over reachable `effect_seed` rows through DB/RAG/TUI exact APIs; the process-invariant downstream proof is fixture-backed because active real-corpus backups do not include process bench/build targets. |
| Performance work | Reach/path/frontier queries expose known helpers, external calls, cfgs, and argument shape. | Partial: hot-path/cost/blocking annotations are not modeled. |
| Refactoring support | Impact, direct callsites, source files/modules/crates, boundary edges, and callsite buckets support migration planning. | Strong for caller inventory; move-safety/cycle prediction needs dependency-policy rules. |
| Test planning | Impact test/non-test buckets, source metadata, admitted generated test-harness entrypoint summaries, and typed `call_test_entrypoints` identify stored test callers and proof-only generated entrypoints. | Partial: generated harness summaries remain proof facts, and CI test selection is not modeled. |
| Architecture review | `module_boundary_edges_from_owner`, RAG `exact_module_boundary_edges_from_owner`, reach `boundary_edges`, lookup/edges `module_boundary_edges` payloads, and source modules expose cross-module call edges with caller/callee/site metadata. DB `module_boundary_policy_violations_from_owner`, RAG `exact_module_boundary_policy_violations_from_owner`, and exact TUI `code_item_boundary_policy` now evaluate caller-supplied forbidden module-prefix rules over resolved-only boundary edges. | Strong for module-boundary inventory and exact module-prefix policy checks; broader dependency-policy semantics are not modeled. |
| Debugging | Owner/target context, exact paths, frontier status buckets, proof context, and source spans map persisted edges/blockers back to source callsites. | Strong current surface. |
| API understanding | Impact buckets, argument/generic argument counts, constructor relation kinds, path-shape counts, aliases/re-exports, and source crates show real target usage. | Strong current surface. |
| Documentation and RAG | RAG exact call context, exact paths, impact/reach summaries, proof context, and tool payloads expose caller/callee context and fail-closed blockers. | Strong current surface. |
| Build or deployment optimization | Source crates/modules/cfgs and component impact reports answer affected components and feature/platform-gated paths. Exact DB/RAG/tool build-domain summaries and typed test-entrypoint summaries now expose linked admitted build/test proof metadata for generated test-entrypoint summaries without adding source call edges. | Partial: actual CI test selection policy is not modeled. |

## Concrete Existing Proof Points

- DB: `axum_usage_questions_*` in
  `crates/ploke-db/tests/unit/call_graph_fixture_queries/real_target_matrix/usage_questions.rs`.
- DB/RAG/TUI metadata propagation: fixture-backed async item metadata tests for
  `make_ready_local_assoc` and `call_await_result_instance_method`, plus
  unsafe item metadata tests for `unsafe_target`.
- RAG: real-corpus exact path, impact, reach, module-boundary, module-boundary
  policy, effect-guard, frontier, and
  private-uncalled tests in
  `crates/ploke-rag/src/core/unit_tests/tests/call_context/collection/cases/fixtures/real_corpus.rs`.
- Proof graph: strict `effect_policy` storage/projection tests in
  `crates/ploke-db/tests/proof_graph_store/effect_policy.rs`.
- TUI/tools: `code_item_call_path`, `code_item_effect_guard`,
  `code_item_boundary_policy`, `code_item_lookup`, `code_item_edges`,
  `code_private_uncalled`, and
  targetless matrix integration tests under
  `crates/ploke-tui/tests/integration/`. The exact effect-guard tool proves the
  axum `tokio::spawn` effect guard/violation contract without fabricating a
  local edge to the external targetless callsite. The exact lookup/edges and
  private-uncalled tools now expose admitted generated-entrypoint summaries
  through typed payloads without fabricating source call edges.

## Selected Next Bucket

Current query helpers are sufficient for the next implementation step. The next
bucket should return to semantic expansion:

```text
binding/type-aware proof for unsupported receiver and dynamic callable rows
```

Exit criteria for the next bucket:

- Pick one existing targetless real-corpus or fixture-backed row from the
  coverage matrix.
- Identify the exact missing proof input: local binding, receiver type,
  callable value flow, external summary, or generated item.
- Add the smallest typed parser/resolver/transform fact needed to prove that
  row, or keep it fail-closed with a stronger blocker if proof is still
  unavailable.
- Verify DB first, then propagate to RAG and TUI only if the row is exposed
  there.

Suggested first candidate: local binding/type proof for a still-unsupported
receiver or callable value shape, because it matches the larger plan's next
phase and directly improves several partially covered usage-question sections
without weakening frontier semantics.

### 2026-07-10 inventory update

Active fixtures were regenerated and backup DB verification passed for the
current call-graph schema. The historical `call_graph` feature gate is closed:
workspace crates keep the feature only as a compatibility alias, and active
fixtures include baseline call-graph relations.

The post-regeneration inventory found that the obvious nearby candidates are
already closed or intentionally fail-closed with proof rows:

- fixture test-body macro expansion blockers
- public callable parameter and holder blockers
- missing trait-visibility blockers
- axum-core `request_parts.rs:164` tuple-return receiver resolution
- routing helper rows hidden behind macro/generated source boundaries
- generated `IntoServiceFuture::new` rows
- current effect and external-summary reach surfaces

The next implementation bucket should therefore start with a genuinely new
proof input instead of more breadth over those closed rows. Viable carriers are
macro-expanded/generated source bodies, async poll/resume execution proof,
interprocedural callable argument/value-flow, broader callable trait-object
dispatch, source/sink policy annotations, or build/test entrypoint
execution-policy summaries.

### 2026-07-12 generated-item inventory update

After fixture regeneration, the real-corpus `IntoServiceFuture::new` row was
rechecked as the most obvious generated-source candidate:

- Source oracle:
  `axum/src/handler/future.rs:11-18` invokes `opaque_future!`,
  `axum/src/macros.rs:19-20` templates the generated inherent `new`, and
  `axum/src/handler/service.rs:174` calls
  `super::future::IntoServiceFuture::new(future)`.
- Existing parser macro support is intentionally much narrower: no-arg
  `macro_rules!` expansions inside function bodies that produce exactly one
  local `fn`, local `const`, local `static`, or one statement-position path
  call.
- `opaque_future!` is a parameterized item macro that generates top-level
  struct and impl items, so making this row traversable requires a generated
  item modeling slice, not a small path resolver change.
- The current DB/RAG/TUI contract is therefore still correct for this row:
  preserve the unresolved targetless callsite, retain the admitted
  `expansion_boundary` / `expanded_item` proof metadata, and do not fabricate a
  local call edge until a generated method node exists.

The next semantic implementation slice should avoid more targetless proof
breadth and should choose either:

- an explicitly scoped generated-item model for parameterized item macros, with
  parser-generated struct/impl/function nodes and DB-first traversal tests; or
- a smaller non-generated proof carrier with concrete new syntax evidence that
  is not already represented by the current fixture-backed private callable
  forwarding, receiver, or async future-flow rows.

### 2026-07-12 post-regeneration candidate boundary

After regenerating fixtures and rechecking the nearby fixture/real-corpus rows,
the smallest apparent non-generated candidates are already covered:

- returned function-pointer parameter forwarding through
  `return_forwarded_function_pointer(local_target)()`;
- returned closure literals, returned local closure bindings, and returned
  closure aliases;
- tuple-field, named-field, alias, and alias-chain awaited async-closure future
  bindings;
- private one-hop and two-hop forwarding for `fn()`, `&dyn Fn`, `Box<dyn Fn>`,
  and constructed holder-field callable parameters;
- mutable referenced `dyn FnMut` local binding proof.

Remaining visible targetless rows are not safe to promote with the current proof
carriers. They need one of the larger models listed above:

- object/field value-flow for real-corpus dynamic callable fields such as axum
  `self.into_route`, `self.layer`, and `self.tap_fn`;
- generated top-level item modeling for parameterized item macros such as axum
  `opaque_future!`;
- macro-expanded arbitrary-expression bodies for memchr's generated
  `transmute::<Fn, RealFn>(fun)(...)` callsites;
- broader runtime trait-object dispatch summaries for real-corpus boxed
  `dyn FnMut` fields;
- external-return or source/sink policy summaries when the target is outside
  local source ownership.

Do not spend another slice adding fixture breadth around the already-covered
private callable or awaited-future cases. The next implementation should pick
one larger proof model explicitly and keep the first test DB-first.
