# ADR 006: OpenRouter Snippet Truncation Policy

## Status
Approved (2026-01-05)

## Context
- Indexing `crates/ingest/syn_parser` failed with OpenRouter 404 "No successful provider responses."
- Diagnostics showed an oversized snippet (~44k chars) being sent to the embeddings endpoint.
- Truncation implemented only in the indexer fixed the production path but did not cover other
  callers (including live tests), and risked policy coupling or drift across call sites.

## Decision
- Move truncation logic into the OpenRouter backend so all callers share the same behavior.
- Add `TruncatePolicy` to `OpenRouterConfig` with options:
  - `truncate` (default)
  - `reject`
  - `pass_through`
- Derive max snippet length from OpenRouter model context length, using the embeddings models
  fixture; fall back to a dims-based heuristic if model metadata is missing.
- Add a unit test (no network) to verify truncation behavior for a long snippet using model
  context length.

## Consequences
### Positive
- Consistent truncation across all embedding call sites, including tests.
- Explicit policy surface reduces hidden coupling and makes behavior configurable.
- Unit coverage for truncation logic without requiring network access.

### Negative
- Backend policy may surprise callers unless they opt into `pass_through` or `reject`.
- Reliance on fixture data requires keeping model metadata up to date.

### Neutral
- Truncation is a conservative fix; long-term resolution may still require chunking or upstream
  node range corrections.

## Affected Files
- crates/ingest/ploke-embed/src/providers/openrouter.rs
- crates/ingest/ploke-embed/src/config.rs
- crates/ingest/ploke-embed/src/indexer/mod.rs
- crates/ingest/ploke-embed/tests/openrouter_live_snippet_repro.rs
