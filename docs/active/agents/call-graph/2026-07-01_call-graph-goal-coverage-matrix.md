# 2026-07-01 Call Graph Goal Coverage Matrix

Short description: workflow guardrail and status matrix for implementing the call-graph usage-question goal without over-focusing on the latest local concern.

Related planning files:
- [`2026-07-01_call-graph-larger-plan-map.md`](2026-07-01_call-graph-larger-plan-map.md)
- [`../2026-06-30_call-graph-usage-questions.md`](../2026-06-30_call-graph-usage-questions.md)
- [`2026-06-25_call-graph-coverage-inventory.md`](2026-06-25_call-graph-coverage-inventory.md)
- [`2026-06-28_real-corpus-call-site-case-matrix.md`](2026-06-28_real-corpus-call-site-case-matrix.md)
- [`2026-06-28_real-corpus-call-site-oracle-matrices.md`](2026-06-28_real-corpus-call-site-oracle-matrices.md)
- [`2026-07-07_call-graph-usage-question-gap-audit.md`](2026-07-07_call-graph-usage-question-gap-audit.md)

Status date: 2026-07-10
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

Current bucket: select the next proof-carrier slice.

Exit criteria:

- Choose a row that has new source evidence or proof payload not already covered
  by the completed buckets below.
- Keep unsupported rows fail-closed unless the selected proof carrier can prove
  an exact local target.
- Add one DB assertion, one RAG assertion if exposed downstream, one TUI/tool
  assertion if exposed downstream, and one consolidated doc note.

Status: ready for selection; no implementation slice is currently selected.

Latest completed slice: dynamic self-field callee evidence and downstream
call-context path payloads. Parser, transform, DB, RAG, and exact TUI tests now
preserve targetless dynamic self-field callees from regenerated axum fixture
`corpus_axum_call_graph_2026-07-10`: `axum/src/boxed.rs:85`
`["self", "into_route"]`, `boxed.rs:120` `["self", "into_route"]`,
`boxed.rs:159` `["self", "layer"]`, and
`serve/listener.rs:236` `["self", "tap_fn"]`. These rows remain unsupported,
targetless, and edge-free with `dynamic_dispatch_unbounded` proof blockers.
`CallContextInfo.path` now exposes the persisted call-site path payload through
RAG and exact TUI `code_item_lookup` / `code_item_edges` surfaces without
promoting dynamic dispatch.

Previous completed bucket: build/test-domain summary surface for generated
test-entrypoint proof.

Latest completed slice: DB, RAG, and exact TUI lookup/edges surfaces now expose
linked `build_domain` proof metadata for the axum generated test-harness
`entrypoint_summary` on `error_handling::traits`. The query reads exact proof
links instead of proof-context text search, preserves zero incoming source-call
paths, and reports the admitted axum library build domain without blocker
reasons when cfg/rustc evidence is present. The direct
`code_private_uncalled` tool now also returns those build-domain rows next to
its entrypoint-summary proof rows. The follow-up typed entrypoint slice added
`call_test_entrypoints` / `CallTestEntrypointInfo` through DB, RAG, exact
lookup/edges, and `code_private_uncalled`, with real-corpus axum assertions
that preserve the admitted generated test-harness metadata without fabricating
source call edges.

Previous completed bucket: callable trait-object runtime-dispatch blocker proof
over real memchr boxed `dyn FnMut` path rows.

Latest completed slice: DB, RAG, and TUI tests now attach and preserve explicit
`dynamic_dispatch_unbounded` proof blockers for
`memchr/src/tests/substring/mod.rs:94,110` `Runner::run` boxed `dyn FnMut`
`fwd(...)` / `rev(...)` path callsites. The rows remain unsupported,
targetless, and edge-free until callable trait-object value-flow proof exists.

Next bucket: choose another new proof-carrier slice only if it has fresh proof
input. The 2026-07-10 inventory found that missing trait visibility proof,
public callable parameter blockers, fixture test-body macro blockers, dynamic
self-field evidence payloads, the axum-core `request_parts.rs:164` tuple-return
receiver row, routing helper macro/generated rows, generated
`IntoServiceFuture::new` rows, caller-supplied effect-policy allowlist queries,
exact tool policy inputs, generated test entrypoint payloads, and persisted
effect-policy proof annotations are now covered or intentionally fail-closed.
The memchr boxed `dyn FnMut` rows are likewise covered as explicit blockers.
Do not revisit those families unless the implementation adds new proof input
such as macro-expanded/generated source bodies, async poll/resume execution
proof, interprocedural callable argument/value-flow, broader callable
trait-object dispatch, additional source/sink policy annotation families, or
build/test entrypoint execution-policy summaries.

Previous completed bucket: persisted source/sink effect-policy proof
annotations.

Latest completed slice: proof graph storage now accepts strict
`effect_policy` artifacts, validates required policy metadata and
`allowed_effects`, and rejects ambiguous multiple admitted owner policies when
the stored-policy usage query is evaluated. DB and RAG real-corpus tests prove
the axum task-spawn sink is reported from the admitted owner policy without
creating local edges for `tokio::spawn`.

Previous completed bucket: exact tool-level source/sink effect-policy input.

Exit criteria:

- Reuse the existing DB/RAG effect-policy query and axum source-oracle chain
  `deserialize_error_status_codes -> TestClient::new -> spawn_service ->
  tokio::spawn`.
- Keep `tokio::spawn` an external targetless frontier; policy evaluation must
  never fabricate a local traversal edge.
- Add optional `allowed_effects` input to exact lookup/edges tools. When it is
  omitted, policy violations remain empty; when it is supplied, the tool
  payload reports reachable `effect_seed` rows whose effect class is outside
  the explicit allowlist.
- Assert exact `code_item_lookup` and `code_item_edges` report
  `async_task_spawn` as a violation when only `ffi_boundary` is allowed while
  preserving the same targetless external `tokio::spawn` callsite payload.

Status: completed for this checkpoint.

Latest completed slice: `code_item_lookup` and `code_item_edges` now accept an
optional `allowed_effects` array and expose
`call_effect_policy_violations` / `node_info.call_effect_policy_violations`
through the existing exact payloads. The axum task-spawn tool tests prove
`async_task_spawn` is reported outside an explicit `ffi_boundary` allowlist
while the original `tokio::spawn` row stays external, targetless, and
edge-free.

Previous completed bucket: explicit source/sink effect-policy query over
reachable effect seeds.

Latest completed slice for that bucket:
`Database::call_effect_policy_violations_for_owner` and
`RagService::exact_call_effect_policy_violations_for_owner` evaluate a
caller-supplied effect allowlist over existing reachable effect seeds. The axum
task-spawn oracle proves `async_task_spawn` is reported as a policy violation
from the upstream test owner while the original `tokio::spawn` callsite stays
external, targetless, and edge-free.

Previous completed bucket: public callable parameter blocker proof propagation.

Latest completed slice for that bucket: the remaining fixture-backed public
callable parameter rows are covered beyond the parser/DB context harness. DB
blocker proof queries, RAG proof-context collection, and TUI
request-code-context proof payloads preserve both path and dynamic
public-parameter blockers without inventing traversal edges.

Previous completed bucket: fixture test-body macro blocker proof propagation.

Exit criteria:

- Use the existing fixture source oracle for
  `call_graph_tests::assert_eq_macro_call`, where the `#[cfg(test)]` body calls
  `assert_eq!(1 + 1, 2)`.
- Keep the macro call targetless unsupported because macro expansion bodies and
  generated call edges are not modeled as source call graph edges.
- Assert DB context/proof, RAG proof-context, and TUI proof payload surfaces
  expose the `macro_expansion_not_available` blocker without traversal edges.
- Regenerate and verify active call-graph fixtures.

Status: completed for this checkpoint.

Latest completed slice: the fixture test-body `assert_eq!` macro row is now
covered beyond the parser harness. DB call-context/proof queries, RAG
proof-context collection, and TUI request-code-context proof payloads all
preserve the targetless macro blocker row.

Previous completed bucket: async closure callee evidence and proof-only
poll/resume blockers.

Exit criteria:

- Persist parser-side async closure callee evidence through transform/DB
  projection without flattening it into local traversal edges.
- Keep non-awaited async closure calls targetless unsupported because
  constructing the future does not prove poll/resume execution.
- Derive proof-only `dynamic_dispatch_unbounded` blockers for non-awaited
  async closure bindings/future bindings, and do not derive those blockers for
  awaited/resolved rows.
- Regenerate affected call-graph backup fixtures and update downstream
  DB/RAG/TUI proof assertions.

Status: completed for this checkpoint.

Latest completed slice: `call_callee_evidence` now preserves async closure
callee evidence in the persisted call graph. Proof projection derives
poll/resume blockers from that evidence for non-awaited async closure calls,
while awaited async closure rows remain normal resolved traversal candidates.
The active call-graph fixtures and committed real-corpus seeds were
regenerated/reviewed with the new relation.

Previous completed bucket: public callable field/indexed blocker proof
surfacing.

Exit criteria:

- Use the existing fixture source oracles for public callable field/indexed
  API boundaries: `call_field_function_param`,
  `call_indexed_function_pointer`, `call_indexed_field_function_param`, and
  `call_indexed_tuple_field_function_param`.
- Keep these rows targetless unsupported because public callers do not provide
  complete source-visible callable values.
- Assert the DB/RAG/TUI proof surfaces expose the
  `dynamic_dispatch_unbounded` blocker and do not emit traversal edges.

Status: completed for this checkpoint.

Latest completed slice: public callable field/indexed blockers now have
DB proof, RAG proof-context, and TUI lookup/edges proof payload coverage. This
is proof-surface hardening, not semantic expansion.

Previous completed bucket: conflicting callable field-parameter candidate
proof.

Exit criteria:

- Use the fixture source oracle where a private function has complete local
  callers but conflicting constructed holder-field arguments:
  `call_multi_conflicting_named_field_function_param(holder)`.
- Keep public/opaque callable fields and arrays targetless unsupported.
- Preserve the complete local candidate set as an `Ambiguous` dynamic-call row
  with `DynamicFunction` candidate relations and `candidate_def_ids`; do not
  emit resolved traversal edges or proof `call_edge` facts.
- Assert parser, DB context/proof, RAG collection, and TUI lookup/edges payloads
  for the candidate row.

Status: completed for that checkpoint.

Latest completed slice: private complete-local conflicting named-field
parameter proof now returns a candidate-only dynamic-call row for
`(holder.callback)()`. The shared parameter resolver now returns exact proof
for single/same-target caller sets and ambiguous candidate proof for
conflicting complete value or field caller sets.

Completed evidence:

- `fixture_call_graph_call_multi_conflicting_named_field_function_param_preserves_dynamic_candidates`
  asserts both candidate function IDs in the parser/resolver harness.
- `fixture_context_reads_projected_conflicting_callable_field_candidates`,
  `fixture_projection_marks_conflicting_callable_value_and_field_candidates`,
  and the target-centered candidate/proof lookup tests assert DB rows,
  `candidate_def_ids`, and no proof `call_edge`.
- `call_context_collection_reads_real_fixture_callable_path_rows` asserts RAG
  collection preserves the ambiguous dynamic-function candidate targets.
- `code_item_lookup_returns_function_pointer_param_blocker` and
  `code_item_edges_returns_function_pointer_param_blocker` now distinguish
  public blocked rows from private ambiguous path and dynamic candidate rows.

Previous completed bucket: conflicting callable value-parameter candidate
proof.

Completed evidence:

- `fixture_call_graph_call_multi_conflicting_function_pointer_param_preserves_path_candidates`
  and
  `fixture_call_graph_call_multi_conflicting_generic_fn_once_param_preserves_path_candidates`
  assert both candidate function IDs in the parser/resolver harness.
- `fixture_context_reads_projected_conflicting_callable_value_candidates`,
  `fixture_projection_marks_conflicting_callable_value_and_field_candidates`,
  and the target-centered candidate/proof lookup tests assert DB rows,
  `candidate_def_ids`, and no proof `call_edge`.
- `call_context_collection_reads_real_fixture_callable_path_rows` asserts RAG
  collection preserves the ambiguous candidate targets.
- `code_item_lookup_returns_function_pointer_param_blocker` and
  `code_item_edges_returns_function_pointer_param_blocker` distinguish public
  blocked rows from private ambiguous candidate rows.

Previous completed bucket: ambiguous branch-initialized path-call candidate
proof.

Exit criteria:

- Use the fixture source oracle
  `tests/fixture_crates/fixture_call_graph/src/lib.rs`, where
  `call_if_ambiguous_initialized_function_item_binding(flag)` binds
  `f: fn() -> i32 = if flag { local_target } else { other_target }; f()`.
- Do not fabricate a resolved traversal edge; preserve the complete local
  candidate set as an `Ambiguous` path-call row with `Function` candidate
  relations and `candidate_def_ids`.
- Assert owner-context, target-centered context, proof lookup, and cardinality
  validation for the new path/function candidate carrier.

Status: completed for this checkpoint.

Previous completed slice: parser extraction now records an
`AmbiguousInitializedValueBinding` path-call callee when a local callable
binding is branch-initialized by multiple proven local function items. The
resolver emits `Ambiguous` plus candidate `Function` relations only when every
candidate path resolves locally; DB proof validation permits exactly that
path/function candidate shape and still rejects other non-resolved local
targets.

Completed evidence:

- `fixture_call_graph_call_if_ambiguous_initialized_function_item_binding_preserves_path_candidates_call_site`
  asserts the structural callee carrier and both candidate function IDs.
- `fixture_context_reads_projected_function_item_binding_calls` asserts the
  owner-context row is ambiguous with path/function candidate targets.
- `fixture_context_reads_projected_ambiguous_dynamic_candidates`,
  `fixture_proof_symbol_lookup_matches_ambiguous_dynamic_candidate_payloads`,
  and `fixture_projected_status_rows_match_relation_cardinality` assert
  target-centered context, `candidate_def_ids`, no `call_edge`, and the narrow
  candidate-bearing ambiguous cardinality invariant.
- `proof_projection_rejects_non_resolved_local_targets` and
  `target_centered_proof_projection_rejects_non_resolved_local_targets` still
  reject invalid non-resolved local-target rows.

Previous completed bucket: generated-entrypoint summary proof for
private-uncalled tools.

Exit criteria:

- Reuse the existing real-corpus source oracle
  `axum/src/error_handling/mod.rs:257`, where private `#[test] fn traits()`
  has no persisted source callers but can be reached by the generated test
  harness.
- Keep `private_uncalled_nodes` as a source-call graph query; do not remove the
  node from zero-incoming results and do not fabricate generated harness call
  edges.
- Assert the `code_private_uncalled` payload exposes the admitted
  `entrypoint_summary` proof row for returned nodes.

Status: completed for this checkpoint.

Latest completed slice: `code_private_uncalled` now keeps its existing `nodes`
payload stable while adding `entrypoint_summaries` for returned nodes with
admitted generated-entrypoint proof.

Completed evidence:

- `AxumErrorHandlingTraitsToolFixture` already admits the real axum generated
  test-harness `entrypoint_summary` for `error_handling::traits`.
- `code_private_uncalled_lists_real_corpus_private_zero_caller_target` now
  requires the tool payload to include the admitted proof row with
  `target_kind = "test"` and `target_name = "generated-test-harness"`.
