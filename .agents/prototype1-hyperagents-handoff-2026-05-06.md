# Prototype 1 / HyperAgents Design Handoff

Created: 2026-05-06

Purpose: restart context for continuing design work on Prototype 1 as a
HyperAgents-like archive/self-improvement system without losing the existing
Prototype 1 authority model.

## Current User Frame

The user is increasingly putting the original Ploke product roadmap on hold to
pursue a HyperAgents-like framework:

- partly to bootstrap Ploke itself;
- partly because Prototype 1 is already addressing some safety/sandboxing
  problems called out by HyperAgents;
- specifically through typed runtime authority, bounded mutable surfaces,
  parent/child channel boundaries, History/Crown handoff, and artifact/runtime
  provenance.

Do not talk as if Prototype 1 lacks the architecture and needs a new reward
layer imported from the paper. The correct frame is:

```text
Prototype 1 already has the authority/provenance model.
HyperAgents gives us concrete archive/search/eval patterns to plug into it.
```

## Must Read First

Read these before proposing design changes:

0. `docs/active/todo/2026-05-05_long-horizon.md`
   - User-authored north star for the current phase.
   - The spine is:

```text
typed evidence/provenance
-> scoring/projection
-> archive traversal/selection
-> bounded semantic edit surface
-> longer loop observation
-> multi-ruler scaling
```

   - Do not collapse this into "implement the next slice" without preserving
     the long-term architecture.

1. `crates/ploke-eval/src/cli/prototype1_state/mod.rs`
   - This is the actual Prototype 1 conceptual model.
   - Key terms: Artifact, Runtime, Parent, Child, Successor, Journal, History,
     Crown, Tree, OperationCoordinate, Surface, Intervention.
   - Important design constraints include:
     - do not assume one global active Parent;
     - do not store current best branch as a singleton;
     - append observations and decisions;
     - do not store scores without evaluator/eval-set/policy identity;
     - do not let runtime self-report become promotion without verification.

2. `crates/ploke-eval/src/cli/prototype1_state/history.rs`
   - Current authority model for History/Crown.
   - History is the durable authority surface, not scheduler/branch reports.
   - Startup/admission and successor handoff are cross-runtime contracts.
   - Current code has partial implementation; docs explicitly distinguish
     intended vs implemented claims.

3. `crates/ploke-eval/src/successor_selection/`
   - Current first-pass successor selection module.
   - `SelectionInput` is already the local evidence bundle.
   - `SuccessorDecision` already distinguishes `Accepted`, `ExploreFrom`,
     and `Stop`.
   - `decide_generation` is currently generation-local.

4. `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs`
   - Live controller path around:
     - `resolve_child_plan`
     - `run_child_fanout`
     - `generation_selection`
     - `record_continuation_decision`
     - `spawn_and_handoff_prototype1_successor`

5. `crates/ploke-eval/src/cli/prototype1_state/evidence.rs`
   - Current focus file for typed child evidence grouping.
   - Important correction as of this handoff: declared Prototype 1 records used
     by child evidence, metrics, selection, or future scoring must cross the
     store boundary as typed `Stored<T>` records, where
     `T: Serialize + DeserializeOwned + EvidenceRecord`.
   - Deserialization failure is corruption/boundary error and should abort.
   - Do not reintroduce JSON fallback, path fallback, filename fallback, or a
     "degraded document" abstraction for semantic recovery.

6. `crates/ploke-eval/src/cli/prototype1_state/history_preview.rs`
   - Contains both typed child-evidence store access and generic
     `history preview` catalog compatibility.
   - Be careful: generic `Document` exists only for `history preview`
     catalog/projection compatibility. It must not feed typed child evidence,
     metrics, successor selection, future scoring, or History/Crown authority.

7. `docs/active/agents/prototype1-hyperagents-design-2026-05-06/README.md`
   - Index of sub-agent reports for this design thread.
   - Reports `33` through `36` supersede earlier JSON-fallback/degraded-child
     grouping language from reports `19`, `20`, `21`, `30`, and `32` for the
     normal child-evidence and metrics paths.

8. `docs/workflow/evalnomicon/drafts/runtime/child.md`
   - Execution path from child runtime through result/termination.

9. `docs/workflow/evalnomicon/drafts/runtime/parent-child-channel.md`
   - Expected parent/child communication model.
   - Parent/child communication should move toward typed role/state-indexed
     channels/transports, not random status files.

