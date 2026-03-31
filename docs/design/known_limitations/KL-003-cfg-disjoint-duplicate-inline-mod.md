# KL-003 Duplicate module paths from disjoint cfg-gated inline modules

## Description

`syn_parser` merges parsed files into a single `ModuleTree` keyed by canonical
`NodePath`. Valid Rust may declare **two inline** `mod name { ... }` blocks with
the **same** name under **mutually exclusive** `#[cfg(...)]` attributes (only one
exists for any rustc configuration). The syntactic merge still sees **both**
definitions and indexes them as separate `ModuleNode`s at the same path.

`ModuleTree::add_module` then reports `DuplicatePath` (surfaced as
`Failed to build module tree` / `Duplicate definition path 'crate::…'`). [ADR-025](../adrs/accepted/ADR-025-module-tree-staged-file-duplicate-definitions.md)
staging resolves **file** vs **inline** collisions only; **inline vs inline** at
the same path remains an error.

Earlier notes on the same mechanism (including Phase 3 edge-case fixtures) are
preserved in git only (working copy may exist under gitignored `docs/archive/`):

- **Commit:** `8de1588216561ad23290fac0a35993e4b2288e16`
- **Path:** `docs/design/known_limitations/P3-00-cfg-duplication.md`
- **View:** `git show 8de1588216561ad23290fac0a35993e4b2288e16:docs/design/known_limitations/P3-00-cfg-duplication.md`

## Crate-level summary

See [syn_parser known limitations — L1](../syn_parser_known_limitations.md).

## Repro tests (`syn_parser`)

- [`fixture_duplicate_cfg_test_mods_is_valid_rust`](../../../crates/ingest/syn_parser/tests/repro/fail/cfg_gates.rs) — fixture `tests/fixture_workspace/ws_fixture_02_assoc_local_enum_ids/member_cfg_test_mods_repro`
- [`repro_duplicate_cfg_gated_module_merge_error`](../../../crates/ingest/syn_parser/tests/repro/fail/cfg_gates.rs) — fixture `member_cfg_duplicate_mods_repro`

## Possible future resolution paths

1. **Cfg-aware keys:** extend indexing with cfg predicates or a compilation-unit
   key so alternates do not share one `path_index` slot.
2. **Target-scoped merge:** evaluate `#[cfg]` like Cargo for a chosen triple
   before module-tree construction.
3. **Document-only recovery:** weaker graphs if arbitrarily picking one branch;
   must be explicit policy (see ADR-025 “union-of-all-cfgs” out of scope).

## Current policy

- Fail merge with a clear duplicate-path error rather than silently merging
  incompatible definitions.
- Document as a known limitation rather than relaxing `path_index` invariants
  without a designed cfg model.

## Actions taken

- [syn_parser known limitations L1](../syn_parser_known_limitations.md) documents
  symptom, cause, and repro fixtures.
- Failure repros under `tests/repro/fail/cfg_gates.rs` assert the expected error
  strings.
