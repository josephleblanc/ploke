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
  `b0fd0655 test: split dynamic transform projection cases`
- Current quality focus: production-side pattern gaps before adding parser breadth.
- Recent TUI/model-visible cleanup:
  - `cc808347 test: cover variant constructor tool context`
- Recent RAG/DB context cleanup:
  - `31d19d3c Add variant sparse call-context seeds`
  - `15f0ce68 test: share RAG call expansion assertions`
- Recent RAG test-module cleanup:
  - Public sparse `get_context` call-context tests were moved out of the
    monolithic `ploke-rag/src/core/unit_tests.rs` file into
    `ploke-rag/src/core/unit_tests/tests/call_context.rs`.
- Recent coverage inventory cleanup:
  - Added `2026-06-25_call-graph-coverage-inventory.md` as the compact layer inventory.
- Recent production pattern cleanup:
  - `3cce5b86 Split call graph DB module`
  - `e431a2f7 Expose proof context payload fields`
  - `9ba4d247 Split call proof projection module`
  - `30a9cb2e Expose proof context build domains`
  - `3284dd45 Add proof domain context lookup`
  - `5fad4c98 Remove dormant call graph semantic storage`
- Recent proof context test cleanup:
  - `97e64623 test: cover proof domain store lookups`
  - `8a6eeae1 test: table drive proof context lookups`
- Recent transform test cleanup:
  - `b25fa650 test: table drive dynamic transform projection`
  - `8d3dc030 test: split transform call graph projection tests`
  - `c26aeedd test: share dynamic transform lookup helpers`
  - `b0fd0655 test: split dynamic transform projection cases`
- Recent availability cleanup:
  - `2d6320d1 Require populated call graph availability`
- Recent endpoint-family cleanup:
  - `f7349dd7 Share call target family rules with tests`
  - `38640a2c Centralize call relation endpoint kinds`
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
  - `382d83c7 test: split constructor fixture helpers`
  - `53bc6e52 test: split dynamic fixture helpers`
  - `f1e4bb14 test: split proof fixture helpers`
  - `04070efa test: split selector fixture helpers`
  - `d954a39d test: split call graph common helpers`
  - `a9f365c1 test: split call graph source helpers`
  - `1172f0c1 test: split call graph target helpers`
  - `950c1e94 test: split call graph lookup helpers`
  - `445bee67 test: split call graph row helpers`
  - `5469e87a test: table-drive dynamic context helpers`
  - `af7c5d12 test: table-drive dynamic proof edges`
  - `87600137 test: split target proof fixtures`
  - `7c2a308f test: share targetless call row assertions`
  - `693b305b test: reuse targetless row assertions`
  - `695ad2d9 test: split targetless fixture helpers`
  - `1581abbf test: reuse dynamic proof targetless helper`
  - `f9d60dbc test: share targetless macro assertions`
  - `e0d98916 test: reuse unsupported context helpers`
  - `b5a588f2 test: reuse path context targetless helpers`
  - `9c527985 test: split dynamic context fixtures`
  - `2ba35649 test: split method context fixtures`
  - `cfa3392d test: split call graph context expansion queries`
  - `06daefb6 test: split call graph lookup helpers`
  - `72c56f6f test: split call graph proof helpers`
  - `7710313b test: share callable proof blocker helper`
  - `000a3e58 test: share resolved proof case helper`
  - `e23d501a test: table-drive local target prevalidation`
  - `7140e189 test: share targetless owner case helper`
  - `8f371fa5 test: share projected blocker helper`
  - `162322ec test: reuse external blocker proof helper`
  - `0fee7f60 test: share resolved graph seed helper`
  - `b8173ff9 test: reuse resolved graph seed cases`
  - `4fba3095 test: share targetless status seed helper`
  - `9dbb7fbd test: reuse resolved seed in context queries`
  - `03ab767e test: share target proof site helpers`
  - `3a64af16 test: reuse target proof count helper`
  - `f32af71d test: split targetless fixture helpers`
  - `2aa601b0 test: split dynamic fixture helpers`
  - `8a3a5a7b test: split call graph row helpers`
  - `334b87af test: split target proof helpers`
  - `b8af3b29 test: share call expansion candidate assertions`
  - `83111953 test: batch resolved dynamic proof assertions`
  - `02313e8e test: share target caller assertions`
  - `bb27b696 test: share resolved proof fixture assertion`
  - `28a183da test: split fixture selector helpers`
  - `a363b462 test: compact targetless dynamic cases`
  - `9cb4f5b0 test: share try-result fixture setup`
  - `6d417c1c test: share initializer fixture scenarios`
  - `53081563 test: reuse try-result fixture scenario`
  - `02a874b1 test: split receiver decode query cases`
  - `056705b8 test: split local target prevalidation cases`
- Recent DB test-module split:
  - `7e51101d test: split dynamic call proof fixtures`
  - `09f8f967 test: split target proof fixtures`
  - `1c56f1fa test: split blocker proof fixtures`
  - `4f9c87d5 test: split proof lookup fixtures`
  - `4ea3b53c test: split call context fixture queries`
  - `0c06def4 test: split call graph fixture invariants`
  - `161f8abb test: split mixed proof fixtures`
  - `23db798b test: split resolved proof fixtures`
  - `9740a04e test: split dynamic context fixtures`
  - `feb23599 test: split associated context fixtures`
  - `4574718d test: split trait method context fixtures`
  - `e711b2d0 test: split owner context fixtures`
  - `05c2c273 test: split proof projection queries`
  - `fa3b273e test: split call graph query invariants`
  - `7eb4399f test: split call graph query concerns`
  - `54d94008 test: split call graph fixture query parent`
  - `ad9f0c07 test: split resolved proof fixtures by family`
  - `55b3ef66 test: split proof projection queries`
  - `03c343ec test: split call graph invariant queries`
  - `57ce0e36 test: split fixture context expansion tests`
  - `7221d75f test: split proof prevalidation queries`
  - `fcfafe15 test: split trait method context fixtures`
  - `48c5ceca test: split mixed proof fixtures`
  - `f09b72ff test: split path context fixtures`
  - `1720010c test: split dynamic proof fixtures`
  - `46c5a763 test: split fixture invariant tests`
  - `f3876a74 test: split blocker proof fixtures`
  - `83adea45 test: split proof blocker queries`
  - `011a4264 test: split associated context fixtures`
  - `7398c064 test: split context expansion query tests`
  - `a4159987 test: split target proof method fixtures`

## Recent production cleanup detail

- `CodeGraph` and `ParsedCodeGraph` now own structural call occurrence facts
  only: `call_sites` and `call_site_relations`.
- Semantic call target/status facts are no longer dormant parser graph fields.
  They are produced by `CallResolutionReport` at the resolver/transform
  boundary, matching the current type-graph-style ownership pattern.

## Recent consolidated DB split detail

- `call_graph_fixture_queries/associated_context.rs` is now a thin module root
  with alias, import, inherent, and trait-associated-function concerns split
  into child files.
- `call_graph_queries/context_expansion/expand.rs` is now a thin module root
  with navigation and non-resolved-promotion query behavior split into child
  files.
- `call_graph_fixture_queries/target_proof/methods.rs` is now a thin module
  root with method, associated-function, trait-associated-function, and trait
  dispatch proof concerns split into child files.
- `call_graph_fixture_common/lookup.rs` is now a thin helper root with
  function, method, type, and const/static lookup concerns split into child
  files; the shared Cozo row extraction helpers remain in the root.
- `call_graph_fixture_common/proof.rs` is now a thin helper root with owner
  proof-edge, blocker-proof, and target-centered proof helpers split into
  child files.
- `call_graph_fixture_common/proof/owners.rs` owns the shared
  `ResolvedProofCase`/`ResolvedProofCall` matrix helper used by resolved
  proof fixture families, so owner-context lookup, resolved-target assertions,
  proof projection counts, and `OwnerProofEdge` collection are not copied
  across each fixture file.