- The `nodes` list remains the raw persisted source-call zero-incoming result;
  generated entrypoint summaries remain proof context, not local call edges.
- Verification passed:
  `cargo test -p ploke-tui --test integration code_private_uncalled -- --nocapture`
  and `cargo check -p ploke-tui`.

Next bucket: return to semantic expansion only if there is a new exact
parser-owned proof carrier, or choose another usage-question proof surface only
when an existing proof artifact is not exposed through DB/RAG/TUI.

Previous completed bucket: feature-gated serde_json admitted-summary tool proof.

Exit criteria:

- Reuse the existing real-corpus source oracle
  `axum/src/json.rs:184`, where `Json::from_bytes` calls
  `serde_json::Deserializer::from_slice(bytes)` under the inherited
  `feature = "json"` cfg.
- Do not resolve the external dependency call to a local target; keep it
  targetless and externally summarized only through an admitted summary fact.
- Assert exact lookup and edge tool payloads expose the admitted
  `external_summary` artifact and no longer list the callsite as an
  `external_summary_need` after upsert.

Status: completed for this checkpoint.

Previous completed slice: the existing real-corpus `Json::from_bytes` TUI tests
now admit the feature-gated serde_json frontier summary before tool execution
and require the tool payloads to expose the admitted proof rows.

Completed evidence:

- `AxumJsonFromBytesToolFixture::admit_serde_summary` pins the real axum
  `serde_json::Deserializer::from_slice(bytes)` callsite and reuses
  `axum_serde_json_from_slice_summary_records`.
- Exact lookup
  `code_item_lookup_returns_real_corpus_json_from_bytes_callers` now requires
  the admitted serde summary proof and absence from `external_summary_needs`.
- Exact edges
  `code_item_edges_returns_real_corpus_json_from_bytes_callers` covers the
  same admitted-summary proof through `node_info`.
- This does not change parser extraction, resolver behavior, DB query
  semantics, or fixture bytes.

Previous completed bucket: exact closure-binding cast proof propagation.

Exit criteria:

- Reuse the fixture-backed source oracle
  `call_closure_binding_cast`, where `let closure = || 21; (closure as fn() ->
  i32)()` carries exact local closure-binding proof.
- Keep the target as the closure executable owner, not `local_target`; do not
  generalize to arbitrary callable casts or trait-object dispatch.
- Assert DB proof projection, RAG call-context collection, and exact
  `request_code_context` preserve the `DynamicClosure` edge to the closure
  owner.

Status: completed for this checkpoint.

Latest completed slice: closure-binding cast now has the same downstream
positive proof coverage as the existing dereferenced closure-binding sibling.

Completed evidence:

- DB `fixture_projection_stores_closure_binding_dynamic_proof_facts` now
  batches `call_closure_binding_cast` and `call_dereferenced_closure_binding`
  through the same closure-binding proof helper.
- RAG `call_context_collection_reads_real_fixture_dynamic_rows` now requires
  both closure-binding forms to preserve a resolved `DynamicClosure` target.
- Exact TUI
  `request_code_context_returns_closure_binding_cast_owner_call_context`
  materializes the closure executable target and verifies the outgoing dynamic
  closure edge.
- This does not change parser extraction, resolver behavior, or fixture bytes.

Next bucket: re-check the matrix before adding more dynamic-call coverage; do
not continue in dynamic callable positives unless the next row adds a distinct
proof input or missing downstream surface.

Previous completed bucket: missing-trait-visibility blocker proof propagation.

Exit criteria:

- Reuse the fixture-backed source oracle
  `trait_scope::without_trait_import::call_unimported_trait_method`, where the
  receiver type is local but the trait method is not visible in scope.
- Keep the method call targetless and `Unsupported`; do not import the trait,
  broaden trait dispatch, or fabricate a local edge.
- Assert DB proof projection, RAG proof context, and exact
  `request_code_context` proof payloads expose the existing
  `type_resolution_missing` blocker for the nested owner.

Status: completed for this checkpoint.

Previous completed slice: missing trait visibility now has the same blocker
proof propagation as the existing macro, ambiguous, external, and callable
fixture blockers.

Completed evidence:

- DB
  `fixture_projection_marks_unimported_trait_method_without_edges` asserts
  `value.scoped_value()` remains targetless `Unsupported` and projects the
  owner-linked `type_resolution_missing` proof row.
- RAG
  `proof_context_collection_preserves_projected_blocker_rows` now includes the
  nested `trait_scope::without_trait_import` owner and preserves the blocker in
  exact proof context.
- Exact TUI
  `request_code_context_returns_fixture_blocker_proof_context` now includes
  the same nested owner and proof blocker in the tool payload.
- This does not change parser extraction, resolver behavior, or traversal
  edges.

Next bucket: choose a distinct unresolved proof input from the remaining
focused unsupported inventory; do not continue adding simple blocker-propagation
rows unless a downstream surface is missing coverage.

Previous completed bucket: bounded same-block async-closure future alias-chain
proof.

Exit criteria:

- Add one fixture-backed source oracle where an async closure future is awaited
  through a direct same-block alias chain:
  `future = closure(); alias = future; second = alias; second.await`.
- Reuse the existing awaited async-closure binding payload and traversal edge;
  do not add general async poll/resume modeling or arbitrary future value flow.
- Assert parser, DB traversal, RAG collection, and exact TUI
  `request_code_context` preserve the two-hop path from outer function to
  async-closure owner to `local_target`.

Status: completed for this checkpoint.

Previous completed slice: fixture-backed
`call_awaited_async_closure_future_alias_chain_with_body_call` now proves the
existing same-block future alias tracker covers a two-step direct alias chain.

Completed evidence:

- Parser paranoid coverage asserts the original `closure()` call is recorded as
  `AwaitedAsyncClosureBinding` and resolves to the async-closure executable
  owner, while the closure body `local_target()` remains owned by the
  async-closure owner.
- DB `fixture_context_resolves_awaited_async_closure_future_alias_chain_to_executable_owner`
  asserts the persisted two-hop traversal
  outer function -> async-closure owner -> `local_target`.
- RAG
  `call_context_collection_resolves_awaited_async_closure_future_alias_chain_rows`
  preserves the same outgoing closure edge and nested body call.
- Exact TUI
  `request_code_context_returns_awaited_async_closure_future_alias_chain_owner_call_context`
  preserves the same expansion and call-context payload.
- This does not resolve non-awaited async-closure futures, returned futures,
  async callable trait objects, or general poll/resume semantics.

Next bucket: switch away from async-closure alias coverage unless the next row
adds a distinct exact proof carrier. Re-check the remaining focused unsupported
inventory before selecting another binding/type-aware semantic slice.

Previous completed bucket: proof-only dependency-root carrier for the real axum
`Body::empty` re-export import row.

Exit criteria:

- Reuse the existing resolved `Body::empty` target-centered caller set.
- Add a second exact source oracle for
  `axum/src/extract/raw_form.rs:65`, where `crate::body::Body` re-exports the
  `axum_core::body::Body::empty` workspace target.
- Keep the call graph traversal unchanged; this is an admitted proof-only
  dependency-root record, not broader dependency-root import resolution.
- Assert DB, RAG exact proof context, and exact TUI lookup/edge proof payloads
  expose both the direct `form.rs` row and the re-exported `raw_form.rs` row.

Status: completed for this checkpoint.

Previous completed slice: Body::empty dependency-root proof now covers the
direct `axum/src/form.rs:158` row and the `crate::body::Body` re-export row at
`axum/src/extract/raw_form.rs:65`.

Completed evidence:

- `axum_body_empty_reexport_dependency_record` records the raw-form source
  oracle with `import_path = ["crate", "body", "Body"]` and the same resolved
  workspace target `axum_core::body::Body::empty`.
- DB `axum_real_target_body_empty_projects_proof_facts` now inserts and
  asserts both admitted dependency-root proof rows.
- RAG `proof_context_exact_preserves_axum_supported_target_rows` now requires
  both proof rows through exact proof context.
- Exact TUI `body_empty` lookup/edges tests now require both dependency-root
  proof sites in tool proof payloads.
- This does not change the existing twenty-three `Body::empty` caller edges,
  resolution status, or traversal behavior.
- Verification passed:
  `cargo check -p ploke-test-utils`,
  `cargo test -p ploke-db axum_real_target_body_empty_projects_proof_facts -- --nocapture`,
  `cargo test -p ploke-rag proof_context_exact_preserves_axum_supported_target_rows -- --nocapture`,
  and `cargo test -p ploke-tui --test integration body_empty -- --nocapture`.

Next bucket: choose the next binding/type-aware semantic row from the plan's
remaining proof-input list. Do not add more dependency-root records unless the
next row has a new exact source oracle and DB/RAG/TUI proof value.

Previous completed slice: shared TUI matrix proof payloads now preserve the
generic-array guarded `size_hint` explicit blockers.

Completed evidence:

- `call_shape_case_proof_blockers` and `call_shape_case_blocker_reasons`
  attach the two source-oracle blockers to the existing shared call-shape
  matrix rows for `generic_array_try_from_iter_size_hint_local_receiver` and
  `generic_array_try_from_fallible_iter_size_hint_local_receiver`.
- The TUI shared-matrix fixture upserts those case-owned blockers before
  building `AppState`, then `assert_proof_context` requires both
  `type_resolution_missing` and `external_dependency_summary_missing` blocker
  rows in exact lookup and edge-tool proof payloads.
- This is downstream proof propagation only. It does not resolve the receiver,
  create a traversal edge, or relax the targetless status.
- Verification passed:
  `cargo run -p xtask --features call_graph -- fixtures regenerate --active`,
  `cargo run -p xtask --features call_graph -- verify-backup-dbs`,
  `cargo check -p ploke-test-utils`, and
  `cargo test -p ploke-tui --test integration call_graph_tool_shared_matrix -- --nocapture`.

Previous completed slice: feature-gated serde_json external-summary frontier
proof.

Completed evidence:

- DB `axum_usage_questions_list_external_summary_need_for_feature_gated_json_frontier`
  proves the `serde_json::Deserializer::from_slice(bytes)` row is a direct,
  targetless external frontier, preserves `feature = "json"`, appears in
  `external_summary_needs_for_owner`, and disappears after an admitted
  `external_summary` proof record is linked.
- RAG
  `external_summary_needs_exact_reads_axum_feature_gated_json_queue` preserves
  the same owner-scoped missing-summary queue and discharge behavior through
  `exact_external_summary_needs_for_owner`.
- Exact TUI `code_item_lookup` and `code_item_edges` `Json::from_bytes` tests
  now expose the same `external_summary_needs` payload and UI count while the
  call remains targetless.
- The shared test proof fixture
  `axum_serde_json_from_slice_summary_records` mirrors the existing
  `Request::builder` and `std::mem::replace` admitted-summary pattern.

Previous completed bucket: active call-graph corpus fixture refresh plus
`Default`-bound associated external frontier proof.

Completed evidence:

- The active fixture set was regenerated with
  `cargo run -p xtask --features call_graph -- fixtures regenerate --active`;
  the refreshed memchr, generic-array, chrono, and axum corpus snapshots were
  promoted into `tests/backup_dbs/`.
- Registry-backed verification passed with
  `cargo run -p xtask --features call_graph -- verify-backup-dbs`.
- DB fallback tests now assert the memchr function-pointer field rows and boxed
  `dyn FnMut` callable-field rows remain targetless blockers with projected
  `dynamic_dispatch_unbounded` or `type_resolution_missing` proof rows and
  source provenance.
- Parser, DB, RAG, and TUI proof/context tests now cover fixture-backed
  `T::default()` as an external/prelude `Default`-bound associated path
  frontier. The row remains targetless and does not claim concrete trait
  dispatch.
- `2026-07-05_binding-type-aware-resolver-plan.md` now contains a consolidated
  remaining focused unsupported inventory. The next semantic slice should add
  a new exact proof carrier or an explicitly reviewed source oracle, not
  re-open rows that are already intentionally fail-closed.

Previous completed bucket: axum impl Trait parameter external frontier proof.

Exit criteria:

- Reuse the real-corpus axum row
  `axum-core/src/error.rs:12-14` where `Error::new(error: impl
  Into<BoxError>)` calls `error.into()`.
- Promote the row to a targetless external frontier only through the
  source-visible `impl Into<BoxError>` parameter bound.
- Keep the row targetless, with no fabricated local traversal target.
- Regenerate and promote the axum call-graph fixture seed so normal immutable
  fixture tests exercise the new status instead of the older committed seed.

Status: completed for the real-corpus axum impl Trait parameter external
frontier slice.

Latest completed slice: exact external-frontier classification for
`Error::new(error: impl Into<BoxError>)` `error.into()`.

Completed evidence:

- DB
  `axum_real_target_impl_trait_into_parameter_is_external_frontier` asserts
  the `error.into()` row in `axum-core/src/error.rs:14` is a targetless
  `External` frontier with `LocalBinding("error")`.
- The initial normal immutable fixture run exposed the expected stale-seed
  failure (`Unresolved`). The isolated regenerated snapshot passed, proving
  the resolver and regenerated snapshot were correct before seed promotion.
- Active fixture regeneration and backup verification passed with
  `cargo run -p xtask --features call_graph -- fixtures regenerate --active`
  and `cargo run -p xtask --features call_graph -- verify-backup-dbs`.
- This intentionally does not add a local traversal edge or claim concrete
  `Into` dispatch; it only preserves the external/prelude frontier status.

Previous completed bucket: generic self-field external trait-bound frontier
proof.

Completed evidence:

- Reuse the parser-pinned fixture source rows
  `fixture_nodes/src/impls.rs:77` `self.value.len()` and `:103`
  `self.value.into()`.
- Reuse the exact generic-bound source proof already used for external
  `Service::poll_ready` forwarding.
- Promote `GenericStruct<T> as SimpleTrait::trait_method`
  `self.value.into()` to a targetless external frontier only through the
  source-visible external `Into<i32>` trait bound.
- Keep both generic self-field rows targetless, with no fabricated local
  traversal targets.
- Prove parser, DB, and RAG call-context collection preserve the external
  frontier statuses without local targets.

Next bucket: choose the next uncovered matrix bucket. Do not continue adding
receiver-shape assertions unless the next slice implements exact receiver proof
by shape or covers a missing downstream surface for an already-modeled shape.

Previous completed slice: exact external-frontier classification for the
`GenericStruct<T>` self-field `into` receiver through the external `Into<i32>`
where-bound.

Completed evidence:

- Parser
  `fixture_nodes_generic_simple_trait_method_records_self_field_into_method_call_site`
  now asserts the `self.value.into()` row is targetless `External` with
  `SelfField(["value"])`, using the `T: Into<i32>` impl where-bound.
- DB `fixture_context_reads_generic_self_field_receiver_targetless_statuses`
  asserts both `self.value.len()` and `self.value.into()` are targetless
  `External` frontiers.
- RAG `call_context_collection_reads_generic_self_field_receiver_frontiers`
  asserts the same frontiers survive call-context collection as
  `CallReceiverInfo::SelfField { path: ["value"] }` without targets.
- This intentionally does not add a TUI assertion because the TUI targetless
  receiver matrix already covers current real-corpus unsupported receiver
  payloads; this slice only adds the missing fixture-backed generic field proof.
