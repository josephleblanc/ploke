# Recent Activity

- last_updated: 2026-04-12
- ready_for: P0 eval-infra packet execution under active control plane
- owning_branch: refactor/tool-calls
- review_cadence: update after meaningful workflow-doc changes or handoffs
- update_trigger: update after touching workflow structure, review rules, or active artifact layout

## Guardrails

- **PRODUCTION CODE CHANGES OUTSIDE PLOKE-EVAL REQUIRE EXPLICIT PERMISSION**
  - Before modifying any production code outside `crates/ploke-eval/`:
    1. STOP and ask the user
    2. Wait for explicit permission before proceeding
  - This applies to: `syn_parser`, `ploke-tui`, `ploke-db`, `ploke-llm`, etc.
  - Rationale: Prevent unintended side effects on core infrastructure during eval work

## 2026-04-12

- **EVAL ORCHESTRATION PROTOCOL ADOPTED** — active control plane created for Phase 1 P0 gaps
  - Created [Eval Orchestration Protocol](../../agents/2026-04-12_eval-orchestration-protocol/2026-04-12_eval-orchestration-protocol.md) and compact [templates](../../agents/2026-04-12_eval-orchestration-protocol/2026-04-12_eval-orchestration-templates.md)
  - Workers now report claims plus evidence, not self-certified "verified/done" status
  - Verifier passes are bounded; orchestrator is sole acceptance authority
  - [AGENTS.md](../../../../AGENTS.md) now mirrors the cold-start sequence and points directly at the protocol for eval execution

- **EVAL INFRA SPRINT CONTROL PLANE ACTIVE**
  - Active planning doc moved from audit synthesis to [2026-04-12_eval-infra-sprint-control-plane.md](../../agents/2026-04-12_eval-infra-sprint/2026-04-12_eval-infra-sprint-control-plane.md)
  - Seeded initial P0 packets:
    - `P0A` SetupPhase schema extension
    - `P0B` SetupPhase capture wiring
    - `P0C` historical DB query support
    - `P0D` turn DB-state lookup
    - `P0E` replay query surface
  - **Permission gate:** `P0C` is blocked pending explicit approval because it touches `crates/ploke-db/`
  - This entry supersedes older implied "Phase 1 complete" claims as current operational truth

- **CONTROL PLANE EXPANDED TO MULTI-LANE PROGRAM**
  - Added active non-blocking sidecar lanes so broader concerns do not fall out of scope:
    - `S1-COHERENCE` for `ploke-eval` API/code-quality audit
    - `S2-LONGITUDINAL` for change-over-time metrics/reporting design
    - `S3-META-PROCESS` for workflow/skills adherence audit
  - Seeded sidecar packets:
    - `S1A` ploke-eval coherence audit
    - `S2A` longitudinal metrics design
    - `S3A` workflow and skills adherence audit
  - Primary lane remains the blocking path; sidecars are active parallel work, not deferred backlog

- **PRIMARY PATCH DISPOSITION STARTED**
  - Reviewed current in-worktree `ploke-eval` changes against `P0A/P0B`
  - Added [P0A/P0B initial verification note](../../agents/2026-04-12_eval-infra-sprint/2026-04-12_P0AB_initial-verification-note.md)
  - Current state:
    - setup schema/capture look independently checked inside `ploke-eval`
    - the same patch also includes replay/query additions that should remain unaccepted pending `P0C` permission and stronger evidence
  - Accepted sidecar reports:
    - [S2A report](../../agents/2026-04-12_eval-infra-sprint/2026-04-12_S2A_longitudinal-metrics-report.md)
    - [S3A report](../../agents/2026-04-12_eval-infra-sprint/2026-04-12_S3A_workflow-adherence-audit-report.md)

- **S1A ACCEPTED; NEW FOLLOW-UP PACKETS SEEDED**
  - Accepted [S1A coherence audit report](../../agents/2026-04-12_eval-infra-sprint/2026-04-12_S1A_ploke-eval-coherence-audit-report.md)
  - New primary-lane packet added:
    - `P0F` turn-record fidelity and replay-state reconstruction
  - New sidecar follow-up packets added:
    - `S2B` longitudinal metrics ledger and formula definition
    - `S3B` control-plane and handoff template tightening
  - Operational implication:
    - replay/inspection risk is not only historical-query support; current turn persistence inside `ploke-eval` is itself a blocking fidelity issue

- **S2B/S3B ACCEPTED; P0F ACCEPTED AFTER INDEPENDENT CHECK**
  - Accepted [S2B ledger report](../../agents/2026-04-12_eval-infra-sprint/2026-04-12_S2B_longitudinal-metrics-ledger-report.md) and created [longitudinal-metrics.md](../longitudinal-metrics.md) as the central metrics roll-up artifact
  - Accepted [S3B template report](../../agents/2026-04-12_eval-infra-sprint/2026-04-12_S3B_control-plane-and-handoff-template-tightening-report.md) and tightened:
    - [handoff-template.md](../../../../docs/workflow/handoff-template.md)
    - [eval orchestration templates](../../agents/2026-04-12_eval-orchestration-protocol/2026-04-12_eval-orchestration-templates.md)
  - Current primary-lane state:
    - `P0F` retry changes landed in `crates/ploke-eval/src/record.rs` and related tests
    - independent verification completed against targeted `ploke-eval` tests, so `P0F` is accepted on the strength of [P0F retry report](../../agents/2026-04-12_eval-infra-sprint/2026-04-12_P0F_retry-report.md)
    - remaining ambiguity is now concentrated in the mixed-scope `P0A/P0B/P0D/P0E` patch and the `P0C` permission gate

- **S2B LEDGER CREATED**
  - Added [longitudinal metrics ledger](../longitudinal-metrics.md) as the central roll-up surface for formal eval runs
  - Defined explicit formulas, denominators, source expectations, and derivable-now versus blocked metrics
  - Current blocker remains turn-level misuse and recovery capture/aggregation; the ledger now names that gap directly

- **SIDECAR FOLLOW-UP PACKETS ADDED FROM RESTART REVIEW**
  - Added `S2C` to explore lightweight discovery, durable storage, and auto-rollup for new formal runs feeding [longitudinal-metrics.md](../longitudinal-metrics.md)
  - Added `S3C` to inventory available workflow/process evidence sources and frame exploratory hypotheses for protocol adherence and drift
  - These are active sidecar packets, not deferred backlog, but they remain non-blocking relative to the primary P0 lane

- **PRE-`P0C` QUERY-SURFACE SURVEY ADDED**
  - Added `P0C0` to survey the existing `ploke-db` query-builder and raw-query surface before committing to the historical-query implementation path
  - Rationale: current evidence suggests the builder is real but partial, while many active call sites still bypass it with raw Cozo scripts; the sprint should choose whether to extend, wrap, or deliberately bypass that surface before landing `P0C`
  - `P0C` remains permission-gated for implementation because it touches `crates/ploke-db/`

- **`P0A` / `P0B` / `P0C0` ACCEPTANCE BOUNDARIES CLARIFIED**
  - Accepted `P0A` and `P0B` as setup-only slices on the strength of [P0A/P0B scope separation review](../../agents/2026-04-12_eval-infra-sprint/2026-04-12_P0AB_scope-separation-review.md)
  - Explicitly kept `DbState`, `lookup`, `query`, `replay_query`, and the mixed replay tests outside that acceptance boundary
  - Accepted `P0C0` on the strength of [query-builder survey report](../../agents/2026-04-12_eval-infra-sprint/2026-04-12_P0C0_query-builder-survey-report.md)
  - Chosen direction for `P0C`: use the existing `raw_query_at_timestamp()` / `DbState` helper path rather than extending `QueryBuilder` during the primary P0 lane

- **`P0C` ACCEPTED WITH BASELINE COMPARISON**
  - Accepted the narrow historical-query helper slice on the strength of [P0C report](../../agents/2026-04-12_eval-infra-sprint/2026-04-12_P0C_report.md)
  - Acceptance boundary is explicit: `Database::raw_query_at_timestamp()` now requires at least one `@ 'NOW'` marker, rewrites all such markers to the supplied timestamp, and has targeted tests for historical behavior, missing-marker rejection, and multi-marker rewriting
  - Did not accept the whole dirty `crates/ploke-db/src/database.rs` diff by implication; only the helper-contract/test slice is in scope for this packet
  - Pre/post full-workspace regression runs used the same environment overrides and showed no new failures: both runs remained red only on `ploke-tui` integration tests `post_apply_rescan::approve_emits_rescan_sysinfo_under_default_profile` and `post_apply_rescan::approve_emits_rescan_sysinfo_under_verbose_profile`

- **`P0D` / `P0E` ACCEPTED; PRIMARY P0 LANE CLOSED**
  - Accepted `P0D` and `P0E` on the strength of [P0D/P0E verification report](../../agents/2026-04-12_eval-infra-sprint/2026-04-12_P0DE_verification_report.md)
  - `TurnRecord::db_state()` / `DbState::lookup()` and `RunRecord::replay_query()` now meet their packet criteria on top of accepted `P0C`
  - No code changes were needed in the verification pass; acceptance is based on targeted `ploke-eval` tests over the existing implementation
  - Residual risks were explicitly bounded rather than treated as blockers:
    - `lookup()` is exact-name, fixed-relation, first-hit behavior only
    - `replay_query()` is a thin raw-query wrapper over `P0C`
    - nonexistent-turn handling currently collapses to `TimestampNotFound`
  - Operational consequence: the Phase 1 P0 replay/inspection lane is no longer the blocking item for the eval programme

