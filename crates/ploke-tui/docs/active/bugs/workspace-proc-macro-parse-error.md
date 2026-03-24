# Workspace parsing fails on proc-macro crates with `[lib]` sections

## Status: **FIXED** ✅

## Summary
- Symptom: `cargo xtask profile-ingest --target .` fails when run on the ploke workspace root
- Error: `TOML parse error: missing field 'path'` for proc-macro crates
- Affected crates: `syn_parser_macros`, `ploke-test-macros`, `derive_test_helpers`, `ploke-db-derive`
- Fix: Made `path` field optional in `LibTarget` with default value of `"src/lib.rs"` (per Cargo spec)

## Fix Details
- **File modified:** `crates/ingest/syn_parser/src/discovery/single_crate.rs`
- **Change:** Added `#[serde(default = "default_lib_path")]` to `LibTarget.path` field
- **Default function:** `default_lib_path()` returns `PathBuf::from("src/lib.rs")`
- **Tests added:** 
  - `test_lib_target_default_path` - verifies `[lib]` without path defaults to `src/lib.rs`
  - `test_lib_target_explicit_path` - verifies explicit path is preserved

## Evidence
```
parse_workspace: Multiple errors occurred:
ComplexDiscovery error: syn_parser_macros on path /home/brasides/code/ploke/proc_macros/syn_parser/syn_parser_macros from source: Failed to parse Cargo.toml at /home/brasides/code/ploke/proc_macros/syn_parser/syn_parser_macros/Cargo.toml: TOML parse error at line 8, column 1
  |
8 | [lib]
  | ^^^^^
missing field `path`
```

### Affected Cargo.toml files
All have `[lib]` sections without explicit `path`:

1. `proc_macros/syn_parser/syn_parser_macros/Cargo.toml` (line 8)
2. `proc_macros/syn_parser/ploke-test-macros/Cargo.toml` (line 6)
3. `proc_macros/syn_parser/derive_test_helpers/Cargo.toml` (line 7)
4. `proc_macros/ploke-db-derive/Cargo.toml` (line 6)

Example from `syn_parser_macros/Cargo.toml`:
```toml
[lib]
proc-macro = true
```

According to Cargo docs, when `path` is omitted, it defaults to `src/lib.rs`. The parser should handle this.

## Root cause
The `syn_parser::discovery` module's TOML parsing requires an explicit `path` field in `[lib]` sections, but proc-macro crates commonly omit this (relying on Cargo's default `src/lib.rs` convention).

The error originates from:
- `syn_parser::discovery::workspace::try_parse_manifest` or related manifest parsing code
- The deserializer expects `path: String` but the field is optional per Cargo spec

## Workaround
Profile individual crates instead of the workspace:
```bash
# Works
cargo xtask profile-ingest --target crates/ingest/syn_parser --stages parse,transform
cargo xtask profile-ingest --target crates/ploke-db --stages parse,transform

# Fails
cargo xtask profile-ingest --target . --stages parse,transform
```

## Fix
Update the manifest parsing in `syn_parser::discovery` to make `path` optional in the `[lib]` section deserializer, defaulting to `"src/lib.rs"` when not present.

## Related code
- `crates/ingest/syn_parser/src/discovery/mod.rs`
- `crates/ingest/syn_parser/src/discovery/workspace.rs` (likely location of `try_parse_manifest`)

## Regression test
- Add a test fixture with a proc-macro crate that has `[lib]` without `path`
- Run workspace discovery on a test workspace containing such a crate
- Assert that parsing succeeds and resolves to `src/lib.rs`
