# Snapshot Testing Plan for TUI Rendering and Viewport Behavior

Goals:
- Prevent regressions in conversation viewport scrolling (auto-follow, snap-to-bottom).
- Verify modal overlays (Model Browser) capture input and render as intended.
- Ensure input cursor visibility rules (hidden during modal overlay) remain correct.

Scope:
- ConversationView: flicker-free typing, offset clamping, snap-to-bottom when new content arrives.
- InputView: cursor row/col, vscroll behavior, and cursor hidden when model browser is active.
- Model Browser overlay: navigation, expansion, selection, and exit.

Approach:
1) Use ratatui TestBackend with a fixed terminal size to render frames deterministically.
2) Drive App through its public actions:
   - Simulate key events -> to_action(mapping) -> handle_action.
   - Inject AppEvents (MessageUpdated, ModelSwitched) via the EventBus.
3) Capture the rendered buffer (Frame) and assert with insta snapshots.

Test Harness Outline:
- Create a small helper to build App in a headless mode with:
  - Mocked EventBus (real bus ok), in-memory AppState, and a no-op Io/File manager.
  - Deterministic provider registry (no network).
- Helper functions:
  - render_and_snapshot(name): draws one frame and snapshots the terminal buffer content.
  - push_user_message(text): sends AddUserMessage + MessageUpdated events.
  - append_system_message(text): simulates assistant/system replies and triggers MessageUpdated.
  - open_model_browser(items): bypass network; inject a prepared list.

Test Cases:
1) insert_typing_no_flicker:
   - Select an earlier long message.
   - Enter Insert mode and type characters.
   - Assert viewport offset does not jump (compare before/after snapshots).

2) submit_snaps_to_bottom:
   - In Insert or Command mode, submit a command that results in a long system message.
   - Verify the last frame shows the bottom of the new message (no cut-off).

3) message_updated_autofollow:
   - When auto_follow=true or free_scrolling=true, emitting MessageUpdated should snap to bottom.

4) delete_selected_message:
   - In Normal mode with selection on a message, press <Del>.
   - Verify the message content is gone and selection moves sensibly (snapshot).

5) model_browser_overlay_focus:
   - Open Model Browser; render and snapshot to ensure overlay title/instructions visible.
   - Ensure input cursor hidden (Mode forced to Normal for InputView rendering).
   - Navigate with j/k, expand with space/enter; snapshot expanded state.
   - Press 's' to select; assert AddMessageImmediate contains switch confirmation (can assert via captured events or a mocked sink).

6) input_history_navigation:
   - Populate user messages.
   - In Insert mode, navigate history with Up/Down, PageUp/PageDown; snapshot input box.

Infra Notes:
- Use insta with redaction helpers for dynamic UUIDs/timestamps.
- Provide deterministic widths/heights (e.g., 120x40) to stabilize snapshots.
- Consider a small shim around EventBus to collect emitted StateCommands/AppEvents for assertions.

Future Work:
- Integrate with CI and run on a minimal feature set flag.
- Add a golden master of a multi-frame interaction (typing -> submit -> response) using insta's multi-snapshot stories.
