# Prototype 1 Agent Probe Review A

Review date: 2026-04-28.

Reviewed probes:

- `docs/reports/prototype1-agent-probes/probe-1.md`
- `docs/reports/prototype1-agent-probes/probe-2.md`
- `docs/reports/prototype1-agent-probes/probe-3.md`
- `docs/reports/prototype1-agent-probes/probe-4.md`
- `docs/reports/prototype1-agent-probes/probe-5.md`

Context checked:

- `AGENTS.md`
- `docs/workflow/evalnomicon/drafts/prototype1-history-metrics-agent-brief.md`
- `docs/workflow/evalnomicon/drafts/history-blocks-and-crown-authority.md`
- `docs/workflow/evalnomicon/drafts/formal-procedure-notation.md`
- `docs/workflow/evalnomicon/drafts/runtime-artifact-lineage.md`
- `docs/reports/prototype1-record-audit/history-admission-map.md`
- `crates/ploke-eval/src/cli/prototype1_state/mod.rs`
- `crates/ploke-eval/src/cli/prototype1_state/history.rs`
- `crates/ploke-eval/src/cli/prototype1_state/metrics.rs`

## Overall Finding

All five probes identify the same correct next patch: make `history metrics
--view trajectory` preserve cohort/branch ambiguity instead of collapsing
selected rows into one selected-by-generation chain. That is aligned with the
brief's stated implementation truth: metrics are read-only projections, current
records lack `lineage_id`, and the existing selected-by-generation projection is
only valid for the current single-lineage run shape.

The probes are mostly sound because they stay out of live Crown/History
mutation and avoid treating JSON records as authority. The main corrections are
terminology and feasibility boundaries:

- Do not call the result a real lineage projection. Current records support a
  degraded coordinate: `(parent_node_id, generation)`.
- Do not infer a missing selection slot merely because a cohort has zero
  selected rows unless there is source evidence that a selection was expected.
- Do not use `trajectory_status: single_lineage` unless the status name makes
  clear that this is a projection shape, not a proven lineage fact.
- Do not treat `selection_authority` as Crown authority. In current metrics it
  means selection evidence class, with `transition_journal` stronger than
  `mutable_projection`.
- Do not extend `history.rs`, `SealBlock`, Crown, or LockBox machinery for this
  patch.

## Conceptual Boundaries

The conceptual model says:

- Parent-ness is a runtime role/state, not a backend operation.
- Crown is lineage-scoped authority to mutate one active lineage and choose its
  successor.
- History is sealed authority blocks plus provenance-bearing entries and
  ingress.
- Metrics are projections over evidence or History. They remain useful without
  becoming authority.
- Current records do not carry stable `lineage_id`; cohort metrics group by
  `(parent_node_id, generation)` and should diagnose that degradation.
- Cross-runtime handoff is not a single in-process state machine. The outgoing
  Parent locks handoff material; the successor verifies/unlocks before becoming
  `Parent<Ruling>`.

The implementation matches that boundary. `metrics.rs` builds a `Dashboard`
projection with rows, generations, cohorts, selected-by-generation steps, and
diagnostics. `cohorts` already groups by `parent_node_id` and `generation` with
`lineage: None`. `generation_summary` currently chooses the first selected row
for a generation, and `selected_by_generation` follows those generation
summaries, so it can erase branch structure. `history.rs` contains typestate
History scaffolding, but `SealBlock` explicitly records that
`crown_lock_transition` is header material, not a live authority token.

## Probe 1

Verdict: sound and strongest overall plan, with one naming correction and one
feasibility correction.

Strong points:

- Correctly identifies the main correctness risk: one selected successor per
  generation is an unsafe assumption once multiple parent/generation cohorts
  appear.
- Correctly preserves projection-vs-authority: metrics remain disposable
  projections and should not become Crown evidence or History admission.
- Correctly points implementation at `metrics.rs` and focused tests.
- Correctly requires JSON ambiguity as data rather than display-only prose.
- Correctly rejects broad names such as `HistoryMetrics` and
  `TrajectoryAnalysisReport`.

Corrections:

- `trajectory_status: single_lineage` risks implying a real lineage fact. A
  safer status would be `unambiguous_projection`, `ambiguous_projection`, or
  `incomplete_projection`, with a separate diagnostic that lineage is
  unavailable.
- "Emit a diagnostic when a group has zero selected successors" is only sound
  for observed candidate cohorts where the projection has enough evidence to
  say a selection was expected. It should not invent expected successor slots
  from missing data.
- Emitting one trajectory row for every group with exactly one selected
  successor is not enough to prove one chain. If multiple cohorts in the same
  generation each have one selected row, the projection is branching or
  multi-chain, not a single trajectory.

