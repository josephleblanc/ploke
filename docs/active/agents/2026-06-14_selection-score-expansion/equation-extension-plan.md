# 2026-06-14 selection-score equation extension plan

Task: extend the selection-mechanism equation packet with source-visible archive/admission/parent-selection formulas that are not yet represented cleanly.

Primary artifact for Kanban task `t_848502e9`. This is a documentation/research plan only; no Rust or wiki files were edited by this run.

Inputs inspected:
- `/home/brasides/code/ploke/docs/active/agents/2026-06-13_selection-score-expansion/coverage-map.md` lines 1-149.
- `/home/brasides/wiki/queries/ploke/selection-scoring/index.md` lines 16-58.
- `/home/brasides/wiki/queries/ploke/selection-scoring/archive-parent-selection-mechanisms.md` lines 28-162.
- `/home/brasides/wiki/queries/ploke/selection-scoring/ledgers/mechanism-ledger.md` lines 21-82.
- `/home/brasides/code/ploke/.agents/hyper-agents.txt` lines 276-290, 338-348, 456-463, 1266-1326, 1331-1346, 1352-1360, 1765-1770, 3849-3933.
- `/home/brasides/code/ploke/crates/ploke-selection-score/src/papers/hyperagents.rs` lines 1-315.
- `/home/brasides/code/ploke/crates/ploke-selection-score/src/catalog/registry.rs` lines 44-89.
- June-03 nearby paper notes under `/home/brasides/wiki/queries/research/ploke-arxiv-cs-ai-2026-06-02/new-papers/2026-06-03/papers/`, each cited below by line anchor.

Status words:
- `promote`: source-visible enough to write as a formula/predicate in the selection-scoring packet now.
- `candidate-patch`: source-visible enough for a wiki patch note, but should be labelled draft or interpretive.
- `unresolved`: keep out of final equations until a primary-source deep read supplies missing thresholds, objective functions, schemas, or benchmark definitions.

Exactness labels:
- `faithful-formalization`: direct formula, predicate, or pseudocode visible in the cited source.
- `interpretive-compression`: compact formal wrapper around prose/code-like source fragments.
- `faithful-draft`: ledger/note calls it faithful, but current source basis is still a local skim/abstract note rather than fully audited primary text.
- `not-promoted`: no clean formula is promoted; the lead remains explicitly unresolved.

## 1. Promotion queue

