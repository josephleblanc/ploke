# Call Graph Coverage

Date: 2026-08-02

This document inventories the current call-graph coverage surface. It follows
the same purpose as `TYPE_RESOLUTION_COVERAGE.md`: identify the source rows,
fixtures, query layers, proof layers, and known boundaries that are actually
covered by tests.

The detailed source oracle is
[Real Corpus Call-Site Oracle Matrices](../active/agents/call-graph/2026-06-28_real-corpus-call-site-oracle-matrices.md).

## Status Snapshot

| Layer | Primary tests or sources | Current state | Notes |
| --- | --- | --- | --- |
| Parser structural extraction and resolver | `crates/ingest/syn_parser/tests/uuid_phase3_resolution/call_sites.rs` | Strong fixture coverage | Uses the paranoid call-site harness for exact IDs, owner relation, status, and typed relation checks. |
| Transform projection | `crates/ingest/ploke-transform` focused call-graph tests | Implemented | Projects `call_site`, `call_callee_evidence`, `call_site_edge`, `call_relation`, and `call_resolution_status` from typed parser facts. |
| DB fixture queries | `crates/ploke-db/tests/unit/call_graph_fixture_queries/` | Strong fixture and real-corpus coverage | Covers caller/callee context, paths, impact, reach, frontier buckets, proof rows, and real-target matrices. |
| Shared real-corpus call-shape matrix | `crates/test-utils/src/call_shape_matrix.rs` plus DB/RAG adapter tests | Implemented for current rows | Mirrors the type-shape matrix pattern for source-pinned callsite rows. |
| RAG exact APIs | `crates/ploke-rag/src/core/unit_tests/tests/call_context/` and `proof_context/` | Strong exact coverage | Preserves DB call context, paths, impact, reach, effects, targetless rows, and proof context without search ambiguity. |
| TUI/tool payloads | `crates/ploke-tui/tests/integration/` call-graph lookup/edges/path tests | Strong exact coverage | `code_item_lookup`, `code_item_edges`, `code_item_call_path`, `code_private_uncalled`, and `request_code_context` cover selected strict rows. |
| Proof graph | `crates/ploke-db/tests/proof_graph_store*` and proof-context tests | Substantial | Includes resolved call projection, blockers, external summaries, expansion boundaries, reachable effects, entrypoint summaries, and dependency-root proof rows. |
| Fixture regeneration | `cargo xtask fixtures regenerate --active`; `cargo xtask verify-backup-dbs` | Four active call-graph seeds passed scoped strict verification on 2026-08-02 | `call_graph` is only a compatibility feature alias; backup DBs remain schema-coupled fixtures and should be regenerated instead of loosening imports. |

## Fixture Inventory

| Fixture | Source | Used for | Current state |
| --- | --- | --- | --- |
| `fixture_call_graph` | `tests/fixture_crates/fixture_call_graph` | Parser, transform, DB, RAG, and TUI fixture-backed call-shape contracts | Main artificial fixture for exact supported and fail-closed callsite shapes. |
| `fixture_nodes` and related parser fixtures | `tests/fixture_crates/fixture_nodes`, `fixture_path_resolution`, `fixture_edge_cases`, `fixture_generics`, `fixture_type_resolution_v2` | Structural and resolver edge cases outside the dedicated call-graph fixture | Covered by parser call-site rows and selected DB/RAG/TUI propagation tests. |
| `corpus_axum_call_graph_2026-07-17.sqlite` | `github:tokio-rs/axum@a3446d68bc03d61fb8e7513052bad2825d0c0db1` | Main real-corpus DB/RAG/TUI call graph matrix | Covers functions, methods, imports, re-exports, generated frontiers, proc macros, trait bodies, local items, proof facts, targetless dynamic self-field callees, and usage questions. |
| `corpus_chrono_call_graph_2026-07-17.sqlite` | `github:chronotope/chrono@120686c82c5da90377e815edb82c9a80b6b4f2be` | Alias constructors, try receivers, external slice receiver frontiers | Covered by real-corpus fallback tests. |
| `corpus_memchr_call_graph_2026-07-15.sqlite` | `github:BurntSushi/memchr@24f5daa5257e00e87007c936761600e034827905` | Dynamic field/function-pointer fallback rows, associated-constructor receiver chains, and callable trait-object blockers | Covered by fallback DB/RAG/TUI targetless tests and shared call-shape matrix rows. |
| `corpus_generic_array_call_graph_2026-07-15.sqlite` | `github:fizyk20/generic-array@80bab87431c2e29823dc551a3311324812838a23` | Guarded match-arm method-result receiver rows | Covered by DB fallback tests. |

