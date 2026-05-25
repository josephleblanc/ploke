# HyperAgents Gap Review

Date: 2026-05-17

Purpose: capture the second-pass comparison between `.agents/hyper-agents.txt`
and the current Prototype 1 loop after re-reading the paper, the current loop
docs, History/Crown docs, broad-harness notes, and selection/evaluation record
surfaces.

## Correction From The First Review

The earlier review under-read existing Prototype 1 design material. Prototype 1
does not lack the HyperAgents architecture in the broad sense. It already has
the main authority/provenance shape:

- Artifact/Runtime/Parent/Child/Successor roles.
- History/Crown as the intended authority substrate.
- Broad-harness request/admission boundary.
- History-backed score-child-prop traversal.
- Typed evaluation and selection records with evaluator/eval-set/policy identity
  hooks.

The remaining gaps are narrower:

- benchmark-family and task-set generalization;
- split-aware evaluation policy;
- first-class improvement-generation metrics;
- transfer/stepping-stone projections;
- explicit protocol-upgrade and consensus/finality mechanics for deeper
  self-modification.

## Held-Out Final Test Means

In HyperAgents, held-out final test does not mean the agent gets no evaluation
feedback. During self-modification, agents may receive training feedback, and
where a validation split exists that validation score is used for parent
selection. The test split is withheld from parent selection and ordinary
self-modification feedback, then used only after the run to report whether the
selected agents generalize.

Paper anchors:

- `.agents/hyper-agents.txt:341-348`: parent-selection performance comes from
  validation when available, otherwise training; held-out test tasks are used
  only for final evaluation.
- `.agents/hyper-agents.txt:1700-1703`: Table 2 repeats this split rule.
- `.agents/hyper-agents.txt:1718-1722`: Polyglot uses 60 training tasks with a
  staged 10-then-50 protocol, then evaluates generated agents on 165 unseen
  tasks.
- `.agents/hyper-agents.txt:456-464`: in multi-domain runs, only the best agents
  selected via validation or training scores are evaluated on the test set.

Current Prototype 1 can run two targets, but those targets are one effective
selection/evaluation cohort. `target.instance` and `target.instances` are not
typed as train, validation, probe, or test roles. `target.eval_instances()`
returns the configured cohort, and selection consumes the resulting compared
instances as one set.

This is not counter to HyperAgents. It is a missing role distinction: Prototype
1 can already run an operational smoke/evolution cohort, but paper-quality
claims need an additional final evaluation over targets that did not influence
parent selection, child generation, or prompt/evidence summaries during the run.

## Current Code Difference: One Cohort, Not Split Roles

The current Prototype 1 profile has target selection, but not evaluation-role
selection. The profile target shape is:

```text
target.dataset_key
target.instance
target.instances
```

and `Target::eval_instances()` returns one effective list. That list is then
used as the set of instances to prepare, run, compare, and feed into selection.
This is correct for the current two-target protocol smoke run, but it does not
encode the paper's split semantics.

Concrete differences from HyperAgents:

- HyperAgents has task roles: staged training/probe tasks, training tasks,
  validation tasks when available, and held-out test tasks.
- Prototype 1 currently has target instances, not target roles. A configured
  target can participate in baseline/treatment comparison and successor
  selection; there is no `final_only` target class.
- HyperAgents may use validation for parent selection and then keep test for
  final reporting. Prototype 1 selection consumes the compared instances from
  the branch evaluation report as one set.
- HyperAgents has staged evaluation inside domains, such as "try 10 tasks, then
  expand to the remaining training tasks if the agent clears the gate."
  Prototype 1 has execution stop points and target lists, but not a typed
  staged-evaluation policy.
- HyperAgents final scores can be described as generalization evidence because
  selected agents are evaluated on tasks not used for selection. Prototype 1's
  current two-target result should be described as operational/evolution
  evidence over the configured cohort, not as held-out generalization evidence.

The honest assessment is that this is not a flaw for the current run. The
current run is doing useful systems work: can the loop run, can protocol data be
recorded, can two targets be compared, can history-backed selection operate?
It is just not yet an experimental protocol that can support the same kind of
paper claim.

## Split-Aware Evaluation Roles

The missing semantic object is an evaluation plan with role-labeled task sets:

```text
TaskSet<ProbeTrain>         quick gate / cost control
TaskSet<Train>              ordinary improvement feedback
TaskSet<ValidationSelect>   parent-selection signal when available
TaskSet<HeldOutFinal>       final reporting only
```

