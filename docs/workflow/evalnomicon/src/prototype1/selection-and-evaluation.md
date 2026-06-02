# Selection and Evaluation

Prototype 1 selection decides which child artifact/runtime, if any, becomes the next authoritative parent for a lineage. A child runtime may produce evidence, but it does not self-promote.

## Selection boundary

The core distinction is:

```text
adjudicated protocol output answers "what should we try?"
mechanized evaluation answers "should we keep this branch?"
selection policy decides "what receives successor authority?"
```

Child self-evaluation is evidence. Promotion requires a parent-side policy over admissible evidence and History/Crown admission checks.

## First-pass selection shape

The current research seed suggests a modular scorer rather than a learned reward model:

1. Hard gates: no patch, invalid artifact, failed protocol, failed build/test setup, or malformed result cannot be selected as a successful child.
2. Oracle evidence dominates: external eval pass/keep evidence beats heuristic evidence.
3. Patch viability: candidate compiles, applies cleanly, changes target surface coherently, and avoids broad unrelated churn.
4. Trajectory/process evidence: direct target localization, fewer repeated searches, edit after evidence, validation after edit.
5. Cost evidence: shorter successful runs are preferred only after viability/process gates; cheap failure is not rewarded.
6. LLM adjudication: rubric evidence and critique may support selection, but should not become circular self-authority.
7. Continuation fallback: if no child is acceptable, exploratory continuation needs an explicit state distinct from success.

## Current code-rooted selection sources

The current implementation shape is summarized in:

- `docs/workflow/evalnomicon/drafts/selection/README.md`

Important code anchors listed there include:

- `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs`
- `crates/ploke-eval/src/successor_selection/traversal.rs`
- `crates/ploke-eval/src/successor_selection/metrics.rs`
- `crates/ploke-eval/src/cli/prototype1_state/history.rs`
- `crates/ploke-tree/src/graph/build/selection.rs`

## Research anchors

Local sources under `research-symlink/ploke-child-selection-2026-05-06/` support the following directions:

- SWE-Gym: verifier/reranker over problem, trajectory, and diff.
- SWE-TRACE and AgentPRM: process/rubric reward evidence for long-horizon agent trajectories.
- SWE-Replay: continuation and branch-point selection rather than only final-patch selection.
- Self-Rewarding, STOP, and related work: warnings about circular scoring and self-modifying authority surfaces.
- HyperAgents: archive admission, parent selection, transfer/stepping-stone value, staged/held-out evaluation.
