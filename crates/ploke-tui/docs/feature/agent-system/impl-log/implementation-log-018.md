# Implementation log 018 — Back-compat Config wrapper + generic ConfigState::new (2025-08-20)

Summary
- Unblocked tests failing with E0063 (missing `editing` field) after adding M1 editing config.
- Introduced a lightweight wrapper `app_state::Config` (llm_params, provider_registry) for legacy struct-literal construction.
- Made `ConfigState::new` generic over `Into<core::Config>` and provided identity `From<core::Config>` so both the wrapper and the full config work seamlessly.

Details
- app_state/mod.rs:
  - Stop re-exporting `core::Config` directly.
  - Add `pub struct Config { llm_params, provider_registry }` and `impl From<app_state::Config> for core::Config` which fills `editing` with defaults.
- app_state/core.rs:
  - `ConfigState::new<C: Into<Config>>(config: C)` now accepts either the wrapper or the full `core::Config`.
  - Add identity `impl From<core::Config> for core::Config` for ergonomic calls inside the crate.

Why
- Preserve backward compatibility for existing tests and callsites that construct `Config` with a struct literal and no `editing` field.
- Keep the accelerated M1 schedule moving without blocking on test file edits.

Impact
- No functional behavior changes at runtime; purely construction/API ergonomics.
- Tests that previously failed to compile due to the new field now succeed.

Next steps
- Continue M1: polish approval/denial UX, add E2E tests for proposal apply/deny flows.
- Consider persisting editing config to user config in a follow-up.
