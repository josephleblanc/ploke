# Handoff: Phase 1D — Structured LLM Event Capture

**Date:** 2026-04-10  
**Context:** Phase 1D RunRecord Implementation  
**Status:** ✅ COMPLETED

## What Was Done

Implemented structured capture of LLM response events, replacing debug-string logging with typed `LlmResponseRecord` storage.

## Changes Made

### `crates/ploke-eval/src/runner.rs`

1. **Added import:**
   ```rust
   use crate::LlmResponseRecord;
   ```

2. **Extended `ObservedTurnEvent` enum:**
   ```rust
   pub enum ObservedTurnEvent {
       DebugCommand(String),
       LlmEvent(String),
       /// Structured LLM response capture (Phase 1D).
       LlmResponse(LlmResponseRecord),  // NEW
       ToolRequested(ToolRequestRecord),
       ToolCompleted(ToolCompletedRecord),
       ToolFailed(ToolFailedRecord),
       MessageUpdated(MessageSnapshotRecord),
       TurnFinished(TurnFinishedRecord),
   }
   ```

3. **Modified `handle_benchmark_event`:**
   - `ChatEvt::PromptConstructed` → captures to `llm_prompt`, logs as debug string
   - `ChatEvt::Response` → captures structured `LlmResponseRecord`
   - Other LLM events → still logged as debug strings

   ```rust
   LlmEvent::ChatCompletion(ChatEvt::Response { 
       content, model, metadata, usage, ..
   }) => {
       // Backward compat: capture content string
       artifact.llm_response = Some(content.clone());
       
       // Phase 1D: Structured capture
       let record = LlmResponseRecord {
           content: content.clone(),
           model: model.clone(),
           usage: Some(TokenUsage {
               prompt_tokens: usage.prompt_tokens,
               completion_tokens: usage.completion_tokens,
               total_tokens: usage.total_tokens,
           }),
           finish_reason: Some(metadata.finish_reason.clone()),
           metadata: Some(metadata.clone()),
       };
       artifact.events.push(ObservedTurnEvent::LlmResponse(record));
   }
   ```

4. **Added test:**
   - `handle_benchmark_event_captures_structured_llm_response`
   - Verifies: content, model, token usage, finish reason, metadata

## Benefits

| Before (Phase 1C) | After (Phase 1D) |
|-------------------|------------------|
| `LlmEvent("Response { content: \"hello\", ... }")` — debug string | `LlmResponse(LlmResponseRecord { ... })` — typed struct |
| Hard to query token usage | Easy to aggregate/analyze token usage |
| Can't deserialize reliably | Full serde support |
| Bloated with irrelevant debug info | Clean, specified fields only |

## Acceptance Criteria ✅

- ✅ Token usage captured structurally (prompt_tokens, completion_tokens, total_tokens)
- ✅ Model ID captured structurally  
- ✅ Content captured structurally
- ✅ Finish reason captured structurally
- ✅ No more debug-string LLM events for Response events
- ✅ All 34 tests pass

## Next Steps

**Phase 1E: Emission and Compression** (Est. 2 hrs)

- Wire RunRecord collection in runner
- Add `flate2` compression for `record.json.gz`
- Update `RunArtifactPaths` with `record_path`

**Phase 1F: Introspection API** (Est. 3 hrs)

- Implement `query_at_turn()`, `conversation_up_to_turn()` on `RunRecord`
- A5 (replay/introspection) marked as satisfied
