# Quality Checklist (Live tool-call E2E)

Scope: tests/e2e_openrouter_tools_app.rs and supporting runtime paths.

- Compile-time hygiene
  - [x] No warnings introduced in changed areas (fixed unused_mut; trimmed imports).
- Determinism
  - [ ] tool_choice control exposed; tests can force "required".
  - [x] Early-out on first observed tool call to bound runtime.
- Observability
  - [x] Persist request/decision/response/toolcall artifacts to target/test-output/openrouter_e2e.
  - [x] Terse console status per model.
- Correctness
  - [x] Tools included for selected models (capabilities refreshed/forced).
  - [x] Tool events (Requested/Completed/Failed) recognized and tracked as “saw_tool”.
  - [ ] Stronger assertions on ToolEvent::Completed content shape (JSON schema check).
- Reliability
  - [x] Skip test cleanly when OPENROUTER_API_KEY is unset.
  - [ ] Pin a “golden model” in CI to reduce endpoint variance.
- Safety
  - [x] Capability forcing is confined to test scope (no prod code loosening).

Self-assessment for this change set
- Passed items: compile hygiene, observability artifacts, inclusion of tools, bounded runtime, skip behavior.
- Outstanding: tool_choice required toggle, schema-level assertions, golden model pinning.
- Next iteration will address the remaining items and add DB-backed telemetry assertions.
