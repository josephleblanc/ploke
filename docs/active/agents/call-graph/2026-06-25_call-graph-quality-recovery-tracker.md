# Call graph quality recovery tracker

Date: 2026-06-25
Status: active quality gate for the call-graph goal
Related restart spine: [`README.md`](README.md)

## Purpose

This tracker records the quality issues found in the 2026-06-25 independent
review pass so they survive context compaction and goal resumes. Treat these
items as blockers to continuing broad parser/resolver expansion unless the user
explicitly chooses otherwise.

The implementation must follow nearby type-graph and AST/container patterns.
Large files, copy-pasted query/proof logic, and ad hoc one-off test assertions
are not acceptable as a continuing implementation style.

## Current state

- Branch: `prototype1-parent-mwv-live-r16-dangling-symlink-surface-fix-20260615t003337z-gen0`
- Latest committed call-graph quality checkpoint before endpoint-family work:
  `65ca8758 Fix external call proof state`
- Current quality focus: DB/proof/query hardening before adding parser breadth.

## Resumption rules

- Before adding more call-shape breadth, inventory remaining DB/proof/RAG/TUI
  cases and batch them into table-driven fixture/proof assertions.
- Prefer the type-graph organization patterns:
  - `crates/ploke-db/tests/unit/type_graph_queries/mod.rs`
  - `crates/ploke-db/tests/unit/type_graph_queries/common.rs`
  - `crates/ploke-db/src/type_graph/fixed_rules.rs`
  - `crates/ingest/ploke-transform/src/transform/type_graph.rs`
- Extract shared helpers before adding another cluster of repeated fixture or
  proof assertions.
- Do not keep growing already oversized files when a nearby module/common-helper
  split is available.
- Update docs once per consolidated chunk, not after every micro-slice.
- Keep `CALL_GRAPH_GATE:db-projection` active. Do not relax backup fixture
  imports or schema invariants without explicit user approval.
- Run GitNexus impact before editing indexed symbols and
  `npx gitnexus detect-changes` before committing.
- Use subagents for test execution, per repo instructions.

## Open quality items

| ID | Priority | Status | Issue | Required direction |
| --- | --- | --- | --- | --- |
| CGQ-1 | P1 | Open | Implementation focus drifted toward parser breadth while DB/proof quality lagged. | Shift next resumed work to DB/proof/query hardening and shared test structure. |
| CGQ-2 | P1 | Open | `call_graph_fixture_queries.rs` and `call_graph_queries.rs` are far larger and less modular than the type-graph precedent. | Split by concern and introduce shared `common` helpers/matrices before adding more cases. |
| CGQ-3 | P1 | Open | Call endpoint-family rules are duplicated across transform insertion, DB Cozo queries, Rust validation, and tests. | Centralize the valid call relation/target family matrix; keep one source of truth or one generated matrix. |
| CGQ-4 | P1 | Done 2026-06-25 | DB `CallRelationKind` mixed relation semantics with target-kind semantics, including `Struct` and `Variant`. | `CallTargetKind` now carries DB endpoint families separately from call relation kinds, while persisted relation strings remain unchanged. |
| CGQ-5 | P1 | Done 2026-06-25 | External proof projection reported `externally_summarized` while also using blocker reason `external_dependency_summary_missing`. | Current parser-projected external rows now emit blocked/missing-summary proof state until a real external summary fact exists. |
| CGQ-6 | P2 | Open | Dynamic branch/match candidate provenance can collapse to `Null` in persisted `call_site` rows. | Decide whether proof/RAG needs normalized candidate provenance; do not silently drop proof-critical candidates. |
| CGQ-7 | P2 | Open | The "exactly one status per call site" invariant can be hidden by post-hoc sort/dedup. | Add a pre-dedup invariant check so duplicate identical emissions cannot disappear. |
| CGQ-8 | P2 | Open | `call_resolution.rs` and `call_extraction.rs` are monolithic. | Avoid adding breadth there without local extraction; split path/method/dynamic/macro logic when touching the area. |
| CGQ-9 | P2 | Open | Call-graph docs are serving as a long running diary rather than a stable coverage inventory. | Add or update a stable call-graph coverage document/matrix, mirroring type-resolution coverage practice. |
| CGQ-10 | P3 | Open | `call_graph` feature name is broader than the actual gate: parser facts exist baseline, DB projection is gated. | Make docs explicit that this is currently a DB projection rollout gate. |

## Pattern matches to preserve

- Keep `CallId` as a separate identity universe; do not back call-site IDs with
  `NodeId`.
- Keep structural call occurrence extraction separate from semantic resolution
  and proof facts.
- Keep parser endpoint families typed and fail-closed.
- Keep unresolved, ambiguous, external, and unsupported call sites targetless
  unless a strict local target proof exists.
- Keep backup fixture behavior strict; regenerate/review fixtures rather than
  weakening import semantics.

## Immediate next checkpoint

When the goal is resumed, start by deciding what to do with the current dirty
`call_graph_fixture_queries.rs` helper extraction:

1. Keep and commit it after `npx gitnexus detect-changes`, or
2. Extend it only as part of a consolidated table-driven DB/proof helper chunk,
   or
3. Revert it only if the user explicitly asks.

Do not add new parser/resolver breadth before addressing the DB/proof structure
and semantics above.
