# Multi-Embedding Integration Attempt – Postmortem

## What Happened
- **Goal.** Wire the experimental multi-embedding schema into production (ploke-transform ingest + ploke-db + ploke-embed indexer).
- **Initial edits.** I introduced a new `embeddings.rs` schema module, extended the transform functions to populate `function_multi_embedding`, and began converting ploke-db helpers (batch updates, pending counts, cursor-based fetches) plus the embedding indexer/tests to the new relations.
- **Scope expansion.** Touching ingest schemas forced parallel changes in the database layer (query builders, helper structs, tests) and the embedding pipeline. Updating `get_unembedded_node_data`, `update_embeddings_batch`, `count_pending_embeddings`, HNSW helpers, and the indexer spilled into many files.
- **Rollback attempt.** Partway through I realized the refactor was snowballing. I tried to revert touching files with `git show HEAD:… > file`, but aborted before removing the new `embeddings.rs`. The workspace now contains partially reverted files plus new artifacts, leaving the repo in an inconsistent state.

## What Went Wrong
1. **Underestimated blast radius.** The experiment implemented parallel relations plus parsing/validation helpers. Porting it wholesale required simultaneous schema, DB, ingest, and indexer changes. I started editing all fronts instead of landing incremental, testable steps.
2. **No migration plan.** The DB helpers and indexer still assume a single `embedding` column. I tried to retrofit them while editing ingest, which meant I could not rely on existing tests to stay green.
3. **Rollback via manual file replacement.** Without a clean branch-per-step strategy, I resorted to `git show … > file` and an aborted `rm`, so the repo is neither fully forward nor fully reverted.

## Current State (before you reset)
- `crates/ingest/ploke-transform/src/schema/embeddings.rs` was added but not deleted (the `rm` command was aborted).
- Several files (`ploke-db/src/database.rs`, `ploke-embed/src/indexer/mod.rs`, test utils, etc.) were overwritten with their previous versions, while others (e.g., `ploke-transform/src/schema/mod.rs`, `transform/functions.rs`) still contain partial changes.
- The repo is inconsistent; there is no working build or coherent migration.

## Recommendations
1. **Reset to last known good commit.** The workspace is too inconsistent to salvage piecemeal. A `git reset --hard <last-good>` (outside of this agent session) is the fastest path back to green.
2. **Re-plan in smaller slices.**
   - *Slice 1:* Land the new schema module + ingest wiring only. Include migrations (schema creation, default metadata) and tests confined to ploke-transform.
   - *Slice 2:* Update ploke-db helpers to read both legacy and new relations behind a feature flag, preserving current APIs.
   - *Slice 3:* Teach the embedding indexer/update path to write into `function_embedding_vectors`, again gated until verified.
   - *Slice 4:* Flip feature flag + clean up legacy `embedding` column.
3. **Use feature flags / cfgs.** Keep the multi-embedding experiment behind `#[cfg(feature = "multi_embedding_experiment")]` or similar until end-to-end tests exist.
4. **Add explicit migration tests.** Before touching production DB code, create integration tests that assert the metadata tuples and vector rows stay in sync (similar to the experiment file) so regressions are caught early.

With the reset you mentioned, we can reapply the insights above and retry using the incremental approach.

## Reference Planning Documents
- **Remote Embedding Required Groundwork** (`crates/ploke-tui/docs/active/plans/remote-embedding/required-groundwork.md`) – primary plan for this effort. It tracks the REVIEW questions (model switching, storage layout), enumerates affected modules (ploke-transform schema split, ploke-db helpers, ploke-embed processors), and defines the CLI/database commands, migration path, and telemetry expectations. Each slice of the redo should cite the relevant sections (schema split, trait stack updates, metadata/runtime controls, and telemetry/live gates) to demonstrate compliance.
- **Referenced sub-docs from required-groundwork.** The groundwork document links to specific modules/files (e.g., `crates/ingest/ploke-transform/.../primary_nodes.rs`, `ploke-embed/src/indexer/mod.rs`, provider crates, `/embedding …` command specs). When reimplementing, treat those references as TODO checklists—update the mentioned files and record evidence in the same doc so future reviews can trace the implementation back to the plan.
- **Agentic System Plan** (`crates/ploke-tui/docs/feature/agent-system/agentic_system_plan.md`) – still the umbrella roadmap; continue updating its logs/reports once each remote-embedding slice lands, but lean on the remote-embedding groundwork for day-to-day guidance.

Moving forward we should explicitly annotate each PR/slice with (1) the required-groundwork section it satisfies, and (2) any downstream doc it impacts (trait stack modernization, metadata propagation, telemetry), as the groundwork doc already outlines the expected commands, migrations, and testing artifacts.

## Additional Issues Not Explicitly Covered in the Plan
- **Ad-hoc rollback workflow.** When the refactor ballooned I resorted to manual `git show … > file` overwrites mid-session. The planning docs assume normal git hygiene but do not call out recovery steps. We should codify a “stop-work + branch reset” procedure (e.g., stash changes, create `backup/` diff artifacts) so partial migrations don’t leave the repo half-reverted.
- **Feature-flag strategy mismatch.** Required-groundwork describes dual-write/dual-read phases but I skipped the guard-rail (feature flag) while editing ploke-db and ploke-embed simultaneously. Future slices should explicitly include the feature flag scaffolding before touching call sites, even if the doc doesn’t spell out the macro/layout for each crate.
- **Test fixture availability.** Some integration tests (ploke-db + ploke-embed) rely on `fixture_nodes` data that doesn’t contain multi-embedding rows. The plan mentions telemetry/artifact updates but not how to seed fixtures; we need to extend `setup_db_full_embeddings` (or add new fixtures) early so tests can validate both legacy and new relations without manual DB poking.
