# Prototype 1 Selection Without Candidate Safety Proof

Status: source repaired and historical counterfactual passing; v25 preserved at
R12; fresh multi-generation live handoff proof pending.

## Broken Contract

A current-generation candidate must not become successor-handoff authority
until an artifact-bound review has established that the candidate's exact
repository change is safe and causally relevant to the observed loop problem.

Operational comparison, benchmark resolution, protocol completeness, and
candidate semantic safety are different claims. A `keep` disposition plus
resolved MBE evidence proves the first two claims only. Selection must fail
closed when the semantic-safety claim is absent, malformed, bound to a different
candidate artifact, or blocking.

This admission rule belongs before traversal seals a selection. History,
checkout, executable-epoch, Crown, oracle, and digest verification must remain
strict and unchanged.

## Preserved Incident

Campaign
`p1-v25-strictkeephandoff-mbe-g35f-direct-3g1x3-p3-obs2400-20260717-221126`
reached a verified R12 receipt and deterministically selected:

- node `node-f4f44e07e0be8caa`;
- branch `branch-55892858db150ccd`;
- commit `a9ca1b93510548958eb062c370e7d92e5142c72e`;
- decision `63b8fe16c9b3570fb1a978992ec56266e5c2f2a68a47a3b760f3a936f32e2941`;
- decision hash
  `26e318146c7ca203b18f924b6b329e96975743dfbe416a92d743730912545de6`.

All three generation-one candidates had:

- `overall_disposition = keep`;
- complete treatment evaluation and protocol artifacts;
- one resolved MBE target under the strict `all-resolved` oracle gate; and
- successful child build and runner completion.

The selected commit introduced a process-global Cargo metadata cache whose
authority is keyed only by the focused crate manifest's modification time.
Workspace-root or sibling-member manifest changes can therefore leave cached
workspace/package metadata stale. The candidate supplied no invalidation test
for that case. Its claimed performance motivation was also not supported by
the run: the baseline spent about 298.9 seconds in `request_code_context` and
about 1.4 seconds in Cargo tool calls.

A second unselected candidate,
`node-3247ff02a2053a57` /
`833f657f60819574f1513218d2f9ed86d9cae2d4`, introduced unbounded nested OS
thread fanout, `join().unwrap()` panic paths, and absent-versus-empty filesystem
semantic changes without focused regression coverage.

The third candidate,
`node-2f0b3cda4c2c8d89` /
`d4d6af3e4e5dab47dd014f0bb3a62a5876373b72`, made a narrow error-boundary
repair and passed focused checks. It was not selected because the current
generation-local strategy uses deterministic ScoreChildProp sampling over the
eligible candidates; it is not a safety chooser.

The walk server was stopped at R12. No R13, History, successor checkout, or
handoff evidence was written. The campaign, database, journals, and R12 receipt
remain unchanged. Only its inactive build `target/` directory was cleaned.

The exact persisted `SelectionDecisionEntry` is now pinned as a 573,257-byte
historical fixture. A first replay attempt reconstructed candidate payloads
from the later node files and produced a different decision hash. Structural
comparison against the persisted entry found only three non-derived payload
deltas: each candidate node's `updated_at` had advanced after R11. Because the
complete node record is part of `CandidateArtifact`, those mutable timestamps
changed all three payload hashes and membership IDs, the candidate-set root,
and the deterministic ScoreChildProp sample. Protocol aggregates, child
invocation citations, evaluation evidence, and selection metrics otherwise
matched. Historical replay must therefore hydrate the exact persisted entry,
not rederive selection authority from mutable post-selection node files.

## Source Boundary

Current source has strong structural evidence:

- `PlannedChildOutcome` binds child lifecycle, terminal channel evidence,
  parent comparison, selection input, edit surface, and `ArtifactSurface`;
- `current_generation_candidate_evidence` seals that typed evidence;
- `current_generation_candidates` creates traversal payloads and reports
  missing structural inputs;
- the strict oracle gate excludes missing or unresolved benchmark evidence; and
- `select_artifact_for_handoff` revalidates the selected artifact before
  checkout.

The missing object is an authority-bearing, artifact-bound candidate review.
Neither `Prototype1BranchEvaluationReport.overall_disposition` nor the existing
tool-call protocol artifacts make that claim. Adding a filename denylist,
changing the seed, choosing the best observed score, or weakening handoff
verification would only hide the incident.

