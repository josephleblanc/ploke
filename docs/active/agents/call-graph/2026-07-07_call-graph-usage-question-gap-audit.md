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
entrypoint-summary proof context, and proc-macro entrypoint impact. RAG and
tool tests preserve the same major surfaces through exact APIs and tool
payloads.

The remaining gaps are mostly not missing query helpers. They are missing proof
inputs:

- value-flow and binding proof for unsupported receiver/dynamic callsites;
- external dependency summaries for frontier rows that should become trusted
  external effects;
- source/sink and policy annotations for security/performance/refactoring
  questions that need domain semantics beyond caller/callee reachability;
- generated harness/build-entrypoint summaries for full binary/test/CI
  reachability beyond admitted proof-only entrypoint summaries.

## Usage Question Coverage

| Section | Current evidence | Status |
| --- | --- | --- |
| Impact analysis | `call_impact_for_target`, RAG `exact_call_impact_for_target`, `code_item_lookup`, and `code_item_edges` cover eventual callers, public callers, test/non-test buckets, direct callsites, source files/modules/crates, and callsite buckets. | Strong current surface. |
| Dead code detection | `private_uncalled_nodes`, RAG `exact_private_uncalled_nodes`, and `code_private_uncalled` cover private zero-incoming nodes and cross-check empty impact reports. `code_private_uncalled` now also exposes admitted generated-entrypoint summaries for returned nodes. | Strong for stored source call graph; generated harness reach remains proof-only and does not become a local call edge. |
| Navigation | `callers_for_target`, `call_sites_for_target`, `call_context_for_owner`, `call_paths_*`, RAG exact paths, and `code_item_call_path` cover direct and multi-hop traversal. | Strong current surface. |
| Security analysis | Reach/path/frontier queries can answer "can A reach B?" and expose unsafe/FFI/external frontier rows, including `abs(value)` and unsafe-block metadata. | Partial: source/sink classification and policy-order checks are not modeled. |
| Performance work | Reach/path/frontier queries expose known helpers, external calls, cfgs, and argument shape. | Partial: hot-path/cost/blocking annotations are not modeled. |
| Refactoring support | Impact, direct callsites, source files/modules/crates, boundary edges, and callsite buckets support migration planning. | Strong for caller inventory; move-safety/cycle prediction needs dependency-policy rules. |
| Test planning | Impact test/non-test buckets, source metadata, and admitted generated test-harness entrypoint summaries identify stored test callers and proof-only generated entrypoints. | Partial: generated harness summaries are proof context, and CI test selection is not modeled. |
| Architecture review | `module_boundary_edges_from_owner`, reach `boundary_edges`, lookup/edges payloads, and source modules expose cross-module call edges. | Strong for module-boundary inventory; intended layer policies are not modeled. |
| Debugging | Owner/target context, exact paths, frontier status buckets, proof context, and source spans map persisted edges/blockers back to source callsites. | Strong current surface. |
| API understanding | Impact buckets, argument/generic argument counts, constructor relation kinds, path-shape counts, aliases/re-exports, and source crates show real target usage. | Strong current surface. |
| Documentation and RAG | RAG exact call context, exact paths, impact/reach summaries, proof context, and tool payloads expose caller/callee context and fail-closed blockers. | Strong current surface. |
| Build or deployment optimization | Source crates/modules/cfgs and component impact reports answer affected components and feature/platform-gated paths. | Partial: actual build-target and CI-test selection requires build-domain/test-entrypoint summaries. |

## Concrete Existing Proof Points

- DB: `axum_usage_questions_*` in
  `crates/ploke-db/tests/unit/call_graph_fixture_queries/real_target_matrix/usage_questions.rs`.
- RAG: real-corpus exact path, impact, reach, frontier, and private-uncalled
  tests in
  `crates/ploke-rag/src/core/unit_tests/tests/call_context/collection/cases/fixtures/real_corpus.rs`.
- TUI/tools: `code_item_call_path`, `code_item_lookup`, `code_item_edges`,
  `code_private_uncalled`, and targetless matrix integration tests under
  `crates/ploke-tui/tests/integration/`. The private-uncalled tool now exposes
  admitted generated-entrypoint summaries without fabricating source call
  edges.

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