- **POST-P0 SIDECAR PROMOTION QUEUED**
  - Added `S1B` to promote the accepted `ploke-eval` coherence audit into a bounded cleanup track
  - Added `S1C` to audit the inspect-oriented `ploke-eval` CLI as a frequent internal UX/bootstrap surface for quick eval checks
  - `S2C` and `S3C` remain ready as the longitudinal ingestion/bootstrap and meta-observability follow-ups
  - Intended post-compaction resume point: choose from `S1B`, `S1C`, `S2C`, and `S3C` rather than treating the next step as implicit

## 2026-04-11 (Late Evening)

- **PHASE 1 AUDIT COMPLETE** — Critical gaps identified
  - 4 sub-agents parallel investigation of claimed vs actual implementation
  - **Key finding:** `turn.db_state().lookup()` was claimed complete but is **NOT IMPLEMENTED**
  - **Key finding:** SetupPhase is **NEVER POPULATED** (verified `null` in record.json.gz)
  - **Key finding:** Historical DB queries **NOT POSSIBLE** (all queries hardcode `@ 'NOW'`)
  - **Phase 1 status:** INCOMPLETE - requires 3-4 days additional work
  - **Audit docs:** [PHASE_1_AUDIT_MASTER.md](../../agents/phase-1-audit/PHASE_1_AUDIT_MASTER.md), [AUDIT_SYNTHESIS.md](../../agents/phase-1-audit/AUDIT_SYNTHESIS.md)

- **DUAL SYN VERSION SUPPORT IMPLEMENTED** — A2 parser issue (code complete, validation blocked)
  - syn1 dispatch, conversion layer, DRY refactoring complete
  - 378 unit tests passing
  - **BLOCKED on Phase 1 gaps:** Cannot validate parse results without SetupPhase population
  - Need to complete P0 audit items before claiming A2 validated
  - Created syn1 versions of visitor files (code_visitor_syn1.rs, attribute_processing_syn1.rs, type_processing_syn1.rs)
  - Added edition-based dispatch: syn1 for Rust 2015, syn2 for 2018+
  - **IN PROGRESS:** Syn1→syn2 type conversion in `parser/utils.rs` to enable code reuse
    - Completed: Type, Path, GenericArgument, TypeParamBound, ReturnType, BoundLifetimes, Abi, Macro
    - Added: `Syn1ToSyn2AttributeConversion` error variant for proper error handling
    - Remaining: Fix AssocType/Constraint field mismatches, Attribute conversions
  - All 378 tests pass
  - Rust 2015 bare trait objects (`Arc<Fn(...)>`) now parse successfully
  - Rust 2015 async identifiers (`fn async(&self)`) now parse successfully
  - **Handoff:** [2026-04-11_dual-syn-implementation-handoff.md](2026-04-11_dual-syn-implementation-handoff.md)
  - **Next:** Complete syn1→syn2 conversion, then integrate into `process_fn_arg_syn1`

## 2026-04-10 (Afternoon)

- **A4/A5 VALIDATED** — RunRecord implementation verified with real data
  - Examined existing `record.json.gz` from `BurntSushi__ripgrep-2209` run
  - Schema v1 present, all required fields captured:
    - `conversation`: message history ✓
    - `db_time_travel_index`: Cozo timestamps for replay ✓
    - `phases.agent_turns`: 1 turn with 97 events ✓
  - All 16 record-related tests pass:
    - A4 schema tests: roundtrip, compression, event capture ✓
    - A5 introspection tests: 10 methods all passing ✓
  - **Next:** Can now query runs without re-running (A5 achieved)

- **A2 ISSUE IDENTIFIED** — globset crate fails to parse
  - Attempted live run on `BurntSushi__ripgrep-1294`
  - 6 of 9 ripgrep crates indexed successfully
  - `globset` crate failed: "Partial parsing success: 6 succeeded, 1 failed"
  - Root cause: syn 2.x rejects Rust 2015 bare trait objects (e.g., `Arc<Fn(...)>`)
  - **Solution selected:** Dual syn versions (syn 1.x for Rust 2015, syn 2.x for modern)
  - **Bug report:** [docs/active/bugs/2026-04-10-syn-2-fails-on-rust-2015-bare-trait-objects.md](../../../active/bugs/2026-04-10-syn-2-fails-on-rust-2015-bare-trait-objects.md)
  - **Status:** Awaiting implementation post-context-compaction

- **Qwen Deserialization Bug Fixed**
  - Fixed `RESPONSE_DESERIALIZATION_FAILED` when qwen returns `reasoning` without `content`
  - Feature flag `qwen_reasoning_fix` in `ploke-llm` coalesces reasoning→content when content missing
  - Tests use real captured response from `BurntSushi__ripgrep-2209` run
  - Bug documented in `docs/active/bugs/2026-04-10-qwen-reasoning-content-deserialization-failure.md`

## 2026-04-10 (Morning)

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
