# Bug report

2025-12-30

## Issue: UpdateFailedEvent "Cannot update completed message" after tool-call sequences

### Description

During a recent run, the chat log shows repeated UpdateFailedEvent errors immediately after assistant messages complete and tool messages are added. The failures indicate attempts to update already-completed assistant messages and may be correlated with tool-call handling.

### Evidence (latest run)

File: `crates/ploke-tui/logs/chat_20251230_024920_241593.log`

Tail excerpt:
```
DEBUG chat_tracing_target: Message updated successfully; dispatching MessageUpdatedEvent msg_id=3a2f2ea1-a8d2-441f-b362-9ff459512078 kind=Assistant old_status=Generating new_status=Completed
ERROR chat_tracing_target: Message update failed; dispatching UpdateFailedEvent msg_id=3a2f2ea1-a8d2-441f-b362-9ff459512078 kind=Assistant old_status=Completed error=Cannot update completed message
...
TRACE chat_tracing_target: Starting add_tool_msg_immediate
TRACE chat_tracing_target: Inserted tool message; parent=b0cbb80f-2dfb-4e6a-a7dc-433136b5a6d9 children_before=0 children_after=1 new_current=beb69f49-6903-4a42-9346-95667602c482
TRACE chat_tracing_target: Emitted MessageUpdatedEvent for tool message id=beb69f49-6903-4a42-9346-95667602c482
ERROR chat_tracing_target: Message update failed; dispatching UpdateFailedEvent msg_id=b0cbb80f-2dfb-4e6a-a7dc-433136b5a6d9 kind=Assistant old_status=Completed error=Cannot update completed message
```

### Suspected area

- Assistant placeholder updates vs. completed status in `crates/ploke-tui/src/llm/manager/session.rs`
- Message update validation in `crates/ploke-tui/src/chat_history.rs` (`MessageUpdate::validate` / `try_update`)
- Tool message insertion path in `crates/ploke-tui/src/app_state/handlers/chat.rs` (`add_tool_msg_immediate`)

### Next steps

### Progress

- Confirmed from `crates/ploke-tui/logs/message_update_20251230_024920_241593.log` that the failed
  updates are `requested_status=None`, which points to metadata-only updates landing after the
  assistant message has already transitioned to `Completed`.
- `MessageUpdate::validate` currently blocks any updates on `Completed`, including metadata.
- Implemented a fix to allow metadata-only updates on `Completed` in
  `crates/ploke-tui/src/chat_history.rs`.
- Removed the ad-hoc unit test since it did not serve as a proven reproduction.
- Verified in `crates/ploke-tui/logs/message_update_20251230_030832_247577.log` that metadata-only
  updates now succeed (`requested_status=None` followed by `message update applied` with
  `metadata="present"`). No `UpdateFailedEvent` appears in the tail.

### Next steps

- Reproduce to confirm the UpdateFailedEvent no longer triggers after tool-call chains.
- Correlate with API response logs in `crates/ploke-tui/logs/api_responses_20251230_024920_241593.log`
  if failures persist.
