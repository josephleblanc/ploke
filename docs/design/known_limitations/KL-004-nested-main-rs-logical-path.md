# KL-004 Duplicate module paths from nested `main.rs` logical-path derivation

## Description

`syn_parser` assigns each parsed file a **logical module path** (see
[ADR-005](../adrs/accepted/ADR-005-derive-logical-module-paths.md)) used to build
the `ModuleTree`. The implementation treats any file named `main.rs` like
`mod.rs` or `lib.rs` for path purposes: it **pops** the filename and maps the
file to the **parent directory’s** module path.

That matches the **crate-root** binary file `src/main.rs`, but it is **wrong**
for nested paths such as `src/cli/main.rs` or `src/scheduler/queue/main.rs`:

- A **library** `mod cli;` may resolve to `src/cli/mod.rs`, defining
  `crate::cli`.
- A **binary** target with `path = "src/cli/main.rs"` is then also mapped to
  `crate::cli`, producing two file-backed modules at the same `NodePath`.

Similarly, when `src/scheduler/queue/mod.rs` contains `mod main;` and
`src/scheduler/queue/main.rs` exists, the `main.rs` file should define
`crate::scheduler::queue::main`, but the same rule maps it to
`crate::scheduler::queue`, colliding with the `queue` module from
`queue/mod.rs`.

`ModuleTree::add_module` then reports `DuplicatePath` (surfaced as
`Failed to build module tree` / `Duplicate definition path 'crate::…'`).

## Relationship to other work

- [ADR-025](../adrs/accepted/ADR-025-module-tree-staged-file-duplicate-definitions.md)
  stages **file vs inline** collisions; these failures are **two file-backed**
  definitions at one path from **path assignment**, not from staging gaps.
- [L1 / KL-003](../syn_parser_known_limitations.md) covers **cfg-gated duplicate
  inline** modules; this KL is a **different** mechanism.

## Crate-level summary

See [syn_parser known limitations — L2](../syn_parser_known_limitations.md).

## Repro tests (`syn_parser`)

- [`repro_duplicate_cli_binary_module_merge_error`](../../../crates/ingest/syn_parser/tests/repro/fail/file_links.rs) —
  fixture `tests/fixture_workspace/ws_fixture_03_cli_collision/member_cli_collision`
- [`repro_duplicate_scheduler_queue_mod_merge_error`](../../../crates/ingest/syn_parser/tests/repro/fail/file_links.rs) —
  fixture `tests/fixture_workspace/ws_fixture_02_assoc_local_enum_ids/member_scheduler_queue_repro`

## Possible future resolution paths

1. **Path derivation:** Only apply the `main.rs`-as-root stripping when the file
   is the **compilation-unit root** for a target (e.g. matches the target’s
   declared root path), not for every `**/main.rs`. Nested `main.rs` files should
   follow normal module-file naming (typically `crate::…::main`).
2. **Per-target namespaces:** Build or merge module trees with a disambiguator
   per target (lib vs each bin) so unrelated targets never share one `crate::…`
   key—still needed for multi-target graphs even if (1) fixes the collisions
   above.

## Current policy

- Fail merge with a clear `DuplicatePath` error rather than silently picking a
  branch.
- Track as a known limitation until path rules are revised and repros are
  retargeted.

## Further reading

- Root-cause notes (internal):
  [`docs/active/agents/2026-03-30_syn_parser_repro_rca/reports/2026-03-30_file_links_report.md`](../../active/agents/2026-03-30_syn_parser_repro_rca/reports/2026-03-30_file_links_report.md)
  (sections **repro_duplicate_cli_binary_module_merge_error** and
  **repro_duplicate_scheduler_queue_mod_merge_error**).
