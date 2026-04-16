# 2026-04-15 Builder Revival Note

- Date: 2026-04-15
- Task title: Reviving `ploke-db` builder surfaces as a Rust-native query handler
- Task description: Design-only note for a type-safe query/builder layer over `ploke-db` query methods, revised to incorporate repo-local Cozo framing, time-travel semantics, and a more formal query model. No production code changes.
- Related planning files:
  - [CURRENT_FOCUS.md](../CURRENT_FOCUS.md)
  - [Eval Design](../plans/evals/eval-design.md)
  - [P0C0 Query Builder Survey Report](2026-04-12_eval-infra-sprint/2026-04-12_P0C0_query-builder-survey-report.md)

## I. Reframing The Object

The dormant `ploke-db` builder surface should not be understood as a convenience API for assembling Cozo strings.

It is more faithful to the creator's framing to treat Cozo as **an algebra of relations**:

- stored relations are not merely tables but typed extensional facts
- rules define derived intensional relations
- query construction is composition over relation-valued expressions
- validity markers and `@`-queries change the basis of evaluation rather than merely adding a filter

In that framing, the builder revival problem is:

> How should a Rust-native query surface carry a relation-algebra intent object into executable Cozo while preserving basis, legality, and the boundary between in-Cozo derivation and Rust-side refinement?

That object is larger than the current `QueryBuilder`, and also larger than the live `raw_query_at_timestamp(...)` helper path.

## II. Current Local Evidence

Several local surfaces already imply this richer model.

- `QueryBuilder` is exported as a work-in-progress surface, but its `execute()` path is commented out and it still interpolates filters directly.
- `Database::raw_query_at_timestamp(...)` treats historical querying as explicit basis substitution over `@ 'NOW'` markers, not as a generic runtime option.
- `DbState` in `ploke-eval` treats historical replay as a lightweight typed handle over a timestamped basis.
- the CFG-processing ADRs in `syn_parser` deliberately choose a split architecture:
  - candidate retrieval in Cozo
  - semantic evaluation in Rust
- `ploke-rag` notes repeatedly treat Cozo queries as a compositional retrieval substrate that can combine graph traversal, vector retrieval, and later host-side reranking/refinement

These are not isolated implementation details. Together they imply a stable semantic distinction:

- **relational derivation** belongs to Cozo
- **host-side semantic refinement** may belong in Rust
- **basis binding** must remain explicit

## III. Formal Core

Let:

- `R` be the set of base relations
- `D` be the set of derived relations
- `B` be the set of evaluation bases
- `b_now in B` be the present-time basis
- `b_t in B` be a historical basis at timestamp `t`

Define a query intent object:

`Q = (S, P, J, F, Bq, H)`

Where:

- `S` is the selected subject relation family or relation expression
- `P` is the projection
- `J` is the join/derivation structure
- `F` is the in-Cozo predicate set
- `Bq` is the basis
- `H` is an optional host-side refinement phase

The important point is that `H` is not an error or an escape hatch. In this repo it is already a deliberate design choice in some domains, especially where semantic evaluation is better done in Rust than in Cozo.

The revived builder should therefore model a query as:

`Q : B x I -> C -> O`

More concretely:

- bind a basis `b`
- bind an intent `i`
- render a Cozo candidate program `c`
- optionally apply host refinement `h`
- yield output `o`

This lets the builder represent two valid shapes:

1. fully relational query:
   `Q_rel = render_and_execute(b, i)`
2. relational candidate plus host refinement:
   `Q_hybrid = refine_host(render_and_execute(b, i))`

## IV. Semantic Requirements

The revived surface should preserve these distinctions structurally.

### A. Basis is a semantic coordinate

The historical helper path already shows this clearly.

- `NOW` is not just a magic string
- `@ timestamp` is not just a textual replacement
- a query evaluated at `b_now` and the same query evaluated at `b_t` are not the same object

So basis should be explicit in the typed model.

### B. Relation legality should be structural

The current builder knows relation families and field sets, but it does not carry those constraints far enough.

- field legality should follow relation choice
- relation joins should only allow admissible bindings
- projections should be typed against the selected relation expression

### C. Host refinement is part of the formal model

The CFG ADRs are the clearest local proof.

They deliberately avoid forcing semantic CFG evaluation into Cozo and instead choose:

- Cozo for candidate retrieval and condition lookup
- Rust for parsing and evaluating the final expression

That pattern should not be treated as a failure of the builder. It is part of the query model.

### D. Rendering is downstream, not defining

The query object is not a string. A rendered Cozo script is only one projection of the query intent.

## V. Refinement / Evaluation Cycles

The framework below was refined in five passes and revised after each evaluation.

### Cycle 1

Initial candidate:

- define the revival target as a safer string builder with typestate stages

Evaluation:

- insufficiently faithful
- treats Cozo as output syntax rather than a relation algebra
- basis/time semantics remain secondary
- does not explain why some repo paths intentionally leave semantic work to Rust

Revision:

- elevate Cozo to the semantic substrate
- redefine the builder as a query-intent carrier over relations and bases

### Cycle 2

Revised candidate:

