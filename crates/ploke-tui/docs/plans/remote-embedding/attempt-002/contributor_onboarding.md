# Remote Embedding Attempt 002 — Contributor Onboarding

Purpose: compress the discovery work required to orient yourself on the remote-embedding refactor. Follow the quick reference table to understand why each file matters, then use the checklist to walk through the exact steps (including commands) to confirm the current state **before editing code**—do not start implementation until the planning docs referenced below have been read in full and logged in the implementation log.

## Quick reference map

| Path | Lines | Why it matters |
| --- | --- | --- |
| `crates/ploke-tui/docs/plans/remote-embedding/attempt-002/README.md` | 1-43 | Entry point describing the planning hub contents and expected artifact flow. |
| `crates/ploke-tui/docs/plans/remote-embedding/attempt-002/execution_plan.md` | 1-88 | Slice-by-slice goals; read Slice 1 + future slices to know prerequisites before coding. |
| `crates/ploke-tui/docs/plans/remote-embedding/attempt-002/feature_flags.md` | 7-55 | Describes intended cfg/feature rollout; note gaps vs. current `Cargo.toml` reality. |
| `crates/ploke-tui/docs/plans/remote-embedding/attempt-002/experimental_fixtures_plan.md` | 1-37 | Phase A/B fixture strategy and stop-and-test checkpoint before touching shared fixtures. |
| `crates/ploke-tui/docs/plans/remote-embedding/attempt-002/telemetry_evidence_plan.md` | 5-74 | Artifact requirements per slice; governs JSON outputs under `target/test-output/embedding/`. |
| `crates/ploke-tui/docs/plans/remote-embedding/attempt-002/governance/implementation-log-025.md` | 1-47 | Latest implementation log; shows pending items plus guardrails for updating plans. |
| `crates/ploke-tui/docs/plans/remote-embedding/attempt-002/governance/decisions_required_remote_embedding.md` | 1-46 | Scoped decision queue (kill switch, storage policy) with resolved/active guidance. |
| `crates/ploke-db/src/multi_embedding/mod.rs` | 1-34 | Re-export hub for adapter/schema/vector modules and HNSW distance enum. |
| `crates/ploke-db/src/multi_embedding/adapter.rs` | 1-220 | Adapter traits providing strongly typed access to Cozo relations; mirrors telemetry expectations. |
| `crates/ploke-db/src/multi_embedding/tests.rs` | 1-160 | Validates metadata/vector parity across node specs; shows supported API surface. |
| `crates/test-utils/src/lib.rs` | 197-360 | `setup_db_full_embeddings` + `seed_multi_embedding_schema`; seeds metadata/vector rows when the schema flag is enabled. |
| `xtask/src/main.rs` | 90-214, 346-447 | `cargo xtask verify-fixtures --multi-embedding` implementation plus validation report logic. |
| `target/test-output/embedding/fixtures/multi_embedding_fixture_verification.json` | n/a | Latest evidence from `verify-fixtures`; use as baseline until `slice1-schema.json` exists. |

## Step-by-step onboarding checklist

1. **Establish the planning context**
 - Read `attempt-002/README.md` to understand the hub and governance expectations (`README.md:1-43`).
  - Skim `execution_plan.md:1-88`, focusing on Slice 1 prerequisites (schema, migrations, fixtures) and the later slices to see how work stages.

> **Runtime-owned embeddings decision.** Slice 1 intentionally defers embedding metadata/relations to runtime (`ploke-db`) instead of extending `ploke-transform` to mirror the legacy ingest flow. Contributors must log any work that touches embedding schemas and confirm that runtime helpers (`ensure_embedding_relation`) remain the sole creators of `embedding_nodes` + per-dimension vector relations.

2. **Verify gating expectations**
   - Read `feature_flags.md:7-55` to see the intended `multi_embedding_*` features (schema, db, runtime, release, kill switch).
   - Cross-check actual flags by opening `crates/ploke-db/Cargo.toml:41-44` and `crates/test-utils/Cargo.toml:38-42`. Note that only `multi_embedding_experiment` (plus the local schema wrapper) currently exists—capture discrepancies in your implementation log entry.