Useful first implementation shape:

- Keep the current `target.instances` path as the default "selection cohort" for
  compatibility.
- Add a separate split-aware profile or eval-plan section rather than
  overloading `instances` with conventions.
- Persist the split role in eval-set identity and selection input records.
- Make successor selection declare which split role it is allowed to read.
- Keep final-only tasks out of broad-harness prompt evidence, improvement
  memory, and parent-selection projections during evolution.
- Add a distinct final-report command or phase that evaluates selected/archive
  candidates on held-out tasks after the evolution run.

The line to preserve is:

```text
selection evidence may be inspectable during evolution
final-test evidence is produced after selection and cannot affect the selected
parents it is reporting on
```

That is the part that makes "held out" meaningful.

## Improvement Memory Is Not Counter To HyperAgents

HyperAgents explicitly gives the meta agent previous evaluation results and lets
it inspect evaluations across tasks during self-modification. For Prototype 1,
the compatible object is an eval-owned evidence digest or improvement-memory
projection that children may inspect.

Paper anchors:

- `.agents/hyper-agents.txt:297-305`: the initial meta agent receives the
  agent repository, previous evaluation results, and remaining budget.
- `.agents/hyper-agents.txt:1353-1358`: in multi-domain optimization, agents
  are evaluated across domains and can inspect evaluations from any task.

The important boundary is authority:

```text
children may inspect evidence summaries and use them to plan edits
children may not self-authorize admission, selection, or policy unlocks by
writing a more persuasive summary
```

Existing docs already point in this direction:

- `docs/active/agents/2026-05-15_hyperagents-context-building-handoff.md`
  says to expose evidence and budget, let the harness choose the intervention,
  and avoid automatic source-snippet injection as the initial context.
- `docs/active/agents/2026-05-12_tui-adapter-boundary-review/prompt-evidence-oracles.md`
  lists the missing typed latest-evidence digest: latest evaluation paths,
  current metric/oracle fields, rejected attempts, protocol artifact roots,
  validation commands, and generation/node budget.

So the gap is implementation and carrier shape, not conceptual agreement with
HyperAgents.

## Policy-Surface Self-Modification

HyperAgents defines a hyperagent as one editable program whose task agent and
meta agent can rewrite themselves. The paper also notes that the main
experiments keep the handcrafted parent-selection mechanism fixed, while
Appendix E.5 explores modifiable parent selection and finds it improves over
random but does not outperform handcrafted score-child-prop.

Paper anchors:

- `.agents/hyper-agents.txt:257-267`: task agent and meta agent are one editable
  hyperagent program.
- `.agents/hyper-agents.txt:276-290`: main experiments use a handcrafted
  parent-selection mechanism to isolate hyperagent self-modification.
- `.agents/hyper-agents.txt:3849-3867`: modifiable parent selection improves
  over random but not over handcrafted score-child-prop in the reported runs.

Prototype 1 is intentionally different at the authority boundary. Current
ordinary descendants preserve the `ploke-eval` policy-bearing surface digest.
Changing parent creation, admission, History/Crown, selection, handoff, or other
authority code is a protocol-upgrade/fork problem, not an ordinary child edit.

Existing History docs already capture the right direction:

- `crates/ploke-eval/src/cli/prototype1_state/history.rs` treats
  digest-preserving ordinary succession as the current invariant and explicitly
  describes policy-bearing changes as protocol-upgrade/fork candidates.
- The same module states the current implementation is local and narrow:
  tamper-evident, lineage-scoped, and transition-checked, but not distributed
  consensus, process uniqueness, or global finality.
- `.agents/prototype1-hyperagents-handoff-2026-05-06.md` describes deeper
  mutable surfaces as ledger-backed unlocks, not config toggles.

The corrected review should not say "Prototype 1 lacks a safety plan." It should
say: Prototype 1 has a local digest/History gate and a documented unlock model,
but it does not yet have consensus/finality enforcement for multi-ruler or
policy-surface self-modification.

## Privileged Self-Mutation: Candidate Options

This section is not the chosen implementation direction. It records candidate
mechanisms worth revisiting as the design matures.

The durable design intent is broader than "protect `ploke-eval` forever": the
framework should eventually allow further self-mutation, including more
privileged meta-learning and authority-adjacent areas, once the system can
justify that expansion. The exact mechanism for doing that is still open.

One possible family of mechanisms is to keep sandbox, admission, and unlock
rules inside the preserved authority surface while the system earns authority to
edit progressively more privileged surfaces. Candidate ingredients to evaluate
later include:

- execution containment, such as denying ordinary shell access where the
  benchmark/harness does not require it;
- manifest containment, such as preserving `Cargo.toml` or requiring a stronger
  review path before dependency or feature changes;
- static analysis gates over the code graph, including prohibited imports,
  prohibited functions, unsafe capability use, or authority-surface writes;
- typed evidence requirements for proposed changes to tool schemas, harness
  behavior, evaluator material, selection policy, and eventually History/Crown
  protocol code;
- distributed membership and finality rules for when the network may admit a
  riskier surface unlock.

The useful constraint, regardless of implementation, is that privileged mutation
should not collapse to a single Boolean such as:

```text
allow_edit_ploke_eval = true
```

One possible sketch is a sequence of admitted unlock transitions:

```text
Surface 0: prompts, rubrics, memory summaries
Surface 1: tool descriptions and action contracts
Surface 2: harness behavior and context construction
Surface 3: evaluator/adjudicator material
Surface 4: selection/admission/meta-learning policy code
Surface 5: History/Crown/channel protocol code
```

For each deeper surface, the unlock predicate would need to become stronger.
One possible future rule to revisit is:

```text
MayMutate(surface_k, runtime_R)
  iff
    R is admitted under the current History/Crown contract
    surface_k is named in the mutable-surface contract
    the candidate passes sandbox/static-analysis gates for surface_k
    enough independent rulers have reached membership/finality for this regime
    consensus admits the unlock or protocol-upgrade transition
```

That is materially different from HyperAgents' paper experiments. The paper
allows the hyperagent to edit its own program, while the experimental outer loop
keeps some mechanisms fixed for measurement. Prototype 1's durable design
pressure is to make sandboxing and authority expansion part of the framework
itself, so self-mutation of meta-learning areas can become possible later
without making the current parent the sole judge of its own authority expansion.

Current gap: History/Crown is local and single-ruler today. It can help bind
lineage facts and ordinary digest-preserving succession, but it is not yet the
multi-ruler consensus substrate that would make high-privilege unlocks safe.

## HyperAgents Metrics Not First-Class Yet

Two paper metrics/projections remain missing as first-class Prototype 1 objects.

### `imp@k`

HyperAgents defines improvement@k as the maximum performance improvement a fixed
meta agent can produce within `k` generated task agents under a fixed generation
algorithm and evaluation set. This measures the self-improvement generator, not
only the best current task performer.

Paper anchor: `.agents/hyper-agents.txt:2213-2238`.

Prototype 1 has branch comparisons and selector inputs, but no projection that
asks:

```text
given generator/runtime G and budget k,
how much better was the best descendant than the starting artifact/runtime
under evaluator E on eval set S?
```

This should be a projection over admitted archive/evaluation records with
explicit evaluator, eval-set, budget, and generation-procedure identity. It
should not be encoded as another naked score on `SuccessorDecision`.

### Descendant-Growth Transfer Score

HyperAgents also selects transfer agents using a discounted descendant-growth
score:

```text
G_gamma(i) = average over descendants j of (alpha_j - alpha_i) * gamma^dist(i,j)
```

The point is to identify stepping-stone agents that reliably generate better
descendants, not merely agents that have the highest immediate score.

Paper anchor: `.agents/hyper-agents.txt:2240-2266`.

Prototype 1 now has history traversal and child-count pressure in
score-child-prop, but it does not yet persist or project a lineage-level
"descendant growth" signal. The natural fit is a History/archive projection that
joins:

- node/artifact/runtime ancestry;
- admitted child count;
- accepted and rejected child outcomes;
- evaluation scores with evaluator/eval-set identity;
- distance from candidate ancestor to descendant;
- discount parameter and selection policy identity.

This projection may later feed selection or transfer experiments, but its first
form should be inspectable evidence, not active authority.

## Benchmark Generalization Gap

HyperAgents evaluates coding, paper review, robotics reward design, and
Olympiad-level math grading. Each domain has a task input, required output,
metric, split policy, staged evaluation rule, and domain-specific evaluator.

Prototype 1 is still Multi-SWE-shaped in live code:

- `crates/ploke-eval/src/target_registry.rs` has a single `BenchmarkFamily`:
  `MultiSweBenchRust`.
- `crates/ploke-records/src/evaluation.rs` duplicates the same single-family
  enum in passive records.
