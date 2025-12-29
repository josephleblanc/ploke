# Copy/Paste Support in the TUI

## Summary
Add first-class paste handling for the chat input and add a copy-to-clipboard action for the
currently selected message. The feature targets fast transcript reuse without changing the
multi-mode input model.

## Goals
- Paste text into the chat input (including multi-line content).
- Copy the selected conversation message to the system clipboard.
- Provide clear feedback when copy fails (clipboard unavailable).
- Keep existing keybindings and modes intact.

## Non-Goals
- Multi-selection or block selection in the conversation view.
- Clipboard history, rich text, or formatted exports.
- Mouse-driven text selection inside the TUI.
- Paste handling inside overlays (context browser, model browser) in this pass.

## UX and Keybindings
- Paste: rely on bracketed paste events from the terminal; pasted text is inserted at the end of
  the input buffer and preserves newlines.
- Copy: in Normal mode, press `y` or run `/copy` to copy the selected message.
- Feedback: after copy, emit a SysInfo message ("Copied selection") or an error message.

## Technical Design

### Event Flow
- The main event loop already enables bracketed paste via `EnableBracketedPaste`.
- Handle `Event::Paste(text)` in `App::run_with` by calling a new `App::on_paste(text)` method.
- `App::on_paste` inserts text into the input buffer and marks `needs_redraw = true`.

### Input Buffer Updates
Add a helper to centralize insert logic:
- `insert_input_text(&mut self, text: &str)` appends at the end of `input_buffer`.
- If the current mode is `Normal`, switch to `Insert` before inserting.
- If `CommandStyle::Slash` and `input_buffer` is empty and `text` starts with `/`, switch to
  `Mode::Command` and keep the leading `/` in the buffer (same behavior as typing `/`).
- If `CommandStyle::NeoVim` and `input_buffer` is empty and `text` starts with `:`, switch to
  `Mode::Command` and keep the leading `:`.
- Reset `pending_char` to avoid partial `g`-sequence state when pasting.

### Copy to Clipboard
Introduce a small clipboard adapter:
- New module: `crates/ploke-tui/src/app/clipboard.rs` with a `ClipboardWriter` trait and a
  `SystemClipboard` implementation backed by `arboard`.
- `App` owns a `ClipboardWriter` (or a lazy `SystemClipboard`) to avoid reinitializing per copy.
- Add `Action::CopySelection` and map `Normal` + `y` in `input::keymap`.
- Implement `App::copy_selected_message()`:
  - Read the currently selected message from `AppState`.
  - Use `message.content` if present; if `tool_payload` exists, prefer
    `payload.render(tool_verbosity)` so the copied text matches what users see.
  - Write to clipboard; on error, send a SysInfo error message.

### Message Selection
- Reuse the existing list selection and `ConversationView` selection index.
- If no selection exists (empty history), no-op with a SysInfo message.

### Overlay Interactions
- For this pass, ignore `Event::Paste` while an overlay is active.
- Future extension: add an `OverlayInput` enum (`Key`, `Paste`) and update overlay handlers to
  accept paste data.

## Data Flow
- `Event::Paste` -> `App::on_paste` -> `input_buffer` mutation -> `InputView` render.
- `Normal + y` -> `Action::CopySelection` -> `App::copy_selected_message` -> clipboard write.

## Error Handling and Feedback
- Clipboard failures emit `MessageKind::SysInfo` with a short error description.
- Paste errors are not expected; text is accepted as-is.

## Testing Plan
- Unit tests for `insert_input_text`:
  - Inserts into empty buffer in Insert/Command/Normal mode.
  - Slash/colon command detection on paste.
  - Preserves newlines.
- Unit test for copy:
  - Copies user/assistant content from selection index.
  - Copies rendered tool payload when present.
  - Emits SysInfo on clipboard error (use a fake clipboard in tests).
- UI snapshot/behavior tests (optional): simulate paste events and ensure input buffer updates.

## Rollout / Backout
- Rollout: default-enabled; requires no config changes.
- Backout: remove `Action::CopySelection`, clipboard module, and `Event::Paste` handling; keep
  bracketed paste enable/disable as-is.

## Decisions
- Paste in Normal mode is ignored.
- Tool payload copy omits status buttons text.
- Add a `/copy` command for non-UI environments.
