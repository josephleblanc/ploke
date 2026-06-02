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

- Should result reports carry separate typed selected-child and successor-runtime records more explicitly, instead of flattening those relationships into long id field names?

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

### 13. Model id / provider / route source across profile and campaign

Current working definition:

- `CampaignManifest.model_id` is the Manifest Model ID: the campaign JSON's optional persisted model field.
- `ResolvedCampaignConfig.model_id` is the Resolved Model ID: the runtime model id after campaign defaults and validation.
- Run-profile `[model].id` is the Profile Model ID: an operator default that setup may copy into the generated campaign manifest.
- `provider_slug` is an OpenRouter provider slug; direct Google is a route source and should not be described as an OpenRouter provider slug.

Why it conflicts:

- Setup CLI, campaign manifests, provider preferences, protocol config, protocol artifacts, and run profiles all carry model/provider words with slightly different authority.
- Direct Google can be displayed as `google`, while resolved direct-Google campaign/provider state often stores `provider_slug = None`.
- `--protocol-model-id` sounds independent, but current Prototype 1 baseline setup requires protocol and eval to collapse to the same model/route/provider.

Suggested convention:

- Use `Manifest Model ID`, `Resolved Model ID`, `Profile Model ID`, and `Protocol Model ID` in prose before using unqualified `model_id`.
- Say `direct Google route`, not `google provider slug`, unless the code path is specifically accepting the compatibility string `google`.
- In config docs, explain whether a field is a setup source, a persisted campaign source of truth, a runtime materialization, or provenance written after an API call.

Open question:

- Should the code introduce a typed `ModelRoute { id, route, provider }` carrier at profile->campaign and campaign->runtime boundaries to reduce the current flat field overlap?

## Resolution recommendations from Evalnomicon drafts

This pass reads the conflict list against the current Evalnomicon draft packets,
especially:

- `docs/workflow/evalnomicon/drafts/start-here/stage-overview.md`
- `docs/workflow/evalnomicon/drafts/start-here/evidence-and-artifacts.md`
- `docs/workflow/evalnomicon/drafts/runtime/authority.md`
- `docs/workflow/evalnomicon/drafts/runtime/child.md`
- `docs/workflow/evalnomicon/drafts/runtime/parent-child-channel.md`
- `docs/workflow/evalnomicon/drafts/runtime/artifact-runtime-lineage.md`
- `docs/workflow/evalnomicon/drafts/runtime/loop.md`
- `docs/workflow/evalnomicon/drafts/history/crown-authority-background.md`
- `docs/workflow/evalnomicon/drafts/edit-surface/model.md`
- `docs/workflow/evalnomicon/drafts/persistence/map-2026-05-03/synthesis.md`
- `docs/workflow/evalnomicon/drafts/selection/README.md`
- book-level summaries in `docs/workflow/evalnomicon/src/prototype1/`

### Source precedence

When terms conflict, prefer this order:

1. Current code and current book-level Evalnomicon pages.
2. `drafts/start-here/` operator-facing pages, because they are the most concise
   current public vocabulary.
3. Current dated draft packets in `runtime/`, `edit-surface/`, `selection/`,
   `persistence/`, and `observability` when they describe the same current
   implementation boundary.
4. Older chat-history/framework notes only as conceptual background.

This precedence matters because some older notes say `baseline -> patch ->
rerun`; the current runtime-loop docs explicitly replace that with runtime
succession: parent runtime, descendant artifact, child runtime, selected
successor, successor admission.

### Global naming rule

Use role/state/authority names for control-flow claims and use projection names
for files/views. A file path does not confer authority by itself. The draft
runtime authority model says a runtime gains a role only through an admitted
execution path and role-shaped surface (`runtime/authority.md:25-50`,
`runtime/authority.md:200-224`). The channel plan is stricter: lifecycle state
must be driven by per-runtime channel evidence, not scheduler/journal/latest
result projections (`runtime/parent-child-channel.md:9-15`,
`runtime/parent-child-channel.md:202-231`). The evidence page gives the same
order: sealed History first, then parent/profile admission evidence, transition
journal, typed boxes/invocations, benchmark/protocol artifacts, and finally
scheduler/branch/node/monitor/dashboard projections (`start-here/evidence-and-artifacts.md:6-25`).

Hard variable/field naming rule: do not name variables or fields with more than
three semantic parts. Three is an upper bound, not a goal. If a proposed name
needs four or more parts, the relationship should be encoded structurally with a
typed carrier, nested field, enum variant, or typestate parameter instead of a
flattened descriptive name. For example, do not introduce fields like
`selected_candidate_membership_id` or `selected_child_node_id`. Prefer a
structural shape such as `Selection { chosen: CandidateMembership }`,
`SelectedChild { node: SearchNodeRef }`, or `Runtime<Successor> { id }`. The
short field name is evidence that the relationship is preserved by the type
structure and can be checked by the compiler.

### Recommended resolutions by conflict

