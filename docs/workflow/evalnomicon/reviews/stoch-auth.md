
Re:
prototype1_state/mod.rs
prototype1_state/history.rs
../chat-history/prototype-1-probability.md
../chat-history/framework-ext-01.md

## Prompt
This is a system for a multi-generation self-improving agent harness. I would like you to review the logic, provide critiques, and try to tie it all together into an even more coherent model and provide likely extension points or areas to carefully attend to regarding both the conceptual modeling of the entities involved as well as the implementation details that will need to be upheld, such as invariants, expected vs. realistic capabilities, possible points of implementation that may invalidate the claims in the overall conceptual model, etc. 
I would very much like a general all-around review. In addition, I would like some special consideration regarding the composability of probabalistic operations over time, and how that affects any claims being made. My primary concern is that there is some hidden or lurking compounding of uncertainty that fundementally undermines the architecture. I think there are some issues regarding long-term feasability, and the goal is for this program to be capable of continuing to improve over an arbitrarily large number of generations on an arbitrarily wide set of concurrently executing runtimes that are tied together via a blockchain protocol.

## Comments on following Review

1. PolicyRef vs ProcedureRef
   The reviewer treats policy and procedure environment as cleanly distinct. Conceptually, yes.
   But your correction still matters: current ProcedureRef is closer to a procedural environment
   or runtime contract than “one procedure.” I’d split eventually into:

   ProcedureEnvironmentRef / RuntimeContractRef
   PolicyRef
   but I would avoid pretending the current ProcedureRef was just a function name.
   but I would avoid pretending the current ProcedureRef was just a function name.
2. “Runtime cannot be evaluated externally”
   The reviewer is right to push back if any doc says this too strongly. The better claim is:

   Some internal metrics can only be produced by running that Runtime,
   but external validators can hydrate the Artifact/Runtime and produce fresh observations.

   That distinction is central to the reputation model.

      > re: 2: “Runtime cannot be evaluated externally” 
      > This is a fair point,
      > but I think we do still want to clearly model the uncertainty on this more
      > broadly. Even if there is a provable equivalence via the underlying code
      > base (which, that's a big claim already we'd need to back up with formal
      > verification), we'd still want to have an element in our probability
      > distribution analysis that models this, because we absolutely do want to
      > include cases where the self-analysis protocol itself undergoes
      > modification and is a valid target for the bounded surface over which a
      > runtime may produce a patch on a target artifact.

      > For the theorem-bearing structures, we should preserve room for:
      >   evaluation_origin
      >   equivalence_basis
      >   equivalence_strength
      >   protocol_drift_risk
      >   policy class for evaluator/procedure changes

3. Extension-point names
   The proposed CandidatePromotionEvidence, PromotionDecision, RiskLedger, etc. are useful
   semantic objects, but I would not directly implement those names as big structs yet. They risk
   becoming another report hierarchy. I’d use them as conceptual groupings and then encode them
   structurally around Entry, block content, policy, and evaluation samples.
4. “Close startup/handoff first”
   The review recommends startup/handoff first. I mostly agree, but our current conversation
   showed why the block contents/policy shape needs one more design pass first. Otherwise we wire
   a sealed block that immediately becomes too thin.

## Overall assessment

Your architecture is strongest when read as **an authority-and-evidence system around stochastic self-improvement**, not as a deterministic chain that proves each generation is objectively better than the last. That framing is already present in the notes: deterministic commitments cover identity, custody, hashes, artifacts, procedure refs, and authority; stochastic evidence covers patch generation, LLM adjudication, self-evaluation, oracle-backed scoring, branch comparison, and validator re-evaluation. The block is where those layers meet, but it should claim “this authority path admitted this artifact with these evidence samples,” not “this artifact is definitely an improvement.” 

The central risk you are worried about is real: **with an arbitrarily long horizon, any nonzero per-generation probability of bad promotion, false validation, evaluator capture, or corrupted authority eventually dominates unless the system explicitly models and budgets that risk**. The architecture is not fundamentally doomed by probabilistic composition, but it must stop treating probability as an annotation and start treating it as a first-class state variable carried through History, selection, validation, reputation, and consensus.

My strongest recommendation is to make the system’s main claim this:

> A generation-indexed runtime can propose bounded modifications to artifact-defined future procedure environments. Those proposals are committed only through typed authority transitions, validated against external or policy-frozen evaluators, recorded in tamper-evident History, and compared using explicitly modeled uncertainty and reputation-weighted independent evidence.

That is coherent. A stronger claim like “the chain can keep improving indefinitely over arbitrary generations and arbitrary runtimes” is not yet justified.

---

## The coherent model

The clean conceptual model is:

```text
Σ_g = configuration state at generation g
Γ_g = procedure/protocol environment at generation g
A_g = active Artifact
R_g = Runtime hydrated from A_g
H_g = sealed/admitted History state
π_g = selection/admission policy
```

A mutation-capable protocol should be modeled as a stochastic kernel:

```text
K_Γ(P) : I_P × View_R(Σ_g) -> Δ(O_P × PatchSet × U_P)
```

where `U_P` is not decorative; it is the uncertainty/risk payload that later selection and reputation policy must consume. The proposed patch should not directly mutate `Σ`; it should go through a transactional commit layer:

```text
Commit : Σ_g × PatchSet × ValidationResult -> Σ_{g+1}
```

This matches the extension draft’s distinction between read-only procedures, mutation-capable procedures, patch proposals, validation, and generation-staged reflective execution. 

The important staging rule is:

```text
Γ_g is frozen during generation g.
A patch may produce Γ_{g+1}.
It must not alter the semantics of the procedure currently judging itself.
```

That is the move that prevents semantic self-reference from collapsing the model. A runtime may produce a successor runtime; a procedure may produce a future version of its procedure environment; but it does not change the meaning of itself mid-run.

The object graph should not be “one git tree.” The notes correctly move toward three related graphs: an artifact graph, a runtime derivation graph, and an operation graph. The operation coordinate is not merely “artifact A produced artifact B”; it is “runtime R, hydrated from artifact A or another artifact, operated over target artifact T and produced patch attempt P.” That distinction is essential once cross-lineage operations are allowed. 

A compact full-cycle model would be:

```text
1. Observe evidence under Γ_g and Σ_g.
2. Select target Artifact/Protocol region T.
3. Generate PatchAttempt:
      (generator Runtime, target Artifact, attempt id) -> PatchAttempt
4. Apply patch in isolated branch/worktree.
5. Build/hydrate candidate Runtime.
6. Evaluate candidate under declared Γ_eval, Oracle, policy, environment.
7. Produce EvaluationSample records.
8. Select successor under uncertainty-aware policy.
9. Seal History block for the Crown epoch.
10. Successor verifies sealed predecessor block.
11. Successor becomes Parent<Ruling> for the next lineage epoch.
```

This ties together Artifact, Runtime, Procedure Environment, Crown, History, evaluation, and stochastic improvement without pretending the process is deterministic.

---

## Core invariants to uphold

The design already names many of the right invariants: no global active parent assumption, no singleton “current best branch,” no overwritten evaluation state, no score without evaluator/eval-set/policy identity, no analysis engine in the trusted root, no self-report promotion without independent verification, no coupling of worktree layout to semantic identity, and no successor authority implying global authority. 

I would sharpen them into these implementation-level invariants.

| Area                         | Required invariant                                                                                                                                              | What breaks if violated                                                                           |
| ---------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------- |
| **Authority**                | For one lineage, at most one valid carrier holds `Crown<Ruling>` at a time.                                                                                     | Two runtimes can both mutate the same lineage and produce conflicting “authoritative” successors. |
| **Startup**                  | A runtime may enter `Parent<Ruling>` only after proving `ProducedBy(SelfRuntime, CurrentArtifact)` and `AdmittedBy(CurrentArtifact, Lineage, Policy, History)`. | Any binary can claim parent status by showing plausible local files.                              |
| **Artifact identity**        | Worktree path, branch name, and scheduler node are handles, not semantic identity. Durable records must carry tree key / commit / manifest digest.              | Later graph traversal confuses filesystem layout with artifact lineage.                           |
| **Patch boundary**           | A patch’s write set must be inside its target’s allowed region and must preserve target contracts.                                                              | “Improvement” can come from editing the evaluator, oracle, promotion policy, or provenance layer. |
| **Procedure staging**        | `Γ_g` is frozen during generation `g`; `Γ_{g+1}` only takes effect after validation and generation advance.                                                     | The system can judge itself using rules it just rewrote.                                          |
| **Evaluation comparability** | Every score must name evaluated artifact/runtime, evaluator, eval set, policy, oracle, procedure version, model/runtime environment, and sample identity.       | Scores become non-comparable anecdotes.                                                           |
| **History authority**        | Scheduler files, branch registries, reports, invocation files, and monitor outputs are evidence/projections until admitted into sealed History.                 | Mutable operational files become de facto consensus.                                              |
| **Self-report**              | Child/runtime self-evaluation may be evidence, but not promotion.                                                                                               | A runtime can improve by becoming a better liar.                                                  |
| **Append-only evidence**     | Observations, decisions, failures, and rejected attempts are appended, not overwritten.                                                                         | Selection bias and recovery failure become invisible.                                             |
| **Lineage locality**         | Crown, height, parent block hash, and successor authority are lineage-scoped, not global.                                                                       | Multi-parent concurrency collapses into accidental singleton semantics.                           |

