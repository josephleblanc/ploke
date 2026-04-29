# History Blocks v2

Recorded: 2026-04-29 10:59 PDT

Status: current conceptual anchor for Prototype 1 History block contents and
blockchain-style authority. The canonical implementation claims remain in
`crates/ploke-eval/src/cli/prototype1_state/history.rs`; this note explains the
larger model, unresolved concerns, and why the next type slice should look the
way it does.

## Core Claim

The system is not a deterministic proof of permanent improvement. It is an
authority-and-evidence system around stochastic self-improvement.

History records authority and evidence. Evaluation estimates performance.
Consensus/reputation weighs evidence. Policy admits successors under bounded
risk. None of these alone proves permanent improvement.

## Blockchain Definition

A blockchain, in this model, is not one global list of blocks. It is an
authenticated, append-only evidence/authority structure over one or more
lineage-local chains, plus policy/finality rules for deciding which heads are
accepted under a given store scope.

```text
History =
  authenticated substrate containing sealed blocks, head-state proofs,
  ingress/evidence references, and policy-scoped admission state.

Block =
  one sealed authority/evidence epoch for a lineage under policy,
  content-addressed by header material, admitted entries/evidence roots,
  artifact commitments, stochastic evidence commitments, head-state/finality
  commitments, and parent links.
```

Current implementation note: Prototype 1 does not yet implement distributed
consensus, authenticated head-map proofs, full policy/finality semantics, or
live startup admission through sealed History. Current code is a local,
partial, tamper-evident History core.

## What A Block Proves

A block should not prove:

```text
Artifact A definitely improved the system.
```

It should prove:

```text
these claims/evidence were admitted by this authority under this policy,
with these artifact/procedure/oracle commitments,
at this lineage position,
and have not changed since sealing.
```

Evaluation and validation evidence can support claims about improvement, but
those claims remain probabilistic unless backed by stronger external proof.

## Accepted Invariants

### Crown

For one configured History store and lineage, at most one valid typestate
carrier may hold mutable ruling authority. `Crown<Locked>` is the structural
carrier that may seal the open block for that lineage. This is a typed
authority invariant, not a proof that only one OS process exists.

### Artifact And Runtime

Every admitted Runtime is `ProducedBy(Artifact)`. If an Artifact produces an
admitted Runtime, it must be recoverable from the Tree at some branch, commit,
tree key, or equivalent backend coordinate. Dirty worktrees can produce
provisional runtimes, but they do not become fully admitted authority without a
recoverable identity.

### Startup

Intended startup invariant, not yet the live startup gate as of 2026-04-29
11:58 PDT: a Runtime may enter `Parent<Ruling>` only after establishing:

```text
ProducedBy(SelfRuntime, CurrentArtifact)
AdmittedBy(CurrentArtifact, Lineage, Policy, History)
```

For the intended hot path, startup should validate the immediate sealed head
and current Artifact commitment. Full History replay can remain a separate
validation procedure because block hashes and parent links support recursive
verification. Current Prototype 1 startup still uses checkout, parent identity,
scheduler, and invocation evidence; it does not yet validate sealed History
head admission.

Refined invariant, 2026-04-29 11:58 PDT:

```text
ProducedBy(A_i, R_i)
```

means Artifact `A_i` hydrates Runtime `R_i`.

For a configured History surface `H`, lineage coordinate `L`, and policy `P`,
the intended local admission rule is:

```text
MayEnterRuling(H, L, P, R_i)
  only if
∃ A_i.
  ProducedBy(A_i, R_i)
  ∧ CurrentCheckout(R_i) commits to TreeKey(A_i)
  ∧ Head(H, L, P) commits to ExpectedSuccessor(A_i)
```

Under the current single-ruler local policy, mutable History authority is
therefore mediated by the artifact named by the sealed head, not by a generic
process identity. This is the invariant the startup gate should encode. It
does not yet prove OS-process uniqueness:

```text
MayEnterRuling(H, L, P, R_i) ∧ MayEnterRuling(H, L, P, R_j)
  does not imply
ProcessId(R_i) = ProcessId(R_j)
```

Two processes can still execute the same admitted Artifact until a lease, lock,
or consensus layer is added. That process-uniqueness property is outside the
current type-state claim.

### Procedure Environment

`ProcedureRef` in the current code should be read as a reference to a procedure
environment or runtime contract: the operations, protocols, oracle bindings,
and admissible behaviors available to a Runtime built from an Artifact. It is
not merely one narrow function call.

### Stochastic Evidence

LLM-mediated patch generation, adjudication, self-evaluation, and many oracle
workflows should be modeled as stochastic unless the environment gives an
explicit reproducibility contract. Rerunning a procedure generally produces
another sample, not the same sample.

## Policy And Surface

`PolicyRef` and `PolicyScope` should be defined through `Surface`. The policy
question is not only "which rule was used?" but "over which bounded surface does
this rule apply, and which operations are admissible on that surface?"

