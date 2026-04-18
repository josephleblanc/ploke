# 2026-04-17 Failure Inventory

- date: 2026-04-17
- source campaign: `rust-baseline-grok4-xai`
- scope: current `18` eval-failure rows from closure recompute

## Cluster Summary

| cluster_id | count | instances | primary_category | known_limitation_or_bug | status | notes |
|---|---:|---|---|---|---|---|
| `F-CLAP-PARTIAL-PARSE` | 2 | `clap-rs__clap-1624`, `clap-rs__clap-941` | `INDEX_FIDELITY` | [KL-006 partial parse](../../../design/known_limitations/KL-006-partial-parse-non-compilable-files.md) | `documented` | existing known limitation covers the failure family; no new bug doc added in this pass |
| `F-NUSHELL-DUP-COMMANDS` | 4 | `nushell__nushell-10395`, `nushell__nushell-12901`, `nushell__nushell-13357`, `nushell__nushell-13870` | `INDEX_FIDELITY` | [duplicate commands bug](../../../active/bugs/2026-04-17-nushell-duplicate-commands-module-path.md) | `newly_documented` | duplicate `crate::commands` module-tree path does not match existing `KL-003`/`KL-004` exactly |
| `F-GENERIC-LIFETIME` | 6 | `nushell__nushell-11493`, `nushell__nushell-11672`, `nushell__nushell-11948`, `nushell__nushell-12118`, `serde-rs__serde-2709`, `serde-rs__serde-2798` | `INDEX_FIDELITY` | [generic_lifetime bug](../../../active/bugs/2026-04-17-generic-lifetime-transform-failure.md) | `newly_documented` | current artifacts support a stable failure family but not yet a narrow crate-local root cause |
| `F-NUSHELL-INDEXING-TIMEOUT` | 6 | `nushell__nushell-10381`, `nushell__nushell-10405`, `nushell__nushell-10613`, `nushell__nushell-10629`, `nushell__nushell-11169`, `nushell__nushell-11292` | `SETUP_ENVIRONMENT` | [indexing timeout bug](../../../active/bugs/2026-04-17-nushell-indexing-completed-timeout.md) | `newly_documented` | repeated 300s timeout symptom; active `nushell` target limitation recorded in target-capability registry |

## Audit Notes

- This file is the live landing surface for cluster-level claims, evidence, and
  documentation status.
- `generic_lifetime` was narrowed from the initial draft after artifact review;
  `nushell__nushell-12901`, `-13357`, and `-13870` belong to the duplicate
  `crate::commands` cluster instead.
- `nushell__nushell` now has an active target-capability entry at
  [target-capability-registry.md](../../workflow/target-capability-registry.md).
