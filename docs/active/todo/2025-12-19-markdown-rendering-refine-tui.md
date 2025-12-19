# Markdown-aware rendering for TUI conversation and approvals

## Goal
Replace hand-rolled fence parsing with a markdown event pipeline (via `pulldown-cmark`) to get correct fence detection, cleaner inline formatting, and leaner allocations while keeping `syntect` for code highlighting and diff styling for non-code text.

## Plan
- **Dependency**: Add `pulldown-cmark` (default features only) to `ploke-tui`.
- **Pipeline**:
  - Parse each message once with `pulldown-cmark`, iterate events.
  - Map events to spans: `Text`/breaks -> plain; inline `Code` -> monospace style; `Emphasis`/`Strong` -> optional modifiers; `CodeBlock(info)` -> collect block text + lang hint, send to syntect; strip markdown markers.
  - Diff styling stays for text outside code blocks; inside code blocks syntect handles it.
  - Wrap after span assembly with the existing width-aware wrapper; no extra pending-text/code buffers.
  - Borrow `&str` from the parser; allocate only when building `StyledSpan`; reuse cached `SyntaxSet`/`Theme` (optionally small `HighlightLines` cache).
- **Integration**:
  - `highlight_message_lines` uses the markdown event pipeline.
  - Conversation rendering (`message_item.rs`) stays the same interface; approvals keep `highlight_diff_text` but can share markdown path if needed for markdown content.
- **Performance discipline**:
  - Single-pass event walk; no `to_string()`/Vec churn for fences.
  - Fast path: if no `CodeBlock` events, skip syntect entirely.
  - Reserve buffers when assembling spans/lines; avoid cloning info strings.
  - Keep existing UI perf guard; add a small non-failing timing check around highlight in tests/bench harness.
- **Tests**:
  - Markdown correctness: inline strong/emphasis/code rendered without raw markers; mixed-case/spacey info strings; overlong/indented/unclosed fences; inline ```text``` remains inline; zero-width chars near fences; nested fences inside code blocks.
  - Regression for Claude’s example: no color leak between sections, and plain text (e.g., “break” outside code) stays plain.
  - Diff coexistence: diff markers colored outside code blocks, not inside.
  - Performance sanity: ensure UI perf test still passes; optional micro-check on large fenced block.
- **Docs**: Update ADR 003 and the existing 2025-12-17 highlighting TODO to note the markdown-parser approach, new regression coverage, and perf expectations (no skipping/short-circuiting; just efficient pipeline).

## Risks / watchpoints
- Style choices for inline emphasis/strong need to stay legible with existing base colors.
- Large blocks still pay syntect cost; keep caches hot and avoid extra allocations to stay within perf guard.
- Ensure diff styling doesn’t bleed into code blocks when the markdown parser is present.
