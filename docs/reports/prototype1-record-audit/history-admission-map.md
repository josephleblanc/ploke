# Prototype 1 History Admission Map

This document bridges the existing Prototype 1 persisted-record audits into the
new History model in
`crates/ploke-eval/src/cli/prototype1_state/history.rs`.

It does not repeat the record inventory. Source inventories live in:

- [`campaign-scheduler.md`](campaign-scheduler.md)
- [`cli-monitor.md`](cli-monitor.md)
- [`evaluation-protocol.md`](evaluation-protocol.md)
- [`state-process.md`](state-process.md)
- [`../prototype1-run12-introspection/persisted-data-profile.md`](../prototype1-run12-introspection/persisted-data-profile.md)

The purpose here is to decide what existing records become admissible History
evidence, what remains a projection, and which duplicated fields History should
own, reference, or derive.

## Admission Classes

| Existing source | History treatment | Primary entry kind | Authority level | Notes |
| --- | --- | --- | --- | --- |
| `prototype1/transition-journal.jsonl` | Admit selected events as block entries | `Transition`, `Observation`, `Decision` | Append-only transition evidence, degraded until sealed by History | Best current ordering source. Keep legacy variant names as storage labels, not History ontology. |
| `prototype1/evaluations/*.json` | Admit as evaluation judgment/decision evidence | `Judgment`, `Decision`, `ProcedureRun` | Canonical branch evaluation artifact for current Prototype 1 | Needs schema version and source record digests before long-horizon reliance. |
| `nodes/*/results/<runtime-id>.json` | Admit as attempt-scoped child result evidence | `ProcedureRun`, `Observation` | Better than latest `runner-result.json` because it is attempt scoped | Prefer over latest result copy. |
| `nodes/*/runner-result.json` | Projection/latest pointer | Projection only unless imported with degraded provenance | Mutable/latest convenience copy | Should become a pointer or be ignored by History import when attempt result exists. |
| `nodes/*/invocations/*.json` | Admit as runtime authority/input evidence | `Transition`, `Observation` | Attempt-scoped bootstrap contract | Useful for executor/runtime binding; not enough alone to prove Crown authority. |
| `successor-ready/*.json` | Admit as latch evidence, or ingress if observed after block lock | `Observation`, `Transition` | Process-bound acknowledgement evidence | Must be reconciled with `successor::Record::Ready`. |
| `successor-completion/*.json` | Admit as terminal successor evidence | `Observation`, `ProcedureRun` | Terminal status evidence | Add typed load path before relying on it broadly. |
| `scheduler.json` | Projection/cache | Projection only | Mutable controller state | Useful for final stop reason and frontier view, but not block authority. |
| `branches.json` | Branch graph/catalog projection | Projection plus selected branch evidence when backed by journal/evaluation refs | Mutable registry | History should reference branch/artifact ids and source hashes, not copy the whole registry as authority. |
| `nodes/*/node.json` | Node projection/cache | Projection, occasionally evidence with degraded provenance | Mutable node-local scheduler mirror | Should not override journal or artifact identity. |
| `runner-request.json` | Admit as node execution plan evidence | `Observation`, `ProcedureRun` | Node-scoped execution request | Must be checked against invocation/runtime before use. |
| `.ploke/prototype1/parent_identity.json` | Admit as artifact-carried identity evidence | `Observation`, `Transition` | Best current artifact identity signal | History should hash/reference it at block boundaries. |
| `record.json.gz` | Admit as replay archive for an eval run | `ProcedureRun`, `Observation` | Richest eval/run replay source | History should store refs/digests and selected metrics, not duplicate the whole archive. |
| `agent-turn-trace.json` / `agent-turn-summary.json` | Admit as turn evidence when no better event journal exists | `ProcedureRun`, `Observation` | Snapshot-like event evidence | Prefer normalized turn/tool events from `record.json.gz` when available. |
| Tool call records in `RunRecord` | Admit as tool procedure events or aggregate metrics | `ProcedureRun`, `Observation` | Mechanized run evidence | Preserve request/result pairing and tool-surface provenance. |
| Protocol artifacts | Admit as adjudicated analysis evidence | `Judgment`, `ProcedureRun` | LLM/procedure adjudication evidence | Must preserve procedure, model/executor, admissible input refs, and output hash. |
| Process stream logs | Evidence references only | Usually not direct entries | Raw artifact evidence | Store paths/digests/excerpts only when needed. |
| CLI reports and monitor output | Projection only | Projection | Human-facing view | Do not import as authority unless explicitly saved as an analysis artifact. |

