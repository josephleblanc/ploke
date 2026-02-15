# Panic: index out of bounds in ploke-tui App::draw

Summary:
An unexpected panic occurred at crates/ploke-tui/src/app/mod.rs: index out of bounds: the len is 5 but the index is 5. This happened when rendering the chat view after recent refactors to support external conversation scrolling state.

Root cause:
- The renderer computes per-message heights for the current path, then uses the currently selected index (from a persisted ListState) to calculate minimal-reveal scrolling.
- If the chat path shrinks (e.g., due to state changes) and the stored selection index points past the end of the current path, we attempt to index `heights[selected_index]` with `selected_index == path.len()`, causing an out-of-bounds panic.
- Specifically, we were using `self.list.selected()` directly without clamping or validating it against the latest `path.len()`.

Fix:
- Clamp the selected index every frame against the current `path.len()` before using it in any calculations:
  - `selected_index = min(selected_index, path.len().saturating_sub(1))`
- Use this clamped index for measuring and reveal/scroll logic.

How to avoid in the future:
- Always validate or clamp persisted UI indices (selection, cursor, scroll positions) against the current data length after any state changes that can alter the list size.
- Prefer computing dependent rendering indices from the latest source of truth each frame, and only persist UI-local state that cannot go stale without validation.
- Consider syncing the selection explicitly (e.g., to the last item) when the current selection becomes invalid, if that matches the intended UX.

Plan updates (short):
- Completed: external offset rendering, baseline selection-to-viewport minimal reveal, and now a safety clamp for selection index.
- Next:
  - Implement free-scrolling input: mouse wheel and key bindings (Ctrl+n/Ctrl+p for line; J/K for page; gg/G for top/bottom).
  - Add optional auto-follow refinement (stick to bottom when at end or when last is selected).
  - Optional: visual scrollbar for conversation area.
- Testing focus:
  - Shrinking path while selection is at/after the last item.
  - Rapid message arrival/removal while scrolling and navigating.