- `call_graph_fixture_common/targetless.rs` owns the shared
  `TargetlessOwnerCase` helper for owner lookup, context row-count checks, and
  targetless row assertions across mixed targetless fixture cases.
- `call_graph_fixture_common/proof/blockers.rs` owns the shared
  `TargetlessBlockerCase` projection helper for targetless blocker proof rows,
  now reused by callable, dynamic, and external blocker proof fixtures instead
  of copying owner lookup, call-context lookup, projection-count checks, and
  `BlockerProofSite` collection.
- `call_graph_common/resolved.rs` owns the synthetic `ResolvedGraphSeed`
  helper used by proof-projection query tests, so resolved path graph setup is
  shared across graph-context, prevalidation, resolved proof-projection, mixed
  proof-shape, and context-expansion query tests.
- `call_graph_common/targetless.rs` owns the synthetic `TargetlessStatusSeed`
  helper used by proof-projection blocker query tests for external and dynamic
  targetless status rows.
- `ProofGraphStore` now exposes a build-domain scoped proof context query and
  shares linked-call-site row selection across symbol, GraphRAG text, and
  build-domain proof lookups. Real fixture proof lookup assertions are
  table-driven across those query surfaces.
- `ProofGraphContextRow` now exposes `build_domain_id` for context consumers,
  along with persisted proof payload fields needed by downstream context
  consumers: `call_edge_id`, `resolution_state`, source span bounds,
  `effect_class`, and `status`. Low-level proof store tests cover
  build-domain lookup across generic proof facts, linked blockers, effect
  seeds, and source span payloads.
- `proof_graph.rs` now keeps public/store/query orchestration in the root while
  call-proof projection generation, call-context validation, proof fact JSON
  construction, and owner source-file lookup live in
  `proof_graph/call_projection.rs`.
- `call_graph.rs` now keeps only the public DB call-graph surface while
  concern modules own row DTOs, call-site/target/status kind decoding,
  receiver decoding, target-family rules, row validation, and query methods:
  `call_graph/{rows,kinds,receiver,families,decode,queries}.rs`.
- `call_graph_tests/dynamic.rs` in `ploke-transform` now shares local
  `DynamicFunction` lookup helpers instead of repeating the full
  `CallRelation::DynamicFunction` scan for every dynamic projection case.
- `call_graph_tests/dynamic.rs` in `ploke-transform` is now a thin test
  orchestration module. Dynamic projection case construction, dynamic call
  lookup, and persisted Cozo assertion helpers live in
  `call_graph_tests/dynamic/{cases,lookup,assertions}.rs`.
- `call_graph_fixture_common/proof/targets.rs` now owns shared target-centered
  proof-site case helpers used by function, dynamic, method, associated
  function, trait dispatch, and imported trait associated-function proof
  fixtures.
- Target-centered proof fixture linkage now reuses the shared
  `expected_target_proof_count` helper instead of copying the resolved/blocker
  proof fact count formula.
- RAG call-context public sparse `get_context` tests now share call-expansion
  provenance assertions for incoming and outgoing call-context expansion rows.
- `call_graph_fixture_common/targetless.rs` is now split into a thin
  `targetless/` helper root with row, method, and macro targetless assertions
  separated by concern while preserving the existing `targetless::*` import
  surface.
- `call_graph_fixture_common/dynamic.rs` is now split into a thin `dynamic/`
  helper root with ambiguous candidate helpers, dynamic context matrices, and
  dynamic proof assertions separated by concern while preserving the existing
  `dynamic::*` import surface.
- `call_graph_fixture_common/rows.rs` is now split into a thin `rows/` helper
  root with Cozo value decoding, site/status/relation row queries,
  status-shape validation, and proof-row helpers separated by concern while
  preserving the existing `rows::*` import surface.
- `call_graph_fixture_common/proof/targets.rs` is now split into a thin
  `proof/targets/` helper root with target-proof case construction,
  caller/count validation, and target proof projection assertions separated by
  concern while preserving the existing target-proof helper surface.
- Context-expansion fixture tests now share incoming/outgoing candidate
  assertions through `call_graph_fixture_common/selectors.rs`, so relation,
  target, call-site, and distance checks are centralized instead of copied in
  each expansion test.
- RAG public sparse `get_context` call-context tests now live in a dedicated
  `unit_tests/tests/call_context.rs` concern module while retaining the shared
  call-expansion assertion helpers in the parent test module for the next
  collection/expansion split.
- Resolved dynamic proof fixture tests now share a dynamic proof batch helper
  that preserves fresh-DB isolation per proof group while moving repeated
  target lookup, proof projection, and owner-edge assertions out of each test.
- Target-caller context expansion fixture tests now share caller target/status
  shape assertions through `call_graph_fixture_common/selectors.rs`.
- Resolved proof fixtures now share the common fixture domain/source/blocker
  assertion wrapper for owner proof projection, avoiding repeated
  `resolved_proof_edges` plus `assert_owner_proof_edges` boilerplate across
  associated-function, trait-family, local-receiver, and result/field method
  proof tests.
- `call_graph_fixture_common/selectors.rs` is now split into a thin
  `selectors/` helper root with row selectors, caller selectors/assertions,
  expansion candidate assertions, and path construction separated by concern
  while preserving the existing `selectors::*` import surface.
- `TargetlessDynamicContextCase` now has unsupported-case constructors used by
  the dynamic targetless fixture matrix, keeping the table focused on owner
  names and only spelling out paths for the exceptional path-bearing rows.
- `call_graph_fixture_common/scenarios.rs` owns the shared try-result fixture
  owner, context rows, call-site IDs, and target IDs used by context expansion
  and mixed-proof fixture assertions; method result-receiver fixtures now reuse
  the same scenario for the try-result receiver case.
- `call_graph_fixture_common/scenarios.rs` also owns const/static and
  associated-const initializer scenario matrices plus shared initializer
  context/proof assertions, so owner-context and mixed-proof initializer
  fixtures no longer duplicate owner lookup, context-row validation, source
  provenance, and proof-edge setup.
- `call_graph_queries/receiver_decode.rs` is now a thin synthetic query module
  root with receiver decode matrices split by persisted receiver shape:
  structured receiver payloads and raw receiver payloads share one insertion
  and assertion helper.
- `call_graph_queries/proof_projection/prevalidation/local_targets.rs` is now
  a thin module root with owner-scoped and target-centered local-target
  prevalidation tests split by concern and sharing synthetic local target row
  setup.

## Recent verification

- `b0fd0655 test: split dynamic transform projection cases`
  - `cargo test -p ploke-transform --features call_graph transform::call_graph_tests::dynamic -- --nocapture`
    passed: 1 passed, 0 failed.
  - `cargo test -p ploke-transform --features call_graph transform::call_graph_tests -- --nocapture`
    passed: 4 passed, 0 failed.
  - `cargo test -p ploke-transform --features call_graph transform::tests -- --nocapture`
    passed: 1 passed, 0 failed.
- RAG public sparse `get_context` call-context module split
  - `cargo test -p ploke-rag --features call_graph call_context_sparse_get_context -- --nocapture`
    passed: 8 passed, 0 failed.
- `056705b8 test: split local target prevalidation cases`
  - `cargo test -p ploke-db --features call_graph unit::call_graph_queries::proof_projection::prevalidation::local_targets -- --nocapture`
    passed: 4 passed, 0 failed.
  - `cargo test -p ploke-db --features call_graph unit::call_graph_queries::proof_projection -- --nocapture`
    passed: 14 passed, 0 failed.
  - `cargo test -p ploke-db --features call_graph unit::call_graph_queries -- --nocapture`
    passed: 34 passed, 0 failed.
- `02a874b1 test: split receiver decode query cases`
  - `cargo test -p ploke-db --features call_graph unit::call_graph_queries::receiver_decode -- --nocapture`
    passed: 2 passed, 0 failed.
  - `cargo test -p ploke-db --features call_graph unit::call_graph_queries -- --nocapture`
    passed: 34 passed, 0 failed.
