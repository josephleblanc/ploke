# Blocker 05 – Telemetry, Evidence Artifacts & Live Gate Discipline

_Last updated: 2025-11-14_

## Problem statement
- AGENTS.md mandates “evidence-based changes” plus live gate discipline: we must persist pass/fail counts, tool-call traces, and proof that live paths were exercised under `target/test-output/...`.
- Remote embedding groundwork calls for telemetry + artifact capture (`target/test-output/embedding/…`) but no schema or ownership was defined.
- Without a clear plan, we cannot prove readiness when enabling remote embedding, nor can we audit regressions.

## Goals
1. Define artifact structure + JSON schemas for offline tests, live provider invocations, migrations, and lifecycle events.
2. Identify instrumentation points in code (EmbeddingService, EmbeddingManager, IndexerTask, CLI commands) responsible for writing artifacts.
3. Describe how gating logic consumes these artifacts to mark suites as pass/fail.
4. Ensure artifacts are hash-checked/staged via IoManager so they can be attached to release notes.

## Directory layout
```
target/test-output/embedding/
├── offline/
│   ├── unit_<timestamp>.json
│   └── integration_<timestamp>.json
├── live/
│   ├── openai_<timestamp>.json
│   └── huggingface_<timestamp>.json
├── lifecycle/
│   └── activation_<timestamp>.json
├── migration/
│   └── phaseN_<timestamp>.json
└── telemetry_index.json   // rolling manifest of latest artifacts
```
- `timestamp` is UTC `YYYYMMDDThhmmssZ`.
- `telemetry_index.json` links to the most recent entry per category to simplify gating checks.

## JSON schemas (pseudo)
### Offline unit/integration
```json
{
  "run_id": "uuid",
  "suite": "unit" | "integration",
  "workspace": "uuid",
  "embedding_set_id": "uuid?",
  "provider": "local" | "openai" | "hf" | ...,
  "results": {
    "passed": 132,
    "failed": 0,
    "ignored": 4
  },
  "artifacts": [
    "logs/embedding_manager/run_id.log"
  ],
  "git": {
    "rev": "...",
    "dirty": false
  }
}
```

### Live provider trace
```json
{
  "run_id": "uuid",
  "provider": "openai",
  "model_id": "text-embedding-3-large",
  "embedding_set_id": "uuid",
  "requests": [
    {
      "timestamp": "2025-11-14T19:32:11Z",
      "batch_size": 32,
      "dimensions": 3072,
      "latency_ms": 423,
      "status": 200,
      "tool_calls": [
        {"tool": "request_more_context", "observed": true }
      ],
      "cost_estimate_usd": 0.0123,
      "trace_id": "uuid"
    }
  ],
  "evidence": {
    "source": "EmbeddingWireRequest",
    "hash": "sha256:..."
  }
}
```
- Live gate policy: `cargo test -p ploke-tui --features live_api_tests` fails unless at least one live artifact exists for each provider flagged as “ON” in config.

### Lifecycle / activation
```json
{
  "event": "activation",
  "timestamp": "2025-11-14T19:45:00Z",
  "actor": "user",
  "previous_set": "uuid",
  "next_set": "uuid",
  "validation": {
    "rows_total": 12345,
    "rows_with_vectors": 12345,
    "hnsw_indexes": ["function", "struct"]
  },
  "commands": ["/embedding use 1234abcd"],
  "status": "success" | "failure",
  "error": null
}
```

### Migration phase
Extends Blocker 01 plan with explicit `phase` tags and row-count diffs.

## Instrumentation points
1. **EmbeddingService implementations** (Blocker 03) emit per-request telemetry using `EmbeddingWireRequest`. They call `TelemetrySink::record_live(req_summary)` whenever hitting remote HTTP.
2. **EmbeddingManager** writes lifecycle artifacts each time a context swap succeeds/fails (Blocker 04). Hooks into `/embedding use`, `/embedding drop`, etc.
3. **IndexerTask** writes migration artifacts (Phase 1 vs Phase 2) when dual-write validation runs. The CLI `cargo xtask embedding-migration --verify` collects these and ensures counts match.
4. **Test harness** (`ploke-tui/tests/embedding_live_tests.rs`) records offline/live artifact base entries and updates `telemetry_index.json`.
5. **CLI commands** attach artifact metadata to user-visible output (e.g., `/embedding status` shows “latest live check: openai_20251114T1932Z.json”).

## IoManager integration
- Artifact writers go through `IoManagerHandle::write_artifact(path, data, EvidenceMeta)` which computes SHA-256, stages to temp file, verifies expected hash is unused, and atomically moves into `target/test-output/...`.
- `EvidenceMeta` captures command, actor, and optional tool-call IDs so docs/reports can cite them.

## Gating logic
- `cargo xtask verify-embedding-gates` script reads `telemetry_index.json` and enforces:
  - `offline.unit` + `offline.integration` entries exist and reflect the current git revision.
  - For each provider with `live_gate = true` (config), a `live/<provider>_*.json` artifact exists within the past 24h.
  - Lifecycle + migration artifacts exist whenever schema changes are staged (phases from Blocker 01 doc).
- CI fails if these conditions are not met, ensuring we never claim readiness without evidence.

## Tests
- Unit test `telemetry::manifest_tests` verifies `telemetry_index.json` updates correctly when new artifacts are written.
- Integration test `embedding_live_gate_tests` sets `PLKE_EMBEDDING_LIVE=mock` and ensures gating script handles mocked artifacts.

## Open questions
1. Should artifacts be committed to git for long-term auditing or kept in `target/` only? – Recommendation: keep them in `target/` but link summaries into docs (e.g., `docs/reports/embedding_status.md`), referencing file paths + hashes.
2. Storage growth: need rota/cleanup policy (keep last N artifacts per type). IoManager helper should prune older entries beyond configurable limit.

With this schema we can confidently prove remote embedding readiness and satisfy the evidence requirements.
