# Prototype 1 Metrics Next-Patch Plan

Read set used:

- `AGENTS.md`
- `docs/workflow/evalnomicon/drafts/prototype1-history-metrics-agent-brief.md`

## Recommendation

Patch `history metrics --view trajectory` so it becomes explicitly
cohort/single-lineage aware. The current trajectory projection assumes one
selected successor per generation. That is useful for the current single-lineage
run shape, but it becomes misleading as soon as selected rows branch within a
generation or parent cohort. The next useful metrics improvement is to surface
that ambiguity directly instead of silently collapsing it into a chain.

This should be a small metrics/CLI patch, not a History authority patch.

## Patch Shape

Latent object:

The object is a lineage projection over observed campaign evidence. It is not a
sealed History block and not the Parent's historical policy. The useful question
is: "What selected parent-successor path can the operator inspect, and where
does the evidence stop supporting a single path?"

Reduction to avoid:

Do not create another broad `HistoryMetrics`-style abstraction or invent a new
generic authority layer. Do not encode the problem into names like
`prototype1_monitor_trajectory_branch_conflict_detector`. Use a focused
trajectory projection type or local context that carries cohort/generation
structure, with short methods such as `selected`, `ambiguities`, `chain`, and
`diagnostics`.

Files I expect to touch:

- `crates/ploke-eval/src/cli/prototype1_state/metrics.rs`
- Possibly the local CLI/view dispatch file if `--view trajectory` rendering is
  outside `metrics.rs`
- Tests colocated with the metrics code if they already exist; otherwise add
  focused unit coverage near the projection logic

Invariants this patch must preserve:

- Metrics remain projections over current evidence, not authoritative History.
- JSON/current records do not become authoritative because they appear in a
  trajectory.
- Existing commands keep working:
  - `ploke-eval history metrics --view cohorts`
  - `ploke-eval history metrics --view trajectory`
  - `ploke-eval history metrics --generation 2 --rows 12`
- Compatibility aliases may keep working, but the primary UX remains
  `history metrics`.
- Parent/child and Crown concepts are not flattened into event-style ontology.
- Any diagnostic about missing lineage id must remain explicit; grouping by
  `(parent_node_id, generation)` is still degraded evidence.

## Concrete Behavior

Implement the trajectory view as a projection with three visible sections in
table output and matching fields in JSON output:

1. `chain`: the selected parent-successor rows only when each generation/cohort
   has exactly one selected successor.
2. `ambiguities`: generations or cohorts where more than one selected row
   exists, including parent node id, generation, candidate node ids, statuses,
   dispositions, dashboard rank/score fields, source refs, and runtime refs.
3. `diagnostics`: degraded-lineage notes such as missing lineage id and any
   skipped rows needed to explain why no single chain was emitted.

If the projection sees multiple selected rows for a generation or cohort, table
output should say that the trajectory is ambiguous and show the competing
selected candidates. JSON output should not return a fake single chain; it
should return `chain: []` or a partial chain plus a populated `ambiguities`
array, depending on what the evidence supports.

If the evidence is single-lineage, preserve the current useful trajectory
display but enrich each row with the fields operators need for triage:

- node id
- parent node id
- generation
- status and disposition
- branch/runtime refs
- source refs
- patch attempt/apply/submission state
- tool call totals/failures
- abort and repair-loop counts
- dashboard rank and dashboard score, displayed as separate fields

## Acceptance Criteria

- `--view trajectory` no longer silently chooses one selected row when multiple
  selected rows exist for a generation or parent cohort.
- Table output gives an operator enough information to decide which candidate
  needs inspection next without opening raw record files.
- JSON output has stable keys for `chain`, `ambiguities`, and `diagnostics`.
- `dashboard_rank` and `dashboard_score` are not presented as if the score
  fully explains the rank.
- The implementation uses local structure to express the projection instead of
  adding long prefixed helper names.
- Documentation or command help, if touched, says this is a projection over
  evidence and does not claim live Crown-gated sealed History.

## Verification Commands

Run the focused checks first:

```text
cargo test -p ploke-eval prototype1_state::metrics
cargo check -p ploke-eval --tests
```

Then run representative CLI smoke checks against an available Prototype 1
campaign:

```text
cargo run -p ploke-eval -- history metrics --view trajectory
cargo run -p ploke-eval -- history metrics --view cohorts
cargo run -p ploke-eval -- history metrics --generation 2 --rows 12
```

If the test names are not addressable that narrowly, fall back to the smallest
available `ploke-eval` test target that covers `prototype1_state`.

## Known Gaps This Patch Will Not Solve

- It will not add live `Crown<Locked>` gating.
- It will not make current JSON records authoritative History.
- It will not recover a true lineage id that is absent from current records.
- It will not replace scheduler or journal files.
- It will not implement multi-parent authority; it only makes branch evidence
  visible in the metrics projection.
