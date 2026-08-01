# 2026-05-02 Agent A Type-Resolution Test Review

- Date: 2026-05-02
- Task title: Type-use resolution test review
- Task description: Independent review of the current parser type-use/type-resolution test work around late type-use resolution and import backlink behavior.
- Related planning files: `docs/active/agents/readme.md`, `docs/active/agents/2026-05-02_tt-expr-core_type-resolution-test-review/README.md`

## Scope Reviewed

Reviewed the new parser-side type-use tests and harness:

- `crates/ingest/syn_parser/tests/uuid_phase3_resolution/type_use_resolution.rs`
- `crates/ingest/syn_parser/tests/common/type_use_resolution.rs`
- `crates/ingest/syn_parser/tests/common/macro_rule_tests.rs`
- `crates/ingest/syn_parser/src/resolve/type_resolution.rs`
- `crates/ingest/syn_parser/src/parser/graph/mod.rs`
- `crates/ingest/syn_parser/tests/uuid_phase3_resolution/backlink_imports.rs`
- Relevant fixture syntax under `tests/fixture_crates/fixture_types`, `tests/fixture_crates/fixture_nodes`, and `tests/fixture_crates/fixture_conflation`
- The nearby transform persistence boundary in `crates/ingest/ploke-transform/src/schema/edges.rs` and `crates/ingest/ploke-transform/src/transform/mod.rs`, because the intended path includes emitted semantic edge rows.

## What Is Actually Tested

The new fixture-backed tests assert 21 selected positive item-backed type-use resolutions. Each generated test calls `assert_type_use_resolution_once` through `type_use_resolution_case!` (`crates/ingest/syn_parser/tests/common/macro_rule_tests.rs:770`).

The harness resolves an exact owner selector and target selector, derives the expected source slot root `TypeId`, then filters `TypeResolutionReport.resolutions` for exactly one row matching owner, role, item target, and a source type contained under the selected slot (`crates/ingest/syn_parser/tests/common/type_use_resolution.rs:165`). It also asserts `resolved_type_id` is `Some(TypeId::Resolved(_))` when requested (`crates/ingest/syn_parser/tests/common/type_use_resolution.rs:201`).

Positive cases currently cover:

- Imported function parameter and return aliases in `fixture_types`, including duplicate function names in a nested module (`type_use_resolution.rs:35`, `type_use_resolution.rs:74`, fixture syntax at `tests/fixture_crates/fixture_types/src/func/return_types.rs:3`).
- Impl self and impl trait type uses, including imports in an inner module (`type_use_resolution.rs:108`, `type_use_resolution.rs:121`, fixture syntax at `tests/fixture_crates/fixture_nodes/src/impls.rs:146`).
- `Self` in an inherent impl method return (`type_use_resolution.rs:138`, fixture syntax at `tests/fixture_crates/fixture_nodes/src/impls.rs:35`).
- Trait supertrait edges, including multiple supertraits and one generic supertrait root (`type_use_resolution.rs:180`, `type_use_resolution.rs:193`, `type_use_resolution.rs:219`, fixture syntax at `tests/fixture_crates/fixture_nodes/src/traits.rs:64`).
- Type alias target edges for local aliases and private nested module paths (`type_use_resolution.rs:232`, `type_use_resolution.rs:245`, `type_use_resolution.rs:258`, fixture syntax at `tests/fixture_crates/fixture_nodes/src/type_alias.rs:29`).
- Const type edges for a struct and an alias (`type_use_resolution.rs:275`, `type_use_resolution.rs:288`, fixture syntax at `tests/fixture_crates/fixture_nodes/src/const_static.rs:39`).
- Two conflation fixture cases for imported top-level trait in an impl and `Self` return in a generic nested impl (`type_use_resolution.rs:301`, `type_use_resolution.rs:318`, fixture syntax at `tests/fixture_crates/fixture_conflation/src/lib.rs:140`).

The resolver being exercised walks function params/returns, struct/enum/union fields, aliases, supertraits, impl self/trait, consts/statics, and method params/returns (`crates/ingest/syn_parser/src/resolve/type_resolution.rs:1100`). It recursively descends `TypeNode.related_types`, emitting a report row only for `TypeKind::Named` and `TypeKind::TraitBound` (`type_resolution.rs:1266`).

## What Is Not Tested

Field roles are implemented and harness-supported but not asserted by any current case: struct fields, tuple struct fields, enum variant fields, and union fields are all untested despite fixture coverage (`type_resolution.rs:1123`, `type_resolution.rs:1137`, `type_resolution.rs:1162`; fixture examples at `tests/fixture_crates/fixture_nodes/src/structs.rs:4`, `tests/fixture_crates/fixture_nodes/src/enums.rs:89`, `tests/fixture_crates/fixture_nodes/src/unions.rs:1`).

Method parameters and static item types are implemented and harness-supported but absent from the current test cases (`type_resolution.rs:1210`, `type_resolution.rs:1232`; harness slots at `type_use_resolution.rs:63`, `type_use_resolution.rs:71`). This leaves `TypeUseRole::MethodParam` and `TypeUseRole::Static` without fixture-backed regression coverage.

Generic parameter resolution is present in `LateResolver::resolve_generic_param` (`type_resolution.rs:939`) but not directly tested through the new harness. Existing fixtures contain many type parameter uses, such as `generic_func<T>` and `GenericSuperTrait<T>` (`tests/fixture_crates/fixture_types/src/func/return_types.rs:9`, `tests/fixture_crates/fixture_nodes/src/traits.rs:75`), but the current cases assert only item-backed targets and never `Target::GenericParam`.

Negative states are not tested: no `UnresolvedReason`, `AmbiguousReason`, primitive/builtin non-promotion, external dependency handling, unsupported qualified-self, or not-type-target case is asserted, even though these are first-class report states (`type_resolution.rs:115`, `type_resolution.rs:149`, `type_resolution.rs:476`).

Backlink dependency is indirectly exercised by imported type names, but the tests do not assert that removing or breaking `ImportedBy` relations breaks type resolution. Existing backlink tests prove exact import backlinks separately (`backlink_imports.rs:41`, `relation_paranoid.rs:270`), but there is no coupled test that demonstrates late type resolution is actually using those backlinks rather than succeeding through some other visible binding path.

Downstream semantic edge persistence is barely tested. `resolved_type_use` rows are inserted in transform (`ploke-transform/src/schema/edges.rs:112`, `ploke-transform/src/transform/edges.rs:106`), but the transform test only checks that some `method_return` rows exist (`ploke-transform/src/transform/mod.rs:241`). It does not check exact owner/type/target/role rows or resolved `TypeId`.

## Correctness/Fidelity Critique

The core parser harness is reasonably strict for positive item-backed cases. It rejects zero matches and duplicate matching report rows, and it selects owners/targets by module path, item name, and `ItemKind` rather than name alone (`tests/common/type_use_resolution.rs:170`, `tests/common/resolution.rs:168`).

The slot filter is useful but not fully exact. It accepts any emitted row whose `source_type_id` is contained anywhere under the selected slot root (`tests/common/type_use_resolution.rs:181`, `tests/common/type_use_resolution.rs:541`). That is correct for composite types like `Option<Point>` where the named occurrence is nested, but it means the test does not assert which nested occurrence inside the slot matched. In composite types with repeated or multiple same-target nested uses, the harness can only enforce uniqueness by `(owner, role, target, slot-tree)` and cannot distinguish the intended child path.

The current assertions validate report rows, not mutation of the original owner slot from `TypeId::Synthetic` to `TypeId::Resolved`. In the current implementation, `resolve_type_uses_after_tree` reports `resolved_type_id` but does not rewrite the source graph type slot (`type_resolution.rs:1322`). If the intended semantic path literally requires replacing owner slots from synthetic IDs to resolved IDs, these tests do not prove it.