- `crates/ploke-eval/src/registry.rs` has one builtin dataset key, `ripgrep`.
- `crates/ploke-eval/src/spec.rs` has `RunSource::MultiSweBench` as the only
  run-source variant.
- `crates/ploke-records/src/evaluation.rs` has
  `BenchmarkPatchProjectionRecord -> MultiSweBenchTarget`.
- `crates/ploke-eval/src/runner.rs` writes
  `MultiSweBenchSubmissionRecord` and requires `RunSource::MultiSweBench` for
  benchmark patch projection.

The useful existing hooks are:

- `Evaluator`;
- `EvalSet`;
- `EvalPolicy`;
- compared instances;
- selector domain findings;
- operational/protocol/oracle/adjudication vocabulary in passive records.

The missing object is not a bigger `instances` array. It is a benchmark family
adapter shape that preserves:

```text
BenchmarkFamily
TaskSet
TaskInput
TaskOutput
SubmissionArtifact
EvaluatorProcedure
MetricDomain
SplitPolicy
StagedEvaluationPolicy
HeldOutPolicy
CostBudget
```

The next design pass should survey several benchmark families before writing
the Rust shape. Use HyperAgents' four domains as the first survey set because
they force different input/output/evaluator forms:

- coding patch benchmark: repo + instruction -> patch -> test/pass metric;
- paper review: paper text -> accept/reject -> classifier accuracy against human labels;
- robotics reward design: task description -> reward function -> simulator task score;
- IMO grading: problem + solution -> grade label -> accuracy against expert grade.

That survey should classify each benchmark by task substrate, output artifact,
evaluator substrate, feedback visibility, split policy, staged-eval policy, and
whether the evaluator is mechanized, model-judged, human-labeled, or hybrid.

## Potential Follow-Up Slices To Revisit

These are candidate slices, not a committed roadmap.

1. Add a design-only benchmark survey matrix before changing enums.
2. Explore split roles in the run/eval model: probe/train, validation-selection,
   and held-out-final.
3. Explore a typed evidence digest/improvement-memory projection for child
   planning, explicitly marked as projection/evidence rather than authority.
4. Explore archive projections for `imp@k` and descendant-growth transfer score.
5. Revisit privileged mutation only through an explicit protocol-upgrade,
   unlock, or fork transition rather than ordinary child edits.

## Possible Resolution Paths

The paths below are implementation options. They should be reviewed against the
current code and experiment goals before any one is treated as project
direction.

### Split-Aware Evaluation

A plausible low-risk path would start with records and profile semantics, not
runner behavior:

1. Define split roles in `ploke-records` eval-set identity.
2. Teach `ploke-eval` run profiles to name role-labeled task sets.
3. Preserve compatibility by treating old `target.instances` as the
   selection/evolution cohort.
4. Make selection strategies state which role they score from.
5. Add a final-report path that evaluates selected archive nodes on
   `HeldOutFinal` tasks and records the result without changing the selected
   parent.

### Benchmark Families

One plausible path is to do a survey pass before touching enums. The survey
could compare at least the four HyperAgents domains across:

- input substrate;
- output artifact;
- evaluator procedure;
- metric domain;
- feedback visibility;
- split policy;
- staged-eval policy;
- cost/budget model;
- whether ground truth is mechanized, human-labeled, model-judged, or hybrid.

After that, one possible implementation would make Multi-SWE one adapter rather
than the framework shape.

### Improvement Metrics

One possible path is to implement `imp@k` and descendant-growth as archive
projections first. They would read admitted History/evaluation records and
produce inspectable evidence with evaluator/eval-set/procedure identity. Only
after the projection is stable should it be considered as an active selection
input.

### Privileged Unlocks

One possible conservative path:

1. Define surface levels and what each level may read/write.
2. Encode simple static gates for the low levels.
3. Add a protocol-upgrade record shape that can describe a proposed deeper
   surface mutation.
4. Keep single-ruler local History as "not enough" for deep unlocks.
5. Later, add multi-ruler membership, quorum/finality evidence, and consensus
   admission before allowing mutation of selection/admission/History code.

This preserves the HyperAgents ambition, but makes the safety boundary part of
the self-improvement framework rather than an external trust assumption. This is
an option to revisit, not a settled design.

## Reduction To Refuse

Do not reduce these gaps to "add more target instances" or "let children edit
`ploke-eval`." The richer object is a benchmark- and split-aware archive
experiment over admitted Artifacts/Runtimes, with typed evidence projections and
explicit authority transitions for deeper self-modification.
