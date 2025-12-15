Backlink Gaps to Target

  - Functions & inherent impls – No regression yet checks that free functions or inherent impl items
    imported via aliasing (fixture_path_resolution or future additions) link back to their ImportNode.
    Same for methods exposed via pub use self::Struct::method; worth adding once fixtures exist.
  - Traits / trait aliases – The fixture still lacks a trait alias, so we cannot assert backlinks for
    that node type. Once a trait alias is added (definition + import), add a regression to ensure the
    relation covers alias definitions, too.
  - Type aliases & generics – SimpleId is imported but not part of the new regression file. We should
    add a case for type aliases, especially generic ones, to prove the relation handles non-struct
    definitions.
  - Extern crates + renamed externs – extern crate serde and extern crate serde as SerdeAlias exist;
    verifying the relation from the extern definition to the import node ensures macro crates or external
    namespaces don’t regress.
  - Modules & nested modules – We only cover the TraitsMod alias. Future tests should cover module
    declarations imported via self/super and a pub(crate) use variant to ensure visibility-adjusted
    backlinks behave correctly.
  - Enum variants – The fixture now imports SampleEnum1::Variant1 as EnumVariant1, but we haven’t added
    a regression test to ensure variant definitions link to variant imports. Variants often behave
    differently in the graph, so a dedicated test helps.
  - Glob imports / self imports – We don’t yet check backlinks for items brought in through std::env::*
    or {self, Item} groupings. Once the relation supports mapping specific glob expansions to import
    nodes, add regression cases (probably low priority until feature ready).
  - CFG / visibility edge cases – No fixture currently has #[cfg] on a use or pub(crate) use …. When
    we add those, we’ll need regressions to ensure the relation is either conditional or documented as
    unsupported (e.g., verifying that an ignored cfg import doesn’t create a relation).

  Edge Cases to Track

  - Renamed re-export chains – fixture_spp_edge_cases has multi-hop pub use … as …. Once the linker
    supports cross-module backlinks along chains, we should write a regression that checks a definition
    in chain_a links to the root-level alias (and maybe intermediate nodes).
  - Glob re-exports – Ensure that when a module glob re-exports a definition, the final alias still
    backlinks to the original definition; may need fixture support first.
  - Inline modules / #[path] – Items defined inside inline modules or modules specified via #[path]
    should also link to imports; we don’t yet have a regression around those definitions.
  - Multiple imports of same definition – If two imports reference the same node (e.g., direct +
    alias), confirm the relation either records both or the intended one. Setting up a fixture case and
    regression will clarify expectations.

  In summary, once we extend the fixtures with functions, type aliases, variants, extern crates, cfg/
  visibility cases, and possibly multi-hop re-exports, we should add corresponding backlink tests.
  Covering these scenarios ensures the relation handles every node type and import syntax we rely on.
