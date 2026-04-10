# Handoff: Conversation Capture Design Decision

**Date:** 2026-04-10  
**Context:** Phase 1C RunRecord Implementation  
**Status:** ✅ COMPLETED — Event-based capture implemented

## What We Were Doing

Implementing Phase 1C: capturing conversation history at turn boundaries for the RunRecord.

## The Problem

Started with `capture_conversation()` reading from `state.chat.0.read().messages` and converting `Message` to a serializable form. This had issues:

1. **Code duplication** — recreating types that already exist
2. **Wrong abstraction** — `Message` is internal TUI state, not what the LLM sees
3. **TTL mutation** — `current_path_as_llm_request_messages()` requires `&mut self` because it decrements leasing TTL
4. **Not the actual request** — we want what went over the wire, not a reconstruction

## The Discovery

Through examining `crates/ploke-tui/src/app/commands/unit_tests/harness.rs`, we realized:

**The test harness provides event channels:**
- `realtime_tx_rx` — realtime events  
- `background_tx_rx` — background events
- `event_rx` — app events

**The LLM emits events through these channels** when sending/receiving messages. These are `AppEvent::Llm(LlmEvent::ChatCompletion(ChatEvt::...))` events.

## The Decision

**Capture conversation via event channels, not state.chat.**

Instead of:
```rust
// BAD: Reads state, requires mutable access for TTL, reconstructs what we think was sent
pub fn capture_conversation(state: &Arc<AppState>) -> Vec<Message> {
    let mut chat = state.chat.0.write().await;  // Blocks, mutates TTL
    chat.current_path_as_llm_request_messages()  // Reconstruction
}
```

Use:
```rust
// GOOD: Capture events as they flow through the system
async fn handle_benchmark_event(
    artifact: &mut AgentTurnArtifact,
    state: &Arc<AppState>,
    event: AppEvent,
) {
    match event {
        AppEvent::Llm(llm_event) => {
            match &llm_event {
                LlmEvent::ChatCompletion(ChatEvt::PromptConstructed { formatted_prompt, .. }) => {
                    // Capture the exact prompt sent to the LLM
                    artifact.llm_prompt = formatted_prompt.clone();
                }
                LlmEvent::ChatCompletion(ChatEvt::Response { content, .. }) => {
                    // Capture the LLM's response
                    artifact.llm_response = Some(content.clone());
                }
                _ => {}
            }
        }
        // ...
    }
}
```

## Implementation

**Changes made to `crates/ploke-eval/src/runner.rs`:**

1. **Added imports:**
   - `use ploke_llm::manager::RequestMessage;`
   - `use ploke_tui::llm::{ChatEvt, LlmEvent};`

2. **Modified `AgentTurnArtifact`:**
   - Removed: `conversation: Vec<ploke_tui::chat_history::Message>`
   - Added: `llm_prompt: Vec<RequestMessage>` — what the LLM actually saw
   - Added: `llm_response: Option<String>` — the LLM's response content

3. **Updated `handle_benchmark_event`:**
   - Now matches on `ChatEvt::PromptConstructed` to capture `llm_prompt`
   - Matches on `ChatEvt::Response` to capture `llm_response`
   - No side effects, no TTL mutation

4. **Removed:**
   - `capture_conversation()` function (no longer needed)
   - Old tests for the removed function

5. **Added new tests:**
   - `handle_benchmark_event_captures_prompt_constructed`
   - `handle_benchmark_event_captures_llm_response`

## Key Insight

The `TestRuntime` harness is the canonical way to observe the system. It gives us:
- Event subscriptions (read-only, no side effects)
- Access to actual network traffic (via events)
- No TTL mutation or state modification

## Reference

**Test harness file:** `crates/ploke-tui/src/app/commands/unit_tests/harness.rs`

**Added to AGENTS.md:** Reference to always read this file before eval work.

## Result

✅ Phase 1C now captures the actual LLM prompt (as `Vec<RequestMessage>`) and response (as `String`) via event-based capture, without requiring mutable state access or causing TTL mutations.

**Next:** Phase 1D — Structured LLM event capture (LlmEventRecord for complete request/response/raw event serialization)
