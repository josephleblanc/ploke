# Current Type Resolution State

- Date: 2026-05-10
- Task title: Current type-resolution restart context
- Task description: Handoff summary for revisiting the `tt-expr-core` type-resolution work after a pause.
- Related planning files: `docs/active/agents/2026-05-10_tt-expr-core_type-resolution-handoff/README.md`, `docs/active/agents/2026-05-02_tt-expr-core_type-resolution-test-review/README.md`, `docs/active/agents/2026-05-05_tt-expr-core_type-resolution-perf/README.md`

## Branch State

Current branch observed: `tt-expr-core`.

Recent type-resolution commit sequence:

- `32c77fae Add late type resolver and structural type IDs`
- `6c5715c8 Add type-use resolution test harness and schema`
- `1b18ff7d Refactor typed IDs and add TypeRelation enum`
- `a57aa79f Add typed type nodes and v2 type resolution`
- `554d5668 Add typed_type_graph feature flag`
- `42d7fad5 Introduce feature-gated type slots in syn_parser`
- `5235381c Replace AnyTypeUseId with OrdinaryTypeUseId`
- `139eeae6 Refactor type resolution v2 with target callbacks`
- `23e78f6d Add typed_type_graph feature and benchmarks`
- `6114a6b6 Add type graph edge transforms and query tests`
- `f2e03059 Add type-context expansion types to ploke-db`
- `42fe564a Add type-context expansion to RAG retrieval`
- `3c63975a Add type context to RAG context parts`

The current uncommitted change at handoff time was the restoration of `crates/ploke-tui/src/tools/request_code_context.rs` from `3c63975a^`. The latest commit deleted that file while `tools/mod.rs` and `llm/manager/mod.rs` still referenced the request-code-context tool. Restoring it made `cargo build --release` pass locally with warnings only.

## Semantic Shape

The original question was framed as resolving `ImportNode` / `ImportNodeId` back to defining items, then using that link to change `TypeId::Synthetic` into `TypeId::Resolved`.

The branch now has a stronger v2 shape under `typed_type_graph`: keep structural type nodes and slots, then emit typed semantic relations from terminal type-use sources to proven targets. That avoids collapsing composite type structure into one rewritten ID.

The durable v2 relations are:

- `type_use`: owner item to root type-use slot.
- `type_contains`: parent structural type node to child structural type node.
- `type_relation`: terminal typed source to resolved target.

This means a query can move from a function to its return type root, through nested structural types, to a resolved struct/type alias/trait/generic-param target, then to related owners that share the same resolved target.

## Cross-Crate Touchpoints

`syn_parser`:

- `crates/ingest/syn_parser/src/resolve/mod.rs` feature-gates legacy `type_resolution` off and v2 `type_resolution_v2` on with `typed_type_graph`.
- `crates/ingest/syn_parser/src/parser/graph/parsed_graph.rs` documents the correct post-tree point: module tree built, path reconciliation done, file-module pruning done, and `ImportedBy` backlinks linked.
- `crates/ingest/syn_parser/src/resolve/type_resolution_v2.rs` resolves ordinary type sources to structs, enums, unions, type aliases, and type generic params; trait sources resolve only to traits.
- `type_resolution_v2` traverses import backlinks and glob imports through `ImportedBy`, and traverses nested type trees through generic args, references, tuples, function types, trait objects, and impl-trait bounds.

`ploke-transform`:

- `crates/ingest/ploke-transform/src/schema/edges.rs` defines `type_use`, `type_contains`, and `type_relation` under `typed_type_graph`.
- `crates/ingest/ploke-transform/src/transform/type_graph.rs` persists owner/root slots and structural containment.
- `crates/ingest/ploke-transform/src/transform/mod.rs` calls `resolve_type_relations_after_tree` before consuming the parsed graph and persists the resulting `type_relation` rows.

`ploke-db`:

- `crates/ploke-db/src/type_graph.rs` exposes typed graph query APIs.
- `type_related_owners` joins `type_use`, `type_contains`, and `type_relation` to find owners related by shared resolved type targets.
- `expand_type_context` wraps lower-level type graph traversals into application-facing context candidates with relation reasons and distances.

`ploke-rag` / `ploke-core` / `ploke-tui`:

- `crates/ploke-core/src/rag_types.rs` adds optional `type_context` metadata to `ContextPart` and `ConciseContext`.
- `crates/ploke-rag/src/core/mod.rs` expands retrieved hits with DB type context before assembling context when `typed_type_graph` is enabled.
- `crates/ploke-tui/Cargo.toml` enables `typed_type_graph` by default and propagates it to `syn_parser`, `ploke-transform`, `ploke-db`, and `ploke-rag`.
- `crates/ploke-tui/src/tools/request_code_context.rs` should remain present unless the surrounding registrations are also deliberately removed or replaced.

