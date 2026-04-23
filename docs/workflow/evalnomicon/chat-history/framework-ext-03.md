
Yes. The missing definitions should be something like this:

```text
C_g          = total configuration at generation g
τ            = trajectory: the path of configurations and transition events
H_g          = history available at generation g
ι or α        = intervention: a typed action/change-attempt applied to a configuration
Π            = policy: chooses interventions from history
```

I would avoid using `I` for interventions because you already use `I_x` for input states. Use something like:

```text
ι  = intervention spec
α  = realized intervention event
```

## 1. Trajectory

A trajectory is the actual path the system has taken.

The simplest state-only version is:

```text
τ : ℕ -> Cfg
```

where:

```text
τ(g) = C_g
```

But that loses the most important information: **what caused each transition**.

So the better version is an alternating sequence:

```text
τ_{0:g} =
  C_0 --α_0--> C_1 --α_1--> C_2 --α_2--> ... --α_{g-1}--> C_g
```

or written as a tuple:

```text
τ_{0:g} = (C_0, α_0, C_1, α_1, ..., α_{g-1}, C_g)
```

Each `α_i` is the realized intervention event that moved the system from `C_i` to `C_{i+1}`.

So:

```text
τ = what happened
```

It is the actual path, not the policy, not the environment, and not the mutable artifact state.

## 2. History

`History` is not exactly the same as trajectory.

History is the information available to the system when choosing what to do next.

So:

```text
H_g = available historical information at generation g
```

The clean definition is:

```text
H_g = HistView(τ_{0:g}, branch_records, rejected_candidates, eval_records, uncertainty_records)
```

or more compactly:

```text
H_g = HistView(τ_{0:g}, R_g, V_g)
```

where:

```text
R_g = records, eval results, provenance, uncertainty ledgers
V_g = version graph, branches, snapshots, restore points
```

The distinction matters because the full trajectory may contain more information than a given policy is allowed to use.

For example:

```text
τ_{0:g}
```

might include every raw trace, patch, failed branch, seed, prompt, adjudicator output, and oracle result.

But the intervention policy may only be allowed to see:

```text
summarized eval deltas
approved uncertainty summaries
current branch state
validated failure diagnoses
```

So `H_g` is a **view or projection** of the past.

```text
H_g = View_allowed(τ_{0:g})
```

In implementation terms, `H_g` is what your improvement controller reads before deciding the next action.

## 3. Why not just use `C_g`?

Sometimes you can.

If your current configuration `C_g` contains a complete ledger of prior interventions, eval results, rejected patches, branches, and uncertainty records, then you can treat history as recoverable from configuration:

```text
H_g = HistView(C_g)
```

That is the “state is sufficient” case.

But it is often safer to keep the distinction:

```text
C_g = current configuration
H_g = current configuration plus admissible memory of how we got here
```

For self-improving systems, this distinction is important because two identical-looking current configurations may have different evidential status.

For example:

```text
C_g and C'_g have the same current code
```

but:

```text
C_g got there after 30 failed branches and strong oracle evidence
C'_g got there after one lucky run
```

Those should not necessarily be treated the same.

So history carries epistemic weight.

## 4. Intervention spec versus realized intervention

I would distinguish two levels.

An **intervention spec** is the action the system intends to try:

```text
ι_g ∈ Act(C_g, H_g)
```

A **realized intervention event** is what actually happened:

```text
α_g
```

The spec might say:

```text
try to improve code_item_search using the tool-improvement protocol
```

The realized event records:

```text
what evidence was used
what patch was proposed
what branch was created
whether the patch built
what evals were run
what the oracle said
whether the patch was promoted or rejected
what uncertainty was assigned
```

So:

```text
ι = planned/selected intervention
α = realized transition event
```

This is useful because a planned intervention may fail, partially succeed, or result in no artifact change.

## 5. Intervention as a typed object

An intervention spec can be defined as:

```text
ι = (
  kind,
  target,
  read_scope,
  write_scope,
  proposer,
  validation_policy,
  promotion_rule,
  budget,
  utility_model
)
```