- `53081563 test: reuse try-result fixture scenario`
  - `cargo test -p ploke-db --features call_graph unit::call_graph_fixture_queries::method_context -- --nocapture`
    passed: 8 passed, 0 failed.
  - `cargo test -p ploke-db --features call_graph unit::call_graph_fixture_queries -- --nocapture`
    passed: 93 passed, 0 failed.
- `6d417c1c test: share initializer fixture scenarios`
  - `cargo test -p ploke-db --features call_graph unit::call_graph_fixture_queries::owner_context -- --nocapture`
    passed: 3 passed, 0 failed.
  - `cargo test -p ploke-db --features call_graph unit::call_graph_fixture_queries::mixed_proof -- --nocapture`
    passed: 5 passed, 0 failed.
  - `cargo test -p ploke-db --features call_graph unit::call_graph_fixture_queries -- --nocapture`
    passed: 93 passed, 0 failed.
- `9cb4f5b0 test: share try-result fixture setup`
  - `cargo test -p ploke-db --features call_graph unit::call_graph_fixture_queries::context_expansion -- --nocapture`
    passed: 7 passed, 0 failed.
  - `cargo test -p ploke-db --features call_graph unit::call_graph_fixture_queries::mixed_proof -- --nocapture`
    passed: 5 passed, 0 failed.
  - `cargo test -p ploke-db --features call_graph unit::call_graph_fixture_queries -- --nocapture`
    passed: 93 passed, 0 failed.
- `a363b462 test: compact targetless dynamic cases`
  - `cargo test -p ploke-db --features call_graph unit::call_graph_fixture_queries::dynamic_context -- --nocapture`
    passed: 9 passed, 0 failed.
  - `cargo test -p ploke-db --features call_graph unit::call_graph_fixture_queries -- --nocapture`
    passed: 93 passed, 0 failed.
- `28a183da test: split fixture selector helpers`
  - `cargo test -p ploke-db --features call_graph unit::call_graph_fixture_queries -- --nocapture`
    passed: 93 passed, 0 failed.
- `bb27b696 test: share resolved proof fixture assertion`
  - `cargo test -p ploke-db --features call_graph unit::call_graph_fixture_queries::resolved_proof -- --nocapture`
    passed: 7 passed, 0 failed.
  - `cargo test -p ploke-db --features call_graph unit::call_graph_fixture_queries -- --nocapture`
    passed: 93 passed, 0 failed.
- `02313e8e test: share target caller assertions`
  - `cargo test -p ploke-db --features call_graph unit::call_graph_fixture_queries::context_expansion -- --nocapture`
    passed: 7 passed, 0 failed.
  - `cargo test -p ploke-db --features call_graph unit::call_graph_fixture_queries -- --nocapture`
    passed: 93 passed, 0 failed.
- `83111953 test: batch resolved dynamic proof assertions`
  - `cargo test -p ploke-db --features call_graph unit::call_graph_fixture_queries::dynamic_proof -- --nocapture`
    passed: 7 passed, 0 failed.
  - `cargo test -p ploke-db --features call_graph unit::call_graph_fixture_queries -- --nocapture`
    passed: 93 passed, 0 failed.
- `b8af3b29 test: share call expansion candidate assertions`
  - `cargo test -p ploke-db --features call_graph unit::call_graph_fixture_queries::context_expansion -- --nocapture`
    passed: 7 passed, 0 failed.
  - `cargo test -p ploke-db --features call_graph unit::call_graph_fixture_queries -- --nocapture`
    passed: 93 passed, 0 failed.
- `334b87af test: split target proof helpers`
  - `cargo test -p ploke-db --features call_graph unit::call_graph_fixture_queries -- --nocapture`
    passed: 93 passed, 0 failed.
- `8a3a5a7b test: split call graph row helpers`
  - `cargo test -p ploke-db --features call_graph unit::call_graph_fixture_queries -- --nocapture`
    passed: 93 passed, 0 failed.
- `2aa601b0 test: split dynamic fixture helpers`
  - `cargo test -p ploke-db --features call_graph unit::call_graph_fixture_queries -- --nocapture`
    passed: 93 passed, 0 failed.
- `f32af71d test: split targetless fixture helpers`
  - `cargo test -p ploke-db --features call_graph unit::call_graph_fixture_queries -- --nocapture`
    passed: 93 passed, 0 failed.
- `15f0ce68 test: share RAG call expansion assertions`
  - `cargo test -p ploke-rag --features call_graph call_context_sparse_get_context -- --nocapture`
    passed: 8 passed, 0 failed.
  - `cargo test -p ploke-rag --features call_graph call_context -- --nocapture`
    passed: 25 passed, 0 failed.
- `3a64af16 test: reuse target proof count helper`
  - `cargo test -p ploke-db --features call_graph unit::call_graph_fixture_queries::target_proof -- --nocapture`
    passed: 9 passed, 0 failed.
  - `cargo test -p ploke-db --features call_graph unit::call_graph_fixture_queries -- --nocapture`
    passed: 93 passed, 0 failed.