10. `docs/workflow/evalnomicon/drafts/runtime/authority.md`
   - Current authority notation and boundary language.

11. `README.md`
   - Product-level Ploke framing: Rust code graph, LLM tools, approved edits,
     mediated cargo tooling, security defaults.

12. `docs/workflow/evalnomicon/drafts/README.md`
   - Index of current evalnomicon drafts. Use it to choose the smallest
     relevant reading set instead of recursively reading every draft.

13. `docs/workflow/evalnomicon/drafts/formal-procedure-notation.md`
    - Best current notation for mixed mechanized/adjudicated procedures,
      evidential outputs, fork/merge, recording vs forwarding, and metric
      maturity. Useful when talking about "selection score" so we do not
      collapse procedure, executor, evidence, and metric into one number.

14. `docs/workflow/evalnomicon/drafts/eval-triage-rubric.md`
    - Important for Multi-SWE-bench bootstrap. It distinguishes harness
      invalidity, known frontier limits, and action-surface failures. This is a
      good first failure taxonomy for child admission/evaluation.

15. `docs/workflow/evalnomicon/drafts/prototype-1-intervention-loop-v2.md`
    - Current semantic source for the trampoline loop after the parent/child
      runtime seam became explicit. Key correction: a descendant binary must
      evaluate itself because artifact mutation and runtime semantics diverge
      until rebuild/spawn.

16. `docs/workflow/evalnomicon/drafts/prototype1-history-metrics-agent-brief.md`
    - Compact guardrail doc for projection vs authority, metrics views, and
     History/Crown discipline. Some implementation-truth bullets may be stale;
     use the file for claim-boundary discipline, then verify current code.

17. `docs/workflow/evalnomicon/drafts/module-tree-and-trait-algebra.md`
    - Useful as structural background, but its older `Policy` trait framing
      needs the correction below: policy authority is embodied by the admitted
      Rust harness surface, not an external policy object.

## HyperAgents Research Files

The local research folder is:

```text
research-papers/ploke-child-selection-2026-05-06/
```

Read:

- `hyper-agents.pdf`
- `hyper-agents.txt`
- `hyper-agents-ploke-notes.md`
- `README.md`

Important paper lessons after full reading:

- HyperAgents is not mainly a verifier/reranker paper.
- It is an archive-based self-improvement system.
- The core loop is:

```text
archive starts with a0
select parents from archive
parents self-modify to produce children
evaluate children
admit valid children to archive
repeat
```

- Their initial agent is deliberately simple.
- Most useful improvements discovered are boring process infrastructure:
  eval analysis, persistent memory, performance tracking, bias/failure
  detection, prompt templates, structured rubrics, and compute-aware planning.
- Appendix E.5 is a warning: letting the system modify parent selection
  improves over random but does not beat the simple handcrafted
  score-child-prop strategy.

Primary online references for quick verification:

- HyperAgents arXiv: `https://arxiv.org/abs/2603.19461`
- Multi-SWE-bench arXiv: `https://arxiv.org/abs/2504.02605`
- Multi-SWE-bench experiments/leaderboard submission format:
  `https://github.com/multi-swe-bench/experiments`

Do not spend context re-verifying leaderboard minutiae unless the user asks.
The design-relevant point is stable: Multi-SWE-bench is sparse and difficult,
so early bootstrapping needs dense internal evidence in addition to resolved
rate.

## Corrected Conceptual Alignment

Earlier discussion over-emphasized "reward function" and "child selection".
The better decomposition is:

```text
Child generation:
  produce candidate Artifact/Runtime

Admission:
  decide whether candidate is valid enough to enter the archive/evidence set

Evaluation:
  attach evaluator/policy/eval-set identified evidence and scores

Parent selection:
  choose future Parent from admitted archive/frontier

Transfer / stepping-stone selection:
  identify nodes whose descendants improve well, not only highest performers
```

Prototype 1 already has much of this conceptually. The gap is mostly wiring and
evidence richness, not architecture invention.

## Policy Correction

The word `Policy` has drifted across drafts. Current user correction:

```text
Policy is not primarily a runtime-loaded object or file.
The policy that matters is the contract embodied by the admitted Rust harness
surface itself.
```

For current Prototype 1, the policy-bearing surface is mainly the `ploke-eval`
Rust code that defines parent creation, child/successor execution,
History/Crown admission, selection, and handoff. Ordinary descendants must
preserve the digest of that surface until an explicit protocol-upgrade
transition exists.