The tests do not verify `resolved_ref.provenance` beyond `item_target()`. They do not assert `PathForm`, original path segments, containing module, or owner provenance (`type_resolution.rs:167`). A resolver could produce the right target but lose diagnostic fidelity, and these cases would still pass.

Trait target strictness is partly covered through `TypeUseRole::ImplTrait` and `TypeUseRole::TraitSuper`, because those roles call trait-specific resolution (`type_resolution.rs:276`, `type_resolution.rs:1293`). There is no negative case proving an ordinary type with the same name as a trait, or vice versa, is rejected as `NotTypeTarget`.

## False Positive/Negative Risks

False positive: A resolver that correctly resolves only the selected positive paths can pass all new tests while still missing implemented roles. `TypeUseWalker` contains branches for fields, statics, and method parameters, but no current test exercises those role branches (`type_resolution.rs:1123`, `type_resolution.rs:1210`, `type_resolution.rs:1232`).

False positive: A resolver that emits additional wrong rows for the same owner and role can pass if the extra rows have different targets or come from different nested source type IDs. The harness enforces exactly one matching row, not exact complete row set for an owner/role or for the whole fixture report.

False positive: A resolver that resolves imported names through local contained items rather than through import backlinks could still pass for cases where the final visible item is also reachable by direct module containment or by a different import chain. The tests assert output, not the dependency on `ImportedBy`.

False negative: For legitimate composite type uses with more than one occurrence resolving to the same target under the same source slot, `assert_type_use_resolution_once` would fail even if emitting multiple semantic uses is the desired behavior. This will matter for cases like `(Point, Point)`, `HashMap<Point, Point>`, or repeated trait bounds.

False negative: `resolve_impl` selects impl blocks by matching the root path of `self_type` and optional `trait_type` (`tests/common/type_use_resolution.rs:317`). This is concise, but it may become ambiguous for multiple impls of the same root type with different generic arguments or where-clauses. The helper would fail before evaluating report correctness.

False positive: `expect_resolved_type_id` only checks `Some(TypeId::Resolved(_))`, not that the generated resolved ID matches the canonical target file/path/cfg identity. A wrong resolved ID with the right variant shape would satisfy the assertion (`tests/common/type_use_resolution.rs:201`).

## Fixture-Driven Edge Cases

`fixture_nodes` already contains untested field targets: `SampleStruct.field: String`, tuple struct fields, enum tuple/struct variant fields, union fields, and aliases to nested private modules (`tests/fixture_crates/fixture_nodes/src/structs.rs:4`, `tests/fixture_crates/fixture_nodes/src/enums.rs:89`, `tests/fixture_crates/fixture_nodes/src/unions.rs:1`, `tests/fixture_crates/fixture_nodes/src/type_alias.rs:38`).

`fixture_nodes::traits` contains good generic and `Self` material not yet asserted by the new harness: `GenericSuperTrait<T>: GenericTrait<T>`, `SelfUsageTrait` method signatures, and `SelfInAssocBound::get_related -> Self::Related` (`tests/fixture_crates/fixture_nodes/src/traits.rs:75`, `tests/fixture_crates/fixture_nodes/src/traits.rs:115`, `tests/fixture_crates/fixture_nodes/src/traits.rs:123`). These can expose gaps around generic params, trait method owners, and qualified associated types.

`fixture_nodes::imports` contains renamed imports, grouped imports, globs, multi-hop re-exports, cfg-gated imports, and restricted visibility imports (`tests/fixture_crates/fixture_nodes/src/imports.rs:13`, `tests/fixture_crates/fixture_nodes/src/imports.rs:24`, `tests/fixture_crates/fixture_nodes/src/imports.rs:44`, `tests/fixture_crates/fixture_nodes/src/imports.rs:47`, `tests/fixture_crates/fixture_nodes/src/imports.rs:91`). Current type-use tests cover only a narrow subset of this import surface.