## Deduped Field Ownership

These field families appear across multiple records. The next History importer
should choose one owning representation and treat the rest as references,
checks, or projections.

| Field family | Current duplicate locations | History owner | Import rule |
| --- | --- | --- | --- |
| `campaign_id` | campaign manifest, scheduler, journal entries, invocations, successor records, run/eval reports | Block or entry environment | Store once per block/environment when stable; entries may reference it when imported from external run records. |
| `node_id` | scheduler, node records, runner request/result, journal refs, invocation, branch evaluation reports | Entry subject or artifact/runtime ref | Use as a subject/ref key, not as the only artifact identity. |
| `generation` | scheduler nodes, branch registry source states, journal entries, parent identity, runner records | Block height where it matches Crown epoch; otherwise entry metadata | Do not assume generation equals block height after branching/merge. |
| `runtime_id` | invocation, child/successor journal records, ready/completion files, results path | `ActorRef::Runtime` / executor | Make runtime the executor/observer where it performed the action; keep process ids separate. |
| `pid` | child/successor records, ready files, spawn entries, parent started | Operational environment detail | Never use pid as durable actor identity. |
| parent identity | `.ploke/prototype1/parent_identity.json`, journal snapshots, active checkout records | Artifact identity evidence | Hash/reference the artifact-carried file; treat copied snapshots as corroborating evidence. |
| branch id / candidate id / source state id | branch registry, scheduler nodes, child plans, evaluation reports, journal summaries | Artifact/subject refs | Keep ids as refs; History facts should point to content hashes and artifact refs when available. |
| target path / relpath | branch registry, scheduler node, runner request, journal paths, evaluation report | Entry subject plus artifact evidence | Store as subject when the event is about a file surface; pair with source/proposed hash. |
| proposed/current/applied content hashes | branch registry, materialize journal entries, apply reports | Evidence refs/payload hashes | Prefer hashes over inline content for History. Inline content remains artifact evidence if needed. |
| git commit / branch names | child artifact committed, active checkout advanced, parent identity, branch registry | Artifact refs | Use commit hash as artifact evidence; branch name is a mutable handle. |
| run record path | evaluation report, branch registry summary, run registration, closure state | EvidenceRef | Store path plus digest; use registration for discovery, run record for replay. |
| baseline/treatment campaign ids | evaluation report, branch evaluation summary, run records | Procedure/evaluation context | Keep as evaluation context; do not make treatment campaign a History authority. |
| operational metrics | evaluation report, operational metrics CLI, run record-derived summaries | Derived projection with source refs | Persist metric values only with derivation version and source record digests. |
| branch evaluation disposition/reasons | evaluation report, branch registry latest summary, runner result, observe-child journal | `Decision` entry | Use full evaluation report as source; registry summary is a projection. |
| stop reason / continuation decision | scheduler latest decision, successor selected journal entry, successor stdout | `Decision` entry and block seal context | Prefer journal decision plus scheduler policy; scheduler latest field is projection/current state. |
| tool call ids and parent/request ids | `RunRecord`, agent turn artifact, protocol artifacts | Tool procedure subevent refs | Preserve pairing keys; avoid copying tool request fields into every derived metric row. |
| LLM prompt/response text | run record, full response JSONL, agent turn artifact, protocol artifact inputs | Evidence refs and payload hashes | Store digest/ref and bounded summary; only admit full text by reference. |
| stream paths/stdout/stderr excerpts | spawn records, failure info, runner reports, node stream dirs | Evidence refs | Reference logs; import excerpts only as diagnostic observation payloads. |
| timestamps | almost every record family | Entry observed/recorded/occurred times; block opened/sealed times | Preserve role-specific times rather than collapsing to one `timestamp`. |
| statuses/outcomes | scheduler/node status, runner result disposition, CLI report strings, successor status | Typed entry payload or projection | Convert string statuses to typed facts at import; leave CLI strings as display only. |

## Missing Or Weak Fields