The History notes are already careful here: the Crown invariant is about ruling authority, not process uniqueness; multiple runtimes may execute, but that is not the same as multiple rulers. The notes also explicitly state that filesystem scheduler/report artifacts are not History authority until admitted or imported under policy. 

---

## The probability problem: your concern is valid

The hidden compounding issue is the main thing to attend to.

Suppose each generation has probability `q` of promoting a candidate that should not have been promoted: bad patch, overfit patch, compromised evaluator, false-positive benchmark result, malformed provenance, or authority bug. If `q` is constant and greater than zero, then after `N` generations:

```text
P(at least one bad promotion) = 1 - (1 - q)^N
```

As `N -> ∞`, that probability approaches `1`.

So for an arbitrarily long self-improvement chain, a fixed local failure probability is not acceptable. The architecture must either:

```text
1. weaken the claim,
2. make failure recoverable,
3. reduce per-generation catastrophic risk under a summable budget,
4. or add independent consensus/validation that catches failures before they become final.
```

For infinite-horizon claims, the relevant bound is closer to:

```text
P(any catastrophic false admission ever) ≤ Σ_g q_g
```

That means the risk schedule must be summable. For example:

```text
q_g = q0 / 2^(g+1)
```

gives:

```text
Σ_g q_g ≤ q0
```

But this has a practical cost: as the permitted false-admission probability shrinks, validation usually becomes more expensive. That is not a reason to abandon the architecture; it is a reason to make risk budget, validation cost, and finality policy explicit.

### The biggest probabilistic failure modes

**1. Winner’s curse from fan-out.**
If each generation creates many child candidates and selects the best observed score, the selected score is biased upward. With many candidates, at least one will look good by chance. A single branch-vs-baseline comparison is not enough.

**2. Adaptive testing / optional stopping.**
If the system keeps sampling until something passes, ordinary confidence thresholds are invalid. Promotion policy must record the number of attempts, stopped runs, rejected candidates, reused eval sets, and adaptive decision path.

**3. Correlated validators.**
“Ten validators” is not ten independent samples if they share the same base model, prompt, eval set, provider, harness bug, lineage, or oracle. Reputation should model dependence, not just count validators.

**4. Evaluation drift.**
A score from `Γ_eval_7` may not be comparable to a score from `Γ_eval_19`. The notes already say evaluation results are samples induced by `(Artifact, Γ_eval, Oracle, operational environment)` rather than deterministic facts; that needs to become an enforced record schema. 

**5. Benchmark overfitting.**
A long-running self-improver will eventually optimize against the evaluator if the evaluator is reused and visible. You need sealed holdouts, rotating evals, adversarial eval refresh, and frozen promotion policies that candidate generations cannot patch.

**6. Self-reinforcing reputation.**
If validators gain reputation by agreeing with high-reputation validators, reputation becomes social proof rather than calibration. Reputation updates should reward predictive accuracy against later independent evidence, not mere agreement.

**7. Miscalibrated uncertainty payloads.**
LLM-generated “uncertainty” is not calibrated by default. `U_P` should be treated as a claim to be scored later. A runtime should gain reputation when its uncertainty estimates are well-calibrated, not merely when its patches occasionally win.

---

## How to make probabilistic composition safe enough

You need a first-class **Risk Ledger** and **Evaluation Evidence Model**.

