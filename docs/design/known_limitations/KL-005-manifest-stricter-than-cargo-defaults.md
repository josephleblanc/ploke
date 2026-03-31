# KL-005 Manifest parsing aligned with Cargo (resolved)

## Status

**Resolved** for `syn_parser` workspace discovery: manifests are loaded with
[`cargo_toml::Manifest::from_path`](https://docs.rs/cargo-toml/latest/cargo_toml/struct.Manifest.html#method.from_path),
which applies Cargo-style completion on disk (including workspace inheritance and implicit
`lib`/`bin` defaults). The previous strict `toml::deserialize` into hand-written structs no longer
applies to this path.

Discovery still fails loudly on I/O errors and unrecoverable parse errors; it does not skip members
silently.

## Historical description (pre-migration)

Workspace discovery deserialized each member `Cargo.toml` with **stricter required fields** than
Cargo. Valid layouts could fail **before** parsing Rust source, with serde-style `missing field …`
messages. Typical shapes:

1. **Omitted `package.version`:** Cargo may treat missing version as **0.0.0** or resolve via
   workspace metadata depending on edition and `[workspace.package]`; strict structs often
   required an explicit `version` field.

2. **`[[bin]]` without `path`:** Cargo defaults binary paths to `src/bin/<name>.rs` when `path` is
   omitted; a strict schema could still require `path` on each `[[bin]]` entry.

## Crate-level summary

See [syn_parser known limitations — L3](../syn_parser_known_limitations.md) (retired as an active
limitation; cross-link preserved for navigation).

## Regression tests (`syn_parser`)

Success repros (Cargo-faithful minimal shapes, corpus provenance in file comments):

- [`repro_workspace_package_missing_version_bevy_like_accepts_like_cargo`](../../../crates/ingest/syn_parser/tests/repro/success/kl005_manifest_cargo_alignment.rs) —
  member with only `name` + `edition` in `[package]`; asserts resolved version **0.0.0** like Cargo
  for the Bevy `benches`-like hotspot shape.

- [`repro_bin_target_omitted_path_defaults_like_cargo`](../../../crates/ingest/syn_parser/tests/repro/success/kl005_manifest_cargo_alignment.rs) —
  `[[bin]]` with `name` only and `src/bin/<name>.rs` on disk; asserts default bin path.

**Workspace-inherited `version` via `version.workspace = true`** (alternate shape, not the Bevy
hotspot) is covered by the crate unit test `discovery::tests::test_toml_basic` against
`tests/fixture_workspace/ws_fixture_00` — not duplicated in the KL-005 repro module to avoid
replacing one corpus story with a different manifest.

## Policy (unchanged)

- Fail discovery with manifest parse errors when parsing truly fails; do not silently skip
  members.
