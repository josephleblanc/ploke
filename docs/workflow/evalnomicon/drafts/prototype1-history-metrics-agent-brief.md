# Prototype 1 History And Metrics Agent Brief

Last updated: 2026-04-29 11:31 PDT.

Purpose: compact post-compaction context for work on Prototype 1 History,
metrics projections, and CLI surfaces. Read this before editing
`crates/ploke-eval/src/cli/prototype1_state/` or adding History/metrics
commands. This is not the full design source; it is a lossy but intentional
operator brief.

## Read Order

Minimum read set for most tasks:

1. `AGENTS.md`
2. this file
3. `crates/ploke-eval/src/cli/prototype1_state/mod.rs`
4. `crates/ploke-eval/src/cli/prototype1_state/history.rs` when touching
   History, Crown, Block, policy, or authority claims
5. the directly edited source file
6. `docs/reports/prototype1-record-audit/history-admission-map.md`

Read deeper only when the task touches the relevant concept:

- current History/Crown/blockchain framing:
  `docs/workflow/evalnomicon/chat-history/history-blocks-v2.md`
- older Crown/authority background:
  `docs/workflow/evalnomicon/drafts/history-blocks-and-crown-authority.md`
- procedure graphs and mixed LLM/mechanized metrics:
  `docs/workflow/evalnomicon/drafts/formal-procedure-notation.md`
- runtime/artifact graph location:
  `docs/workflow/evalnomicon/drafts/runtime-artifact-lineage.md`
- record inventory:
  `docs/reports/prototype1-record-audit/*.md`

For broad filesystem or record-shape discovery, use explorer sub-agents. The
main agent should not spend context reading large record files unless an
explorer has narrowed the target and suggested a minimal verification command.

## Semantic Model

Prototype 1 is a self-improving trampoline loop over runtime/artifact pairs.
The current proof of concept edits a bounded text surface included in the
binary, but the long-term target is a `ploke-tui`-backed Rust code agent that
operates over real code graph surfaces.

Core terms:

- Artifact: a checkout/source state that can hydrate a Runtime.
- Runtime: an executing process hydrated from an Artifact; every Runtime is a
  potential Parent if granted authority.
- Parent: a Runtime in a role/state with authority over one active lineage.
- Child: a Runtime evaluating a candidate Artifact.
- Crown: authority to mutate one active lineage and choose its successor.
- History: the intended durable substrate: sealed local authority blocks,
  provenance-bearing entries, evidence references, policy-scoped admission
  state, and future head-state/finality material.
- Journal/current JSON records: transitional evidence, mutable buffers, or
  projections. They are not sealed History.
- Projection: a disposable view or derived metric over evidence or History.
  A projection can be useful without becoming authority.
- Ingress: late/backchannel observations outside a sealed Crown epoch.

The graph is not just git ancestry. A Runtime can operate over an Artifact that
did not hydrate it. Semantic location eventually needs both operative Runtime
and derivation/source Artifact coordinates.

## Non-Negotiable Constraints

Do not flatten structure into new names. Examples to avoid:

- `JournalView`, `EvaluationView`, `HistoryMetrics`
- `ChildArtifactCommittedEntry` as a History ontology
- `CrownLockEvidence` as a structural authority carrier
- helper names that encode subsystem + phase + action because there is no
  module/type boundary

Prefer structure:

- CLI: `history metrics`, `history preview`
- states: `Parent<Ruling>`, `Crown<Locked>`, `Block<Sealed>`
- entries: role/state/procedure facts admitted with explicit provenance
- modules/types/traits to carry meaning instead of long prefixes

Compatibility aliases may still contain older flattened names. Do not treat
those as precedent for new code or docs.

Typed transitions must not be public status writes. Authoritative states should
be hard to construct accidentally: private fields, module-private or sealed
markers, move-only transitions, and durable records emitted as projections of
allowed transitions.

Do not treat nearby Prototype 1 code as authoritative just because it exists.
Some of it is sprint code and known to contain duplicated records, oversized
helpers, and weak abstractions.

## Current Implementation Truth

Implemented:

- `history.rs` contains a self-contained typestate framework:
  `Entry<Draft> -> Entry<Observed> -> Entry<Proposed> -> Entry<Admitted>`,
  `Block<Open> -> Block<Sealed>`, and ingress import scaffolding.