## Parser Fixture Coverage

| Family | Representative source shapes | Property tested | Current state |
| --- | --- | --- | --- |
| Path calls | `callee()`, `crate::m::callee()`, `self::m::callee()`, `super::callee()` | Structural path rows and exact local function resolution when proven | Covered |
| Method calls | `self.method()`, typed/local/initialized/borrowed/dereferenced receiver methods | Structural receiver payload and exact local method or trait-impl target when proven | Covered for bounded local proof shapes |
| Associated functions | `Self::make()`, `Type::make()`, trait associated function paths, imported aliases | Associated-function relation to exact method target | Covered for visible local proof shapes |
| Constructors | Tuple struct and tuple enum variant constructor syntax, including module-qualified local tuple constructors such as `private::ServeFuture(...)` | Refined constructor relation only for proven callable constructors | Covered |
| Dynamic calls | Parenthesized paths, initialized callable bindings, branch/match callees, callable fields, closure literals, returned callables, bounded private callable forwarding | Exact dynamic function or closure edges only with finite proof; ambiguous candidates preserved without resolved edge | Broad partial coverage |
| Closures and local items | Closure bodies, async closures, async blocks, local const/static/fn/impl method bodies | Nested calls are owned by executable owners and not flattened into enclosing functions | Covered for current fixture and selected axum rows |
| Macro calls | Expression, statement, imported, crate-qualified, item macro invocations | Macro callsites persist as structural unsupported rows unless expanded source is parsed | Covered structurally |
| External/frontier rows | `std`, dependency roots, extern C, external receiver methods | External rows remain targetless unless proof-summary semantics explicitly admit a summary | Covered as fail-closed/frontier rows |

## DB Query Coverage

| Query surface | Representative tests | Property tested | Current state |
| --- | --- | --- | --- |
| Owner context | `call_context_for_owner` fixture and real-corpus tests | Outgoing callsites preserve owner, kind, callee shape, status, targets, and receiver metadata | Covered |
| Target callers | `callers_for_target`, target proof tests | Incoming caller rows and direct callsite identities are target-centered | Covered |
| Paths | `call_paths_from_owner`, `call_paths_between` | Multi-hop traversal uses resolved local edges only | Covered |
| Impact | `call_impact_for_target` usage-question tests | Direct/eventual callers, public/test buckets, source files/modules/crates/cfgs | Covered |
| Reach | `call_reach_for_owner` usage-question tests | Reachable callees plus external/unsupported/unresolved/ambiguous frontier buckets | Covered |
| Module-boundary policy | `module_boundary_policy_violations_from_owner` usage-question tests and `code_item_boundary_policy` integration tests | Caller-supplied forbidden module-prefix rules are evaluated over resolved-only boundary edges | Covered for DB/RAG/TUI exact APIs |
| Effects | `call_effects_reachable_from_owner` | Annotated effect seeds are reachable through resolved paths to the effect owner without fabricating frontier edges | Covered |
| Private zero-incoming | `private_uncalled_nodes` | Stored source graph can identify private nodes with no incoming persisted source callers | Covered |
| Proof context | `proof_graphrag_context`, `proof_symbol_lookup`, `proof_blockers` | Proof rows and blockers explain admitted, blocked, summarized, generated, dependency-root, and targetless facts | Covered for current proof kinds |

## Real-Corpus Matrix Coverage

