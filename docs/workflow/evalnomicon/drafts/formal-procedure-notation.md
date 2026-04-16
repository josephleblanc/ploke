# Formal Procedure Notation Draft

Draft for architecture on mixed adjudication and programmatic composing procedures.

An eval may have metrics which are derived programmatically or by LLM adjudication. These metrics may be said to be derived by a procedure x which operates on an input of admissible evidence carried by state s with an executor e such that: 
Exec(e, x, s) = s'
where the executor e may be either a programmatic or an LLM adjudication.
The state s may be modeled with typed state transitions which may branch or merge and are composable into arbitrary direct acyclical graphs.

## 1. Universes And Basic Sorts

```text
M      = set of named metrics
M_op   = set of operationalized metrics

O, N, C = obvious, non-obvious, conceptual metrics
D       = uncaptured dimensions

X      = set of procedure specifications
E      = set of executors
S      = set of typed procedure states
V      = set of values
P(A)   = power set of A, i.e. the set of all subsets of A
A × B  = Cartesian product of A and B, i.e. the set of ordered pairs (a, b)
```

```text
M = O ∪ N ∪ C
M_op = O ∪ N

O ∩ N = ∅
O ∩ C = ∅
N ∩ C = ∅
D ∩ M = ∅
```

```text
D -> C -> N -> O
```

Interpretation:

- `D -> C -> N -> O` is a common maturation path, not a law.
- `N` and `O` are operational classes.
- `C` is conceptually specified but not yet satisfactorily operationalized.

## 2. Procedure, Executor, And State

For a procedure specification `x ∈ X`:

```text
I_x ⊆ S   = admissible input states for x
O_x ⊆ S   = admissible output states for x
```

For an executor `e ∈ E`:

```text
Exec(e, x, s) = s'   where s ∈ I_x and s' ∈ O_x
```

Interpretation:

- `x` is a specification, not an execution.
- `e` is the thing that carries out `x`.
- both input and output are modeled as typed states, not bare values.

## 3. States, Values, And Metrics

Each state may carry one or more typed values:

```text
val : S -> P(V)
```

Some states are metric-bearing:

```text
metric : S -> P(M × V)
```

Interpretation:

- not every state is itself a final metric state
- a state may carry intermediate values, evidential judgments, supporting
  metrics, or a target metric

## 4. Target Metrics And Evidential Outputs

For a procedure `x`, define:

```text
T_x ⊆ M    = target metrics of x
U_x ⊆ S    = evidential output states of x
K_x ⊆ S    = supporting-metric states of x
```

Intended distinctions:

```text
t ∈ T_x   => t is a terminal metric the procedure is meant to produce
u ∈ U_x   => u is instrumentally relevant to some downstream target metric
k ∈ K_x   => k carries a supporting metric worth recording in its own right
```

A supporting-metric state may also be evidential:

```text
K_x ∩ U_x may be non-empty
```

Interpretation:

- intermediate outputs need not be final target metrics
- their relevance is mediated by composition
- some intermediate outputs are worth both recording and forwarding

## 5. Recording And Forwarding

For a produced state `s ∈ S`:

```text
Rec(s)   := s is recorded as an artifact
Fwd(s)   := s is forwarded as admissible input to a downstream step
```

Possible cases:

```text
Rec(s) ∧ ¬Fwd(s)   = record only
¬Rec(s) ∧ Fwd(s)   = forward only
Rec(s) ∧ Fwd(s)    = record and forward
```

Interpretation:

- this separates persistence concerns from compositional concerns
- many useful intermediate judgments should be both recorded and forwarded

## 6. Sequential Composition

Let:

```text
x = (x_1, x_2, ..., x_n)
```

with:

```text
I_i ⊆ S
O_i ⊆ S
```

Sequential composition is admissible when:

```text
O_i ⊆ I_(i+1)
```

or more generally:

```text
proj(O_i) ⊆ I_(i+1)
```

when the downstream step consumes only a typed projection of the prior state.

Interpretation:

- the composed procedure remains state-based
- later steps may consume the full prior state or a typed projection of it

## 7. Fork And Merge

Forks and merges are first-class procedure operations.

### Fork

