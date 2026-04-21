# Bugs that are still alive

## Duplicate Relation (Sep 24, 2025)

Detected: Sep 24, 2025

function‑scoped duplicate imports of the same
item in the same module.

- What duplicates: Two identical lines use crate::llm::ProviderSlug as _; appear in the same
file/module, inside different functions:
    - crates/ploke-tui/src/app/commands/exec.rs:270
    - crates/ploke-tui/src/app/commands/exec.rs:404
    - crates/ploke-tui/src/app/commands/exec.rs:404

## Duplicate Relation (Sep 1, 2025)

Detected: Sep 1, 2025

Resolved: Sep 1, 2025

Notes: Added note to add new test in `docs/active/TECH_DEBT.md` with reference to this document.

### Initial detection
The test `test::test::transform_tui` in `ploke-transform` failed while running `cargo test`.

This test failed with a message that there was a duplicate relation detected, providing the following message.
```text
Expected unique relations, found invalid duplicate with error: Duplicate node found for ID AnyNodeId::Import(S:50299b64..4690258f) when only one was expected.
```

### Investigating root cause

Due to the duplicate relation detected, the bug is known to be in `syn_parser`, where the panic occurs.

There was a similar test in `syn_parser`, which could be run with logging (`syn_parser` is still using `log` over `tracing`, needs update), we could run:
```bash
RUST_LOG=dup=trace cargo test -p syn_parser --test mod full::parse_self::new_parse_tui -- --test-threads=1 2>&1 | rg -C 20 "50299b64"
```
This output some helpful information about the nodes from the logging in the parsing process so we could identify the code items in the target files.

#### Initial assessment

A duplicate relation of an import (both `ModuleImports` and `Contains`) was detected for:
```txt
ImportNode {
  id: ImportNodeId(Synthetic(50299b64-dc28-515b-a0e5-890f4690258f)),
  span: (660, 669),
  source_path: ["crate", "tools", "Tool"],
  kind: UseStatement(Inherited),
  visible_name: "_",
  original_name: Some("Tool"),
  is_glob: false,
  is_self_import: false,
  cfgs: []
}
```

I suspect this is an issue that may be with the `_` anonymous name, and might be with cfgs, and might be with both.

#### Follow-up assessment

Upon further investigation, it appears that this is an issue of Id Conflation. There was a second node detected with the same Id:
```
ImportNode { 
  id: ImportNodeId(Synthetic(50299b64-dc28-515b-a0e5-890f4690258f)), 
  span: (95, 112),
  source_path: ["ploke_rag", "TokenCounter"],
  kind: UseStatement(Inherited),
  visible_name: "_",
  original_name: Some("TokenCounter"),
  is_glob: false,
  is_self_import: false,
  cfgs: [] 
}
```

Both of these nodes were in `ploke-tui/llm/mod.rs`, and not nested in any further modules.

### Root cause identified

Looking at the definition of the function that generates the synthetic Ids:
```rust
        pub fn generate_synthetic(
            crate_namespace: uuid::Uuid,
            file_path: &std::path::Path,
            relative_path: &[String],
            item_name: &str,
            item_kind: crate::ItemKind, // Use ItemKind from this crate
            parent_scope_id: Option<NodeId>,
            cfg_bytes: Option<&[u8]>,
        ) -> Self {
```
Each of these is going to be the same, notably the `item_name` will be the same, `"_"`.
- Therefore the v5 hash will be identical from the identical inputs

### Initial Fix Proposed

To resolve this, we will need to add an exception for the handling of the `_` name in the parsing of imports:
- use the `original_name` if the name is `_`
- panic if `original_name` is `None` when `visible_name` is `_`

#### Fix Applied

file: `crates/ingest/syn_parser/src/parser/visitor/code_visitor.rs`
```rust
                let registration_result = if visible_name.as_str() == "_" {
                    self.register_new_node_id(
                        &original_name,
                        ItemKind::Import,
                        cfg_bytes, // Pass down received cfg_bytes
                    )
                } else {
                    self.register_new_node_id(
                        &visible_name,
                        ItemKind::Import,
                        cfg_bytes, // Pass down received cfg_bytes
                    )
                };
```

- No performance impact
- No additional allocation

#### Fix assessment

Result of tests:
```bash
RUST_LOG=trace cargo test -p syn_parser --test mod full::parse_self::new_parse_tui -- --test-threads=1
```
 - passes 

```bash
cargo test -p ploke-transform --lib tests::test_transform_tui
```
- passes

```bash
cargo test
```
- All tests that passed previously still pass, along with the previously failing `tests::test_transform_tui`
- Previously failing tests still fail, unrelated issues local to `ploke-tui`

### Conclusions

Initial project-wide test suite caught an issue because we have been parsing our own code base, this is a good habit and way to provide ongoing testing of the application. We had not created a test for this specific issue, and now are aware of a previously uncaught quirk in parsing Rust.

Bug completely resolved.

### Next Steps

Add more tests to `syn_parser` for this specific case. While our current tests will catch the issue, it remains somewhat opaque and would require additional effort to diagnose the issue should it recur.

Additionally, this was caught by finding a change by parsing our own files, which may change, so there is no guarentee that the problematic pattern will be tested by analyzing our own crates in the future. Adding this to a dedicated fixture, or integrating with a previously existing fixture, will provide ongoing testing to catch potential future regressions.

#### Actions Taken

- Added to `docs/active/TECH_DEBT.md` with reference to this document.

## Issue switching from input to command mode by using the `/` key while in `Insert` mode

### Description

Pressing `/` while in `Insert` mode causes the cursor to flicker to two locations for a moment, one perhaps 4-5 rows down and a similar number of cols to the right. The other location is on the right side of the input box border.

After less than a second the cursor returns to the expected position just after the `/` in the input area.
