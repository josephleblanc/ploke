# 2026-06-02 Prototype 1 state-loop terminology conflicts and questions

Status: draft terminology audit companion
Related walkthrough: [`README.md`](README.md)
Scope: terms used around `ploke-eval loop prototype1-state`

## Purpose

This companion note tracks terms whose current usage is overloaded, partially formalized, or easy to confuse. The walkthrough glossary gives source-grounded working definitions. This document records places where we should tighten vocabulary or make an intentional naming decision.

## Current conflicts / ambiguity to resolve

### 1. Node

Current working definition: a Prototype 1 search candidate represented by `Prototype1NodeRecord`.

Why it conflicts:

- `node` can also mean a Rust syntax/tree node in `ploke-tree`/parser contexts.
- The same repo also has graph/node language around code intelligence and records.
- Prototype 1 node ids are scheduler/search ids, not AST ids.

Suggested convention:

- In Prototype 1 docs, say `Prototype 1 node` or `search node` on first use.
- Use `syntax node`, `graph node`, or `code graph node` for parser/ploke-tree meanings.

Open question:

- Should persisted ids use/mention `scheduler_node_id` in docs to reduce ambiguity, since `ploke_records::ids::SchedulerNodeId` appears in projection conversion?

### 2. Parent id vs parent node id vs node id

Current working definition: `ParentIdentity` stores `parent_id`, `node_id`, optional `parent_node_id`, and generation. Root bootstrap sets `parent_id == node_id`; selected successors also currently set `parent_id` from the selected node.

Why it conflicts:

- `parent_id` sounds like the predecessor parent, while `parent_node_id` is the predecessor node link.
- In current constructors, `parent_id` appears to be the active parent coordinate, not the previous parent.
- `previous_parent_id` separately stores predecessor parent id.

Suggested convention:

- Use `active parent id` when referring to `ParentIdentity.parent_id`.
- Use `predecessor parent id` for `previous_parent_id`.
- Use `parent_node_id` only for a node's direct parent in the search tree.

Open question:

- Should `ParentIdentity.parent_id` eventually be renamed or described as `active_parent_id` in passive docs/records?

### 3. Child vs successor

Current working definition: a child is a leaf evaluator; a successor is the next parent runtime.

Why it conflicts:

- Both are spawned runtimes with invocation JSON.
- Both can send messages over the runtime channel.
- A successful child can become the selected successor coordinate, but the child process itself is not the successor parent runtime.

Suggested convention:

- Say `child runtime` only for the leaf `prototype1-runner --execute` process.
- Say `successor runtime` or `successor parent runtime` for the next `prototype1-state` process.
- Say `selected child node` when the search candidate is selected; do not say the child process becomes the successor.

Open question:

- Should result reports distinguish `selected_child_node_id` from `successor_runtime_id` more explicitly?

### 4. Runtime vs run vs turn

Current working definition:

- `runtime`: concrete process attempt with `RuntimeId`.
- `run`: benchmark/eval run, often with `RunRecord` and run manifest.
- `turn`: one parent controller execution or one agent turn inside a `RunRecord`, depending context.

Why it conflicts:

- The walkthrough uses `parent turn`, while `RunRecord` also contains `TurnRecord` for LLM/tool turns.
- `run` appears in benchmark commands and `RunMsbAgentBatchRequest`, while `runtime_id` appears in Prototype 1 authority/invocation records.

Suggested convention:

- Use `parent turn` for `Prototype1StateCommand::run_turn`.
- Use `agent turn` for `RunRecord`/LLM turns.
- Use `runtime attempt` where a `RuntimeId` is involved.
- Use `benchmark run` or `eval run` where `RunRecord`/run manifests are involved.

Open question:

- Should `Prototype1StateReport` fields be documented as runtime-attempt fields to avoid confusion with eval runs?

### 5. Branch, treatment branch, artifact branch, git branch

Current working definition:

- `treatment branch` / `candidate branch`: proposed content state tracked by `TreatmentBranchNode` and `branch_id`.
- `artifact branch`: git branch name associated with a parent identity/artifact checkout.
- `git branch`: actual VCS branch.

Why it conflicts:

- `branch_id` is not necessarily a git branch name.
- `artifact_branch` sounds like `branch_id` but appears in `ParentIdentity` as a checkout/artifact coordinate.
- A treatment branch may later be materialized into a git worktree/branch, but the identity layers are different.

Suggested convention:

- Use `treatment branch id` for `branch_id` in `TreatmentBranchNode`/node records.
- Use `git branch` when referring to VCS branch names.
- Use `artifact branch` only for the `ParentIdentity.artifact_branch` field and explain it as a checkout/artifact locator.

Open question:

- Do we want a separate persisted `GitRef`/locator term in docs so `branch_id` and git branch names never share the word `branch` unqualified?

### 6. Artifact vs worktree vs source state

Current working definition:

- `ArtifactId` is a recoverable artifact identity; dirty worktrees should not get one.
- `worktree` is the live filesystem checkout.
- `source_state_id` identifies source content/state used for candidate generation.

Why it conflicts:

- Live Prototype 1 still has dirty-worktree paths and text-file surfaces while the graph vocabulary anticipates durable artifact ids.
- Some records carry both source-state ids and artifact ids, but not all paths are fully artifact-backed yet.

Suggested convention:

- Use `worktree` for mutable local filesystem state.
- Use `artifact` only when a recoverable identity exists or when referring to the forward graph vocabulary.
- Use `source state` for candidate-generation input content, not for a whole checkout unless the record says so.

Open question:

- Which current Prototype 1 persisted files should be treated as canonical artifact evidence versus compatibility projections during the `ploke-loop` extraction?

### 7. Authority vs projection

Current working definition: authority is typed/persisted evidence that permits a runtime action; projection is a cache/view derived from evidence or history.

Why it conflicts:

- The identity module says `parent_identity.json` is not a standalone proof of authority; startup and handoff still validate campaign/History state.
- The walkthrough uses phrases like “checkout-carried authority” for parent identity, which is directionally useful but could overstate the file's standalone power.

Suggested convention:

- Say `authority input` or `authority-bearing record` when the file participates in admission but is not sufficient alone.
- Say `projection` for `node.json`, branch summaries, scheduler snapshots, and derived reports unless they are explicitly the admission source.

Open question:

- Which records are intended to become authoritative after History extraction, and which should remain projections forever?

### 8. History, lineage, Crown, and scheduler/search tree

Current working definition: History is the durable authority surface over sealed lineage-local blocks; the scheduler/search tree is a mutable/projection layer over candidate nodes.

Why it conflicts:

- The live controller still uses transition journals, invocation files, scheduler/branch projections, and History-backed traversal together.
- History docs explicitly warn that scheduler snapshots, branch registries, CLI reports, and metrics dashboards are not themselves History authority.

Suggested convention:

- Use `History lineage` for authority-chain claims.
- Use `search tree` for node/generation/child planning claims.
- Use `History-backed traversal` for successor-selection scoring that imports History evidence, not for all search-tree traversal.

Open question:

- Where should docs draw the boundary between current Prototype 1 implementation and intended History authority model during the transition to `ploke-loop`?

### 9. Closure

Current working definition: `ClosureState` summarizes registry/eval/protocol completeness for campaign instances.

Why it conflicts:

- “Closure” can be confused with Rust closures or mathematical closure.
- The code uses `advance_eval_closure`, `advance_protocol_closure`, and `recompute_closure_state`, which can sound like three different systems.

Suggested convention:

- Say `campaign closure state` on first use.
- Say `eval closure section` and `protocol closure section` when referring to parts of `ClosureState`.

Open question:

- Should future docs rename this concept to `campaign completeness state` for non-project readers, while preserving `ClosureState` as the code type?

### 10. Protocol procedure names

Current working definition: default required procedures are `tool-call-intent-segments`, `tool-call-review`, and `tool-call-segment-review`.

Why it conflicts:

- The code normalizes aliases such as `tool_call_intent_segmentation` and `tool-call-intent-segmentation` into `tool-call-intent-segments`.
- Stored protocol artifact kinds use snake_case constants internally.

Suggested convention:

- Use the canonical kebab-case procedure ids in docs.
- Mention aliases only in config/compatibility sections.

Open question:

- Should protocol artifact filenames/kinds converge on the same canonical kebab-case ids, or keep snake_case storage kind names for compatibility?

### 11. MBE / MSB / Multi-SWE-Bench

Current working definition: MSB means Multi-SWE-Bench; MBE is the local module for Multi-SWE-Bench evaluation/harness integration.

Why it conflicts:

- `MSB` and `MBE` are both short acronyms and can look interchangeable.
- The code module is `mbe`, but user-facing files are named `multi-swe-bench-submission.jsonl` and the external module is `multi_swe_bench.harness.run_evaluation`.

Suggested convention:

- Use `Multi-SWE-Bench` on first use.
- Use `MSB` only for Multi-SWE-Bench benchmark/submission concepts.
- Use `MBE harness` or `Multi-SWE-Bench evaluator` for the `mbe` module path.

Open question:

- Is `MBE` intended to stand for “Multi-SWE-Bench Evaluation,” or is it just a module shorthand? The code supports the former interpretation but does not explicitly expand the acronym.

### 12. Baseline vs parent

Current working definition: baseline is evidence for the current parent/current branch before applying a treatment.

Why it conflicts:

- Generation 0 establishes a baseline from the initial campaign.
- Later generations promote selected child evidence into the next parent baseline, so “parent” and “baseline” can drift if stale evidence is accidentally reused.

Suggested convention:

- Say `parent baseline evidence` when comparing against a child treatment.
- Include campaign id/branch id when discussing baseline promotion.

Open question:

- Should branch evaluation reports explicitly label whether baseline evidence came from generation-0 closure or selected-child promotion?

## Follow-up process

When updating the walkthrough glossary:

1. Add or refine the source-grounded working definition in the walkthrough.
2. If usage is overloaded, add/update an entry in this companion document.
3. Prefer source file/line evidence over inference.
4. If the code does not answer the terminology question, keep it here as an open question rather than inventing a definition.
