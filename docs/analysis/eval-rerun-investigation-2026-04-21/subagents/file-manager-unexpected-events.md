# FileManager Unexpected LLM Events

## Conclusion

This warning is **expected-but-noisy**, not a state-management bug that affects eval correctness.

The underlying cause is coarse event-bus fanout: `FileManager` subscribes to the entire background `AppEvent` stream, while `ChatCompletion::Request` and `ChatCompletion::PromptConstructed` are also routed to that background stream. `FileManager` does not handle those variants, so it logs them as "unexpected" and does nothing else. Because the bus is `tokio::broadcast`, `FileManager` seeing the event does **not** consume it away from the LLM manager or the eval harness. The turn continues normally.

## Artifact Evidence

- The April 21 `run single agent` execution is [execution-log.json](/home/brasides/.ploke-eval/runs/BurntSushi__ripgrep-2209/runs/run-1776814500568-structured-current-policy-245b2fa1/execution-log.json:1), which identifies the run arm as `run single agent` and the output directory used for the investigation.
- The runtime log shows the exact warning for the LLM request event at [ploke_eval_20260421_163458_2.log](/home/brasides/.ploke-eval/logs/ploke_eval_20260421_163458_2.log:19).
- The same log shows the warning again for `PromptConstructed` at [ploke_eval_20260421_163458_2.log](/home/brasides/.ploke-eval/logs/ploke_eval_20260421_163458_2.log:85).
- The recorded turn trace for that same run shows the matching `Request` event at [agent-turn-trace.json](/home/brasides/.ploke-eval/runs/BurntSushi__ripgrep-2209/runs/run-1776814500568-structured-current-policy-245b2fa1/agent-turn-trace.json:10).
- The trace then shows `PromptConstructed` for the same `parent_id` at [agent-turn-trace.json](/home/brasides/.ploke-eval/runs/BurntSushi__ripgrep-2209/runs/run-1776814500568-structured-current-policy-245b2fa1/agent-turn-trace.json:65).
- The same trace proceeds into tool execution immediately after prompt construction, with the first tool request at [agent-turn-trace.json](/home/brasides/.ploke-eval/runs/BurntSushi__ripgrep-2209/runs/run-1776814500568-structured-current-policy-245b2fa1/agent-turn-trace.json:72). That is the important correctness signal: the turn is not stalled or mispaired by the warning.
- The log also shows `process_llm_request` building and sending the actual chat request after the warning, starting at [ploke_eval_20260421_163458_2.log](/home/brasides/.ploke-eval/logs/ploke_eval_20260421_163458_2.log:86). This confirms the warning does not block prompt submission.

## Code Path

- `FileManager` is explicitly subscribed to the background event channel in [crates/ploke-tui/src/lib.rs](/home/brasides/code/ploke/crates/ploke-tui/src/lib.rs:252).
- `FileManager::handle_event` only handles a few `SystemEvent` variants and otherwise warns on everything else in [crates/ploke-tui/src/file_man.rs](/home/brasides/code/ploke/crates/ploke-tui/src/file_man.rs:53) and [crates/ploke-tui/src/file_man.rs](/home/brasides/code/ploke/crates/ploke-tui/src/file_man.rs:141).
- `AppEvent::priority()` routes `ChatCompletion::Request` to `Background`, and all non-response LLM events, including `PromptConstructed`, also fall through to `Background` in [crates/ploke-tui/src/lib.rs](/home/brasides/code/ploke/crates/ploke-tui/src/lib.rs:406) and [crates/ploke-tui/src/lib.rs](/home/brasides/code/ploke/crates/ploke-tui/src/lib.rs:412).
- The event bus uses `tokio::broadcast`, so every background subscriber gets its own copy of the event in [crates/ploke-tui/src/event_bus/mod.rs](/home/brasides/code/ploke/crates/ploke-tui/src/event_bus/mod.rs:171) and [crates/ploke-tui/src/event_bus/mod.rs](/home/brasides/code/ploke/crates/ploke-tui/src/event_bus/mod.rs:185).
- The user message path emits `ChatCompletion::Request` from chat-state handling in [crates/ploke-tui/src/app_state/handlers/chat.rs](/home/brasides/code/ploke/crates/ploke-tui/src/app_state/handlers/chat.rs:323).
- Prompt construction emits `ChatCompletion::PromptConstructed` from RAG/context assembly in [crates/ploke-tui/src/rag/context.rs](/home/brasides/code/ploke/crates/ploke-tui/src/rag/context.rs:225) and [crates/ploke-tui/src/rag/context.rs](/home/brasides/code/ploke/crates/ploke-tui/src/rag/context.rs:272).
- The LLM manager listens on both realtime and background channels, then pairs `Request` with `PromptConstructed` by `parent_id` in [crates/ploke-tui/src/llm/manager/mod.rs](/home/brasides/code/ploke/crates/ploke-tui/src/llm/manager/mod.rs:115) and [crates/ploke-tui/src/llm/manager/mod.rs](/home/brasides/code/ploke/crates/ploke-tui/src/llm/manager/mod.rs:223).
- The eval runtime harness also drains both realtime and background channels and records these same events into the turn artifact in [crates/ploke-eval/src/runner.rs](/home/brasides/code/ploke/crates/ploke-eval/src/runner.rs:2178), [crates/ploke-eval/src/runner.rs](/home/brasides/code/ploke/crates/ploke-eval/src/runner.rs:3196), and [crates/ploke-eval/src/runner.rs](/home/brasides/code/ploke/crates/ploke-eval/src/runner.rs:3421).

## Cause -> Effect Narrative

1. The benchmark prompt is added to chat, which emits `AppEvent::Llm(ChatCompletion::Request)` on the shared event bus.
2. `AppEvent::priority()` classifies that request as `Background`, so the broadcast goes to every background subscriber.
3. `FileManager` is one of those subscribers, but it only meaningfully handles file-related `SystemEvent`s. It therefore falls through to its generic warning arm and logs `FileManager received unexpected event`.
4. Independently, the LLM manager receives the same broadcast copy, stores the request, then later receives `PromptConstructed` and starts `process_llm_request`.
5. The eval runtime harness also receives and records both events, which is why the turn trace shows `Request`, then `PromptConstructed`, then tool activity.

So the warning reflects **subscription scope**, not corrupted state. The only demonstrated effect is log noise and possible operator confusion when reading eval logs.

## Correctness Assessment

- **Not a FileManager state bug:** the fallback arm only logs; it does not mutate shared chat or routing state.
- **Not an eval-routing failure:** the same run artifact proves the LLM manager and harness still observe and act on the events.
- **Real issue:** background-channel multiplexing is too broad for `FileManager`'s warning policy. The current warning text makes routine background events look exceptional.

If this should be cleaned up, the smallest fix is to stop warning for unrelated non-file events in `FileManager`, or to narrow what `FileManager` subscribes to. That would improve observability hygiene, but the current behavior does not by itself invalidate the BurntSushi__ripgrep-2209 eval result.