- Verification passed:
  `cargo test -p syn_parser --features call_graph fixture_nodes_generic_simple_trait_method_records_self_field_into_method_call_site -- --nocapture`,
  `cargo test -p ploke-db fixture_context_reads_generic_self_field_receiver_targetless_statuses -- --nocapture`,
  `cargo test -p ploke-rag call_context_collection_reads_generic_self_field_receiver_frontiers -- --nocapture`,
  `cargo fmt --all --check`, and `git diff --check`.
- This is exact external-frontier promotion over parser-pinned receiver
  evidence. It does not convert any row into a local traversal edge.

Previous completed slice: exact external-frontier classification for the
`GenericStruct<&str>` self-field `len` receiver.

Completed evidence:

- Parser `fixture_nodes_get_str_len_records_self_field_len_method_call_site`
  now asserts the `self.value.len()` row is targetless `External` with
  `SelfField(["value"])`, using the concrete `&str` impl argument.
- DB and RAG targetless self-field receiver tests preserved the row as an
  external frontier without local targets.
- This did not broaden receiver resolution or convert targetless rows into
  traversal edges.

Previous completed slice: deep explicit local path coverage.

Completed evidence:

- Added fixture source oracles for
  `deep_path_root::call_self_deep_path_target() -> self::branch::leaf::deep_target()`,
  root `call_crate_deep_path_target() -> crate::deep_path_root::branch::leaf::deep_target()`,
  and root `call_self_deep_path_target() -> self::deep_path_root::branch::leaf::deep_target()`.
- Parser paranoid tests prove all three calls preserve their full observed path
  segments and resolve to `deep_path_root::branch::leaf::deep_target`.
- Existing DB path-context and proof tables now include the three deep explicit
  path forms, so persisted call graph rows and projected proof rows cover more
  than one nested module segment.
- RAG path-family expansion now propagates the same three resolved path rows to
  the deep target.
- Verification passed with focused parser, DB, and RAG tests, plus
  `cargo run -p xtask --features call_graph -- fixtures regenerate --active`,
  `cargo run -p xtask --features call_graph -- verify-backup-dbs`,
  `cargo fmt --all --check`, and `git diff --check`.
- This is coverage over existing resolver behavior. It does not broaden
  import/re-export/glob semantics, dependency-root imports, or macro-expanded
  path resolution.

Latest completed slice: parameter-alias method receiver proof.

Completed evidence:

- Added fixture source oracle
  `call_param_alias_instance_method(value: LocalAssoc) { let alias = value; alias.instance_value() }`.
- Parser extraction now records the method receiver as
  `AliasedLocalBinding { name: "alias", source_path: ["value"] }` instead of
  dropping the value-alias proof to `Unsupported`.
- Resolver proof remains narrow: only one-segment alias sources are followed,
  and the target is resolved by reusing the existing parameter method proof for
  `value: LocalAssoc`.
- Transform, DB decode, and RAG receiver payloads preserve the new receiver
  shape; DB context/proof tests and RAG incoming expansion tests assert the
  resolved `LocalAssoc::instance_value` edge.
- Verification passed with focused parser, DB, and RAG tests, plus
  `cargo run -p xtask --features call_graph -- fixtures regenerate --active`,
  `cargo run -p xtask --features call_graph -- verify-backup-dbs`,
  `cargo fmt --all --check`, and `git diff --check`.
- This remains bounded to direct local aliases of visible parameters. Borrowed,
  dereferenced, field, multi-segment, external, and arbitrary expression alias
  sources remain future binding/type-flow work.

Latest completed slice: real-corpus callback-parameter runtime-dispatch
blocker proof.

Completed evidence:

- The source oracle is `axum-macros/src/lib.rs:727,734-738`, where
  `expand_attr_with` accepts `f: F` with a callable bound, immediately invokes
  a closure, and the closure body calls `f(attr, input)`.
- The persisted call graph keeps the closure-owned `f(attr, input)` path row
  `Unsupported`, targetless, and absent from traversal edges.
- DB, RAG proof context, and exact TUI `code_item_lookup` / `code_item_edges`
  tests now attach and expose an explicit `dynamic_dispatch_unbounded`
  `proof_blocker` for that same callsite, while preserving the projected
  `type_resolution_missing` call-resolution row.
- This is proof visibility only. It does not perform interprocedural callback
  argument flow and does not fabricate a local callee edge for `f`.

Latest completed slice: complete private multi-caller named-field holder
callable proof.

Completed evidence:

- The source oracles are
  `tests/fixture_crates/fixture_call_graph/src/lib.rs:1847-1877`.
  `call_multi_named_field_function_param(holder: CallbackHolder)` calls
  `(holder.callback)()`, and both local callers construct
  `CallbackHolder { callback: local_target }`.
- Parser extraction records the dynamic callee as the existing
  field-local-binding shape; resolver proof reuses the complete private
  caller-set boundary and resolves the dynamic call to `local_target`.
- The paired
  `call_multi_conflicting_named_field_function_param` fixture has two local
  callers that pass different function items. It now preserves both local
  function items as candidate-only ambiguous `DynamicFunction` targets; no
  resolved traversal edge is fabricated.
- DB owner context, target-centered callers, one-hop paths, dynamic proof
  rows, RAG call/proof collection, and exact tool payloads preserve the
  positive edge, incoming wrapper helper rows, and candidate-only ambiguous
  conflicting row.
- This is not arbitrary callable holder/value-flow dispatch. It is only the
  exact private helper case where the complete local caller set proves one
  function target.

Latest completed slice in current bucket: branch-initialized local receiver
proof.

Completed evidence:

- The source oracles are
  `tests/fixture_crates/fixture_call_graph/src/lib.rs:1834-1845`, where
  `let value = if flag { LocalAssoc } else { LocalAssoc };
  value.instance_value()` and the equivalent `match` initializer prove
  `value` has the exact `LocalAssoc` initializer.
- Parser extraction reuses the existing `branch_init_path` local-binding
  evidence and records both receiver calls as
  `InitializedLocalBinding { name: "value", init_path: ["LocalAssoc"] }`.
- DB owner context, resolved proof rows, and target-centered caller/proof rows
  prove both exact local method edges to `LocalAssoc::instance_value`.
- RAG call-context collection preserves the same resolved receiver payload and
  target for both owners.
- Exact TUI `code_item_lookup` and `code_item_edges` payloads preserve the
  same outgoing call-context and proof rows for both owners.
- Verification passed:
  `cargo test -p syn_parser fixture_call_graph_call_if_initialized_local_instance_method_resolves_initialized_receiver_method_call_site -- --nocapture`,
  `cargo test -p syn_parser fixture_call_graph_call_match_initialized_local_instance_method_resolves_initialized_receiver_method_call_site -- --nocapture`,
  `cargo test -p ploke-db fixture_context_reads_projected_local_and_alias_instance_method_receivers -- --nocapture`,
  `cargo test -p ploke-db fixture_projection_stores_real_local_receiver_method_call_proof_facts -- --nocapture`,
  `cargo test -p ploke-db fixture_projection_stores_real_target_centered_method_call_proof_facts -- --nocapture`, and
  `cargo test -p ploke-rag call_context_collection_reads_branch_initialized_local_receiver_rows -- --nocapture`.
  TUI tool verification passed:
  `cargo test -p ploke-tui --test integration code_item_lookup_returns_branch_initialized_receiver_method_context -- --nocapture` and
  `cargo test -p ploke-tui --test integration code_item_edges_returns_branch_initialized_receiver_method_context -- --nocapture`.

Previously completed slice: match struct-pattern initialized receiver proof.

Completed evidence:

- The source oracle is
  `tests/fixture_crates/fixture_call_graph/src/lib.rs:1828-1831`, where
  `match (ParamFieldMethodReceiver { value: LocalAssoc }) {
  ParamFieldMethodReceiver { value } => value.instance_value() }` proves
  `value` has the exact `LocalAssoc` initializer.
- Parser extraction now derives the arm-local binding from matching
  struct-pattern and struct-expression field names, then reuses
  `InitializedLocalBinding { name: "value", init_path: ["LocalAssoc"] }`.
- DB owner context and resolved proof rows prove the exact local method edge
  to `LocalAssoc::instance_value`.
- RAG call-context collection preserves the same resolved receiver payload and
  target.
- Verification passed:
  `cargo test -p syn_parser fixture_call_graph_call_match_struct_pattern_initialized_receiver_method_resolves_call_site -- --nocapture`,
  `cargo test -p ploke-db fixture_context_reads_projected_match_struct_pattern_initialized_receiver -- --nocapture`,
  `cargo test -p ploke-db fixture_projection_stores_real_local_receiver_method_call_proof_facts -- --nocapture`, and
  `cargo test -p ploke-rag call_context_collection_reads_match_struct_pattern_receiver_rows -- --nocapture`.

Previously completed slice: same-block async-closure future block-alias proof.

Completed evidence:

- The source oracle is
  `tests/fixture_crates/fixture_call_graph/src/lib.rs:1821-1825`, where
  `future = closure(); alias = { future }; alias.await;` proves the original
  `closure()` future binding is awaited through a single-expression block
  alias in the same block.
- Parser extraction now accepts that block alias only when it reduces to a
  path expression, and marks the original async-closure binding call as
  `AwaitedAsyncClosureBinding`.
- DB owner context and traversal prove the two-hop path
  outer function -> async-closure owner -> `local_target`.
- RAG call-context collection preserves the same resolved closure edge and
  nested body call.
- Verification passed:
  `cargo test -p syn_parser fixture_call_graph_call_awaited_async_closure_future_block_alias_resolves_path_call_site -- --nocapture`,
  `cargo test -p ploke-db fixture_context_resolves_awaited_async_closure_future_block_alias_to_executable_owner -- --nocapture`, and
  `cargo test -p ploke-rag call_context_collection_resolves_awaited_async_closure_future_block_alias_rows -- --nocapture`.

Previously completed slice: explicit blocker proof for the real generic-array
guarded `size_hint` receiver rows.

Completed evidence:

- The source oracle is
  `tests/fixture_github_clones/corpus/generic-array/src/lib.rs:1239` and
  `tests/fixture_github_clones/corpus/generic-array/src/lib.rs:1276`, where
  guarded match arms call `iter.size_hint()`.
- The persisted call graph keeps both rows `Unsupported`, with no
  `call_relation` targets and no traversal candidates.
- DB proof lookup and GraphRAG proof context expose explicit
  `type_resolution_missing` blockers for both callsite ids. The same oracle
  now also attaches an `external_dependency_summary_missing` blocker naming
  the missing proof input: an external summary for
  `IntoIterator::into_iter` that proves the returned iterator supports
  `Iterator::size_hint`.
- This does not resolve the receiver and does not create traversal edges; it
  keeps the row fail-closed while documenting why local binding proof alone is
  insufficient.
- The shared TUI call-shape matrix now reuses the same matrix-owned blocker
  facts and asserts exact `code_item_lookup` / `code_item_edges` proof payloads
  include both blockers for each row.
- Verification passed:
  `cargo run -p xtask --features call_graph -- fixtures regenerate --active`,
  `cargo run -p xtask --features call_graph -- verify-backup-dbs`,
  `cargo test -p ploke-db generic_array_guarded_match_arm_method_guard_is_targetless_fallback_oracle -- --nocapture`,
  `cargo fmt --all --check`, and `git diff --check`.

Previously completed slice: proof-only dependency-root carrier for
the real axum `Body::empty` direct workspace import row.

Completed evidence:

- The source oracle is `tests/fixture_github_clones/corpus/axum/axum/src/form.rs:158`,
  where the test module imports `axum_core::body::Body` and calls
  `Body::empty()` from `check_query`.
- The existing call graph still resolves all twenty-three `Body::empty` caller
  rows to `axum_core::body::Body::empty`; this slice admits only one
  proof-only `dependency_root` row for the direct `form.rs` import site.
- DB proof lookup, RAG exact proof context, and exact `code_item_lookup` /
  `code_item_edges` Body::empty tests expose the same admitted
  `dependency_root` row without changing traversal or target counts.
- Verification passed:
  `cargo test -p ploke-db axum_real_target_body_empty_projects_proof_facts -- --nocapture`,
  `cargo test -p ploke-rag proof_context_exact_preserves_axum_supported_target_rows -- --nocapture`,
  `cargo test -p ploke-tui code_item_lookup_returns_real_corpus_body_empty_callers -- --nocapture`,
  `cargo test -p ploke-tui code_item_edges_returns_real_corpus_body_empty_callers -- --nocapture`,
  `cargo fmt --all --check`, and `git diff --check`.

Previously completed slice in current bucket: exact parameter-field receiver
method proof for a fixture-backed local struct field.

Completed evidence:

- Added the `fixture_call_graph` oracle
  `call_param_field_instance_method(holder: ParamFieldMethodReceiver)` with
  `holder.value.instance_value()`, where `ParamFieldMethodReceiver::value` has
  the exact local type `LocalAssoc`.
- The parser already records this shape as
  `MethodCallReceiver::FieldLocalBinding { name: "holder", field_path:
  ["value"] }`; the method resolver now reuses owner parameter type evidence
  plus the existing field-type method resolver to admit only this exact local
  proof.
- DB owner context, resolved proof projection, target-centered callers, and
  caller expansion all preserve the new `Method` edge to
  `LocalAssoc::instance_value`.
- RAG call-context collection, RAG proof context, RAG target expansion, and the
  exact `request_code_context` call/proof payloads preserve the same receiver
  and proof rows.
- This does not broaden to external field receivers, autoderef, trait-object
  dispatch, dynamic callable fields, or multi-field parameter walks without
  exact local field-type proof.

Previously completed slice in current bucket: owner-scoped external summary need
queue exposed through DB, RAG, and exact TUI tools. Detailed evidence is
recorded below under the same heading.

Previously completed slice in current bucket: generated `routing::post`
macro-boundary summary proof over a real axum grouped-import free-function
frontier.

Completed evidence:

- The axum real-corpus grouped-import source oracle remains
  `axum/src/json.rs:237` importing `routing::post`, with
  `axum/src/json.rs:248` calling `post(echo_json)` from `deserialize_body`.
- The generated function binding is still absent because
  `axum/src/routing/method_routing.rs:445` expands
  `top_level_handler_fn!(post, POST)` from the macro template at
  `method_routing.rs:165`; the call graph keeps the `post` callsite
  `Unsupported`, targetless, and absent from traversal edges.
- DB, RAG, `code_item_lookup`, and `code_item_edges` tests now admit and
  expose a callsite-linked `expansion_boundary` plus admitted
  `external_summary` artifact for that generated-function boundary while the
  projected `call_resolution` remains blocked on `type_resolution_missing`.
- This is proof visibility only. It does not expand the macro, does not create
  a generated `post` function node, and does not fabricate a local call edge.

Previously completed downstream consistency slice: generated test-entrypoint
summary through `code_item_edges`.

Completed evidence:

- The axum real-corpus private zero-source-caller target
  `error_handling::traits` still reports no persisted incoming source paths or
  impact callers.
- Exact `code_item_edges` now matches `code_item_lookup` by exposing the
  admitted generated test-harness `entrypoint_summary` in
  `node_info.proof_context`, while preserving zero incoming paths and empty
  source-caller impact sets. No generated harness source edge is fabricated.

