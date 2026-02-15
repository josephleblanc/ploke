Production Readiness Notes — 2025-08-26 20:47:58Z

Highlights
- Security: Validate tool inputs (file paths, canon) strictly; honor roots/symlink policies in ploke-io; never write outside workspace roots.
- Performance: Avoid blocking UI; spawn background tasks; cache provider metadata; cap context history.
- Observability: Persist session traces, tool lifecycles, retrieval events, proposal/apply results, usage/cost; surface in TUI overlays.
- Reliability: Add idempotent DB updates with NOW snapshots; retry with backoff where safe; explicit timeouts.
- Config: Gated external tests and live API usage; pin known cheap tool‑capable providers; expose safety toggles.
- UX: Provide fast revert path (git branch), open‑in‑editor; ensure diffs are legible.

Suggestions
- Add a cargo feature `ci-sandbox` to automatically ignore DB‑dependent tests.
- Add a lint pass in CI for unsafe fs usage patterns.
- Set up criterion benches for BM25 rebuild and dense search in isolation.
- Add a minimal compat layer for ratatui snapshots to ensure UI rendering regressions are caught.

