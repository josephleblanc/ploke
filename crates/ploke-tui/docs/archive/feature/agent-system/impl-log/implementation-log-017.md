# Implementation log 017 — Runtime editing config commands (2025-08-20)

Summary
- Added runtime commands to control M1 safe-editing preview and flow:
  - edit preview mode <code|diff>
  - edit preview lines <N>
  - edit auto <on|off>
- StateManager updates AppState.config.editing and emits a SysInfo confirmation message for each change.

Details
- New StateCommand variants:
  - SetEditingPreviewMode { mode }
  - SetEditingMaxPreviewLines { lines }
  - SetEditingAutoConfirm { enabled }
- Parser:
  - Recognizes the above edit subcommands and validates inputs.
- Executor/Dispatcher:
  - Routes new structured commands; centralized mutation via state_manager.
- Help text updated to document the new commands.

Why
- Completes Phase 7 (UX toggles) from m1_granular_plan.md, enabling fast iteration without rebuilds.
- Operators can switch between code-block and unified diff previews and adjust truncation on the fly.

Next steps
- Optional: persist editing config to user-config file and load on startup.
- Tests for parsing edge cases and command side-effects.
- Continue M1 approval/denial polish and observability integration when ploke-db endpoints are available.
