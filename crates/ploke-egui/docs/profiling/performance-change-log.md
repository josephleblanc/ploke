# Performance Change Log

Update this file when a `ploke-egui` change has measured or plausible
performance impact. Keep entries short and cite the verification surface.

## 2026-05-16 Profiling Baseline

- Change: added `profiling` scopes and the `profile-with-puffin` feature for
  interactive profiler backends.
- Expected positive impact: gives frame/projection spans for local profiler
  sessions without changing normal no-backend builds.
- Expected negative impact: enabling a profiler backend intentionally adds
  measurement overhead; compare backend-enabled runs only with other
  backend-enabled runs.
- Change: added `--perf-log`, a dev-only typed rolling log for coarse import and
  graph projection timings.
- Expected positive impact: provides a stable five-sample local baseline for
  catching large regressions in graph loading and graph-view projection.
- Baseline target: `/home/brasides/.ploke-eval/campaigns/p1-five-gen-1x3-20260516-1/prototype1`.
- Baseline guardrail: `--perf-log` requires an explicit run root so it cannot
  silently measure the sample graph or run-picker fallback graph.
- Limitation: `--perf-log` measures import plus contract diagnostics, not native
  pointer interaction, GPU painting, or live right-panel interaction.