- `03ab767e test: share target proof site helpers`
  - `cargo test -p ploke-db --features call_graph unit::call_graph_fixture_queries::target_proof -- --nocapture`
    passed: 9 passed, 0 failed.
  - `cargo test -p ploke-db --features call_graph unit::call_graph_fixture_queries -- --nocapture`
    passed: 93 passed, 0 failed.

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
| CGQ-2 | P1 | Partial 2026-06-25 | Call-graph DB query tests and helpers were far larger and less modular than the type-graph precedent. | First cleanup extracted repeated ambiguous dynamic candidate context/proof assertions into shared helpers, moved them to `call_graph_fixture_common.rs`, moved shared proof and constructor fixture helpers to common, split the constructor helper case matrix into `call_graph_fixture_common/constructor.rs`, split ambiguous dynamic helper assertions into `call_graph_fixture_common/dynamic.rs`, table-drove resolved/targetless dynamic context assertions and resolved dynamic proof-edge assertions with shared dynamic helpers, split proof fixture assertions into `call_graph_fixture_common/proof.rs`, split row/candidate selectors into `call_graph_fixture_common/selectors.rs`, split lookup helpers into `call_graph_fixture_common/lookup.rs`, split row/shape helpers into `call_graph_fixture_common/rows.rs`, split synthetic call-site/proof-fact/value helpers into `call_graph_common/{site,facts,values}.rs`, split proof owner-source setup into `call_graph_common/source.rs`, split synthetic endpoint setup into `call_graph_common/targets.rs`, split unsupported/external targetless row assertions into `call_graph_fixture_common/targetless.rs`, reused those helpers across matching dynamic context/proof/method-context cases including dynamic proof, macro targetless rows, unsupported-context rows, and path-context targetless rows, and split invariant, dynamic, target-centered, blocker, proof-lookup, constructor proof, call-context, mixed proof, resolved proof, dynamic context, method context, associated context, trait-method context, owner-context, proof-projection, query-invariant, schema, context-expansion, receiver-decode, relation-decode, path-context, constructor-context, unsupported-context, low-level-helper, resolved-proof-family, target-proof, proof-projection query, invariant query, dynamic-context tests, context-expansion query tests, fixture context-expansion tests, proof-prevalidation query tests, trait-method context fixtures, mixed-proof fixtures, path-context fixtures, dynamic-proof fixtures, fixture invariant tests, blocker-proof fixtures, and proof-blocker query tests into submodules. `call_graph_queries.rs`, `call_graph_queries/context_expansion.rs`, `call_graph_queries/proof_projection.rs`, `call_graph_queries/proof_projection/prevalidation.rs`, `call_graph_queries/proof_projection/blockers.rs`, `call_graph_queries/invariants.rs`, `call_graph_fixture_queries/dynamic_context.rs`, `call_graph_fixture_queries/method_context.rs`, `call_graph_fixture_queries/target_proof.rs`, `call_graph_fixture_queries/context_expansion.rs`, `call_graph_fixture_queries/trait_method_context.rs`, `call_graph_fixture_queries/mixed_proof.rs`, `call_graph_fixture_queries/path_context.rs`, `call_graph_fixture_queries/dynamic_proof.rs`, `call_graph_fixture_queries/blocker_proof.rs`, `call_graph_fixture_queries/invariants.rs`, and `call_graph_fixture_queries.rs` are thin module roots, context-expansion query tests are grouped by owner/callers/expand surfaces, fixture context-expansion tests are grouped by callers/constructors/expand surfaces, proof prevalidation tests are grouped by domain/source/local-target concerns, proof blocker queries are grouped by graph-context/unresolved/mixed-shape/invariant concerns, trait-method fixtures are grouped by dispatch/generics/imports/blanket/receiver concerns, mixed proof fixtures are grouped by returned-function/initializer/multi-row concerns, path context fixtures are grouped by basic/resolution/special-form/raw-identifier/prelude concerns, dynamic proof fixtures are grouped by resolved/candidate/blocker concerns, blocker proof fixtures are grouped by external/macro/ambiguous/callable concerns, fixture invariant tests are grouped by body-edge/anchor/cardinality/id-universe concerns, dynamic context fixtures are grouped by bindings/resolved/targetless/ownership/candidates concerns, method context fixtures are grouped by local/external/precedence/result-receiver/field-receiver concerns, resolved proof fixtures are grouped by proof family, target proof fixtures are grouped by linkage/basic/method concerns, proof projection tests are grouped by resolved/prevalidation/blocker concerns, invariant tests are grouped by endpoint/status/cardinality/body-edge/site-shape concerns, and receiver decoding is table-driven. Production proof graph cleanup split invariant evaluation, proof-fact projection parsing/JSON validation, row decoding, and call-proof projection into `proof_graph/{invariants,projection,rows,call_projection}.rs`, reducing the root proof graph module to the public/store/query orchestration surface. Proof context lookup now shares linked-row selection across symbol, GraphRAG text, and build-domain queries, with fixture and low-level store assertions covering those query surfaces; context rows expose `build_domain_id` for downstream consumers. Remaining work is to continue splitting large fixture/common/proof files by concern and introduce shared matrices before adding more cases. |
| CGQ-2A | P1 | Done 2026-06-25 | Proof validation allowed arbitrary ambiguous rows with local targets while fixture invariants only allow dynamic function candidates. | `validate_call_context` now rejects non-resolved local targets except the established ambiguous dynamic-candidate shape (`Dynamic` site, `DynamicFunction` relation, `Function` target). |
| CGQ-3 | P1 | Done 2026-06-25 | Call endpoint-family rules were duplicated across transform insertion, DB Cozo queries, Rust validation, and tests. | DB query helpers and Rust validation share `VALID_CALL_TARGET_FAMILIES`; transform insertion uses typed `CallRelation` endpoint-kind helpers; fixture invariants now use exported DB helper predicates instead of a copied test-local family matrix. |
| CGQ-4 | P1 | Done 2026-06-25 | DB `CallRelationKind` mixed relation semantics with target-kind semantics, including `Struct` and `Variant`. | `CallTargetKind` now carries DB endpoint families separately from call relation kinds, while persisted relation strings remain unchanged. |
| CGQ-5 | P1 | Done 2026-06-25 | External proof projection reported `externally_summarized` while also using blocker reason `external_dependency_summary_missing`. | Current parser-projected external rows now emit blocked/missing-summary proof state until a real external summary fact exists. |
| CGQ-6 | P2 | Done 2026-06-25 | Dynamic branch/match candidate provenance can collapse to `Null` in persisted `call_site` rows. | Ambiguous branch/match dynamic calls now keep proven local function candidates as `DynamicFunction` relations while preserving `Ambiguous` status; proof projection exposes them as `candidate_def_ids` without promoting them to resolved call edges. |
| CGQ-7 | P2 | Done 2026-06-25 | The "exactly one status per call site" invariant could be hidden by post-hoc sort/dedup. | `CallRelationResolver` now rejects duplicate status sources before dedup, including identical duplicates. |
| CGQ-8 | P2 | Open | `call_resolution.rs` and `call_extraction.rs` are monolithic. | Avoid adding breadth there without local extraction; split path/method/dynamic/macro logic when touching the area. |
| CGQ-9 | P2 | Done 2026-06-25 | Call-graph docs were serving as a long running diary rather than a stable coverage inventory. | Added `2026-06-25_call-graph-coverage-inventory.md` as the compact layer-by-layer restart inventory and linked it from the call-graph restart spine. |
| CGQ-10 | P3 | Done 2026-06-25 | `call_graph` feature name is broader than the actual gate: parser facts exist baseline, DB projection is gated. | Cargo feature comments and active gate docs now state that `call_graph` is a historical feature name whose active rollout gate is DB projection plus downstream consumers while backup fixtures are reviewed/regenerated. |
| CGQ-11 | P1 | Done 2026-06-25 | `CodeGraph` carried dormant semantic call-target/status storage even though transform consumes `CallResolutionReport` directly. | `CodeGraph`/`ParsedCodeGraph` now keep only structural call occurrence facts; semantic call relations/statuses are report-owned at the resolver/transform boundary. |

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

Continue production-side pattern cleanup before parser/resolver breadth. The
next likely slices are:

1. Continue DB/proof/RAG/TUI coverage work before parser breadth. Prefer
   existing common helpers and only extract new macro/method blocker helpers if
   another file shares the same shape.
2. Use the stable coverage inventory plus detailed call-site matrix to choose
   the next DB/proof/RAG/TUI batch before parser breadth.
3. Keep splitting/table-driving any large helper or projection test touched by
   that batch before adding cases.

## Latest verification

For `c26aeedd test: share dynamic transform lookup helpers`:

- `cargo test -p ploke-transform --features call_graph transform::call_graph_tests::dynamic -- --nocapture`
  - passed: dynamic transform call-graph filter ran 1 test, 0 failed.
- `cargo test -p ploke-transform --features call_graph transform::call_graph_tests -- --nocapture`
  - passed: transform call-graph filter ran 4 tests, 0 failed.
- `cargo test -p ploke-transform --features call_graph transform::tests -- --nocapture`
  - passed: remaining transform test filter ran 1 test, 0 failed.

For `9dbb7fbd test: reuse resolved seed in context queries`:

- `cargo test -p ploke-db --features call_graph unit::call_graph_queries::context_expansion::owner -- --nocapture`
  - passed: owner context-expansion query filter ran 1 test, 0 failed.
- `cargo test -p ploke-db --features call_graph unit::call_graph_queries::context_expansion::callers -- --nocapture`
  - passed: target callers context-expansion query filter ran 1 test, 0 failed.
- `cargo test -p ploke-db --features call_graph unit::call_graph_queries::context_expansion::expand::navigation -- --nocapture`
  - passed: expansion navigation query filter ran 1 test, 0 failed.
- `cargo test -p ploke-db --features call_graph unit::call_graph_queries::context_expansion -- --nocapture`
  - passed: context-expansion query filter ran 4 tests, 0 failed.
- `cargo test -p ploke-db --features call_graph unit::call_graph_queries -- --nocapture`
  - passed: synthetic call-graph query filter ran 33 tests, 0 failed.

For `4fba3095 test: share targetless status seed helper`:

- `cargo test -p ploke-db --features call_graph unit::call_graph_queries::proof_projection::blockers::unresolved -- --nocapture`
  - passed: unresolved blocker projection query filter ran 1 test, 0 failed.
- `cargo test -p ploke-db --features call_graph unit::call_graph_queries::proof_projection::blockers::mixed_shape -- --nocapture`
  - passed: mixed-shape blocker projection query filter ran 1 test, 0 failed.
- `cargo test -p ploke-db --features call_graph unit::call_graph_queries::proof_projection::blockers::invariants -- --nocapture`
  - passed: blocker invariant query filter ran 1 test, 0 failed.
- `cargo test -p ploke-db --features call_graph unit::call_graph_queries::proof_projection::blockers -- --nocapture`
  - passed: proof blocker projection query filter ran 5 tests, 0 failed.
