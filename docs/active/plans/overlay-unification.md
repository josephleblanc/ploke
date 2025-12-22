# Overlay Unification Plan

## Goal
Unify TUI overlays under a structured system, reduce ad hoc handling, and migrate existing overlays to an intent-based command flow. Add a user-facing config overlay that supports safe, enumerated edits with optional persistence.

## Current State
- Config overlay skeleton exists (3-pane UI, help footer) with enum/bool selection only.
- Model and embedding overlays emit intents via `OverlayAction` and are dispatched by `App`.
- Footer rendering regression test added for config overlay.

## Scope
- Convert remaining overlays (context search, approvals) to the intent pattern.
- Introduce a shared overlay interface/manager for rendering, input routing, and ticks.
- Keep side effects centralized in App (or a dispatcher), not inside overlay logic.
- Expand config overlay UX to support navigation and apply/persist actions.

## Constraints
- Avoid inline text editing initially; only enumerate values with short lists (bools, enums).
- Avoid introducing concurrency hazards; runtime config should be thread-safe and updateable.
- Do not use `Box::leak` or similar patterns.

## Invariants
- Overlays do not mutate `App` directly; they emit `OverlayAction` intents.
- All overlay side effects are handled in `App` (or a dispatcher owned by `App`).
- Overlays are responsible for their local state only.
- Rendering is pure: no IO or network from render paths.
- Only one overlay accepts input at a time; input is routed exclusively to the active overlay.
- Input maps to action enums (navigation and commands); overlays do not act on raw key bindings directly.
- Lists are scrollable and always keep the selected item within the visible viewport.
- Selection is always valid; never points to a missing or filtered-out item.
- Empty states are explicit; panels never render as blank when empty.
- Focus is always visible; when space allows, show child items for the focused parent.
- Actions are idempotent where reasonable; repeated input should not double-apply.
- `Esc` closes the active overlay unless explicitly disabled for a modal confirm.
- `?` toggles help for the active overlay.
- Overlays never draw outside their assigned rect.
- Panel layout is stable; toggling help doesn't shift unrelated panels.
- Overlay state updates are synchronous and bounded per tick.
- Runtime config updates are atomic and thread-safe.
- Persist is explicit (apply vs apply+persist clearly separated).
- Filtering/searching never mutates the source list; it only affects the view.

## Phases

### Phase 1: Intent Parity
- Migrate context search overlay input handling to return intents.
- Migrate approvals overlay input handling to return intents.
- Add a small intent dispatcher for overlay side effects (if needed beyond current `handle_overlay_actions`).

### Phase 2: Overlay Manager
- Add an `OverlayManager` (single active overlay or stack).
- Define a minimal overlay interface: `render`, `handle_input`, `tick`, `on_open`, `on_close`.
- Centralize layout and footer/help rendering for consistent styling.
  - Shared "help" region that can be toggled.
  - Shared top/bottom framing and key hints.
 - Identify shared widgets to extract into reusable components.
   - Search bar with query, cursor position, and hint text.
   - Diff preview panel with unified styling and scroll behavior.
   - Empty-state panel for no results or filtered lists.

### Phase 3: Config Overlay Apply/Persist
- Add `Apply` and `Apply+Persist` actions for config overlay.
- Wire through `RuntimeConfig` update and `UserConfig::save_to_path`.
- Add validation/clamping on apply (if needed).
- Navigation/detail expansion:
  - Category list left, setting list middle, value list right.
  - Focus movement with `Tab`/`Shift+Tab` and arrows within a panel.
  - Only enumerate bool/enum values; everything else is read-only for now.
  - `Enter` applies selection for the focused setting.
  - `Ctrl+S` or `P` to persist (toggle or single-action apply+persist).
  - `?` toggles help display.

### Phase 4: Tests
- Add intent-based tests for context/approvals overlays (no insta).
- Add an overlay manager smoke test (open, input, close).
  - Add config overlay tests for help text and enum selection highlight.

## Notes
- Preserve current keyboard behavior for each overlay.
- Keep overlay state self-contained; side effects are dispatched via intents.
- Prefer minimal refactors per phase to avoid large regressions.
 - Use defaults on load (equivalent to `.unwrap_or_default()`).

## Status Tracking
- See `docs/active/todo/2025-12-21-overlay-unification.md` for day-by-day progress and open tasks.
