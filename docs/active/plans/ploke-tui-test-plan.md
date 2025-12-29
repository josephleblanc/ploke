# Ploke TUI Test Plan (Options 1, 3, 5 Prioritized)

## Goals
- Expand whole-app confidence using the existing TestBackend + harness.
- Add property-based tests to lock in invariants on core data structures and UI state.
- Build a focused edge-case suite to defend against layout, size, and content extremes.

## Scope
- Crate: `crates/ploke-tui`
- Primary surfaces: App loop, ConversationView, InputView, Approvals overlay, command parsing, chat history updates.

## Non-Goals (for this phase)
- Performance benchmarking and CI gating.
- External live API tests and networked LLM flows.
- Full fuzzing infrastructure (can follow later as a separate phase).

## Phase 1: Whole-App Integration Tests (Option 1)
Priority: P0

### Targets
- App loop setup + teardown without terminal side effects.
- Event routing and state transitions for indexing progress and errors.
- Overlay open/close cycles and mode switching.
- Message send + update flows with UI rendering.

### Proposed Tests
1) `tests/app_smoke_run.rs`
   - Use `TestBackend` + `RunOptions { setup_terminal_modes: false }`.
   - Spawn app with test harness, simulate a minimal event loop tick, then graceful shutdown.
   - Assert no panic, app exits cleanly, and key state fields are initialized.

2) `tests/app_event_routing.rs`
   - Emit indexing events (started -> progress -> completed) and error events.
   - Assert App receives expected UI updates (e.g., status indicators and messages).
   - Confirm event ordering does not panic or deadlock.

3) `tests/app_overlay_cycle.rs`
   - Open config overlay, approvals overlay, and close them via input events.
   - Assert overlays activate/deactivate and the base UI restores correctly.

4) `tests/app_message_flow.rs`
   - Seed a chat history with a few messages, then simulate message updates.
   - Assert ConversationView is refreshed, auto-follow behavior is correct, and no invalid scroll state appears.

### Acceptance Criteria
- Each test runs deterministically with TestBackend.
- No panics and state transitions observed are consistent with expected UI states.

## Phase 2: Property-Based Tests for Core Invariants (Option 3)
Priority: P0

### Targets
- `chat_history.rs`: `MessageUpdate::validate` transitions.
- `ConversationView` scrolling and auto-follow invariants.
- Input layout bounds in `input_box.rs` (cursor and line wrapping).

### Proposed Tests
1) `tests/prop_message_update.rs`
   - Generate random sequences of updates and statuses.
   - Assert `validate` only allows legal transitions.
   - Check completed messages stay immutable.

2) `tests/prop_conversation_scroll.rs`
   - Generate random message heights, viewport sizes, and selections.
   - After `prepare`, assert `offset_y <= max_offset` and no underflow.
   - Validate auto-follow toggling logic when last message is selected.

3) `tests/prop_input_layout.rs`
   - Generate random input strings and terminal widths.
   - Assert cursor position is clamped to content bounds.
   - Assert input height does not exceed layout constraints.

### Acceptance Criteria
- Property tests run under standard `cargo test -p ploke-tui`.
- Failing cases produce shrinking output that is actionable.

## Phase 3: Edge-Case UI Suite (Option 5)
Priority: P1

### Targets
- Extreme terminal sizes and layout boundaries.
- Very large content and tool payloads.
- Empty states and single-item lists across key components.

### Proposed Tests
1) `tests/ui_edge_terminal_sizes.rs`
   - Sizes: 1x1, 2x10, 10x2, 20x5, 120x40.
   - Render ConversationView, InputView, ApprovalsView.
   - Assert no panic and buffer is still renderable.

2) `tests/ui_edge_content.rs`
   - Single very long word, large code block, and large tool payload.
   - Render to TestBackend and assert no overflow/panic.

3) `tests/ui_edge_empty_states.rs`
   - Empty message list, empty approvals list, empty model list.
   - Assert UI renders expected placeholders or empty layouts without panics.

### Acceptance Criteria
- Edge-case suite passes in CI without special flags.
- UI renders without panics or broken state at extreme dimensions.

## Sequencing and Milestones
1) Milestone A: Add Phase 1 tests (app_smoke_run, app_event_routing, app_overlay_cycle).
2) Milestone B: Add Phase 2 property tests (message_update + conversation_scroll).
3) Milestone C: Add Phase 3 edge-case UI tests (terminal sizes + empty states).
4) Milestone D: Expand input layout property tests and edge content tests.

## Test Infrastructure Notes
- Prefer existing helpers in `crates/ploke-tui/src/test_utils/new_test_harness.rs`.
- Use `ratatui::backend::TestBackend` and `buffer_to_string` helpers.
- Keep tests deterministic: avoid real timers and async sleeps when possible.

## Risks / Open Questions
- Property test shrinkers may need custom strategies for complex UI state.
- Some UI components may require additional public helpers to be testable.
- If UI rendering depends on real-time events, consider adding test-only hooks.

## Success Criteria (Overall)
- New tests provide coverage for whole-app flows, invariants, and edge cases.
- No flaky tests; runs are deterministic and fast.
- Clear failures with actionable diagnostics.