| Conflict | Recommended resolution |
| --- | --- |
| Node | In Prototype 1 docs, use `search node` or `Prototype 1 search node` for `Prototype1NodeRecord`/scheduler-owned node records. Use `code graph node` or `syntax node` for parsed/indexed code facts. Use `candidate artifact` or `descendant artifact` for the material code state. Rationale: persistence docs classify `nodes/<node-id>/node.json` as a scheduler-owned node record/projection (`persistence/map-2026-05-03/synthesis.md:40-42`), while the edit-surface model says code graph nodes are a derived view over an Artifact (`edit-surface/model.md:190-210`). |
| Parent id / parent node id / node id | Keep `node_id` as the active search-node id in code discussions. In prose, write `active parent search node id` for the parent node that owns child-plan publication, and `predecessor search node id` or `search-tree parent node id` for the edge to the prior node. Do not turn those prose phrases into flat field names. Prefer structural carriers such as `ParentIdentity { id, node }`, `SearchNode { id }`, or `NodeEdge { parent, child }`, where the surrounding type names the relationship and fields stay short. Rationale: the start-here docs use `parent node id` for child-plan ownership (`stage-overview.md:91-103`) and the operator map separates active parent checkout, campaign root, and child worktree (`prototype1-loop-operator.md:12-28`). |
| Child vs successor | Adopt the lifecycle ladder from the runtime-loop draft: `proposed child`, `realized child artifact`, `built child`, `acknowledged child runtime`, `evaluated child`, `selected successor`, `successor runtime` (`runtime/loop.md:184-210`). Do not say a child process becomes the successor. Say an evaluated child/candidate artifact is selected, then a fresh successor runtime is launched/admitted. Persisted reports should keep the selected child and successor runtime as separate typed values, not fields named like `selected_child_node_id` and `successor_runtime_id`. Rationale: the child role is leaf evaluation only (`runtime/child.md:20-49`), while successor validation precedes entry into parent authority (`runtime/authority.md:191-199`). |
| Runtime vs run vs turn | Use `runtime` for an OS process hydrated from an Artifact and occupying exactly one `Role<State>` (`edit-surface/model.md:303-340`). Use `runtime attempt` where a `RuntimeId` is present. Use `benchmark run` or `eval run` for `RunRecord` / run manifests. Use `parent turn` or `parent control turn` for one controller pass through the active parent. Use `agent turn` for LLM/tool turns inside run records. Avoid unqualified `run` in new docs unless the command name itself uses it. |
| Branch / treatment branch / artifact branch / git branch | Keep `treatment branch id` as prose for branch-registry/candidate identity, but prefer a typed `TreatmentBranch { id }` or equivalent carrier in code. Use `GitRef`, `CheckoutRef`, or another typed ref for VCS names. Prefer `selected Artifact` over `selected branch` when discussing handoff, because handoff installs the selected Artifact into the active checkout before launching the successor (`stage-overview.md:175-188`, `prototype1-loop-operator.md:92-112`). Avoid long flat fields such as `artifact_checkout_ref` if the relationship can be represented as `Artifact { checkout: CheckoutRef }` or `Checkout { git: GitRef }`. |
| Artifact vs worktree vs source state | Use `Artifact` for source/tree material that can hydrate a Runtime (`edit-surface/model.md:119-139`). Use `derived Artifact` for a checked/applied candidate transition (`edit-surface/model.md:497-568`). Use `worktree` for a mutable local filesystem handle; it is not durable identity. Use `source state` for candidate-generation input content/state, not for the whole checkout unless a specific record says that. Rationale: the lineage draft warns that an uncommitted worktree can be lost and therefore is a poor graph identity (`runtime/artifact-runtime-lineage.md:68-91`), while operator docs warn child worktrees are temporary surfaces, not successor homes (`stage-overview.md:51-60`). |
| Authority vs projection | Replace loose `authority` with one of: `authority substrate`, `authority-bearing admission input`, `transition evidence`, `runtime channel evidence`, or `projection`. Sealed History is the intended authority substrate; parent identity/profile commitments and invocations are admission inputs; transition journal entries are transition evidence; node/scheduler/branch/latest-result/dashboard files are projections unless explicitly converted/admitted. Rationale: History/Crown docs say scheduler snapshots, branch registries, CLI reports, and dashboards are projections, not History authority (`history-crown.md:5-18`), and the channel plan says attempt results/projections should not drive live lifecycle advancement (`runtime/parent-child-channel.md:202-231`). |
| History / lineage / Crown / scheduler tree | Use `History lineage` for admitted lineage facts and Crown handoff claims; use `search tree` or `scheduler frontier` for candidate planning/status. Do not call scheduler state “History” unless the record has been sealed/admitted or explicitly imported by policy. Rationale: book-level `History` is an authenticated store over sealed lineage-local blocks and `Crown` is one-at-a-time lineage mutation authority, not a process id, git branch, path, or global singleton (`history-crown.md:5-30`). The persistence map classifies scheduler/node/branch surfaces as mutable projections and sealed block streams as local History authority (`persistence/map-2026-05-03/synthesis.md:40-60`). |
| Closure | Keep the code term `ClosureState`, but introduce it in docs as `campaign completeness / closure state`. Use `eval closure section` and `protocol closure section` when referring to subparts. Explicitly say generation-0 baseline closure is not the same thing as later parent baselines. Rationale: stage docs say generation 0 enters baseline eval/protocol via closure, but later candidate planning waits for both (`stage-overview.md:62-84`); runtime-loop docs say generation > 0 parent baselines are promoted from selected-child treatment reports, not the original generation-0 closure (`runtime/loop.md:231-252`). |
| Protocol procedure names | Use `procedure id` for canonical docs/config ids and keep them kebab-case: `tool-call-intent-segments`, `tool-call-review`, `tool-call-segment-review`. Treat snake_case artifact kind strings and filename stems as storage/schema compatibility names, not public procedure names. Add an alias table near config docs if users still encounter `tool_call_intent_segmentation` or similar legacy names. Rationale: protocol artifacts are persisted under procedure-specific filenames/envelopes and are inspected as stored evidence (`persistence/map-2026-05-03/synthesis.md:72-78`); docs should not make filename/kind compatibility drive public vocabulary. |
| MBE / MSB / Multi-SWE-Bench | Public docs should spell out `Multi-SWE-Bench` on first use. Use `MSB` only for benchmark/submission concepts. Use `Multi-SWE-Bench evaluator` or ``mbe` module` for the local module path instead of implying `MBE` is the canonical user-facing acronym. If future code cleanup is allowed, consider renaming module-facing docs toward `multi_swe_bench` rather than inventing an additional acronym. Rationale: the persistence map names the submission artifact as `multi-swe-bench-submission.jsonl` (`persistence/map-2026-05-03/synthesis.md:71`) while older conceptual notes use `MBE` loosely for external oracle/evaluator background. |
| Baseline vs parent | Use three explicit terms: `generation-0 campaign baseline`, `active parent baseline evidence`, and `candidate/treatment evidence`. Answer the open question as yes: branch evaluation reports and selection displays should label baseline provenance, at least as `baseline_source = generation0_closure | selected_child_promotion`, plus the baseline campaign/branch/run-record paths. Rationale: the current runtime-loop draft says later parents compare against selected-child treatment evidence promoted into the next parent baseline (`runtime/loop.md:231-252`), while stage docs say generation-0 baseline closure is the startup gate (`stage-overview.md:62-84`). |
| Model id / provider / route source | Use role-qualified prose terms: `Profile Model ID` for `[model].id`, `Manifest Model ID` for `CampaignManifest.model_id`, `Resolved Model ID` for `ResolvedCampaignConfig.model_id`, and `Protocol Model ID` for protocol JSON config or setup protocol overrides. Treat `provider_slug` as an OpenRouter concept; say `direct Google route` for direct-Google calls. Keep route/model/provider relationships in a typed carrier if refactoring, not by adding longer flat field names. |

