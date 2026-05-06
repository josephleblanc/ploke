# Agent 12: HyperAgents Selection And Transfer Notes

Date: 2026-05-06

Scope read:

- `.agents/hyper-agents.txt`
- `research-papers/ploke-child-selection-2026-05-06/hyper-agents.txt`
- `research-papers/ploke-child-selection-2026-05-06/hyper-agents-ploke-notes.md`
- `docs/workflow/evalnomicon/chat-history/on-hyper-agents.md`
- `AGENTS.md` for Ploke authority/naming constraints

The two extracted paper copies are byte-identical in this workspace:
`.agents/hyper-agents.txt` and
`research-papers/ploke-child-selection-2026-05-06/hyper-agents.txt`
both have SHA-256
`6a5bd40db2c0b4d0d2828ed9fb83252145363a4e7243922e5be20fd2516c87dd`.
Line citations below use the `research-papers/.../hyper-agents.txt` copy.

## Summary

HyperAgents' main transferable contribution is archive traversal, not a
winner-take-all child selector. The loop keeps a growing archive, samples
parents probabilistically from all archived agents, lets selected parents
self-modify into children, evaluates those children, and admits valid children
back into the archive. The paper's child handling is therefore admission by
validity plus scoring, not "select the best child and continue."

For Ploke, the clean import is:

```text
candidate Runtime evidence -> archive admission -> archive traversal decision
-> one chosen Runtime becomes Parent through Ploke authority
```

The unsafe import is:

```text
mutable agent code/policy directly decides parent authority or evaluation policy
```

Ploke should take the archive traversal, staged evaluation, descendant-growth
score, and process-memory ideas. It should not import HyperAgents' broad
"edit any code" affordance as ordinary self-improvement authority.

## Parent Selection

The paper's parent selector is defined in Appendix A.2. At each iteration, it
selects parents from the whole archive, not only from the latest or best node:
`At = {a0, a1, ..., at}` and "All agents in the archive are eligible for parent
selection" (`research-papers/ploke-child-selection-2026-05-06/hyper-agents.txt:1274-1279`).

The selector computes a frontier midpoint from the top `m = 3` agents, applies a
sigmoid around that midpoint, multiplies by a child-count penalty, normalizes,
and samples categorically:

```text
alpha_i = performance(a_i)
alpha_mid = average(top 3 alpha)
s_i = sigmoid(lambda * (alpha_i - alpha_mid)), lambda = 10
h_i = 1 / (1 + n_i)
w_i = s_i * h_i
p_i = w_i / sum(w)
parents ~ Categorical(p_i), independently with replacement
```

Evidence: top-3 midpoint and lambda are described at
`hyper-agents.txt:1280-1296`; `h_i = 1 / (1 + n_i)` and `w_i = s_i h_i` at
`hyper-agents.txt:1297-1305`; categorical sampling with replacement at
`hyper-agents.txt:1306-1318`.

Interpretation:

- Performance is softened into a frontier eligibility band; it is not argmax.
- `n_i` is the number of compiled children produced by that agent, so the
  policy downweights already-mined parents without excluding them.
- Sampling with replacement allows a strong parent to produce multiple children
  while preserving stochastic exploration.
- The paper calls this roughly proportional to performance and inversely
  proportional to successfully compiled children (`hyper-agents.txt:1266-1272`).

Ploke transfer:

- Use a fixed, inspectable parent selector first.
- Map `alpha_i` to an evidence-backed evaluation score, not a raw mutable
  scheduler field.
- Map `n_i` to admitted valid children, not attempted children and not failed
  staged-eval probes.
- Make the selector a traversal recommendation over admitted Runtime/Artifact
  candidates; the selector itself should not be the authority transition that
  grants Parent-ness.

## Child Selection And Admission

The paper does not define a separate child-selection rule inside the main loop.
Algorithm 1 says:

```text
P <- SelectParents(A)
for each parent:
  a' <- a.Modify(a, A)
  s' <- Evaluate(a', T)
  if IsValid(a') then
    A <- A union {(a', s')}
return A
```

Evidence: `hyper-agents.txt:1332-1344`. Section 3 says selected parents
generate modified versions, each modified hyperagent is evaluated, and then
added to the archive (`hyper-agents.txt:281-286`).

There is selection of best discovered agents for final test evaluation, but
that is not the loop's successor policy. Figure 2 text says "best discovered
task agents" are selected by validation or training scores for test evaluation
(`hyper-agents.txt:451-454`), and the domain section says only best agents are
evaluated on the test set (`hyper-agents.txt:462-463`).

Staged evaluation is also child gating. The paper first evaluates on a small
training subset; agents that fail do not get full evaluation and receive zero
for unevaluated tasks (`hyper-agents.txt:338-348`). In joint paper-review and
robotics runs, staged failure in either domain blocks full training evaluation
for both and assigns zero for remaining tasks (`hyper-agents.txt:456-463`).

