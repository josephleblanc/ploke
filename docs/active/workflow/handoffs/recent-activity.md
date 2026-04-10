# Recent Activity

- last_updated: 2026-04-10
- ready_for: Context compaction and Phase 1 completion review
- owning_branch: `refactor/tool-calls`
- review_cadence: update after meaningful workflow-doc changes or handoffs
- update_trigger: update after touching workflow structure, review rules, or active artifact layout

## 2026-04-10

- **Phase 1C COMPLETE** — conversation capture refactored to use event channels
  - Removed `capture_conversation()` function that read from `state.chat` (required write lock, caused TTL mutations)
  - Modified `AgentTurnArtifact`: replaced `conversation` field with `llm_prompt: Vec<RequestMessage>` and `llm_response: Option<String>`
  - Updated `handle_benchmark_event` to capture `ChatEvt::PromptConstructed` and `ChatEvt::Response` events
  - This captures what the LLM actually sees/sends without side effects
  - All 33 tests pass
  - Reference: [2026-04-10_conversation-capture-design.md](./2026-04-10_conversation-capture-design.md)

- **Phase 1D COMPLETE** — structured LLM event capture
  - Added `LlmResponse(LlmResponseRecord)` variant to `ObservedTurnEvent` enum
  - Modified `handle_benchmark_event` to capture structured data from `ChatEvt::Response`
  - Captures: content, model, token usage (prompt/completion/total), finish reason, full metadata
  - No more debug strings for Response events — all data is structured
  - Added test: `handle_benchmark_event_captures_structured_llm_response`
  - All 34 tests pass (1 new test added)

- **Fixed pre-existing test failures in ploke-tui**
  - `schema_guidance_mentions_method_targets`: Updated assertion to match actual schema description
  - `de_to_value` (request_code_context): Fixed test expectation to match implementation typo ("guide" → "guides")
  - Both schema tests now pass

- **Phase 1E COMPLETE** — RunRecord emission and compression
  - Added `flate2` dependency for gzip compression
  - Implemented `write_compressed_record()` and `read_compressed_record()` helpers
  - Wired RunRecord collection in `RunMsbAgentSingleRequest::run`:
    - Initialize at run start: `RunRecord::new(&prepared)`
    - Capture turn data after `run_benchmark_turn()` completes
    - Emit `record.json.gz` at end of run
  - `RunArtifactPaths.record_path` now populated with path to compressed record
  - Added tests: `write_and_read_compressed_record_roundtrip`, `compressed_record_achieves_compression_ratio`

- **Phase 1F COMPLETE** — Introspection API
  - Implemented 9 introspection methods on `RunRecord`:
    - `timestamp_for_turn()` — Get Cozo DB timestamp for historical queries
    - `turn_record()` — Get full TurnRecord for a turn
    - `tool_calls_in_turn()` — Get tool calls from a specific turn
    - `llm_response_at_turn()` — Get LLM response from a turn
    - `replay_state_at_turn()` — Reconstruct complete state for replay
    - `total_token_usage()` — Sum tokens across all turns
    - `turn_count()` — Get total number of turns
    - `was_tool_used()` — Check if a tool was used anywhere
    - `turns_with_tool()` — Find all turns using a specific tool
    - `outcome_summary()` — Get high-level run statistics
  - Added `ReplayState` struct for state reconstruction
  - Added 10 comprehensive tests for all introspection methods

- **Phase 1 COMPLETE** — All RunRecord deliverables finished
  - 46 tests passing in ploke-eval (was 34, added 12 new)
  - No changes required outside ploke-eval crate

## 2026-04-09

- formalized the split between [docs/workflow](../../../workflow) and [docs/active/workflow](..)
- created durable workflow docs for manifests, experiment config, EDRs, checklists, and skills
- populated the living workflow artifacts for the programme charter, registry, evidence ledger, taxonomy, and active EDR area
- converted the lab book into an `mdbook` and added an explicit archive-boundary chapter
- added `owning_branch`, `review_cadence`, and `update_trigger` metadata to the active workflow artifacts
- ran five independent doc-review passes and folded the highest-signal issues into the workflow docs; see [2026-04-09-doc-review-followups.md](2026-04-09-doc-review-followups.md)
- **AGENTS.md** now references eval workflow documentation
- **A5** marked as hard gate for H0 interpretation in hypothesis registry
- **Diagnostic hypotheses** added to registry with `D-{DOMAIN}-{NNN}` format (Option C)
- **Cozo time travel** clarified for DB snapshot strategy — see [2026-04-09_run-manifest-design-note.md](../../agents/2026-04-09_run-manifest-design-note.md)
- **Run manifest vs run record** design converged — manifest is lightweight/differentiating, record is comprehensive with Cozo timestamps
- **Type inventory** created — complete catalog of serializable types for run record implementation — see [2026-04-09_run-record-type-inventory.md](../../agents/2026-04-09_run-record-type-inventory.md)
- **Handoff doc** created — [2026-04-09_run-record-design-handoff.md](./2026-04-09_run-record-design-handoff.md)
- **Phase 1 tracking** created — [phase-1-runrecord-tracking.md](../../plans/evals/phase-1-runrecord-tracking.md) — implementation plan validated, ready to begin