### Suggested implementation/doc follow-ups

1. Add a short “Terminology precedence” note to the walkthrough glossary:
   role/state/authority terms win for control-flow claims; file names are named
   as projections unless the doc identifies an admission transition.
2. In future passive record docs, do not solve ambiguity by concatenating more
   qualifiers into field names. Use the three-part limit as a design alarm:
   - prefer `SearchNode { id }` over `search_node_id` when the surrounding type
     can carry the search-node meaning;
   - prefer `Parent { node }` or `ParentIdentity { node }` over
     `active_parent_search_node_id`;
   - prefer `NodeEdge { parent, child }` over `predecessor_search_node_id`;
   - prefer `Runtime<Attempt> { id }` or `Attempt { runtime }` over
     `runtime_attempt_id` when the distinction is structural;
   - prefer `Checkout { git: GitRef }` over `checkout_git_ref`;
   - prefer `Selection { chosen }`, `SelectedChild { node }`, and
     `Runtime<Successor> { id }` over flat fields such as
     `selected_candidate_membership_id`, `selected_child_node_id`, or
     `successor_runtime_id`.
3. In operator docs, prefer the stage vocabulary from `drafts/start-here/`:
   setup/admission, baseline eval, baseline protocol, child planning,
   materialize, build, spawn, observe, select, handoff, complete/blocked.
4. In architecture docs, prefer the runtime succession wording from the current
   book pages: active parent runtime -> descendant candidate/artifact -> child
   runtime -> selected successor artifact -> successor runtime -> next parent.
5. Do not erase the open questions above yet. Treat these recommendations as the
   proposed answer set; close each question only after either code renames or
   durable glossary updates make the convention hard to miss.

## Follow-up process

When updating the walkthrough glossary:

1. Add or refine the source-grounded working definition in the walkthrough.
2. If usage is overloaded, add/update an entry in this companion document.
3. Prefer source file/line evidence over inference.
4. If the code does not answer the terminology question, keep it here as an open question rather than inventing a definition.
