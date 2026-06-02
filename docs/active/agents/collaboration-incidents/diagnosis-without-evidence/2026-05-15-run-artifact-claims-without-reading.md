# 2026-05-15 Run Artifact Claims Without Reading

- Trigger:
  User asked where the benchmark/eval data actually lived and later explicitly
  demanded confirmation from the files themselves rather than more inference.
- User-visible failure:
  The agent talked about which run artifacts would be useful before proving, by
  direct reads of the current run files, what data those artifacts actually
  contained and what was absent.
- Touched code surface:
  - `crates/ploke-eval/src/layout.rs`
  - `crates/ploke-eval/src/cli/prototype1_state/edit_surface/harness_request.rs`
  - run artifacts under
    `~/.ploke-eval/instances/prototype1/p1-broad-harness-minimaxm25-20260515-8/.../runs/run-1778904521508-structured-current-policy-9cf6c975/`
- What the agent did:
  - inferred that `record.json.gz`, `agent-turn-summary.json`, and related
    sidecars were the right evidence surfaces
  - explained their likely usefulness before reading enough of the concrete run
    files to prove the exact fields present
  - made the user spend another turn forcing a file-backed answer
- Skipped docs / skills / instructions:
  - skipped the user's demand for direct confirmation from the run artifacts
  - violated the spirit of typed-persistence-spine by reasoning about persisted
    surfaces before reading the owned typed artifacts that exist for the active
    run
- Why the behavior was risky:
  It blurred the difference between "the pipeline map says this file family is
  important" and "this specific run file contains the exact data the patcher
  needs." That wastes operator time and makes prompt/input redesign harder.
- Concrete prevention rule:
  When the user asks what data a persisted run surface contains, answer only
  after reading the concrete current-run files and listing exact present and
  absent fields. Do not substitute pipeline docs for artifact inspection.
- Memory hypothesis:
  If memory helps, the agent should default to a short evidence table:
  `file -> present fields -> absent fields -> why it matters`.
