# 2026-07-01 Call Graph Goal Coverage Matrix

Short description: workflow guardrail and status matrix for implementing the call-graph usage-question goal without over-focusing on the latest local concern.

Related planning files:
- [`2026-07-01_call-graph-larger-plan-map.md`](2026-07-01_call-graph-larger-plan-map.md)
- [`../2026-06-30_call-graph-usage-questions.md`](../2026-06-30_call-graph-usage-questions.md)
- [`2026-06-25_call-graph-coverage-inventory.md`](2026-06-25_call-graph-coverage-inventory.md)
- [`2026-06-28_real-corpus-call-site-case-matrix.md`](2026-06-28_real-corpus-call-site-case-matrix.md)
- [`2026-06-28_real-corpus-call-site-oracle-matrices.md`](2026-06-28_real-corpus-call-site-oracle-matrices.md)

Status date: 2026-07-01
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

Current bucket: local callable bindings initialized by single-expression blocks.

Exit criteria:

- Reuse the existing local-binding proof path (`inferred_init_path`,
  `block_path_expr`, initialized value binding call-site expectations) instead
  of introducing a new resolver model.
- Prove parser extraction/resolution for `let f = { local_target }; f()` as a
  path call and `let f = { local_target }; (f)()` as a dynamic call.
- Prove DB context and proof-fact projection for the block-initialized path and
  dynamic call rows.
- Prove RAG exact call-context propagation for both rows.
- Prove `code_item_lookup` exposes the resolved dynamic-function row and proof
  rows for the parenthesized block-initialized binding.

Completed evidence:

- Parser:
  `fixture_call_graph_call_block_initialized_function_item_binding_resolves_initialized_value_binding_path_call_site`
  and
  `fixture_call_graph_call_parenthesized_block_initialized_function_item_binding_resolves_dynamic_function_call_site`.
- DB:
  `fixture_context_reads_projected_function_item_binding_calls`,
  `fixture_context_reads_projected_parenthesized_binding_dynamic_calls`, and
  `fixture_projection_stores_real_callable_expression_dynamic_call_proof_facts`.
- RAG:
  `call_context_collection_reads_real_fixture_callable_path_rows` and
  `call_context_collection_reads_real_fixture_dynamic_rows`.
- TUI/tool:
  `code_item_lookup_returns_resolved_dynamic_callable_context`.

Reason to switch after this bucket: this closes the single-expression block
initializer form for the existing exact local callable-binding model. Broader
callable trait objects, closure-return values, typed function-pointer inference,
and closure/async body ownership still require the larger binding/body-owner
plan before their fail-closed contracts should change.

## Coverage Matrix

