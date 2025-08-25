# Change Context — Files not needed for the immediate task

Purpose
- Track which referenced files are not required right now to reduce cognitive load.
- These files remain useful references but are not needed to rewrite the system prompt or validate tool-call wiring.

Not needed for immediate changes
- docs/testing/llm_request_snapshot_harness.md
  - Useful for: Describes the minimal snapshot harness and structures for request payloads.
  - Recent changes: Notes about using insta and OpenAI-style request builder.
  - Reason: Our current task is prompt rewrite + tool-call guidance; no change to request builder or tests in this step.

- docs/testing/snapshot_tests_plan.md
  - Useful for: Outlines TUI rendering snapshot testing strategy.
  - Recent changes: None relevant to tool calls.
  - Reason: Not needed to modify prompts or tool-call behavior.

- docs/planning/model_provider_selection_mvp.md
  - Useful for: Planning model/provider selection overlay and events.
  - Reason: Not directly related to prompt/tool-call messaging; we are not touching overlay wiring in this step.

- docs/openrouter/tool_ready_selection.md
  - Useful for: Catalog fields and per-provider tool support guidance.
  - Reason: Helpful background, but not required for prompt editing or immediate tool-call flow.

- crates/ploke-tui/docs/feature/agent-system/agentic_system_plan.md
  - Useful for: Long-term roadmap and milestones (M0–M10).
  - Reason: Context only; not needed to implement the prompt rewrite.

- crates/ploke-tui/docs/feature/agent-system/impl-log/implementation-log-018.md
  - Useful for: Historical record (back-compat config wrapper).
  - Reason: Not needed for current prompt/tool-call adjustments.

- crates/ploke-tui/docs/feature/agent-system/impl-log/implementation-log-019.md
  - Useful for: M1 core wiring verification notes.
  - Reason: Reference only for status; current change is isolated to the prompt string.

Possibly needed soon (keep handy, but not edited now)
- docs/bugs/tui_ux_issues.md
  - Useful for: Known issues and async model search strategy; contains TODOs we will return to.
  - Reason: Not required for this prompt-only change.

- crates/ploke-tui/Cargo.toml
  - Useful for: Dev-deps (insta) and features; may matter when adding tests.
  - Reason: Build context; no edits needed now.

- crates/ploke-tui/src/app/commands/{parser.rs,exec.rs}
  - Useful for: Model search UX and commands.
  - Reason: Not part of prompt/tool-call text; no changes in this step.

- crates/ploke-tui/src/llm/{mod.rs,session.rs}
  - Useful for: Tool schemas, request execution, and tool-call dispatch.
  - Reason: We rely on these as-is; prompt change does not require code changes here.

Kept and used now
- crates/ploke-tui/src/app_state/handlers/rag.rs
  - Useful for: Houses PROMPT_CODE and tool-call handler; we updated PROMPT_CODE here.

- crates/ploke-tui/src/app_state/{dispatcher.rs,commands.rs}
  - Useful for: Hooking prompt and tool events into the app loop.
  - Reason: Not modified now, but part of the active flow.

- crates/ploke-tui/src/observability.rs
  - Useful for: Tool-call lifecycle persistence; no edits needed to change prompts.

Note
- This list is scoped to the immediate prompt rewrite and tool-call guidance. It will evolve as we move to end-to-end tool-call validation and UX polish.
