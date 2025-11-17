# Remote Embedding Attempt 002 – Telemetry & Evidence Plan

Purpose: define the evidence requirements (tests, artifacts, tracing) for each slice so we can prove readiness before advancing. Every artifact must include the active feature flags and live-gate status.

## Artifact conventions

- Base directory: `target/test-output/embedding/`
- Naming scheme: `slice<N>-<topic>.json` for offline runs, `slice<N>-<topic>-live-<timestamp>.json` for live tests.
- Metadata required in every artifact:
  - `slice`: number (1-4)
  - `feature_flags`: list of enabled flags (`multi_embedding_schema`, etc.)
  - `tests`: array of `{name, status, pass_count, fail_count, ignored}`
  - `artifacts`: references to additional files (fixture hashes, logs)
  - `live`: boolean (true only when cfg(feature = "live_api_tests") was enabled and remote providers were contacted)
  - `tool_calls_observed`: count + sample IDs for live runs
  - `notes`: free-form summary with links to impl log entries
  - `flag_validation`: array summarizing the Validation Matrix commands (from `feature_flags.md`). Each entry must include the exact command, feature tier, observed outcome (`pass`, `fail`, `compile_error`, `not_applicable`), and—when no tests executed—a short `note` explaining why (e.g., “flag not wired in this crate yet”). Do not omit failing tiers—record the failure reason instead.

## Slice-specific requirements

### Slice 1 – Schema & fixtures
- **Offline tests**: `cargo test -p ploke-db multi_embedding_experiment`, `cargo xtask verify-fixtures --multi-embedding`, `cargo test -p test-utils setup_db_full_embeddings` when feature flags enabled.
- **Validation matrix**: execute every command listed in `feature_flags.md#validation-matrix` (schema/db/runtime tiers) and record the results in `slice1-schema.json`, even if a command fails or the feature is not yet wired.
- **Artifacts**:
  - `slice1-schema.json` summarizing schema tests + fixture hashes.
  - `experiment/` subfolder with Cozo query outputs showing metadata/vector parity.
  - Fixture hash files: `fixtures/<fixture_name>-hash.txt` referencing the run.
- **Live tests**: not required (no remote calls).
- **Evidence**: link to implementation log entry, attach references in `remote-embedding-slice1-report.md`.

### Slice 2 – Database dual-write/read
- **Offline tests**: `cargo test -p ploke-db --features "multi_embedding_db"` for affected modules, integration tests verifying HNSW search parity using synthetic vectors.
- **Validation matrix**: rerun the full matrix so schema/db/runtime/release tiers capture the state of dual-write changes; attach the results to `slice2-db.json`.
- **Artifacts**:
  - `slice2-db.json` capturing dual-write parity metrics (rows written to legacy vs new relations, mismatch count).
  - Query dumps showing `embedding_nodes` counts per node type.
- **Live tests**: optional; if run, include `slice2-db-live-<timestamp>.json` showing real DB operations with tool-call traces.
- **Telemetry**: add `tracing` spans around dual-write code paths to log provider/model/dimension; include span samples in artifact attachments.

### Slice 3 – Runtime/indexer
- **Offline tests**: TEST_APP harness run with `multi_embedding_runtime` enabled, unit tests for indexer tasks, CLI smoke tests for `/embedding use`.
- **Validation matrix**: include all tiers, plus runtime crate commands, in both offline and live artifacts so reviewers know how each combination behaved.
- **Live tests**: required when claiming readiness. Must run with `cfg(feature = "live_api_tests")` to exercise real provider calls (OpenRouter/local). Artifact `slice3-runtime-live-<timestamp>.json` must include:
  - Provider call metadata (model, dimensions, latency, HTTP status) without secrets.
  - Tool-call traces proving multi-set requests executed.
  - Evidence that kill switch was OFF during the run.
- **Offline artifacts**: `slice3-runtime.json` summarizing job counts, embedding set IDs, and telemetry output.

### Slice 4 – Cleanup + enablement
- **Offline tests**: full `cargo test` (workspace) with flags ON by default, `cargo xtask verify-fixtures` to ensure legacy columns absent.
- **Validation matrix**: run the matrix even after defaults flip to ON to ensure the kill switch + release bundle continue to compile/testing; archive the outputs with the soak evidence.
- **Live tests**: run `cfg(feature = "live_api_tests")` suite with `multi_embedding_release` enabled. Artifact `slice4-release-live-<timestamp>.json` must demonstrate:
  - Active embedding set toggling (`/embedding use`) without reindexing when switching back to cached sets.
  - Successful `/embedding list|drop|prune` commands.
  - Evidence that feature flags are ON by default (captured in metadata).
- **Post-soak**: record soak run summaries (duration, pass/fail counts) and attach to `slice4-release.json`. Document kill-switch status change when removed.

## Reporting workflow

1. After each slice’s test run, generate the required JSON artifacts and place them under the base directory.
2. Update `remote-embedding-slice<N>-report.md` with:
   - Links to artifacts
   - Highlighted verifications (e.g., parity counts, latency metrics)
   - Open issues
3. Reference the report + artifacts in the slice’s implementation log entry.
4. Live runs must also include sanitized logs under `target/test-output/embedding/live_logs/` for auditing (one file per provider call).

## Tools & automation

- Provide helper scripts under `xtask`:
  - `xtask embedding:collect-evidence --slice <n>` to run the standard tests and assemble JSON artifacts.
  - `xtask embedding:verify-live` to check for tool_call traces and required metadata before marking live gate green.
- Ensure these commands validate the presence of required metadata fields; fail fast if missing.

## Gate criteria

- A slice is “ready” only when its report + artifacts exist and link back to this plan.
- Live gate (Slice 3+4) cannot be marked pass without tool_call evidence and live artifact files.
- If any stop-and-test checkpoint fails, record the failure + remediation in the next implementation log update.
