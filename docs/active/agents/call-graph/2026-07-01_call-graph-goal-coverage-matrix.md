# 2026-07-01 Call Graph Goal Coverage Matrix

Short description: workflow guardrail and status matrix for implementing the call-graph usage-question goal without over-focusing on the latest local concern.

Related planning files:
- [`2026-07-01_call-graph-larger-plan-map.md`](2026-07-01_call-graph-larger-plan-map.md)
- [`../2026-06-30_call-graph-usage-questions.md`](../2026-06-30_call-graph-usage-questions.md)
- [`2026-06-25_call-graph-coverage-inventory.md`](2026-06-25_call-graph-coverage-inventory.md)
- [`2026-06-28_real-corpus-call-site-case-matrix.md`](2026-06-28_real-corpus-call-site-case-matrix.md)
- [`2026-06-28_real-corpus-call-site-oracle-matrices.md`](2026-06-28_real-corpus-call-site-oracle-matrices.md)

Status date: 2026-07-05
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

Current bucket: executable-local body ownership.

Exit criteria:

- Preserve the existing fail-closed invariant that closure, async-block, and
  function-local item bodies are not flattened into their enclosing owner.
- Define the typed nested-owner identity and metadata contract before parser or
  DB code starts projecting nested executable-body rows.
- Avoid adding nested owners to `AnyNodeId` or using raw `Uuid` parser
  endpoints.
- Do not touch registered backup fixture schemas until the nested-owner
  projection is intentionally scheduled for fixture review/regeneration.

Completed evidence:

- Design checkpoint:
  [`2026-07-01_executable-local-owner-plan.md`](2026-07-01_executable-local-owner-plan.md).
- Existing parser and DB tests assert nested closure/async/local-const body
  calls are not attributed to the outer owner.
- Parser-side `ClosureBodyId`, `AsyncBlockBodyId`, and `ExecutableBodyId`
  facts now cover ordinary and `move` closure body calls plus async block body
  calls under `CallBodyOwnerId::Executable`.
- DB projection now persists `call_body_owner` metadata for ordinary closure
  and async block owners, keeps executable-local body calls off the outer
  function owner, and makes those owners snippet-materializable for RAG/tool
  expansion.
- Named closure bindings such as `let closure = || local_target(); closure()`
  now resolve the outer `closure()` path call to the closure owner, enabling
  the DB to traverse outer function -> closure body -> `local_target`.
- RAG and `request_code_context` now preserve the closure owner as both the
  incoming target of the binding call and the owner/caller for fixture-backed
  `|| local_target()` rows, preserve async block owners as callers for
  fixture-backed `async { local_target(); }` rows, and can expose projected
  proof facts for those executable-local owners.