Residual risk:

- The plan says `--view cohorts` should include selected-count and
  candidate-count if missing. `selected_count` and node count already exist in
  current `Cohort`; the patch should avoid churn unless the JSON/table names
  need clarification.

## Probe 2

Verdict: sound, but more likely than Probe 1 to overstate "lineage" and to
broaden the JSON shape.

Strong points:

- Correctly frames the work as a small metrics/CLI patch, not a History
  authority patch.
- Correctly proposes `chain`, `ambiguities`, and `diagnostics` as structured
  JSON fields.
- Correctly says not to present dashboard heuristics as Parent policy, oracle
  truth, or sealed History.
- Correctly calls for local structure instead of long prefixed helper names.

Corrections:

- "The object is a lineage projection" should be narrowed. With current
  records, the object is a selected-candidate projection over degraded cohort
  coordinates. It may approximate one lineage in the present run shape, but it
  is not lineage evidence.
- A partial `chain` is feasible only if each step preserves parent continuity:
  the selected row in generation `n + 1` must cite the selected node from
  generation `n` as `parent_node_id`, or the projection should split/stop and
  report ambiguity.
- The plan asks for source refs and many operational fields in ambiguity rows.
  That is useful, but it may require a new focused row payload rather than
  reusing the current `Step`, which only contains summary fields. That is an
  implementation cost, not a conceptual problem.

Residual risk:

- "Focused trajectory payload" is desirable, but current `Slice` returns rows,
  generations, cohorts, selected-by-generation, and diagnostics for JSON. The
  patch should decide whether to add a focused `trajectory` field without
  breaking existing JSON consumers.

## Probe 3

Verdict: sound concise plan, with residual underspecification.

Strong points:

- Correctly identifies the latent object as parent-to-successor choices with
  diagnostics when the single-lineage assumption fails.
- Correctly preserves the projection boundary.
- Correctly lists the important ambiguity cases: multiple selected rows in a
  cohort, missing parent coordinates, and generation-only collapse.
- Correctly leaves dashboard score/rank alignment as a follow-up.

Corrections:

- "Inside a runtime/artifact lineage" should be treated as future intent, not
  current evidence. Runtime/artifact operation coordinates are central to the
  conceptual model, but current metrics rows mostly have `runtime_id`,
  `branch_id`, `node_id`, `parent_node_id`, and generation. They do not prove
  the full operation coordinate.
- The probe does not spell out what should happen when multiple cohorts exist
  in one generation but each has exactly one selected row. That is the likely
  failure case for selected-by-generation collapse and should be explicit.

Residual risk:

- The verification command `cargo test -p ploke-eval` may be broader than
  needed. Use the narrow metrics/prototype1 tests first, then expand if the
  touched code warrants it.

## Probe 4

Verdict: sound and structurally disciplined, with a minor naming risk.

Strong points:

- Best at identifying the missing structural carrier: cohort coordinate,
  generation rows, selected rows, and diagnostics.
- Correctly rejects flattened names like
  `HistoryMetricsTrajectoryAmbiguity`.
- Correctly states that this patch must not imply sealed History authority,
  Crown-gated mutation, or compiler-enforced transition validity.
- Correctly notes that `dashboard_score` should not be presented as explaining
  `dashboard_rank` unless that alignment is implemented.

Corrections:

- "Selected parent-successor chain inside a cohort" is slightly too narrow:
  a cohort is one `(parent_node_id, generation)` candidate set, while a chain
  crosses cohorts/generations through parent continuity. The carrier should
  likely distinguish `Cohort` from `Trajectory` or `Chain`, not make one contain
  the other ambiguously.
- "Selected rows appear in more than one cohort for the requested trajectory
  view" should be treated as branching or multi-chain evidence, not necessarily
  an error.

Residual risk:

- Probe 4 says to preserve existing single-lineage output when exactly one
  selected row exists for each generation in the chosen cohort. A "chosen
  cohort" is not currently an input parameter. Without such a selector, the
  implementation should derive all observed chains or report that the view is
  ambiguous.

## Probe 5

Verdict: sound high-level plan, but it contains the most ontology drift.

Strong points:

- Correctly says the patch should avoid History authority code.
- Correctly requires `NoSelection`, `OneSelection`, and
  `MultipleSelections`-style state instead of silent collapse.
- Correctly calls out missing lineage ids and dashboard score/rank as residual
  limits.

Corrections:

- The "latent object" includes "parent runtime/source coordinate when
  available", but current metric grouping does not have that full coordinate.
  The conceptual model needs generator Runtime plus target/source Artifact, but
  current metrics should not invent that from `parent_node_id`, branch names, or
  worktree paths.
