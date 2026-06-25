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
- Latest committed call-graph quality checkpoint:
  `4ea3b53c test: split call context fixture queries`
- Current quality focus: DB/proof/query hardening before adding parser breadth.
- Recent endpoint-family cleanup:
  - `e53a2301 Split call target endpoint kind`
  - `29eb4f8a Centralize call endpoint families`
- Recent invariant cleanup:
  - `e14aa283 Guard duplicate call status emissions`
- Recent candidate-provenance cleanup:
  - `67fc3a8a Preserve ambiguous dynamic call candidates`
- Recent DB test-helper cleanup:
  - `7b9d872e test: extract dynamic candidate assertions`
  - `73e9855a test: move call candidate helpers to common`
  - `74ec3d06 test: move proof fixture helpers to common`
  - `4f9c87d5 test: split proof lookup fixtures`
- Recent DB test-module split:
  - `7e51101d test: split dynamic call proof fixtures`
  - `09f8f967 test: split target proof fixtures`
  - `1c56f1fa test: split blocker proof fixtures`
  - `4f9c87d5 test: split proof lookup fixtures`
  - `4ea3b53c test: split call context fixture queries`

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
| CGQ-2 | P1 | Partial 2026-06-25 | `call_graph_fixture_queries.rs` and `call_graph_queries.rs` are far larger and less modular than the type-graph precedent. | First cleanup extracted repeated ambiguous dynamic candidate context/proof assertions into shared helpers, moved them to `call_graph_fixture_common.rs`, moved shared proof and constructor fixture helpers to common, and split dynamic, target-centered, blocker, proof-lookup, constructor proof, and call-context fixture tests into `call_graph_fixture_queries/` submodules. Remaining work is to continue splitting by concern and introduce shared matrices before adding more cases. |
| CGQ-3 | P1 | Partial 2026-06-25 | Call endpoint-family rules are duplicated across transform insertion, DB Cozo queries, Rust validation, and tests. | DB query helpers and Rust validation now share `VALID_CALL_TARGET_FAMILIES`; remaining work is transform insertion/test helper surfaces. |
| CGQ-4 | P1 | Done 2026-06-25 | DB `CallRelationKind` mixed relation semantics with target-kind semantics, including `Struct` and `Variant`. | `CallTargetKind` now carries DB endpoint families separately from call relation kinds, while persisted relation strings remain unchanged. |
| CGQ-5 | P1 | Done 2026-06-25 | External proof projection reported `externally_summarized` while also using blocker reason `external_dependency_summary_missing`. | Current parser-projected external rows now emit blocked/missing-summary proof state until a real external summary fact exists. |
| CGQ-6 | P2 | Done 2026-06-25 | Dynamic branch/match candidate provenance can collapse to `Null` in persisted `call_site` rows. | Ambiguous branch/match dynamic calls now keep proven local function candidates as `DynamicFunction` relations while preserving `Ambiguous` status; proof projection exposes them as `candidate_def_ids` without promoting them to resolved call edges. |
| CGQ-7 | P2 | Done 2026-06-25 | The "exactly one status per call site" invariant could be hidden by post-hoc sort/dedup. | `CallRelationResolver` now rejects duplicate status sources before dedup, including identical duplicates. |
| CGQ-8 | P2 | Open | `call_resolution.rs` and `call_extraction.rs` are monolithic. | Avoid adding breadth there without local extraction; split path/method/dynamic/macro logic when touching the area. |
| CGQ-9 | P2 | Open | Call-graph docs are serving as a long running diary rather than a stable coverage inventory. | Add or update a stable call-graph coverage document/matrix, mirroring type-resolution coverage practice. |
| CGQ-10 | P3 | Open | `call_graph` feature name is broader than the actual gate: parser facts exist baseline, DB projection is gated. | Make docs explicit that this is currently a DB projection rollout gate. |

## Pattern matches to preserve

- Keep `CallId` as a separate identity universe; do not back call-site IDs with
  `NodeId`.
- Keep structural call occurrence extraction separate from semantic resolution
  and proof facts.