Ploke transfer:

- Keep "child generation", "evaluation", "archive admission", and "successor /
  Parent selection" as distinct records.
- A child may be valid and admitted without becoming the next Parent.
- A failed staged evaluation should remain evidence with explicit zeroed
  dimensions, not a silent non-record.
- Do not record a "successful child" counter for invalid, non-built, or
  non-admitted candidates.

## Archive Traversal

The archive is the actual search object. Section 3 says DGM-H maintains an
archive initialized with one hyperagent and expanded by accumulating generated
variants (`hyper-agents.txt:276-280`). The no-open-ended-exploration baseline
removes that archive and instead makes each newly generated hyperagent replace
its predecessor and automatically become the next selected parent
(`hyper-agents.txt:321-327`). That baseline exists precisely to isolate the
value of archive traversal.

The local chat-history note captures the Ploke implication directly:
"HyperAgents' traversal is not 'pick the best child and continue.' It is archive
traversal" (`docs/workflow/evalnomicon/chat-history/on-hyper-agents.md:1`).
It also states the key architectural split: "successor selection != archive
traversal" (`on-hyper-agents.md:32-36`), and lists ordinary traversal moves:
continue from accepted child, explore from rejected child, backtrack to prior
admitted runtime, branch from sibling/ancestor, sample from archive, or stop
(`on-hyper-agents.md:38-47`).

Ploke transfer:

- Treat backtracking as normal archive traversal, not only recovery.
- Keep a traversal layer above local successor choice.
- The Crown path can still install exactly one next Parent, but the candidate
  should eventually be chosen from the admitted archive, not only from direct
  children of the current Parent (`on-hyper-agents.md:71`).

## Transfer And Growth Scoring

Appendix D.4 selects transfer agents using descendant growth, not current score
alone. The criterion favors nodes that are strong stepping stones rather than
"merely high-scoring themselves" (`hyper-agents.txt:2241-2243`).

The growth score is:

```text
G_gamma(i) = (1 / |D(i)|) * sum_{j in D(i)} (alpha_j - alpha_i) * gamma^dist(i,j)
```

where `D(i)` is the set of descendants and `dist(i, j)` is lineage distance.
The paper sets `gamma = 0.6` and only considers nodes with at least three
descendants (`hyper-agents.txt:2247-2266`).

Interpretation:

- A node is valuable if its descendants improve reliably and quickly.
- Parent value differs from immediate task score.
- Lineage structure is required to compute the score.

Ploke transfer:

- Store enough lineage to compute descendant-growth later.
- Use growth scoring as a retrospective archive-analysis metric before using it
  as a live parent selector.
- Prefer a candidate Runtime whose admitted descendants improve over a candidate
  with one high immediate score but poor downstream yield.
- Keep `G_gamma` an evidence projection. It can recommend a Parent coordinate;
  it should not itself confer Parent authority.

## Policy Enactment And Self-Modification Affordances

HyperAgents merges task agent and meta agent into one editable program. The
paper defines a hyperagent as a single editable program containing task and meta
agent, where the meta agent can rewrite itself (`hyper-agents.txt:262-267`).
Metacognitive self-modification means modifying the procedures that propose and
apply later improvements (`hyper-agents.txt:268-275`). The initial agent is
deliberately simple; the important affordance is an outer loop with bash/file
editing, previous evaluation results, remaining iteration budget, and permission
to modify any part of the codebase (`hyper-agents.txt:297-307`; searchable
phrase in Appendix E.3: "modify any part of the codebase" at
`hyper-agents.txt:3400-3404`).

The paper keeps outer-loop policy fixed in the main experiments. It explicitly
notes that parent selection and evaluation remain fixed, which improves
stability and safety while limiting full self-modifiability
(`hyper-agents.txt:821-826`). Appendix E.5 explores modifiable parent
selection. It improves over random but does not beat the handcrafted
score-child-prop selector (`hyper-agents.txt:3849-3867`). The learned selectors
rediscover UCB-style exploration, softmax temperature sampling, adaptive
temperature, diversity/recency/elite bonuses, and stagnation detection
(`hyper-agents.txt:3868-3914`).

Ploke transfer:

- Make prompt/rubric/process surfaces editable first.
- Expose selection metrics, archive summaries, staged-eval summaries, and
  descendant-growth analyses to agents as evidence.
- Let agents propose selection-policy or evaluation-policy changes as artifacts
  requiring stronger admission, not as live authority mutations.
- Keep parent selection handcrafted until the evaluation evidence is mature; the
  local notes make the same recommendation at
  `research-papers/ploke-child-selection-2026-05-06/hyper-agents-ploke-notes.md:166-187`.