Previously completed slice in current bucket: exact request-parts external
tuple-return summary over the real axum-core turbofish receiver row.

Completed evidence:

- The axum-core `request_parts.rs:164`
  `parts.extract_with_state::<State<String>, String>(&state)` row now resolves
  to `RequestPartsExt for Parts::extract_with_state` while preserving the
  `TupleMethodReturn(parts, into_parts, index 0)` receiver proof and the two
  explicit method generic arguments.
- Added the parser-side exact summary carrier for the source-visible shape
  `http::Request::into_parts -> (http::request::Parts, _)`, index 0 only. The
  carrier feeds existing local extension-trait receiver matching; it does not
  introduce broad external method traversal.
- DB, RAG, `code_item_lookup`, and `code_item_edges` tests now assert the
  resolved local edge and proof rows for the same site.
  `cargo fmt --all --check`, and `git diff --check`.

Previously completed slice in current bucket: workspace dependency-root proof carrier
over real axum `TestClient::new` workspace glob rows.

Completed evidence:

- The axum-core `request_parts.rs:193` `TestClient::new(...)` source oracle
  imports `axum::{test_helpers::*, Router}` from another workspace crate and
  still traverses to `axum::test_helpers::TestClient::new` through one
  resolved associated-function edge.
- Added an exact proof-only `dependency_root` record for that callsite, with
  `import_path = ["axum", "test_helpers", "*"]` and
  `resolved_path = ["axum", "test_helpers", "TestClient", "new"]`.
- DB proof context, RAG exact proof context, and exact `code_item_lookup` /
  `code_item_edges` high-fanout `TestClient::new` tests expose the same
  `dependency_root` row without changing the 168 incoming caller rows.

Previously completed slice in current bucket: workspace dependency-root proof carrier
over real axum `FromRef::from_ref` dependency-root rows.

Completed evidence:

- Added a strict `dependency_root` proof fact kind. The fact requires the
  source callsite, caller definition, resolved target definition, dependency
  root name, target metadata, import path, resolved path, artifact/review
  metadata, status, and proof evidence use.
- DB proof-store tests admit the new fact kind and reject missing
  `resolved_def_id` or missing `import_path`, preserving fail-closed proof
  validation.
- The axum real-corpus dependency-root test still proves
  `axum/src/extract/state.rs:309` `InnerState::from_ref(state)` and
  `axum/src/middleware/from_extractor.rs:328` `Secret::from_ref(state)` each
  traverse to `axum_core::extract::FromRef::from_ref` through one resolved
  associated-function edge, then admits proof-only dependency-root records for
  those exact callsite ids.
- RAG exact proof context for `FromRef::from_ref` exposes the two
  `dependency_root` records alongside the four normal incoming caller-site
  proof rows.
- Exact `code_item_lookup` and `code_item_edges` target-centered FromRef tool
  tests expose the same two dependency-root proof rows without changing the
  four incoming caller rows or flattening nested local-item owners into parent
  functions.
- Verification passed:
  `cargo test -p ploke-db --test proof_graph_store dependency_root -- --nocapture`,
  `cargo test -p ploke-db axum_real_target_from_ref_dependency_root_bound_reaches_workspace_trait_method -- --nocapture`,
  `cargo test -p ploke-rag proof_context_exact_preserves_axum_supported_target_rows -- --nocapture`,
  `cargo test -p ploke-tui --test integration code_item_lookup_returns_trait_bound_remaining_real_corpus_callers -- --nocapture`,
  `cargo test -p ploke-tui --test integration code_item_edges_returns_trait_bound_remaining_real_corpus_callers -- --nocapture`,
  `cargo check -p ploke-rag --tests`,
  `cargo fmt --all --check`, and `git diff --check`.

Previously completed slice in current bucket: generated test-entrypoint summary
proof over a real axum private zero-source-caller target.

Completed evidence:

- Regenerated active fixture snapshots with the `call_graph` feature and
  verified the active backup DB registry; no tracked fixture bytes changed.
- Added a strict `entrypoint_summary` proof fact kind for generated harness or
  build entrypoint reachability summaries. The fact requires target identity,
  summary artifact metadata, review/scope/invalidation fields, status, and
  proof evidence use.
- DB proof-store tests admit the new fact kind, reject missing
  `definition_id`, and reject non-schema `target_kind` values.
- The axum real-corpus dead-code test still proves
  `error_handling::traits` has zero persisted source callers and zero call
  impact edges, then admits a proof-only generated test-harness summary for
  that definition.
- RAG exact proof context and `code_item_lookup` expose the same
  `entrypoint_summary` row while preserving empty source caller/path/impact
  sets. No source call edge is fabricated for the generated test harness.
- Verification passed:
  `cargo run -p xtask --features call_graph -- fixtures regenerate --active`,
  `cargo run -p xtask --features call_graph -- verify-backup-dbs`,
  `cargo test -p ploke-db --test proof_graph_store entrypoint_summary -- --nocapture`,
  `cargo test -p ploke-db axum_usage_questions_list_private_nodes_without_incoming_callers -- --nocapture`,
  `cargo test -p ploke-rag call_impact_exact_reports_private_target_without_incoming_callers -- --nocapture`,
  `cargo test -p ploke-tui --test integration code_item_lookup_reports_private_target_without_incoming_callers -- --nocapture`,
  and
  `cargo test -p ploke-tui --test integration code_private_uncalled_lists_real_corpus_private_zero_caller_target -- --nocapture`.

Previously completed slice in current bucket: source/sink reachable-effect path
carrier over a real axum task-spawn frontier.

Completed evidence:

- Extended `call_effects_reachable_from_owner(...)` so each reachable
  `effect_seed` now carries `paths_to_owner`: resolved call paths from the
  selected owner to the owner that contains the annotated effect callsite.
- The carrier remains resolved-edge-only. For the axum task-spawn oracle, the
  path stops at `spawn_service`; the `tokio::spawn(...)` call itself remains an
  external targetless frontier with no fabricated local edge.
- DB, RAG, `code_item_lookup`, and `code_item_edges` tests now prove the
  source-oracle chain
  `deserialize_error_status_codes -> TestClient::new -> spawn_service ->
  tokio::spawn` answers both "is the task-spawn sink reachable?" and "which
  resolved call path reaches the sink owner?"
- This is a query-carrier slice only. It does not add new effect classes,
  security policy evaluation, cost modeling, or broader source/sink inference.
- Verification passed:
  `cargo test -p ploke-db axum_usage_questions_report_reachable_effect_seed_for_task_spawn -- --nocapture`,
  `cargo test -p ploke-rag call_effects_exact_reads_axum_task_spawn_seed -- --nocapture`,
  `cargo test -p ploke-tui --test integration code_item_lookup_returns_real_corpus_reachable_effects -- --nocapture`,
  and
  `cargo test -p ploke-tui --test integration code_item_edges_returns_real_corpus_reachable_effects -- --nocapture`.

Previously completed slice in current bucket: runtime trait-object dispatch
blocker proof over a real axum dyn Future poll frontier.

Completed evidence:

- Regenerated active fixture snapshots and verified the active backup DB
  registry; no tracked fixture bytes changed.
- Added a DB real-target usage-question test for
  `axum/src/error_handling/mod.rs:251`
  `HandleErrorFuture::poll -> self.project().future.poll(cx)`, whose receiver
  field type is the `Pin<Box<dyn Future<...>>>` stored at
  `axum/src/error_handling/mod.rs:240`.
- The test proves the dyn `Future::poll` row remains `Unsupported`,
  targetless, and absent from traversal edges, while `call_reach_for_owner`
  exposes it as an unsupported frontier from the owner.
- The same test attaches an explicit `dynamic_dispatch_unbounded`
  `proof_blocker` to the real callsite id and proves both
  `proof_blockers()` and `proof_graphrag_context(...)` can retrieve the blocker
  without fabricating a local callee edge.
- RAG proof-context coverage now attaches the same explicit
  `dynamic_dispatch_unbounded` blocker to the dyn Future poll callsite and
  proves `collect_proof_context(...)` preserves both the projected
  `type_resolution_missing` row and the explicit runtime-dispatch blocker.
- Exact `code_item_lookup` and `code_item_edges` unsupported-receiver matrix
  tests now assert the same blocker appears in tool proof-context payloads for
  the axum dyn Future poll case.
- No new `effect_class` enum was added for poll/resume here. GitNexus reported
  `validate_enum_fields` as CRITICAL blast radius, so async poll/resume effect
  taxonomy remains a separate schema decision instead of being widened inside a
  test slice.
- Verification passed:
  `cargo xtask fixtures regenerate --active`,
  `cargo xtask verify-backup-dbs`, and
  `cargo test -p ploke-db axum_usage_questions_report_dyn_future_poll_runtime_dispatch_blocker -- --nocapture`,
  `cargo test -p ploke-rag proof_context_collection_preserves_axum_future_poll_trait_object_blocker -- --nocapture`,
  `cargo test -p ploke-tui --test integration code_item_lookup_returns_unsupported_receiver_targetless_real_corpus_rows -- --nocapture`,
  and
  `cargo test -p ploke-tui --test integration code_item_edges_returns_unsupported_receiver_targetless_real_corpus_rows -- --nocapture`.

Completed slice in current bucket: fixture-backed FFI boundary effect
annotation over an external targetless frontier.

Completed evidence:

- Added a shared proof fixture for the dedicated
  `effect:fixture-extern-c-abs` seed on the
  `call_extern_c_function -> abs(value)` callsite in
  `tests/fixture_crates/fixture_call_graph/src/lib.rs`.
- DB reach now proves the `ffi_boundary` effect is reachable from
  `call_extern_c_function`, preserves the linked `abs(value)` external
  targetless callsite, keeps `paths_to_owner` empty for the direct frontier,
  and carries the existing `external_dependency_summary_missing` blocker
  reason without fabricating a local edge.
- RAG exact effects and exact `code_item_lookup` / `code_item_edges` payloads
  preserve the same effect seed, targetless callsite, and blocker reason.
- This is proof annotation only. It does not classify FFI automatically and
  does not traverse the foreign function as a local callee.

Completed slice in current bucket: real-corpus async-block poll/resume blocker
proof over axum `Handler::call`.

Completed evidence:

- The existing axum source oracle remains
  `axum/src/handler/mod.rs:217`
  `Box::pin(async move { self().await.into_response() })`.
- DB still proves the nested async-block executable owner, keeps `self()` and
  `into_response()` unsupported/targetless, and asserts both callsites have no
  traversal candidates.
- Added explicit proof-only `dynamic_dispatch_unbounded` blockers for both
  callsites to explain the missing callable binding plus async poll/resume
  proof input without changing the projected `type_resolution_missing`
  call-resolution rows.
- RAG proof context and exact `code_item_lookup` / `code_item_edges` payloads
  now preserve both facts for each site: the fail-closed call-resolution row
  and the explicit async poll/resume blocker.
- This is blocker visibility only. It does not infer runtime polling, does not
  traverse `self()` to a concrete handler, and does not treat awaited
  `into_response()` as a resolved receiver chain.

Previously completed slice in current bucket: reachable source/sink effect
annotation query over a real axum task-spawn frontier.

Completed evidence:

- Added `CallReachEffect` plus `Database::call_effects_reachable_from_owner`,
  which combines resolved owner reachability, targetless frontier context rows,
  and proof `effect_seed` facts to answer "which annotated effects are
  reachable from this owner?"
- Added a DB real-target usage-question test for the source chain
  `axum/src/form.rs:262` `TestClient::new(app)` ->
  `axum/src/test_helpers/test_client.rs:36` `spawn_service(svc)` ->
  `axum/src/test_helpers/test_client.rs:23` `tokio::spawn(...)`.
- The test annotates the real `tokio::spawn` callsite with
  `effect_class = async_task_spawn`, proves the new query returns that effect
  from the upstream test owner, and reasserts the sink remains an external
  targetless frontier with zero local call edges.
- Added `CallReachEffectInfo` and
  `RagService::exact_call_effects_reachable_from_owner`, preserving the same
  targetless callsite payload and proof blockers through the existing RAG exact
  wrapper pattern.
- Added a RAG real-target test for the same axum source chain, proving the
  exact RAG API reports the `async_task_spawn` effect from the upstream owner
  without creating a RAG target row for the external `tokio::spawn` frontier.
- Added `call_reach_effects` to exact `code_item_lookup` and
  `code_item_edges` payloads, plus tool-description text and real-target TUI
  tests proving both tools expose the same task-spawn effect seed with the
  original external targetless callsite payload.
- This is a query/proof slice only. It does not add source/sink inference, does
  not classify effects automatically, and does not traverse external frontier
  rows as local callees.
- Verification passed:
  `cargo test -p ploke-db axum_usage_questions_report_reachable_effect_seed_for_task_spawn -- --nocapture`,
  `cargo test -p ploke-rag call_effects_exact_reads_axum_task_spawn_seed -- --nocapture`,
  `cargo test -p ploke-tui --test integration code_item_lookup_returns_real_corpus_reachable_effects -- --nocapture`,
  `cargo test -p ploke-tui --test integration code_item_edges_returns_real_corpus_reachable_effects -- --nocapture`,
  `cargo fmt --all --check`, and `git diff --check`.

Previously completed slice in current bucket: generated-item macro boundary
summary proof over a real unresolved constructor frontier.

Completed evidence:

- Added a DB real-target proof test for the axum generated constructor gap:
  `axum/src/handler/future.rs:11-18` invokes `opaque_future!`,
  `axum/src/macros.rs:19-20` is the `new` template, and
  `axum/src/handler/service.rs:174` calls
  `super::future::IntoServiceFuture::new(future)`.
- The test proves the call graph keeps the generated constructor callsite
  unresolved and targetless, with zero local call edges, before and after
  admitting a summary for the macro-expansion boundary.
- The test adds an `expansion_boundary` proof row for the real macro invocation,
  observes the initial `macro_expansion_not_available` blocker, admits a linked
  summary artifact, and proves only the boundary blocker is discharged. The
  callsite-level `type_resolution_missing` blocker remains until generated
  inherent items are actually modeled.
- Extracted shared real-target axum proof-domain/admitted-summary helpers for
  the external-frontier and generated-boundary proof tests instead of copying
  build-domain, cfg-domain, rustc-invocation, and summary JSON records.
- This is a proof-layer slice only. It does not expand macros, does not create
  generated method nodes, and does not fabricate a traversal edge.
- Follow-up propagation now links the admitted `opaque_future!` boundary summary
  to the unresolved `IntoServiceFuture::new` callsite. RAG exact proof context
  and exact `code_item_lookup` / `code_item_edges` payloads expose the
  callsite-linked `expansion_boundary` plus admitted summary artifact while the
  generated constructor row remains `Unresolved`, targetless, and blocked on
  `type_resolution_missing`.
- Verification passed:
  `cargo test -p ploke-db axum_generated_constructor_macro_boundary_accepts_summary_proof -- --nocapture`,
  `cargo test -p ploke-db axum_external_frontier_accepts_admitted_summary_proof -- --nocapture`,
  `cargo test -p ploke-rag proof_context_collection_preserves_axum_generated_constructor_frontier -- --nocapture`,
  `cargo test -p ploke-tui --test integration code_item_lookup_returns_generated_constructor_frontier_path_rows -- --nocapture`,
  `cargo test -p ploke-tui --test integration code_item_edges_returns_generated_constructor_frontier_path_rows -- --nocapture`,
  `cargo fmt --all --check`, and `git diff --check`.