## Existing Handoff And Review Docs

- `docs/active/agents/2026-05-02_tt-expr-core_type-resolution-test-review/README.md`
  Index for two independent reviews of the parser type-use/type-relation tests.
- `docs/active/agents/2026-05-02_tt-expr-core_type-resolution-test-review/2026-05-02_agent-a_type-resolution-test-review.md`
  Detailed critique of current legacy report-style tests, false-positive risks, fixture gaps, and TDD next cases.
- `docs/active/agents/2026-05-02_tt-expr-core_type-resolution-test-review/2026-05-02_agent-b_type-resolution-test-review.md`
  Independent review with similar conclusions: add exact identity checks, missing role coverage, import-chain cases, and transform persistence checks.
- `docs/active/agents/2026-05-05_tt-expr-core_type-resolution-perf/2026-05-05_type-resolution-v1-v2-perf.md`
  Performance comparison between legacy resolver and typed v2 resolver. V2 is much faster in resolver-only benches, but corpus runs showed import-chain depth failures on `dtolnay__proc-macro2` and `dtolnay__syn`.

## Current Test Coverage Shape

Legacy report tests live in `crates/ingest/syn_parser/tests/uuid_phase3_resolution/type_use_resolution.rs`. Its file-level comment is an important map of known passing, partial, untested, and failing shapes.

Typed v2 tests live in `crates/ingest/syn_parser/tests/uuid_phase3_resolution/type_relations_v2.rs`. They assert exact typed relation endpoints, including complete composite-slot cases such as:

- `GenericSuperTrait<T>: GenericTrait<T>` resolving both the trait target and nested generic-param argument.
- `NestedGeneric<TopLevelStruct<T>>` resolving both the nested item target and nested generic-param target.
- `fixture_nodes::structs::GenericStruct<T>` field type resolving to the struct's generic parameter.
- `fixture_nodes::enums::JustTypeGeneric<A>::VariantA(A)` resolving to the enum's generic parameter.
- `fixture_nodes::unions::GenericUnion<T>::value: ManuallyDrop<T>` resolving the nested `T` argument through containment.
- `fixture_nodes::impls::GenericTrait<T> for GenericStruct<T>::generic_trait_method(value: T)` resolving the method parameter through the impl's generic parameter.

The v2 helper surface lives in `crates/ingest/syn_parser/tests/common/type_relation_resolution.rs` and is intentionally table-shaped: source owner, slot, terminal selector, and target selector.

DB-level typed graph tests live in `crates/ploke-db/tests/unit/type_graph_queries/`. The real-source backup corpus contracts are in `corpus_contracts.rs` and validate committed backup DBs rather than reparsing the corpora by default.

`corpus_contracts.rs` is now organized as a coverage map instead of a flat corpus-order list:

- `ordinary_reachability`: passing real-corpus contracts for function parameter roots and function return roots.
- `structural_containment`: passing nested field containment, array-element containment, and source-only ignored tuple-return coverage.
- `method_roots`: passing method parameter and method return contracts over real backup fixtures.
- `impl_roots`: passing impl self and local impl trait contracts over real backup fixtures.
- `alias_expansion`: passing type-alias target/root traversal over backup fixtures.
- `const_static_roots`: passing top-level const/static type contracts over real backup fixtures.
- `generic_bounds`: passing generic declaration-bound and generic-param-owned bound-root contracts over regenerated real backup fixtures.
- `qualified_projections`: passing qualified associated type projection contracts over regenerated real backup fixtures.
- `associated_type_bounds`: passing associated type bound contracts over regenerated real backup fixtures.

The file-level doc comment contains the current real-corpus coverage table. It distinguishes passing, source-only ignored, and not-covered capability buckets so future tests can fill the grid instead of accumulating unstructured cases.

Current real-source typed graph backup fixtures:

- `tests/backup_dbs/corpus_semver_type_graph_2026-05-06.sqlite`
- `tests/backup_dbs/corpus_memchr_type_graph_2026-05-06.sqlite`
- `tests/backup_dbs/corpus_generic_array_type_graph_2026-05-10.sqlite`
- `tests/backup_dbs/corpus_chrono_type_graph_2026-05-10.sqlite`

Those fixtures are registered in `crates/test-utils/src/fixture_dbs.rs` and documented in `docs/testing/BACKUP_DB_FIXTURES.md`. They are intended to cover persisted DB traversal over real crates, not just parser fixture rows.

Verification checkpoint from earlier on 2026-05-10:

- `cargo test -p syn_parser --features typed_type_graph type_relations_v2 -- --nocapture`
  Passed via test sub-agent: 28 passed, 0 failed, 368 filtered out.
