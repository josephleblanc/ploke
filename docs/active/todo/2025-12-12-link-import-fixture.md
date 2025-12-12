## Linkable item coverage checklist (2025-12-12)

The following checklist tracks which Rust item kinds and import syntaxes already have usable fixtures for definition↔import backlink tests. Each checked entry lists at least one concrete fixture reference; unchecked entries still need a fixture import before we can write a regression.

### Definition targets
- [x] Named struct – `tests/fixture_crates/fixture_nodes/src/imports.rs:13` imports `crate::structs::SampleStruct` as `MySimpleStruct`.
- [x] Tuple struct – `tests/fixture_crates/fixture_nodes/src/imports.rs:7` imports `crate::structs::TupleStruct`.
- [ ] Unit struct – only defined (`tests/fixture_crates/fixture_nodes/src/structs.rs:12`) but never imported.
- [x] Enum type – `tests/fixture_crates/fixture_nodes/src/imports.rs:17-19` import `EnumWithData` and `SampleEnum1`.
- [ ] Enum variant – no `use crate::enums::SampleEnum1::Variant1` style import exists in any fixture.
- [x] Trait – `tests/fixture_crates/fixture_nodes/src/imports.rs:17-20` import `SimpleTrait` and `GenericTrait as MyGenTrait`; `tests/fixture_crates/fixture_impls/src/main.rs:36` also uses `use crate::TestImplStruct` for inherent impl testing.
- [x] Type alias – `tests/fixture_crates/fixture_nodes/src/imports.rs:32` imports `crate::type_alias::SimpleId`.
- [ ] Trait alias – not represented in fixtures (none defined or imported).
- [x] Function (`use`) – `tests/fixture_crates/fixture_path_resolution/src/lib.rs:89` conditionally imports `crate::local_mod::func_using_dep as aliased_func_a` behind `feature_a`.
- [x] Function (`pub use`) – `tests/fixture_crates/fixture_path_resolution/src/lib.rs:93` conditionally re-exports `crate::local_mod::local_func` behind `feature_b`; `tests/fixture_crates/fixture_spp_edge_cases/src/lib.rs:87-109` provide multi-hop `pub use crate::chain_a::item_a` chains without cfg.
- [ ] Const – no fixture imports `crate::const_static::TOP_LEVEL_BOOL` (or similar) even though definitions exist.
- [ ] Static – same gap as const; `tests/fixture_crates/fixture_nodes/src/const_static.rs:20` defines `TOP_LEVEL_COUNTER` but nothing imports it.
- [x] Module definition – `tests/fixture_crates/fixture_path_resolution/src/lib.rs:82` imports `crate::local_mod::nested` as `PrivateNestedAlias`.
- [x] Nested module via `self::` – `tests/fixture_crates/fixture_nodes/src/imports.rs:30` imports `self::sub_imports::SubItem`.
- [x] Module via `super::` – `tests/fixture_crates/fixture_nodes/src/imports.rs:31` imports `super::structs::AttributedStruct`.
- [x] Module via `crate::` – `tests/fixture_crates/fixture_nodes/src/imports.rs:32` imports `crate::type_alias::SimpleId`.
- [x] Module re-export – `tests/fixture_crates/fixture_path_resolution/src/lib.rs:118` re-exports `local_mod::nested` as `reexported_nested_mod`.
- [ ] Union – `tests/fixture_crates/fixture_nodes/src/unions.rs` defines several unions but none are imported anywhere.
- [ ] Macro (`macro_rules!`/proc) – `tests/fixture_crates/fixture_nodes/src/macros.rs` defines macros but nothing `use`s or re-exports them.
- [x] Extern crate – `tests/fixture_crates/fixture_nodes/src/imports.rs:38-39` include `extern crate serde;` and `extern crate serde as SerdeAlias;`.

