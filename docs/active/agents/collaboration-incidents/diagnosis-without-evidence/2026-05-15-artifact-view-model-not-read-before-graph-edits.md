# 2026-05-15 Artifact View Model Not Read Before Graph Edits

- Trigger:
  User reported that the artifact graph looked worse after recent edge and
  continuity changes, then explicitly ordered the agent to read
  `crates/ploke-eval/src/cli/prototype1_state/mod.rs` and the
  `crates/ploke-egui/docs/model` documents instead of guessing.
- User-visible failure:
  The agent changed default artifact-graph semantics without first grounding
  itself in the documented product target. That produced a graph that drifted
  away from the intended artifact-first view and forced the user to restate the
  core model.
- Touched code surface:
  - `crates/ploke-egui/src/ui/view/projection.rs`
  - `crates/ploke-egui/src/ui/view/edge.rs`
  - `crates/ploke-egui/src/ui/view/diagnostics.rs`
  - `crates/ploke-egui/docs/model/default-view-contract.md`
  - `crates/ploke-egui/docs/model/run-graph-crosswalk.md`
  - `docs/active/agents/ploke-ui-task-readability/artifact-tree-default/README.md`
- What the agent did:
  - pushed `P_O` History context edges into the default artifact canvas
  - treated same-artifact continuity as something to draw with extra loop
    geometry before re-reading the documented default-view contract
  - reasoned from local implementation pressure instead of the stated product
    shape: artifact nodes as the primary geometry, `P_H union P_B` as primary
    edges, and History used for reveal/highlight rather than as the canvas
    spine
- Skipped docs / skills / instructions:
  - skipped the effective intent of
    `docs/active/agents/ploke-ui-task-readability/artifact-tree-default/README.md`
  - skipped re-reading `crates/ploke-eval/src/cli/prototype1_state/mod.rs`
    before changing artifact semantics
  - skipped the model docs under `crates/ploke-egui/docs/model/` that define
    the artifact-first default view
  - violated the spirit of `semantic-architecture` by changing the projection
    before restating the semantic object and relation set
- Why the behavior was risky:
  It let renderer-local pressure rewrite the product model. That produces a
  graph that may be internally consistent as code while still being wrong for
  the operator question the default view is supposed to answer.
- Concrete prevention rule:
  Before changing default artifact-graph semantics, re-read:
  - `crates/ploke-eval/src/cli/prototype1_state/mod.rs`
  - `crates/ploke-egui/docs/model/README.md`
  - `crates/ploke-egui/docs/model/source-process-graph.md`
  - `crates/ploke-egui/docs/model/default-view-contract.md`
  - `crates/ploke-egui/docs/model/view-set-contract.md`
  - `crates/ploke-egui/docs/model/run-graph-crosswalk.md`
  - `docs/active/agents/ploke-ui-task-readability/artifact-tree-default/README.md`

  Then restate, in the change description, which node set and which edge set
  the default view is supposed to render, and which relation families are only
  marks, drilldown, or alternate overlays.
