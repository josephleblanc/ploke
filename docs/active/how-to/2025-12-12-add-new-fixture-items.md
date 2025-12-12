• To extend the fixtures and keep test coverage on par with the existing uuid_phase2_partial_graphs
  suite, follow this pattern (the enum file is a good template):

  1. Track coverage in the module‑level docstring
      - Each nodes/<kind>.rs file begins with a Markdown doc comment that lists the fixture files it
        exercises and the specific items already covered. When you add a new fixture item (e.g., a
        union import), update this section to mention the new definition and why it matters (visibility,
        attributes, cfg, etc.).
      - Keep the structure shown in nodes/enums.rs: bullets grouped by scenario (visibility, generics,
        attributes, etc.), ending with conclusions or limitations. This documentation is treated as the
        single place where future agents understand what is and isn’t validated.
  2. Add entries to the EXPECTED_*_DATA map
      - Each file declares a lazy_static! HashMap<&'static str, Expected…Node> (e.g.,
        EXPECTED_ENUMS_DATA). For a new fixture item, add a new m.insert("crate::path::Item",
        ExpectedXXXNode { … }) entry describing the parsed properties we expect (name, visibility,
        docstring, counts, attributes, cfgs, etc.).
      - Only include fields that the Expected…Node struct exposes (those come from the
        derive(ExpectedData) macro on the node type). If we need deeper assertions (e.g., enum variant
        details), either extend the struct via the derive macro or add manual assertions in the test
        body.
  3. Add a matching ParanoidArgs entry
      - In the companion map (EXPECTED_*_ARGS), insert the metadata telling the macro where to find
        the fixture (fixture_nodes, fixture_path_resolution, etc.), the relative_file_path, canonical
        expected_path, and ItemKind. This ensures the macro can load the correct file and confirm the
        module path.
  4. Hook the entry into paranoid_test_fields_and_values!
      - If the test macro already iterates over the entire map, the new entry is exercised automatically.
        If there are multiple macro invocations (e.g., separate tests per grouping), add the new fully-
        qualified name to the relevant invocation.
      - For cfg-dependent items, either gate the ParanoidArgs entry with #[cfg] or leave a commented-out
        section (as done for GenericEnum/CfgEnum) explaining why it’s disabled until fixtures/tests can
        control the feature flags.
  5. Reflect special handling in the docs
      - When an item is disabled or partially validated, note it in the docstring (“Currently skipped
        because …”). This mirrors the commented-out macro invocations so future updates aren’t
        accidental.
