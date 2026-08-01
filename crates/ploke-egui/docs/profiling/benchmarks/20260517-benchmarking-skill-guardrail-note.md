# Benchmarking Skill Guardrail Note

Verified surface: not tested; this change adds a project skill and profiling
docs only, with no `ploke-egui` runtime code changed in this turn.

## Change

- Added `.codex/skills/ploke-egui-benchmarking/SKILL.md`.
- Updated profiling indexes so benchmark reports and short notes are
  discoverable.

## Baseline

- Existing native smoke report:
  `crates/ploke-egui/docs/profiling/benchmarks/20260517-dc92c2aecd61-standard/report.json`.
- No new runtime measurement was taken because the edited surfaces are skill and
  documentation files.

## Performance Increases

- None measured.
- Process improvement: future `ploke-egui` edits now require a benchmark note or
  generated native benchmark report.

## Regressions

- No runtime regression expected; no application code or benchmark framework code
  changed in this turn.

## Residual Risk

- The new skill itself is procedural. Its effectiveness depends on future agents
  loading it whenever they edit `crates/ploke-egui/**`.