| ID | Mechanism | Action | Source anchors | Current crate/wiki anchors | Exactness | Notes |
| --- | --- | --- | --- | --- | --- | --- |
| E1-HYP-01 | DGM-H parent weights and categorical parent draw | promote / already mostly represented | `/home/brasides/code/ploke/.agents/hyper-agents.txt:1266-1326`; prose pointer at `:276-290` | wiki `archive-parent-selection-mechanisms.md:40-97`; code `hyperagents.rs:66-131`; registry `registry.rs:46-56` | `faithful-formalization` | Keep this as the clean authority row for archive parent selection. It is parent sampling, not archive admission and not Ploke live child selection. |
| E1-HYP-02 | DGM-H valid-child archive admission | promote / already represented | `/home/brasides/code/ploke/.agents/hyper-agents.txt:1331-1346`; prose at `:285-290` | wiki `archive-parent-selection-mechanisms.md:98-120`; code `hyperagents.rs:133-139`; registry `registry.rs:57-67` | `faithful-formalization` | Admission is the hard `IsValid(a')` / compiled-child predicate. It should remain separate from parent weights. |
| E1-HYP-03 | DGM-H staged gate with failed-gate zeroing | promote / tighten exactness label | `/home/brasides/code/ploke/.agents/hyper-agents.txt:338-348`, `:456-463`, paper-review gate `:1765-1770` | wiki `archive-parent-selection-mechanisms.md:121-147`; code `hyperagents.rs:141-202`; registry `registry.rs:68-78` | `faithful-formalization` for the zeroing/gates; `interpretive-compression` only for the compact generic notation | The source explicitly says agents failing staged evaluation get zero on remaining tasks (`:456-459`). Domain gates are source-visible enough to keep as named predicates. |
| E1-HYP-04 | DGM-H cross-domain average parent score | promote / already represented | `/home/brasides/code/ploke/.agents/hyper-agents.txt:1352-1360` plus staged-score source above | wiki `archive-parent-selection-mechanisms.md:130-147`; code `hyperagents.rs:156-170`; registry `registry.rs:68-78` | `faithful-formalization` | The parent score is average performance across domains after staged zeroing/validation-vs-training choice. |
| E1-HYP-05 | Appendix E.5 learned-selector families: UCB, softmax, adaptive exploration, component score | promote as interpretive patch, not as default selector | `/home/brasides/code/ploke/.agents/hyper-agents.txt:3849-3933` | code `hyperagents.rs:204-307`; registry `registry.rs:79-89`; wiki currently only parks it at `archive-parent-selection-mechanisms.md:148-154` | `interpretive-compression` | This is the main clean extension: source-visible formulas exist, but they are qualitative learned traces and Algorithm 5, not a single stable selector. Add a wiki formula section with a warning label. |
| E1-SKILLDAG-01 | SkillDAG typed skill graph edge admission | candidate-patch; do not promote as final formula without deep read | note `/home/brasides/wiki/queries/research/ploke-arxiv-cs-ai-2026-06-02/new-papers/2026-06-03/papers/2606.03056-skilldag.md:30-49`; ledger `mechanism-ledger.md:68` | no crate helper; coverage map `coverage-map.md:106` | `faithful-draft` | Candidate predicate is clean enough for a draft note: admit edge only if evidence-backed, acyclic, non-contradictory, and rollback-logged. Missing primary-PDF line anchors and any ranking/probability rule. |
| E1-DELTAMEM-01 | DELTAMEM residual-tree write/admission rule | candidate-patch; do not promote as final formula without deep read | note `/home/brasides/wiki/queries/research/ploke-arxiv-cs-ai-2026-06-02/new-papers/2026-06-03/papers/2606.03083-deltamem.md:30-49`; ledger `mechanism-ledger.md:70` | no crate helper; coverage map `coverage-map.md:108` | `faithful-draft` | Candidate predicate: write a residual delta only when retrieved parent chain fails to cover the episode; consolidate frequent paths into new roots. Missing thresholds and retrieval objective details. |
| E1-PRIM-01 | Reasoning-primitive induction admission | candidate-patch; do not promote as final formula without deep read | note `/home/brasides/wiki/queries/research/ploke-arxiv-cs-ai-2026-06-02/new-papers/2026-06-03/papers/2606.02994-reasoning-primitives.md:30-49`; ledger `mechanism-ledger.md:75` | no crate helper; coverage map `coverage-map.md:113` | `faithful-draft` | Candidate predicate: promote recurrent successful trace clusters into pseudo-tools. Missing clustering criterion, frequency/usefulness threshold, leakage controls, and demotion path. |

## 2. Promoted equation packet details

### E1-HYP-01: DGM-H parent weights

Source-visible equation set:

```math
A_t = \{a_0, a_1, \ldots, a_t\}
```

```math
\alpha_i = \operatorname{performance}(a_i)
```

```math
\alpha_{mid}(t) = \frac{1}{m}\sum_{a_j \in T_t^{(m)}} \alpha_j,
\qquad m=3 \text{ in the reported experiments}
```

```math
s_i = \sigma(\lambda(\alpha_i - \alpha_{mid}(t))),
\qquad \sigma(x)=\frac{1}{1+e^{-x}},
\qquad \lambda=10
```

```math
h_i = \frac{1}{1+n_i}
```

```math
w_i = s_i h_i
```

