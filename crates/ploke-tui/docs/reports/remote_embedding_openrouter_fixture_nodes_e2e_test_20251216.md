# Remote Embedding E2E: OpenRouter + `fixture_nodes` (2025-12-16)

## Why
- We were debugging remote embedding regressions by manually running `ploke-tui` and scraping logs.
- Failures included “transport error: error decoding response body” and a UI progress bar that appeared to stall.
- This adds a repeatable, artifact-producing regression test that exercises the **real** OpenRouter embeddings endpoint on a small batch, while still using the full `fixture_nodes` transform output.

## What was added
- Live-gated E2E test: `crates/ingest/ploke-embed/tests/openrouter_live_fixture_nodes_e2e.rs`
  - Builds a fresh in-memory Cozo DB from the `tests/fixture_crates/fixture_nodes` fixture (parse → module tree → transform).
  - Sets the active embedding set to an OpenRouter model + dims.
  - Seeds dummy vectors for most nodes so only a small batch hits the live endpoint.
  - Runs the embedding indexer, asserts vectors are written, and builds an HNSW index.
  - Writes a JSON artifact under `target/test-output/embedding/live/`.

## How to run (explicit, low-RPS)
```bash
export OPENROUTER_API_KEY=...
cargo test -p ploke-embed --test openrouter_live_fixture_nodes_e2e -- --ignored --nocapture --test-threads=1
```

Optional overrides:
```bash
export PLOKE_OPENROUTER_EMBED_MODEL="openai/text-embedding-3-small"
export PLOKE_OPENROUTER_EMBED_DIMS=256
```

Artifacts:
- `target/test-output/embedding/live/openrouter_fixture_nodes_e2e_<timestamp>.json`

## Debugging improvements
- OpenRouter decode failures now include `status`, `content-type`, `x-request-id` (if present), and a truncated `body_snippet` to quickly distinguish:
  - non-JSON/HTML bodies,
  - JSON error envelopes,
  - schema drift.