- `cargo test -p ploke-db --features call_graph unit::call_graph_queries::proof_projection -- --nocapture`
  - passed: proof projection query filter ran 14 tests, 0 failed.
- `cargo test -p ploke-db --features call_graph unit::call_graph_queries -- --nocapture`
  - passed: synthetic call-graph query filter ran 33 tests, 0 failed.

For `b8173ff9 test: reuse resolved graph seed cases`:

- `cargo test -p ploke-db --features call_graph unit::call_graph_queries::proof_projection::resolved -- --nocapture`
  - passed: resolved proof-projection query filter ran 2 tests, 0 failed.
- `cargo test -p ploke-db --features call_graph unit::call_graph_queries::proof_projection::blockers::mixed_shape -- --nocapture`
  - passed: mixed-shape blocker projection query filter ran 1 test, 0 failed.
- `cargo test -p ploke-db --features call_graph unit::call_graph_queries::proof_projection -- --nocapture`
  - passed: proof projection query filter ran 14 tests, 0 failed.
- `cargo test -p ploke-db --features call_graph unit::call_graph_queries -- --nocapture`
  - passed: synthetic call-graph query filter ran 33 tests, 0 failed.

For `0fee7f60 test: share resolved graph seed helper`:

- `cargo test -p ploke-db --features call_graph unit::call_graph_queries::proof_projection::prevalidation -- --nocapture`
  - passed: proof prevalidation filter ran 7 tests, 0 failed.
- `cargo test -p ploke-db --features call_graph unit::call_graph_queries::proof_projection::blockers::graph_context -- --nocapture`
  - passed: graph-context blocker projection filter ran 2 tests, 0 failed.
- `cargo test -p ploke-db --features call_graph unit::call_graph_queries::proof_projection::blockers -- --nocapture`
  - passed: proof blocker projection filter ran 5 tests, 0 failed.
- `cargo test -p ploke-db --features call_graph unit::call_graph_queries::proof_projection -- --nocapture`
  - passed: proof projection query filter ran 14 tests, 0 failed.
- `cargo test -p ploke-db --features call_graph unit::call_graph_queries -- --nocapture`
  - passed: synthetic call-graph query filter ran 33 tests, 0 failed.

For `162322ec test: reuse external blocker proof helper`:

- `cargo test -p ploke-db --features call_graph unit::call_graph_fixture_queries::blocker_proof::external -- --nocapture`
  - passed: external blocker proof fixture filter ran 2 tests, 0 failed.
- `cargo test -p ploke-db --features call_graph unit::call_graph_fixture_queries::blocker_proof -- --nocapture`
  - passed: blocker proof fixture filter ran 6 tests, 0 failed.
- `cargo test -p ploke-db --features call_graph unit::call_graph_fixture_queries -- --nocapture`
  - passed: fixture-backed call-graph query filter ran 93 tests, 0 failed.

For `8f371fa5 test: share projected blocker helper`:

- `cargo test -p ploke-db --features call_graph unit::call_graph_fixture_queries::blocker_proof::callable -- --nocapture`
  - passed: callable blocker proof fixture filter ran 2 tests, 0 failed.
- `cargo test -p ploke-db --features call_graph unit::call_graph_fixture_queries::dynamic_proof::blockers -- --nocapture`
  - passed: dynamic blocker proof fixture filter ran 2 tests, 0 failed.
- `cargo test -p ploke-db --features call_graph unit::call_graph_fixture_queries::dynamic_proof -- --nocapture`
  - passed: dynamic proof fixture filter ran 7 tests, 0 failed.
- `cargo test -p ploke-db --features call_graph unit::call_graph_fixture_queries -- --nocapture`
  - passed: fixture-backed call-graph query filter ran 93 tests, 0 failed.

For `7140e189 test: share targetless owner case helper`:

- `cargo test -p ploke-db --features call_graph unit::call_graph_fixture_queries::dynamic_context::targetless -- --nocapture`
  - passed: dynamic targetless fixture filter ran 2 tests, 0 failed.
- `cargo test -p ploke-db --features call_graph unit::call_graph_fixture_queries::dynamic_context -- --nocapture`
  - passed: dynamic context fixture filter ran 9 tests, 0 failed.
- `cargo test -p ploke-db --features call_graph unit::call_graph_fixture_queries -- --nocapture`
  - passed: fixture-backed call-graph query filter ran 93 tests, 0 failed.

For `e23d501a test: table-drive local target prevalidation`:

- `cargo test -p ploke-db --features call_graph unit::call_graph_queries::proof_projection::prevalidation::local_targets -- --nocapture`
  - passed: local target prevalidation filter ran 4 tests, 0 failed.
- `cargo test -p ploke-db --features call_graph unit::call_graph_queries::proof_projection::prevalidation -- --nocapture`
  - passed: proof prevalidation filter ran 7 tests, 0 failed.
- `cargo test -p ploke-db --features call_graph unit::call_graph_queries::proof_projection -- --nocapture`
  - passed: proof projection query filter ran 14 tests, 0 failed.

For `000a3e58 test: share resolved proof case helper`:

- `cargo test -p ploke-db --features call_graph unit::call_graph_fixture_queries::resolved_proof -- --nocapture`
  - passed: resolved proof fixture filter ran 7 tests, 0 failed.
- `cargo test -p ploke-db --features call_graph unit::call_graph_fixture_queries -- --nocapture`
  - passed: fixture-backed call-graph query filter ran 93 tests, 0 failed.

For `7710313b test: share callable proof blocker helper`:

- `cargo test -p ploke-db --features call_graph unit::call_graph_fixture_queries::blocker_proof::callable -- --nocapture`
  - passed: callable blocker proof fixture filter ran 2 tests, 0 failed.
- `cargo test -p ploke-db --features call_graph unit::call_graph_fixture_queries::blocker_proof -- --nocapture`
  - passed: blocker proof fixture filter ran 6 tests, 0 failed.

For `cc808347 test: cover variant constructor tool context`:

- `cargo test -p ploke-tui --features call_graph,test_harness request_code_context_returns_constructor_target_callers_with_call_context -- --nocapture`
  - passed: focused request-code-context constructor filter ran 1 test, 0
    failed.
- `cargo test -p ploke-tui --features call_graph,test_harness call_context -- --nocapture`
  - passed: TUI call-context filter ran 12 tests, 0 failed.

For `31d19d3c Add variant sparse call-context seeds`:

- Red check before implementation:
  `cargo test -p ploke-rag --features call_graph call_context_sparse_get_context_expands_constructor_target_hits_to_fixture_callers -- --nocapture`
  failed because the enum-variant constructor query produced no sparse hit for
  the `Variant` target.
- `cargo test -p ploke-db collect_rebuild_sources_includes_secondary_search_nodes -- --nocapture`
  - passed: BM25 source matrix ran 1 test, 0 failed.
- `cargo test -p ploke-rag --features call_graph call_context_sparse_get_context_expands_constructor_target_hits_to_fixture_callers -- --nocapture`
  - passed: public sparse constructor `get_context` filter ran 1 test, 0
    failed.
- `cargo test -p ploke-db bm25_index::tests -- --nocapture`
  - passed: BM25 module filter ran 12 tests, 0 failed.
- `cargo test -p ploke-rag --features call_graph call_context -- --nocapture`
  - passed: RAG call-context filter ran 25 tests, 0 failed.

For `3cce5b86 Split call graph DB module` and `e431a2f7 Expose proof context payload fields`:

- Red check before exposing proof context payload fields:
  `cargo test -p ploke-db --test proof_graph_store -- --nocapture`
  failed because `ProofGraphContextRow` did not yet expose `start_byte`,
  `end_byte`, `line_end`, `call_edge_id`, `resolution_state`,
  `effect_class`, or `status`.
- `cargo test -p ploke-db --test proof_graph_store -- --nocapture`
  - passed: proof graph store test binary ran 11 tests, 0 failed.
- `cargo test -p ploke-db --features call_graph unit::call_graph_queries::proof_projection -- --nocapture`
  - passed: proof projection filter ran 14 tests, 0 failed.
