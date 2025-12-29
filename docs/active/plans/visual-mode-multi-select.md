Title: Visual mode multi-select (line-mapped)
Status: Draft

Overview
Introduce a Vim-style Visual mode for selecting multiple message lines in the conversation view.
Selection is line-based (post-wrapping) using a line mapping built from the rendered layout.
The selection can then be copied to the OS clipboard with `y`.

Goals
- Add a real `Mode::Visual` with clear UI mode indicator.
- Enable line-based selection across messages using line mapping.
- Copy selected lines to clipboard with `y`.

Non-goals
- No mouse-based selection.
- No rectangle/block selection.
- No changes to how messages are stored in the chat model.

User experience
- `Shift+v` in Normal mode enters Visual mode and anchors selection on the current line.
- `j/k` or arrow navigation moves the head line; the highlight expands/contracts accordingly.
- `y` copies the selected lines to clipboard; exits Visual (same as Vim).
- `Esc` exits Visual without copying.

Approach (line mapping)
- Build a line map each render pass using existing wrapping/highlighting logic.
- Map global visual line index -> (message index, line index within message).
- Store `VisualSelection { anchor_line, head_line }` in App state.
- Provide helpers to:
  - Convert from current message selection to a line index.
  - Compute selected line ranges across messages.

Data model changes
- Add `Mode::Visual` to `crates/ploke-tui/src/app/types.rs`.
- Add `visual_selection: Option<VisualSelection>` to `App`.
- Add `VisualSelection` struct in `crates/ploke-tui/src/app/types.rs` or `app/mod.rs`.

Input handling
- In `crates/ploke-tui/src/app/input/keymap.rs`:
  - `Shift+v` (KeyCode::Char('V')) -> Switch to Visual mode.
  - In Visual mode, `j/k` and arrow keys move the head line via line map.
  - `y` copies selection (Action::CopyVisualSelection).
  - `Esc` exits Visual.

Rendering
- Extend conversation rendering to highlight selected lines:
  - Compute selection span in global line indices (min..=max).
  - While rendering each wrapped line, check if its global index is in selection.
  - Apply background style for selected lines (distinct from "current" marker).

Copy behavior
- Build clipboard text by concatenating selected lines in visual order.
- Use wrapped lines (post-render) so copied content matches visible selection.
- Exclude tool button rows.

Edge cases
- Selection across tool payload messages still uses rendered text only.
- Selection while overlay is active is ignored (consistent with other inputs).
- If wrapping changes (terminal resize), keep anchor/head as global line indices
  and clamp to valid range; recompute selection on next render.

Tasks
1) Add `Mode::Visual` and `VisualSelection` types.
2) Add line map generation to conversation rendering path.
3) Implement Visual input actions (enter/exit, move head, copy).
4) Implement line-based highlight in render.
5) Implement clipboard copy for selected lines.
6) Update help text and docs.