where:

```text
kind              = object-level, meta-level, eval-only, restore, promote, etc.
target            = artifact or configuration region to affect
read_scope         = what the procedure may inspect
write_scope        = what it may modify
proposer           = procedure/protocol used to generate the candidate change
validation_policy  = checks required before commit
promotion_rule     = rule for accepting into mainline
budget             = time/token/eval/search bounds
utility_model      = what counts as improvement
```

For example:

```text
ι_tool =
  kind: object_tool_mutation
  target: Σ.tool.code_item_search
  read_scope: failure diagnoses involving code_item_search
  write_scope: description string or source files for code_item_search
  proposer: tool_improvement_protocol
  validation_policy: build + schema + regression eval + MBE oracle
  promotion_rule: promote if posterior improvement exceeds threshold
```

And:

```text
ι_analysis =
  kind: meta_analysis_mutation
  target: Σ.analysis.failure_analysis_protocol
  read_scope: prior diagnoses, oracle outcomes, disagreement records
  write_scope: analysis protocol source/prompt/schema
  proposer: analysis_improvement_protocol
  validation_policy: build + schema + provenance preservation + held-out evals
  promotion_rule: promote if downstream tool-patch quality improves
```

## 6. Intervention as a transition kernel

Because your system includes LLMs, stochastic patch generation, eval variation, and uncertain analysis, an intervention should usually be modeled probabilistically.

A selected intervention spec induces a transition kernel:

```text
T_ι : C_g × H_g -> Δ(C_{g+1} × α_g)
```

Read this as:

> Given the current configuration and history, trying intervention `ι` induces a distribution over next configurations and realized intervention events.

In deterministic cases:

```text
C_{g+1} = Step(C_g, ι)
```

But for your setting, the probabilistic version is more accurate:

```text
(C_{g+1}, α_g) ~ T_ι(C_g, H_g)
```

The realized event `α_g` is then appended to the trajectory:

```text
τ_{0:g+1} =
  τ_{0:g} --α_g--> C_{g+1}
```

## 7. Policy

The policy chooses which intervention to try next.

```text
Π : H_g -> Δ(Act(C_g))
```

or, deterministically:

```text
Π : H_g -> Act(C_g)
```

So the loop is:

```text
H_g = HistView(τ_{0:g})
ι_g ~ Π(H_g)
(C_{g+1}, α_g) ~ T_{ι_g}(C_g, H_g)
τ_{0:g+1} = τ_{0:g} extended by α_g and C_{g+1}
```

This gives you the adaptive trajectory framing:

```text
history -> intervention choice -> transition -> new history
```

That is the piece needed for your “continue along either branch as performance changes” idea.

## 8. What is inside a realized intervention event?

A realized intervention event should be richer than a label like “tool mutation.”

For your use case:

```text
α_g = (
  intervention_spec,
  pre_state_ref,
  post_state_ref,
  evidence_state,
  target,
  proposed_patches,
  selected_patch,
  branch_ref,
  validation_result,
  oracle_result,
  uncertainty_payload,
  provenance,
  decision
)
```

Where:

```text
decision ∈ {promoted, rejected, logged_only, restored, forked, abandoned}
```

Example:

```text
α_17 = {
  spec: ι_tool,
  pre_state: C_17,
  target: Σ.tool.code_item_search,
  evidence: failure diagnosis from eval run E_42,
  proposed_patch: p_17,
  branch: branch/tool-code-item-search-17,
  validation: build_passed, schema_passed, unit_tests_passed,
  oracle_result: MBE improvement +0.031,
  uncertainty: P(improvement > 0) = 0.91,
  decision: promoted,
  post_state: C_18
}
```

This makes the transition auditable.

## 9. Mainline trajectory versus branch tree

Once you have branches, one linear trajectory may not be enough.

You may need two related objects:

```text
τ = selected mainline trajectory
Ξ = exploration tree / branch graph
```

A branch graph has configurations as nodes and intervention events as edges:

```text
C_0
 ├──α_A──> C_A
 │          └──α_A2──> C_A2
 └──α_B──> C_B
            └──α_B2──> C_B2
```

A single trajectory is one path through this tree:

```text
τ_A = C_0 --α_A--> C_A --α_A2--> C_A2
```

Another trajectory is:

```text
τ_B = C_0 --α_B--> C_B --α_B2--> C_B2
```

So for your Branch A / Branch B example:

```text
Ξ = the whole experimental branch graph
τ_A = the object-tool-mutation path
τ_B = the analysis-mutation-then-tool-mutation path
```

This is better than trying to force every branch into one linear sequence.

## 10. History on a branch

Each branch has its own local history:

```text
H^A_g = HistView(τ^A_{0:g}, Ξ)
H^B_g = HistView(τ^B_{0:g}, Ξ)
```

The branch-local history includes the path taken on that branch.

The global branch graph `Ξ` may include sibling branch outcomes too, but whether a policy can see sibling branches is a design choice.

For example, when choosing the next intervention on Branch B, do you allow it to see that Branch A failed?

Two options:

```text
local-only history:
  H^B_g = only the evidence on Branch B

global experimental history:
  H^B_g = Branch B evidence plus relevant results from Branch A
```

Both are legitimate, but they answer different questions.

If you want a clean causal comparison, you often want local-only history during execution, then global history during comparison.

## 11. Types of interventions

You probably want a small intervention taxonomy.

```text
Observe
Evaluate
Diagnose
PatchObject
PatchAnalysis
PatchImprover
Fork
Restore
Validate
Promote
Reject
Calibrate
Stop
```

More formally:

```text
kind(ι) ∈ {
  observe,
  evaluate,
  diagnose,
  object_mutation,
  meta_mutation,
  meta_meta_mutation,
  branch_operation,
  validation_operation,
  promotion_operation,
  restore_operation,
  calibration_update,
  terminal
}
```

Examples:

```text
object_mutation:
  target = tool description or tool implementation

meta_mutation:
  target = analysis protocol

meta_meta_mutation:
  target = tool-improvement protocol itself

branch_operation:
  fork, checkpoint, checkout, restore

validation_operation:
  build, run tests, run oracle, compare branches

calibration_update:
  update uncertainty model or executor reliability estimate
```

The taxonomy matters because each kind has different allowed write scopes and validation rules.

## 12. The target structure of an intervention

For correctness, the target should be explicit:

```text
Target T = (
  artifact_ref,
  level,
  allowed_region,
  preserved_regions,
  contract,
  validation_policy
)
```

where:

```text
level ∈ {object, meta, meta_meta, infrastructure}
```

Example object-level target:

```text
T_tool = {
  artifact_ref: code_item_search,
  level: object,
  allowed_region: Σ.tool.code_item_search.description,
  preserved_regions: Σ.analysis, Σ.oracle, Σ.benchmark,
  contract: tool schema and output contract,
  validation_policy: description parse + eval harness build + oracle eval
}
```

Example meta-level target:

```text
T_analysis = {
  artifact_ref: failure_analysis_protocol,
  level: meta,
  allowed_region: Σ.analysis.failure_analysis_protocol,
  preserved_regions: Σ.tool, Σ.oracle, Σ.benchmark, promotion_policy,
  contract: output schema + provenance + uncertainty fields,
  validation_policy: build + schema + held-out eval + downstream utility
}
```

This is how you prevent a self-improving system from silently changing the thing that judges improvement.

## 13. How History, Policy, and Intervention connect to uncertainty

The probabilistic layer fits here:

```text
Bel_g = Belief(H_g)
```

where `Bel_g` might contain:

```text
posterior over failure causes
posterior over patch efficacy
executor reliability estimates
uncertainty about branch comparisons
risk of regression
expected value of further evaluation
```

The policy can then choose interventions by expected utility:

```text
Π(H_g) =
  argmax_{ι ∈ Act(C_g)}
    E[Utility(C_{g+1}) - Cost(ι) | H_g, ι]
```

or probabilistically:

```text
Π(H_g)(ι) ∝ exp(ExpectedUtility(ι | H_g))
```

