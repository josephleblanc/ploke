# Plan

Capture concrete diagnostics for the indexing failure while keeping the current panic, then
reproduce and codify the root cause in a targeted test before implementing a minimal fix.

## Requirements
- Preserve the current panic behavior during the diagnostic run.
- Add logging that captures failure context sufficient to identify the problematic snippet(s),
  including model metadata.
- Produce a minimal reproduction and a failing live test for the confirmed cause.
- Implement the fix and validate indexing `syn_parser` succeeds.

## Scope
- In: richer logging around embedding failures; rerun indexing to capture evidence; repro test;
  fix; regression test.
- Out: changing panic semantics now; broad retry/fallback strategy changes.

## Files and entry points
- `crates/ingest/ploke-embed/src/indexer/mod.rs` (failure handling + batch context)
- `crates/ingest/ploke-embed/src/providers/openrouter.rs` (request metadata / errors)
- `crates/ploke-tui/logs/` (diagnostic log review)

## Data model / API changes
- None.

## Action items
[x] Add structured logging on embedding failure (snippet text, lengths, file paths, byte ranges,
    model id, request dimensions, batch size, etc.).
[x] Rerun indexing for `crates/ingest/syn_parser` and capture logs.
[x] Identify and save the exact failing snippet(s) and construct a minimal reproduction request.
[x] Add a failing live network test (OpenRouter) behind a cfg gate for live API tests that sends
    the failing snippet and asserts the concrete failure mode.
[x] Implement the minimal fix by truncating snippets in the OpenRouter backend using a configurable
    policy (default Truncate) and add a unit test for truncation (done), then re-run the live repro
    test and re-index `syn_parser` (pending).
[x] Rerun the live test and re-index `syn_parser` to confirm resolution.

## Testing and validation
- Re-run `ploke-tui` indexing on `crates/ingest/syn_parser` and inspect logs.
- Add a cfg-gated live API test that reproduces the failure and verify it passes after the fix.

## Risks and edge cases
- Logging sensitive code content; acceptable for this temporary diagnostic pass.
- Provider variability; keep the repro test close to the live request shape.

## Open questions
- Should snippet logging be redacted after the reproduction is captured?
- Should we retain the live test long-term or convert to a mocked regression test?
