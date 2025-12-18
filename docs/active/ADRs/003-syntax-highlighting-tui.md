# ADR 003: Syntax Highlighting for TUI Conversation and Approvals

## Status
ACCEPTED (2025-12-17) 

- signed off by JL

## Context
- Conversation rendering (`render_messages` in `src/app/message_item.rs`) paints plain wrapped text with only per-speaker base colors; fenced code blocks and diffs are unhighlighted.
- Approvals overlay renders unified diffs without syntax-aware coloring.
- Current wrapping uses `textwrap` on raw strings, so there is no styled-span-aware measurement; scrolling relies on accurate height calculations.

## Decision
- Adopt `syntect` in `ploke-tui` for syntax highlighting of fenced code blocks (language optional; fallback to plain text).
- Introduce a shared rendering helper (e.g., `app/view/rendering/highlight.rs`) that:
  - Detects fenced code blocks and diff markers.
  - Runs `syntect` on code fences; applies diff-aware styling for `+`, `-`, `@@`.
  - Emits styled spans that a width-aware wrapper (using `unicode-width`) can wrap while preserving the existing 1-column gutter semantics for selection.
  - Parses fences with a stack-aware, hand-rolled parser (no markdown dep) that tolerates indents, nested/longer inner fences, and ignores unicode lookalikes to keep highlighting stable against LLM edge cases.
- Replace the `textwrap`-only pipeline in conversation rendering with the styled wrapper; reuse the same helper in approvals overlay diff previews to keep measurement and rendering consistent.
- Replace the hand-rolled fence parsing with a markdown parser (`pulldown-cmark`) to identify code fences and inline formatting; keep syntect for code and diff styling for non-code text.
- Add tests for markdown-driven fence detection (including inline/overlong/indented/unterminated), diff styling, wrapping correctness, highlight output sanity, and render snapshots for conversation and approvals.

## Implementation (planned)
- Add `syntect` to `crates/ploke-tui/Cargo.toml`.
- New shared highlight+wrap module producing styled lines for measurement/render.
- Update conversation renderer to use the new pipeline while maintaining scroll/offset accuracy and the selection gutter.
- Update approvals overlay diff preview to use the same pipeline.
- Add regression tests (unit + render snapshots) with emphasis on markdown-driven fences (nested/unterminated/inline/overlong), inline formatting, diff detection, and lightweight performance guards (current UI perf threshold relaxed modestly to ~20ms in the comprehensive suite to accommodate syntect).

## Consequences
### Positive
- Consistent syntax highlighting across conversation and approvals; better readability for code and diffs.
- Shared helper reduces duplication and keeps scrolling math in sync across contexts.
- `syntect` provides broad language coverage without bespoke lexers.

### Negative
- Additional dependency (`syntect`) increases build size and CPU cost; may need guardrails for very large messages/diffs.
- Custom width-aware wrapping replaces `textwrap`, so correctness must be validated.

### Neutral
- Fence-only parsing avoids markdown dependency; could be revisited later for richer formatting.

## References
- Rendering notes: `crates/ploke-tui/conversation-render-report.md`
- Plan: `docs/active/todo/2025-12-17-syntax-highlighting-tui.md`