```math
p_i =
\begin{cases}
\dfrac{w_i}{\sum_{j=0}^{t}w_j}, & \sum_{j=0}^{t}w_j>0 \\
\dfrac{1}{t+1}, & \text{otherwise}
\end{cases}
```

```math
\{\text{parents}\} \sim \operatorname{Categorical}(\{p_i\}_{i=0}^{t})
```

Source anchors: `/home/brasides/code/ploke/.agents/hyper-agents.txt:1266-1326`. Code anchors: `hyperagents.rs:66-131`. Exactness: `faithful-formalization`.

Authority boundary: this samples archive parents; it does not decide whether a generated child enters the archive and does not execute a Ploke selection decision.

### E1-HYP-02: DGM-H archive admission

Source-visible predicate:

```math
A_{t+1}=\begin{cases}
A_t\cup\{(a',s')\}, & \operatorname{IsValid}(a') \\
A_t, & \text{otherwise}
\end{cases}
```

Source anchors: `/home/brasides/code/ploke/.agents/hyper-agents.txt:1331-1346`, with prose context at `:276-290`. Code anchors: `hyperagents.rs:133-139`. Exactness: `faithful-formalization`.

Patch note: use `compiled/valid child` as the local predicate name. Do not soften this into a score penalty; the source gate is hard.

### E1-HYP-03/E1-HYP-04: staged gates and cross-domain average

Compact packet formula:

```math
\widetilde q_d(a)=
\begin{cases}
q^{full}_d(a), & \operatorname{Gate}_d(a) \\
0, & \neg \operatorname{Gate}_d(a)
\end{cases}
```

```math
\alpha_i = \frac{1}{|D|}\sum_{d\in D}\widetilde q_d(a_i)
```

Domain gate anchors:
- Generic staged evaluation and validation-vs-training choice: `/home/brasides/code/ploke/.agents/hyper-agents.txt:338-348`.
- Cross-domain failure zeroing: `/home/brasides/code/ploke/.agents/hyper-agents.txt:456-463`.
- Multi-domain average parent score: `/home/brasides/code/ploke/.agents/hyper-agents.txt:1352-1360`.
- Paper-review gate example: `/home/brasides/code/ploke/.agents/hyper-agents.txt:1765-1770`.

Code anchors: `hyperagents.rs:141-202`. Exactness: `faithful-formalization` for the named gates/zeroing and average; `interpretive-compression` if the wiki uses one generic `Gate_d` notation for all domains.

Patch note: the existing wiki line `archive-parent-selection-mechanisms.md:39` says staged/multi-domain summaries are interpretive. Tighten this to: `faithful-formalization for the named staged gates, zeroing rule, and cross-domain average; interpretive-compression for the generic \operatorname{Gate}_d wrapper`.

### E1-HYP-05: Appendix E.5 learned-selector families

This is the highest-value extension because the current wiki parks the mechanism as a later candidate, while the source and crate already expose formula-like fragments.

#### E1-HYP-05a: modifiable-selector authority loop

```math
(a_{latest}, \_) = \operatorname{last}(A_t)
```

```math
P_t = a_{latest}.\operatorname{SelectParents}(A_t)
```

```math
a' = a.\operatorname{Modify}(a, A_t)
```

```math
A_{t+1}=\begin{cases}
A_t\cup\{(a',s')\}, & \operatorname{IsValid}(a') \\
A_t, & \text{otherwise}
\end{cases}
```

Source anchor: `/home/brasides/code/ploke/.agents/hyper-agents.txt:3915-3933`. Exactness: `faithful-formalization` for Algorithm 5 pseudocode, but selector internals remain `interpretive-compression`.

Patch note: emphasize that parent-selection authority is delegated to the most recently admitted archive agent, which is a different authority shape from fixed `score-child-prop`.

#### E1-HYP-05b: UCB-style learned selector

Source-visible fragment:

```math
\operatorname{exploration\_bonus}_i
= \eta \sqrt{\frac{\log(N+1)}{n_i+1}}
```