3. **Understand fixture and telemetry obligations**
   - Follow `experimental_fixtures_plan.md:1-37` for Phase A/B tasks and the “Stop & Test” checkpoint prior to editing production fixtures.
   - Read `telemetry_evidence_plan.md:5-74` to know which JSON artifacts (e.g., `slice1-schema.json`) must exist before claiming readiness.

4. **Review governance artifacts**
   - Open `governance/implementation-log-025.md:1-47` to see what has been logged and which items remain TODO (e.g., updating required-groundwork references, telemetry doc linkage).
   - Check `governance/decisions_required_remote_embedding.md:1-46` for currently open vs. resolved decisions so you do not reinvent policy discussions.

5. **Inspect the experimental implementation**
   - Start from `crates/ploke-db/src/multi_embedding/mod.rs:1-34` to see module exports and the `HnswDistance` enum.
   - Dive into:
     - `schema/metadata.rs`, `schema/node_specs.rs`, and `schema/vector_dims.rs` for relation definitions and supported dimensions.
     - `vectors.rs` for `ExperimentalVectorRelation::ensure_registered`, `insert_row`, and naming conventions (per-dimension `<F32; dims>`).
     - `adapter.rs:1-220` to understand the typed helpers used by tests + xtask verification.
     - `tests.rs:1-160` to observe coverage across node types and HNSW helpers.

6. **Map test utilities and fixtures**
   - `crates/test-utils/src/lib.rs:197-360` shows how `setup_db_full_embeddings` conditionally seeds multi-embedding metadata/vector rows when the `multi_embedding_schema` feature is enabled. This is the entry point used by integration tests and fixture regeneration commands.
   - Use `rg -n "multi_embedding_schema" crates/test-utils/src/lib.rs` to quickly jump between helper definitions if you need to extend seeding logic.
   - Keep the implementation layout aligned with the legacy system: schema structs/migrations belong in `ploke-transform`, and `ploke-db` owns adapters/search helpers. When adding new files, mirror the existing folder structure so dual-write parity work and eventual cleanup remain straightforward.

7. **Confirm fixture verification tooling**
   - `xtask/src/main.rs:90-214` explains how `cargo xtask verify-fixtures --multi-embedding` validates both legacy and schema-tagged backups, printing row counts for metadata/vector relations.
   - `xtask/src/main.rs:346-447` iterates every node spec and dimension, asserting relation existence and matching row counts. Update this if schemas or dimensions change.
   - Run `cargo xtask verify-fixtures --multi-embedding` before touching fixtures; store the JSON output under `target/test-output/embedding/fixtures/`. The command now auto-regenerates missing legacy/multi backups by invoking `cargo run -p ploke-test-utils --bin regenerate_fixture` with the required features, so new machines pick up the correct Cozo databases without manual seeding (watch the console output in case regeneration fails and manual intervention is required).

8. **Check current evidence**
   - Inspect `target/test-output/embedding/fixtures/multi_embedding_fixture_verification.json` (latest run: `2025-11-17T12:38:09Z`) to know the current metadata/vector counts (183/732). Link this artifact in new implementation log entries alongside `slice1-schema.json`.

9. **Audit error handling**
   - `crates/ploke-db/src/error.rs:32-56` declares experimental error variants behind `multi_embedding_experiment`. Any schema/runtime changes must keep these variants in sync.

10. **Commands to re-run when onboarding**
    - `rg -n "multi_embedding" -g "*.rs"`: identify code paths guarded by the feature flags.
    - `cargo test -p ploke-db multi_embedding_experiment --features multi_embedding_experiment`: run the experimental suite described in Phase A.
    - `cargo xtask verify-fixtures --multi-embedding`: verify fixture backups and regenerate `target/test-output/embedding/fixtures/*.json`.

Following these steps ensures every contributor touches the same plans, governance notes, and evidence before making changes, eliminating the “hunt through the repo” burden encountered during this onboarding pass.