### Import syntax + scenarios
- [x] Simple `use path::Item` – `tests/fixture_crates/fixture_nodes/src/imports.rs:7` (`TupleStruct`).
- [x] Grouped braces – `tests/fixture_crates/fixture_nodes/src/imports.rs:17-23` combine multiple enums/traits/modules.
- [x] Renamed import (`as`) – `tests/fixture_crates/fixture_nodes/src/imports.rs:13` (`SampleStruct as MySimpleStruct`); `tests/fixture_crates/fixture_path_resolution/src/lib.rs:89` (`func_using_dep as aliased_func_a` under cfg).
- [x] Glob import – `tests/fixture_crates/fixture_nodes/src/imports.rs:27` (`std::env::*` external) and `tests/fixture_crates/fixture_spp_edge_cases/src/lib.rs:141` (`pub use glob_target::*` for local items).
- [x] `self::` path – `tests/fixture_crates/fixture_nodes/src/imports.rs:30`.
- [x] `super::` path – `tests/fixture_crates/fixture_nodes/src/imports.rs:31`.
- [x] `crate::` path – `tests/fixture_crates/fixture_nodes/src/imports.rs:7-33`.
- [x] Absolute `::std::...` – `tests/fixture_crates/fixture_nodes/src/imports.rs:35`.
- [x] `pub use` simple – `tests/fixture_crates/fixture_path_resolution/src/lib.rs:107-130`; `tests/fixture_crates/fixture_spp_edge_cases/src/lib.rs:87-267`.
- [x] `pub use` rename chains – `tests/fixture_crates/fixture_spp_edge_cases/src/lib.rs:211-283`.
- [x] `pub use` with `self`/`super` – `tests/fixture_crates/fixture_spp_edge_cases/src/lib.rs:194-199`.
- [x] `#[cfg]`-gated `use`/`pub use` – `tests/fixture_crates/fixture_path_resolution/src/lib.rs:82-95` (feature_a/feature_b gating).
- [ ] Importing a const/static/union/macro via any syntax – still missing entirely.
- [ ] Importing an enum variant specifically – not yet represented.
- [ ] Importing a `pub mod name as Alias` (module alias via `pub use`); current fixtures only alias modules via private `use` statements.

### Requested fixture review
- `fixture_nodes` – already covers structs, tuple structs, enums, traits, type aliases, nested module imports, rename/group/glob/self/super/crate/absolute, and extern crates. Missing function/const/static/union/macro imports.
- `fixture_path_resolution` – covers module aliasing (`use crate::local_mod::nested as PrivateNestedAlias`), cfg-gated function import/re-export, and module re-exports. Good target for testing cfg-aware backlinks, but still lacks const/static/union/macro usage.
- `fixture_spp_edge_cases` – supplies extensive `pub use crate::...` scenarios (multi-hop renames, glob re-exports, self/super re-exports, deep chains) suitable for re-export backlink tests. These focus on functions/modules only; no new item kinds appear.
- `fixture_spp_edge_cases_no_cfg` – mirrors the previous fixture without cfg gates, so it can back tests that must run without feature flags.
- `file_dir_detection` – (named `file_dir_detection` in the tree) contains module declarations but no actual `use crate::...` statements; therefore it does **not** cover any of the missing item kinds.

### Outstanding gaps / next steps
1. Add fixture imports for unit structs, enum variants, consts, statics, unions, and macros (e.g., extend `tests/fixture_crates/fixture_nodes/src/imports.rs` with `use crate::structs::UnitStruct;`, `use crate::enums::SampleEnum1::Variant1;`, `use crate::const_static::TOP_LEVEL_BOOL;`, `use crate::unions::IntOrFloat;`, `use crate::macros::exported_macro;`).
2. Consider a fixture that `pub use`s a module under a new alias so we can test module alias backlinks (currently only private aliases exist).
3. For cfg-gated imports we already have examples (`fixture_path_resolution`), but we still need non-gated coverage for const/static/macro/union items to keep default tests simple.
4. Once fixtures exist, add regression tests mirroring `expect_backlink_from_definition_to_import_for_sample_struct` for each checked item type plus explicit tests for the new import syntaxes (enum variants, const/static, macros, unions, module aliasing).