```math
\operatorname{ucb\_score}_i
= \operatorname{normalized\_score}_i + \operatorname{exploration\_bonus}_i
```

where `N = total_children`, `n_i = children` for candidate `i`, and `\eta = exploration_weight`.

Source anchor: `/home/brasides/code/ploke/.agents/hyper-agents.txt:3868-3879`. Code anchor: `hyperagents.rs:232-253`. Exactness: `interpretive-compression` because the source presents qualitative learned variants rather than one canonical algorithm.

#### E1-HYP-05c: temperature softmax parent probabilities

Source-visible fragment:

```math
\tilde p_i = \exp(score_i / \tau),
\qquad
p_i = \frac{\tilde p_i}{\sum_j \tilde p_j}
```

```math
a_{parent}\sim\operatorname{Categorical}(\{p_i\})
```

Source anchor: `/home/brasides/code/ploke/.agents/hyper-agents.txt:3880-3887`. Code anchor: `hyperagents.rs:204-230`. Exactness: `interpretive-compression`.

Patch note: do not claim `\tau` adaptation is specified; the source says temperature becomes adaptive based on archive size, recent progress, or stagnation but does not give one formula at `:3887-3888`.

#### E1-HYP-05d: component selector and stagnation-triggered exploration

Source-visible fragment:

```math
score_i =
( normalized\_score_i
+ \eta \cdot exploration\_bonus_i
+ diversity\_bonus_i
+ recency\_bonus_i ) \cdot elite\_bonus_i
```

```math
\eta_{t+1}=\begin{cases}
1.4\eta_t, & \operatorname{Var}(score)<0.01 \\
\eta_t, & \text{otherwise}
\end{cases}
```

Source anchor: `/home/brasides/code/ploke/.agents/hyper-agents.txt:3887-3914`. Code anchors: `hyperagents.rs:255-307`. Exactness: `interpretive-compression`.

Patch note: component names are source-visible, but diversity/recency/elite definitions are not. Keep them as named components, not fully specified equations.

## 3. June-03 nearby mechanisms: candidate equations and unresolved leads

### E1-SKILLDAG-01: typed skill graph admission

Candidate equation for patch notes only:

```math
\operatorname{AdmitEdge}(e, G)=
\operatorname{EvidenceBacked}(e)\land
\operatorname{Acyclic}(G\cup\{e\})\land
\operatorname{NonContradictory}(G\cup\{e\})\land
\operatorname{RollbackLogged}(e)
```

Source anchors: local note `/home/brasides/wiki/queries/research/ploke-arxiv-cs-ai-2026-06-02/new-papers/2026-06-03/papers/2606.03056-skilldag.md:30-49`; ledger `/home/brasides/wiki/queries/ploke/selection-scoring/ledgers/mechanism-ledger.md:68`. Exactness label: `faithful-draft`.

Unresolved: no primary PDF line anchors were inspected in this run; source note says exact metric/code verification remains open at `skilldag.md:47-49`. Do not implement a selector probability or skill ranking formula from this alone.

### E1-EVOTRAINER-01: diagnostic branch admission

Candidate patch shape:

```math
status(v) \in \{keep, edit, merge, prune\}
= F(diagnostic\_gap_v, rollout\_evidence_v, backtest_v, human\_gate_v)
```

Source anchors: note `/home/brasides/wiki/queries/research/ploke-arxiv-cs-ai-2026-06-02/new-papers/2026-06-03/papers/2606.03108-evotrainer.md:30-49`; ledger `mechanism-ledger.md:69`. Exactness label: `interpretive-compression`.

Unresolved: no exact `F` is source-visible in the local note. Keep as a schema/admission family, not a formula.

### E1-DELTAMEM-01: residual tree write rule

Candidate equation for patch notes only:

```math
\operatorname{WriteDelta}(x, C)=\neg \operatorname{Covers}(\operatorname{RetrieveChain}(x), x)
```