| Bucket | Real corpus | Current state |
| --- | --- | --- |
| Regular free functions | axum | One-hop and multi-hop function chains are covered, including `from_request::expand -> impl_struct_by_extracting_each_field -> extract_fields`. |
| Method and trait-method traversal | axum | Covered for representative multi-hop paths and exact local/external-trait receiver proof shapes. |
| Import/re-export/glob paths | axum, chrono | Covered for selected source-visible imports, re-exports, glob imports, aliases, module-qualified local constructor paths, and generated-frontier fail-closed rows. |
| Generated and macro frontiers | axum | Generated constructor and generated handler frontiers remain targetless; proof summaries explain generated boundaries without adding local edges. |
| Proc-macro body owners | axum-macros | Public proc-macro entrypoints traverse to local helpers such as `expand_with` and `expand_attr_with`. |
| Local executable owners | axum | Closure, async-block, local const/static/fn, and local impl method owners are exact-addressable where persisted. |
| External frontiers | axum, chrono, local fixture | External dependency, std-root, extern C, and external receiver rows stay targetless unless explicitly summarized. |
| Dynamic/callable fallbacks | axum, memchr | Callable fields, callback parameters, boxed dyn callable rows, and arbitrary expression dynamic callees remain visible blockers unless exact local proof exists. Fixture coverage includes complete private callable-parameter and named holder-field forwarding through two explicit helper calls, returned private callable parameters, and conflicting caller targets kept ambiguous and edge-free. |
| Source metadata and usage questions | axum | Source files/modules/crates/cfgs, public/test buckets, architecture boundaries, dead-code, and effect seeds are covered by usage-question tests. |

## RAG and TUI Coverage

| Surface | Representative coverage | Current state |
| --- | --- | --- |
| RAG exact call context | Fixture and real-corpus call-context tests | Preserves owner/target context and targetless rows without search ambiguity. |
| RAG exact paths, impact, reach, effects, module-boundary policy | Real-corpus exact API tests | Mirrors DB query results with typed RAG payload structs. |
| RAG proof context | Proof-context fixture and real-corpus tests | Preserves call proof rows, blockers, summaries, dependency-root rows, effect metadata, and targetless proof payloads. |
| `code_item_lookup` | Integration tests under `crates/ploke-tui/tests/integration` | Exposes exact call impact, reach, effects, proof context, and callsite rows for strict selectors. |
| `code_item_edges` | Integration tests under `crates/ploke-tui/tests/integration` | Mirrors lookup summaries and exact edge/path payloads, including targetless and proof rows. |
| `code_item_call_path` | Integration tests | Returns strict multi-hop call paths for selected real-corpus and fixture rows. |
| `code_item_boundary_policy` | Integration tests | Reports caller-supplied module-boundary policy violations over resolved-only boundary edges. |
| `request_code_context` | Fixture-backed exact tests | Preserves selected call context/proof payloads; search-seeded live/provider behavior is not used as authoritative proof. |

## Known Boundaries

These are intentionally not claimed as solved:

- Broad Rust method resolution, autoderef/autoref, and complete trait selection.
- Arbitrary interprocedural callable value flow beyond bounded complete-private-caller forwarding.
- Callable trait-object dispatch without exact initializer or complete private-caller proof.
- General async poll/resume, returned futures, and non-immediate future value flow.
- Macro-expanded generated source bodies as normal parsed call graph nodes.
- External dependency traversal without proof-authoritative summaries.
- Source/sink, cost, layer, CI, and policy inference beyond explicit proof facts and usage-query carriers.
- Search-seeded or live-provider TUI flows as proof of graph correctness.

## Current Verification Commands

Run the registry-backed fixture checks and the focused layer matrices before
claiming call-graph coverage:

- `cargo run -p xtask --features call_graph -- fixtures regenerate --active`
- `cargo run -p xtask --features call_graph -- verify-backup-dbs`
- `cargo test -p syn_parser --features call_graph call_sites`
- `cargo test -p ploke-transform --features call_graph transform::call_graph_tests`
- `cargo test -p ploke-db real_target_matrix -- --nocapture`
- `cargo test -p ploke-rag real_corpus -- --nocapture`
- `cargo test -p ploke-tui --test integration real_corpus -- --nocapture`
- `cargo test -p ploke-tui --test integration call_graph_tool_shared_matrix -- --nocapture`
- `cargo test -p ploke-tui --test integration call_graph_tool_targetless_matrix -- --nocapture`

Historical refresh narratives and focused command transcripts are retained in
Git history rather than accumulated in this coverage contract.
