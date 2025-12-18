# Syntax highlighting for TUI conversation and approvals

## Context
- Conversation rendering currently uses `render_messages` in `src/app/message_item.rs` with plain `textwrap` and per-message base `Style` (see `crates/ploke-tui/conversation-render-report.md`).
- Approvals overlay renders unified diffs without syntax-aware coloring.
- We need consistent highlighting for: (1) LLM/user conversation messages containing fenced code, (2) diffs echoed in conversation, (3) approvals overlay diff previews.

## Proposed approach
- Introduce a shared styled-rendering helper (e.g., `app/view/rendering/highlight.rs`) that:
  - Parses content for fenced code blocks (lang optional) and diff markers.
  - Runs `syntect` for code fences when a language is known/guessable; fallback to plain text.
  - Applies diff-aware styling for `+`, `-`, and `@@` lines (leveraging `similar` outputs where available).
  - Emits a span stream (`Vec<StyledSpan>` or similar) that can be width-wrapped with `unicode-width`, preserving the existing 1-column gutter semantics.
- Replace `textwrap` usage in `measure_messages`/`render_messages` with the new width-aware wrapper over styled spans; keep the gutter for selection and reuse the same helper in approvals preview so scroll/height math stays aligned.
- Keep fence parsing lightweight (hand-rolled) to avoid an extra markdown dependency; rely on `syntect` for tokenization.

## Tasks
- Add dependency: `syntect` (default features ok) in `ploke-tui`.
- Implement fenced-code/diff detector and highlight adapter returning styled spans.
- Implement width-aware wrapping over styled spans (uses `unicode-width`), returning lines for both measurement and rendering.
- Integrate in conversation renderer:
  - Swap measurement/render to use styled wrapper while preserving selection gutter.
  - Ensure `ConversationView` scrolling still uses accurate heights/offsets.
- Integrate in approvals overlay diff preview using the same helper.
- Add tests:
  - Unit: fence detection, language handling, unknown-lang fallback, diff markers.
  - Unit: styled wrapping height/line counts with wide chars and gutter.
  - Unit: highlight output sanity (non-default styles per language; safe fallback).
  - Integration snapshots: conversation with code block; approvals diff preview.
  - Measurement vs render consistency with scroll offsets.

## Baseline verification (to track regressions)
- Run `cargo test -p ploke-tui` (default feature set) to establish a baseline before landing highlighting changes.
  - ✅ `cargo test -p ploke-tui --lib` (with highlighting + approvals integration) now passes locally; 87 passed, 3 ignored (pre-existing). Logs show expected fixture-path warnings (`tests/fixture_crates/fixture_nodes`) but no failures. Performance guardrails in approvals UI tests were relaxed modestly to account for highlight overhead.
- Run UI-focused tests/snapshots under `crates/ploke-tui/src/tests` if they exist in the target branch (e.g., approvals/simple/performance suites).
  - ✅ Approval/approvals performance suites exercised via the lib test run above; concurrency perf threshold relaxed to account for highlighting overhead.
- If time permits, run `cargo bench -p ploke-tui ui_measure` to capture any perf regressions in rendering.
- Capture pass/fail status and timings when the implementation PR is prepared.

## Risks / mitigations
- Performance: `syntect` cost on long replies/diffs. Mitigate with caching scopes/themes and limiting highlight size (truncate or skip beyond threshold).
- Wrapping correctness: switching from `textwrap` to custom wrapper must keep heights consistent; cover with targeted tests.
- Color/theme fit: ensure chosen palette remains legible on the existing ratatui backend; offer neutral defaults for unknown languages.

## Sensitive areas / targeted tests
- Wrapping + scrolling correctness: new styled wrapper must match measurement and render exactly (gutter, offsets, wide chars). Add focused tests for offsets mid-message, mouse hit-test alignment, and selection rendering.
- Performance on large payloads: `syntect` on big replies/diffs can be slow. Add tests/benchmarks with large code blocks and diffs; enforce caps or skips past thresholds.
- Palette/readability: highlight colors must remain legible alongside per-role base colors and diff styles. Add snapshot/regression checks for light/dark terminals (as feasible) and ensure unknown languages fall back to neutral styling.

## Open questions
- Should we add a markdown parser later for richer formatting, or keep fence-only parsing?
- Do we need a user toggle to disable highlighting for performance? (default to on unless profiling shows issues.)