`fixture_conflation` has richer generic/type conflation cases than the two asserted tests: generic impl self types, associated types, trait method returns, top-level aliases, enum fields, and nested aliases (`tests/fixture_crates/fixture_conflation/src/lib.rs:95`, `tests/fixture_crates/fixture_conflation/src/lib.rs:106`, `tests/fixture_crates/fixture_conflation/src/lib.rs:124`, `tests/fixture_crates/fixture_conflation/src/lib.rs:127`). These are appropriate for TDD cases because the fixture comments already describe intended type-use slots.

The fixture set includes builtins and external paths throughout (`std::fmt::Debug`, `Vec<T>`, `String`, primitives). These should be asserted as unresolved/external/builtin where appropriate so the resolver does not silently promote unsupported or external targets.

## TDD Next Cases

Add one strict positive case for every currently unasserted role: `Field`, `MethodParam`, and `Static`. Use existing fixture syntax before adding new fixtures.

Add generic-param report assertions for function params/returns and generic trait supertraits. The harness likely needs a `GenericParam` target selector or a separate assertion helper because `item_target()` intentionally excludes generic targets (`type_resolution.rs:291`).

Add negative report assertions for builtin primitives, external dependencies, not-type-target, unresolved private/invisible paths if visibility is intended, and ambiguity. These should assert concrete `UnresolvedReason` or `AmbiguousReason`, not just absence of item-backed rows.

Add import-backlink-coupled cases: renamed import, glob import, multi-hop re-export, and restricted visibility import where the only valid path depends on `ImportedBy`. Prefer a test shape that proves the resolved target is reached through the import node or at least uses a fixture path with no direct local definition fallback.

Add composite-slot cases that force recursive traversal: `Option<Point>`, `&Point`, `fn(Point) -> MathOperation`, tuple aliases containing repeated named types, and dyn/impl trait bounds. Decide whether repeated named occurrences should produce one row per occurrence; then encode that explicitly.

Add exact persistence tests for `resolved_type_use`: query by known owner/target/role/type ID, assert one row, assert `resolved_type_id` is non-null for item-backed targets, and assert generic/unresolved rows are either intentionally omitted or stored by a separate design.

## Verification Performed

Command run:

```sh
cargo test -p syn_parser --test mod uuid_phase3_resolution::type_use_resolution -- --nocapture
```

Result: passed. The run executed 21 type-use resolution tests, all passed, with 373 unrelated tests filtered out. The build emitted existing unused/dead-code warnings in `syn_parser` and test helpers, but no test failures.

I did not modify production or test code. I did not run the full workspace suite or transform crate tests.

## Recommended Improvements

Keep the current positive harness, but add an exact-report helper that can assert all rows for an owner/role or all rows for a fixture. This would catch extra wrong rows and make recursive/composite behavior intentional.

Extend the target selector model to cover `Target::GenericParam`, `State::Unresolved`, and `State::Ambiguous`. The resolver models these states explicitly; tests should preserve that contract rather than only checking `item_target()`.

Assert canonical `resolved_type_id` identity, not only `TypeId::Resolved(_)`. At minimum, compare against `TypeId::generate_resolved` from the target item's defining file, canonical path, and cfgs, mirroring `promote_resolved_type_id` (`type_resolution.rs:441`).

Add parser-to-transform parity tests for `resolved_type_use` rows. The parser harness proves the report surface; the transform layer should prove those exact semantic edges persist without role/owner/target loss.

Document the intended semantics for repeated nested type occurrences. The current `assert_type_use_resolution_once` shape implies one semantic edge per owner/role/target/slot tree, while the resolver traversal appears capable of one row per named occurrence. That decision should be explicit before adding composite cases.
