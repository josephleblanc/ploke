# Implementation Log 005 — Introduce IoManagerBuilder and Config Precedence

Date: 2025-08-18

Summary
- Added IoManagerBuilder to configure the I/O actor before startup:
  - with_semaphore_permits(usize)
  - with_fd_limit(usize) — clamped 4..=1024 and respects env PLOKE_IO_FD_LIMIT when explicit permits are not set
  - with_roots(Vec<PathBuf>) — stored for future path policy enforcement
  - build() -> IoManagerHandle
- Kept backward compatibility:
  - IoManagerHandle::new() unchanged
  - Added IoManagerHandle::builder()
  - IoManager::new() preserved; added IoManager::new_with(...) for builder path
- Introduced compute_fd_limit_from_inputs helper to unify precedence and clamping logic.

Rationale
- Implements Phase 2 of docs/production_plan.md: Provide a configuration surface and keep a convenient default constructor.
- Sets the stage for future path policy and watcher features without behavioral changes today.

Changes Made
- crates/ploke-io/src/lib.rs:
  - Added IoManagerBuilder struct and impl.
  - Added IoManagerHandle::builder().
  - Added IoManager::new_with(...) and stored optional roots on IoManager.
  - Added compute_fd_limit_from_inputs helper.
  - Added unit tests covering limit precedence and clamping.

Tests/Verification
- Run: cargo test -p ploke-io
- All existing tests pass; new unit tests for limit computation are green.

Impact/Risks
- No runtime behavior changes unless builder is used.
- Roots are stored only; enforcement is deferred to Phase 7.

Next Steps
- Phase 2: Add documentation section “Configuration and Builder”.
- Phase 3: Consider integrating roots into path validation logic once policy is finalized.