Previously completed slice in current bucket: process-effect proof-policy
consistency for security/performance usage questions.

Completed evidence:

- `effect_seed` storage already accepted all stable
  `operating_system_process_*` classes, but the proof invariant classifier only
  treated `create` and `replace` as detached-process effects.
- The invariant classifier now treats `create`, `replace`, `configure`,
  `wait`, `kill`, and `reap` as OS process effects for detached-process
  handoff obligations.
- The proof-invariant test is table-driven over the same OS process effect
  subset and proves each class fails closed without admitted successor handoff
  evidence.
- This does not change call graph traversal, resolver semantics, or any
  non-process effect class such as `async_task_spawn`, `ffi_boundary`, or
  summary-boundary effects.
- GitNexus impact for `is_process_effect_class` reported LOW risk, exact
  target, zero indexed upstream dependents, and zero affected processes.
- Verification passed:
  `cargo test -p ploke-db --test proof_invariant_checker proof_invariant_checker_treats_os_process_effects_as_detached_process_obligations -- --nocapture`,
  `cargo test -p ploke-db --test proof_graph_store proof_graph_store_accepts_all_stable_effect_class_values -- --nocapture`,
  `cargo fmt --all --check`, and `git diff --check`.

Latest completed slice in current bucket: owner-scoped external summary need
queue exposed through DB, RAG, and exact TUI tools.

Completed evidence:

- Added an owner-scoped `external_summary_needs` query surface that reports
  active `external_dependency_summary_missing` blockers for reachable
  targetless frontier callsites, including the original callsite row,
  blocker reasons, and resolved paths to intermediate owners when present.
- The query remains proof-authoritative: an admitted linked
  `external_summary` discharges the need, but no local call edge or target is
  fabricated for external calls.
- DB and RAG tests prove the real
  `axum/src/middleware/from_fn.rs:411` `Request::builder()` frontier appears
  in the queue before summary admission and disappears after the admitted
  summary records are inserted.
- RAG exact coverage now table-drives the same queue/discharge contract for
  `axum/src/response/sse.rs:449` `std::mem::replace(...)`, preserving the
  targetless frontier row after summary admission.
- Exact `code_item_lookup` and `code_item_edges` payloads now include
  `external_summary_needs`, and their UI payloads report the same count.
  Positive TUI checks cover unresolved real external frontiers; admitted
  `Request::builder` and `std::mem::replace` checks prove discharged sites do
  not remain in the queue.
- The proof-blocker lookup used by owner-scoped call-graph usage queries now
  derives blockers only for the already-selected reachable callsite ids while
  preserving all proof rows as discharge context, avoiding a global derived
  blocker expansion in large fixture-backed tool tests.

Previous completed slice in current bucket: proof-authoritative external
summary admission over the real `Request::builder` alias frontier, propagated
through DB, RAG, and TUI tool payloads.

Completed evidence:

- Added shared proof-fixture records for the real
  `axum/src/middleware/from_fn.rs:411` `Request::builder()` callsite, where
  the `Request = http::Request` alias leaves the selected local workspace.
- The DB real-target receiver-chain matrix now proves that the row starts as a
  targetless `External` frontier with an `external_dependency_summary_missing`
  blocker, accepts a linked admitted `external_summary`, discharges the
  blocker through `proof_blockers()` and `proof_graphrag_context(...)`, and
  still has zero local call edges after proof admission.
- RAG exact proof context for `from_fn::tests::basic` now preserves both the
  initial blocker and the admitted-summary state for the same real callsite.
- Exact TUI `code_item_lookup` and `code_item_edges` external path-frontier
  tests now table-drive both admitted external summaries:
  `Request::builder` and `std::mem::replace`.
- This is still a proof-layer slice. It does not add a local traversal target
  for `http::Request::builder`, and it does not weaken targetless external
  frontier semantics.
- Verification passed:
  `cargo test -p ploke-db axum_real_target_result_receiver_chains_are_documented_gaps -- --nocapture`,
  `cargo test -p ploke-rag proof_context_collection_preserves_axum_request_builder_alias_frontier -- --nocapture`,
  `cargo test -p ploke-tui --test integration code_item_lookup_returns_request_builder_alias_external_path_rows -- --nocapture`,
  `cargo test -p ploke-tui --test integration code_item_edges_returns_request_builder_alias_external_path_rows -- --nocapture`,
  `cargo run -p xtask --features call_graph -- fixtures regenerate --active`,
  `cargo run -p xtask --features call_graph -- verify-backup-dbs`,
  `cargo fmt --all --check`, and `git diff --check`.

Previous completed slice in current bucket: proof-authoritative external summary
admission over a real external frontier, propagated through DB, RAG, and TUI
tool payloads.

Completed evidence:

- Added a DB real-target proof test for
  `axum/src/response/sse.rs:449`, where `write_buf` calls
  `std::mem::replace(&mut self.data_written, true)`.
- The test proves the call graph keeps the `std::mem::replace` row as an
  external targetless frontier with zero local call edges before and after proof
  admission.
- The test projects proof rows for the real owner, observes the initial
  `external_dependency_summary_missing` blocker, admits a build-domain-scoped
  `external_summary` fact plus a linked `externally_summarized`
  `call_resolution` fact, and proves the blocker is discharged through
  `proof_blockers()` and `proof_graphrag_context(...)`.
- `proof_symbol_lookup(...)` now follows a selected callsite's
  `external_summary_id` to the linked `external_summary` artifact, matching the
  existing `proof_graphrag_context(...)` summary-link behavior.
- RAG exact proof context for `EventDataWriter::write_buf` now preserves the
  admitted `std::mem::replace` summary artifact and no longer reports the
  missing-summary blocker for that callsite after admission.
- Exact TUI `code_item_lookup` and `code_item_edges` now table-drive
  `Request::builder` as the still-blocked external frontier and
  `std::mem::replace` as the admitted external-summary frontier, preserving the
  linked `call_resolution` and `external_summary` proof rows without creating a
  local edge.
- This is a proof-layer and query-linking slice only. It does not add
  parser/resolver breadth, does not fabricate a local edge for external
  dependencies, and does not yet provide production external-summary authoring
  or policy review UX.
- Active fixtures were regenerated and `verify-backup-dbs` passed with no
  tracked fixture drift.
- Verification passed:
  `cargo xtask fixtures regenerate --active`, `cargo xtask verify-backup-dbs`,
  `cargo test -p ploke-db axum_external_frontier_accepts_admitted_summary_proof -- --nocapture`,
  `cargo test -p ploke-db proof_symbol_lookup_links_call_site_context_to_external_summary_artifact -- --nocapture`,
  `cargo test -p ploke-rag proof_context_collection_preserves_axum_std_mem_replace_admitted_summary -- --nocapture`,
  `cargo test -p ploke-tui --test integration code_item_lookup_returns_request_builder_alias_external_path_rows -- --nocapture`,
  `cargo test -p ploke-tui --test integration code_item_edges_returns_request_builder_alias_external_path_rows -- --nocapture`,
  `cargo fmt --all --check`, and `git diff --check`.

Completed slice in current bucket: complete private multi-caller generic
`FnOnce` parameter proof and downstream propagation.

Completed evidence:

- Added fixture rows for
  `call_multi_generic_fn_once_param<F: FnOnce() -> i32>(generic_f) { generic_f() }`
  with two local callers that both pass `local_target`, plus
  `call_multi_conflicting_generic_fn_once_param<F: FnOnce() -> i32>(generic_f) { generic_f() }`
  with one `local_target` caller and one `other_target` caller.
- Parser, DB, RAG, and TUI tests now prove the same complete-private-caller
  contract for generic callable parameters that was previously covered for
  bare function-pointer parameters: multiple local callers are accepted only
  when every inspected argument proves the same exact target.
- The same-target generic helper resolves `generic_f()` to `local_target`;
  the conflicting helper remains `Unsupported`, targetless, and carries a
  `type_resolution_missing` proof blocker.
- The wrapper functions remain ordinary path calls to their private helpers;
  argument proof is resolver evidence for the helper body and does not replace
  the caller-to-helper edges.

Previously completed slice in current bucket: complete private multi-caller
callable parameter proof and downstream propagation.

Completed evidence:

- Added fixture rows for
  `call_multi_function_pointer_param(f: fn() -> i32) { f() }` with two local
  callers that both pass `local_target`, plus
  `call_multi_conflicting_function_pointer_param(f: fn() -> i32) { f() }`
  with one `local_target` caller and one `other_target` caller.
- Parser and DB tests now prove the existing complete-private-caller resolver
  contract: multiple local callers are accepted only when every inspected
  argument proves the same target. The same-target helper resolves `f()` to
  `local_target`; the conflicting helper remains `Unsupported`, targetless,
  and carries a `type_resolution_missing` proof blocker.
- RAG call-context collection preserves the same resolved same-target helper row
  and the fail-closed conflicting helper blocker row without fabricating a target.
- Exact TUI `code_item_lookup` and `code_item_edges` tests preserve the
  conflicting helper's targetless blocker proof alongside the existing public
  opaque function-pointer parameter blocker and public generic `FnOnce`
  parameter blocker.
- Exact TUI `code_item_lookup` and `code_item_edges` tests now also preserve
  the positive same-target helper row: `f()` resolves to `local_target` as a
  `Function` edge with call-site, call-edge, and resolved call-resolution proof
  rows.
- The wrapper functions remain ordinary path calls to their private helpers;
  argument proof is additional resolver evidence and does not replace those
  caller edges.
- Active fixtures were regenerated and `verify-backup-dbs` passed with no
  tracked fixture drift.
- Verification passed:
  `cargo test -p syn_parser --features call_graph call_multi_function_pointer_param -- --nocapture`,
  `cargo test -p syn_parser --features call_graph call_multi_conflicting_function_pointer_param -- --nocapture`,
  `cargo test -p ploke-db function_pointer_parameter -- --nocapture`,
  `cargo test -p ploke-db callable_path -- --nocapture`,
  `cargo test -p ploke-db path_resolution -- --nocapture`,
  `cargo test -p ploke-rag call_context_collection_reads_real_fixture_callable_path_rows -- --nocapture`,
  `cargo test -p ploke-tui --test integration code_item_lookup_returns_function_pointer_param_blocker -- --nocapture`,
  `cargo test -p ploke-tui --test integration code_item_edges_returns_function_pointer_param_blocker -- --nocapture`,
  `cargo xtask fixtures regenerate --active`, `cargo xtask verify-backup-dbs`,
  `cargo fmt --all --check`, and `git diff --check`.
- This remains bounded to private helpers with complete source-visible caller
  sets. Public callable parameters, divergent private caller arguments,
  interprocedural callable value flow beyond direct call arguments, and general
  callable trait-object dispatch remain fail-closed.

Previously completed bucket: mixed path/closure dynamic branch candidate proof.

Completed evidence:

- Existing fixture rows
  `call_if_closure_branch(flag) { (if flag { local_target } else { || 8 })() }`
  and
  `call_match_closure_arm(flag) { (match flag { true => local_target, false => || 13 })() }`
  now record structured mixed branch targets instead of falling back to an
  unsupported dynamic call.
- Parser/resolver proof preserves both candidates with an `Ambiguous` status:
  one `DynamicFunction` candidate edge to `local_target` and one
  `DynamicClosure` candidate edge to the inline closure executable owner.
- Transform keeps the mixed branch call-site path targetless, matching the
  existing branch-expression projection shape, while persisting the candidate
  `call_relation` rows.
- DB owner and target-centered context tests prove both candidates are
  queryable. Proof projection emits `call_site` plus `call_resolution` with
  `candidate_def_ids` and still emits no `call_edge` for the ambiguous site.
- Active fixtures were regenerated and `verify-backup-dbs` passed with no
  tracked fixture drift.
- Verification passed:
  `cargo test -p syn_parser --features call_graph preserves_mixed_dynamic_candidates -- --nocapture`,
  `cargo test -p ploke-db mixed_branch_dynamic -- --nocapture`,
  `cargo test -p ploke-db dynamic_context -- --nocapture`,
  `cargo test -p ploke-db dynamic_proof -- --nocapture`,
  `cargo test -p syn_parser --features call_graph call_sites -- --nocapture`,
  `cargo test -p ploke-transform --features call_graph dynamic -- --nocapture`,
  `cargo xtask verify-backup-dbs`, `cargo fmt --all --check`, and
  `git diff --check`.
- This remains bounded to locally visible item-path and inline closure branch
  targets. Public callable parameters, callable field values, boxed trait
  objects without initializer proof, arbitrary expression-produced callees, and
  general callable trait-object dispatch remain fail-closed.

Previously completed bucket: usage-question query-surface gap audit after shared
matrix propagation.

Completed evidence:

- Added
  [`2026-07-07_call-graph-usage-question-gap-audit.md`](2026-07-07_call-graph-usage-question-gap-audit.md),
  mapping impact, dead-code, navigation, security, performance, refactoring,
  test-planning, architecture, debugging, API-understanding, RAG, and
  build/deployment questions to existing DB/RAG/TUI coverage.
- The audit found the current query-helper surface is strong enough for the
  next implementation step; remaining gaps are mostly semantic proof inputs
  such as binding/value-flow, external summaries, source/sink annotations, and
  generated harness/build-entrypoint summaries.
- Selected next implementation bucket:
  binding/type-aware proof for unsupported receiver and dynamic callable rows.

Previously completed bucket: downstream propagation for the shared real-target
call-shape matrix.

Completed evidence:

- Added a RAG adapter test under real-corpus call-context coverage that consumes
  every shared matrix row marked `RagApi`, resolves owners and targets against
  the registered corpus backup fixtures, preserves selected callsite shape and
  argument counts, asserts resolved target rows and targetless blocker rows, and
  checks target-centered exact RAG context for resolved rows.
- Verification passed:
  `cargo test -p ploke-rag shared_call_shape_matrix_rows_reach_rag_call_context -- --nocapture`
  (`1 passed; 0 failed`) and `cargo fmt --all --check`.
- The current shared matrix rows remain unmarked for `TuiTool`, so this bucket
  does not require a tool-specific adapter yet.

Previously completed bucket: shared real-target DB call-shape matrix and
call-graph corpus unsafe-block schema refresh.

Completed evidence:

- Added `ploke_test_utils::call_shape_matrix` as the call-graph counterpart to
  the existing type-shape matrix. The first shared rows cover an Axum explicit
  free-function path, a Chrono alias enum-variant constructor, an Axum
  generated constructor frontier, and an Axum dynamic callable field.
- Added a DB adapter test under `ploke-db` real-target matrix coverage that
  table-drives owner selection, callsite selection, target resolution,
  targetless status assertions, raw edge counts, and one-hop/no-hop traversal
  checks from the shared matrix.
- Recreated Axum, Chrono, Generic Array, and Memchr call-graph corpus fixtures
  from source so registered 2026-07-07 seeds include the strict
  `call_site.unsafe_block` column rather than stale 12-column roundtripped
  snapshots.
