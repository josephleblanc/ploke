# Remote Embedding Feature Flags (Attempt 002)

Purpose: codify the gating strategy for the multi-embedding rollout so every slice and crate uses the same cfg/feature names and exit criteria. No code changes may ship until the relevant section below is implemented and referenced in the implementation log.

## Flag overview

| Flag | Scope | Declared in | Default | Purpose | Owned by | Exit criteria |
| --- | --- | --- | --- | --- | --- | --- |
| `multi_embedding_schema` | Cargo feature + `#[cfg]` | `crates/ingest/ploke-transform` (and crates that depend on new schemas) | OFF | Enables new embedding schema modules, ingest structs, and fixture generation without touching DB/query logic. | Ingest team (ploke-transform maintainers) | Slice 1 validated (schema tests + `verify-fixtures`), migration scripts landed, plan + reports updated. |
| `multi_embedding_db` (implies `multi_embedding_schema`) | Cargo feature | `crates/ploke-db`, `ploke-transform` (for dual-write), `ploke-embed` (for helper calls) | OFF | Dual-write/dual-read in `ploke-db` helpers, new relations, HNSW builders. Exposed via runtime config knob + env var `PLOKE_MULTI_EMBEDDING_DB=1` for integration tests. | Database team (ploke-db maintainers) | Slice 2 validated across all node types, parity tests + artifacts `target/test-output/embedding/slice2-db.json`, fixtures updated, legacy columns still present. |
| `multi_embedding_runtime` (implies db flag) | Cargo feature | `ploke-embed`, `ploke-tui`, `ploke-rag` | OFF | Enables indexer + runtime writers/readers to route through embedding-set aware APIs; unlocks `/embedding use` and manager changes. | Runtime team (tui/indexer maintainers) | Slice 3 evidence (TEST_APP harness run, telemetry artifacts `slice3-runtime.json`) and `/embedding` commands validated offline. |
| `multi_embedding_release` | Workspace-level meta feature | workspace `Cargo.toml` | OFF | Convenience umbrella enabling schema+db+runtime flags together for pre-release soak tests. | Release engineering | Enabled only after Slice 3 sign-off; removed after Slice 4 when defaults flip to ON. |
| `multi_embedding_kill_switch` | Runtime config flag + environment variable | `ploke-tui`, `ploke-embed` | ON (kill switch engaged) | Allows disabling remote-embedding behavior at runtime even if compile-time features are ON (e.g., revert to legacy columns if needed). Implementation detail: when true, runtime always selects legacy embedding set and refuses to schedule multi-model jobs. | Runtime on-call | Removed once Slice 4 is stable for N releases; documented rollback procedure verifies kill switch works. |

## Current implementation status (Nov 2025)

- `ploke-db` now defines the full flag ladder (`multi_embedding_schema`, `multi_embedding_db`, `multi_embedding_runtime`, `multi_embedding_release`, `multi_embedding_kill_switch`). The legacy `multi_embedding_experiment` feature still exists but simply re-exports `multi_embedding_schema` so existing commands/tests keep working while downstream crates migrate.
- `ploke-transform`, `ploke-embed`, `ploke-rag`, `ploke-tui`, and `ploke-test-utils` expose matching features that forward to their dependencies (e.g., enabling `ploke-tui/multi_embedding_runtime` automatically enables the schema+db flags in `ploke-transform`, `ploke-embed`, `ploke-db`, and `ploke-rag`).
- Tooling (`xtask`) and telemetry artifacts now record the new feature names (see `target/test-output/embedding/slice1-schema.json` for the latest run).
- Remaining action items:
  1. Migrate code-level `#[cfg(feature = "multi_embedding_experiment")]` attributes in dependent crates to the new names (ploke-db has already switched; runtime crates will follow as features ship).
  2. Start using the new flag names in all test commands (`cargo test -p ploke-db --features multi_embedding_schema`, etc.) so telemetry artifacts match reality.
  3. Remove the `multi_embedding_experiment` alias once Slice 1 stabilizes under the new feature hierarchy.

## Implementation rollout playbook