```text
Fork : S -> S_1 × S_2 × ... × S_n
```

Each branch state is a typed projection, copy, or refinement of a common prior
state.

### Merge

```text
Merge : S_1 × S_2 × ... × S_n -> S_m
```

Default merge discipline:

```text
prov(s_i) is preserved in s_m unless the procedure explicitly discards it
```

Interpretation:

- branches may support independent judgments over the same base evidence
- merge states may synthesize multiple prior branch outputs into one later state
- merge is not assumed to be additive, commutative, or branch-erasing
- by default, a merge state should preserve branch provenance rather than
  collapse distinct prior outputs into one indistinguishable blob
- this is a better model than treating every procedure as a single linear chain

One useful intuition is a branch-labeled structured state:

```text
s_m = {
  branch_1 : payload_1,
  branch_2 : payload_2,
  synthesis : payload_syn
}
```

This notation is informal record notation, not set notation.

It means:

- `s_m` carries a field named `branch_1` containing the first branch output
- `s_m` carries a field named `branch_2` containing the second branch output
- `s_m` may also carry a synthesized field derived from both branches

The important point is not this exact notation. The point is that merge should
usually preserve branch identity in the resulting state.

## 8. Procedure Graphs

A procedure need not be only a sequence. More generally:

```text
x = (G_x, in_x, out_x)
```

where:

```text
G_x = a directed acyclic graph of typed state transitions
in_x ⊆ S = admissible source states
out_x ⊆ S = admissible terminal states
```

Nodes may be treated as states and edges as typed transitions, or vice versa,
so long as the typing discipline is preserved consistently within one formalism.

Interpretation:

- the important invariant is typed admissibility
- the framework should support linear, forked, and merged procedures

## 9. Step-Local Executors

For a composed procedure:

```text
x = (x_1, ..., x_n)
e = (e_1, ..., e_n)
```

with:

```text
Exec(e_i, x_i, s_i) = s_(i+1)
```

Interpretation:

- executor identity is step-local, not necessarily procedure-global
- mixed mechanized, adjudicative, and later human steps fit naturally here

## 10. Reliability Properties

For a procedure specification `x`:

```text
B(x)    := x is bounded in time, scope, and effort
Ex(x)   := x is explicit enough to be written down and followed
Adm(x)  := x specifies admissible input states
```

For an executor applying a procedure:

```text
Rep(e, x) := e produces acceptably consistent outputs on the same admissible inputs
Rel(e, x) := e is reliable enough for routine use on x
Cal(e, x) := e is acceptably calibrated against a stronger reference for x
```

Operational reliability:

```text
R(e, x) := B(x) ∧ Ex(x) ∧ Adm(x) ∧ Rep(e, x) ∧ Rel(e, x)
```

Interpretation:

- `Cal(e, x)` remains separable from routine operational reliability for now
- calibration may later be required for particular procedure classes

## 11. Metric Class Membership

Let `m ∈ M`.

Define:

```text
X_prog ⊆ X   = programmatic procedures
X_adj  ⊆ X   = adjudicative procedures

Op : X -> P(M)
```

where `Op(x)` is the set of metrics operationalized by procedure `x`.

```text
m ∈ O  iff  ∃ x ∈ X_prog, ∃ e ∈ E such that
             m ∈ Op(x)
             ∧ R(e, x)

m ∈ N  iff  ¬∃ x ∈ X_prog, ∃ e ∈ E such that
               m ∈ Op(x)
               ∧ R(e, x)
           ∧ ∃ x' ∈ X_adj, ∃ e' ∈ E such that
               m ∈ Op(x')
               ∧ R(e', x')

m ∈ C  iff  m is conceptually specified
           ∧ m ∉ O
           ∧ m ∉ N
```

Interpretation:

- adjudicated procedures may produce `N` metrics even when some internal steps
  are mechanized
- a NOM procedure may therefore contain many OM-like substeps

## 12. Procedure Scale

Procedure scale is not itself a metric class distinction, but it is useful for
design.

Let:

```text
micro(x) := x operates over a highly localized unit such as one call
local(x) := x operates over a bounded trace window or turn
macro(x) := x operates over a whole run or cross-run aggregate
```

