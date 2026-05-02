Here is the central articulation.

We are not building a deterministic self-improvement chain. We are building an authority and evidence system around stochastic self-improvement.

The deterministic layer gives us identity, custody, and tamper evidence:

```text
Artifact identity
Runtime provenance
Crown authority
History block hash
Entry hashes
Tree key / commit / manifest digest
Procedure environment refs
Oracle refs
```

The stochastic layer gives us performance evidence:

```text
patch generation
LLM adjudication
self-evaluation
oracle-backed scoring
branch comparison
validator/ruler re-evaluation
```

The block is where those two layers meet.

A block should not claim:

```text
Artifact A deterministically improves the system.
```

It should claim:

```text
Under authority path P, ruler R admitted artifact A with these deterministic
provenance commitments and these observed evaluation samples.
```

Then later blocks or validation records can add more samples, producing stronger posterior estimates.

The key invariants:

```text
Crown:
  For one configured History store and lineage, at most one valid typed carrier
  holds mutable ruling authority.

Artifact/Runtime:
  Every admitted Runtime is ProducedBy some recoverable Artifact.

Startup:
  A Runtime may become Parent<Ruling> only if its current Artifact matches the
  artifact commitment expected by the latest accepted History head.

History:
  Blocks are sealed, hash-addressed authority epochs. Mutable reports and
  scheduler files are projections/evidence, not authority.

Procedure Environment:
  Γ is not one procedure. It is the artifact-defined environment of available
  operations, protocols, oracle bindings, and admissible behaviors under which
  a Runtime acts.

Evaluation:
  Evaluation results are samples from a distribution induced by
  (Artifact, Γ_eval, Oracle, operational environment), not deterministic facts.

Consensus/Reputation:
  Independent re-evaluations produce more samples. Those samples estimate both
  candidate artifact quality and reporter/ruler reliability.
```

The powerful move is second-order validation:

```text
R8 reports C9 scored y8.

Other rulers independently evaluate C9:

Y_a, Y_b, ..., Y_k

Those samples estimate the posterior predictive distribution for C9.

Then we can ask:

How surprising was R8's report under that distribution?
```

That gives us:

```text
quality(C9)
reliability(R8 as producer/reporter)
reliability(R_a...R_k as validators/rankers)
```

So the system can reward not just “found a good child,” but “made claims that survive independent validation.”

This is where blockchain analogy becomes real:

```text
Bitcoin:
  scarce resource = proof-of-work

Ethereum:
  scarce resource = stake at risk

Ploke self-improvement chain:
  scarce resource = credible, reproducible evaluation work
  plus reputation at risk
```

Not reproducible as “rerun LLM and get identical output,” but reproducible as:

```text
checkout artifact
recover procedure/oracle/evidence refs
rerun evaluation protocol
produce additional samples from the same declared distribution
```

So “proof of improvement” should probably become something like:

```text
ProofOfEvaluatedProgress
```

Meaning:

```text
Candidate artifact A has been evaluated under declared Γ/O/policy,
with content-addressed samples,
and independent validators have produced evidence supporting improvement over baseline
within stated uncertainty.
```

This lets us avoid self-referential collapse because the chain is grounded in external oracles and independently repeated evaluation work.

The block-content model follows:

```text
Authority:
  lineage/store scope
  Crown transition
  ruler identity
  opening authority
  policy/procedure environment ref

Artifact:
  active artifact commitment
  selected successor artifact commitment
  tree key / commit / manifest digest

Evidence:
  admitted entries
  sample refs
  oracle/eval refs
  payload hashes

Selection:
  reported score/decision
  baseline comparison
  uncertainty summary

Validation:
  independent evaluator refs
  validation sample refs
  reputation/ranking weights

Hashing:
  entries root
  block hash
  parent block hashes
  authenticated head map root eventually
```

The current local-ruler model is then a special case:

```text
One ruler, one configured History store, one local Crown.
```

The larger model is:

```text
many local candidate histories
  -> independent validation
  -> reputation-weighted/evidence-weighted admission
  -> CompleteHistory
```

That is the shape that ties together History, Crown, Tree, Artifact, probability, oracle grounding, and reputation.