- `cargo test -p ploke-db --features call_graph unit::call_graph_queries -- --nocapture`
  - passed after the DB module split: synthetic call-graph query filter ran
    33 tests, 0 failed.
- `cargo test -p ploke-db --features call_graph unit::call_graph_fixture_queries -- --nocapture`
  - passed after the DB module split: fixture-backed call-graph query filter
    ran 93 tests, 0 failed.
- `cargo test -p ploke-rag --features call_graph call_context -- --nocapture`
  - passed after the DB module split: RAG call-context filter ran 25 tests,
    0 failed.

For `9ba4d247 Split call proof projection module`, `30a9cb2e Expose proof context build domains`, and `97e64623 test: cover proof domain store lookups`:

- Red check before exposing build domains:
  `cargo test -p ploke-db --test proof_graph_store -- --nocapture`
  failed because `ProofGraphContextRow` did not yet expose `build_domain_id`.
- `cargo test -p ploke-db --test proof_graph_store -- --nocapture`
  - passed: proof graph store test binary ran 11 tests, 0 failed.
- `cargo test -p ploke-db --features call_graph unit::call_graph_queries::proof_projection::blockers::graph_context -- --nocapture`
  - passed: graph-context proof projection filter ran 2 tests, 0 failed.
- `cargo test -p ploke-db --features call_graph unit::call_graph_queries::proof_projection -- --nocapture`
  - passed after the call-projection module split: proof projection filter ran 14 tests, 0 failed.

For `8a6eeae1 test: table drive proof context lookups` and `3284dd45 Add proof domain context lookup`:

- Red check before implementation:
  `cargo test -p ploke-db --features call_graph unit::call_graph_queries::proof_projection::blockers::graph_context -- --nocapture`
  failed because `Database` did not yet expose `proof_domain_context`.
- `cargo test -p ploke-db --features call_graph unit::call_graph_queries::proof_projection::blockers::graph_context -- --nocapture`
  - passed: graph-context proof projection filter ran 2 tests, 0 failed.
- `cargo test -p ploke-db --features call_graph unit::call_graph_queries::proof_projection -- --nocapture`
  - passed: proof projection filter ran 14 tests, 0 failed.
- `cargo test -p ploke-db --features call_graph unit::call_graph_fixture_queries::proof_lookup -- --nocapture`
  - passed: fixture proof lookup filter ran 2 tests, 0 failed.
- `cargo test -p ploke-db --features call_graph unit::call_graph_fixture_queries -- --nocapture`
  - passed: fixture query filter ran 93 tests, 0 failed.

For `4f6e314a Split proof fact row decoding`, `853e308e Split proof fact projection parsing`, and `1c49e955 Split proof graph invariant evaluation`:

- `cargo test -p ploke-db --features call_graph unit::call_graph_queries::proof_projection -- --nocapture`
  - passed after each split: proof projection filter ran 13 tests, 0 failed.

For `b25fa650 test: table drive dynamic transform projection`:

- `cargo test -p ploke-transform --features call_graph transform::call_graph_tests -- --nocapture`
  - passed: moved call-graph projection filter ran 4 tests, 0 failed.

For `8d3dc030 test: split transform call graph projection tests`:

- `cargo test -p ploke-transform --features call_graph transform::call_graph_tests -- --nocapture`
  - passed: moved call-graph projection filter ran 4 tests, 0 failed.
- `cargo test -p ploke-transform --features call_graph transform::tests -- --nocapture`
  - passed: remaining inline transform filter ran 1 test, 0 failed.

For `2d6320d1 Require populated call graph availability`:

- Red check before implementation:
  `cargo test -p ploke-db --features call_graph unit::call_graph_queries::schema -- --nocapture`
  failed because schema-only databases still reported call graph availability.
- Green checks after implementation:
  - `cargo test -p ploke-db --features call_graph unit::call_graph_queries::schema -- --nocapture`
    - passed: schema availability filter ran 1 test, 0 failed.
  - `cargo test -p ploke-rag --features call_graph call_context_disabled_safely_when_relations_absent -- --nocapture`
    - passed: RAG absent-call-graph degradation filter ran 1 test, 0 failed.
  - `cargo test -p ploke-rag --features call_graph call_context_collection_attaches_outgoing_call_payloads -- --nocapture`
    - passed: RAG populated call-context collection filter ran 1 test, 0 failed.

For `f7349dd7 Share call target family rules with tests`:

- `cargo test -p ploke-db --features call_graph unit::call_graph_fixture_queries::invariants -- --nocapture`
  - passed: fixture invariant filter ran 5 tests, 0 failed.
- `cargo test -p ploke-db --features call_graph unit::call_graph_queries::invariants -- --nocapture`
  - passed: call-graph query invariant filter ran 11 tests, 0 failed.

For `38640a2c Centralize call relation endpoint kinds`:

- `cargo test -p syn_parser --features call_graph call_sites`
  - passed: `syn_parser` unit tests 206 passed; `tests/mod.rs` 206 passed, 0 failed.
- `cargo test -p ploke-transform --features call_graph transform::tests`
  - passed: transform-focused filter ran 5 tests, 0 failed.

For `382d83c7 test: split constructor fixture helpers`:

- `cargo test -p ploke-db --features call_graph unit::call_graph_fixture_queries::constructor -- --nocapture`
  - passed: constructor-focused filter ran 3 tests, 0 failed.
- `cargo test -p ploke-db --features call_graph fixture_projection -- --nocapture`
  - passed: `tests/mod.rs` 34 passed, 0 failed.

For `55b3ef66 test: split proof projection queries`:

- `cargo test -p ploke-db --features call_graph unit::call_graph_queries::proof_projection -- --nocapture`
  - passed: `tests/mod.rs` 13 passed, 0 failed.
- `cargo test -p ploke-db --features call_graph unit::call_graph_queries -- --nocapture`
  - passed: `tests/mod.rs` 32 passed, 0 failed.

For `03c343ec test: split call graph invariant queries`:

- `cargo test -p ploke-db --features call_graph unit::call_graph_queries::invariants -- --nocapture`
  - passed: `tests/mod.rs` 11 passed, 0 failed.
- `cargo test -p ploke-db --features call_graph unit::call_graph_queries -- --nocapture`
  - passed: `tests/mod.rs` 32 passed, 0 failed.

For `53bc6e52 test: split dynamic fixture helpers`:

- `cargo test -p ploke-db --features call_graph unit::call_graph_fixture_queries::dynamic -- --nocapture`
  - passed: `tests/mod.rs` 16 passed, 0 failed.
- `cargo test -p ploke-db --features call_graph fixture_projection -- --nocapture`
  - passed: `tests/mod.rs` 34 passed, 0 failed.

For `f1e4bb14 test: split proof fixture helpers`:

- `cargo test -p ploke-db --features call_graph _proof -- --nocapture`
  - passed: proof-filtered run 47 passed, 0 failed.
- `cargo test -p ploke-db --features call_graph fixture_projection -- --nocapture`
  - passed: `tests/mod.rs` 34 passed, 0 failed.

For `04070efa test: split selector fixture helpers`:

- `cargo test -p ploke-db --features call_graph unit::call_graph_fixture_queries::context_expansion -- --nocapture`
  - passed: `tests/mod.rs` 7 passed, 0 failed.
- `cargo test -p ploke-db --features call_graph unit::call_graph_fixture_queries::method_context -- --nocapture`
  - passed: `tests/mod.rs` 8 passed, 0 failed.
- `cargo test -p ploke-db --features call_graph unit::call_graph_fixture_queries::dynamic_context -- --nocapture`
  - passed: `tests/mod.rs` 9 passed, 0 failed.
- `cargo test -p ploke-db --features call_graph unit::call_graph_fixture_queries::target_proof -- --nocapture`
  - passed: `tests/mod.rs` 9 passed, 0 failed.
- `cargo test -p ploke-db --features call_graph fixture_projection -- --nocapture`
  - passed: `tests/mod.rs` 34 passed, 0 failed.