- Keep parser endpoint families typed and fail-closed.
- Keep unresolved, external, and unsupported call sites targetless. Ambiguous
  call sites may carry proven local candidates, but must remain ambiguous and
  must not emit resolved proof/navigation edges.
- Keep backup fixture behavior strict; regenerate/review fixtures rather than
  weakening import semantics.

## Immediate next checkpoint

Continue DB-side hardening before parser/resolver breadth. The next likely
slices are:

1. Finish CGQ-3 by reducing transform/test-helper endpoint-family duplication.
2. Continue CGQ-2 by splitting oversized call-graph DB tests by concern.
3. Keep candidate-bearing ambiguous rows covered in DB/proof tests when
   splitting helpers; do not move them back into targetless failure tables.

## Latest verification

For `67fc3a8a Preserve ambiguous dynamic call candidates`:

- `cargo test -p syn_parser --features call_graph call_sites -- --nocapture`
  - passed: `tests/mod.rs` 206 passed, 0 failed.
- `cargo test -p ploke-transform --features call_graph transform::tests -- --nocapture`
  - passed: 5 passed, 0 failed.
- `cargo test -p ploke-db --features call_graph fixture_projection -- --nocapture`
  - passed: `unit::call_graph_fixture_queries` 34 passed, 0 failed.

For `7e51101d test: split dynamic call proof fixtures`:

- `cargo test -p ploke-db --features call_graph dynamic_proof -- --nocapture`
  - passed: `tests/mod.rs` 7 passed, 0 failed.
- `cargo test -p ploke-db --features call_graph fixture_projection -- --nocapture`
  - passed: `tests/mod.rs` 34 passed, 0 failed.

For `09f8f967 test: split target proof fixtures`:

- `cargo test -p ploke-db --features call_graph target_proof -- --nocapture`
  - passed: `tests/mod.rs` 9 passed, 0 failed.
- `cargo test -p ploke-db --features call_graph fixture_projection -- --nocapture`
  - passed: `tests/mod.rs` 34 passed, 0 failed.

For `74ec3d06 test: move proof fixture helpers to common`:

- `cargo test -p ploke-db --features call_graph dynamic_proof -- --nocapture`
  - passed: `tests/mod.rs` 7 passed, 0 failed.
- `cargo test -p ploke-db --features call_graph target_proof -- --nocapture`
  - passed: `tests/mod.rs` 9 passed, 0 failed.
- `cargo test -p ploke-db --features call_graph fixture_projection -- --nocapture`
  - passed: `tests/mod.rs` 34 passed, 0 failed.

For `1c56f1fa test: split blocker proof fixtures`:

- `cargo test -p ploke-db --features call_graph blocker_proof -- --nocapture`
  - passed: `tests/mod.rs` 6 passed, 0 failed.
- `cargo test -p ploke-db --features call_graph fixture_projection -- --nocapture`
  - passed: `tests/mod.rs` 34 passed, 0 failed.

For `4f9c87d5 test: split proof lookup fixtures`:

- `cargo test -p ploke-db --features call_graph proof_lookup -- --nocapture`
  - passed: `tests/mod.rs` 3 passed, 0 failed.
- `cargo test -p ploke-db --features call_graph constructor_proof -- --nocapture`
  - passed: constructor proof filter passed, 0 failed.
- `cargo test -p ploke-db --features call_graph target_proof -- --nocapture`
  - passed: `tests/mod.rs` 9 passed, 0 failed.
- `cargo test -p ploke-db --features call_graph fixture_projection -- --nocapture`
  - passed: `tests/mod.rs` 34 passed, 0 failed.

For `4ea3b53c test: split call context fixture queries`:

- `cargo test -p ploke-db --features call_graph unit::call_graph_fixture_queries::context_expansion -- --nocapture`
  - passed: `tests/mod.rs` 7 passed, 0 failed.
- `cargo test -p ploke-db --features call_graph fixture_projection -- --nocapture`
  - passed: `tests/mod.rs` 34 passed, 0 failed.
