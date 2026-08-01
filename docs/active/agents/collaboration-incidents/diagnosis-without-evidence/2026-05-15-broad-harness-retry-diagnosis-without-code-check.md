# 2026-05-15 Broad Harness Retry Diagnosis Without Code Check

- Trigger:
  User asked why the live broad-harness run was flooding the terminal with
  repeated patch-apply failures and later called out that the diagnosis had not
  actually followed the controlling code.
- User-visible failure:
  The agent described the behavior in terms of broad retry counts and noisy
  logs before reading the real `ploke-tui` session loop and `ploke-eval`
  headless runtime config. The user had to push repeatedly for an actual code
  trace.
- Touched code surface:
  - `crates/ploke-eval/src/runner.rs`
  - `crates/ploke-eval/src/cli/prototype1_state/edit_surface/tui_adapter.rs`
  - `crates/ploke-tui/src/llm/manager/session.rs`
  - `crates/ploke-tui/src/rag/editing.rs`
  - `crates/ploke-io/src/write.rs`
- What the agent did:
  - inferred retry behavior from symptoms before reading the controlling loop
  - initially talked as though slot counts explained the flood
  - only later verified that `ploke-eval` had raised the headless
    `tool_call_chain_limit` to `500`
  - only later verified that non-semantic patch apply claims a rescan was
    scheduled even though it does not actually call the rescan helper
- Skipped docs / skills / instructions:
  - skipped the practical core of runtime-error discipline: trace the concrete
    failure path before explaining it
  - skipped the user's direct request to diagnose instead of speculate
- Why the behavior was risky:
  It made the user spend extra turns policing whether the diagnosis was real,
  while the actual runtime bug involved a concrete interaction between stale
  file hashes, missing non-semantic rescan, and an oversized inner tool-call
  limit. That is exactly the kind of issue where wrong causal claims waste time
  and trust.
- Concrete prevention rule:
  For live-loop or runtime triage, do not explain retry behavior until the
  controlling loop and config writes have been read. The answer must name the
  exact file and line where the retry/continue decision is made before
  summarizing operator-visible behavior.
- Memory hypothesis:
  If memory helps, the agent should stop at "I need to trace the controlling
  loop" instead of producing an early causal story from logs alone.