Use these distinctions:

- `policy-bearing surface`
  - Rust code whose digest is part of the admitted runtime contract.
- `policy material`
  - Prompts, rubrics, task configs, timeout constants, selection parameters,
    eval-set refs, and similar material interpreted by the policy-bearing
    runtime.
- `policy projection`
  - CLI views, metrics tables, scheduler summaries, and reports. Useful, but
    not authority.
- `policy decision`
  - A recorded result of executing the admitted contract over admissible
    evidence, such as `SuccessorDecision`.

Older drafts may show a `trait Policy` shape. That can still be useful as an
implementation abstraction, but it must not imply that an external policy file
or pluggable preference is the authority. The authority claim should be:

```text
this admitted runtime, compiled from this checked surface digest,
executed this decision procedure over these evidence refs,
and recorded this decision.
```

Changing the policy-bearing Rust surface is not an ordinary child edit. It is a
protocol-upgrade or deeper-surface unlock problem.

## Surface And Unlock Model

The larger design is moving toward concentric or disjoint mutable surfaces with
typed unlocks. The rough shape:

```text
An admitted runtime may operate over a bounded set of Surfaces.
Each Surface has authority rules, evidence requirements, and admission gates.
Wider or more dangerous Surfaces unlock only after sufficient ledger-backed
cohesion evidence.
```

Possible surface ordering:

```text
Surface 0: process/rubric/prompt text
Surface 1: tool descriptions and action contracts
Surface 2: evaluator/adjudicator material
Surface 3: agent harness behavior
Surface 4: eval harness / selection / admission code
Surface 5: History/Crown/channel protocol code
```

These do not have to be strictly nested forever; some may be disjoint
capability regions. "Concentric" is a useful first model because mutation risk
generally rises as the surface approaches admission and authority code.

The unlock rule should not be a config knob such as
`editable = ["crates/ploke-eval"]`. That collapses authority into settings.
The stronger model is that the mutable surface set is itself an admitted state
of the runtime lineage:

```text
MayMutate(surface_k, runtime_R)
  iff
    R is admitted under the current History/Crown contract
    surface_k is named in the mutable surface contract
    ledger evidence satisfies unlock predicate U_k
    proposed change preserves or explicitly upgrades required invariants
```

For the future multi-ruler design, `U_k` should be a ledger-level cohesion
predicate, not local self-confidence. The point is that enough independently
admitted rulers/evaluators agree the system is stable, useful, and not drifting
before deeper mutation is allowed.

Useful distinction:

- `Surface`: what can be read or written.
- `Contract`: Rust-defined rules that interpret writes, validate evidence, and
  admit transitions.
- `Unlock`: a History/ledger-backed transition that expands the mutable surface
  or changes the contract itself.

## What We Already Have

Conceptually present in `prototype1_state/mod.rs`:

- Artifact/Runtime/Parent/Child/Successor model.
- Journal and intended History substrate.
- Crown authority and successor handoff.
- Explicit warning against singleton current-best branch.
- Evaluation records must name evaluated runtime/artifact, oracle/eval-set, and
  policy.
- Future graph model:

```text
artifact graph
runtime derivation graph
operation graph
OperationCoordinate = (generator Runtime, target Artifact)
```

Live path already has:

- `resolve_child_plan`
- `run_child_fanout`
- `generation_selection`
- `SuccessorDecision`
- `record_continuation_decision`
- `SuccessorRecord::selected_with_decision`
- `spawn_and_handoff_prototype1_successor`
- child budget min/max fanout
- `Accepted` vs `ExploreFromRejected` continuation distinction

## Current Evidence Boundary

This is the most important correction from the latest cleanup.

The normal child evidence and metrics paths are now typed-boundary paths:

```text
declared Prototype 1 record file
-> deserialize as its declared Rust record type
-> Stored<T>
-> ChildEvidenceRecords
-> ChildEvidenceSet
-> metrics / operator projection / later selection projection
```

where:

```rust
T: Serialize + DeserializeOwned + EvidenceRecord
```

The invariant is:

```text
If a declared Prototype 1 record cannot deserialize as its declared T,
the record is corrupt or the boundary is wrong.
Abort the evidence build.
Do not recover semantic facts from serde_json::Value, filenames, or paths.
```

Do not introduce any of these as normal semantic evidence paths:

- `DegradedDocument`
- `RawDocument`
- `JsonFallback`
- filename-derived branch/runtime/node identity
- path-derived branch/runtime/node identity
- loose `Document` extraction for metrics, selection, child evidence, or scoring

Paths, hashes, and ref ids are provenance for successfully typed records. They
are not semantic recovery mechanisms.

Current state after the cleanup:

- `history child-evidence` uses typed child records.
- `history metrics` uses typed `ChildEvidenceRecords` / `ChildEvidenceSet` for
  child/runtime/result/evaluation facts.
- scheduler/branch-registry selected-row markers are loaded through typed
  preview records and remain mutable projection evidence, not authority.
- generic `Document` remains only for `history preview` catalog/projection
  compatibility. It must not be used by later selection, scoring, or evidence
  assembly.

The relevant reports are:

- `33-typed-evidence-boundary-cleanup.md`
- `34-typed-evidence-boundary-review.md`
- `35-remaining-document-compat-cleanup.md`
- `36-remaining-document-compat-review.md`

Older reports that mention JSON fallback, path fallback, filename fallback, or
degraded child-evidence grouping should be read as historical context, not as
permission to reintroduce that shape.

## What Is Still Missing / Weak

Do not phrase these as "we need HyperAgents architecture"; phrase them as
Prototype 1 wiring gaps:

1. Selection is generation-local.
   - `generation_selection` only looks at current fanout outcomes.
   - It does not sample future parents from a wider archive/frontier.

2. Selection evidence is narrow and still has a boundary smell.
   - `SelectionInput` currently includes candidate, branch disposition,
     evaluation artifact path, and run comparisons.
   - It does not yet carry a richer modular evaluation/admission report.
   - The `ChildEvidence -> SelectionInput` projection should move out of
     `evidence.rs` into a selection-facing adapter so the evidence layer does
     not depend directly on successor-selection policy types.

3. Child archive admission is not a first-class distinct step.
   - Parent/History/successor admission exists conceptually and partly in code.
   - Child-as-archive-member admission with score/evidence identity should be
     made explicit before archive-aware parent selection.

4. Parent selection is not HyperAgents-style yet.
   - Current loop is current Parent -> children -> select successor.
   - It is not yet archive sampling with performance plus novelty.

5. Parent selection should not become freely self-editable first.
   - HyperAgents Appendix E.5 suggests a simple handcrafted parent selector is
     safer and stronger initially.

## Likely Next Design Direction

Short version:

```text
Keep the typed evidence boundary stable.
Enrich and persist child evaluation/admission evidence next.
Then make parent selection archive-aware.
Only later allow proposed edits to parent-selection policy behind gates.
```

Immediate next implementation direction after compaction:

1. Do not proceed to scoring/archive traversal until the typed evidence boundary
   remains clean under review.
2. Move `ChildEvidence -> SelectionInput` out of `evidence.rs` into a
   selection-facing projection module.
3. Attach/load `RunRecord`, `RunRegistration`, `RunArtifactRefs`, and protocol
   artifact refs under compared instances, still as typed records.
4. Define first score projections over typed evidence with evaluator/eval-set/
   procedure identity.
5. Only then implement HyperAgents-style archive traversal.

First archive-aware selector to consider:

```text
alpha_i = performance(agent_i)
alpha_mid = average score of top m agents, m = 3
s_i = sigmoid(lambda * (alpha_i - alpha_mid))
h_i = 1 / (1 + admitted_child_count_i)
w_i = s_i * h_i
p_i = w_i / sum(w)
sample parents from categorical p
```

For Ploke:

- `alpha_i` should be derived from modular evidence, not a naked score.
- `admitted_child_count_i` should count valid/admitted children, not failed
  attempts.
- Sampling should come after records are good enough to explain why a parent was
  eligible and selected.

Also consider descendant-growth score later:

```text
G_gamma(i) = average over descendants j of (alpha_j - alpha_i) * gamma^dist(i,j)
```

This is for identifying stepping-stone runtimes, not immediate local winners.

## Editable Surface Direction

Near-term self-editing should target low-risk process surfaces, not
policy-bearing runtime code:

- target-selection prompt/rubric
- child-generation prompt/rubric
- tool-use guidance
- validation checklist
- failure taxonomy
- evaluation summary generation
- memory/performance-summary text

Do not make `ploke-eval` policy-bearing surface ordinary edit scope until a
protocol-upgrade transition exists. The module docs are explicit that ordinary
descendants must preserve the policy-bearing surface digest.