1. **Workspace scaffolding.** Add `[features]` entries to the workspace `Cargo.toml` so `multi_embedding_schema`, `multi_embedding_db`, `multi_embedding_runtime`, `multi_embedding_release`, and `multi_embedding_kill_switch` can be toggled at the root. The workspace definitions simply propagate to the owning crates listed in the overview table (schema → db → runtime). Keep the existing `multi_embedding_experiment` cfgs temporarily aliased to `multi_embedding_schema` while the refactor lands so dependent crates continue compiling.
2. **Database + ingest crates.** Update `crates/ploke-db/Cargo.toml` to expose `multi_embedding_schema` (replacing `multi_embedding_experiment`) and gate `crates/ingest/ploke-transform` schema modules behind the same feature. Dual-write helpers and adapters remain under a new `multi_embedding_db` flag that `ploke-transform` re-exports when dual-write support is required.
3. **Runtime crates.** Once Slice 2 is ready, introduce `multi_embedding_runtime` in `ploke-embed`, `ploke-tui`, and `ploke-rag`. This feature implies the db flag and will be the gate for `/embedding use`, TEST_APP harness flows, and the kill-switch plumbing. Runtime crates should also respect a config/env toggle named in this doc.
4. **Release bundling.** Define the `multi_embedding_release` meta feature alongside `multi_embedding_kill_switch` so release builds (`cargo build -F multi_embedding_release`) automatically flip schema/db/runtime ON and wire the kill switch for soak testing.
5. **CI + tooling updates.** Update `xtask`, fixture generators, and CI workflows to use the new feature names (`multi_embedding_schema`, etc.) instead of the `multi_embedding_experiment` placeholder. Track every change by referencing this playbook plus the owning files (e.g., `crates/ploke-db/Cargo.toml:features`, `crates/test-utils/Cargo.toml:features`) in implementation log updates.

## Dependency rules

1. `multi_embedding_runtime` requires `multi_embedding_db`, which in turn requires `multi_embedding_schema`. Cargo features should enforce this via `features = ["multi_embedding_db"]` style constraints.
2. Build scripts/tests that only need schema metadata (e.g., fixture verification) must enable `multi_embedding_schema` directly to avoid pulling in DB/runtime deps.
3. Integration tests that exercise DB helpers should enable `--features "multi_embedding_db"` on `ploke-db` and set `PLOKE_MULTI_EMBEDDING_DB=1` (or use `Config::enable_multi_embedding_db()` once added).
4. TEST_APP / live harnesses only enable `multi_embedding_runtime` when running against dedicated databases prepared with the new relations. The default developer workflow keeps this feature OFF until Slice 3 merges.

## Runtime configuration knobs

| Component | Flag/Env | Behavior |
| --- | --- | --- |
| `ploke-tui` | `embedding.multi_embedding=true` in config + `PLOKE_MULTI_EMBEDDING_DB=1` env | Allows `/embedding use` to surface new embedding sets. If unset, commands are hidden and handlers fall back to legacy embedding column. |
| `ploke-embed` | `PLOKE_MULTI_EMBEDDING_RUNTIME=1` env | Indexer emits embedding-set metadata, but still respects kill switch. |
| `xtask verify-fixtures` | `--multi-embedding` CLI flag | Forces fixture verification to assert presence of new relations; requires schema feature enabled. |

## Gating + evidence expectations

- Every slice must note which flags were enabled when collecting test evidence; store this next to the artifact JSON under `target/test-output/embedding/`.
- Live API tests remain behind `cfg(feature = "live_api_tests")` regardless of multi-embedding flags. When `multi_embedding_runtime` is enabled, the live gate report must include tool-call traces proving multi-set requests executed.
- Before flipping defaults to ON (Slice 4), run a soak build with `multi_embedding_release` enabled and record pass/fail counts in `remote-embedding-slice4-report.md`.

## Ownership & change control

- Flag definition edits must be recorded in the relevant `Cargo.toml` plus this document. Cross-crate changes require sign-off from the owning team noted above.
- Removal of any flag requires an explicit decision entry in `crates/ploke-tui/docs/decisions_required.md` referencing the evidence used to justify removal.

## Workflow checklist before editing code

1. Update this document if flag scopes/names change.
2. Link the implementation log entry for the slice and note which flags are being touched.
3. Ensure any PR description references both the execution plan and this flag document.

## End-state expectation

These flags are transitional. The end goal for attempt 002 is a complete migration to the multi-embedding architecture with **no** legacy single-embedding code paths left behind. After Slice 4 ships and we have validated parity plus live telemetry evidence, we will:

1. Flip `multi_embedding_schema`, `multi_embedding_db`, and `multi_embedding_runtime` ON by default.
2. Run a soak period with `multi_embedding_release` to detect regressions while legacy columns still exist.
3. Remove the legacy columns, the transitional feature flags, and the kill switch once the soak succeeds and the decisions log records approval.

Post-migration, the multi-embedding design becomes the only supported storage + runtime model; the flag document will be retired after the cleanup commits land.

This doc lives alongside the execution plan so future agents can quickly verify the gating state before contributing to remote-embedding work.