A candidate promotion record should include something like:

```text
CandidatePromotionEvidence {
  candidate_artifact,
  baseline_artifact,
  generator_runtime,
  target_artifact,
  patch_attempt_id,

  eval_policy_ref,
  oracle_ref,
  eval_set_ref,
  procedure_environment_ref,
  operational_environment_ref,

  sample_count,
  candidate_count_this_selection_round,
  prior_attempts_on_same_target,
  adaptive_stopping_rule,
  heldout_status,

  posterior_effect_distribution,
  posterior_probability_improvement,
  posterior_regression_risk,
  practical_effect_threshold_epsilon,

  uncertainty_budget_spent,
  validator_attestations,
  reputation_weights,

  rejected_sibling_refs,
  failure_refs,
}
```

Promotion should not be:

```text
score(candidate) > score(baseline)
```

It should be closer to:

```text
Promote c over b only if:

P(θ_c > θ_b + ε | D) ≥ τ_g

and

P(θ_c < θ_b - r | D) ≤ ρ_g

and

validation_contracts_pass(c)

and

risk_budget_remaining(lineage, policy, generation) is sufficient

and

candidate did not modify evaluator/oracle/promotion/provenance surfaces
unless explicitly authorized by a higher-level policy.
```

Where:

```text
ε = minimum meaningful improvement
r = maximum tolerated regression
τ_g = generation-specific confidence threshold
ρ_g = generation-specific risk threshold
D = all recorded evidence, including failed/rejected attempts
```

The extension draft already points in this direction with posterior branch comparison:

```text
P(Y_B > Y_A + ε | eval data)
```

and with explicit branch performance posteriors rather than single-run comparisons. 

For long horizons, use one of these policies:

```text
Conservative infinite-horizon policy:
  Σ_g ρ_g ≤ total_regression_budget
  Σ_g catastrophic_risk_g ≤ total_catastrophic_budget

Pragmatic recoverable policy:
  allow bounded local regressions
  require checkpoint/rollback
  increase validation after regression
  never allow unreviewed mutations to trusted roots

Exploratory policy:
  accept higher local risk in non-authoritative branches
  require stricter evidence before lineage-head promotion
```

This distinction matters. You can tolerate noisy exploration in child branches. You cannot tolerate the same error rate at the lineage-head admission boundary.

---

## Reputation and validator modeling

The reputation idea is good, but it must be framed as **calibration under later evidence**, not as generic trust.

A useful model is:

```text
θ_c = latent quality of candidate c
b_r = bias of reporter/validator r
σ_r = noise of reporter/validator r
κ_r = calibration quality of reporter/validator r
```

A reporter’s claim is not merely:

```text
R8 says C9 scored 0.82
```

It should be:

```text
R8 claims:
  under eval policy E,
  oracle O,
  procedure environment Γ,
  operational environment Ω,
  candidate C9 has posterior performance distribution Q_R8.
```

Later validators produce additional samples. Then reputation can update by asking:

```text
How well did R8's predictive distribution anticipate later independent samples?
```

The pasted notes already articulate this as “made claims that survive independent validation,” which is the right principle. 

Use proper scoring rules where possible:

```text
reputation_update ∝ log p_R8(later_validation_samples)
```

or a bounded variant to avoid catastrophic punishment from one noisy disagreement.

Also separate at least four reputations:

```text
1. producer_quality: tends to generate good candidates
2. reporter_calibration: makes accurate claims about candidates
3. validator_quality: produces reliable validation samples
4. policy_quality: selects successors that survive later validation
```

Do not collapse these into one “agent score.” A runtime can be a good patch generator and a poor evaluator, or a conservative validator and a weak generator.

---

## Blockchain / History model

The blockchain analogy is useful only if it stays precise.

A block should not be “proof that improvement happened.” It should be a tamper-evident authority epoch that commits to:

```text
lineage/store scope
opening authority
ruling authority
active artifact
selected successor artifact/runtime
procedure/policy refs
entries root
evidence refs
evaluation sample refs
uncertainty summary
validator attestations
parent block hashes
block hash
```

The pasted notes’ proposed term “ProofOfEvaluatedProgress” is better than “proof of improvement,” because the claim is about evaluated evidence under declared conditions, not about objective final truth. 

For concurrency, the key is that History must be:

```text
global authenticated substrate
over lineage-local authority chains
```

not:

```text
one global linear chain whose height defines all lineages
```

That distinction is already in `mod.rs`. 

To support arbitrary concurrent runtimes, you will need more than the current local History store:

```text
AuthenticatedHeadMap:
  lineage_id -> current sealed head

Required proofs:
  inclusion proof for existing head
  absence proof for genesis admission
  compare-and-swap proof for head transition
  fork/conflict evidence
  validator signatures
  finality policy
```

The current files correctly say the filesystem `heads.json` projection is not such a proof and that cryptographic signatures/distributed consensus are not implemented. 

So the safe current claim is:

> Prototype 1 is moving toward tamper-evident, lineage-scoped, transition-checked local History.

The unsafe current claim would be:

> Prototype 1 has blockchain consensus or globally trustworthy proof of improvement.

---

## Implementation review

### What is strong

The strongest implementation direction is the typestate/transition discipline. The History module defines state carriers like `Block<Open>`, `Block<Sealed>`, `Entry<Draft>`, `Entry<Observed>`, `Entry<Proposed>`, `Entry<Admitted>`, and ingress states; it also keeps authoritative carriers serializable but not trivially deserializable, so verified loading must become an explicit transition. 

The `Crown<Locked> -> Block<Sealed>` boundary is also the right shape. Sealing requires a locked Crown carrier with matching lineage, and tests cover mismatched lineage rejection and deterministic block hashes. 

The code is also moving in the right direction by making `TreeKeyHash` a commitment derived from backend-owned tree keys rather than accepting caller-authored strings as the artifact identity witness. 

### Main implementation gaps

The current implementation does not yet enforce the full startup/handoff model. The History notes say `Parent<Ruling>` is not yet gated by `Startup<Validated>`, live handoff does not yet seal or persist a History block, incoming runtimes do not yet validate through History before entering the parent path, gen0 does not yet append an explicit genesis block, and whole-artifact/runtime/build identities still need canonical refs. 

That gap is critical. Until it is closed, the conceptual Crown/History model is ahead of the live authority path.

The `FsBlockStore` is also still a prototype store. Its `append` verifies the block hash, appends JSONL records, updates indexes, reads `heads.json`, inserts the new head, and writes it back. That is useful locally, but it is not an atomic compare-and-swap lineage-head transition, not an authenticated map, and not safe as distributed consensus. 

The string-ref constructors are another subtle issue. `ProcedureRef`, `EvidenceRef`, and `ArtifactRef` currently accept arbitrary strings via `new(...)`, while `TreeKeyHash` is more carefully derived from backend-owned keys. That is fine for scaffolding, but conceptually a ref is not evidence. A future `ArtifactRef` should either be a verified digest/manifest/tree key or carry degraded-provenance status explicitly. 

The record sprawl is a real implementation risk. `mod.rs` lists scheduler files, branch registries, evaluation summaries, transition journals, invocation files, result files, successor-ready files, completion files, worktrees, build dirs, and parent identity files. It also explicitly says current persistence is enough to recover individual node outcomes but not enough for safe fan-out, and that cleanup should not add another parallel status document. 

### A conceptual wording problem: “Runtime cannot be evaluated externally”

The `Runtime` description says a runtime cannot be evaluated externally and must itself produce the record containing metrics used to evaluate it, because metrics are of a procedure for which the runtime is the operational environment. 

I would weaken or split that claim.

A better formulation:

```text
A Runtime may be the only environment that can produce certain internal
procedure metrics.

But external validators can still evaluate a Runtime/Artifact by hydrating it,
running declared tasks under a declared oracle/eval policy, and recording
fresh external observations.

Self-produced metrics are evidence.
They are not promotion authority without independent validation.
```

This matters because your reputation and blockchain model depends on independent validators. If “external evaluation” is ruled out too literally, the system collapses back into self-report.

---

## Where implementation could invalidate the conceptual model

These are the places I would audit aggressively.

### 1. Startup admission

If a successor can enter parent mode by consulting scheduler/invocation state rather than sealed History, the Crown model is mostly ceremonial.

Required gate:

```text
derive clean TreeKey from active checkout
verify artifact-carried parent identity
load authenticated lineage head
verify sealed predecessor block
verify selected successor artifact/runtime
verify policy decision and evidence refs
only then enter Parent<Ruling>
```