- typed query intent over relation family, projection, predicates, and basis

Evaluation:

- improved, but still assumes every meaningful step should end inside Cozo
- conflicts with the CFG-processing design, where host-side semantic evaluation is intentional

Revision:

- add an explicit host-refinement component `H`
- distinguish in-Cozo derivation from post-Cozo evaluation

### Cycle 3

Revised candidate:

- two-stage model:
  - relational candidate derivation
  - optional host refinement

Evaluation:

- corrects the CFG case, but still under-specifies historical evaluation
- basis is present, but not yet strong enough to prevent accidental cross-basis comparison

Revision:

- strengthen basis from a plain field to a semantic coordinate
- define basis-bound query intent as a different state from unbound intent

### Cycle 4

Revised candidate:

- basis-bound query intent with separate present/historical modes

Evaluation:

- stronger, but still too implementation-neutral about legality
- does not yet identify what the Rust builder must guarantee before rendering

Revision:

- make legality first-class:
  - legal projection
  - legal predicate attachment
  - legal relation composition
  - legal basis binding

### Cycle 5

Revised candidate:

- query-intent object with basis, relation legality, renderability, and optional host refinement

Evaluation:

- materially consistent with:
  - `raw_query_at_timestamp(...)`
  - `DbState`
  - CFG post-query evaluation
  - existing relation-family/schema surfaces
- remaining weakness:
  - still vulnerable to overgrowth if pushed into a universal AST before one narrow family is proven

Final revision:

- keep the formal model broad
- keep the implementation path narrow
- require the first implementation slice to prove the model on one query family before generalization

## VI. Final Framework

The revived builder should be understood as a typed carrier for the following semantic object:

`Intent = (Basis, RelationExpr, Projection, Predicates, HostRefinement?)`

The corresponding typestate progression should reflect semantic commitment rather than string assembly steps.

Suggested states:

- `UnboundIntent`
- `RelationBound`
- `ProjectionBound`
- `PredicateBound`
- `BasisBound`
- `ReadyForCandidates`
- `ReadyForExecution`
- `ReadyForHostRefinement`

Not every query must visit every state, but the sequence should encode real semantic commitments.

Two canonical paths should exist:

1. `RelationBound -> ProjectionBound -> PredicateBound -> BasisBound -> ReadyForExecution`
2. `RelationBound -> ProjectionBound -> PredicateBound -> BasisBound -> ReadyForCandidates -> ReadyForHostRefinement`

This second path is essential. Without it, the builder would incorrectly imply that all admissible semantics belong inside Cozo.

## VII. Correctness And Consistency Evaluation

The resulting framework appears locally correct and internally consistent on the following points.

### Correctness relative to current repo behavior

- It matches the accepted `P0C0` decision to bypass the old builder in the live historical-query lane.
- It respects `raw_query_at_timestamp(...)` as the current narrow historical helper.
- It matches `DbState`'s role as a basis-bound historical query handle.
- It matches the CFG ADR decision to split candidate retrieval from semantic evaluation.

### Internal consistency

- basis is explicit rather than implicit
- host refinement is modeled, not smuggled in
- rendering is derivative, not primary
- legality is structural, not merely documented

### Remaining pressure points

- the formal model is larger than what should be implemented first
- relation-family coverage should start narrow
- basis-aware rendering needs stable parameterization rules if it is to replace raw script interpolation safely

## VIII. Constraints On Any Revival

- Preserve `raw_query_at_timestamp(...)` as the live historical path until one typed basis-aware family is proven.
- Do not reduce the model back to a string builder with nicer methods.
- Do not force all semantic evaluation into Cozo when the local architecture already uses host refinement deliberately.
- Do not compare results across different bases as if they were produced under the same query object.
- Do not hide basis, legality, or host-refinement boundaries behind convenience calls.

## IX. Phased Revival Path

### Phase 0: Name the first family

Choose one narrow family:

- simple primary-node lookup
- one historical lookup family
- one graph traversal family

The first implementation should not try to solve everything.

### Phase 1: Implement basis-aware intent without rendering ambition

Represent:

- relation family
- projection
- predicate set
- basis

but keep rendering narrow and testable.

### Phase 2: Prove legality

Ensure the chosen family actually benefits from structural legality:

- illegal fields unavailable
- illegal basis omissions impossible
- deterministic rendering

### Phase 3: Add one hybrid path

Prove the model can represent:

- relational candidate derivation
- host-side refinement

without collapsing back into ad hoc helper composition.

### Phase 4: Compare against the live helper path

For the chosen family, compare:

- readability
- correctness
- rendered query stability
- ergonomics at call sites

### Phase 5: Expand only after the first family is clearly better

Only then widen to additional relation families or historical helpers.

## X. Design Bottom Line

The builder revival should target a basis-aware relation-algebra intent surface, not a prettier string API.

In this repo's own architecture, Cozo is best understood as an algebra of relations over present and historical bases. The Rust builder should therefore carry:

- what relation object is being formed
- under what basis it is being formed
- which legality constraints govern it
- whether execution ends in Cozo or continues through an explicit host refinement phase

If the revived surface cannot represent those distinctions, it is still too small.