These gaps matter for long-horizon analytics and should be addressed before
relying on History as the sole analysis surface.

- `sealed_by` / committer actor for block sealing is not yet recorded in
  `history.rs`.
- Live `Parent<Ruling>`, `Crown<Locked>`, and `Successor<Admitted>` carriers do
  not yet gate History transitions.
- Branch evaluation reports need schema/version and source record digests.
- Registry summaries should include a path/ref to the full evaluation artifact.
- Latest runner result should become a pointer to an attempt result or be
  ignored when attempt-scoped results exist.
- Successor completion needs a typed reader before monitor/report code relies on
  it.
- Tool surface, model, prompt, and full-response evidence should be referenced
  consistently for LLM/protocol-derived judgments.
- Metrics need derivation version and source digests before comparisons can be
  treated as durable facts rather than convenient snapshots.

## First Import Shape

The first read-only History aggregate should not seal real authority blocks. It
should produce a preview with explicit degraded provenance:

```text
campaign root
-> source refs and payload hashes
-> one provisional block per observed generation/Crown epoch
-> admitted-preview entries from journal/evaluation/run artifacts
-> projection rows for scheduler/branch/frontier summaries
-> rejected/deferred evidence list with reasons
```

This lets us inspect the data with `jq` or a finite CLI report before wiring
live History writes. Once the preview exposes useful analytics and missing
fields, live code can write History alongside the existing records on a recovery
branch.

## Implemented Preview Surface

As of 2026-04-28, the first read-only preview is implemented in
`crates/ploke-eval/src/cli/prototype1_state/history_preview.rs` and exposed
through:

```text
cargo run -p ploke-eval -- history preview --campaign <campaign-id>
```

The older `loop prototype1-monitor ... history-preview` path remains a
compatibility alias, but new operator-facing examples should use
`history preview`: `history` names the record surface, and `preview` names the
operation over that surface.

The command preserves the old full table/JSON behavior by default, and also
supports bounded inspection for iterative development:

```text
history-preview --entries 5 --diagnostics 5
history-preview --entry 80
history-preview --format json --entries 0 --diagnostics 0
```

The importer has a narrow `EvidenceStore` trait and a filesystem-backed
implementation for the current campaign layout. It imports transition journal
lines plus the adjacent JSON evidence classes listed in this map. Adjacent JSON
documents now contribute lightweight typed facts when the fields are present:
`node_id`, `generation`, `runtime_id`, `branch_id`, timestamps, status and
disposition fields, evaluation paths, runner result paths, baseline/treatment
record paths, and basic graph refs such as source state, base artifact, and
patch id.

Current block placement remains provisional. The preview builds a node index
from `nodes/*/node.json` and uses it to infer generation for invocation,
successor, evaluation, and other adjacent records when possible. This improves
analysis without changing the authority claim: mutable node records are still
projection evidence and cannot override the transition journal or a future
sealed History block.

On `p1-3gen-15nodes-run12`, the preview currently reports 97 entries and 27
diagnostics. The reduction from the earlier raw import diagnostics comes from
field extraction rather than a stronger trust claim. Remaining diagnostics
mostly identify missing generation/timestamp data in legacy journal variants
or fields hidden behind module-private record surfaces.

Important limits still hold:

- no live `Crown<Locked>` or `Block<Sealed>` writes happen here;
- no JSON document imported by the preview becomes authoritative History;
- `runner-result.json`, `scheduler.json`, `branches.json`, and node records
  remain projections or degraded evidence;
- successor ready/completion records may later need ingress treatment once the
  Crown lock boundary is live;
- per-entry operational environment is still partial and should be expanded
  before long-horizon claims depend on it.

## Implemented Metrics Projection

As of 2026-04-28, the first finite metrics dashboard is implemented in
`crates/ploke-eval/src/cli/prototype1_state/metrics.rs` and exposed through:

```text
cargo run -p ploke-eval -- history metrics --campaign <campaign-id>
```

Despite living under the `history` command, the output is intentionally labeled
`prototype1 metrics projection`. It is not sealed History and does not upgrade
mutable scheduler, branch registry, or node-local records into authority.

If `--campaign` is omitted, `history metrics` and `history preview` resolve the
campaign from the current parent identity, then the active selection, then the
most recently modified Prototype 1 campaign under `~/.ploke-eval/campaigns`.