- `history_preview.rs` imports existing campaign evidence into a read-only
  History-shaped preview with degraded provenance.
- `metrics.rs` derives read-only metrics projections over current evidence.
- CLI now has operator-facing commands:
  - `ploke-eval history metrics`
  - `ploke-eval history preview`
- Older nested commands under `loop prototype1-monitor` remain compatibility
  aliases but should not be advertised as the primary UX.

Not implemented:

- live `Crown<Locked>` does not yet gate block sealing.
- live startup validation does not yet gate `Parent<Ruling>` on the current
  checkout Tree key matching the sealed History head.
- live `Crown<Locked>` sealing does not yet persist the block that the next
  runtime must verify before entering `Parent<Ruling>`.
- current metrics are not sealed History entries.
- current JSON records do not become authoritative by being imported.
- lineage id is not available in current records; cohort metrics group by
  `(parent_node_id, generation)` and emit a diagnostic.

Important claim boundary:

```text
This system currently gives useful projections and a History-shaped preview.
It does not yet give live Crown-gated sealed History.
```

## Metrics Surface

Current command examples:

```text
ploke-eval history metrics --view cohorts
ploke-eval history metrics --view trajectory
ploke-eval history metrics --generation 2 --rows 12
ploke-eval history preview --entries 5 --diagnostics 5
ploke-eval history preview --entry 80
```

If `--campaign` is omitted, `history` commands resolve campaign from:

1. current parent identity
2. active selection
3. most recently modified Prototype 1 campaign

Metrics currently provide:

- node rows with status, disposition, branch/runtime refs, source refs, tool
  call totals/failures, patch attempts, patch apply states, submission states,
  abort/repair-loop counts, heuristic rank/score, and row-local score
  derivation
- generation summaries
- cohorts grouped by parent and generation
- cohort-aware trajectory projection with structural `Delta` records over
  selected steps and cohort decisions

Known metrics issues:

- trajectory now derives from rows/cohorts rather than generation summaries.
  Keep it that way; generation summaries still exist for compatibility and must
  not become the semantic source for trajectory.
- current records do not prove lineage. Wording such as "single lineage" must
  be reserved for the future state where a real lineage coordinate exists, or
  qualified as an unambiguous projection under degraded coordinates.
- zero selected rows are not automatically a protocol failure. Treat them as
  missing or incomplete evidence only when the inspected cohort/trajectory had
  enough source evidence to expect a selection.
- `dashboard_rank` and `dashboard_score` are related but not identical; avoid
  implying the score fully explains rank unless the implementation is aligned.
- dashboard scores are local analysis heuristics, not oracle truth and not the
  historical Parent policy.
- trajectory deltas are dashboard-score projection comparisons only. They are
  represented structurally as `Delta { basis, state, against, value, count }`
  with bases such as `Parent`, `Alternative`, and `Previous`; do not reintroduce
  flattened prose names such as `dashboard_score_delta_from_top`.

## History/Crown Direction

The target authority sequence is:

```text
Runtime starts from a checkout
Startup derives the backend Tree key for that checkout
Startup verifies the current sealed History head expects that Tree key
Startup validates artifact-carried parent identity as redundant evidence
Startup enters Parent<Ruling>
Parent<Ruling> records entries while it has the Crown
Parent<Ruling> installs selected Artifact
Parent<Ruling> locks Crown<Locked>
Crown<Locked> seals Block<Sealed>
The next Runtime repeats Startup validation before entering Parent<Ruling>
```

This is a cross-runtime protocol, not a single in-process state machine. The
runtime that locks the box is not the runtime that opens it. Each generation is
compiled with the same shared contract, but the transition is offset across
Parent runtimes: the outgoing Parent writes/locks the handoff material at the
end of its rule, and the next runtime later verifies the sealed head and its
own checkout before entering the next Parent role. Locally, `Parent<Ruling>`
is admitted through the artifact named by the sealed History head: the startup
gate must relate the runtime, its current checkout Tree key, and the expected
successor Artifact before granting Crown-backed mutable History authority.
This does not claim OS-process uniqueness; duplicate processes executing the
same admitted Artifact remain outside the current type-state guarantee.

