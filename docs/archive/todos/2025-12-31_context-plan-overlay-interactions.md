# 2025-12-31 ContextPlan Overlay Interactions Plan

Purpose: capture implementation ideas and next steps for improving the ContextPlan overlay
interactive UX, context accounting, and discoverability.

## Goals (interactive UX)
- Avoid selectable-but-non-interactive header rows in the list.
- Improve navigation between logical sections (messages, RAG parts, exclusions).
- Make the UI self-explaining and fluid for keyboard navigation.

## Proposed UX changes
- Replace header rows as list items with section containers:
  - Option A: render each section as its own ExpandingList, stacked vertically.
  - Option B: keep a single list but skip header rows in navigation and render them
    as non-selectable separators.
- Add section focus behavior:
  - Use Tab / Shift-Tab to move focus between sections.
  - If using a single list, add logic so Down from last row in section jumps
    to first row in next non-empty section; Up from first row jumps to previous section.
- Add a visible section indicator (e.g., "[Messages]", "[RAG parts]") in a left gutter
  or a small banner line above each list.
- Selected item UX:
  - Keep selection highlight on the item title line.
  - Do not apply selection styling to syntax-highlighted snippets.
  - Add a left gutter bar for snippet lines when the parent item is selected.

## Token accounting coverage
- Expand ContextPlan data to include token profiling from:
  - tool call request/response payloads,
  - system prompt components,
  - the RAG parts already included,
  - message history items.
- Likely data/emit touchpoints:
  - `crates/ploke-tui/src/context_plan.rs` (ContextPlan/ContextPlanSnapshot storage),
  - `crates/ploke-tui/src/rag/context.rs` (snapshot emission from prompt assembly),
  - `crates/ploke-tui/src/chat_history.rs` (source for message text/token estimates),
  - `crates/ploke-tui/src/llm/manager/events.rs` (ContextPlan* message structs).
- Present token totals in a summary block at top of overlay:
  - total tokens, subtotal by category, and percent contributions.
  - show an explanation string for how totals are computed.
- Estimated vs actual token values should be color-coded via `UiTheme`, with a
  brief tip in the help section explaining the meaning of the colors.
- Add per-item token breakdown in expanded details (already there for messages/RAG)
  and add it for tool-related items once exposed.

## Message preview line
- In expanded content for included/excluded conversation messages, add a one-line
  preview string so the user can identify which message is referenced.
- Preview should be:
  - truncated to a fixed width (based on overlay width),
  - include role prefix ("User:", "Assistant:", "Tool:").
- Source for preview should come from message text in chat history or LLM request
  message content.
- Likely UI location:
  - `crates/ploke-tui/src/app/view/components/context_plan_overlay.rs` (expanded details).

## Data/model changes needed
- ContextPlanSnapshot should include enough data to render:
  - message preview string,
  - tool call entries (including token totals),
  - optional system prompt token allocations.
- Decide whether to store raw text in snapshot or derive previews via lookups:
  - if lookups: provide ids and a back-reference to history/store.
  - if snapshot: capture lightweight previews and token counts at emit time.
- Candidate structures:
  - `crates/ploke-tui/src/context_plan.rs` (snapshot payload definition),
  - `crates/ploke-tui/src/llm/manager/events.rs` (ContextPlanMessage/ExcludedMessage).

## Implementation sketch (non-binding)
- Section rendering:
  - Build a `ContextPlanSections` struct containing per-section items.
  - Each section renders via ExpandingList and uses its own state.
  - Global overlay state holds current section index + per-section state.
- Navigation:
  - Up/Down route within current section.
  - Tab/Shift-Tab switch sections and clamp selection to available rows.
  - When moving Down at end of a section, jump to next non-empty section and
    reset selection to first item (to feel like a cohesive list).
  - Wrap behavior optional (configurable).
- Header rendering:
  - Render section headers outside of list items to avoid selection.
- Likely UI module:
  - `crates/ploke-tui/src/app/view/components/context_plan_overlay.rs`.

## Current implementations
- Overlay UI state/rendering: `crates/ploke-tui/src/app/view/components/context_plan_overlay.rs`.
- Expanding list widget + selection/scroll: `crates/ploke-tui/src/app/view/widgets/expanding_list.rs`.
- Context plan construction + snapshot emission: `crates/ploke-tui/src/rag/context.rs`.
- Context plan snapshot/history types: `crates/ploke-tui/src/context_plan.rs`.
- Context plan event types: `crates/ploke-tui/src/llm/manager/events.rs`.
- UI theme/colors: `crates/ploke-tui/src/ui_theme.rs`.