- `cargo test -p ploke-db --features typed_type_graph type_graph_queries::corpus_contracts -- --nocapture`
  Passed via test sub-agent: 5 passed, 0 failed, 6 ignored, 55 filtered out.

Current corpus-contract status after adding edge-pushing real-corpus tests and reorganizing `corpus_contracts.rs`:

- Existing passing backup contracts remain: `matches_req` function params, `VersionReq.comparators: Vec<Comparator>`, `ConstGenericArray` alias, `memchr_iter` return, and `MappedLocalTime` alias.
- Newly added passing backup contracts cover:
  - `VersionReq::matches(&self, version: &Version)` method parameter traversal to `Version`.
  - `Parsed::to_datetime() -> ParseResult<DateTime<FixedOffset>>` method return traversal to `DateTime` and nested `FixedOffset`.
  - `GenericSequence::generate(...) -> GenericArray<T, N>` method return traversal.
  - `impl GenericSequence<T> for GenericArray<T, N>` self and local trait roots.
  - `WeekdaySet::from_array(days: [Weekday; C])` array element containment.
  - `MIN_DATE: NaiveDate` const type traversal.
  - `D_FMT: &[Item<'static>]` static reference/slice element traversal.
- Generic declaration-bound contracts now pass over regenerated 2026-05-10 typed corpus backups:
  - `GenericArray<T, N: ArrayLength>` reaches `ArrayLength`.
  - `Date<Tz: TimeZone>` reaches `TimeZone`.
  - `DateTime::Tz` generic-param node reaches `TimeZone`.
- Qualified associated type projection contracts now pass over the regenerated `corpus_generic_array_type_graph` backup:
  - `ConstArrayLength = <Const<N> as IntoArrayLength>::ArrayLength` reaches `IntoArrayLength`.
- Associated type bound contracts now pass over the regenerated `corpus_chrono_type_graph` backup:
  - `TimeZone { type Offset: Offset; }` reaches `Offset`.
- `cargo check -p ploke-db --tests --features typed_type_graph` passed after the module split, with existing unrelated warnings.
- Focused associated-type-bound run:
  `cargo test -p ploke-db --features typed_type_graph chrono_backup_timezone_associated_offset_bound_reaches_offset_trait -- --nocapture`
  passed after regenerating `corpus_chrono_type_graph`.

## Known Gaps

- Method-param, method-return, enum-variant-field, union-field, impl-root, const-root, static-root, and array-containment coverage now exists, but it is not exhaustive across all target families or import/re-export shapes.
- Generic declaration bounds such as `struct S<T: Debug>`, type where-clause predicates such as `where T: Trait` and `where Vec<T>: Trait`, qualified associated type projection traits such as `<T as Trait>::Assoc`, and trait associated type bounds such as `type Offset: Offset` are supported for fresh ingestion.
- Type where-clause predicates are modeled as predicate subjects plus trait-position bounds for fresh fixture ingestion; real-corpus backup contracts still need to be added/regenerated for that surface.
- Associated type/const items are still not parsed as precise associated-item owners; trait associated type bounds are currently exposed through the containing trait.
- File-module qualified path resolution has a known blocker in the legacy comments: module declaration to definition traversal needs equivalent treatment to import backlink traversal for module segments.
- Corpus perf notes show typed v2 can hit the import-chain depth limit on large import/re-export chains.
- DB coverage has real-corpus backup contracts for several successful graphRAG traversals, but we still need edge-pushing corpus tests that intentionally expose limits: where predicates over real corpus, trait super roots, deeper re-export chains, external dependency types, union targets, raw pointer/function pointer structural nodes, and impl-surface expansion from resolved self/trait targets.
- RAG integration should get exact assertions that type-context expansion retrieves expected related owners through `type_relation`, not just parser-level relation rows or DB-only reachability.

## Suggested Restart Point

Start from the v2 relation model rather than reviving slot mutation as the main abstraction. The next useful work is to tighten the typed relation surface:

- Add missing role tests in `type_relations_v2.rs`.
- Write real-target corpus tests that push beyond current passing contracts. Prefer strict tests over placeholders: if a capability is not implemented, let the test fail with the concrete missing traversal.
- Use those failing or edge tests to document current limitations directly next to the contract they expose.
- Add import-chain stress cases that mirror backlink coverage: renamed imports, glob imports, and multi-hop re-exports used in actual type slots, especially in real corpus crates.
- Add real-corpus where-clause contracts now that predicate owners and bounds are represented structurally for fresh ingestion.
- Add transform/DB/RAG integration tests for exact `type_use` + `type_contains` + `type_relation` query behavior.
- Investigate the v2 import-chain depth failures before relying on corpus-scale typed context expansion.