Reason to stay in this bucket: only if the next slice targets a remaining
executable-local shape such as function-local executable items. Otherwise
switch buckets; ordinary closure-owner, async-closure-owner, and
async-block-owner DB/RAG/TUI propagation are met for now.

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
| External dependency frontier | Covered as frontier | External rows are exposed but not traversed | Reach summaries expose external frontier calls | Tool summaries count/display frontier rows | axum | Keep fail-closed; do not convert to traversal without external-summary semantics. |
| Unsupported receiver / trait-object shapes | Stronger blocker visibility plus bounded receiver positives | Targetless/unsupported rows remain for generic field/result/await/local receiver gaps; exact local `Router::clone` receiver rows traverse; borrowed initialized local receivers such as `let value = LocalAssoc; (&value).instance_value()` now carry initializer proof and resolve in fixture-backed DB/proof rows; same-target if-expression receivers such as `(if flag { LocalAssoc } else { LocalAssoc }).instance_value()` now carry branch-path proof and resolve in fixture-backed DB/proof rows; self-field method-result chains such as `self.value.clone_assoc().instance_value()` reuse self-field proof for the inner call and return-type proof for the outer call in fixture-backed parser/DB/proof rows; initialized `Request::new` local receiver rows classify as external frontiers; qualified trait-object qself calls such as `<dyn std::any::Any>::downcast_mut::<T>(...)` project as external targetless path rows in fixture-backed tests; unknown receiver expressions persist as `Unsupported` receiver rows instead of being dropped | RAG preserves blocker/frontier rows, the exact positive subset, fixture-backed unsupported receiver rows, borrowed-initialized, if-branch, self-field method-result receiver payloads, and qualified dyn Any external path rows | Tool tests surface unsupported rows/counts, `RouterClone` positives, `&value = LocalAssoc` receiver formatting, if-branch receiver formatting, self-field method-result call context/proof payloads, and qualified dyn Any external proof rows | axum plus local fixture fallback | Switch buckets; future implementation should add exact receiver proof by shape, not weaken unsupported rows; regenerate/review axum backup before turning real-corpus dyn Any rows positive. |
| Dynamic callable values | Stronger fixture-backed subset plus real-corpus targetless fallback | Fixture-backed direct, alias, same-target branch/match including guarded same-target match arms, single-expression block initialized callable values, exact `Box<dyn Fn()> = Box::new(local_target)` path/dynamic calls, named local closure binding `(closure)()` calls, local closure-binding `as fn` casts, pathless non-async closure literal calls, and direct returned-path calls such as `make_fn()()` resolve; async closure invocations remain targetless unsupported dynamic rows while their bodies are owned separately; mixed-target branches remain targetless; memchr function-pointer field rows remain unsupported and targetless by owner/source-line fanout | RAG preserves resolved direct/branch/block/guarded-match initialized rows, exact boxed dyn Fn initializer rows, direct returned-path dynamic rows, named and literal dynamic-closure rows, blocker rows, async-closure unsupported dynamic rows, and now preserves the two memchr function-pointer field blockers with argument counts 4 and 2 | `code_item_lookup` covers resolved dynamic-function callable rows, including block-initialized and guarded same-target match forms; DB/RAG/TUI owner/proof queries now cover direct returned-path dynamic calls; DB owner/caller/path queries now cover resolved `DynamicClosure` rows including closure literals and closure-binding `as fn` casts; lookup and edges targetless matrix covers axum callable-field rows plus memchr function-pointer field rows and blocked proof rows | local fixture; memchr real-corpus fallback | Switch buckets; broader callable trait objects without exact initializer proof, returned closures beyond direct returned function paths, and poll/resume edges for async closure futures still need binding/body/effect work. |
| Proc-macro entrypoint body owners | Met for now | `CallBodyOwnerId::Macro` owners project through axum `expand_with` and `expand_attr_with` real-corpus helper calls; regenerated axum closure callback rows traverse to `debug_handler::expand`; regenerated axum IIFE rows resolve to closure-owner dynamic targets while callable-parameter invocations remain fail-closed | Exact impact summaries expose public proc-macro callers and direct helper callsites | `code_item_lookup` surfaces four public macro callers for `expand_with` | axum | Switch buckets; callback argument value-flow remains fail-closed until interprocedural callable proof exists. |
| Closures / executable-local body ownership | Named closure-binding, non-async closure-literal, real-corpus closure body rows, fixture-backed async-closure body ownership, fixture-backed async-block traversal, and function-local const initializer ownership met for now; broader local item bodies remain future | Ordinary, `move`, and async closure body calls plus async block body calls and function-local const initializer calls are parser-owned by `CallBodyOwnerId::Executable`; `call_body_owner` projects closure, async-closure label, async block metadata, and local-item `local_const` metadata, validates them as call owners, keeps nested body calls off enclosing function owners, exposes `callers_for_target(...)` and one-hop paths from executable owners, resolves named closure binding calls and non-async closure literal dynamic calls to closure owners, proves outer function -> closure body -> `local_target` two-hop paths for non-async closure invocation, preserves async closure invocation as targetless until future poll/resume modeling, and snippet-materializes fixture-backed executable owners for RAG expansion | RAG collection, incoming expansion, and projected proof-context tests preserve fixture-backed closure, async-closure, and async block owners as callers for `local_target()` and preserve outer closure binding/literal calls to closure owners; local const initializer owners now have DB/snippet proof and should receive RAG/tool proof only after a dedicated surface slice | `request_code_context` tests materialize closure, async-closure, and async block owners with call context and projected proof rows, including move closure literal traversal; outer functions are not fabricated as direct nested-body callers | axum closure-owner rows are positive for selected source-oracle cases such as `from_ref` `expand_field` and `debug_handler::expand`; axum local const `HeaderValue::from_static` rows are projected as external targetless `LocalItem` owners; async-block rows are pinned targetless for selected `post`, `Self::from_request_parts`, and awaited `unwrap` cases; fixture-backed ordinary closure, non-async closure literal, async closure body, async block, and local const initializer cases are positive | Switch buckets unless the next planned slice explicitly targets broader function-local executable owners or async poll/resume effect edges. |
| Import / re-export / glob completeness | Partial, inherited-glob subset improved | Explicit and some imported path calls work; chrono alias constructor path calls resolve through typed alias evidence; fixture-backed `use super::*` inherited parent glob imports now resolve associated-function calls; regenerated axum backup data resolves 167 projected `TestClient::new` rows, including nested/direct re-export import rows and routing child-module inherited glob rows; the axum-core `request_parts.rs:193` row remains unsupported/targetless; axum `Request::new` through imported `Request = http::Request` is classified external and targetless; broader import completeness remains partial | RAG exact context preserves alias rows and the regenerated 167 `TestClient::new` resolved caller rows plus the targetless axum-core boundary row | Tool coverage includes chrono alias rows plus dedicated high-fanout lookup/edges tests for `AxumRemainingTarget::TestClientNew` | axum, chrono plus local fixture fallback | Keep strict source oracles for the remaining source-text/projection gaps and the axum-core targetless row; broaden import resolution by proof shape, not by weakening targetless rows. |
| Dead-code / private zero-incoming | Met for now | `private_uncalled_nodes` lists axum `error_handling::traits`, excludes called `parse_attrs`, and exact impact reports no incoming source calls | Exact impact reports the same private target and empty caller/path/bucket sets | `code_item_lookup` reports zero incoming paths and zero impact caller counts | axum | Switch buckets; do not widen to generated/dynamic reachability here. |
| DB usage summaries | Strong current surface | `call_impact_for_target`, `call_reach_for_owner`, paths, source files/modules, buckets, boundary/frontier rows | N/A | N/A | axum | Add fields only when they answer a matrix question, not opportunistically. |
| RAG usage summaries | Strong current surface | N/A | Exact call paths, impact, reach, source metadata | N/A | axum | Add only when DB bucket already has proof. |
| TUI/tool usage summaries | Strong current surface | N/A | N/A | `code_item_lookup`, `code_item_edges`, and exact call-path tool coverage; `code_item_edges` now carries the same existing impact/reach summaries in `node_info` that lookup exposes, with real-corpus assertions for paths, boundary edges, direct callsites, callsite buckets, source files/modules, and UI counts | axum | Keep tool changes thin; do not invent semantics outside RAG/DB. |

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
