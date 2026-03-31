# `syn_parser` known limitations

This document records behaviors that are **expected today**: valid Rust that `rustc` accepts for a given configuration may still fail module-tree merge with `DuplicatePath`, or may include items from multiple cfg branches at once. **Pre-expansion parsing** uses `syn::parse_file` per file; sources that never compile, or that only compile after proc-macro expansion, may fail parsing or produce partial-parse errors. See [ADR-025](adrs/accepted/ADR-025-module-tree-staged-file-duplicate-definitions.md) for file-vs-inline staging; the items below include issues **outside** that ADR’s scope.

**Manifest discovery** uses `cargo_toml` with on-disk completion; the former KL-005 gap (stricter deserialization than Cargo) is **resolved** — see [KL-005](known_limitations/KL-005-manifest-stricter-than-cargo-defaults.md) for history and regression tests.

---

## L1 — Duplicate inline `mod` definitions at the same path under disjoint cfgs

**KL index:** [KL-003](known_limitations/KL-003-cfg-disjoint-duplicate-inline-mod.md).

**Symptom:** Merge fails with `Failed to build module tree` and  
`Duplicate definition path 'crate::…'` for a path that appears **once** per rustc configuration.

**Cause:** The parser builds a **single** graph without rustc-style cfg selection. Mutually exclusive `#[cfg(...)]` blocks can each define an **inline** submodule with the same name (same `NodePath`). `ModuleTree` keeps at most one definition per path ([ADR-025](adrs/accepted/ADR-025-module-tree-staged-file-duplicate-definitions.md) staging applies only to **file** vs **inline** collisions, not inline vs inline).

**Workarounds (future):** Target-scoped or cfg-aware module keys (e.g. `NodePath` + cfg predicate), or evaluating cfgs like Cargo for a chosen triple.

**Repro tests / fixtures** (`crates/ingest/syn_parser`):

- `fixture_duplicate_cfg_test_mods_is_valid_rust` — fixture `member_cfg_test_mods_repro`
- `repro_duplicate_cfg_gated_module_merge_error` — fixture `member_cfg_duplicate_mods_repro`

---

## L2 — Duplicate file-backed module paths from nested `main.rs` logical-path rules

**KL index:** [KL-004](known_limitations/KL-004-nested-main-rs-logical-path.md).

**Symptom:** Merge fails with `Failed to build module tree` and  
`Duplicate definition path 'crate::…'` for layouts that **Cargo accepts**, e.g. a lib `mod cli` next to a bin rooted at `src/cli/main.rs`, or `mod main;` beside `queue/mod.rs` with a nested `queue/main.rs`.

**Cause:** Logical paths treat **any** `main.rs` like a directory module root (same stripping as `mod.rs` / crate-root `main.rs`), so nested `**/main.rs` files can be keyed to the **parent** module’s `NodePath` and collide with the real module file at that path. This is **not** the same as L1 (cfg duplicate inline mods) and is **not** fixed by ADR-025 file-vs-inline staging when **both** sides are file-backed definitions at one path.

**Workarounds (future):** Refine logical-path derivation so only **target root** `main.rs` files get the special-case mapping; consider per-target or per-compilation-unit namespaces when merging graphs.

**Repro tests / fixtures** (`crates/ingest/syn_parser`):

- `repro_duplicate_cli_binary_module_merge_error` — `tests/fixture_workspace/ws_fixture_03_cli_collision/member_cli_collision`
- `repro_duplicate_scheduler_queue_mod_merge_error` — `tests/fixture_workspace/ws_fixture_02_assoc_local_enum_ids/member_scheduler_queue_repro`

---

## L3 — `Cargo.toml` parsing stricter than Cargo defaults / workspace package *(resolved)*

**KL index:** [KL-005](known_limitations/KL-005-manifest-stricter-than-cargo-defaults.md).

**Status:** Addressed by migrating discovery to `cargo_toml::Manifest::from_path` (completion on disk). Former failure modes (missing `version` / `[[bin]].path` where Cargo supplies defaults) are covered by success repros in [`tests/repro/success/kl005_manifest_cargo_alignment.rs`](../../crates/ingest/syn_parser/tests/repro/success/kl005_manifest_cargo_alignment.rs). Workspace `version.workspace = true` inheritance is additionally exercised by `discovery::tests::test_toml_basic` / `tests/fixture_workspace/ws_fixture_00`.

**Historical symptom (no longer current):** `parse_workspace` could fail during discovery with `Failed to parse manifest` and serde `missing field \`version\`` / `missing field \`path\`` on manifests `cargo` could build.

---

## L4 — Partial parse: valid modules alongside intentionally invalid / template Rust

**KL index:** [KL-006](known_limitations/KL-006-partial-parse-non-compilable-files.md).

**Symptom:** Resolve fails with `Partial parsing success: …` / `SynParserError::PartialParsing` when one module file uses placeholder or invalid syntax (`...`, etc.) and other files parse.

**Cause:** Each file must parse through `syn` before visitors run; non-compilable template sources are not skipped by default.

**Workarounds (future):** Configurable exclude globs, opt-in per-file skip with explicit graph markers, or remove/repair template files in the target crate.

**Repro tests** (`crates/ingest/syn_parser`):

- `repro_partial_parsing_with_template_placeholders`

---

## L5 — Proc-macro pre-expansion: compile-valid raw source not parseable by `syn`

**KL index:** [KL-002](known_limitations/KL-002-proc-macro-pre-expansion-syntax.md).

**Symptom:** `syn::parse_file` errors (e.g. `expected ','`) on source that **rustc** accepts after proc-macro expansion (e.g. `#[duplicate_item(...)]` token trees), or aggregate `MultipleErrors` from the same class of failure.

**Cause:** The pipeline parses **pre-expansion** Rust; macro-generated valid syntax is not visible to `syn` at parse time.

**Workarounds (future):** Expansion pipeline, post-expansion parse, or narrowly scoped macro-specific preprocessors (brittle).

**Repro tests** (`crates/ingest/syn_parser`):

- `repro_duplicate_item_placeholder_trait_signatures`