- "Selected row exists but lacks stable lineage id" should not be treated as a
  per-row anomaly. That is currently a global limitation of this record family;
  the projection should diagnose it once or attach it as projection metadata.
- "Change JSON output so `--view trajectory` returns a focused trajectory
  payload rather than a broad all-metrics payload" is directionally good but may
  be a compatibility break. Prefer adding a focused `trajectory` payload first,
  then narrow older fields only if the command contract allows it.

Residual risk:

- The plan mentions patch apply state and submission state, but current `Row`
  stores aggregate counts such as `patch_attempted_instances`,
  `applied_patch_instances`, `partial_patch_instances`,
  `missing_submission_instances`, and `empty_submission_instances`, not a
  single state. The implementation should preserve that aggregate semantics.

## Assumed, Invented, Or Implied Invariants To Reject

Reject these invariants if they appear during implementation:

- One selected row per generation. Current code assumes this through
  `generation_summary` and `selected_by_generation`; the patch exists to remove
  that assumption.
- One exact selected row per `(parent_node_id, generation)` means one lineage.
  It means one selected row in a degraded cohort coordinate. It becomes a chain
  only when parent continuity also holds across generations.
- `parent_node_id + generation` is a stable lineage id. It is only the best
  available degraded grouping coordinate.
- Generation equals block height. The admission map explicitly says generation
  is block height only where it matches a Crown epoch; branching/merge can break
  that equivalence.
- Branch id, worktree path, or scheduler node id is Artifact identity. These
  are handles/refs unless backed by durable artifact identity or content hash.
- `selection_authority` in metrics is Crown authority. It is evidence-source
  strength, currently `transition_journal` or `mutable_projection`.
- Scheduler or branch registry selection is durable authority. Those records
  remain mutable projections unless admitted later with explicit provenance.
- Dashboard score explains dashboard rank. Current rank key includes fields the
  score does not include, so they are related heuristics, not the same policy.
- Metrics output can become History evidence by being under the `history`
  command. It remains a projection.
- A `crown_lock_transition` evidence ref is a `Crown<Locked>` authority token.
  Current `history.rs` explicitly says it is not.
- Cross-runtime Crown/LockBox handoff can be represented as a local trajectory
  metric. The handoff protocol is future live authority work; metrics can only
  expose evidence about selections and continuity.
- Missing lineage ids can be repaired by naming a field `lineage` and filling
  it from generation, branch, or campaign. That would erase the actual data
  gap.

## Recommended Patch Direction

Use Probe 1 as the base plan, borrowing Probe 2's structured
`chain`/`ambiguities`/`diagnostics` JSON shape and Probe 5's explicit
selection-state enum. Keep the implementation local to `metrics.rs` unless CLI
dispatch code must change.

Suggested internal structure:

- `Coordinate { lineage: Option<String>, parent_node_id: Option<String>, generation: u32 }`
- `SelectionState::{None, One(Choice), Many(Vec<Choice>)}`
- `Trajectory { status, chain, ambiguities, diagnostics }`

Name the status as projection state rather than lineage truth:

- `unambiguous_projection`
- `ambiguous_projection`
- `incomplete_projection`

For table output, keep the existing compact selected-by-generation section only
when the projection is unambiguous. When ambiguous, show competing selected
candidates grouped by coordinate and print diagnostics.

For JSON output, expose ambiguity as structured data. If compatibility matters,
add a new focused trajectory field before removing the existing broad `Slice`
fields.

## Feasibility Notes

This is implementation-feasible as a focused metrics patch:

- The current `Row` already carries generation, node id, parent node id,
  branch/runtime refs, selected flag, selection sources, dashboard rank/score,
  operational aggregate counts, and source refs.
- `Cohort` already carries `selected_count`, `selected`, `top`, and aggregate
  operational counts.
- Tests already exist in `metrics.rs` for selected source attachment, cohort
  grouping, and richer operational metrics, so ambiguity tests can be added
  locally.

The likely code changes are:

- derive trajectory from rows/cohorts before `Dashboard` construction;
- stop deriving trajectory only from `Generation.selected_node_id`;
- add tests for multiple selected rows in one cohort, multiple selected cohorts
  in one generation, missing parent coordinates, and preserved single-chain
  behavior.

No History/Crown code should be touched. If a proposed implementation needs
`history.rs`, `SealBlock`, `Crown<Locked>`, `Parent<Ruling>`, `Successor`,
LockBox files, or block sealing to complete this metrics patch, that is a sign
the patch has crossed the projection-vs-authority boundary.