Interpretation:

- these are scale descriptors, not validity judgments
- many currently useful adjudicated procedures are likely `local`, not `micro`

## 13. Minimal Worked Shape

Suppose `x` is a local trace-analysis procedure.

```text
s_0 ∈ I_x                        initial trace subject
s_1 = Exec(e_1, x_1, s_0)        mechanized extraction
s_2a, s_2b = Fork(s_1)           branch into two evidential paths
s_3a = Exec(e_2a, x_2a, s_2a)    adjudicated branch
s_3b = Exec(e_2b, x_2b, s_2b)    mechanized branch
s_4 = Merge(s_3a, s_3b)          synthesis state
s_5 = Exec(e_3, x_3, s_4)        target metric state
```

Possible role assignment:

```text
s_3a ∈ U_x
s_3b ∈ U_x ∩ K_x
s_5 carries t where t ∈ T_x
```

This is the intended general shape for:

- independent concurrent judgments on shared evidence
- later synthesis into one final aggregate or interpretation
- selective recording and forwarding of intermediate outputs

## 14. Why This Draft Exists

This draft is trying to keep four distinctions explicit:

1. metric vs procedure vs executor
2. target metric vs evidential output vs supporting metric
3. sequence vs fork/merge graph
4. recording vs forwarding

If a proposed concept cannot be placed cleanly into one of these relations, it
is a sign that the concept is still too fuzzy and needs to be pressed further.

## 15. Implementation Note

This notation does not imply that the full global procedure graph should be
encoded as one giant compile-time type-state object.

A more plausible implementation strategy is:

- keep step-local input/output boundaries strongly typed
- keep step-local executors strongly typed
- represent the larger procedure as a runtime graph with stable node ids,
  branch structure, and execution artifacts

Interpretation:

- the typing discipline is local and structural
- the execution graph is global and dynamic enough to support forks, merges, and
  larger procedures without exploding the type surface

## 16. Bounded Inquiry Procedures

Not every useful procedure is a closed one-shot transformation.

Some useful procedures are better modeled as bounded inquiries with admissible
inspection actions and terminal return actions.

For such a procedure `x`, define:

```text
Q_x      = set of inquiry states for x
A_x(q)   = set of admissible actions from inquiry state q ∈ Q_x
δ_x      = inquiry transition function
T_x ⊆ Q_x = terminal inquiry states
Ret_x    = set of admissible return payloads for x
```

with:

```text
δ_x : Q_x × A_x(q) -> Q_x
```

and terminal return discipline:

```text
if q ∈ T_x, then q carries some r where r ∈ Ret_x
```

Interpretation:

- an inquiry state is not just raw evidence; it is evidence plus a current set
  of admissible next moves
- some actions inspect more evidence
- some actions refine intermediate structure
- at least one action returns a terminal payload

### 16.1 Inspection And Return Actions

Useful inquiry actions often fall into two classes:

```text
Inspect_x(q) ⊆ A_x(q)
Return_x(q)  ⊆ A_x(q)
```

Interpretation:

- `Inspect_x(q)` reveals more admissible evidence without leaving the procedure
- `Return_x(q)` terminates the inquiry with a structured output

### 16.2 Boundedness In Inquiry Procedures

For inquiry procedures, boundedness may be enforced not only by time or token
 limits but by action constraints.

Examples:

```text
NoRepeat_x(q, a) := action a may not be taken again from descendant states
Budget_x(q)      := remaining action or evidence-expansion budget
```

This allows bounded optional loops such as:

- inspect item `i`
- inspect item `j`
- return payload

without permitting unbounded drift.

### 16.3 Inquiry Procedures As Procedure States

An inquiry procedure is still compatible with the broader state framework.

Its execution may be viewed as:

```text
s_0 -> q_0 -> q_1 -> ... -> q_t -> s_out
```

where:

- `s_0` is the initial input state
- `q_i` are inquiry states internal to the procedure
- `q_t ∈ T_x`
- `s_out` is the output state constructed from the terminal return payload

Interpretation:

- inquiry is not outside the procedure model
- it is a structured internal mode of execution within it

## 17. Worked Sketch: `segment_tools`