| Bucket | Current status | DB proof | RAG proof | TUI/tool proof | Real-corpus target | Next action |
| --- | --- | --- | --- | --- | --- | --- |
| Method / trait-method multi-hop | Met for now | `RequestExt::extract -> extract_with_state -> FromRequest::from_request` two-hop paths and summaries | Exact call paths, expansion, impact, reach | `code_item_lookup` two-hop payload and UI counts | axum | Do not deepen by default; switch buckets unless a regression appears. |
| Regular free-function one-hop | Covered | `parse_attrs`, `run_ui_tests`, and related target fanout assertions | Exact call-context tests for current real-corpus function callers | `code_item_lookup` function caller regressions | axum | Use as source pool for finding a free-function multi-hop chain. |
| Regular free-function multi-hop | Met for now | `from_request::expand -> impl_struct_by_extracting_each_field -> extract_fields` ordered two-hop traversal through `call_paths_between`, owner, and target path APIs | Exact call paths expose the same ordered function chain and source-node metadata | `code_item_call_path` returns the same real-corpus function reachability path | axum | Switch buckets; do not add more free-function breadth by default. |
| Inherent method one-hop | Covered/partial | Examples include `Json::from_bytes` and related method/associated-function rows | Exact call-context propagation exists | Exact lookup regressions exist | axum | Revisit only after broader buckets have at least one proof. |
| Exact local external-trait impl receiver methods | Met for now | 13 `Router::clone` typed-local/self-field receiver rows resolve to `impl<S> Clone for Router<S>::clone` | Exact call context preserves the same caller-site identities and receiver buckets | Remaining real-corpus TUI matrix includes `RouterClone` | axum | Switch buckets; do not broaden to arbitrary external trait dispatch by default. |
| Associated-function path calls | Covered/partial | `Self::from_bytes`, `E::from_request`, `MethodRouter::new` style rows | Impact/reach summaries carry relation/kind | Tool payloads carry relation/kind and callsite buckets | axum | Later: separate associated-function multi-hop bucket if needed. |
| Constructors | Stronger one-hop | Tuple struct / enum variant constructor rows are asserted in real-corpus matrix, including chrono alias constructor rows | Exact context propagation exists for direct and alias constructor rows | Tool regressions include direct and alias variant/constructor rows | axum, chrono | Later: multi-hop constructor path only if a real usage question requires it. |
| External dependency frontier | Covered as frontier | External rows are exposed but not traversed | Reach summaries expose external frontier calls | Tool summaries count/display frontier rows | axum | Keep fail-closed; do not convert to traversal without external-summary semantics. |
| Unsupported receiver shapes | Covered as blockers/frontier, with exact positive/frontier subsets | Targetless/unsupported rows remain for generic field/result/await/local receiver gaps; exact local `Router::clone` receiver rows traverse; initialized `Request::new` local receiver rows classify as external frontiers | RAG preserves blocker/frontier rows and the exact positive subset | Tool tests surface unsupported rows/counts and `RouterClone` positives | axum | Future implementation bucket after binding/type tracking plan. |
| Dynamic callable values | Stronger fixture-backed subset | Fixture-backed direct, alias, same-target branch/match, and single-expression block initialized callable values resolve; mixed-target branches remain targetless | RAG preserves resolved direct/branch/block initialized rows and blocker rows | `code_item_lookup` covers resolved dynamic-function callable bindings, including the block-initialized form, and the targetless matrix covers unsupported rows | local fixture; no representative found in checked-out axum/serde/memchr corpora | Switch buckets; broader callable trait objects/returned closures still need binding/body ownership work. |
| Proc-macro entrypoint body owners | Met for now | `CallBodyOwnerId::Macro` owners project through axum `expand_with` and `expand_attr_with` real-corpus helper calls | Exact impact summaries expose public proc-macro callers and direct helper callsites | `code_item_lookup` surfaces four public macro callers for `expand_with` | axum | Switch buckets; callback arguments and closure/IIFE body calls remain fail-closed. |
| Closures / nested body ownership | Partial/gap documented | Some nested rows intentionally absent to avoid flattening outer owners | RAG follows current DB surface | Tool coverage follows current DB surface | axum `parse_attrs` closure-body rows | Future bucket: closure/async body ownership before multi-hop closure traversal. |
| Import / re-export / glob completeness | Partial, nested-glob subset improved | Explicit and some imported path calls work; chrono alias constructor path calls resolve through typed alias evidence; axum `TestClient::new` now resolves 98 nested `test_helpers::* -> pub use test_client::*` rows while 69 direct/other import rows remain targetless; axum `Request::new` through imported `Request = http::Request` is classified external and targetless; broader import completeness remains partial | RAG exact context preserves alias rows and the 98 `TestClient::new` nested-glob caller rows | Tool coverage includes chrono alias rows plus dedicated high-fanout lookup/edges tests for `AxumRemainingTarget::TestClientNew` | axum, chrono | Switch buckets; keep strict source oracles for remaining fanout. |
| Dead-code / private zero-incoming | Met for now | `private_uncalled_nodes` lists axum `error_handling::traits`, excludes called `parse_attrs`, and exact impact reports no incoming source calls | Exact impact reports the same private target and empty caller/path/bucket sets | `code_item_lookup` reports zero incoming paths and zero impact caller counts | axum | Switch buckets; do not widen to generated/dynamic reachability here. |
| DB usage summaries | Strong current surface | `call_impact_for_target`, `call_reach_for_owner`, paths, source files/modules, buckets, boundary/frontier rows | N/A | N/A | axum | Add fields only when they answer a matrix question, not opportunistically. |
| RAG usage summaries | Strong current surface | N/A | Exact call paths, impact, reach, source metadata | N/A | axum | Add only when DB bucket already has proof. |
| TUI/tool usage summaries | Strong current surface | N/A | N/A | `code_item_lookup`, `code_item_edges`, and exact call-path tool coverage | axum | Keep tool changes thin; do not invent semantics outside RAG/DB. |

## Parking Lot

- Decide whether associated-function multi-hop needs a distinct proof row or can stay covered by the method/trait-method chain for now.
- Add binding tracking plan that ties syntax body ownership, local bindings, and type graph edges before attempting receiver/dynamic traversal.
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