```math
\operatorname{Consolidate}(path) \Leftarrow \operatorname{Frequency}(path) \ge \theta_{root}
```

Source anchors: note `/home/brasides/wiki/queries/research/ploke-arxiv-cs-ai-2026-06-02/new-papers/2026-06-03/papers/2606.03083-deltamem.md:30-49`; ledger `mechanism-ledger.md:70`. Exactness label: `faithful-draft`.

Unresolved: thresholds, failure-penalized similarity objective, and consolidation details are not reconstructed; source note says threshold-search details and benchmark tables remain open at `deltamem.md:47-49`.

### E1-STEPFINDER-01: step-level failure attribution

Candidate patch shape:

```math
\hat s = \arg\max_{s_k\in\tau}\operatorname{FailureScore}(s_k\mid \tau)
```

Source anchors: note `/home/brasides/wiki/queries/research/ploke-arxiv-cs-ai-2026-06-02/new-papers/2026-06-03/papers/2606.03467-stepfinder.md:30-49`; ledger `mechanism-ledger.md:72`. Exactness label: `interpretive-compression`.

Unresolved: local note is abstract-grounded only (`stepfinder.md:47-49`); exact model architecture, labels, and metric definition remain unresolved. This is an evaluation/repair routing mechanism, not archive admission.

### E1-HANDOFF-01: handoff debt / rediscovery cost

Candidate metric shape:

```math
\Delta Events(view)=Events_{repo\ only}-Events_{view}
```

```math
\Delta Tokens(view)=Tokens_{repo\ only}-Tokens_{view}
```

Source anchors: note `/home/brasides/wiki/queries/research/ploke-arxiv-cs-ai-2026-06-02/new-papers/2026-06-03/papers/2606.02875-handoff-debt.md:30-49`; ledger `mechanism-ledger.md:73`. Exactness label: `interpretive-compression`.

Unresolved: local note is abstract-grounded only (`handoff-debt.md:47-49`); structured-note schema and deterministic handoff-point protocol remain unresolved. This is observability/protocol scoring, not parent selection.

### E1-SAGE-01: socialized evolution condition comparator

Candidate metric shape:

```math
\Delta_{social}(M,A)=Score(M,A,SocialEvo)-Score(M,A,SelfEvo)
```

with condition variables for filtered peer history, raw peer history, reflective summary, model family, and arena.

Source anchors: note `/home/brasides/wiki/queries/research/ploke-arxiv-cs-ai-2026-06-02/new-papers/2026-06-03/papers/2606.03544-sage-socialized-evolution.md:30-47`; ledger `mechanism-ledger.md:74`. Exactness label: `interpretive-compression`.

Unresolved: local note is abstract-grounded only (`sage-socialized-evolution.md:45-47`). Do not treat peer history as a parent-selection rule; it is a comparator for exposure policy.

### E1-PRIM-01: reasoning primitive induction

Candidate equation for patch notes only:

```math
\operatorname{AdmitPrimitive}(c)=
\operatorname{SuccessfulTraceCluster}(c)\land
\operatorname{Frequent}(c)\land
\operatorname{Useful}(c)\land
\operatorname{ScopedDocstring}(c)\land
\operatorname{RollbackPath}(c)
```

Source anchors: note `/home/brasides/wiki/queries/research/ploke-arxiv-cs-ai-2026-06-02/new-papers/2026-06-03/papers/2606.02994-reasoning-primitives.md:30-49`; ledger `mechanism-ledger.md:75`. Exactness label: `faithful-draft` for trace-to-primitive admission family; local Ploke additions `ScopedDocstring` and `RollbackPath` are `interpretive-compression` safety wrappers.

Unresolved: local note is abstract-grounded only (`reasoning-primitives.md:47-49`). Need clustering details, thresholds, leakage controls, and demotion criteria before a final equation.

