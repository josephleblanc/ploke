# Store Worker Report - Run Attempt Metadata - 2026-05-11

Task: `store-load-run-attempt-metadata`

Changed files:

- `crates/ploke-tree/src/store/evidence.rs`
- `crates/ploke-tree/src/store/fs.rs`
- `crates/ploke-tree/src/lib.rs`

Implemented:

- Added `PassiveEvidence::run_attempts` with `RunAttemptEvidence` and
  `RunAttemptSummary`.
- `FsRunStore` now discovers and deserializes:
  - `nodes/*/runner-request.json`
  - `nodes/*/runner-result.json`
  - `nodes/*/invocations/*.json`
- Loaded records use existing typed DTOs:
  `RunnerRequestRecord`, `RunnerResultRecord`, and `InvocationRecord`.
- Keys are run-root-relative paths, so downstream consumers do not need to
  rediscover files.
- No production `serde_json::Value` reader was added.

Verification reported by worker:

- `cargo test -p ploke-tree fs_run_store_loads_run_attempt_evidence 2>&1 | tail -n 80`
- `cargo check -p ploke-tree`
- `cargo test -p ploke-tree 2>&1 | tail -n 80`: passed, 37 passed / 3 ignored

Handoff:

- `nodes/*/results/*.json` remains unloaded because the survey marks its
  passive shape unresolved.
- Graph ingestion does not consume `PassiveEvidence::run_attempts` yet.
