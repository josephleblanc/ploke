# date: 2026-03-30
# task title: syn_parser manifest error expected-failure RCA
# task description: root-cause analysis and fix suggestions for the manifest-parse repros in `crates/ingest/syn_parser/tests/repro/fail/manifest_errors.rs`
# related planning files: /home/brasides/code/ploke/docs/active/agents/2026-03-30_syn_parser_repro_rca/2026-03-30_syn_parser_repro_rca-plan.md, /home/brasides/code/ploke/docs/active/agents/2026-03-29_corpus-triage/2026-03-30_corpus-triage-run-1774867607815.md

## Failure: `repro_workspace_package_missing_version_manifest_parse_error`

Root cause:
- Discovery deserializes `Cargo.toml` into `CargoManifest`/`PackageInfo` via `toml::from_str`, and `PackageInfo.version` is a required field (`PackageInfo { version: PackageVersion }`).
- Cargo accepts a member manifest without `package.version` when the workspace root defines `[workspace.package].version` (workspace package inheritance), but syn_parser’s manifest schema only supports:
  - `version = "x.y.z"` (explicit), or
  - `version.workspace = true` (explicit inheritance link).
- So omitting the `version` key entirely fails at TOML deserialization time with `missing field 'version'` before syn_parser can apply workspace defaults.

Evidence:
- Required field: `PackageInfo.version: PackageVersion` in [`single_crate.rs`](/home/brasides/code/ploke/crates/ingest/syn_parser/src/discovery/single_crate.rs#L49) (see lines 51-54 in the line-numbered view).
- Manifest parse is a hard deserialize step: `toml::from_str(&cargo_content)` into `CargoManifest` in [`discovery/mod.rs`](/home/brasides/code/ploke/crates/ingest/syn_parser/src/discovery/mod.rs#L66).
- The repro test’s minimized manifest shape omits `package.version` but defines `[workspace.package].version`, then asserts the surfaced error contains `Failed to parse manifest` and `missing field \`version\`` in [`manifest_errors.rs`](/home/brasides/code/ploke/crates/ingest/syn_parser/tests/repro/fail/manifest_errors.rs#L31).

Suggested fix / mitigation (no code changes made here):
- Align deserialization with Cargo workspace.package inheritance:
  - Change `PackageInfo.version` to be optional at the TOML schema layer (for example `Option<PackageVersion>` with `#[serde(default)]`).
  - Update version resolution logic so that:
    - `Some(Explicit(..))` works as today.
    - `Some(Workspace(..))` works as today.
    - `None` attempts to read `workspace.package.version` from the nearest workspace manifest (reusing the existing workspace lookup code in `discovery/workspace.rs`), and:
      - if present, uses it (Cargo-compatible).
      - if absent / no workspace, returns a structured error (likely `DiscoveryError::MissingPackageVersion` or a new “version missing and no workspace default” variant) rather than a raw TOML “missing field” error.
- Alternative (heavier, higher fidelity): replace custom TOML schema parsing for targets/metadata with `cargo metadata` output, but that carries runtime and environment costs and may not fit the design intent of a “read-only” discovery phase.

Confidence:
- High. The failure string matches a serde-required-field error, and the `PackageInfo` type requires `version` unconditionally.

## Failure: `repro_bin_target_missing_path_manifest_parse_error`

Root cause:
- `[[bin]]` targets are deserialized into `BinTarget { name: String, path: PathBuf }` and `path` is required.
- Cargo allows `[[bin]] name = \"helper\"` without an explicit `path`, defaulting to `src/bin/helper.rs` (or similar conventional layouts).
- syn_parser fails earlier during `toml::from_str` into `CargoManifest` with `missing field 'path'`, even though later discovery logic already scans `src/bin/*.rs` when `autobins` is enabled.

Evidence:
- Required field: `BinTarget.path: PathBuf` in [`single_crate.rs`](/home/brasides/code/ploke/crates/ingest/syn_parser/src/discovery/single_crate.rs#L626) (see lines 626-633 in the line-numbered view).
- The manifest is deserialized as a whole (so a single missing field in `[[bin]]` rejects the entire `Cargo.toml`) in [`discovery/mod.rs`](/home/brasides/code/ploke/crates/ingest/syn_parser/src/discovery/mod.rs#L66).
- The repro test’s minimized manifest declares a `[[bin]]` with only `name` and asserts `missing field \`path\`` in [`manifest_errors.rs`](/home/brasides/code/ploke/crates/ingest/syn_parser/tests/repro/fail/manifest_errors.rs#L90).

Suggested fix / mitigation (no code changes made here):
- Make `BinTarget.path` optional at the manifest schema layer:
  - Use `path: Option<PathBuf>` with `#[serde(default)]` so TOML deserialization succeeds when `path` is omitted.
  - When processing explicit `[[bin]]` targets, if `path` is `None`, derive the default path using Cargo conventions, for example:
    - prefer `src/bin/<name>.rs` if present
    - else consider `src/bin/<name>/main.rs` if present
    - else fall back to the existing “autobins scan” behavior (and possibly emit a non-fatal warning if the explicit bin cannot be resolved)
- This is especially important when `package.autobins = false`: explicit `[[bin]]` entries should still resolve even without an explicit path.

Confidence:
- High. The `BinTarget` schema makes `path` required and the reported error is exactly the serde missing-field message.