### 2. Head advancement

If appending a block does not atomically prove the previous head and install the next head, concurrent parents can overwrite or fork silently.

Required operation:

```text
append_if_head(lineage, expected_parent_head, sealed_block) -> new_head | conflict
```

For the blockchain version, this becomes a consensus transaction.

### 3. Artifact identity

If branch names, worktree paths, text-file fallback IDs, or scheduler node IDs stand in for artifact identity, later validators cannot reliably reconstruct what was evaluated.

The notes already identify this: dirty worktrees are provisional, and durable Artifacts should carry a provenance manifest committed by the tree and admitted by History. 

### 4. Evaluator mutability

If a candidate can patch the oracle, eval harness, promotion threshold, benchmark set, logging/provenance code, or History admission policy, then “improvement” can mean “changed the definition of winning.”

Reflective mutation is fine only if target regions are explicit:

```text
ordinary improvement:
  may patch tool/prompt/analysis region
  may not patch oracle/promotion/history root

meta-governance improvement:
  may patch oracle/promotion/history root
  requires higher-level policy, stronger validation, and probably human or consensus approval
```

### 5. Selective reporting

If rejected candidates, failed builds, partial evals, aborted runs, or timeout failures are omitted from History/evidence, all posterior estimates become biased.

The system must preserve rejected sibling attempts, not just winners.

### 6. Validator independence

If validators reuse the same hidden bug or model family, the reputation layer will overcount evidence. Operational environment fields already include runtime, artifact, binary, tool surface, procedure version, model, code graph, oracle task, and recorder; those should become required for evaluation records, not optional niceties. 

### 7. Sandboxing and capability boundaries

A child runtime is untrusted code produced by the system. It needs bounded filesystem, network, process, credential, and History-store capabilities. Otherwise “child self-evaluation” can become “child tampers with its evidence.”

### 8. Resource exhaustion

The single-successor path already notes that the selected Artifact should be moved into the stable active checkout and temporary child worktrees cleaned up. 

For arbitrary fan-out, cleanup is not operational polish; it is a semantic requirement. Otherwise “arbitrary generations” fails by disk exhaustion, orphaned processes, stale locks, or unbounded evidence growth.

---

## Extension points I would add now

### 1. `ArtifactManifest`

Make durable Artifact identity a manifest, not a branch/path string.

```text
ArtifactManifest {
  artifact_id,
  backend_tree_key,
  manifest_digest,
  source_commit_or_tree,
  parent_artifact_refs,
  patch_attempt_ref,
  generator_runtime_ref,
  build_recipe_ref,
  dependency_lock_refs,
  procedure_environment_ref,
  provenance_refs,
}
```

### 2. `PatchTarget` and `PatchContract`

Patch admissibility should be target-specific.

```text
PatchTarget {
  artifact_ref,
  allowed_region,
  forbidden_regions,
  interface_contract,
  validation_policy,
  authority_required,
}
```

### 3. `EvaluationSample`

Scores should be samples, not scalar truth.

```text
EvaluationSample {
  sample_id,
  candidate_ref,
  baseline_ref,
  evaluator_ref,
  oracle_ref,
  eval_set_ref,
  policy_ref,
  procedure_environment_ref,
  operational_environment_ref,
  random_seed_or_attempt_ref,
  outcome,
  uncertainty,
  payload_hash,
}
```

### 4. `PromotionDecision`

Selection should preserve the full decision context.

```text
PromotionDecision {
  candidate_scope,
  selected_candidate,
  rejected_candidates,
  posterior_comparison,
  risk_budget_spent,
  policy_ref,
  decision_authority,
  evidence_refs,
}
```

### 5. `RiskLedger`

Track uncertainty as state.

```text
RiskLedger {
  lineage_id,
  policy_ref,
  total_budget,
  spent_budget,
  per_generation_budget,
  catastrophic_budget,
  regression_budget,
  open_risks,
}
```

### 6. `ValidatorAttestation`

For blockchain-style validation:

```text
ValidatorAttestation {
  validator_runtime_or_actor,
  candidate_artifact,
  reproduced_from_manifest,
  eval_policy,
  samples,
  signature,
  claimed_distribution,
  calibration_score_update,
}
```