## HyperAgents Bootstrap Applied To Ploke Tools

The user wants the next design pass rooted in what HyperAgents actually did.
Their useful bootstrap pattern is:

```text
archive of agents
-> select parents
-> parent self-modifies into child
-> evaluate child
-> admit valid child into archive
-> repeat
```

For Ploke, translate that as:

```text
task agent        = ploke-tui / ploke-eval child solving Rust issue tasks
meta agent        = child-generation / self-improvement procedure
agent archive     = admitted Runtime/Artifact records in Prototype 1 History
valid child       = candidate that passes harness/admission gates
performance       = external eval + internal tool/process evidence
editable program  = bounded surfaces we deliberately unlock
```

Multi-SWE-bench Rust is a sparse, hard benchmark. Early runs should optimize for
learning signal, not only final resolved-rate. The sparse external score should
be paired with dense internal process evidence.

External sparse score:

```text
Multi-SWE resolved / failed
```

Dense internal process evidence:

```text
target localized?
correct file inspected?
useful semantic/BM25/code-item lookup?
repeated search/tool churn?
edit attempted?
patch applied?
cargo/test validation attempted?
failure class?
suspected tool-surface bottleneck?
```

The first admission/evaluation taxonomy should reuse
`eval-triage-rubric.md`:

```text
Harness Invalidity:
  fix before treating the run as evidence

Known Frontier Limit:
  record and do not confuse with tool-surface improvement

Action-Surface Failure:
  prime target for self-improvement
```

Suggested staged unlock for tools:

```text
Phase A:
  fixed tool code
  editable tool descriptions / rubrics / failure taxonomy / memory

Phase B:
  archive children with dense tool-friction evidence, not only pass/fail

Phase C:
  let children propose tool-schema or tool-contract changes

Phase D:
  only unlock tool implementation edits after repeated evidence shows
  the bottleneck is action-surface/tool friction,
  not model failure, harness invalidity, or known Rust frontier limit
```

This follows the HyperAgents lesson: make the loop able to see, score,
preserve, and reuse improvements before making deeper self-editing available.
It also respects Appendix E.5: keep parent selection/admission code handcrafted
at first; allow self-improvement on process/tool-use surfaces before letting
the system mutate the mechanism that defines selection authority.

## Current Live Run

A 3-generation telemetry run was started:

```text
campaign: p1-3gen-ripgrep-telemetry-20260506-1
worktree: /home/brasides/.ploke-eval/worktrees/p1-3gen-ripgrep-telemetry-20260506-1
gen0 parent: node-9d68e7f4312c5f49
```

Watcher confirmed provider-attempt span attribution is fixed for parent calls:

```text
gen node      branch       request attempt elapsed outcome
0   node-9d68 prototype1-p 1       1/1     14.6s   completed
```

The next useful observation is whether child calls in generation 1 also carry
the right gen/node/branch fields.

## Avoid These Mistakes

- Do not say "we need an archive model" as if Prototype 1 does not already
  encode it conceptually.
- Do not propose a parallel reward/report object that bypasses
  History/Crown/scheduler/SuccessorDecision.
- Do not reintroduce loose `Document`, JSON fallback, filename fallback, or path
  fallback into child evidence, metrics, selection, scoring, or archive
  traversal.
- Do not call corrupt typed records "degraded evidence"; corrupt typed records
  are boundary errors.
- Do not store scores without evaluator/eval-set/policy identity.
- Do not collapse `Accepted`, `ExploreFrom`, and rejected continuation into one
  generic "selected" state.
- Do not suggest making parent selection self-editable as the first step.
- Do not overread mutable scheduler/report projections as authority.
- Do not use branch names/worktree paths as semantic Artifact identity.

## Good Next Conversation Starting Point

Ask:

```text
How should we define the first dense child admission/evaluation record for
tool-use self-improvement, using HyperAgents as the archive loop and
eval-triage-rubric.md as the failure taxonomy?
```

The likely next concrete design object is not archive-aware parent selection
yet. It is a durable child evidence/admission record rich enough to support:

- external Multi-SWE result evidence
- dense tool-friction/process evidence
- failure-class triage
- archive admission without implying successful continuation
- later score-child-prop parent selection over admitted archive members

Keep the conversation grounded in the existing Prototype 1 model. Do not
propose another standalone reward/report hierarchy.
