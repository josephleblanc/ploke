Here are the key issues and concrete, low-friction improvements I recommend for crates/ploke-tui/src/app/mod.rs.

Correctness and UX bugs
- gg/G behavior is inverted vs the help text and also mixes selection vs scroll:
  - Current: gg selects Top but scrolls to bottom; G selects Bottom but scrolls to top.
  - Fix:
    - gg: select Top and scroll to top.
    - G: select Bottom and scroll to bottom.
  Brief change:
    - In handle_normal_mode:
      - gg branch: send NavigateList::Top; set convo_offset_y = 0.
      - G branch: send NavigateList::Bottom; set convo_offset_y = u16::MAX (will clamp to bottom on draw).
    - Alternatively, update the HELP_COMMANDS text to match if you prefer the current behavior.
- Paste events are ignored. Users expect pasted text to enter the input buffer.
  - In Event::Paste(s): append s to input_buffer and mark needs_redraw = true.
- Page scrolling with Shift-J/K moves by only 1–5 lines. That’s not “page”.
  - Use viewport_height.saturating_sub(1) (or a larger chunk) for page size.
- Cursor visibility: You import Hide/Show but never use them. Today you “hide” the cursor by not setting it. For consistent behavior across terminals, explicitly hide/show on mode switches or around draw.

State sync and event handling
- Mouse selection sends multiple incremental NavigateList commands to reach the clicked index. This is both noisy and can desync.
  - Add a dedicated StateCommand::SelectIndex(usize) or StateCommand::SelectMessage(Uuid) that directly sets current. Use it in mouse selection and gg/G jumps.
- You only call sync_list_selection on MessageUpdated/UpdateFailed. Selection changes caused by branch/list navigation should also trigger a UI sync.
  - Make state_manager emit a small AppEvent (e.g., AppEvent::Ui(UiEvent::SelectionChanged)) and handle it similarly to MessageUpdated.
  - Or optimistically update self.list locally on navigation, then reconcile on events.
- App event stream errors are ignored. If the broadcast channel closes or lags, you silently stall.
  - Handle Err(Lagged(_)) by logging and continuing; handle Err(Closed) by resubscribing or exiting the UI loop cleanly.

Rendering and performance
- Avoid cloning message content every frame. RenderableMessage holds an owned String.
  - Change RenderableMessage to borrow: content: Cow<'a, str> or &'a str and plumb lifetimes through measure_messages/render_messages.
  - Or make measure/render accept &[&Message] directly.
- Unify text measurement cache. You already compute heights via measure_messages; also cache per (msg_id, width) to avoid re-wrapping on every draw when nothing changed (message_item.rs hints at pre-wrapped lines).
- The “magic number” 6 in conversation_width = chat_area.width.saturating_sub(6) is undocumented. Replace with a named constant or derive it from known paddings/gutter width so future layout changes don’t drift.
- Scrollbars:
  - You maintain convo_scrollstate/input_scrollstate but don’t render them (chat scrollbar) or don’t keep them in sync (input).
  - Update convo_scrollstate = ScrollbarState::new(total_height as usize).position(self.convo_offset_y as usize); then render the scrollbar next to chat_area.
  - If you keep input_scrollstate, render it (or remove it until you do).
- Active model flash indicator depends on time but no redraw is scheduled while idle.
  - Add a low-frequency ticker (e.g., tokio::time::interval(Duration::from_millis(100))) in the select! that sets needs_redraw = true while the indicator is active (elapsed < 2s).
- Mixed logging frameworks: you use tracing::* and log::debug. Pick tracing and replace log::debug! with tracing::debug!.

Input handling and cursor math
- Cursor column uses last line .len() and a trailing whitespace hack. That’s fragile and wrong for wide Unicode/graphemes.
  - Track a logical cursor index (byte or char) and compute visual column with unicode-width; recompute row/col from wrapped lines each draw. Drop is_trailing_whitespace hacks.
- input_vscroll is mostly unused; either wire it to real vertical scroll logic or remove until needed.

Structure and clarity
- draw is large. Split into small helpers:
  - compute_layout, update_viewport_offset, render_chat, render_input, render_status, render_model_indicator.
- Event handling block in run is large. Move AppEvent handling into a dedicated method (handle_app_event) for readability.
- model indicator “always show” booleans: show_indicator is always true; remove the conditional or make it a configuration toggle.

Error handling and resilience
- crossterm::EventStream next() can yield None or Err. You only handle Some(Ok(_)). At least log/ignore the error variants to avoid silent stalls.
- Keep disabling of terminal modes in a Drop guard (RAII) to ensure cleanup on panic.

Minor cleanups
- Remove unused imports: Hide, Show, toml::to_string, ratatui::widgets::Wrap, tracing::instrument (and any others).
- Replace block_in_place + block_on for reading config with an async read you await before draw, or accept the blocking read but add a comment acknowledging the cost. The current pattern is ok for a quick read of an RwLock, but it’s atypical inside the UI loop.

Concrete patches (brief)
- Fix gg/G:
  - gg: send NavigateList::Top; self.convo_offset_y = 0; pending_char = None.
  - G: send NavigateList::Bottom; self.convo_offset_y = u16::MAX; pending_char = None.
- Add a direct selection command:
  - enum StateCommand { SelectIndex(usize), /* ... */ }
  - On mouse click: self.list.select(Some(target_idx)); self.send_cmd(StateCommand::SelectIndex(target_idx));
- Handle Paste:
  - Event::Paste(s) => { self.input_buffer.push_str(&s); self.needs_redraw = true; }
- Scrollbar state:
  - self.convo_scrollstate = ScrollbarState::new(self.convo_content_height as usize).position(self.convo_offset_y as usize);
  - Render a vertical scrollbar next to chat_area.
- Replace RenderableMessage.content: String with Cow<'a, str> and plumb through measure/render.

If you want, I can propose concrete diffs for any subset (e.g., gg/G, selection command, paste, scrollbar wiring) and keep each change minimal.
