# 2026-05-16 Protocol Dir Rule Answered Before Tracing Write Target

- Trigger:
  User asked where the protocol data was actually being written for the run
  behind a fresh `ploke-egui` diagnostics snapshot.
- User-visible failure:
  The agent answered from the generic `protocol-artifacts` path rule and the
  `ploke-tree` loader hook before tracing the real write call chain and the
  concrete `record_path` / registration-backed run roots for the active
  Prototype 1 campaign.
- Touched code surface:
  - `crates/ploke-eval/src/cli.rs`
  - `crates/ploke-eval/src/protocol_artifacts.rs`
  - `crates/ploke-eval/src/run_registry.rs`
  - `crates/ploke-eval/src/inner/registry.rs`
  - `crates/ploke-records/src/evaluation.rs`
  - current campaign data under
    `~/.ploke-eval/campaigns/p1-five-gen-1x3-20260516-1/` and
    `~/.ploke-eval/instances/prototype1/p1-five-gen-1x3-20260516-1/`
- What the agent did:
  - projected the canonical helper path `run_root/protocol-artifacts`
  - searched under the `prototype1` campaign root from the egui snapshot
  - only after the user pushed back did the agent trace `write_protocol_artifact`
    callers, read the evaluation artifact's compared instance record paths, and
    inspect the run registrations that own the actual write targets
- Skipped docs / skills / instructions:
  - skipped the user's request to find the directory where the data is being
    written for the concrete run
  - violated the repository's "do not guess; inspect code" expectation in
    operator-facing archaeology work
- Why the behavior was risky:
  It collapsed three different things into one answer:
  the generic layout rule, the `ploke-tree` import opt-in path, and the live
  `ploke-eval` protocol writer target. In this case the campaign root shown by
  egui was not the run root used by protocol writes, so the premature answer
  hid the real storage boundary.
- Concrete prevention rule:
  When asked where a persisted protocol/eval artifact is being written for a
  specific run, trace from the writer call site to the concrete `record_path`
  and registration-owned `RunArtifactRefs` first. Only then mention the generic
  layout helper.
- Memory hypothesis:
  If memory helps, the agent should default to a four-step proof:
  `writer -> record_path argument -> resolved run_root -> on-disk directory
  state`.