## UI theme integration
- Any new highlight color, text color, or accent color used by the overlay must
  come from `crates/ploke-tui/src/ui_theme.rs`. Expand `UiTheme` as needed and
  wire the overlay to read from it.
- Dimming for excluded items can remain as-is and does not need theme wiring.

## Overlay verification/debug output
- Add a debug render dump that can write:
  - a plain-text glyph grid (for layout/content verification), and
  - a parallel style metadata file (per-cell fg/bg/modifiers) for style inspection.

## Performance/allocations
- Avoid intermediate allocations in render path; prefer borrowing and formatting
  into fixed buffers where possible.
- Cache derived display strings (e.g., preview line) inside `ContextPlanSnapshot`
  so the overlay does not rebuild them per frame.
- Prefer small, reusable `Vec` buffers in overlay state for composed rows to
  avoid repeated allocations on every draw.
- Keep per-item token formatting in a lightweight helper that avoids `format!`
  in tight loops when possible.
- Optimizations to consider (check off each item as it is applied or explicitly
  rejected; add a brief reasoning note here in the doc to track decisions):
  - [ ] Cache `rows`/`items` per `(plan_id, filter, expanded set, snippet set, width)`
    and only rebuild when inputs change.
  - [ ] Avoid cloning in `build_rows` by iterating directly and storing indices or
    references into the snapshot rather than `collect::<Vec<_>>()`.
  - [ ] Precompute render-ready strings (headers, titles, previews) in
    `ContextPlanSnapshot` to avoid `format!` in the render loop.
  - [ ] Cache snippet highlight lines per `part_id` + width; reuse until width changes.
  - [ ] Consider a non-allocating `ExpandingList` render path using a reusable `Vec<Line>`.
  - [ ] Use `SmallVec` for short `details` collections to avoid heap allocations.

Decision log:
- (add decisions here as you check off items above)
- Current render path notes (observed hot spots):
  - `crates/ploke-tui/src/app/view/components/context_plan_overlay.rs` builds
    `rows` and `items` on every render, cloning messages/parts and allocating
    strings (`format!`) for headers and titles.
  - `build_rows` allocates intermediate `Vec`s for excluded messages via
    `collect::<Vec<_>>()` before iterating.
  - `build_display_items` allocates `Vec<Line>` per item, and for snippets calls
    `highlight_snippet_lines`, which builds a fenced string and runs syntax
    highlight each time a snippet is visible.
  - `crates/ploke-tui/src/app/view/widgets/expanding_list.rs` rebuilds a full
    `Vec<Line>` on every render; `detail_lines()` allocates per item.

## Future follow-ons (not implementing now)
- Add pin/remove actions from the overlay:
  - keybinds on selected item (pin/unpin/remove).
  - visual state for pinned/excluded items.
- Add ability to include specific files/code items:
  - open context search overlay from context plan overlay,
  - select item and add a sticky pin.
  - design a stacked overlay UX (mini context plan visible vs. mode indicator).
- Related areas:
  - `crates/ploke-tui/src/app/view/components/context_plan_overlay.rs` (keybinds/actions),
  - `crates/ploke-tui/src/app/view/components/context_search_overlay.rs` (selection + include),
  - `crates/ploke-tui/src/app/overlay_manager.rs` (stacked overlay coordination).

## Open questions
- Should token accounting show "estimated" vs "actual" in distinct columns?
- Should we include the system prompt tokens as a separate section or fold into
  the summary block?
- Should message preview lines be sourced from raw chat history or from the
  LLM request message list?

## Progress updates (2025-12-31)
- Implemented snippet gutter indicator and removed selection styling from snippet lines.
- Added debug buffer dump helpers for text + style metadata.
- Adjusted overlay sizing target to ~80% height with vertical margin.
- Snippet gutter now uses the same bar glyph as conversation history.
- Added snippet truncation marker and word-wrap-on-whitespace for snippet lines.
- Snippet gutter persists for visible snippets even when selection moves.

Next:
- Validate overlay output using the new debug dump helpers and adjust styling as needed.
