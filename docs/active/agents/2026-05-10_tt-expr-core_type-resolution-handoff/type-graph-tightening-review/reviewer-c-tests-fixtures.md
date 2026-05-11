# Reviewer C: Tests, Fixtures, and Corpus Contracts

- date: 2026-05-10
- task title: Type graph tightening review, reviewer C
- task description: Review recent `typed_type_graph` tests, fixtures, corpus contracts, and known-limitation docs for false positives/negatives and lax API coverage.
- related planning files: [`README.md`](./README.md), [`../README.md`](../README.md), [`../2026-05-10_current-type-resolution-state.md`](../2026-05-10_current-type-resolution-state.md), [`../2026-05-10_current-type-resolution-workflow.md`](../2026-05-10_current-type-resolution-workflow.md)

## Findings

### High: Corpus reachability contracts can pass through the wrong root, slot, or depth

The corpus helper only checks that some reachable path has the requested owner, target, relation kind, and `depth >= min_depth`; it does not assert exact depth, the root type, the terminal type, or the `type_use` role that produced the path. See [`common.rs`](../../../../../crates/ploke-db/tests/unit/type_graph_queries/common.rs#L465-L480). Most corpus contracts use that helper, for example direct-return and direct-alias cases in [`corpus_contracts.rs`](../../../../../crates/ploke-db/tests/unit/type_graph_queries/corpus_contracts.rs#L154-L161), [`corpus_contracts.rs`](../../../../../crates/ploke-db/tests/unit/type_graph_queries/corpus_contracts.rs#L428-L435), and [`corpus_contracts.rs`](../../../../../crates/ploke-db/tests/unit/type_graph_queries/corpus_contracts.rs#L464-L471).

What is actually tested: the DB can reach a particular target somewhere from the owner through `type_use -> type_contains* -> type_relation`.

What is not tested: that a direct root stayed direct, that an expected nested traversal stayed at the intended depth, that the expected syntactic slot/role produced the path, or that no broader traversal added a second accidental path. This is especially important because `type_targets_reachable_from_owner` itself collapses every `type_use` role for an owner into unlabelled roots before joining `type_relation` rows; see [`type_graph.rs`](../../../../../crates/ploke-db/src/type_graph.rs#L321-L349). A regression that drops the intended `MethodReturn`, `ImplSelf`, or `TypeAliasTarget` root but adds another root on the same owner can still satisfy these corpus assertions.

Recommended tightening:

- Add an `assert_owner_reaches_target_exact` test helper for cases with known distance. It should assert `depth == expected_depth` by default, not `>=`.
- Add role/root-aware test helpers at the DB-test layer by first selecting the exact `TypeUseRoot` for the expected role and slot, then asserting the reachable target from that root/terminal. If production APIs intentionally omit role from `TypeTargetPath`, keep this as test-local Cozo queries rather than weakening the API.
- Convert direct corpus cases with stable syntax to exact depth: `memchr_iter -> Memchr` at depth `0`, `ConstGenericArray -> GenericArray` at depth `0`, `MappedLocalTime -> LocalResult` at depth `0`, `MIN_DATE -> NaiveDate` at depth `0`, and `GenericSequence impl -> ImplSelf/ImplTrait` at depth `0`.
- Add negative corpus checks for representative owners: no extra path to the same target at an unexpected depth for direct-root cases, and no ordinary/trait family crossing for trait-position cases.

### High: Parser relation tests assert expected positives, but not invalid extra relations

The typed parser tests are strong on exact endpoint equality for each named relation, but each case only counts one exact expected relation. [`TypeRelationView::assert_once`](../../../../../crates/ingest/syn_parser/tests/common/type_relation_resolution.rs#L309-L324) filters for equality with the expected row; [`assert_present`](../../../../../crates/ingest/syn_parser/tests/common/type_relation_resolution.rs#L329-L336) just repeats that check. The macro layer preserves this positive-only shape in [`macro_rule_tests.rs`](../../../../../crates/ingest/syn_parser/tests/common/macro_rule_tests.rs#L818-L878).

What is actually tested: each listed source/target pair exists exactly once, including typed ordinary-vs-trait endpoint families.

What is not tested: that the same source did not also emit an invalid relation in the other family, that a trait-position source has exactly the expected target set, or that a composite slot did not resolve unrelated terminals. For example, the supertrait and where-clause positives in [`type_relations_v2.rs`](../../../../../crates/ingest/syn_parser/tests/uuid_phase3_resolution/type_relations_v2.rs#L72-L141) would still pass if the resolver also emitted extra ordinary/trait rows for the same source.

Recommended tightening:

- Add a source-scoped assertion helper: resolve the source selector, collect all `TypeRelation` rows with that source ID, and compare the exact set of expected rows.
- Add explicit family-crossing negative cases: no `Ordinary` relation for a root trait source such as `ChildTrait: LocalTrait`; no `Trait` relation for ordinary field/function parameter roots such as `concrete(value: T)`.
- Add composite-source exact-set cases for `Vec<T>`, tuple returns, qualified projections, trait objects, and generic supertraits. The existing `type_relations_present_case!` rows at [`type_relations_v2.rs`](../../../../../crates/ingest/syn_parser/tests/uuid_phase3_resolution/type_relations_v2.rs#L366-L388) and [`type_relations_v2.rs`](../../../../../crates/ingest/syn_parser/tests/uuid_phase3_resolution/type_relations_v2.rs#L420-L441) are a good base, but should reject extra rows for the same slot.

### Medium: Direct-root DB coverage is narrow and leaves many owner/role contracts unpinned

`direct_roots.rs` pins function params, function returns, aliases, generic bounds, associated type bounds, and where predicates. It does not directly test method param/return roots, field roots, impl self/trait roots, trait super roots, const roots, static roots, or corpus-backup root roles. The current module index explicitly lists this as not exhaustive in [`mod.rs`](../../../../../crates/ploke-db/tests/unit/type_graph_queries/mod.rs#L31-L35).

What is actually tested: a small set of direct owner/root rows exists in fresh fixture DBs, and several newer constraint roles exist for `fixture_type_resolution_v2`.

What is not tested: that every role emitted by [`transform/type_graph.rs`](../../../../../crates/ingest/ploke-transform/src/transform/type_graph.rs#L23-L147) is queryable with the correct owner and slot; that real corpus backup DBs carry those same roles; and that reachability success is not coming from the wrong role. Parser relation tests cover many of these roles at the parser resolver layer, and corpus reachability tests cover some downstream effects, but neither replaces direct DB root-role assertions.

Recommended tightening:

- Add one exact direct-root test per `TypeUseRole` variant in [`type_graph.rs`](../../../../../crates/ploke-db/src/type_graph.rs#L19-L37).
- For each role, assert owner ID, root type ID, role, and slot index exactly, not just `.any(...)` on role.
- Add fixture-backed direct-root cases for method param/return, struct/enum/union fields, impl self/trait, trait super, const, and static.
- Add at least one backup-corpus direct-root role test for each newly claimed corpus bucket: method roots, impl roots, const/static roots, generic-param-owned roots, associated type bounds, and qualified projections.

### Medium: Generic-bound and where-predicate owner invariants are only partially pinned

The generic-bound direct-root test finds the first `GenericBound` root for `LocallyBound`, then only checks that the generic parameter owner has the same root as `GenericParamBound`; see [`direct_roots.rs`](../../../../../crates/ploke-db/tests/unit/type_graph_queries/direct_roots.rs#L87-L113). Where-predicate tests similarly assert the presence of subject and bound roles, then check that a direct generic-param subject gets the same bound root on the generic parameter owner; see [`direct_roots.rs`](../../../../../crates/ploke-db/tests/unit/type_graph_queries/direct_roots.rs#L143-L181). Reachability then checks that containing and generic-param owners reach `LocalTrait` at depth `0`; see [`reachability.rs`](../../../../../crates/ploke-db/tests/unit/type_graph_queries/reachability.rs#L128-L174).

What is actually tested: the item owner and generic-param owner can both reach the bound trait, and a composite where subject does not create `WhereGenericParamBound` on the involved generic parameter.

What is not tested: exact owner-role separation for all possible owners, exact subject root identity, exact bound root identity, slot numbering when multiple predicates/bounds exist, and absence of inappropriate roots on the containing item or unrelated generic parameters. A bug that emits the right target through an extra or duplicated role can still satisfy the current positive reachability checks.

Recommended tightening:

- Add multi-bound and multi-predicate fixture cases, such as `struct Multi<T, U> where T: A + B, Vec<U>: C`, and assert exact owner slot order for each subject and bound.
- Add negative owner checks: unrelated generic params must not receive `WhereGenericParamBound`; containing item owners must not receive `GenericParamBound`; generic-param owners must not receive `GenericBound`.
- Add exact subject-root tests for direct `T: Trait` versus composite `Vec<T>: Trait`, using the subject root and terminal ID rather than only the eventual target.
- Add owner-family coverage for function, method, trait, impl, type alias, enum, and union where predicates. Current fresh fixture coverage is mostly struct/type-alias shaped in [`fixture_type_resolution_v2/src/lib.rs`](../../../../../tests/fixture_crates/fixture_type_resolution_v2/src/lib.rs#L37-L47).

### Medium: Backup corpus contracts can hide current parser/transform regressions

The corpus contract file states that source-parse variants are ignored by default and backup variants are the ordinary executable contracts; see [`corpus_contracts.rs`](../../../../../crates/ploke-db/tests/unit/type_graph_queries/corpus_contracts.rs#L13-L15). The source variants are explicitly ignored, for example semver at [`corpus_contracts.rs`](../../../../../crates/ploke-db/tests/unit/type_graph_queries/corpus_contracts.rs#L84-L95), memchr at [`corpus_contracts.rs`](../../../../../crates/ploke-db/tests/unit/type_graph_queries/corpus_contracts.rs#L130-L141), and generic-array alias source parsing at [`corpus_contracts.rs`](../../../../../crates/ploke-db/tests/unit/type_graph_queries/corpus_contracts.rs#L402-L413).

What is actually tested in default runs: committed backup DBs still satisfy selected graph queries.

What is not tested in default runs: that current parser and transform code can still regenerate those rows from real corpus source. This is acceptable for fast contracts, but it can hide degradation in ingestion while backup DBs remain green.

Recommended tightening:

- Keep backup contracts as fast executable tests, but add a scheduled or explicitly named source-ingestion test job for the same corpus assertions.
- Add a small, non-ignored real-corpus smoke fixture or reduced corpus crate that exercises at least one import-heavy path and one where/associated-bound path through fresh ingestion.
- Add metadata assertions around backup fixture versions when a test depends on 2026-05-10 regenerated semantics, so stale backup inputs fail loudly instead of silently preserving old behavior.

### Medium: KL-008 contradicts itself on real-corpus where-clause coverage

KL-008 says real-corpus backup contracts now pass for “type where-clause predicates” in [`KL-008-typed-type-graph-constraint-surfaces.md`](../../../../../docs/design/known_limitations/KL-008-typed-type-graph-constraint-surfaces.md#L12-L18). The same document later says real-corpus backup contracts for where-clause surfaces still need to be added/regenerated in [`KL-008-typed-type-graph-constraint-surfaces.md`](../../../../../docs/design/known_limitations/KL-008-typed-type-graph-constraint-surfaces.md#L38-L44), [`KL-008-typed-type-graph-constraint-surfaces.md`](../../../../../docs/design/known_limitations/KL-008-typed-type-graph-constraint-surfaces.md#L53-L56), and [`KL-008-typed-type-graph-constraint-surfaces.md`](../../../../../docs/design/known_limitations/KL-008-typed-type-graph-constraint-surfaces.md#L115-L117).

What is actually tested: fresh fixture where-clause coverage in parser and DB tests; no real-corpus where-clause backup contract is present in `corpus_contracts.rs`.

What is not tested: the real-corpus where-clause claim. This documentation mismatch can cause future reviewers to treat a missing corpus surface as already green.

Recommended tightening:

- Update KL-008 to state that where-clause predicates are fresh-fixture covered, not corpus-backup covered.
- Add the missing corpus backup where-clause contract before moving that bullet to “passing.”
- Keep the “expected current result” block aligned with executable test names rather than broad capability phrases.

### Low: `fixture_type_resolution_v2` is too top-level and single-crate to stress owner/import degradation

The focused fixture is useful, but it currently uses a small set of top-level declarations: local structs/functions, simple bounds, one qualified projection, and struct/type-alias where predicates in [`fixture_type_resolution_v2/src/lib.rs`](../../../../../tests/fixture_crates/fixture_type_resolution_v2/src/lib.rs#L3-L47). This makes the tests easy to read, but it also means regressions in nested modules, renamed imports, re-exports, `super`/`self`/`crate` paths, file modules, duplicate names across modules, and method/impl where-clause ownership may not be exposed.

Recommended tightening:

- Add a second fixture module inside `fixture_type_resolution_v2` rather than overloading the existing simple cases.
- Include renamed import and shadowing cases where a local generic, local item, imported item, and re-exported item share a terminal name.
- Include where predicates and generic bounds on functions, methods, traits, impls, type aliases, enums, and unions.
- Include one associated type default and one impl associated type definition as red/TDD cases if the parser still lacks precise associated-item owners.

## Strong Tests As-Is

- Parser positive endpoint tests are strong for typed ordinary-vs-trait endpoint equality. The helper builds typed source/target selectors and compares exact `TypeRelation` rows in [`type_relation_resolution.rs`](../../../../../crates/ingest/syn_parser/tests/common/type_relation_resolution.rs#L339-L352), which is substantially better than legacy role/provenance-only assertions.
- The `generic_shadow` fixture case is a strong local shadowing regression: [`fixture_type_resolution_v2/src/lib.rs`](../../../../../tests/fixture_crates/fixture_type_resolution_v2/src/lib.rs#L13-L13) plus [`type_relations_v2.rs`](../../../../../crates/ingest/syn_parser/tests/uuid_phase3_resolution/type_relations_v2.rs#L56-L70) distinguishes a generic type parameter from the top-level `T` struct.
- `composite_where_predicate_subject_is_not_a_generic_param_bound` is a valuable negative test because it rejects the tempting but wrong collapse of `Vec<T>: Trait` into a direct bound on `T`; see [`direct_roots.rs`](../../../../../crates/ploke-db/tests/unit/type_graph_queries/direct_roots.rs#L187-L219).
- Related-owner ranking tests assert exact zero-depth ranking for direct users and positive-depth ranking for nested users in [`related_owners.rs`](../../../../../crates/ploke-db/tests/unit/type_graph_queries/related_owners.rs#L13-L57) and [`related_owners.rs`](../../../../../crates/ploke-db/tests/unit/type_graph_queries/related_owners.rs#L63-L103). These are stronger than most corpus reachability assertions because they check relative ranking, not just presence.
- Structural containment tests assert ordered generic arguments, tuple elements, reference cardinality, function pointer children, and trait object bounds in [`containment.rs`](../../../../../crates/ploke-db/tests/unit/type_graph_queries/containment.rs#L13-L138). They are good low-level contracts and should be kept as-is while adding more node-family coverage.

## Recommended TDD Task List

1. Add exact source-scoped parser relation assertions that reject extra relations for the same source.
2. Add DB direct-root tests covering every `TypeUseRole`, with exact owner/root/role/slot checks.
3. Replace corpus `depth >= min_depth` checks with exact-depth assertions where syntax is stable; add explicit tests for any case where “at least” is intentionally required.
4. Add role-aware DB reachability tests that prove the target came from the intended syntactic slot.
5. Add negative family-crossing tests for ordinary-vs-trait relations at both parser and DB layers.
6. Add multi-bound/multi-predicate where-clause fixtures to verify slot order, owner separation, and no derived generic-param roots for composite subjects.
7. Add default-run coverage that fresh-ingests at least a reduced real-corpus-style crate, while keeping full corpus source tests ignored or scheduled if needed for runtime.
8. Correct KL-008’s corpus where-clause status before relying on it as the planning source for remaining limitation work.