Policy is distinct from the procedure environment:

```text
ProcedureEnvironment:
  what operations/procedures/tools/oracles exist and how they execute

Policy:
  what evidence is admissible and what decisions are allowed from it
```

For now, `PolicyRef` should be minimal and explicit. It should not become a
policy engine in the first type slice. Policy must eventually decide:

- who may mint or admit;
- which surfaces may be patched;
- whether oracle/eval/promotion/provenance surfaces may be modified;
- what evidence is admissible;
- what validation and sample budget is required;
- how uncertainty and risk budgets are spent;
- how rollback, fork, conflict, and finality are handled.

## Block Content Groups

The next History type slice should preserve these groups without turning them
into giant report structs.

### First-Class In Block / History

- lineage coordinate and parent block hashes;
- lineage-local height as an index/projection, not identity;
- store scope / authority scope;
- opening authority;
- ruling authority;
- Crown transition;
- `PolicyRef` / `PolicyScope` through `Surface`;
- procedure environment / runtime contract ref;
- active Artifact commitment;
- selected successor Artifact commitment;
- selected successor Runtime identity/ref;
- entries root and entry count;
- evidence/sample roots or refs where used for admission;
- uncertainty/risk roots or refs where used for admission;
- rejected/failure evidence refs where needed to avoid selection bias;
- head-state concerns: rollback, fork/conflict, admission, and finality status;
- block hash.

### Referenced Through Artifact Or Evidence

Some important information should usually live in the Artifact or an external
content-addressed evidence surface, with the block committing to a digest/ref:

- validator independence evidence;
- evaluator/capability containment evidence;
- resource and sample budget details;
- oracle/eval benchmark digests;
- code/protocol references used to define cross-binary invariants;
- artifact-local provenance manifest.

This keeps the block from becoming a payload bag while preserving the ability to
audit and replay the authority/evidence path.

### Punted For Now

Human or root authority is intentionally not part of the current block
invariant. The model should leave room to define it later, but it is too
underspecified to make load-bearing now.

Semantic naming discipline and claim discipline remain implementation guidance,
not block fields.

## Artifact Commitment Shape

A plain `ArtifactRef` is not enough for durable authority. The block should
eventually commit to an Artifact through:

```text
backend tree key commitment
artifact-local provenance manifest digest/ref
recoverable backend coordinate such as commit/tree/ref
```

The manifest is the natural home for reconstructive evidence such as production
provenance, intervention refs, self-evaluation refs, build/runtime refs,
validator attestations, and code/protocol references used by policy.

## Probability And Reputation

The system should not compare a single observed score against another single
observed score as if both were deterministic facts.

For candidate Artifact `C9`, independent validators/rulers produce samples:

```text
Y_a, Y_b, ..., Y_k ~ Eval(Γ_eval, O, C9)
```

Those samples estimate a distribution. A reporter/ruler's claim can then be
scored against later independent evidence:

```text
How surprising was R8's reported score under the distribution estimated by
independent validators?
```

That yields two separate signals:

- quality of the candidate Artifact;
- reliability/calibration of the reporter, producer, validator, or policy.

Reputation should be calibration under later evidence, not generic trust or
agreement with consensus.

## Risk Of Probabilistic Collapse

If each generation has a fixed nonzero probability of false promotion, then over
an arbitrarily long horizon the probability of at least one bad promotion tends
toward one:

```text
P(at least one bad promotion) = 1 - (1 - q)^N
```

Therefore uncertainty must not be a decorative field. It must compose through
History, evaluation, policy, and reputation. A promotion should preserve the
evidence and uncertainty that policy consumed, including sample count,
candidate count, rejected attempts, failed attempts, adaptive stopping context,
known correlation/dependence risks, and risk-budget effects where policy relies
on them.

No probabilistic claim should be silently promoted into deterministic authority.
It may become decision-support authority under policy.

## Local And Complete History

The current Prototype 1 claim is local single-ruler authority:

```text
Given configured store H, lineage L, and current runtime contract,
only the valid Crown carrier may seal the next local block for H[L].
```

A larger blockchain model distinguishes local and complete authority:

```text
LocalHistory:
  what one runtime/store accepts under Crown authority

CompleteHistory:
  globally/canonically admitted ledger under consensus/finality policy
```

Future consensus may admit blocks produced by multiple local rulers. The
current local Crown block should not be described as global finality.

## Documentation And Implementation Implications

The next implementation tasks should add only minimal structural carriers:

- `PolicyRef` and `PolicyScope`, rooted in `Surface`;
- artifact commitment / manifest reference shape;
- head-state / rollback / finality placeholders;
- refs or roots for stochastic evidence, uncertainty/risk, rejected/failure
  evidence, and validation samples.

Do not introduce large names that flatten structure into identifiers. Preserve
structure through typed carriers, modules, explicit refs, and state transitions.
