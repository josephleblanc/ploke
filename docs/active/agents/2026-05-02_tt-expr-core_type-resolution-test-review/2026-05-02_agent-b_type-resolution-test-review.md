# 2026-05-02 Agent B Type-Resolution Test Review

- Date: 2026-05-02
- Task title: Type-use resolution test review
- Task description: Independent review of the new parser type-use/type-resolution tests and harness, with focus on correctness, coverage, strictness, false-positive/negative risk, fixture edge cases, and next TDD cases.
- Related planning files: `docs/active/agents/readme.md`, `docs/active/agents/2026-05-02_tt-expr-core_type-resolution-test-review/README.md`

## Scope Reviewed

Reviewed the new type-use test surface in `crates/ingest/syn_parser/tests/uuid_phase3_resolution/type_use_resolution.rs`, the shared harness in `crates/ingest/syn_parser/tests/common/type_use_resolution.rs`, the `type_use_resolution_case!` macro in `crates/ingest/syn_parser/tests/common/macro_rule_tests.rs`, the late resolver/walker in `crates/ingest/syn_parser/src/resolve/type_resolution.rs`, backlink tests in `crates/ingest/syn_parser/tests/uuid_phase3_resolution/backlink_imports.rs`, and fixtures under `tests/fixture_crates/{fixture_types,fixture_nodes,fixture_conflation}`. I also checked the downstream persistence seam in `crates/ingest/ploke-transform/src/{schema/edges.rs,transform/edges.rs,transform/mod.rs}` because the intended behavior includes semantic edge emission.

## What Is Actually Tested

The type-use suite builds three complete post-tree fixtures and calls `resolve_type_uses_after_tree` once per fixture via lazy statics (`type_use_resolution.rs:19-32`). It then asserts 21 exact report rows:

- `fixture_types`: imported `Point` and `MathOperation` in function parameter/return positions, including a nested duplicate module using the same root imports (`type_use_resolution.rs:35-106`; fixture source `tests/fixture_crates/fixture_types/src/func/return_types.rs:3-42`).
- `fixture_nodes`: impl self/trait types, one inherent method `Self` return, local supertraits, type-alias targets, and const types (`type_use_resolution.rs:108-299`).
- `fixture_conflation`: one imported trait impl target and one `Self` method return in a generic/cfg-heavy fixture (`type_use_resolution.rs:301-332`).

The harness resolves concrete owner and target selectors, identifies the source slot, then requires exactly one report row with matching `owner`, `role`, `item_target()`, and `source_type_id` contained in the selected slot's type tree (`type_use_resolution.rs:165-184`). For all current cases it also requires `resolved_type_id` to be `Some(TypeId::Resolved(_))` (`type_use_resolution.rs:200-209`). This is materially stricter than smoke tests and not trivially passing.

The backlink tests separately assert exact `ImportedBy` definition-to-import relations for representative imports, including renamed imports, cfg aliases, globs, and module aliases (`backlink_imports.rs:41-67`, `backlink_imports.rs:69-228`). The type-use tests therefore exercise import-aware resolution indirectly: if relevant `ImportedBy` links break, imported type-use rows should disappear or point elsewhere.

## What Is Not Tested

- No `TypeUseRole::Field`, `MethodParam`, or `Static` cases are asserted, even though the walker collects them (`type_resolution.rs:1120-1170`, `type_resolution.rs:1210-1219`, `type_resolution.rs:1232-1239`) and the harness already has selectors for struct/union/enum fields and static source slots (`type_use_resolution.rs:41-72`).
- No union field or enum variant field type-use case is present, despite fixture coverage for unions/enums.
- No negative report rows are asserted: unresolved external/builtin/generic-param/ambiguous cases are not pinned, even though `TypeResolutionReport` records all states and a summary (`type_resolution.rs:281-317`, `type_resolution.rs:1066-1082`).
- No fixture-wide totals are checked by role, target kind, or resolution state. Extra incorrect rows can be emitted without failing these tests unless they duplicate one tested owner/role/target/source combination.
- No test proves downstream `resolved_type_use` rows have exact owner/source/target/resolved IDs. The transform test only checks that some `method_return` rows exist (`transform/mod.rs:244-258`).

## Correctness/Fidelity Critique

The core assertion shape is good: it pins an exact owner, role, source slot, and defining target, and it catches duplicate matching rows. This matches the intended semantic path of resolving structural `TypeId::Synthetic` type occurrences into item-backed semantic targets.

The main fidelity gap is that `resolved_type_id` is only checked by variant, not identity. A wrong `TypeId::Resolved` generated from the wrong path/cfg/file could pass as long as `resolved_ref` points to the expected item (`type_use_resolution.rs:200-209`). Since downstream schema persists both `target_id` and `resolved_type_id` (`schema/edges.rs:123-153`), tests should assert the promoted ID equals the canonical target-derived `TypeId`, or query the persisted row for exact equality.

The `Self` case checks target identity but not `Target::SelfType` state. `item_target()` deliberately accepts both `Target::Item` and `Target::SelfType` (`type_resolution.rs:291-300`), so a regression that stops distinguishing explicit `Self` from a normal named path could still pass if it resolves to `SimpleStruct`. Add a state/provenance assertion for `Self` returns.

The test suite validates report rows, not semantic edge insertion. Parser-level report tests are appropriate, but the intended “emitting report rows/semantic edges” behavior is only weakly smoke-tested in transform. There is no exact persisted edge test for a known fixture row.

## False Positive/Negative Risks

