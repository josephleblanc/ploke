# 2025-12-30 ContextPlan Overlay Progress

Purpose: capture current progress and remaining work for the ContextPlan overlay
before context compaction.

## Done so far
- Added a reusable expanding list widget (`StatefulWidget`) for overlay use.
- Added unit tests, doc comments, and doc tests for the expanding list helper.
- Began ContextPlan overlay implementation (new overlay state + rendering + input handling).
- Added a ContextPlan history snapshot type and event emission from RAG prompt assembly.
- Wired a new keymap action (`p`) and a command (`/contextplan`) to open the overlay.
- Fixed ContextPlan overlay build errors (key handling, borrow guard, state defaults).
- Added history navigation keybinds (Shift+H/L, Shift+Left/Right) in the overlay.
- Confirmed filter labeling + included/excluded grouping in overlay rendering.
- Reused fenced-code highlighting for snippet rendering in the overlay.
- `cargo check -p ploke-tui` passes (warnings only).

## Left to do
- Manual QA: open the overlay in `ploke-tui` and verify rendering, filtering, snippet toggle,
  and history navigation behavior with real context plan snapshots.

## Implementation notes
- ContextPlan snapshots should be emitted when the prompt is constructed:
  - Use `ContextPlanSnapshot::new(context_plan.clone(), Some(rag_ctx.clone()))` when RAG runs.
  - Use `ContextPlanSnapshot::new(context_plan.clone(), None)` for fallback paths.
- The overlay should read from an in-memory `ContextPlanHistory` and support stepping through
  prior snapshots (live-updating if follow-latest is enabled).
- Snippet rendering should reuse the existing markdown highlighter used for message rendering
  (`highlight_message_lines` + `styled_to_ratatui_lines`), by wrapping snippets in fenced code blocks.
- ContextPlan filter states should be clearly labeled and cycle on `f`.

## Files touched (and what they contain)
- `crates/ploke-tui/src/app/view/widgets/expanding_list.rs`: new reusable expanding list widget + tests.
- `crates/ploke-tui/src/app/view/widgets/mod.rs`: module export for widgets.
- `crates/ploke-tui/src/app/view/mod.rs`: exported widgets module.
- `crates/ploke-tui/src/context_plan.rs`: new ContextPlan history/snapshot types.
- `crates/ploke-tui/src/lib.rs`: new AppEvent variant (ContextPlanSnapshot) and module export.
- `crates/ploke-tui/src/rag/context.rs`: emits ContextPlanSnapshot on prompt construction.
- `crates/ploke-tui/src/app/events.rs`: receives ContextPlanSnapshot and pushes into history.
- `crates/ploke-tui/src/app/mod.rs`: stores `context_plan_history` and opens overlay from action.
- `crates/ploke-tui/src/app/input/keymap.rs`: new `Action::OpenContextPlan` bound to `p`.
- `crates/ploke-tui/src/app/commands/parser.rs`: add `/contextplan` command.
- `crates/ploke-tui/src/app/commands/exec.rs`: open ContextPlan overlay on command.
- `crates/ploke-tui/src/app/overlay.rs`: overlay kind + overlay trait impl for ContextPlan.
- `crates/ploke-tui/src/app/overlay_manager.rs`: register ContextPlan overlay in manager.
- `crates/ploke-tui/src/app/view/components/context_plan_overlay.rs`: new overlay UI (WIP).