The pre-child planning review also has an evidence-routing defect that
contributed to poor candidate quality. Its Direct Google call receives rendered
path strings but not the cited trace/protocol contents, and response validation
checks only that citation strings are nonempty. V25 consequently cited a future
submission path, a missing History path, and a literal protocol-path template.
This defect should be repaired independently by loading existing typed run and
protocol carriers, rendering bounded content with hashes, and validating the
returned citations against the exact supplied source set.

## Required Repair

The smallest safe design is a separate candidate-admission gate before
ScoreChildProp:

1. review the exact parent-to-candidate diff and its bound `ArtifactSurface`
   after child evaluation;
2. persist a typed verdict with evidence citations, artifact identity, reviewer
   identity, rationale, and blocking findings;
3. require a complete, internally consistent, non-blocking verdict under an
   explicit run-profile policy;
4. project excluded candidates and their reasons into the durable no-selection
   receipt and owner database; and
5. keep ranking unchanged for the remaining admitted candidates.

The gate must not reinterpret tool-call usefulness reviews as whole-patch
safety reviews. If no existing carrier expresses the full claim, add the
narrowest domain type at the evaluation/selection boundary rather than storing
an untyped JSON flag.

## Docs Expectation

The run-profile documentation must name semantic candidate admission separately
from operational disposition and oracle admission. The strict live profile used
for the multi-generation proof must enable all three.

Operator surfaces must show, per candidate:

- operational disposition;
- oracle gate status and target evidence;
- semantic review status and exact artifact binding;
- exclusion reason, if any; and
- whether the candidate entered the final traversal set.

## Implemented Repair

The source repair keeps ranking and every existing handoff verification intact:

- `[selection.patch] gate = "reviewed-admissible"` enables a separate
  pre-ranking semantic admission policy; the omitted/default policy remains
  `disabled` for historical profile compatibility.
- Candidate review records bind the exact candidate coordinate, derived
  artifact, full admitted change set, artifact surface, evaluation artifact,
  resolved reviewer configuration, provider output, verdict, findings, and
  citation hash.
- Missing, malformed, mismatched, rejected, or inconclusive reviews make a
  candidate ineligible before either traversal strategy ranks candidates. An
  all-rejected set seals a typed no-selection receipt with explicit
  `patch_gate_not_satisfied` rows.
- R0 setup persists and verifies the admitted gate plus reviewer-config hash.
  Selection persistence verifies that setup authority before any selection
  schema or row write.
- The owner database stores normalized gate, review, and per-path change rows.
  Walk audit requires those rows to match the typed receipt,
  production-derived decision ID, normalized decision, candidate membership,
  and candidate identity. Receipt-only or file-only corruption fails closed.

The immutable V25 fixture now exercises production traversal twice. The
original disabled-gate payloads reproduce the recorded unsafe selection. Typed
post-incident overlays using the production reviewer-config identity then
exclude both unsafe candidates and select the narrow candidate; a second
all-rejected overlay proves durable no-selection replay. Both paths assert that
the fixture, History, review directory, and owner database remain unwritten.

Focused setup, persistence, audit, traversal, and V25 regressions pass. The
`ploke-eval` library suite passes 1,442 tests; its sole remaining failure is the
pre-existing backup fixture that lacks the current `array_type` relation. The
operator UI currently projects semantic gate/verdict status and counts; a
future UI slice still needs the complete binding and finding drilldown described
above.

## Required Repro Coverage

The historical replay must use immutable, SHA-256-pinned V25 artifacts and
production selection code. It must first reproduce the recorded deterministic
selection. With the strict semantic gate enabled, the same replay must prove:

- the selected unsafe candidate is excluded by a blocking verdict bound to its
  exact commit and artifact surface;
- absent, mismatched, or malformed review evidence fails closed;
- the independently reviewed narrow candidate remains eligible;
- traversal cannot select either blocked candidate;
- the no-selection path remains durable if every candidate is blocked; and
- no History, checkout, or handoff mutation occurs before admission succeeds.

The source replay coverage above is implemented. A fresh campaign is still
required, and V25 must not be retrofitted. The live proof is complete only when
an admitted candidate crosses the existing strict R12-to-R13b boundary, the
successor server takes control, and that successor advances at least one
further generation without rewriting prior evidence.