Suppose `x_seg` is a local trace-analysis procedure whose goal is to segment an
ordered tool-call sequence into intent clusters.

### 17.1 Initial State

Let:

```text
s_0 ∈ I_(x_seg)
```

carry:

- ordered tool-call summaries
- minimal per-call metadata
- admissible inspection handles for call-level expansion

### 17.2 Inquiry State

Construct:

```text
q_0 = InitInquiry(x_seg, s_0)
```

where `q_0 ∈ Q_(x_seg)` carries:

- the visible sequence summary
- the set of calls already inspected
- the remaining inspect budget
- the current candidate segmentation, possibly empty

### 17.3 Admissible Actions

Possible actions include:

```text
inspect(i)   = inspect call i in more detail
return(c)    = return a candidate clustering c
```

with:

```text
inspect(i) ∈ A_(x_seg)(q)  only if i has not already been inspected
return(c)  ∈ A_(x_seg)(q)  only if c satisfies the procedure output contract
```

The transition function:

```text
δ_(x_seg)(q, inspect(i)) = q'
```

updates the inquiry state by:

- adding call `i` to the inspected set
- adding the admissible expanded evidence for `i`
- decrementing any relevant inspection budget

### 17.4 Return Payload

Let:

```text
c ∈ Ret_(x_seg)
```

be a structured clustering payload.

Let:

```text
I_call = {0, 1, ..., n-1}
```

be the index set of tool calls in the ordered sequence under inspection.

A clustering payload may be modeled minimally as:

```text
c = (C, λ, μ)
```

where:

```text
C = {C_1, ..., C_k}
```

is a family of non-empty subsets of `I_call`,

```text
λ : C -> L ∪ {unknown}
```

assigns each cluster an intent label from some label set `L` or `unknown`, and

```text
μ : C -> U_conf
```

assigns each cluster an uncertainty or confidence marker from some finite set
`U_conf`.

Admissibility constraints for `c` may include:

- each `C_i` is non-empty
- cluster members preserve sequence order when rendered back into the trace
- clusters are pairwise disjoint when a strict partition is intended
- `⋃ C_i = I_call` when a full partition is intended
- `⋃ C_i ⊂ I_call` when unclustered residual calls are permitted by the
  procedure

This lets the procedure choose between:

- strict partition of the full trace
- partial clustering with admissible residual calls

depending on the intended output contract.

Return validity is not implicit.

If a proposed payload fails the output contract, the inquiry should not silently
terminate. Instead, define bounded recovery.

Let:

```text
attempts(q) ∈ {0, 1, 2, 3}
```

count failed return attempts in inquiry state `q`.

If `return(c)` is proposed but `c ∉ Ret_(x_seg)`, then:

```text
δ_(x_seg)(q, return(c)) = q_recover
```

where `q_recover` carries:

- a recovery hint to the AM describing the violated output constraint
- the incremented failed-attempt count
- the still-admissible next actions

and boundedness requires:

```text
attempts(q_recover) ≤ 3
```

If the AM exceeds the retry bound, the procedure must terminate in an explicit
failure state rather than loop indefinitely.

Then:

```text
δ_(x_seg)(q, return(c)) = q_t
```

with:

```text
q_t ∈ T_(x_seg)
```

and `q_t` carries `c`.

### 17.5 Output State

The inquiry result is then reified as a later procedure state:

```text
s_cluster = Out(x_seg, q_t)
```

Possible role assignment:

```text
s_cluster ∈ U_(x_seg)
```

and possibly:

```text
s_cluster ∈ K_(x_seg)
```

if the clustering is itself worth recording as a supporting metric or durable
intermediate artifact.

### 17.6 Why This Matters

This procedure is useful because it transforms a flat ordered sequence into a
more semantically meaningful structured state that later procedures can consume.

For example, downstream procedures may branch from `s_cluster` to assess:

- cluster-level success or failure
- cluster-level failure cause
- call-level contribution within a cluster
- search drift or thrash
- retrieval relevance within a cluster

So `segment_tools` is a good example of a state that is:

- not yet the final target metric
- clearly evidential for downstream target metrics
- worth recording and forwarding
