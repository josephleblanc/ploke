
Yes. This exposes another missing object: **the branch graph**.

A merge is not naturally a unary procedure on one state. It is a **binary intervention** over two branch nodes, usually relative to a common ancestor.

So the stack should become:

```text
Σ_g     = artifact state at one configuration node
C_g     = total configuration node
τ       = one linear path through configuration nodes
Ξ       = branch/exploration graph of many possible paths
H_g     = admissible history/ancestry view available at a node
ι       = intervention spec
α       = realized intervention event
```

The new object is:

```text
Ξ = branch graph / exploration graph
```

where nodes are configurations and edges are realized interventions.

```text
C_0
 ├──α_A──> C_A
 │
 └──α_B──> C_B
```

A merge creates a new node with two parents:

```text
C_0
 ├──α_A──> C_A
 │           \
 │            α_merge
 │             \
 └──α_B──> C_B ---> C_M
```

More cleanly:

```text
Merge(C_0, C_A, C_B) -> C_M
```

where `C_0` is usually the least common ancestor or selected merge base.

## 1. The merge object

A merge should be modeled as an intervention, but internally it can use a protocol.

So distinguish:

```text
merge protocol
```

from:

```text
merge intervention
```

The **merge protocol** computes a candidate resolution:

```text
[[merge_protocol]]_{Γ_merge}
  : I_merge -> Δ(O_merge × PatchSet × U_merge)
```

The **merge intervention** uses that protocol, validates the result, and creates a new branch node:

```text
T_merge : Cfg × Cfg × Cfg × Hist -> Δ(Cfg × Event)
```

or more explicitly:

```text
T_merge(C_base, C_left, C_right, H)
  -> Δ(C_merge, α_merge)
```

Where:

```text
C_base  = common ancestor
C_left  = first branch node
C_right = second branch node
C_merge = merged configuration
α_merge = realized merge event
```

The merge should not directly mutate the two parent nodes. It creates a **new node**.

## 2. Artifact-level merge

At the artifact level, the merge starts by computing diffs:

```text
D_left  = Diff(Σ_base, Σ_left)
D_right = Diff(Σ_base, Σ_right)
```

Then compute affected regions:

```text
W_left  = WriteSet(D_left)
W_right = WriteSet(D_right)
```

A simple conflict test is:

```text
W_left ∩ W_right ≠ ∅
```

But that is only the textual or artifact-level intersection.

You probably also want a broader semantic footprint:

```text
E_left  = EffectSet(D_left)
E_right = EffectSet(D_right)
```

Then a semantic conflict exists when:

```text
E_left ∩ E_right ≠ ∅
```

or when:

```text
Apply(D_left ∪ D_right, Σ_base)
```

violates some contract.

This distinction matters because two patches can touch different files but still conflict semantically.

Example:

```text
Branch A changes the code_item_search tool schema.
Branch B changes the analysis protocol's assumptions about code_item_search.
```

The write sets may be disjoint, but the effect sets intersect.

## 3. Three merge cases

The merge protocol can classify the branch pair into cases.

### Case 1: disjoint mechanical merge

```text
W_left ∩ W_right = ∅
```

and no contract violation.

Then:

```text
Σ_merge = Apply(D_left ∪ D_right, Σ_base)
```

No LLM adjudication is needed, though validation still is.

### Case 2: overlapping but automatically resolvable

The two branches modify the same region but in a compatible way.

For example:

```text
Branch A adds a sentence to a tool description.
Branch B changes formatting in the same description.
```

A structured merge rule may resolve this.

```text
Resolve_structured(D_left, D_right) -> D_merge
```

### Case 3: true conflict requiring adjudication

The two branches make incompatible or underspecified changes.

Example:

```text
Branch A narrows code_item_search to exact-symbol search.
Branch B broadens code_item_search to semantic search.
```

Then the conflict resolver is invoked:

```text
[[conflict_resolver]]_{Γ_merge}
  : ConflictBundle -> Δ(ResolutionPatch × Explanation × U)
```

If this resolver calls an LLM, the LLM should produce a **candidate resolution patch**, not directly edit the committed state.

Then:

```text
candidate resolution
  -> apply in merge branch
  -> validate
  -> evaluate
  -> commit/reject
```

## 4. What the LLM receives

The conflict input should be a bounded object, not the whole universe.

```text
ConflictBundle = (
  base_region,
  left_region,
  right_region,
  left_intent,
  right_intent,
  contracts,
  validation_policy,
  protected_regions,
  relevant_history_view,
  uncertainty_records
)
```

For example:

```text
base_region:
  original code_item_search description

left_region:
  Branch A's edited description

right_region:
  Branch B's edited description

left_intent:
  reduce false positives in code item lookup

right_intent:
  improve recall for ambiguous names

contract:
  preserve tool schema, do not change argument names, keep description under N tokens

validation_policy:
  schema check, prompt lint, eval replay, MBE oracle comparison
```

