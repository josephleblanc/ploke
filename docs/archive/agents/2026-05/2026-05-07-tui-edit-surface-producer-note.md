# TUI Edit Surface Producer Note

Date: 2026-05-07

The live `candidate-generator=tui-edit-surface` path now has a minimal
parent-side producer for `edit-surface=ploke-tui-tools`.

Current behavior:

- generates deterministic single-file direct-splice edit proposals against
  `crates/ploke-tui/src/tools/code_edit.rs`;
- appends harmless Rust comments so candidates should remain compile-safe;
- dedupes duplicate proposed file contents before validation;
- requires at least the configured minimum child count and caps production at
  the configured maximum;
- publishes `ChildPlan` only after each candidate passes
  `GitWorktreeBackend::validate_edit_surface_candidate` and converts through
  the checked edit-surface bridge.

This intentionally does not use `AppState.proposals`, UI status, auto-apply
state, or rendered TUI proposal storage as authority. The direct-splice spans
are a small interim harness path; true LLM-driven proposal generation over the
bounded `ploke-tui` tool surface is the next implementation step.