- Verification passed:
  `cargo test -p ploke-db real_target_matrix -- --nocapture`
  (`76 passed; 0 failed`),
  `cargo check -p ploke-test-utils`, and focused fixture verification for the
  four regenerated call-graph corpus fixtures.

Previously completed bucket: private parenthesized generic `FnOnce`
single-caller proof.

Completed evidence:

- `call_single_parenthesized_generic_fn_once_param<F>(generic_f: F) where F:
  FnOnce() -> i32 { (generic_f)() }` records a dynamic callable-parameter row
  and resolves it exactly to `local_target` only because the helper is private
  and its complete local caller set supplies that function item.
- Parser and DB tests now batch the regular path form `generic_f()` and the
  parenthesized dynamic form `(generic_f)()` under the same complete
  single-caller proof boundary.
- DB owner context, target-centered caller queries, and one-hop traversal prove
  the dynamic edge while preserving the wrapper helper's normal direct call
  row.
- RAG call-context collection and exact `request_code_context` TUI/tool tests
  preserve the same resolved path and dynamic generic callable-parameter rows.
- Public callable-trait parameters, missing/unproven arguments, multi-target
  caller sets, and arbitrary callable-trait dispatch remain targetless rather
  than guessed.

Previously completed bucket: same-block awaited async-closure future alias
proof.

Completed evidence:

- Parser extraction now recognizes the bounded same-block alias proof shape
  `let future = closure(); let alias = future; alias.await;` and the adjacent
  single-expression block alias shape `let alias = { future }; alias.await;`,
  and marks the original async-closure binding call as awaited.
- `call_awaited_async_closure_future_alias_with_body_call()` records the
  original `closure()` path call as an awaited async-closure binding call once
  the same block later awaits the one-step alias.
- `call_awaited_async_closure_future_block_alias_with_body_call()` records the
  same awaited edge when the alias initializer is a single-expression block
  containing the previously recorded future binding.
- DB owner context and traversal prove the supported two-hop path
  outer function -> async-closure owner -> `local_target`.
- RAG call-context collection and exact `request_code_context` TUI/tool tests
  preserve the direct-alias expansion; RAG call-context collection also
  preserves the block-alias expansion into the async-closure body.
- This remains bounded to a direct same-block future binding plus one direct
  alias or one single-expression block alias before `.await`; nested control
  flow, arbitrary future value flow, returned futures, async callable trait
  objects, and general poll/resume semantics remain future work.

Previously completed bucket: downstream edge-tool safety boundary proof.

Completed evidence:

- Active fixtures were regenerated with `--features call_graph` and
  round-tripped successfully; no fixture file diff was produced.
- `code_item_edges` now preserves target-centered unsafe item metadata for
  `unsafe_target`, including the safe direct caller
  `call_unsafe_function`, matching the existing DB/RAG/lookup behavior.
- `code_item_edges` now preserves the owner-centered extern-C
  `abs(value)` reach summary as a targetless `External` frontier row,
  with no fabricated local paths/callees and with fixture source-file
  metadata.
- Focused `code_item_edges` integration tests passed for both safety-boundary
  rows. This is a downstream proof slice only; it does not add parser or
  resolver breadth.

Previously completed bucket: awaited async closure future-binding proof.

Completed evidence:

- Parser extraction now recognizes the bounded same-block proof shape
  `let future = closure(); future.await;` and marks the original async-closure
  binding call as awaited.
- `call_async_closure_future_binding_without_await_with_body_call()` preserves
  `_future = closure()` as an unsupported targetless path call, while the
  separately owned async-closure body still owns and resolves its
  `local_target()` path row.
- `call_awaited_async_closure_future_binding_with_body_call()` records the
  earlier `closure()` path call as an awaited async-closure binding call once
  the same block later executes `future.await`.
- DB owner context and traversal prove the supported two-hop path
  outer function -> async-closure owner -> `local_target`, while the
  non-awaited stored future exposes no path from the outer function to
  `local_target`.
- RAG call-context collection and exact `request_code_context` TUI/tool tests
  preserve both downstream surfaces: unsupported targetless non-awaited stored
  future rows and resolved awaited future expansion into the async-closure
  body.
- Active fixtures were regenerated with `--features call_graph` and
  round-tripped successfully. This remains bounded to a direct same-block
  future binding and direct `future.await`; nested control flow, arbitrary
  future value flow, returned futures, async callable trait objects, and
  general poll/resume semantics remain future work.

Previously completed bucket: immediate awaited async closure binding proof.

Completed evidence:

- Parser local binding proof distinguishes async closure bindings from ordinary
  closure bindings and records `closure()` differently depending on whether
  the returned future is immediately awaited.
- `call_async_closure_binding_without_await_with_body_call()` preserves the
  local async-closure binding as an unsupported targetless path call, while the
  separately owned async-closure body still owns and resolves its
  `local_target()` path row.
- `call_awaited_async_closure_binding_with_body_call()` records
  `closure().await` as an awaited async-closure binding path call and resolves
  it to a `Closure` edge from the outer function to the async-closure
  executable owner.
- DB owner context and traversal prove the supported two-hop path
  outer function -> async-closure owner -> `local_target`, while the
  non-awaited binding exposes no path from the outer function to
  `local_target`.
- RAG call-context collection and exact `request_code_context` TUI/tool tests
  preserve both downstream surfaces: unsupported targetless non-awaited
  binding rows and resolved awaited binding expansion into the async-closure
  body.
- This remains bounded to immediate `.await` on a locally bound async closure;
  broader async callable values and poll/resume semantics remain future work.

Previously completed slice: fixture-backed non-awaited async closure
poll/resume blocker proof.

Completed evidence:

- The source oracles are
  `tests/fixture_crates/fixture_call_graph/src/lib.rs:1701-1714`, where
  `closure()` is called without awaiting the returned async-closure future,
  both directly and through `_future = closure()`.
- The persisted call graph keeps each outer `closure()` row `Unsupported` and
  targetless, while the nested async-closure executable owner still owns and
  resolves the body `local_target()` call.
- DB proof projection preserves the fail-closed `type_resolution_missing`
  call-resolution row for each targetless outer callsite, then explicit
  `dynamic_dispatch_unbounded` proof blockers explain that traversal is
  blocked on async poll/resume proof rather than a missing parser row.
- RAG proof context and exact `code_item_lookup` / `code_item_edges` payloads
  expose both facts for each site without fabricating a traversal edge.
- Verification passed:
  `cargo test -p ploke-db fixture_projection_attaches_non_awaited_async_closure_poll_resume_blockers_without_edges -- --nocapture`,
  `cargo test -p ploke-rag proof_context_collection_preserves_non_awaited_async_closure_poll_resume_blockers -- --nocapture`,
  `cargo test -p ploke-tui --test integration code_item_lookup_returns_non_awaited_async_closure_poll_resume_blockers -- --nocapture`, and
  `cargo test -p ploke-tui --test integration code_item_edges_returns_non_awaited_async_closure_poll_resume_blockers -- --nocapture`.

Previously completed bucket: dereferenced boxed dyn Fn exact initializer proof.

Completed evidence:

- Parser dynamic callee classification now preserves exact callable trait-object
  initializer proof through `(*boxed_fn)()` when the local binding is
  `let boxed_fn: Box<dyn Fn() -> i32> = Box::new(local_target)`.
- The dynamic resolver reuses the existing initialized-local-binding path to
  resolve the dereferenced call exactly to `local_target`; the `Box::new(...)`
  setup call remains a targetless external frontier row.
- DB owner-scoped context, dynamic proof batches, and target-centered caller
  queries include the dereferenced boxed dynamic caller alongside the existing
  direct and parenthesized boxed cases.
- RAG incoming expansion and dynamic call-context collection preserve the same
  owner, the external setup row, and the resolved dynamic edge to
  `local_target`.
- Exact TUI lookup, edges, and request-code-context matrices preserve the
  resolved dynamic edge and proof payload for the dereferenced boxed callable
  owner.
- Active fixtures were regenerated with `--features call_graph` and
  round-tripped successfully. This remains bounded to exact callable
  trait-object initializers; unproven trait objects and arbitrary callable
  value flow remain fail-closed.

Previously completed bucket: borrowed concrete trait-object receiver proof.

Completed evidence:

- Parser receiver classification now preserves local trait-object proof through
  an explicit borrow, so
  `let value: &dyn LocalDispatchTrait = &TraitDispatchTarget; (&value).trait_value()`
  records `BorrowedInitializedLocalBinding { init_path: TraitDispatchTarget }`
  instead of falling back to an untyped borrowed local receiver.
- The existing method resolver reuses the borrowed initialized local receiver
  path to resolve the call exactly to
  `impl LocalDispatchTrait for TraitDispatchTarget::trait_value`.
- DB owner-scoped context, owner proof rows, and target-centered proof rows now
  include the borrowed trait-object caller, raising the exact target-centered
  local trait-dispatch caller count from three to four in the fixture.
- RAG call-context/proof-context expansion and the `request_code_context` TUI
  tool preserve the borrowed receiver payload and proof rows for the same
  target-driven query.
- Active fixtures were regenerated with `--features call_graph` and
  round-tripped successfully. This remains a narrow local binding proof; it
  does not add broad trait-object dispatch or arbitrary borrow/value-flow.

Previously completed bucket: unsafe callable item metadata for usage summaries.

Completed evidence:

- Parser stores source signature unsafety on `FunctionNode::is_unsafe` and
  `MethodNode::is_unsafe` for free functions, inherent impl methods, and trait
  method declarations.
- Transform projects non-null `is_unsafe` values into the `function` and
  `method` relations, and active backup fixtures were regenerated with
  `--features call_graph` and round-tripped successfully.
- DB fixture coverage proves `call_unsafe_function() -> unsafe_target()` marks
  the unsafe target function while keeping the safe wrapper unmarked, and
  `call_impact_for_target` exposes the same target/caller metadata.
- RAG exact call-impact summaries and the `code_item_lookup` / `code_item_edges`
  TUI tools preserve the same unsafe target and safe direct caller metadata.
- Parser, transform, DB, RAG proof context, and `request_code_context` proof
  payloads now preserve `unsafe_block` occurrence metadata for call sites
  lexically inside `unsafe { ... }`, including
  `call_unsafe_function() -> unsafe_target()` and the targetless extern-C
  frontier `call_extern_c_function() -> abs(value)`.
- This covers unsafe function/method item qualifiers and unsafe-block
  occurrence metadata only. Unsafe trait obligations, unsafe impl metadata,
  inherited trait-method unsafety, and semantic FFI boundary summaries remain
  separate future buckets.

Previously completed bucket: downstream targetless propagation for a real-corpus
local shadowing boundary.

Completed evidence:

- DB already pins `axum/src/routing/tests/mod.rs:{412,413}` as the only two
  projected `get(...)` path rows for `what_matches_wildcard`, while
  `mod.rs:418` shadows imported routing `get` with a local closure and
  `mod.rs:423-434` calls that closure inside `assert_eq!` macro arguments.
- RAG call-context tests now assert exact owner collection preserves exactly
  those two targetless unsupported path rows, with no fabricated edge to the
  imported routing helper.
- Exact `code_item_lookup` and `code_item_edges` now assert the same two-row
  targetless boundary and blocked `type_resolution_missing` proof rows.
- This closes downstream visibility for the stored real-corpus shadowing
  boundary only. Macro-argument body extraction and closure call traversal
  remain future parser/body-owner work.

Previously completed bucket: downstream targetless propagation for real-corpus
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
- Exact `code_item_edges` owner lookup preserves the same targetless
  `External` frontier row, source-file metadata, and zero local traversal
  paths/callees.
- This chunk did not add unsafe blocks or unsafe item metadata. Standalone
  `FunctionNode` / `MethodNode` unsafe qualifiers are covered by the later
  unsafe callable item metadata bucket.

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
  return types. External function calls and arbitrary expression-produced tuple
  destructuring remain unsupported until broader binding/type-flow proof exists.

Previously completed bucket: untyped tuple-pattern receiver from local method
tuple return.

Completed evidence:

- `call_method_tuple_return_pattern_local_instance_method()` now proves
  `let (next, _) = value.tuple_pair(); next.instance_value()` by using the
  exact initializer method callsite for `tuple_pair`, resolving that method
  target, and extracting tuple element `0` from the method return type.
- Parser extraction records the receiver as
  `TupleMethodReturn { name: "next", method_name: "tuple_pair", method_span: (40981, 40999), index: 0 }`,
  while still preserving the initializer `value.tuple_pair()` method call as a
  separate resolved method edge.
- Transform/DB projection, raw and structured DB receiver decode, resolved
  proof rows, RAG call-context collection, and TUI call-context formatting
  preserve the tuple-method-return receiver payload.
- This remains bounded to local parsed initializer method calls with exact
  source spans and source-visible tuple return types. External method-result
  tuple destructuring such as `Request::new(()).into_parts()`, nested
  value-flow, and arbitrary expression-produced tuple destructuring remain
  unsupported until broader binding/type-flow or external-summary proof exists.

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

Current bucket: match-arm initialized receiver binding proof.

Current evidence:

- `call_match_arm_initialized_receiver_method(flag)` proves
  `match LocalAssoc { value if flag && value.instance_value() > 0 => value.instance_value(), _ => 0 }`
  as two exact local method edges to `LocalAssoc::instance_value`.
- Parser extraction now visits match guards and bodies with a temporary
  arm-local scope derived from the arm pattern and the scrutinee expression,
  reusing the existing `InitializedLocalBinding { name: "value", init_path:
  ["LocalAssoc"] }` receiver carrier rather than adding a new receiver kind.
- DB owner context and proof projection assert both the guard callsite and the
  body callsite, including their distinct source spans and resolved method
  edges.
- RAG call-context collection and `request_code_context` method/proof payload
  tests preserve the two resolved rows without requiring the target-centered
  expansion carrier to collapse them into a single callsite.
- A later bounded sibling slice adds named-field struct-pattern proof for
  `ParamFieldMethodReceiver { value }` when the scrutinee is the exact
  matching struct expression `ParamFieldMethodReceiver { value: LocalAssoc }`.
  That slice reuses the same `InitializedLocalBinding` receiver carrier and
  table-driven local receiver proof assertions.
- This remains bounded to simple identifier arm patterns and the exact
  same-struct named-field pattern/expression form. Tuple/enum-pattern
  destructuring, borrowed pattern bindings, match ergonomics, rest patterns,
  and real-corpus iterator guard rows remain future proof shapes unless
  separately modeled.

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
- DB and RAG owner-scoped external-summary queue tests now prove the same
  feature-gated serde_json frontier is listed as
  `external_dependency_summary_missing` until an admitted external summary is
  linked. Exact `code_item_lookup` and `code_item_edges` payloads expose that
  same `external_summary_needs` row and UI count without inventing a local
  edge.

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

Completed bucket, 2026-07-07: chrono Option ok_or try receiver proof.

- Real-corpus chrono callsites
  `DateTime::from_timestamp_secs(ts).ok_or(OUT_OF_RANGE)?.naive_utc()` and
  `DateTime::from_timestamp(timestamp, nanosecond).ok_or(OUT_OF_RANGE)?.naive_utc()`
  now resolve the outer try-receiver `naive_utc` call to
  `DateTime<Tz>::naive_utc`.