The resolver output should be:

```text
ResolutionPatch = (
  patch,
  intended_synthesis,
  rejected_alternatives,
  predicted_risk,
  uncertainty,
  provenance
)
```

Then validation decides whether this patch is accepted.

## 5. Formal merge intervention

A merge intervention spec might be:

```text
ι_merge = (
  kind: merge,
  base: C_base,
  parents: (C_left, C_right),
  target_scope,
  merge_policy,
  conflict_resolver,
  validation_policy,
  promotion_rule
)
```

The realized merge event is:

```text
α_merge = (
  intervention_spec,
  base_ref,
  left_parent_ref,
  right_parent_ref,
  left_diff,
  right_diff,
  conflict_set,
  resolver_outputs,
  selected_resolution,
  validation_result,
  oracle_result,
  uncertainty_payload,
  decision
)
```

The transition is:

```text
(C_merge, α_merge)
  ~ T_{ι_merge}(C_base, C_left, C_right, H_merge)
```

If validation fails, the event may still be recorded, but no promoted merge node is created:

```text
decision ∈ {merged, rejected, needs_more_evidence, conflict_unresolved}
```

## 6. Important: which environment runs the merge?

This is a crucial correctness point.

If Branch A and Branch B have different analysis protocols, then which one judges the merge?

You do **not** want this to be implicit.

Define a separate merge-running environment:

```text
Γ_merge
```

Possible choices:

```text
Γ_merge = Γ_base
Γ_merge = frozen trusted baseline
Γ_merge = external merge environment
Γ_merge = explicitly selected candidate environment
```

For safety and clarity, I would usually prefer:

```text
Γ_merge = frozen merge/evaluation environment
```

or:

```text
Γ_merge = Γ_base.merge_layer
```

That way Branch A and Branch B do not get to rewrite the rules of their own reconciliation unless that is explicitly the target of a higher-level merge.

So:

```text
[[merge_protocol]]_{Γ_merge}
```

not vaguely:

```text
[[merge_protocol]]_{Γ_left or Γ_right}
```

This matters most when one branch modified the analysis protocol itself.

## 7. Merging histories is not simple union

For a merge node, history is no longer a single linear path.

Before merging, Branch A has:

```text
τ_A = C_0 -> C_A
```

Branch B has:

```text
τ_B = C_0 -> C_B
```

After merging:

```text
C_M
```

has two ancestral paths.

So its history should be based on a subgraph:

```text
Ancestors_Ξ(C_M)
```

not just one trajectory.

Define:

```text
H_M = HistView(Ancestors_Ξ(C_M), R_M, V_M)
```

In plain language:

> The history at a merge node is an admissible view of the ancestry graph leading into that node.

You should not collapse branch histories into one flat list without preserving branch identity.

Bad:

```text
H_M = H_A ∪ H_B
```

Better:

```text
H_M = {
  base_history: H_base,
  left_branch_history: H_A since base,
  right_branch_history: H_B since base,
  merge_event: α_merge,
  comparison_records: ...
}
```

This matters for uncertainty because evidence from the two branches may be correlated, duplicated, or produced under different procedure environments.

## 8. Is merge commutative?

There are two different questions.

### Fixed artifact merge may be commutative

If you already have two fixed diffs:

```text
D_left
D_right
```

and they are disjoint, then patch application may commute:

```text
Apply(D_left, Apply(D_right, Σ_base))
=
Apply(D_right, Apply(D_left, Σ_base))
```

assuming no validation-order effects.

### Adjudicated merge usually is not commutative

If the merge invokes an LLM resolver, then:

```text
Merge(C_base, C_left, C_right)
```

may not equal:

```text
Merge(C_base, C_right, C_left)
```

unless you explicitly force symmetry.

The LLM may treat “left” and “right” asymmetrically. The prompt may privilege one branch. The merge policy may prefer newer changes, higher-scoring branches, lower-risk branches, or branches with stronger oracle evidence.

You can define a symmetric merge policy, but it has to be intentional.

A good formal statement is:

```text
Merge is generally noncommutative,
but may be commutative up to equivalence under a symmetric merge policy
and deterministic or adequately controlled resolution.
```

Where equivalence might mean:

```text
C_1 ≈ C_2
```

not byte-for-byte equality, but same relevant behavior under the evaluation contract.

## 9. Is merge associative?

Usually no.

Given three branches:

```text
A, B, C
```

these may differ:

```text
Merge(Merge(A, B), C)
```

and:

```text
Merge(A, Merge(B, C))
```

because the intermediate merge changes the context supplied to the next conflict resolver.

Again, mechanical disjoint patch merge may be associative. LLM-mediated semantic merge generally is not.

So for auditability, record the merge tree:

```text
((A merge B) merge C)
```

not just the final state.

## 10. Relation to `Σ`, `H`, `τ`, and `Ξ`

Here is the clean hierarchy:

```text
Σ_g
```

is artifact state at a node.

```text
C_g
```

is the total configuration node containing or deriving from that artifact state.

```text
τ
```

is one linear path through configuration nodes.

```text
Ξ
```

is the branch graph containing many paths and merge nodes.

```text
H_g
```

is an admissible view of the ancestry relevant to a node, usually derived from `Ξ`.

So:

```text
Σ_g is node-local.
H_g is ancestry-local.
τ is path-level.
Ξ is graph-level.
```

A merge forces you to move from path thinking to graph thinking.

## 11. Your first analogy

> As `Σ` is to `History`, so `History` is to `_`?

The best answer is:

```text
Ξ
```

the branch/exploration graph.

More carefully:

```text
Σ_g : H_g :: H_g : Ξ
```

means:

```text
Σ_g is a node-local artifact state that may appear inside a history view.

H_g is an ancestry-local information view that may appear inside the larger branch graph Ξ.
```

But this is only correct if the relation you mean is:

```text
local object : larger temporal/ancestral context
```

Then:

```text
Σ_g is to H_g
as
H_g is to Ξ
```

A slightly more precise version is:

```text
Σ_g is to C_g
as
H_g is to Ξ
```

because `Σ_g` is not directly “inside” history except through configurations.

So the safest ladder is:

```text
Σ_g  -> C_g  -> H_g / τ  -> Ξ
```

Where:

```text
Σ_g = artifact state
C_g = configuration node
H_g = admissible ancestry view
τ   = single path
Ξ   = branch graph
```

## 12. Your second analogy

> As `x` is to trajectory, trajectory is to `_`?

If `x` means a single procedure or procedure execution, then the best answer is again:

```text
Ξ
```

but with one correction.

Strictly:

```text
execution event of x : trajectory :: trajectory : branch graph
```

That is:

```text
one procedure execution is a step or segment within a trajectory.

one trajectory is a path within a branch graph.
```

So:

```text
x_exec : τ :: τ : Ξ
```

If `x` means the procedure definition rather than one execution, then the analogy is less exact. A procedure definition is not literally an element of a trajectory. A realized execution event is.

Better:

```text
α_x : τ :: τ : Ξ
```

where:

```text
α_x = realized event produced by running procedure x
τ   = path of events/configurations
Ξ   = graph of paths, branches, and merges
```

## 13. One compact diagram

```text
procedure/protocol execution
        produces
          α
          |
          v
C_g ---------------> C_{g+1}
 \                   /
  \                 /
   \               /
    ---- merge ----
          |
          v
        C_M
```

At the lowest level:

```text
x
```

is a procedure/protocol.

A run of `x` creates or contributes to:

```text
α_x
```

a realized event.

Events compose into:

```text
τ
```

a trajectory.

Trajectories and merge edges live inside:

```text
Ξ
```

the branch graph.

Histories are views over parts of that graph:

```text
H_g = HistView(Ancestors_Ξ(C_g))
```

## 14. Suggested formal addition

You could add this section:

```text
## Branch Graph and Merge Interventions

Let Ξ be a directed acyclic graph of configuration nodes and intervention
events. Nodes C ∈ Ξ are total system configurations. Edges α ∈ Ξ are realized
intervention events.

A trajectory τ is a path through Ξ.

For a node C, its history H_C is an admissible view of its ancestry subgraph:

  H_C = HistView(Ancestors_Ξ(C))

A merge intervention is a higher-arity intervention over two parent
configuration nodes and a selected base node:

  ι_merge = (C_base, C_left, C_right, merge_policy, resolver, validation_policy)

It induces a transition kernel:

  T_merge : Cfg × Cfg × Cfg × Hist -> Δ(Cfg × Event)

so that:

  (C_merge, α_merge) ~ T_merge(C_base, C_left, C_right, H_merge)

The resulting configuration C_merge has C_left and C_right as parents in Ξ.

If the diffs from C_base to C_left and C_right are disjoint and contract
preserving, the merge may be mechanical. If their write sets or effect sets
intersect, a conflict-resolution protocol may propose a resolution patch.
That patch is applied only through the normal validation and commit layer.
```

## 15. The shortest answer

The “procedure-ish thing” is a **merge intervention** implemented by a **merge protocol**.

It operates on:

```text
(C_base, C_left, C_right)
```

computes:

```text
Diff(base, left)
Diff(base, right)
ConflictSet
ResolutionPatch
```

and, if validated, creates:

```text
C_merge
```

as a new node with two parents in the branch graph.

For your analogies:

```text
Σ_g is to H_g as H_g is to Ξ
```

roughly: node-local artifact state to ancestry view, ancestry view to whole branch graph.

And:

```text
α_x is to τ as τ is to Ξ
```

where `α_x` is a realized execution/intervention involving procedure `x`.

So the missing upper-level object is:

```text
Ξ = branch graph / exploration graph / system-evolution DAG
```