For `d954a39d test: split call graph common helpers`:

- `cargo test -p ploke-db --features call_graph unit::call_graph_queries -- --nocapture`
  - passed: `tests/mod.rs` 32 passed, 0 failed.

For `a9f365c1 test: split call graph source helpers`:

- `cargo test -p ploke-db --features call_graph unit::call_graph_queries::proof_projection -- --nocapture`
  - passed: `tests/mod.rs` 13 passed, 0 failed.

For `1172f0c1 test: split call graph target helpers`:

- `cargo test -p ploke-db --features call_graph unit::call_graph_queries -- --nocapture`
  - passed: `tests/mod.rs` 32 passed, 0 failed.

For `950c1e94 test: split call graph lookup helpers`:

- `cargo test -p ploke-db --features call_graph unit::call_graph_fixture_queries -- --nocapture`
  - passed: `tests/mod.rs` 94 passed, 0 failed.

For `445bee67 test: split call graph row helpers`:

- `cargo test -p ploke-db --features call_graph unit::call_graph_fixture_queries -- --nocapture`
  - passed: `tests/mod.rs` 94 passed, 0 failed.

For `5469e87a test: table-drive dynamic context helpers`:

- `cargo test -p ploke-db --features call_graph unit::call_graph_fixture_queries::dynamic_context -- --nocapture`
  - passed: `tests/mod.rs` 9 passed, 0 failed.

For `af7c5d12 test: table-drive dynamic proof edges`:

- `cargo test -p ploke-db --features call_graph unit::call_graph_fixture_queries::dynamic_proof -- --nocapture`
  - passed: `tests/mod.rs` 7 passed, 0 failed.

For `87600137 test: split target proof fixtures`:

- `cargo test -p ploke-db --features call_graph unit::call_graph_fixture_queries::target_proof -- --nocapture`
  - passed: `tests/mod.rs` 9 passed, 0 failed.

For `7c2a308f test: share targetless call row assertions`:

- `cargo test -p ploke-db --features call_graph unit::call_graph_fixture_queries -- --nocapture`
  - passed: `tests/mod.rs` 94 passed, 0 failed.

For `693b305b test: reuse targetless row assertions`:

- `cargo test -p ploke-db --features call_graph unit::call_graph_fixture_queries::dynamic_context -- --nocapture`
  - passed: `tests/mod.rs` 9 passed, 0 failed.
- `cargo test -p ploke-db --features call_graph unit::call_graph_fixture_queries::blocker_proof -- --nocapture`
  - passed: `tests/mod.rs` 6 passed, 0 failed.

For `695ad2d9 test: split targetless fixture helpers`:

- `cargo test -p ploke-db --features call_graph unit::call_graph_fixture_queries -- --nocapture`
  - passed: `tests/mod.rs` 94 passed, 0 failed.

For `1581abbf test: reuse dynamic proof targetless helper`:

- `cargo test -p ploke-db --features call_graph unit::call_graph_fixture_queries::dynamic_proof -- --nocapture`
  - passed: `tests/mod.rs` 7 passed, 0 failed.

For `f9d60dbc test: share targetless macro assertions`:

- `cargo test -p ploke-db --features call_graph unit::call_graph_fixture_queries::blocker_proof -- --nocapture`
  - passed: `tests/mod.rs` 6 passed, 0 failed.
- `cargo test -p ploke-db --features call_graph unit::call_graph_fixture_queries::unsupported_context -- --nocapture`
  - passed: `tests/mod.rs` 5 passed, 0 failed.

For `e0d98916 test: reuse unsupported context helpers`:

- `cargo test -p ploke-db --features call_graph unit::call_graph_fixture_queries::unsupported_context -- --nocapture`
  - passed: `tests/mod.rs` 5 passed, 0 failed.

For `b5a588f2 test: reuse path context targetless helpers`:

- `cargo test -p ploke-db --features call_graph unit::call_graph_fixture_queries::path_context -- --nocapture`
  - passed: `tests/mod.rs` 5 passed, 0 failed.

For `9c527985 test: split dynamic context fixtures`:

- `cargo test -p ploke-db --features call_graph unit::call_graph_fixture_queries::dynamic_context -- --nocapture`
  - passed: `tests/mod.rs` 9 passed, 0 failed.

For `2ba35649 test: split method context fixtures`:

- `cargo test -p ploke-db --features call_graph unit::call_graph_fixture_queries::method_context -- --nocapture`
  - passed: `tests/mod.rs` 8 passed, 0 failed.

For `cfa3392d test: split call graph context expansion queries`:

- `cargo test -p ploke-db --features call_graph unit::call_graph_queries::context_expansion -- --nocapture`
  - passed: `tests/mod.rs` 4 passed, 0 failed.

For `57ce0e36 test: split fixture context expansion tests`:

- `cargo test -p ploke-db --features call_graph unit::call_graph_fixture_queries::context_expansion -- --nocapture`
  - passed: `tests/mod.rs` 7 passed, 0 failed.

For `7221d75f test: split proof prevalidation queries`:

- `cargo test -p ploke-db --features call_graph unit::call_graph_queries::proof_projection::prevalidation -- --nocapture`
  - passed: `tests/mod.rs` 7 passed, 0 failed.

For `fcfafe15 test: split trait method context fixtures`:

- `cargo test -p ploke-db --features call_graph unit::call_graph_fixture_queries::trait_method_context -- --nocapture`
  - passed: `tests/mod.rs` 6 passed, 0 failed.

For `48c5ceca test: split mixed proof fixtures`:

- `cargo test -p ploke-db --features call_graph unit::call_graph_fixture_queries::mixed_proof -- --nocapture`
  - passed: `tests/mod.rs` 5 passed, 0 failed.

For `f09b72ff test: split path context fixtures`:

- `cargo test -p ploke-db --features call_graph unit::call_graph_fixture_queries::path_context -- --nocapture`
  - passed: `tests/mod.rs` 5 passed, 0 failed.

For `1720010c test: split dynamic proof fixtures`:

- `cargo test -p ploke-db --features call_graph unit::call_graph_fixture_queries::dynamic_proof -- --nocapture`
  - passed: `tests/mod.rs` 7 passed, 0 failed.

For `46c5a763 test: split fixture invariant tests`:

- `cargo test -p ploke-db --features call_graph unit::call_graph_fixture_queries::invariants -- --nocapture`
  - passed: `tests/mod.rs` 5 passed, 0 failed.

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

For `0c06def4 test: split call graph fixture invariants`:

- `cargo test -p ploke-db --features call_graph unit::call_graph_fixture_queries::invariants -- --nocapture`
  - passed: `tests/mod.rs` 5 passed, 0 failed.
- `cargo test -p ploke-db --features call_graph fixture_projection -- --nocapture`
  - passed: `tests/mod.rs` 34 passed, 0 failed.

For `161f8abb test: split mixed proof fixtures`:

- `cargo test -p ploke-db --features call_graph unit::call_graph_fixture_queries::mixed_proof -- --nocapture`
  - passed: `tests/mod.rs` 5 passed, 0 failed.
- `cargo test -p ploke-db --features call_graph fixture_projection -- --nocapture`
  - passed: `tests/mod.rs` 34 passed, 0 failed.

For `23db798b test: split resolved proof fixtures`:

- `cargo test -p ploke-db --features call_graph unit::call_graph_fixture_queries::resolved_proof -- --nocapture`
  - passed: `tests/mod.rs` 7 passed, 0 failed.
- `cargo test -p ploke-db --features call_graph fixture_projection -- --nocapture`
  - passed: `tests/mod.rs` 34 passed, 0 failed.

For `9740a04e test: split dynamic context fixtures`:

- `cargo test -p ploke-db --features call_graph unit::call_graph_fixture_queries::dynamic_context -- --nocapture`
  - passed: `tests/mod.rs` 8 passed, 0 failed.
- `cargo test -p ploke-db --features call_graph fixture_projection -- --nocapture`
  - passed: `tests/mod.rs` 34 passed, 0 failed.

