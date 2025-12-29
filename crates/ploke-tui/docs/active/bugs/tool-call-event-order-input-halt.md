# Tool-call event ordering stalls input

## Summary
- Symptom: UI continues rendering (cursor blinks) but keystrokes stop updating the input box.
- Latest evidence: `crates/ploke-tui/logs/ploke_20251228_112956_100525.log` shows tool-call lifecycle warnings and unexpected system events.
- Likely impact: tool-call bursts can starve UI input and observability records tool calls out of order.

## Evidence
- `WARN ploke_tui::app::events: ... Unused system event in main app loop: ToolCallRequested`
- `WARN ploke_tui::observability: record_tool_call_done ... Cannot record completion without a prior requested row`
- `WARN ploke_tui::file_man: FileManager received unexpected event: System(ToolCallRequested ...)`

## Root cause
- `SystemEvent::ToolCallRequested` is routed to the background channel while `ToolCallCompleted/Failed`
  are realtime. Observability can process completion before the requested row exists, causing an
  invalid lifecycle transition and repeated warnings.
- The app loop also consumes these system events and logs warnings, which adds overhead during
  heavy tool-call activity.

## Fix
- Route `ToolCallRequested` through the realtime channel to preserve ordering with terminal events.
- Ignore tool-call system events in the main app loop to avoid noisy warnings and reduce UI work.

## Regression test
- `tool_call_requested_uses_realtime_channel` in `crates/ploke-tui/src/event_bus/mod.rs` asserts that
  ToolCallRequested events are emitted on the realtime channel (fails when routed to background).

## Related docs
- `crates/ploke-tui/INDEXING_FREEZE_INVESTIGATION.md` (similar UI stall symptom: input not processed).
