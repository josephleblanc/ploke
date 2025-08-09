 # Conversation Viewport Scrolling: Design and Plan

 Goal: Add smooth, line-based scrolling for the conversation viewport that works independently of message
 selection, with mouse wheel support and optional key bindings. Scrolling should not change which message is
 “selected”; selection-based navigation still works and recenters the viewport as needed.

 ## Summary of the problem

 - Currently, the conversation view autoscrolls to keep the selected message fully visible.
 - Long messages that exceed the viewport can’t be scrolled line-by-line; users can only move between messages.
 - We need to support:
   - Mouse wheel scrolling that moves the viewport without changing selection.
   - Key-based scrolling (line and page).
   - Sensible interaction between selection navigation and free scrolling (e.g., when a selection action
 occurs, center or fully reveal the selected item).

 ## Terminology

 - Selection: Which message is selected in Normal mode.
 - Viewport: The visible window over the rendered conversation.
 - Offset (offset_y): The top line of the virtual conversation content currently visible.
 - Follow mode: Automatically stick to the bottom when new messages arrive (optional).

 ## Current behavior overview (relevant parts)

 - Rendering is in `crates/ploke-tui/src/app/message_item.rs::render_messages`.
   - Calculates per-message wrapped heights and computes `offset_y` based on the selected message to keep it
 visible.
   - It does not currently accept an external scroll offset.
 - Event handling is in `crates/ploke-tui/src/app/mod.rs::App::run`.
   - Mouse events are read in the select loop but ignored.
   - Normal-mode navigation emits `StateCommand::NavigateList` actions (selection-based).
 - Chat history path used for display comes from `AppState.chat` via `get_full_path()`.

 ## Proposed design

 ### 1) Add conversation viewport UI state (UI-only)

 Add UI-local fields to `App`:
 - `convo_offset_y: u16` — Current scroll offset in lines.
 - `convo_content_height: u16` — Total virtual height of the conversation content (computed each frame).
 - `convo_item_heights: Vec<u16>` — Per-message heights (computed each frame).
 - `convo_scroll_mode: enum { Free, AutoAlign }` — Whether to respect user free-scrolling, or auto-align to
 selected (we’ll infer this as needed rather than expose a mode toggle up-front).
 - `convo_auto_follow: bool` — Optional future enhancement: if true and user is at the bottom, keep bottom on
 new content.

 Initial rules:
 - Default `convo_offset_y = 0`.
 - While user scrolls via mouse or scroll-keys, we remain in “Free” behavior and do not recenter to selection.
 - When a selection navigation event (Up/Down/Top/Bottom) occurs, we adjust `convo_offset_y` to reveal/center
 the selected message.

 ### 2) Rendering changes

 - Move the responsibility for determining `offset_y` out of `render_messages` and into the caller
 (`App::draw`) so `render_messages` becomes a pure renderer:
   - Accept `offset_y` as an argument.
   - Return the computed `total_height` and `heights` to the caller (or accept a mutable reference to populate
 them).
 - Clamp `offset_y` within `[0, total_height.saturating_sub(viewport_height)]`.

 This allows:
 - The App to preserve `convo_offset_y` across frames even when selection is not changing.
 - Mouse scroll updates `convo_offset_y` without touching selection.

 ### 3) Input handling

 - Mouse: Handle `MouseEventKind::ScrollUp`/`ScrollDown` in `App::run` event loop.
   - Update `convo_offset_y` by ±N lines (start with 3 lines).
   - Set a “recently scrolled” flag or simply rely on not overriding `offset_y` in the renderer to prevent
 auto-centering on the next frame.
 - Keyboard (initial default bindings to discuss):
   - Line down: `Ctrl+e` (vim-ish)
   - Line up: `Ctrl+y`
   - Page down: `Ctrl+f` or `PageDown`
   - Page up: `Ctrl+b` or `PageUp`
   - These should only affect the viewport offset, not selection.
 - Do not capture these in Insert mode; only in Normal mode for now (subject to UX tweaks later).

 ### 4) Selection and scrolling interplay

 When selection changes (via `NavigateList`):
 - Compute the selected message’s top and bottom in virtual space using `heights` + prefix sums.
 - If the selected message is not fully visible within the current
 `convo_offset_y..convo_offset_y+viewport_height`, adjust `convo_offset_y` minimally to reveal it:
   - If the selected top < offset, set offset = selected top.
   - Else if selected bottom > offset + viewport_height, set offset = selected bottom - viewport_height.
 - If selection jumps to top or bottom (K/J), additionally consider:
   - For bottom jump, set `offset_y = total_height - viewport_height`.
   - For top jump, set `offset_y = 0`.

 New messages arriving:
 - If the selected message is last OR `convo_offset_y` is near the bottom (e.g., within 1 page), optionally
 auto-follow by sticking to bottom; otherwise preserve current offset (user is reading older content).
 - This is a future enhancement; initial version can simply preserve offset unless selection is on last
 message.

 ### 5) Configuration hooks (future)

 - Add keybindings to `user_config` to allow remapping scroll keys.
 - Config option to change mouse scroll granularity.
 - Config option to enable “auto-follow tail.”

 ### 6) Data and invariants

 - `convo_offset_y` must be clamped each frame against the latest `total_height` and viewport height.
 - Resizing the terminal triggers re-wrapping; we recompute heights and re-clamp offset.
 - Selection index is independent from `convo_offset_y`.

 ## Concrete next steps (incremental)

 1) Refactor renderer API
 - Change `render_messages` signature to accept `offset_y` and return `(total_height, heights)`.
 - Remove the internal auto-centering logic from `render_messages`.

 2) Introduce `App` UI state for convo scrolling
 - Add `convo_offset_y`, `convo_content_height`, `convo_item_heights`.
 - Initialize defaults in `App::new`.

 3) Draw path using external offset
 - In `App::draw`, compute heights/total via `render_messages`, store in `App`, clamp `convo_offset_y`, pass it
 back to render.

 4) Handle mouse wheel
 - In the `Event::Mouse` branch, implement:
   - On `ScrollUp`: `convo_offset_y = convo_offset_y.saturating_sub(3)`
   - On `ScrollDown`: `convo_offset_y = (convo_offset_y + 3).min(max_offset)`
 - Request a redraw (already done each loop).

 5) Add key bindings (Normal mode only)
 - Map `Ctrl+e`/`Ctrl+y` (line scroll) and `PageDown`/`PageUp` (page scroll).
 - Adjust `convo_offset_y` accordingly.

 6) Integrate with selection navigation
 - On list navigation events (Up/Down/Top/Bottom), after `self.sync_list_selection()`, compute the selected
 item’s virtual span and adjust `convo_offset_y` minimally to reveal it.

 7) Optional polish
 - Auto-follow bottom when selection is last and a new message arrives.
 - Visual scroll bar for conversation area using `Scrollbar` (now that we know content length and offset).

 ## Possible test cases (manual and automated where feasible)

 Rendering/offset
 - With a known set of messages and terminal size, verify `heights` and `total_height` are correct.
 - Scroll down beyond the last line clamps at max offset.

 Mouse
 - Mouse wheel down increments offset; wheel up decrements; selection remains unchanged.
 - Mouse scrolling works both when the selected message is on-screen and when it’s off-screen.

 Keyboard
 - Ctrl+e/y scroll exactly one line in Normal mode.
 - PageDown/PageUp scroll by viewport height minus one line (or exactly height).

 Selection interplay
 - After free scrolling to hide the selected message, pressing `j/k` reveals the selected message with minimal
 offset change.
 - Jump to top/bottom sets offset to 0/end respectively.

 Resize
 - Resize to narrower width wraps lines more; offset stays clamped and view remains sensible.

 New messages
 - When a new message arrives while the user is not at the bottom, offset is preserved.
 - When at the bottom (or selection last), offset moves to bottom (if auto-follow is enabled later).

 ## Clarifying questions

 - Should mouse wheel scrolling be active in Insert mode as well, or only in Normal mode?
    - Mouse wheel scrolling should be active in Insert mode as well. Entering a new message should make the viewport snap to the bottom.
 - Do we want page-scrolling to be exactly viewport height, or viewport height minus one line (to provide
 context)?
    - Not exactly viewport height, we want either 5 lines or 10% of the viewport height, whichever is smaller.
 - When selection changes via h/l (branch navigation, soon), should we center the selected item or just
 minimally reveal it?
    - I'm not sure exactly what you mean here, but the overall design principle is for the
    navigation via h/l to be as minimally disruptive as possible while still conveying feedback to
    the user that they have switched to a new conversation branch.
 - Should we support an “auto-follow” toggle in the UI?
    - For now let's add auto-follow as a field in the app state, then set it to true whenever the
    bottom of the most recent message is at the end of the viewport.
 - Preferred default keybindings for line/page scroll if Ctrl+e/y conflict with your workflow?
    - D o not use Ctrl+e/y, instead use Ctrl+n/p, and add the keybind gg to go to the beginning fo
    the conversation history, and G to go to the end of the conversation history. For page
    scrolling, use J/K

 ## Files we plan to modify

 - crates/ploke-tui/src/app/message_item.rs — Refactor `render_messages` to accept offset and return metrics.
 - crates/ploke-tui/src/app/mod.rs — Add UI state for scrolling, handle `Event::Mouse`, new keybindings,
 integrate offset with selection navigation.
 - crates/ploke-tui/src/chat_history.rs — No changes needed for scrolling logic (UI-only), but we may reuse
 path APIs. If we decide to render only conversation messages (User/Assistant), we may need to expose a switch.

 Potential future/config files (please add if/when we proceed):
 - crates/ploke-tui/src/user_config.rs — To define customizable keybindings and mouse scroll granularity.
 - crates/ploke-tui/src/event_bus.rs or AppEvent definitions — If we decide to emit/view events around
 following tail or scroll telemetry.

 If you want me to start implementing, please add any of the above files not already shared to the chat.

 ## Risks and mitigations

 - Performance: Rewrapping each frame can be expensive. Mitigation: We already rewrap per frame; heights
 caching is still ephemeral but OK. If needed, cache wrapping by (message_id, width).
 - UX confusion: Two scroll modes (selection vs. free). Mitigation: Clear rules; selection reveals item;
 mouse/keys free-scroll without changing selection.
 - Keybinding conflicts: Make configurable later.

 ## Telemetry/logging

 - Log scroll offset changes at trace level while developing.
 - Log when auto-follow toggles.

 ## Pseudocode snippets

 Mouse handling:
 - On ScrollDown:
   - offset_y = min(offset_y + 3, max_offset)
 - On ScrollUp:
   - offset_y = offset_y.saturating_sub(3)

 Selection reveal:
 - if selected_top < offset_y: offset_y = selected_top
 - else if selected_bottom > offset_y + viewport_height: offset_y = selected_bottom - viewport_height

 ## Task list

 - [ ] Refactor renderer to accept external `offset_y` and return `(total_height, heights)`.
 - [ ] Add `convo_offset_y`, `convo_content_height`, `convo_item_heights` to `App`.
 - [ ] Clamp `convo_offset_y` each frame based on content height and viewport.
 - [ ] Handle mouse wheel for free scrolling.
 - [ ] Add Normal-mode keybindings for line and page scrolling.
 - [ ] Adjust offset when selection changes to reveal the selected item.
 - [ ] Optional: Add scrollbar widget for the conversation area.
 - [ ] Optional: Auto-follow when at bottom or selection last.
 - [ ] Tests: Manual verification scenarios described above.
 - [ ] Feedback: Capture UX notes and iterate.

 ## Status update (as of commit fc638a3)

 - Where we are in the plan:
   - Completed:
     - 1) Refactor renderer to accept external offset and return (total_height, heights) via measure_messages/render_messages split.
     - 2) Add App UI state: convo_offset_y, convo_content_height, convo_item_heights.
     - 3) Draw path using external offset; clamp offset to [0, total_height - viewport_height] each frame.
   - In progress:
     - 6) Selection and scrolling interplay: minimal “reveal selected” logic exists, but it is currently bypassed due to sticky auto-follow.
   - Not started:
     - 4) Mouse wheel free-scrolling.
     - 5) Keyboard scrolling (Ctrl+n/p for line scroll, J/K for page scroll, gg/G for top/bottom).
     - 7) Optional polish (scrollbar, refined auto-follow behavior).

 - Expected current behavior (with the above code):
   - When the selected message is the last message or when auto-follow is active, the viewport snaps to and stays at the bottom (shows the most recent message).
   - When the selection moves away from the last message, the viewport should minimally adjust to reveal the selected message within the viewport.
   - Known deviation right now: the viewport often stays clamped to the bottom even when selection moves to earlier messages.

 - Why the deviation happens:
   - auto_follow is set true when we’re at the bottom and is never cleared on selection changes or user actions.
   - In App::draw, the condition “if selected is last OR convo_auto_follow” forces offset_y = bottom, which overrides the minimal-reveal logic for any other selection.
   - Result: selection can change, but the viewport remains pinned to the bottom (the most recent message appears; earlier selected items go off-screen).

 - Is this expected?
   - This is unintended per the plan. It’s a bug stemming from doing step 6 only partially. We split measure/render and added minimal reveal, but we did not add the rules to exit auto-follow on selection/navigation, so auto-follow remains sticky and dominates.

 - Next steps to align with the plan:
   1) Exit auto-follow when the user navigates selection to a non-last message (ListNavigation Up/Down/Top), and on any future free-scrolling (mouse/keys).
      - Implementation note: In the “Decide/adjust offset” pass, if selected_index exists and selected_index + 1 < path.len(), set convo_auto_follow = false before applying reveal logic.
   2) Only re-enter auto-follow when:
      - The selected message is the last message, or
      - The viewport is at the bottom after rendering (offset == max_offset).
   3) Then implement step 4 (mouse wheel) and step 5 (keyboard scroll) using the bindings we agreed:
      - Ctrl+n/Ctrl+p for line scroll.
      - J/K for page scroll (page = min(5 lines, 10% of viewport height)).
      - gg to go to top, G to go to bottom.
      - Mouse wheel active in both Normal and Insert modes; entering a new message should snap to bottom.

 - What to test right now:
   - With multiple long messages, select the last message: viewport should be at bottom.
   - Move selection up: currently, viewport likely stays at bottom (known bug). After next step, it should minimally reveal the selected item.

 - Clarifications (confirm we proceed as planned):
   - Exit auto-follow on any selection navigation that results in a non-last selection, and on any free-scrolling action.
   - Re-enter auto-follow when selection is last or when the viewport is snapped to bottom by the user (offset == max_offset).
   - Keep mouse wheel active in Insert mode; submitting a message should snap to bottom.

 ## Feedback log (to fill during implementation)

 - Notes on unexpected behavior:
 - Ideas for improving discoverability:
 - Performance observations:
 - Keybinding conflicts noted:
