
Yes — I think you are circling a real conceptual boundary in the framework.

The thing you are describing is **not merely a procedure that produces an output**. It is a procedure whose output may be an **intervention on the future procedure environment**.

That suggests a major extension:

```text
ordinary procedure:
  evidence state -> output state

mutable procedure:
  evidence state + artifact state -> output state + artifact update

reflective mutable procedure:
  evidence state + artifact state + procedure environment
    -> output state + future procedure environment
```

The important move is to avoid saying “the function modifies itself” too literally. In your compiled-language setting, it is better to say:

> The current generation of the system can propose and validate edits to artifacts that determine the behavior of a later generation of the system.

That gives you self-modification without semantic chaos.

## 1. The core distinction: local state versus artifact state

Your instinct about “a second state” is right, but I would frame it as a split between:

```text
s  = local procedure/evidence state
Σ  = mutable system/artifact state
```

The local state `s` is the thing your current framework already handles well:

```text
trace evidence
tool-call segmentation
failure diagnosis
supporting observations
uncertainty payloads
provenance
```

The mutable artifact state `Σ` is the state of the world that procedures may inspect or modify:

```text
tool descriptions
tool implementations
prompt templates
analysis protocol code
compiled binaries
evaluation harness configuration
version graph / branches
validation results
commit hashes
build artifacts
```

Then an ordinary read-only procedure can remain:

```text
Exec(e, x, s) = s'
```

But a mutation-capable procedure becomes:

```text
Exec(e, x, s, Σ) = (s', Σ')
```

or, probabilistically:

```text
K_(e,x) : I_x × Σ -> Δ(O_x × Σ)
```

This says:

> Given an admissible evidence state and a mutable artifact state, procedure `x` induces a distribution over output states and possible updated artifact states.

But I would actually separate **proposal** from **commit**.

Instead of letting an LLM/procedure directly mutate `Σ`, define:

```text
K_(e,x) : I_x × View_R(Σ) -> Δ(O_x × Patch)
```

Then a trusted transaction layer handles:

```text
ApplyValidateCommit : Σ × Patch -> Σ'
```

That gives you much better conceptual and implementation hygiene.

The procedure proposes a patch. The system decides whether that patch is admissible, valid, reversible, and worth promoting.

## 2. The target of modification should be an artifact, not a vague “state”

To keep the theory general while making the implementation bounded, define a generic artifact layer.

```text
A ∈ Artifacts
```

An artifact may be:

```text
tool description
tool source code
tool schema
prompt template
analysis procedure definition
protocol graph
binary build artifact
test configuration
calibration model
```

Then a mutation target is not just “the system.” It is a bounded artifact reference:

```text
T = (artifact_ref, allowed_region, contract, validation_policy)
```

Examples:

```text
T_1 = tool_description(code_item_search)

T_2 = tool_implementation(code_item_search)

T_3 = analysis_protocol_module(failure_diagnosis)

T_4 = protocol_graph(tool_improvement_protocol)
```

This lets the formalism stay general:

```text
mutate artifact A under bounds B
```

while the implementation stays concrete:

```text
only edit the description string for tool code_item_search
```

or:

```text
only edit files under crates/harness/tools/code_item_search/
```

or:

```text
only edit the analysis protocol module, not the oracle, benchmark runner, or promotion gate
```

A patch is admissible only if its write footprint stays inside the target boundary:

```text
AdmissiblePatch(p, T, Σ)
  iff WriteSet(p) ⊆ AllowedRegion(T)
   ∧ PreservesContract(p, T.contract)
   ∧ PassesValidationPolicy(p, T.validation_policy)
```

This gives you the practical guardrails without baking “tool prompt” or “source file” too deeply into the theory.

## 3. Use staged self-modification, not immediate self-modification

The tricky part is when the target being modified affects the procedure that will later perform analysis.

That should be modeled with **generations**.

Let:

```text
Γ_g = procedure/protocol environment at generation g
Σ_g = artifact state at generation g
s_g = local evidence/procedure state at generation g
```

A procedure runs under the current environment:

```text
[[x]]_{Γ_g}
```

If it modifies procedure `x` itself, or modifies a protocol used by `x`, the modification does **not** alter the meaning of the currently running procedure. It produces a future environment:

```text
Γ_g -> Γ_{g+1}
```

So reflective mutation is staged:

```text
run current system:
  [[x]]_{Γ_g}(s_g, Σ_g) -> patch p

validate and commit:
  Commit(Σ_g, Γ_g, p) -> (Σ_{g+1}, Γ_{g+1})

future runs use:
  [[x]]_{Γ_{g+1}}
```

This avoids the dangerous version:

```text
x changes the meaning of x while x is currently executing
```

and replaces it with:

```text
x_g may produce x_{g+1}
```

That is the cleanest way to describe your compiled-language case. The current binary analyzes, proposes source changes, rebuilds, and the next binary participates in later analysis.

So I would call this:

```text
staged reflective execution
```

or:

```text
generation-indexed self-modification
```

Not ordinary recursion.

It is recursive in the broader sense that the system’s outputs influence its future transition rules, but semantically it is a **time-indexed feedback system**, not a same-instant self-call.

## 4. The procedure environment should become explicit

Right now your framework likely has something like:

```text
x ∈ Procedures
Exec(e, x, s) = s'
```

To support this extension, add an explicit environment:

```text
Γ = procedure environment
```

where `Γ` maps procedure/protocol names to their current definitions:

```text
Γ(x) = definition of procedure x
Γ(P) = definition of protocol P
```

Then execution is interpreted relative to `Γ`:

```text
Exec_Γ(e, x, s) = s'
```

or with mutable artifacts:

```text
Exec_Γ(e, x, s, Σ) = (s', Σ')
```

or, better, with patch proposal:

```text
K_Γ(e, x) : I_x × View_R(Σ) -> Δ(O_x × Patch)
```

This is the key formal move.

Once `Γ` is explicit, changing the analysis protocol is just changing an artifact inside `Γ`.

```text
patch p targets Γ(failure_analysis_protocol)
```

Then:

```text
Γ_{g+1} = Update(Γ_g, p)
```

A protocol can also be first-class:

```text
Protocol P = (interface, internal graph, composition rules, contracts)
```

and can be used as a procedure inside another protocol:

```text
P : I_P -> O_P
```

This matches your whole/part distinction nicely. A protocol is a composite procedure with an interface. Internally it may be many procedures; externally it can be treated as one procedure.

## 5. Three levels of mutation

I would distinguish at least three cases.

### Case 1: Object-level tool mutation

The analysis protocol diagnoses a failure in a tool and proposes a patch to that tool.

```text
diagnosis evidence
  -> tool-improvement protocol
  -> patch to code_item_search
  -> rebuilt harness
  -> eval with oracle
```

Here the analysis engine remains fixed while the tool changes.

Formal shape:

```text
Γ_g fixed
Σ_g(tool) -> Σ_{g+1}(tool)
```

This is your Branch A.

### Case 2: Meta-level analysis-protocol mutation

The tool-improvement protocol targets the code or prompt of the analysis protocol itself.

```text
diagnosis evidence
  -> analysis-protocol-improvement protocol
  -> patch to failure-analysis protocol
  -> rebuild analysis binary
  -> future analysis uses modified protocol
```

Formal shape:

```text
Γ_g(analysis_protocol) -> Γ_{g+1}(analysis_protocol)
```

This is your Branch B.

### Case 3: Trajectory-level optimization

The system repeatedly chooses whether to improve tools, improve analysis, improve the improvement protocol, or stop.

```text
Σ_0, Γ_0
  -> Σ_1, Γ_1
  -> Σ_2, Γ_2
  -> ...
```

This is no longer a single procedure. It is an adaptive policy over a versioned system trajectory.

```text
π : History_g -> Intervention_g
```

where:

```text
History_g = all prior patches, evals, oracle results, uncertainty records, branches
```

This is the extension you gestured at near the end: continuing along either branch as performance changes. That is a higher-level policy problem.

## 6. The branch model is exactly right

Your Branch A / Branch B framing is the right way to avoid muddying the signal.

I would write it like this.

```text
Branch A: tool-targeted intervention

1. Run analysis protocol A_g on eval traces.
2. Identify target tool t.
3. Run tool-improvement protocol on t.
4. Commit candidate tool patch to branch b_A.
5. Rebuild harness.
6. Run evals.
7. Score with oracle O.
```

```text
Branch B: analysis-targeted intervention

1. Run improvement protocol on analysis protocol A_g.
2. Produce candidate analysis protocol A_{g+1}.
3. Rebuild analysis binary.
4. Use A_{g+1} to perform the same type of tool diagnosis/improvement loop.
5. Rebuild harness.
6. Run evals.
7. Score with oracle O.
```

The crucial thing is that Branch B should not be allowed to judge itself using only its own revised analysis protocol.

You need at least one fixed evaluator outside the self-modifying loop:

```text
O = external oracle / MBE / benchmark validation layer
```

Then Branch B can change the analysis engine, but it does not get to redefine success.

That gives you:

```text
mutable analysis
fixed promotion criterion
```

Without that, the system can “improve” by changing its own standards.

## 7. Add a frozen reference judge

For practical correctness, I would keep a frozen baseline analysis protocol around.

```text
A_0 = frozen baseline analysis protocol
A_g = current candidate analysis protocol
O   = external oracle
```

Then candidate analysis improvements can be evaluated in several ways:

```text
A_0's diagnosis quality
A_g's diagnosis quality
downstream tool patches produced by A_0
downstream tool patches produced by A_g
oracle score after applying those patches
```

This lets you distinguish:

```text
the new analysis protocol sounds better
```

from:

```text
the new analysis protocol causes better tool patches
```

Those are different claims.

The more important metric is probably not:

```text
Did the analysis protocol produce a more plausible explanation?
```

but:

```text
Did the analysis protocol cause the tool-improvement protocol to produce patches that improve oracle-measured outcomes?
```

So the analysis protocol is evaluated partly by its **downstream causal usefulness**.

## 8. Use intervention notation

A useful framing here is causal rather than merely functional.

Branch A is an intervention:

```text
do(Σ.tool[code_item_search] := patched_version)
```

Branch B is a different intervention:

```text
do(Γ.analysis_protocol := patched_analysis_protocol)
```

Then compare outcomes:

```text
Y_A = oracle outcome under Branch A
Y_B = oracle outcome under Branch B
```

The thing you care about is:

```text
Δ = Y_B - Y_A
```

or, if comparing against baseline:

```text
Δ_A = Y_A - Y_baseline
Δ_B = Y_B - Y_baseline
```

Because patch generation and eval execution may be noisy, you probably want a posterior over improvement:

```text
P(Δ_B > Δ_A | observed evals)
```

rather than a single point estimate.

That connects directly to the probabilistic-programming extension from before.

## 9. Patch generation should be treated as a stochastic kernel

For tool improvement:

```text
K_tool_improve :
  DiagnosisEvidence × TargetSpec × ArtifactState
    -> Δ(PatchProposal)
```

For analysis-protocol improvement:

```text
K_analysis_improve :
  MetaEvidence × TargetSpec × ProcedureEnvironment
    -> Δ(PatchProposal)
```

The output is not “the patch.” It is a distribution over candidate patches, plus an uncertainty payload.

```text
patch_candidate = {
  patch,
  target,
  intended_effect,
  predicted_risk,
  predicted_improvement,
  evidence_used,
  uncertainty,
  provenance
}
```

Then validation converts that proposal into a committed artifact state only if it passes gates:

```text
candidate patch
  -> apply in branch
  -> build
  -> static checks
  -> contract checks
  -> local tests
  -> evals
  -> oracle score
  -> promote/reject
```

So the mutating procedure itself can remain probabilistic and uncertain, while the commit layer is strict and auditable.

## 10. You need a versioned configuration object

I would add something like this to the framework:

```text
Configuration State

Σ_g = (
  Γ_g,      procedure/protocol environment
  A_g,      mutable artifacts
  B_g,      build/runtime artifacts
  V_g,      version graph / branches / snapshots
  R_g,      evaluation records
  L_g       provenance ledger
)
```

Where:

```text
Γ_g = definitions of procedures and protocols at generation g
A_g = source artifacts, prompts, tool descriptions, schemas
B_g = compiled binaries, generated code, lockfiles, build products
V_g = branch graph, commit hashes, snapshots
R_g = eval results, oracle results, repeated-run measurements
L_g = full provenance and uncertainty records
```

Then a mutation-capable protocol has the shape:

```text
K_Γ(P) : I_P × View_R(Σ_g) -> Δ(O_P × PatchSet × U_P)
```

where:

```text
I_P       = admissible input/evidence state
View_R    = read-bounded view into configuration state
O_P       = ordinary output state
PatchSet  = proposed changes to artifacts
U_P       = uncertainty/risk payload
```

Commit is separate:

```text
Commit : Σ_g × PatchSet × ValidationResult -> Σ_{g+1}
```

Restore is also first-class:

```text
Restore : Σ_g × SnapshotRef -> Σ_k
```

or, with branches:

```text
Checkout : V_g × BranchRef -> Σ_branch
```

This gives you the practical operations you mentioned without contaminating the abstract procedure semantics.

## 11. Minimal practical operations to include

For implementation, I would define these as primitive artifact-state operations:

```text
Snapshot(Σ) -> κ
```

Produces a restorable checkpoint.

```text
Fork(Σ, branch_name) -> Σ_b
```

Creates an isolated branch for candidate modifications.

```text
View(Σ, read_scope) -> v
```

Returns only the admissible part of the artifact state.

```text
Diff(Σ_old, Σ_new) -> d
```

Computes the actual change footprint.

```text
ApplyPatch(Σ_b, p) -> Σ_b'
```

Applies a proposed patch in a branch.

```text
Validate(Σ_b', policy) -> validation_result
```

Checks syntax, build, tests, contracts, allowed write regions, and possibly evals.

```text
Restore(κ) -> Σ
```

Restores a previous state.

```text
Promote(Σ_main, Σ_b') -> Σ_main'
```

Promotes a validated branch into the main candidate line.

```text
Reject(Σ_b') -> archived_failure_record
```

Rejects the branch but preserves evidence.

These are practical operations, but they also have clean formal roles.

In the theory, they are artifact-state transitions. In implementation, they can correspond to git commits, worktrees, lockfiles, build artifacts, and eval result records.

## 12. Validity should be layered

“Check validity of edits” should not be one predicate. It should be a stack.

For a tool-description edit:

```text
valid_description_patch(p)
```

might require:

```text
patch touches only the target description
schema remains valid
tool name and argument schema are unchanged
description parses under expected serialization format
no forbidden instructions are introduced
eval harness still builds
```

For a tool-implementation edit:

```text
valid_tool_patch(p)
```

might require:

```text
patch touches only allowed source files
public interface remains compatible
unit tests pass
integration tests pass
tool output contract remains valid
no new unauthorized side effects
```

For an analysis-protocol edit:

```text
valid_analysis_patch(p)
```

might require:

```text
protocol interface remains compatible
output schema remains valid
uncertainty/provenance fields are preserved
old benchmark cases still run
new protocol cannot modify the oracle
new protocol cannot alter promotion criteria
```

The last two are especially important.

A self-improving analysis system should not be able to patch:

```text
oracle
held-out benchmark
promotion threshold
logging/provenance layer
branch comparison code
```

unless you are intentionally running a higher-level protocol whose target is those objects.

## 13. The recursion issue becomes manageable with phase barriers

The subtle issue is:

> The external state being modified has some impact on the next eval because it is part of the harness being used for those evals.

That is real. I would describe it as a **feedback loop through the configuration state**.