- The resolver uses the inner associated-function path call proof for
  `DateTime::from_timestamp*`, unwraps the local `Option<Self>` return type,
  and carries the associated `DateTime` type target into the outer method
  lookup.
- The chrono call-graph backup fixture was regenerated as
  `corpus_chrono_call_graph_2026-07-07.sqlite`, and the real-corpus DB oracle
  now asserts two incoming callers and one method traversal edge per callsite.
- Exact RAG call context and exact TUI `code_item_lookup` / `code_item_edges`
  tests now preserve the same two incoming method caller-site identities and
  target-scoped proof rows.

## Coverage Matrix

| Bucket | Current status | DB proof | RAG proof | TUI/tool proof | Real-corpus target | Next action |
| --- | --- | --- | --- | --- | --- | --- |
| Method / trait-method multi-hop | Met for now | `RequestExt::extract -> extract_with_state -> FromRequest::from_request` two-hop paths and summaries | Exact call paths, expansion, impact, reach | `code_item_lookup` two-hop payload and UI counts | axum | Do not deepen by default; switch buckets unless a regression appears. |
| Regular free-function one-hop | Covered | `parse_attrs`, `run_ui_tests`, and related target fanout assertions | Exact call-context tests for current real-corpus function callers | `code_item_lookup` function caller regressions | axum | Use as source pool for finding a free-function multi-hop chain. |
| Regular free-function multi-hop | Met for now | `from_request::expand -> impl_struct_by_extracting_each_field -> extract_fields` ordered two-hop traversal through `call_paths_between`, owner, and target path APIs | Exact call paths expose the same ordered function chain and source-node metadata | `code_item_call_path` returns the same real-corpus function reachability path | axum | Switch buckets; do not add more free-function breadth by default. |
| Inherent method one-hop | Covered/partial | Examples include `Json::from_bytes` and related method/associated-function rows | Exact call-context propagation exists | Exact lookup regressions exist | axum | Revisit only after broader buckets have at least one proof. |
| Exact local external-trait impl receiver methods | Met for now | 13 `Router::clone` typed-local/self-field receiver rows resolve to `impl<S> Clone for Router<S>::clone`; axum-core `parts.extract_with_state(state)` resolves through imported `http::request::Parts` receiver proof to `RequestPartsExt for Parts::extract_with_state` | Exact call context preserves the same caller-site identities and receiver buckets | Remaining real-corpus TUI matrix includes `RouterClone` and the focused `RequestPartsExt` local receiver checks | axum | Switch buckets; do not broaden to arbitrary external trait dispatch by default. |
| Associated-function path calls | Met for now | `Self::from_bytes`, `E::from_request`, `MethodRouter::new` style rows; `RequestExt::extract -> extract_with_state -> FromRequest::from_request` proves a two-hop path whose terminal edge is `AssociatedFunction` | Exact paths, expansion, impact, and reach preserve associated-function relation/kind | `code_item_call_path`, `code_item_lookup`, and `get_code_edges` are the strict tool traversal proofs; broad real-corpus `request_code_context` path assertions are quarantined as ranking-sensitive and too expensive for default runs | axum | Switch buckets; do not add a separate associated-function multi-hop row unless this exact contract regresses. |
| Constructors | Stronger one-hop | Tuple struct / enum variant constructor rows are asserted in real-corpus matrix, including chrono alias constructor rows; real-corpus `BoxedIntoRoute` `Self(...)` tuple constructors and fixture-backed method-owned `Self(...)` tuple constructors now resolve through the enclosing impl self type | Exact context propagation exists for direct, alias, and `Self(...)` constructor rows | Tool regressions include direct, alias, variant, and `Self(...)` constructor rows; lookup and edges both preserve the chrono `MappedLocalTime::Single` alias constructor caller identities | axum, chrono plus local fixture fallback | Switch buckets; add constructor breadth only when a new source shape is represented in the matrix. |
| External dependency frontier | Covered as frontier | External rows are exposed but not traversed; regenerated axum DB fixture classifies `Body::size_hint` `self.0.size_hint()` as an external targetless frontier through tuple-field receiver and local type-alias evidence for `Body(BoxBody)`, classifies fourteen `Request::builder` rows as external frontiers when `Request` resolves through the axum-core `Request = http::Request` alias, classifies selected `self.inner.poll_ready(cx)` / `self.0.poll_ready(cx)` forwarding rows as external frontiers when the receiver field type is either a concrete external service type or a generic parameter bounded by an external `Service` trait, classifies the two axum `Route::oneshot` receiver rows as external frontiers when `tower::ServiceExt` import evidence is source-visible, classifies chrono `StrftimeItems::queue.is_empty()` as an external targetless slice receiver frontier through the source-visible `&'static [Item<'static>]` field type, pins current `std::mem::replace` std-root path rows as targetless external frontiers, and fixture-backed DB reach now preserves the `unsafe extern "C"` `abs(value)` call in `external_frontier_calls` plus a proof `ffi_boundary` effect seed without fabricating a local edge | Reach summaries expose external frontier calls; RAG call/proof context preserves the `Body::size_hint`, `Request::builder`, Route `oneshot`, and `std::mem::replace` external frontier rows without local targets, exact RAG call context preserves the chrono `queue.is_empty` slice frontier without local targets, and exact RAG reach/effects preserve fixture-backed `abs(value)` as an external targetless FFI frontier with its external-summary blocker reason | Tool summaries count/display frontier rows; lookup/edges preserve the `Body::size_hint`, `Request::builder`, `std::mem::replace`, and Route `oneshot` external frontier call/proof rows; `request_code_context` and exact `code_item_edges` preserve the fixture-backed `abs(value)` targetless call/proof or reach payload without fabricating local paths/callees, and lookup/edges now expose the fixture-backed FFI effect seed in `call_reach_effects` | axum, chrono plus local fixture fallback | Keep fail-closed; do not convert to traversal without external-summary semantics. |
| Unsafe callable item and block metadata | Met for item qualifiers and call-site occurrence flag | Parser and transform store `is_unsafe` for function/method items and `unsafe_block` for call-site occurrences; DB fixture coverage proves `call_unsafe_function -> unsafe_target` marks the unsafe target, keeps the safe wrapper unmarked, and marks the source callsite as inside an unsafe block; fixture-backed extern-C `abs(value)` remains targetless external and carries the same unsafe-block callsite flag | RAG exact impact preserves the unsafe target/safe caller metadata, and projected proof context preserves `unsafe_block` on resolved safe-wrapper and ordinary false rows | `code_item_lookup` and exact `code_item_edges` expose the same `call_impact.target.is_unsafe` and safe direct-caller flag; `request_code_context` proof payload preserves `unsafe_block = true` for the targetless extern-C frontier | local fixture fallback | Switch buckets; unsafe trait obligations, unsafe impl metadata, inherited trait-method unsafety, and semantic FFI boundary summaries remain separate future modeling work. |
| Unsupported receiver / trait-object shapes | Stronger blocker visibility plus bounded receiver positives | Targetless/unsupported rows remain for generic field/result/await/local receiver gaps; exact local `Router::clone` and axum-core `parts.extract_with_state(state)` receiver rows traverse; direct and single-reference external parameter receivers such as `call_external_param_vec_len(value: Vec<i32>) { value.len() }`, `call_external_borrowed_param_vec_len(value: &Vec<i32>) { value.len() }`, and axum `req: &mut Request<_>` / `mut req: Request<_>` plus request-routing local-binding `req.extensions_mut()` rows classify as external targetless frontiers; workspace re-exported external receiver aliases are now covered by a transform-level workspace proof where `provider::Request` aliases `http::Request` and `consumer` calls `req.extensions_mut()`; borrowed initialized local receivers such as `let value = LocalAssoc; (&value).instance_value()`, borrowed concrete trait-object receivers such as `let value: &dyn LocalDispatchTrait = &TraitDispatchTarget; (&value).trait_value()`, borrowed value-parameter receivers such as `call_borrowed_value_param_instance_method(value: LocalAssoc) { (&value).instance_value() }`, and borrowed value-parameter method-result chains such as `call_borrowed_value_param_method_result_instance_method(value: LocalAssoc) { (&value).clone_assoc().instance_value() }` now carry exact local receiver proof and resolve in fixture-backed DB/proof rows; same-target if-expression receivers such as `(if flag { LocalAssoc } else { LocalAssoc }).instance_value()` now carry branch-path proof and resolve in fixture-backed DB/proof rows; direct same-arity tuple-pattern local receivers such as `let (value, _) = (LocalAssoc, 0); value.instance_value()` now carry initializer proof and resolve in parser/DB/proof rows; explicitly typed tuple-pattern local receivers such as `let (value, _): (LocalAssoc, i32) = make_local_assoc_pair(); value.instance_value()` now carry per-element type proof and preserve the initializer helper call as a separate resolved path edge; self-field method-result chains such as `self.value.clone_assoc().instance_value()` reuse self-field proof for the inner call and return-type proof for the outer call in fixture-backed parser/DB/proof rows; chrono `DateTime::from_timestamp*(...).ok_or(...)?.naive_utc()` now resolves through inner associated-function proof and `Option<Self>` return evidence; tuple and named self-field receivers can classify external frontiers such as axum `Body(BoxBody)::size_hint`, Route `self.0.oneshot(req)`, and chrono `StrftimeItems::queue.is_empty()` when the source-visible field type proves the external/slice receiver shape; `Service`-bounded self-field `poll_ready` forwarding rows now classify as external targetless frontiers; initialized `Request::new` local receiver rows classify as external frontiers; qualified trait-object qself calls such as `<dyn std::any::Any>::downcast_mut::<T>(...)` project as external targetless path rows in fixture-backed tests and regenerated axum real-corpus rows; axum `request_parts.rs:164` now preserves `TupleMethodReturn(parts, into_parts, index 0)` receiver proof and resolves to `RequestPartsExt for Parts::extract_with_state` through the tuple-return summary; axum `HandleErrorFuture::poll` dyn Future dispatch is pinned as an unsupported, targetless method row; remaining receiver rows without exact type or alias proof, arbitrary external method-result tuple destructuring, and unknown receiver expressions persist as `Unresolved` or `Unsupported` receiver rows instead of being dropped | RAG preserves blocker/frontier rows, the exact positive subset, fixture-backed unsupported receiver rows, borrowed-initialized, borrowed concrete trait-object, borrowed value-parameter, borrowed value-parameter method-result, if-branch, tuple-pattern local binding, self-field method-result receiver payloads, chrono `Option<Self>::ok_or(...)?` try-receiver method payloads, qualified dyn Any external path rows, axum `Body::size_hint`, request-parts turbofish `generic_arg_count = 2` plus resolved tuple-method-return receiver payload, dyn Future `poll`, Route `oneshot`, plus chrono `queue.is_empty` external frontier payloads | Tool tests surface unsupported rows/counts, `RouterClone` positives, focused `RequestPartsExt` local receiver checks, `&value = LocalAssoc` receiver formatting, borrowed concrete trait-object receiver call context/proof payloads, if/match branch receiver formatting, tuple-pattern receiver request-code-context caller/proof expansion, self-field method-result call context/proof payloads, chrono `DateTime::from_timestamp*(...).ok_or(...)?.naive_utc()` incoming lookup/edge payloads, qualified dyn Any external proof rows, and axum `Body::size_hint`, request-parts turbofish tuple-method-return resolved receiver rows, dyn Future `poll` unsupported receiver rows, plus Route `oneshot` external frontier rows | axum, chrono plus local fixture fallback | Switch buckets; future implementation should add exact receiver proof by shape, not weaken unsupported rows. |
| Dynamic callable values | Stronger fixture-backed subset plus real-corpus targetless fallback; private single-caller parameter proof met for now | Fixture-backed direct, alias, same-target branch/match including guarded same-target match arms, single-expression block initialized callable values, exact `Box<dyn Fn()> = Box::new(local_target)` path/dynamic calls, named local closure binding `(closure)()` calls, local closure-binding `as fn` casts, exact local closure-binding deref calls such as `(*closure)()`, pathless non-async closure literal calls, immediately awaited async closure literal calls such as `(async || local_target())().await`, direct returned-path calls such as `make_fn()()`, direct returned closure-literal calls such as `make_closure()()`, returned local closure binding calls such as `make_bound_closure()()`, returned local aliases of closure bindings such as `make_alias_bound_closure()()`, private bare function-pointer single-caller parameter proof for `call_single_function_pointer_param(f: fn() -> i32) { f() }`, `call_single_parenthesized_function_pointer_param(f: fn() -> i32) { (f)() }`, `call_single_if_function_pointer_param_branch(flag, f) { (if flag { f } else { f })() }`, `call_single_match_function_pointer_param_arm(flag, f) { (match flag { true => f, false => f })() }`, and `call_single_function_pointer_param_cast(f: fn() -> i32) { (f as fn() -> i32)() }`, private generic callable-bound single-caller proof for `call_single_generic_fn_once_param<F>(generic_f: F) where F: FnOnce() -> i32 { generic_f() }` and `call_single_parenthesized_generic_fn_once_param<F>(generic_f: F) where F: FnOnce() -> i32 { (generic_f)() }`, and private constructed-holder single-caller field proof for `call_single_named_field_function_param(holder: CallbackHolder) { (holder.callback)() }`, `call_single_indexed_field_function_param(holder: CallbackArrayHolder) { holder.callbacks[0]() }`, and `call_single_indexed_tuple_field_function_param(holder: TupleCallbackArrayHolder) { holder.0[0]() }` when every local caller supplies `local_target` in the callable field slot, plus candidate-only ambiguous proof for `call_multi_conflicting_named_field_function_param(holder: CallbackHolder) { (holder.callback)() }` when complete local callers supply `local_target` and `other_target`; public/unproven function-pointer parameter calls such as `call_function_pointer_param(f: fn() -> i32) { f() }`, `call_parenthesized_function_pointer_param(f: fn() -> i32) { (f)() }`, `call_if_function_pointer_param_branch(flag, f)`, `call_match_function_pointer_param_arm(flag, f)`, and `call_function_pointer_param_cast(f: fn() -> i32) { (f as fn() -> i32)() }`, public holder-parameter calls such as `call_indexed_field_function_param(holder) { holder.callbacks[0]() }`, public/unproven generic callable-trait parameter calls, and real-corpus captured callback parameter calls such as axum `f(attr, input)` remain visible targetless path/dynamic rows with no fabricated edge; non-awaited async closure invocations remain targetless unsupported dynamic rows while their bodies are owned separately; mixed-target branches remain targetless; memchr function-pointer field rows remain unsupported and targetless by owner/source-line fanout; memchr callable trait-object field calls `fwd(...)` and `rev(...)` remain unsupported targetless path rows with dynamic-row absence pinned | RAG preserves resolved direct/branch/block/guarded-match initialized rows, exact boxed dyn Fn initializer rows, direct returned-path dynamic rows, direct returned-closure rows including local closure binding and local alias returns, named and literal dynamic-closure rows, dereferenced local closure-binding dynamic rows, blocker rows including function-pointer parameter path/dynamic blockers and axum closure-owned captured callback parameter blockers, non-awaited async-closure unsupported dynamic rows, the two memchr function-pointer field blockers with argument counts 4 and 2, the two memchr callable trait-object path blockers with argument count 2, the resolved private single-caller function-pointer parameter edges to `local_target` for `f()`, `(f)()`, `(if flag { f } else { f })()`, `(match flag { true => f, false => f })()`, and `(f as fn() -> i32)()`, the resolved private single-caller generic `FnOnce` path `generic_f()` and dynamic `(generic_f)()` edges, and the resolved private holder-parameter edges for `(holder.callback)()`, `holder.callbacks[0]()` / `holder.0[0]()` plus their incoming wrapper helper rows, and the ambiguous named-field holder candidates for conflicting complete local caller sets | `code_item_lookup` covers resolved dynamic-function callable rows, including block-initialized and guarded same-target forms; DB/RAG owner/path queries now cover resolved `DynamicClosure` rows including closure literals, immediately awaited async closure literals, closure-binding `as fn` casts, exact local closure-binding derefs, direct returned closure literals, direct returned local closure bindings, returned local aliases of closure bindings, and real-corpus closure-owned captured callback parameter blockers; DB proof batches, RAG collection, and `code_item_lookup`/`code_item_edges` now cover the private named/indexed holder-parameter `DynamicFunction` edges and the named-field conflicting candidate-only ambiguous row; lookup and edges preserve fixture-backed function-pointer parameter `type_resolution_missing` blocker proof rows, targetless axum callable-field and memchr function-pointer field blockers, and memchr callable trait-object path blockers with `type_resolution_missing` proof rows; `request_code_context` preserves the resolved private single-caller function-pointer parameter edges to `local_target` for path, parenthesized dynamic, branch/match dynamic, cast dynamic, and bounded generic `FnOnce` path/dynamic call forms | local fixture; axum and memchr real-corpus fallback | Switch buckets; broader callable trait objects without exact initializer proof, arbitrary interprocedural callable-parameter value flow beyond complete private single-caller cases, returned closure values beyond direct closure literals, direct local closure bindings, or direct local aliases of closure bindings, and non-immediate poll/resume edges for async closure futures still need binding/body/effect work. |
| Proc-macro entrypoint body owners | Met for now | `CallBodyOwnerId::Macro` owners project through axum `expand_with` and `expand_attr_with` real-corpus helper calls; regenerated axum closure callback rows traverse to `debug_handler::expand`; regenerated axum IIFE rows resolve to closure-owner dynamic targets while callable-parameter invocations remain fail-closed | Exact impact summaries expose public proc-macro callers and direct helper callsites | `code_item_lookup` and `code_item_edges` surface four public macro callers for `expand_with` | axum | Switch buckets; callback argument value-flow remains fail-closed until interprocedural callable proof exists. |
| Closures / executable-local body ownership | Named closure-binding, non-async closure-literal, immediately awaited async-closure literal, real-corpus closure body rows, fixture-backed async-closure body ownership, fixture-backed async-block traversal, function-local const/static/local-`fn`/local impl method ownership, local `fn` item call targets including call-before-declaration, and exact executable-owner tool lookup met for now | Ordinary, `move`, and async closure body calls plus async block body calls and function-local const/static initializer, local `fn`, and local impl method body calls are parser-owned by `CallBodyOwnerId::Executable`; `call_body_owner` projects closure, async-closure label, async block metadata, and local-item `local_const` / `local_static` / `local_fn:<name>` / `local_impl_method:<name>` metadata, validates them as call owners, keeps nested body calls off enclosing function owners, exposes `callers_for_target(...)` and one-hop paths from executable owners, resolves named closure binding calls, block-local `fn` item calls as `LocalFunction -> LocalItem` even when the call precedes the local item declaration, non-async closure literal dynamic calls, and immediate `.await` async-closure literal calls to closure owners, proves outer function -> local `fn` item -> `assoc_const_value` and outer function -> closure body -> `local_target` paths, preserves non-awaited async closure invocation as targetless until broader poll/resume modeling exists, resolves local impl where-bound associated path calls and async-block-owned `Self::...` paths through parent owner bounds, and snippet-materializes fixture-backed executable owners for RAG expansion; real-corpus `Handler::call` async-block `self()` and awaited `into_response()` rows now also carry explicit proof-only `dynamic_dispatch_unbounded` blockers without adding traversal candidates | RAG collection, incoming expansion, and projected proof-context tests preserve fixture-backed closure, async-closure, async block, and local-item owners as callers for nested body rows, including function-local const/static initializer, local `fn`, local impl method `assoc_const_value()` rows, the outer `inner()` caller of `local_fn:inner`, the axum `local_impl_method:from_request_parts` resolved `Secret::from_ref` row, and the axum-core async-block-owned resolved `Self::from_request_parts` row; RAG also preserves real-corpus `Handler::call` async-block owner targetless `self()` / `into_response()` rows, their projected `type_resolution_missing` rows, and explicit async poll/resume blockers, plus outer closure binding/literal calls to closure owners | `request_code_context` tests materialize closure, async-closure, async block, and local-item owners with call context and projected proof rows, including move closure literal, local `fn` body traversal, the outer `inner()` `LocalFunction` edge to `local_fn:inner`, and local impl method body traversal; exact `code_item_lookup`/`code_item_edges` now accept `node_kind=local_item` for executable owners, prove the real-corpus axum `local_impl_method:deserialize` owner exposes external/unsupported call rows without local traversal paths, and parent-qualify the repeated axum `local_impl_method:from_request_parts` label with `parent_name=test_from_extractor` to expose the resolved one-hop `Secret::from_ref` associated-function edge; exact `code_item_lookup`/`code_item_edges` also address the real-corpus axum `Handler::call` nested `node_kind=async_block` owner with `parent_name=call` and preserve its `self()` plus awaited `into_response()` rows as unsupported, targetless blocker-proof rows with explicit async poll/resume blockers; exact `code_item_call_path` accepts the same local-item endpoint and does not fabricate a zero-length self path; exact tests still assert enclosing outer functions do not flatten nested local-item rows unless the outer function is a proven caller of the local item; dedicated real-corpus lookup/edges tests include the closure-owned `expand_field` caller row | axum `HeaderValue::from_static` rows in route local const and JSON nested local `fn` bodies are projected as external targetless `LocalItem` owners; axum `Secret::from_ref` in a function-local impl method is projected as a resolved `LocalItem` owner through executable where-bound resolution and exact-addressable through `parent_name`; axum `local_impl_method:deserialize` in `axum/src/extract/path/mod.rs` is exact-addressable as a local-item tool target with external/unsupported rows; axum-core `Self::from_request_parts` in the ViaParts blanket async block is projected as a resolved `AsyncBlock` owner through parent impl bounds; async-block rows remain pinned targetless for selected `post`, `Handler::call` `self()` / `into_response()`, and awaited `unwrap` cases; fixture-backed ordinary closure, non-async closure literal, immediately awaited async-closure literal, async closure body, async block, local const/static initializer, local `fn`, local `fn` call target, and local impl method body cases are positive | Switch buckets unless the next planned slice explicitly targets broader async poll/resume effect edges, macro-expanded/generated source bodies, or broader local-item semantics beyond scoped local `fn` call targets. |
| Import / re-export / glob completeness | Partial, workspace dependency glob subset improved | Explicit and some imported path calls work; chrono alias constructor path calls resolve through typed alias evidence; fixture-backed `use super::*` inherited parent glob imports now resolve associated-function calls; regenerated axum backup data resolves 168 projected `TestClient::new` rows, including nested/direct re-export import rows, routing child-module inherited glob rows, and the axum-core `request_parts.rs:193` workspace dependency glob row; regenerated axum backup data resolves 310 projected `Router::new` rows including the axum-core `request_parts.rs:193` workspace import row, with an admitted proof-only `dependency_root` row for that exact `Router::new` site; regenerated axum backup data also resolves twenty-three `Body::empty` caller edges through axum-core direct rows, direct parsed-workspace imports, local re-export imports, inherited glob imports, closure-owned rows, and local-item rows, with an admitted proof-only `dependency_root` row for the direct `axum/src/form.rs:158` workspace import; axum `Request::new` and same-crate/imported/inherited-glob `Request::builder` rows through `Request = http::Request` are classified external and targetless; generated `IntoServiceFuture::new` remains a visible unresolved frontier until macro-expanded inherent items are modeled; broader import completeness remains partial | RAG exact context/proof preserves alias rows, the regenerated 168 `TestClient::new` resolved caller rows including the axum-core workspace-glob dependency-root proof row, the 310 `Router::new` resolved caller rows including the axum-core workspace-import dependency-root proof row, the twenty-three `Body::empty` caller rows including the direct `form.rs` dependency-root proof row, and the targetless generated `IntoServiceFuture::new` frontier | Tool coverage includes chrono alias rows, dedicated high-fanout lookup/edges tests for `AxumRemainingTarget::TestClientNew` including the workspace-glob dependency-root proof row, exact remaining-real-corpus lookup/edges tests for `AxumRemainingTarget::RouterNew` including the axum-core workspace-import dependency-root proof row, exact lookup/edges tests for `Body::empty` incoming callers plus the direct `form.rs` dependency-root proof row, exact lookup/edges targetless path rows for `Request::builder`, and exact lookup/edges targetless path rows for generated `IntoServiceFuture::new` via the trait-impl owner qualifier `owner_trait=Service<Request>` plus `owner_type=HandlerService` | axum, chrono plus local fixture fallback | Keep strict source oracles for the remaining source-text/projection gaps; broaden import resolution by proof shape, not by weakening targetless rows. |
| Dead-code / private zero-incoming | Met for now | `private_uncalled_nodes` lists axum `error_handling::traits`, excludes called `parse_attrs`, exact impact reports no incoming source calls, and `call_build_domains_for_node` reports the linked admitted axum library build domain when the generated test-harness entrypoint summary is present | `exact_private_uncalled_nodes` preserves the same DB dead-code list through RAG, exact impact reports the same private target with empty caller/path/bucket sets, and exact build-domain summaries preserve the linked axum domain | `code_private_uncalled` lists the same private zero-caller axum target directly and returns its admitted generated test-harness `entrypoint_summary` plus linked build-domain row; `code_item_lookup` and `code_item_edges` report zero incoming paths, empty source-caller impact counts, the admitted generated test-harness `entrypoint_summary` proof row, and the same build-domain summary | axum | Switch buckets; do not widen to generated/dynamic reachability here. |
| DB usage summaries | Strong current surface | `call_impact_for_target`, `call_reach_for_owner`, paths, owner-scoped module-boundary edges, owner-recursion cycle paths, source files/modules/crates/cfgs, buckets, boundary/frontier rows; real-corpus usage-question tests now prove impact, navigation, dead-code, public/test caller bucketing, architecture boundary edges, external/unsupported/unresolved frontier rows, proc-macro entrypoint impact, Body::empty component/source-module/source-crate impact, axum listener `#[cfg(unix)]` reach source-cfg preservation, axum `Json::from_bytes` `#[cfg(feature = "json")]` reach/callsite cfg preservation through parent module declarations, and `std::any::type_name::<K>()` API argument-shape preservation over the regenerated axum fixture; fixture-backed usage-question coverage now proves FFI `abs(value)` remains an external frontier in owner reach summaries; `call_effect_policy_violations_for_owner` evaluates caller-supplied effect allowlists over existing reachable `effect_seed` facts and proves the axum `tokio::spawn` `async_task_spawn` sink is reported as disallowed without fabricating a local edge; strict `effect_policy` proof facts now supply persisted owner allowlists for `call_effect_policy_violations_for_stored_owner_policy`, with the same axum task-spawn sink reported from an admitted owner policy and ambiguous multiple admitted policies rejected; reach summaries split full nonresolved frontier rows into external, unsupported, unresolved, and ambiguous subsets without promoting them into traversal edges | N/A | N/A | axum plus local fixture fallback | Add fields only when they answer a matrix question, not opportunistically. |
| RAG usage summaries | Strong current surface | N/A | Exact call paths, owner-recursion cycle paths, impact, reach, source metadata; real-corpus reach summaries now preserve external, unsupported, unresolved, and ambiguous frontier subsets from DB, including the axum generated `IntoServiceFuture::new` unresolved frontier, the regenerated `Body::empty` component/source-module/source-crate impact, the axum listener `#[cfg(unix)]` source-cfg reach summary, the axum `Json::from_bytes` `feature = "json"` reach summary, the axum `type_name::<K>()` generic-argument call shape, the axum-core `request_parts.rs:164` resolved turbofish tuple-method-return receiver shape, and fixture-backed FFI `abs(value)` external-frontier reach; `exact_call_effect_policy_violations_for_owner` preserves the same caller-supplied effect allowlist result and the path to the reachable sink owner; `exact_call_effect_policy_violations_for_stored_owner_policy` reads the admitted owner policy from proof facts and preserves the same axum task-spawn violation payload | N/A | axum plus local fixture fallback | Add only when DB bucket already has proof. |
| TUI/tool usage summaries | Strong current surface | N/A | N/A | `code_item_lookup`, `code_item_edges`, and exact call-path tool coverage; `code_item_edges` now carries the same existing impact/reach summaries in `node_info` that lookup exposes, with real-corpus assertions for paths, owner-recursion cycle paths, boundary edges, direct callsites, callsite buckets, source files/modules/crates/cfgs, frontier status counts, safety-boundary metadata, proof context, and UI counts; lookup/edges `Body::empty` tool tests assert the regenerated component-impact callsite buckets, path-shape counts, source file/module/crate carriers, and test/non-test impact partition; lookup/edges generated-constructor tests assert unresolved frontier payload/count propagation; lookup/edges generated test-harness tests assert the `entrypoint_summary` proof row without incoming source edges; lookup/edges `Json::from_bytes` tests assert the inherited `feature = "json"` reach source-cfg payload/count; lookup coverage asserts the axum `type_name::<K>()` generic-argument call shape; lookup/edges targetless matrix tests assert the axum-core `request_parts.rs:164` resolved turbofish tuple-method-return receiver row; lookup/edges executable-owner tests assert the real-corpus axum `Handler::call` async-block owner targetless rows and blocker proof payloads; edge-tool safety tests assert fixture-backed unsafe target impact metadata and extern-C external frontier reach without inventing local traversal; exact lookup/edges task-spawn tests pass `allowed_effects = [ffi_boundary]` and assert `call_effect_policy_violations` reports the reachable axum `async_task_spawn` sink without fabricating a target | axum plus local fixture fallback | Keep tool changes thin; do not invent semantics outside RAG/DB. |

Update 2026-07-09: the request-parts notes in the matrix row above are
superseded by the current tuple-return summary slice. The axum-core
`request_parts.rs:164`
`parts.extract_with_state::<State<String>, String>(&state)` row now resolves
to `RequestPartsExt for Parts::extract_with_state`, and DB/RAG/TUI assertions
preserve the tuple-method-return receiver payload, turbofish arity, resolved
local edge, and resolved proof rows.

## Parking Lot

- Add binding tracking plan that ties syntax body ownership, local bindings, and type graph edges before attempting receiver/dynamic traversal.
- Extend the dependency-root proof carrier only with source oracles for new
  workspace import families. The current carrier covers the real axum
  `FromRef::from_ref` dependency-root rows, the axum-core `Router::new`
  workspace-import row, and the axum-core `TestClient::new` workspace-glob
  dependency-root row; broader workspace import families still need exact
  source oracles before they can be promoted beyond existing resolved rows.
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