For `feb23599 test: split associated context fixtures`:

- `cargo test -p ploke-db --features call_graph unit::call_graph_fixture_queries::associated_context -- --nocapture`
  - passed: `tests/mod.rs` 5 passed, 0 failed.
- `cargo test -p ploke-db --features call_graph fixture_projection -- --nocapture`
  - passed: `tests/mod.rs` 34 passed, 0 failed.

For `4574718d test: split trait method context fixtures`:

- `cargo test -p ploke-db --features call_graph unit::call_graph_fixture_queries::trait_method_context -- --nocapture`
  - passed: `tests/mod.rs` 6 passed, 0 failed.
- `cargo test -p ploke-db --features call_graph fixture_projection -- --nocapture`
  - passed: `tests/mod.rs` 34 passed, 0 failed.

For `e711b2d0 test: split owner context fixtures`:

- `cargo test -p ploke-db --features call_graph unit::call_graph_fixture_queries::owner_context -- --nocapture`
  - passed: `tests/mod.rs` 3 passed, 0 failed.
- `cargo test -p ploke-db --features call_graph fixture_projection -- --nocapture`
  - passed: `tests/mod.rs` 34 passed, 0 failed.

For `05c2c273 test: split proof projection queries`:

- `cargo test -p ploke-db --features call_graph unit::call_graph_queries::proof_projection -- --nocapture`
  - passed: `tests/mod.rs` 13 passed, 0 failed.
- `cargo test -p ploke-db --features call_graph unit::call_graph_queries -- --nocapture`
  - passed: `tests/mod.rs` 34 passed, 0 failed.
- `cargo test -p ploke-db --features call_graph unit::call_graph_fixture_queries::invariants -- --nocapture`
  - passed: `tests/mod.rs` 5 passed, 0 failed.
- `cargo test -p ploke-db --features call_graph fixture_projection -- --nocapture`
  - passed: `tests/mod.rs` 34 passed, 0 failed.

For `fa3b273e test: split call graph query invariants`:

- `cargo test -p ploke-db --features call_graph unit::call_graph_queries::invariants -- --nocapture`
  - passed: `tests/mod.rs` 11 passed, 0 failed.
- `cargo test -p ploke-db --features call_graph unit::call_graph_queries -- --nocapture`
  - passed: `tests/mod.rs` 34 passed, 0 failed.

For `7eb4399f test: split call graph query concerns`:

- `cargo test -p ploke-db --features call_graph unit::call_graph_queries::context_expansion -- --nocapture`
  - passed: `tests/mod.rs` 4 passed, 0 failed.
- `cargo test -p ploke-db --features call_graph unit::call_graph_queries::receiver_decode -- --nocapture`
  - passed: `tests/mod.rs` 1 passed, 0 failed.
- `cargo test -p ploke-db --features call_graph unit::call_graph_queries::relation_decode -- --nocapture`
  - passed: `tests/mod.rs` 2 passed, 0 failed.
- `cargo test -p ploke-db --features call_graph unit::call_graph_queries::schema -- --nocapture`
  - passed: `tests/mod.rs` 1 passed, 0 failed.
- `cargo test -p ploke-db --features call_graph unit::call_graph_queries -- --nocapture`
  - passed: `tests/mod.rs` 32 passed, 0 failed.

For `54d94008 test: split call graph fixture query parent`:

- `cargo test -p ploke-db --features call_graph unit::call_graph_fixture_queries::path_context -- --nocapture`
  - passed: `tests/mod.rs` 5 passed, 0 failed.
- `cargo test -p ploke-db --features call_graph unit::call_graph_fixture_queries::method_context -- --nocapture`
  - passed: `tests/mod.rs` 8 passed, 0 failed.
- `cargo test -p ploke-db --features call_graph unit::call_graph_fixture_queries::constructor_context -- --nocapture`
  - passed: `tests/mod.rs` 2 passed, 0 failed.
- `cargo test -p ploke-db --features call_graph unit::call_graph_fixture_queries::unsupported_context -- --nocapture`
  - passed: `tests/mod.rs` 5 passed, 0 failed.
- `cargo test -p ploke-db --features call_graph unit::call_graph_fixture_queries::low_level_helpers -- --nocapture`
  - passed: `tests/mod.rs` 1 passed, 0 failed.
- `cargo test -p ploke-db --features call_graph unit::call_graph_fixture_queries::dynamic_context -- --nocapture`
  - passed: `tests/mod.rs` 9 passed, 0 failed.
- `cargo test -p ploke-db --features call_graph fixture_projection -- --nocapture`
  - passed: `tests/mod.rs` 34 passed, 0 failed.

For `ad9f0c07 test: split resolved proof fixtures by family`:

- `cargo test -p ploke-db --features call_graph unit::call_graph_fixture_queries::resolved_proof::path_resolution -- --nocapture`
  - passed: `tests/mod.rs` 1 passed, 0 failed.
- `cargo test -p ploke-db --features call_graph unit::call_graph_fixture_queries::resolved_proof::associated_function -- --nocapture`
  - passed: `tests/mod.rs` 1 passed, 0 failed.
- `cargo test -p ploke-db --features call_graph unit::call_graph_fixture_queries::resolved_proof::local_receiver_methods -- --nocapture`
  - passed: `tests/mod.rs` 1 passed, 0 failed.
- `cargo test -p ploke-db --features call_graph unit::call_graph_fixture_queries::resolved_proof::trait_family_methods -- --nocapture`
  - passed: `tests/mod.rs` 1 passed, 0 failed.
- `cargo test -p ploke-db --features call_graph unit::call_graph_fixture_queries::resolved_proof::result_field_methods -- --nocapture`
  - passed: `tests/mod.rs` 1 passed, 0 failed.
- `cargo test -p ploke-db --features call_graph unit::call_graph_fixture_queries::resolved_proof::trait_dispatch -- --nocapture`
  - passed: `tests/mod.rs` 1 passed, 0 failed.
- `cargo test -p ploke-db --features call_graph fixture_projection -- --nocapture`
  - passed: `tests/mod.rs` 34 passed, 0 failed.

For `f3876a74 test: split blocker proof fixtures`:

- `cargo test -p ploke-db --features call_graph unit::call_graph_fixture_queries::blocker_proof -- --nocapture`
  - passed: `tests/mod.rs` 6 passed, 0 failed.

For `83adea45 test: split proof blocker queries`:

- `cargo test -p ploke-db --features call_graph unit::call_graph_queries::proof_projection::blockers -- --nocapture`
  - passed: `tests/mod.rs` 4 passed, 0 failed.

For `011a4264 test: split associated context fixtures`:

- `cargo test -p ploke-db --features call_graph unit::call_graph_fixture_queries::associated_context -- --nocapture`
  - passed: `tests/mod.rs` 5 passed, 0 failed.

For `7398c064 test: split context expansion query tests`:

- `cargo test -p ploke-db --features call_graph unit::call_graph_queries::context_expansion -- --nocapture`
  - passed: `tests/mod.rs` 4 passed, 0 failed.

For `a4159987 test: split target proof method fixtures`:

- `cargo test -p ploke-db --features call_graph unit::call_graph_fixture_queries::target_proof -- --nocapture`
  - passed: `tests/mod.rs` 9 passed, 0 failed.

For `06daefb6 test: split call graph lookup helpers`:

- `cargo test -p ploke-db --features call_graph unit::call_graph_fixture_queries -- --nocapture`
  - passed: `tests/mod.rs` 94 passed, 0 failed.

For `72c56f6f test: split call graph proof helpers`:

- `cargo test -p ploke-db --features call_graph unit::call_graph_fixture_queries -- --nocapture`
  - passed: `tests/mod.rs` 94 passed, 0 failed.

For `5fad4c98 Remove dormant call graph semantic storage`:

- `cargo test -p syn_parser --features call_graph call_sites`
  - passed: `tests/mod.rs` 206 passed, 0 failed.
- `cargo test -p ploke-transform --features call_graph transform::tests`
  - passed: 5 passed, 0 failed.