But you can keep it clean by imposing phase barriers:

```text
Phase 1: observe
Phase 2: diagnose
Phase 3: propose patch
Phase 4: apply in isolated branch
Phase 5: validate
Phase 6: evaluate with frozen oracle
Phase 7: promote/reject
Phase 8: next generation begins
```

A candidate patch cannot affect the evidence used to justify itself inside the same phase unless explicitly allowed.

So:

```text
Σ_g is frozen while generating diagnosis and patch proposal.
Σ_{g+1} exists only after commit.
```

This prevents the system from chasing a moving target inside one run.

For reflective changes:

```text
Γ_g is frozen while evaluating/proposing Γ_{g+1}.
Γ_{g+1} only takes effect after rebuild and generation advance.
```

That is the key correctness principle.

## 14. Suggested formal section: mutable artifact semantics

You could add something like this to your framework.

```text
## Mutable Artifact Semantics

Let Σ denote the mutable configuration state of the system. Σ contains
artifact definitions, procedure environments, build artifacts, version history,
evaluation records, and provenance logs.

A procedure x may be read-only or mutation-capable.

A read-only procedure has semantics:

  K_Γ(e,x) : I_x × View_R(Σ) -> Δ(O_x)

A mutation-capable procedure has semantics:

  K_Γ(e,x) : I_x × View_R(Σ) -> Δ(O_x × PatchSet × U_x)

where Γ is the current procedure/protocol environment, View_R(Σ) is the
read-bounded portion of the configuration state, PatchSet is a set of proposed
artifact modifications, and U_x is an uncertainty/risk payload.

Procedures do not directly mutate Σ. Proposed patches are applied only by a
transactional commit layer:

  Commit : Σ × PatchSet × ValidationResult -> Σ'

A patch p is admissible for target T only if:

  WriteSet(p) ⊆ AllowedRegion(T)
  and p preserves the declared interface and artifact contract
  and p passes the validation policy for T.
```

Then add staged reflectivity:

```text
## Staged Reflective Execution

Let Γ_g be the procedure environment at generation g.

Execution at generation g is interpreted only relative to Γ_g:

  [[x]]_{Γ_g}

A procedure may propose patches to artifacts that define Γ itself. Such patches
do not alter the semantics of the currently executing generation. If validated
and committed, they produce a future environment:

  Γ_g -> Γ_{g+1}

Future executions are interpreted relative to Γ_{g+1}.

This permits self-modification in the generational sense:

  x_g may produce x_{g+1}

while avoiding same-instant mutation of the executing procedure.
```

That is probably the cleanest theoretical addition.

## 15. Suggested formal section: protocols as procedures

Since you are using “protocol” to mean a logical unit composed of procedures, I would make that first-class.

```text
Protocol P = (
  I_P,
  O_P,
  G_P,
  C_P,
  R_P,
  W_P,
  V_P
)
```

where:

```text
I_P = admissible input state type
O_P = admissible output state type
G_P = internal procedure graph
C_P = composition/merge rules
R_P = read footprint over Σ
W_P = possible write footprint over Σ
V_P = validation and return policy
```

Then:

```text
[[P]]_Γ : I_P × View_R(Σ) -> Δ(O_P × PatchSet × U_P)
```

A protocol can be treated as a procedure by another protocol as long as its interface is known:

```text
P ∈ Procedures_Γ
```

This lets you maintain your whole/part distinction without needing two unrelated semantic categories.

## 16. Branch comparison under uncertainty

For Branch A and Branch B, you probably want to avoid comparing single runs.

The structure should be:

```text
candidate branch b
  -> run eval tasks
  -> collect oracle outcomes
  -> estimate performance distribution
```

So each branch has a performance posterior:

```text
Y_A ~ posterior performance of Branch A
Y_B ~ posterior performance of Branch B
```

Then compare:

```text
P(Y_B > Y_A + ε | eval data)
```

where `ε` is a practically meaningful improvement threshold.

This matters because a branch may look better due to noise:

```text
lucky patch generation
lucky eval sampling
lucky nondeterministic agent behavior
different trace distribution
LLM adjudication variance
```

A robust promotion rule might be:

```text
Promote Branch B only if:
  P(Y_B > Y_baseline + ε) ≥ τ
  and regression risk ≤ ρ
  and validation contracts pass
```

For example:

```text
τ = 0.95
ρ = 0.05
```

The exact values are policy choices, not semantic necessities.

## 17. To evaluate the analysis protocol, measure downstream causal effect

For the analysis protocol, the most important question is not:

```text
Does the new analysis protocol produce nicer diagnoses?
```

It is:

```text
Does the new analysis protocol cause better interventions?
```

So define something like:

```text
Utility(A_g) =
  expected oracle improvement of patches produced downstream
  when using analysis protocol A_g
```

More concretely:

```text
A_g analyzes failed traces
  -> identifies target and diagnosis
  -> tool-improvement protocol proposes patch
  -> patch is validated
  -> oracle scores outcome
```

Then:

```text
A_{g+1} is better than A_g
```

only if the whole downstream chain improves, or if it improves some validated intermediate metric that you trust.

This avoids optimizing for plausible explanations that do not actually lead to better patches.

## 18. Crossed evaluation can help separate causes

If you want to diagnose whether improvement came from better analysis or better tool improvement, you can use a crossed design.

Let:

```text
A_0 = old analysis protocol
A_1 = new analysis protocol
I_0 = old tool-improvement protocol
I_1 = new tool-improvement protocol
```

Then evaluate combinations:

```text
A_0 + I_0
A_1 + I_0
A_0 + I_1
A_1 + I_1
```

This helps distinguish:

```text
analysis got better
tool-improvement got better
the combination got better
the apparent gain was interaction-dependent
```

You do not need this for every iteration, but it is useful when trying to attribute changes in performance.

## 19. The next extension is an adaptive trajectory semantics

The final thing you describe — continuing along either branch as analysis and improvement protocols change — is a higher-level extension.

At that point, the system is not just executing a protocol. It is following an adaptive improvement policy.

```text
Π : History_g -> NextIntervention
```

where:

```text
History_g = {
  prior generations,
  branches,
  patches,
  eval outcomes,
  uncertainty estimates,
  rejected candidates,
  accepted candidates,
  oracle results
}
```

Then a trajectory is:

```text
τ = (Σ_0, Γ_0)
    -> (Σ_1, Γ_1)
    -> (Σ_2, Γ_2)
    -> ...
```

The object of evaluation becomes the whole trajectory or policy:

```text
Utility(τ)
```

not just one patch.

This is where you might later define things like:

```text
expected improvement rate
risk-adjusted improvement
probability of regression
expected value of another analysis-improvement step
expected value of another tool-improvement step
```

But I would not put all of that into the first mutable-state extension. First add artifact state, staged reflectivity, bounded patches, and branch validation. Then add trajectory policies later.

## 20. The main conceptual recommendation

I would frame your extension around four ideas:

```text
1. Artifact state
2. Patch proposals
3. Transactional validation/commit
4. Generation-indexed reflective execution
```

The central semantic shape becomes:

```text
K_Γ(P) : I_P × View_R(Σ_g) -> Δ(O_P × PatchSet × U_P)
```

followed by:

```text
Σ_{g+1}, Γ_{g+1}
  =
Commit(Σ_g, Γ_g, PatchSet, ValidationResult)
```

And the crucial staging rule is:

```text
A generation may modify future generations, not itself-in-the-same-run.
```

That one rule handles the apparent recursion.

So your framework does not need to say:

> A function modifies itself.

It can say:

> A procedure, interpreted under environment `Γ_g`, may use admissible evidence to propose bounded modifications to artifacts, including artifacts that define the future procedure environment `Γ_{g+1}`. These modifications are isolated, validated, and committed only through a versioned artifact-state transition. Future evaluations are then performed under the new generation.

That gives you the general theoretical form, while still supporting practical requirements like restore, branch, validate, rebuild, compare, and promote.