The "Crown" names that one-at-a-time lineage authority. Parent is a role a
runtime may hold; Crown is the capability that prevents two Parents from
mutating the same local History lineage as if both were ruling under the same
policy scope. Later multi-parent work must make policy, store scope, lineage
coordinate, and finality explicit so "one Crown" means one local Crown under a
policy-scoped surface, not one global singleton for the whole tree.

`SealBlock` currently commits a `crown_lock_transition` evidence ref into the
sealed header, but that ref is not an authority token. The desired next
authority implementation should make sealing a projection of the live Crown
lock transition, not a direct arbitrary block operation.

History entries should preserve at least:

- subject
- procedure/transition/policy
- executor
- operational environment
- observer
- recorder
- proposer
- ruling authority
- admitting authority when applicable
- input/output refs
- occurred/observed/recorded times
- payload ref/hash

Do not collapse these roles into a generic writer, source, or status.

## Current Task Stack

Focus:

- Improve metrics/History CLI usefulness while preserving the projection vs
  authority boundary.

Done recently:

- added `history metrics` and `history preview`
- kept nested monitor commands as aliases
- added latest Prototype 1 campaign fallback
- added cohort metrics and richer operational row fields
- documented degraded lineage in the admission map
- added cohort-aware trajectory projection from rows/cohorts, preserving
  ambiguity instead of collapsing selected rows by generation
- added row-local `dashboard_score_derivation` with explicit
  `rank_relation: "separate_from_dashboard_rank"`
- added structural trajectory deltas for parent, alternative, and previous
  selected comparisons

Likely next patches:

1. Use the new trajectory deltas to build a compact operator readout for
   selected parent-successor progress across generations. Keep this as a
   projection over degraded evidence, not lineage proof or authority.
2. Improve table/JSON view slicing so `--view cohorts` and `--view trajectory`
   return focused payloads.
3. Add or refine selected-vs-alternative cohort comparisons if the current
   trajectory output is still too hard to scan.
4. Later: start live Crown-gated History mutation beside existing records on a
   recovery branch.

No-goals for small CLI/metrics patches:

- do not add a database backend
- do not replace scheduler/journal files
- do not claim sealed History authority
- do not expand algebra unless repeated implementation pressure demands it
- do not add another broad report abstraction

## Patch Discipline

Before implementation, state:

```text
Latent object:
Reduction to avoid:
Files I expect to touch:
Invariants this patch must preserve:
Known gaps this patch will not solve:
Verification commands:
```

During implementation:

- use narrow local carriers when helpers repeat argument clusters
- keep output bounded and inspectable
- use sub-agents for broad record discovery or independent reviews
- preserve source refs and diagnostics when deriving values from mutable or
  degraded evidence
- avoid deriving trajectory from an already-collapsed generation summary; start
  from rows/cohorts so branch structure is still visible
- keep comparison structure in types and fields such as `Delta`, `Basis`, and
  `DeltaState`; do not encode endpoints into long field names
- keep selection evidence separate from Crown authority; `transition_journal`
  is stronger evidence than mutable scheduler projections, but neither is a
  live Crown token

After implementation, state:

```text
This patch strengthens:
This patch only projects:
This patch does not prove:
Verification run:
```

## Review Rubric For Sub-Agents

When using sub-agents to test comprehension, ask them to identify:

- where a proposed implementation collapses authority into projection
- whether a name is carrying structure that should be a type/module/trait
- whether generation is being mistaken for lineage or block height
- whether mutable scheduler/registry/node records are being treated as
  authority
- whether executor, observer, recorder, proposer, admitting authority, and
  ruling authority remain distinct
- whether a CLI command names the surface and operation structurally
- whether branch/fork/merge provenance is preserved or silently flattened

High-quality answers should cite concrete files and describe what the current
implementation proves versus what the docs only intend.

## Verification Commands

Use these as the default quick loop:

```text
cargo fmt --all
cargo test -p ploke-eval history_
cargo test -p ploke-eval metrics
cargo check -p ploke-eval
./target/debug/ploke-eval history --help
./target/debug/ploke-eval history metrics --view cohorts --rows 4
./target/debug/ploke-eval history preview --entries 2 --diagnostics 2
```

Known unrelated warnings currently include unused/dead-code warnings in
`syn_parser`, `inner::Crown`, `inner::LockBox`, and some runner helpers.