## 4. Candidate wiki patch notes

These are patch notes, not applied edits.

### Patch A: `archive-parent-selection-mechanisms.md`

Target: `/home/brasides/wiki/queries/ploke/selection-scoring/archive-parent-selection-mechanisms.md`.

1. Update source anchors under section 2 to include Appendix E.5:

```markdown
- Appendix E.5 modifiable parent selection and learned selector fragments: lines 3849-3933.
```

2. Replace the exactness sentence near line 39 with:

```markdown
Exactness label: `faithful-formalization` for Appendix A.2 parent-selection equations, Algorithm 1 valid-child admission, named staged gates/zeroing, and the cross-domain average; `interpretive-compression` for the generic staged-gate wrapper and Appendix E.5 learned-selector families because the paper reports representative evolved fragments rather than one stable production selector.
```

3. Insert a new section after current section 5 and before current section 6:

```markdown
## 6. Appendix E.5 modifiable parent selection

Algorithm 5 changes the authority boundary for parent selection. Instead of a fixed handcrafted selector, the most recently admitted archive agent selects parents:

$$
(a_{latest},\_)=\operatorname{last}(A_t),\qquad
P_t=a_{latest}.\operatorname{SelectParents}(A_t)
$$

The rest of the archive update remains the same validity-gated update:

$$
A_{t+1}=\begin{cases}
A_t\cup\{(a',s')\}, & \operatorname{IsValid}(a') \\
A_t, & \text{otherwise}
\end{cases}
$$

Source: `/home/brasides/code/ploke/.agents/hyper-agents.txt:3915-3933`. Exactness: `faithful-formalization` for the pseudocode authority loop.

The paper reports several learned parent-selection families. They are source-visible but should be treated as `interpretive-compression`, not as one canonical selector.

UCB-style score:

$$
\operatorname{ucb\_score}_i = normalized\_score_i + \eta\sqrt{\frac{\log(N+1)}{n_i+1}}
$$

Source: `/home/brasides/code/ploke/.agents/hyper-agents.txt:3868-3879`.

Temperature softmax:

$$
p_i=\frac{\exp(score_i/\tau)}{\sum_j\exp(score_j/\tau)}
$$

Source: `/home/brasides/code/ploke/.agents/hyper-agents.txt:3880-3887`.

Component score and stagnation-triggered exploration:

$$
score_i=(normalized\_score_i + \eta exploration\_bonus_i + diversity\_bonus_i + recency\_bonus_i)elite\_bonus_i
$$

$$
\eta_{t+1}=\begin{cases}
1.4\eta_t, & Var(score)<0.01 \\
\eta_t, & \text{otherwise}
\end{cases}
$$

Source: `/home/brasides/code/ploke/.agents/hyper-agents.txt:3887-3914`.

Caveat: the paper says learned mechanisms did not reliably outperform handcrafted score-child-prop, and diversity/recency/elite component definitions are not fully specified in the text. Keep these as comparator/operator sketches, not Ploke default selector authority.
```

4. Rename current section 6 to `## 7. Similar-category candidates for later additions` and remove the now-stale bullet that says modifiable DGM-H parent selection is only a later candidate.

### Patch B: `mechanism-ledger.md`

Target: `/home/brasides/wiki/queries/ploke/selection-scoring/ledgers/mechanism-ledger.md`.

1. Keep row `dgm-h-modifiable-parent-selection` at line 66, but make the source anchor more precise:

```markdown
/home/brasides/code/ploke/.agents/hyper-agents.txt:3849-3933
```

2. Keep exactness as `interpretive-compression`; append a note that Algorithm 5 authority-loop pseudocode is faithful but the learned selector families are interpretive sketches.

3. For rows 68-75, add `record_status: unresolved-candidate` or an equivalent note until primary PDF deep reads supply formula-level anchors.

### Patch C: candidate nearby-mechanism page

Optional new wiki leaf:

`/home/brasides/wiki/queries/ploke/selection-scoring/archive-admission-nearby-candidates.md`

Suggested frontmatter:

```yaml
---
title: Archive Admission Nearby Candidate Equations
created: 2026-06-14
updated: 2026-06-14
type: query
tags: [ploke, scoring, archive, admission, candidate-equations, research]
sources:
  - queries/ploke/selection-scoring/ledgers/mechanism-ledger.md
  - queries/research/ploke-arxiv-cs-ai-2026-06-02/new-papers/2026-06-03/paper-routing.md
confidence: medium
contested: true
---
```

Initial rows should include the candidate predicates from SkillDAG, DELTAMEM, and Reasoning Primitives, plus unresolved entries for EvoTrainer, StepFinder, Handoff Debt, and SAGE. Every row should include:

```markdown
| mechanism | candidate formula | source anchors | exactness | unresolved before promotion |
```

Do not link it from the root wiki until the page exists and MathJax balance is checked.

## 5. Explicit unresolved list

Keep these unresolved in the equation packet until a deeper source pass repairs them:

| Lead | Source anchors | Why unresolved | Next source action |
| --- | --- | --- | --- |
| EvoTrainer diagnostic branch admission | `2606.03108-evotrainer.md:30-49`; `mechanism-ledger.md:69` | No exact variant status function or backtest threshold visible. | Deep-read PDF sections on diagnostics, variant retention/pruning, and human gates. |
| SkillDAG graph admission | `2606.03056-skilldag.md:30-49`; `mechanism-ledger.md:68` | Predicate is plausible, but local note says exact metric/code verification remains open. | Inspect primary PDF/code for invariant definitions and benchmark retrieval gains. |
| DELTAMEM residual-tree write | `2606.03083-deltamem.md:30-49`; `mechanism-ledger.md:70` | Missing retrieval objective, base/delta thresholds, consolidation policy. | Inspect primary PDF algorithm/threshold sections. |
| Overlaying governance delegation overlay | `mechanism-ledger.md:71` | Belongs to prompt/tool authority, not scoring math yet. | Separate authority-algebra deep read if needed. |
| StepFinder step attribution | `2606.03467-stepfinder.md:30-49`; `mechanism-ledger.md:72` | Abstract-only note; no architecture/score formula. | Deep-read PDF for FailureScore definition and benchmark labels. |
| Handoff Debt rediscovery cost | `2606.02875-handoff-debt.md:30-49`; `mechanism-ledger.md:73` | Abstract-only note; metrics are outcome deltas, not archive admission. | Deep-read structured-note schema and deterministic handoff protocol. |
| SAGE socialized evolution | `2606.03544-sage-socialized-evolution.md:30-47`; `mechanism-ledger.md:74` | Abstract-only note; comparator conditions are not parent-selection policy. | Deep-read arenas, filters, and metric denominators. |
| Reasoning primitive induction | `2606.02994-reasoning-primitives.md:30-49`; `mechanism-ledger.md:75` | Abstract-only note; clustering/frequency/usefulness thresholds absent. | Deep-read induction criteria, leakage controls, and pseudo-tool prompts. |

## 6. Implementation order

1. Patch `archive-parent-selection-mechanisms.md` with Appendix E.5 formulas first. This is the cleanest immediate extension because source text and crate helpers already agree.
2. Update `mechanism-ledger.md` to sharpen the `dgm-h-modifiable-parent-selection` anchor and mark June-03 rows as unresolved candidates unless primary-source deep reads are completed.
3. Create an optional nearby-candidate leaf page only if the wiki owner wants draft candidate equations visible before deep reads.
4. Do not add Rust helpers for June-03 mechanisms yet. The crate already has HyperAgents helpers; the nearby papers need source repair before executable formulas.
5. If Rust eventually changes, add catalog entries only after each candidate has a primary source path, line anchors, exactness label, and tests that preserve authority boundaries.
