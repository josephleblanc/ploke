# 2026-05-17 Ploke Egui Native Benchmark Tunnel Vision

- Trigger:
  User pointed out that repeated full native benchmark attempts were becoming
  very long-running and said the agent appeared to have tunnel vision. The user
  had also told the agent to stop using `/tmp` because it was already too full.
- User-visible failure:
  The agent kept trying to make the full native v3 benchmark run complete even
  after the symptoms pointed to a benchmark-design problem. One run opened the
  native app long enough that the user had to exit it manually. The agent also
  used `/tmp` for an earlier benchmark-controller test fixture before moving
  scratch output into the workspace.
- Touched code surface:
  - `crates/ploke-egui/src/allocation.rs`
  - `crates/ploke-egui/src/benchmark.rs`
  - `crates/ploke-egui/src/native.rs`
  - `crates/ploke-egui/docs/profiling/benchmarks/`
- What the agent did:
  - retried the full native benchmark before fully stepping back from the
    allocator hot path
  - treated "make the standard suite finish" as the next action even after
    per-allocation backtraces and full heap snapshots were likely dominating
    runtime
  - closed the independent reviewer before receiving its final answer, even
    though the user explicitly asked for that review
  - initially used `/tmp` for benchmark test output despite the user's disk
    pressure warning
- Skipped docs / skills / instructions:
  - skipped the `ploke-egui-benchmarking` skill's regression-policy spirit:
    when native benchmark execution itself regresses, diagnose the benchmark
    design before continuing implementation work
  - skipped the collaboration expectation that a long-running or stuck process
    should be explained and bounded rather than repeatedly retried
  - skipped the user's explicit `/tmp` constraint until corrected
- Why the behavior was risky:
  Full native benchmark runs are expensive, interactive, and hard to interrupt
  cleanly from the sandbox. Repeating them without a fresh design diagnosis
  wastes user time, can leave GUI processes running, and hides the actual
  performance bug in the benchmark harness. Using `/tmp` under known disk
  pressure can also make unrelated local work fail.
- Concrete prevention rule:
  After one native benchmark hang, panic, or manual user exit, stop full-suite
  native retries. Inspect the benchmark hot path, add a bounded preflight or
  focused smoke, and only rerun the full suite after a short scenario proves the
  harness can make progress. For this repository, set `TMPDIR` to a workspace
  path such as `target/codex-tmp` for tests and checks unless the user asks
  otherwise.
- Memory hypothesis:
  Memory should make the agent treat native `ploke-egui` benchmark runs as a
  verification surface with real operational cost, not as an ordinary unit test
  loop.