## Process Improvements Worth Importing

The useful self-modifications are mostly process infrastructure:

- Evaluation analysis before edits: the meta agent evolves from blind
  modification to analysis of prior eval results (`hyper-agents.txt:3400-3445`).
- Compute-aware planning: behavior changes based on remaining iterations,
  with early exploration and late conservative refinement
  (`hyper-agents.txt:3447-3452`).
- Performance tracking: persistent metrics across generations, improvement
  trends, and statistics (`hyper-agents.txt:3531-3586`).
- Prompt templates: reusable prompt infrastructure
  (`hyper-agents.txt:3588-3595`).
- Multi-generation refinement: summaries of what worked, regressions, root
  causes, and concrete corrections (`hyper-agents.txt:3664-3704`).
- Persistent memory: timestamped storage, retrieval, and synthesized lessons
  across generations (`hyper-agents.txt:3706-3778`).

Ploke transfer:

- These map well to mutable non-authority surfaces: target-selection rubrics,
  child-generation rubrics, validation checklists, failure taxonomies,
  evaluation summary generation, process memory, and performance summaries.
- They do not require making `ploke-eval` authority code mutable in the ordinary
  loop.

## Authority And Provenance Conflicts

HyperAgents' broad edit model conflicts with Ploke's Prototype 1 authority
model if imported literally.

Ploke constraints from `AGENTS.md`:

- "Every checkout is an Artifact" (`AGENTS.md:23`).
- "Every Artifact is a dehydrated Runtime" (`AGENTS.md:24`).
- "Every Runtime is a potential Parent" (`AGENTS.md:25`).
- "Parent-ness is a role/state, not a backend operation" (`AGENTS.md:26`).
- Parent/child state should be structural, e.g. `Child<Ready>`, not flattened
  event names (`AGENTS.md:27`).
- Typed transitions should have private fields, state markers, move-only
  transition methods, and durable records from allowed transitions, not public
  status writes (`AGENTS.md:10-12`).
- History/Crown claims must not overpromise tamper evidence, Crown authority, or
  compiler-enforced transition validity (`AGENTS.md:32-35`).

Conflicts:

- HyperAgents says a hyperagent can edit any code and can in principle modify
  parent selection and evaluation. In Ploke, ordinary self-improvement must not
  mutate the authority surface that grants Parent-ness or seals provenance.
- HyperAgents treats "compiled child" / `IsValid(a')` as the archive-admission
  gate. Ploke needs a richer admission record: Artifact identity, Runtime
  provenance, evaluation evidence, policy reference, and authority transition.
- HyperAgents' empirical score can guide traversal, but Ploke cannot let a
  mutable score file or scheduler projection become lineage authority.
- HyperAgents' transfer score depends on archive tree lineage. Ploke must record
  lineage as provenance-bearing Artifact/Runtime relations, not reconstruct it
  from rendered logs.
- HyperAgents' fixed benchmarks raise Goodhart risk. Appendix F says evaluation
  gaming can exploit blind spots and safety depends on evaluation-signal
  fidelity (`hyper-agents.txt:3968-3977`). Ploke should therefore preserve
  held-out evals, diverse evidence, and human-readable projections as checks on
  selection pressure.

## Directly Transferable To Ploke

- Archive-wide parent selection over all admitted candidates.
- Probabilistic score-child-count traversal with a soft frontier midpoint.
- Child-count penalty based on admitted valid children.
- Sampling with replacement once archive size supports it.
- Staged evaluation with explicit zeroed dimensions for failed gates.
- Archive admission separate from successor/Parent selection.
- Descendant-growth scoring for retrospective transfer and stepping-stone
  analysis.
- Backtracking/branching from sibling or ancestor as ordinary traversal.
- Persistent memory, multi-generation performance summaries, evaluation
  analysis, prompt/rubric templates, and compute-aware planning.

## Not Directly Transferable

- "Modify any part of the codebase" as an ordinary child affordance.
- Self-modification of parent selection or evaluation policy without a separate
  protocol-upgrade/fork/admission path.
- Treating compile success as sufficient archive authority.
- Treating benchmark score as the authority to become Parent.
- Reconstructing lineage, child counts, or growth from mutable projections or
  rendered logs.

## Recommended Ploke Mapping

Use the HyperAgents loop as a conceptual adapter:

```text
Artifact<ChildCandidate>
  -> EvaluationEvidence
  -> ArchiveAdmission
  -> ArchiveNode<Runtime>
  -> ParentSelectionRecommendation
  -> Crown/History-governed Parent transition
```

The report-level invariant is:

```text
archive traversal recommends; Ploke authority admits and installs.
```

This preserves HyperAgents' open-ended traversal benefit while keeping Ploke's
role/state authority and provenance model intact.