- False positive: `type_tree_contains` accepts any related descendant under the source slot (`type_use_resolution.rs:541-550`). This is useful for nested types, but for root-named cases it can allow a row for a nested related type to satisfy the source slot. Current cases mostly use simple named roots, so the risk is limited now but will grow with generics like `Option<Point>`.
- False positive: `expect_resolved_type_id` does not verify equality to the target's canonical resolved type ID, only `TypeId::Resolved` shape.
- False positive: extra erroneous report rows are ignored unless they duplicate the exact tested combination. A resolver that over-emits unresolved/ambiguous rows, or resolves many unrelated uses incorrectly, can still pass.
- False negative: impl selectors use raw type-root path matching (`type_use_resolution.rs:317-356`, `type_use_resolution.rs:553-568`). If future fixtures intentionally contain multiple impls with the same raw self/trait path but distinct generics/cfgs, selector ambiguity will fail before the resolver behavior is tested.
- False negative: because tests are full-fixture integration tests, failures can come from fixture discovery, module-tree construction, import backlinks, or type resolution. That is acceptable for regression coverage but less precise for TDD isolation.

## Assertion Strictness

The assertions are not tautological. They use exact count assertions for owner/target selector resolution, impl/method/field selection, and final report row matching (`type_use_resolution.rs:186-198`, `type_use_resolution.rs:231-243`, `type_use_resolution.rs:348-356`). Out-of-bounds source indexes panic rather than silently passing.

The weak points are strictness of identity, not pass/fail shape: `Any` exists as an escape hatch in the harness (`type_use_resolution.rs:60-72`, `type_use_resolution.rs:392-393`) but is not used by current cases; `resolved_type_id` shape is weaker than equality; `item_target()` erases `Item` vs `SelfType`; and no report-wide invariants are asserted.

## Fixture-Driven Edge Cases

- `fixture_types/src/func/return_types.rs` gives a useful imported-type baseline and duplicate nested module coverage, but it does not exercise qualified `crate::Point`, `self::`, `super::`, glob imports, renamed imports, or external unresolved behavior for the same owner kinds.
- `fixture_nodes/src/impls.rs` has many useful untested positions: generic method params/returns, trait impls for primitives, associated type `Self::Output`, private trait impls, and inner-module imports (`tests/fixture_crates/fixture_nodes/src/impls.rs:56-143`).
- `fixture_nodes/src/type_alias.rs` has good alias nesting and private module visibility (`tests/fixture_crates/fixture_nodes/src/type_alias.rs:38-54`), but current tests only cover `IdAlias`, `OuterPoint`, and `UseInner`; they do not cover `UseOuterPoint`, qualified std paths, `dyn std::fmt::Debug`, references, raw pointers, or arrays.
- Existing const/static fixtures cover const alias/struct cases, but statics are mostly builtin/reference types (`tests/fixture_crates/fixture_nodes/src/const_static.rs:10-46`), so a nonprimitive static fixture may be needed for `TypeUseRole::Static`.
- Backlink tests are strong for `fixture_nodes` import relations, but there is no type-use case that directly depends on a glob or multi-hop re-export chain. Resolver unit tests cover some glob/re-export paths, but not through `TypeUseWalker` owner slots.

## TDD Next Cases

- Add first fail-until-impl cases for `Field`, `UnionField`, `EnumVariantField`, `MethodParam`, and `Static` using existing harness selectors where possible. If existing fixtures lack nonprimitive static/field targets, add minimal fixture items rather than weakening assertions.
- Add exact resolved-type-ID assertions: compute the canonical `TypeId::Resolved` for the expected target and compare to `resolution.resolved_type_id`, not just the enum variant.
- Add state/provenance assertions for `Self`: require `State::Resolved(Target::SelfType(_))` and source path `Self` for `fixture_nodes_simple_struct_new_self_return`.
- Add import-chain type-use cases that mirror backlink SPP coverage: renamed import, glob import, and multi-hop `pub use` chain used in a function param or return. This would test `scope_candidates`, `glob_candidates`, and `resolve_binding_terminals` through the walker, not only through direct resolver unit tests.
- Add report-wide summary assertions for selected fixture slices: per-role minimum/exact counts for item-backed rows, and explicit expected unresolved reasons for builtin/external/generic targets. Avoid broad “all resolved” assertions because primitives and std paths intentionally do not promote.
- Add an exact transform persistence test for one known row: query `resolved_type_use` by owner ID, source type ID, role, target ID, and resolved type ID, not only `role: "method_return"` non-empty.

## Verification Performed

Note: I ran the focused tests directly in this review environment; no sub-agent execution interface was available.

- `cargo test -p syn_parser uuid_phase3_resolution::type_use_resolution -- --nocapture`: passed. `21 passed; 0 failed; 373 filtered out` in `tests/mod.rs`; warnings only.
- `cargo test -p syn_parser uuid_phase3_resolution::backlink_imports -- --nocapture`: passed. The filter also selected related SPP backlink tests; result was `67 passed; 0 failed; 4 ignored; 323 filtered out`, including expected `should panic` canaries.
- `cargo test -p ploke-transform transform::tests::test_insert_all -- --nocapture`: passed. `1 passed; 0 failed; 33 filtered out`.

## Recommended Improvements

1. Tighten identity checks first: exact `resolved_type_id`, exact `resolved_ref.state` for `Self`, and exact persisted `resolved_type_use` row for one fixture case.
2. Fill role coverage next: field, union field, enum variant field, method param, and static. The harness already supports most of this; missing fixture items should be added deliberately.
3. Add walker-level import-chain cases for rename, glob, and multi-hop re-export so the type-use suite covers the same import/backlink complexity the resolver claims to depend on.
4. Add bounded report-wide invariants for fixture slices to catch over-emission and silent unresolved/ambiguous regressions without making builtin/std/generic behavior falsely fail.