Bounded inspection examples:

```text
history metrics --rows 12
history metrics --generation 2 --rows 5
history metrics --view trajectory --format json
```

The projection currently derives:

- node rows keyed by generation, node, runtime, branch, status, disposition,
  selection source, dashboard rank, dashboard score, evaluation refs,
  tool-call totals, patch-attempt counts, patch apply states, submission
  artifact states, partial patch failures, same-file retry counts, abort
  counts, and repair-loop counts;
- generation summaries with evaluated-node counts, selected node, selection
  authority, top-ranked node under the dashboard heuristic, score delta, and
  tool-failure rate;
- cohort summaries grouped by parent node and generation, with lineage left
  explicit as unavailable until current records carry a stable lineage id;
- a selected-by-generation projection that reports continuity across
  `parent_node_id` where current records make that check possible.

Selection evidence is explicit. Transition-journal selections attach their
JSONL source ref and are marked `transition_journal`; scheduler or registry
selection observations are marked `mutable_projection`. This is required
because scheduler and registry records are useful current-state projections but
not durable History authority.

Dashboard score and rank are local analysis heuristics, not oracle truth and
not the historical selection policy. The score currently favors evaluated
rows, `keep`, oracle eligibility, convergence, nonempty submission artifacts,
applied patches, and fewer failed tool calls. Future work should either derive
ranking from the same policy evidence used by the Parent or record this
dashboard heuristic as a versioned analysis procedure with stronger tests.

On `p1-3gen-15nodes-run12`, the projection currently shows 10 node rows and
four generations. The selected-by-generation projection shows gen1 `keep`,
gen2 `reject`, and gen3 `keep`, with selected-node tool failure rates around
0.27-0.30. The oracle-oriented counts remain zero in this prototype run.

The current cohort view is exposed through:

```text
history metrics --view cohorts
```

This is the first branch-aware metrics shape, but it is still degraded:
current records support grouping by `(parent_node_id, generation)` while the
lineage coordinate is absent. The projection records that absence as a
diagnostic rather than folding it into campaign id or generation.

## Algebraic Import Model

The next implementation pass should avoid new flattened names such as
`JournalView` or `EvaluationView`. The intended shape is now encoded in
`crates/ploke-eval/src/algebra/` and should be used as the general vocabulary
for read-only report and History-import work:

- `ReadSurface<Source>` bounds access to a source of records or graph facts.
- `Projection<Source>` maps a source to an image and declares the kernel, meaning
  the distinctions collapsed by that projection.
- `Projected<Source>` is a value that carries a projection, image, and kernel.
- `Fiber<P, Image>` is only a descriptor for the inverse image of one projection
  result; report and History types should not be forced to store it directly.
- `Provenance` names the producer and subject of a claim across runtime
  boundaries.
- `Witnessed` means a value carries provenance; it does not imply the value is
  projected.
- `VerifiableWith<R>: Provenance` checks that provenance against a concrete
  resolver/store/tree.
- `ResolveWith<R>: VerifiableWith<R>` recovers the source representative named
  by that provenance.
- `Pullback<L, R, K>` composes independently produced values only when their
  provenance keys agree.

This gives the importer a precise split:

```text
payload/image       = what the report or projection says
kernel              = what the projection forgot
provenance          = who/what produced the claim and what subject it names
verification        = resolver-backed check that the provenance still holds
resolution          = resolver-backed recovery of the source representative
pullback            = composition over matching provenance keys
History admission   = policy decision over projected and/or witnessed values
```

History should require verifiable provenance for admitted entries. It should
not assume recomputability unless a stronger capability is added for that
specific provenance type. This matters because a mechanical projection over
JSON records, an LLM adjudication report, and a human note can all be
`Witnessed` while having different verification, resolution, replay, and
recompute properties.

## Patch Order

1. Add a finite Prototype 1 report command that reads the current authoritative
   sources in the order documented by the run12 profile.
2. Add JSON output for that report whose shape is close to the provisional
   History aggregate.
3. Add source refs and payload digests where the report currently has only
   paths or copied summaries.
4. Add read-only History preview export over one completed campaign.
5. Wire live History writes beside the existing records and compare both
   surfaces on a short run.
