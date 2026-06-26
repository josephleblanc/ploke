# Call graph coverage inventory

Date: 2026-06-25
Status: active restart inventory

This document is the compact coverage checkpoint for the call-graph thread. It
does not replace the detailed case matrix in
[`2026-06-22_call-site-coverage-matrix.md`](2026-06-22_call-site-coverage-matrix.md);
it summarizes what is currently modeled, projected, and surfaced by layer so
future work can choose the next batch without rereading the diary-style notes.

## Status labels

| Status | Meaning |
| --- | --- |
| Green | Implemented with focused tests and used by the current call-graph pipeline. |
| Partial | Implemented for conservative subsets; missing cases must fail closed. |
| Open | Not implemented; do not assume coverage. |
| Gated | Implemented but intentionally behind rollout gate or missing fixture review. |

## Layer inventory

| Layer | Status | Covered now | Main gaps / next proof |
| --- | --- | --- | --- |
| Identity | Green | `ploke_core::CallId` is separate from `NodeId`; path, method, dynamic, and macro call-site IDs are backed by `CallId`. | Keep every new call-site family in the `CallId` universe. |
| Structural parser model | Green | `CodeGraph` stores structural `call_sites` and `call_site_relations`; `BodyContainsCall` links `CallBodyOwnerId` to `AnyCallSiteId`. Semantic call target/status facts are report-owned, not dormant parser graph fields. | Nested closure/async body ownership remains open. |
| Structural extraction | Partial | Path calls, method calls, dynamic/non-path calls, macro calls, const/static initializer calls, and associated const initializer calls are extracted for the covered fixtures. Closure/async inner calls are deliberately not attributed to outer owners. | Desugared/implicit calls, macro expansion, build-script generated code, and nested closure/async owners. |
| Resolver status policy | Green | Each structural call site must receive exactly one status; duplicate status sources are rejected before dedup. Non-resolved external/unsupported/unresolved calls remain targetless; ambiguous dynamic calls may carry proven local candidates without becoming resolved. | Status coverage for future call-site families must be added with fail-closed tests. |
| Path-call resolution | Partial | Local unqualified and explicit `crate`/`self`/`super` calls, module-qualified local calls, selected import/re-export/glob local paths, associated-function paths, aliases, tuple constructors, enum variant constructors, raw identifiers, and direct external roots are covered. | Broader import/re-export/glob paths, external summaries, prelude coverage beyond explicit cases, and proof-grade macro-expanded paths. |
| Method resolution | Partial | Exact inherent `self.method()`, selected non-self local receivers, result/await/try receivers, field/tuple receivers, alias-backed receivers, selected trait impl dispatch, generic/impl-trait/trait-object receiver methods, and trait-associated calls are covered conservatively. | Broader trait dispatch, autoderef/autoref depth, dynamic dispatch, blanket impl breadth, and method lookup parity with rustc remain open. |
| Dynamic calls | Partial | Structural dynamic calls and exact local function-item binding calls are modeled; branch/match candidates preserve ambiguous candidate provenance; unsupported Fn/FnOnce and boxed dyn Fn shapes fail closed. | Full dynamic dispatch, closure value targets, Fn/FnOnce/FnMut semantic resolution, and runtime trait-object call edges. |
| Transform projection | Gated | Behind `call_graph`, transform projects `call_site`, `call_site_edge`, `call_relation`, and `call_resolution_status`. Endpoint kind strings are sourced from typed `CallRelation` helpers. Projection tests now live outside the broad transform module, and dynamic projection cases are split into lookup, case construction, and DB assertion helpers. | Default backup fixtures still need review/regeneration before ungating. |
| DB endpoint invariants | Green | DB query helpers and Rust validation share `VALID_CALL_TARGET_FAMILIES`; fixture invariant tests use exported DB helper predicates instead of a copied matrix. Missing endpoint rows and invalid endpoint-family rows are excluded. | Keep future endpoint families in the shared DB family table and transform typed helpers together. |
| DB availability gate | Green | `has_call_graph_relations()` distinguishes relation registration from populated projection data by requiring a coherent projected call site, body edge, and status. RAG disables call context when projection data is absent. | If projection supports targetless-only or alternate owner shapes beyond current rows, update the sentinel with tests. |
| DB query API | Partial | Owner call context, target callers, expansion, receiver decoding, relation decoding, schema/invariant checks, and fixture-backed contexts have focused coverage. | Finish inventory-driven table matrices before adding new query cases; keep large helper files split by concern. |
| RAG call context | Partial | Call-context collection and expansion are feature-gated and tested for populated DBs, degraded DBs, owner seeds, target seeds, dynamic callers, constructors, method targets, associated functions, trait dispatch, and public context payloads. | Broader live/provider/tool-matrix coverage and final prompt payload policy remain open. |
| TUI call context | Partial | Formatter/overlay/tool-carrier coverage exists for call summaries, target payloads, status/resolution, and selected targetless/proof shapes. Proof context payloads now include resolved target IDs, ambiguous candidate IDs, and external summary IDs in model-facing text/serde carriers. `request_code_context` surfaces proof-context degradation when proof facts are absent. | Final interactive call-context UX and proof-facing presentation are not complete. |
| Proof facts | Partial | Proof graph projection covers resolved call edges, blockers, source provenance, candidate preservation, external summary artifact storage, authority terms, and invariant checks for the current DB surfaces. Proof context lookups match candidate IDs, external summary IDs, authority terms, and external-summary artifact metadata from stored proof-fact JSON without changing the `proof_fact` relation shape; `externally_summarized` call-resolution and expansion-boundary facts now require `external_summary_id`, and `external_summary` artifacts require scoped summary metadata. Incoherent admitted opaque summaries are rejected before storage. `proof_blockers()`, `proof_checker_edges()`, and DB/RAG/TUI proof-context surfaces expose invariant-derived blocker reasons. | External summary blocker discharge semantics, macro/build-domain summaries, and proof-authoritative artifacts remain open. |
| Rollout gate | Gated | `CALL_GRAPH_GATE:db-projection` remains active; the Cargo feature is still named `call_graph`, but the active rollout gate is DB projection plus downstream consumers, not parser-side structural modeling. Default fixture imports are not loosened. | Review/regenerate registered backup fixtures before making DB projection baseline. |

## Next implementation batches

1. Use this inventory plus the detailed call-site matrix to choose the next DB
   or proof case batch; do not add parser breadth without matching DB/RAG/proof
   assertions.
2. Keep the rollout gate active until backup fixture review/regeneration is
   explicitly approved and broad workspace verification is green.