So the system can choose among:

```text
patch the tool
patch the analysis protocol
run more evals
inspect more traces
restore a previous state
promote a branch
stop
```

based on expected value under uncertainty.

## 14. Your Branch A / Branch B in this notation

Let:

```text
C_0 = initial configuration
H_0 = initial history
```

Branch A intervention:

```text
ι_A =
  object_mutation targeting Σ.tool.code_item_search
```

Then:

```text
(C_A, α_A) ~ T_{ι_A}(C_0, H_0)
```

Branch B intervention:

```text
ι_B =
  meta_mutation targeting Σ.analysis.failure_analysis_protocol
```

Then:

```text
(C_B, α_B) ~ T_{ι_B}(C_0, H_0)
```

After Branch B modifies the analysis protocol, it may run an object-level intervention using the new analysis environment:

```text
ι_{B2} =
  object_mutation targeting Σ.tool.code_item_search
  under Γ_B.analysis
```

Then:

```text
(C_{B2}, α_{B2}) ~ T_{ι_{B2}}(C_B, H_B)
```

Now compare:

```text
Oracle(C_A)
Oracle(C_{B2})
```

or more probabilistically:

```text
P(Utility(C_{B2}) > Utility(C_A) + ε | H_compare)
```

## 15. Interventions compose, but not always cleanly

Given two interventions:

```text
ι_1
ι_2
```

their composition is only defined relative to the configuration produced by the first:

```text
C_0 --α_1--> C_1 --α_2--> C_2
```

So:

```text
ι_2 ∘ ι_1
```

really means:

```text
try ι_1 from C_0,
then choose or execute ι_2 from the resulting C_1
```

Because `C_1` may differ depending on the realized result of `ι_1`, intervention composition is generally history-sensitive.

That is why object-level and meta-level mutation generally do not commute:

```text
PatchAnalysis ; PatchTool ≠ PatchTool ; PatchAnalysis
```

because the second intervention is selected and executed under a different history and configuration.

## 16. Minimal formal block

I would add something like this to the framework:

```text
Let Cfg be the space of total system configurations.

A trajectory prefix is an alternating sequence:

  τ_{0:g} = (C_0, α_0, C_1, ..., α_{g-1}, C_g)

where each C_i ∈ Cfg and each α_i is a realized intervention event.

A history state is an admissible view of the trajectory prefix and associated
branch/evaluation records:

  H_g = HistView(τ_{0:g}, R_g, V_g)

History need not expose all facts in the trajectory. It exposes only those
records admissible for the next decision procedure.

An intervention spec is a typed action available at a configuration:

  ι ∈ Act(C_g, H_g)

An intervention induces a transition kernel:

  T_ι : Cfg × Hist -> Δ(Cfg × Event)

so that:

  (C_{g+1}, α_g) ~ T_ι(C_g, H_g)

A policy selects interventions from history:

  Π : Hist -> Δ(Act)

The adaptive improvement loop is:

  H_g = HistView(τ_{0:g}, R_g, V_g)
  ι_g ~ Π(H_g)
  (C_{g+1}, α_g) ~ T_{ι_g}(C_g, H_g)
  τ_{0:g+1} = (τ_{0:g}, α_g, C_{g+1})
```

## 17. In plain language

`τ` is the path.

`H_g` is what the system knows or is allowed to use at step `g`.

`ι_g` is what the system decides to try next.

`α_g` is what actually happened when it tried.

`Π` is the controller/policy that chooses interventions.

`T_ι` is the possibly stochastic transition induced by trying intervention `ι`.

For your framework, this gives you the clean conceptual loop:

```text
history of prior evals, patches, failures, branches, and uncertainty
  -> choose bounded intervention
  -> apply/validate/evaluate intervention
  -> produce new configuration and event record
  -> update history
  -> continue, branch, restore, promote, or stop
```

That is the layer above individual procedures and protocols. Individual procedures transform evidence states. Protocols compose procedures. Interventions use protocols to attempt bounded changes to the system configuration. Trajectories record the resulting evolution over time.
