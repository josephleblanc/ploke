# KL-005 Manifest parsing stricter than Cargo defaults and workspace inheritance

## Description

Workspace discovery and manifest loading for `parse_workspace` deserialize each
member `Cargo.toml` with **stricter required fields** than Cargo applies at build
time. Valid workspace layouts can therefore fail **before** parsing any Rust
source, with `Failed to parse manifest` and serde-style `missing field …`
messages.

Two common shapes:

1. **Inherited `package.version`:** A workspace root defines
   `[workspace.package] version = "…"`. Members may omit `version` in
   `[package]` and rely on **Cargo’s** `[package] version.workspace = true` (or
   implicit inheritance, depending on edition/tooling). If the member manifest
   omits `version` **and** does not use `version.workspace = true`, Cargo may
   still resolve version via workspace metadata, while the current deserializer
   may require an explicit `version` field in the parsed struct.

2. **`[[bin]]` without `path`:** Cargo defaults binary paths to
   `src/bin/<name>.rs` when `path` is omitted. The manifest schema used for
   parsing may still require `path` on each `[[bin]]` entry.

## Crate-level summary

See [syn_parser known limitations — L3](../syn_parser_known_limitations.md).

## Repro tests (`syn_parser`)

- [`repro_workspace_package_missing_version_manifest_parse_error`](../../../crates/ingest/syn_parser/tests/repro/fail/manifest_errors.rs) —
  workspace with `[workspace.package] version`, member without `package.version`
- [`repro_bin_target_missing_path_manifest_parse_error`](../../../crates/ingest/syn_parser/tests/repro/fail/manifest_errors.rs) —
  `[[bin]]` with `name` only, `src/bin/<name>.rs` present on disk

## Possible future resolution paths

1. **Align with Cargo:** After deserializing, apply the same defaults and
   workspace inheritance rules as `cargo` (or reuse a shared manifest model).
2. **Optional fields:** Represent `version` and `bin.path` as `Option` and fill
   defaults in a second pass using workspace root metadata and target names.
3. **Explicit error surface:** If staying strict, classify these as a dedicated
   error variant with remediation text (“add `path` or …”) rather than raw
   `missing field`.

## Current policy

- Fail discovery with manifest parse errors; do not silently skip members.
- Document until manifest loading matches Cargo-compatible defaults.