### 7. `AuthenticatedHeadMap`

Replace `heads.json` authority with a map that supports:

```text
prove_present(lineage) -> head + proof
prove_absent(lineage) -> absence proof
compare_and_swap(lineage, expected_head, new_block) -> accepted | conflict
```

### 8. `ParentLease`

Because Crown/History gates artifact eligibility, not OS-process uniqueness, a separate lease/lock/consensus mechanism is needed if you want process-level uniqueness. The History notes explicitly warn about this distinction. 

---

## Safe versus unsafe claims

| Claim                                                                                      | Status                                                                                                                  |
| ------------------------------------------------------------------------------------------ | ----------------------------------------------------------------------------------------------------------------------- |
| “Procedures can be modeled as stochastic kernels over artifact state and patch proposals.” | Safe and coherent.                                                                                                      |
| “Reflective self-modification can be staged generation-to-generation.”                     | Safe if `Γ_g` is frozen and commit is separate.                                                                         |
| “History can provide tamper-evident lineage-local authority epochs.”                       | Safe as a design goal; partially implemented locally.                                                                   |
| “The current code provides distributed consensus.”                                         | Not safe. The files explicitly say signatures/consensus are not implemented.                                            |
| “A sealed block proves a runtime improved.”                                                | Not safe. It proves admitted evidence and authority transition, not objective improvement.                              |
| “Independent validation can strengthen posterior confidence.”                              | Safe if validator independence and environment identity are modeled.                                                    |
| “The system can run arbitrary generations with fixed local error probability.”             | Not safe. Infinite-horizon compounding makes failure eventually likely.                                                 |
| “The system can support arbitrary concurrent runtimes.”                                    | Safe only after lineage-local authority, authenticated head map, leases/consensus, and conflict policy are implemented. |
| “Self-reports can be evidence.”                                                            | Safe.                                                                                                                   |
| “Self-reports can drive promotion without independent validation.”                         | Not safe; your own design constraints reject it.                                                                        |

---

## Recommended near-term implementation order

I would prioritize the work in this order:

1. **Close the startup/handoff gap.**
   Implement `Startup<Observed> -> Startup<Validated> -> Parent<Ruling>` using sealed History, not scheduler/invocation state.

2. **Persist sealed blocks during live handoff.**
   The outgoing Parent should seal and append the block before the successor can validate and become ruling.

3. **Replace head mutation with compare-and-swap semantics.**
   Even locally, `append_if_head(...)` is the right shape.

4. **Introduce artifact manifests.**
   Stop relying on branch/worktree/text fallback IDs for durable identity.

5. **Make evaluation samples first-class.**
   Every evaluation should carry evaluator, eval-set, oracle, policy, procedure environment, operational environment, and sample identity.

6. **Add uncertainty-aware promotion.**
   Use posterior comparison and regression risk, not raw score deltas.

7. **Record rejected attempts.**
   This is required to correct selection bias.

8. **Add target-bounded patch contracts.**
   Make it impossible for ordinary improvement patches to alter oracle, promotion, History, or logging surfaces.

9. **Add validator/reputation machinery only after sample records are solid.**
   Reputation built on weak records will amplify noise.

10. **Only then generalize to distributed blockchain-style consensus.**
    Otherwise the blockchain layer will notarize ambiguous local records.

---

## Bottom line

The architecture is viable if its core claim is:

> This system performs stochastic, generation-staged search over artifact-defined runtimes, with typed authority transitions and tamper-evident evidence accumulation.

The architecture is not viable if its core claim is:

> Each generation is reliably better, and indefinite self-improvement follows from local evaluation and self-report.

Your uncertainty concern does not fundamentally undermine the model, but it does force a major discipline: **probability must compose through the same History machinery as authority**. Every promotion should spend uncertainty budget, every score should be a sample under a declared distribution, every validator should be calibrated against later evidence, and every long-horizon claim should be phrased in terms of bounded risk, recoverability, and posterior confidence rather than deterministic improvement.

The most important conceptual refinement is:

```text
History records authority and evidence.
Evaluation estimates performance.
Consensus/reputation weighs evidence.
Policy admits successors under bounded risk.
None of these alone proves permanent improvement.
```

That framing keeps the model coherent across many generations and many runtimes without hiding the compounding uncertainty problem.
