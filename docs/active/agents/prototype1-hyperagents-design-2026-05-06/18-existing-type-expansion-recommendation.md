# Agent 18: Existing Type Expansion Recommendation

Date: 2026-05-06

Scope:
- Read existing reports `08` through `13` in this directory.
- Reports `14` through `17` were not present when this report was written.
- Performed targeted source reads only.
- Source code was not modified.

## Question

If Prototype 1 needs an operator that locates and unpacks all child evaluation
evidence for successor selection and operator-facing reports, which existing
type/module should be expanded first?

## Recommendation

Expand `history_preview::EvidenceStore` / `history_preview::FsEvidenceStore`
first, keeping it read-only and evidence-shaped.

The concrete expansion should be over the existing `Document`,
`EvidencePointer`, and `EvidenceClass` surface in
`crates/ploke-eval/src/cli/prototype1_state/history_preview.rs`, not over CLI
rendering or the sealed History authority path. `FsEvidenceStore` already knows
how to locate the relevant campaign-local files: transition journal, evaluations,
invocations, attempt results, successor ready/completion files, scheduler,
branch registry, node records, runner requests, and latest runner results
(`history_preview.rs:36`, `history_preview.rs:41`, `history_preview.rs:46`,
`history_preview.rs:70`). It already preserves path, line, class, and content
hash through `EvidencePointer` and `Document` (`history_preview.rs:435`,
`history_preview.rs:473`). `metrics` already consumes the same store rather than
inventing its own locator (`metrics.rs:1`, `metrics.rs:44`).

That makes `history_preview` the best first expansion point because the needed
operator is primarily a provenance-preserving discovery and unpacking operation:

```text
campaign manifest
-> filesystem evidence store
-> classified source refs and payload hashes
-> grouped child/runtime/evaluation evidence
-> selection inputs and operator reports as projections
```

This preserves the current authority model. The store can classify and join
mutable, attempt-scoped, journal, and degraded/pre-History evidence without
claiming that the result is sealed History. Selection and reports can consume
the unpacked evidence, while History admission can later cite the same source
refs through `EvidenceRef` when an explicit admission transition exists.

## Why This Carrier Fits

Reports `09` and `10` both identify the same fracture: useful child evidence
is spread across attempt JSON, branch evaluation reports, run records, journal
rows, successor records, protocol artifacts, and mutable projections. They also
rank sealed History as authority, transition journal as pre-History transition
evidence, attempt JSON as runtime-local evidence, and reports/metrics as
projections. Report `10` states that no single implemented record joins:

```text
campaign/node/generation/runtime/branch
-> child invocation and attempt result
-> branch evaluation report
-> baseline/treatment RunRecord paths and digests
-> protocol artifacts and provider/runtime health
-> SuccessorDecision and continuation decision
-> sealed History/head/surface/artifact citation
```

`history_preview::FsEvidenceStore` is the only current module that is already
positioned at that join boundary without being a renderer or an authority writer.
It already separates evidence class from authority treatment:
`TransitionJournal`, `Evaluation`, `Invocation`, `AttemptResult`,
`SuccessorReady`, `SuccessorCompletion`, `Scheduler`, `BranchRegistry`,
`NodeRecord`, `RunnerRequest`, and `RunnerResult` (`history_preview.rs:575`).

The next design move should therefore be to make this store return a richer
typed unpacking over existing source classes, grouped by child identity where
possible:

- node id, branch id, generation, parent node id when recoverable;
- runtime id recovered from invocation/result/successor paths and journal rows;
- evaluation artifact document and branch evaluation report fields;
- attempt result document, latest result projection, and disagreement notes;
- transition journal pointers for child and successor lifecycle records;
- run record paths and protocol artifact refs found inside evaluation reports;
- source hashes for every file/line used;
- authority treatment: sealed, pre-History/degraded, attempt-scoped, mutable
  projection, or report projection.

The exact public type name can wait. The structural placement should not.

## Candidates Rejected

### Expand sealed `History`, `EvidenceRef`, or `ArtifactRef` first

Rejected as the first step.

`history.rs` correctly defines History as the durable authority surface and
warns that scheduler, branch registry, node records, invocation files,
ready/completion files, and monitor reports are evidence or projections until
admitted. `EvidenceRef` and `ArtifactRef` are currently thin stable wrappers
(`history.rs:1345`, `history.rs:1359`). They are appropriate citation targets
for admitted facts, not the first filesystem discovery operator.

Expanding sealed History first would pull mutable and degraded evidence into the
authority module before the admission transition is defined. That would blur the
important separation: locating child evidence is not the same operation as
admitting it into a sealed block. Use `EvidenceRef` later as an output/citation
of the unpacked store, not as the initial scanner.

### Expand report or metrics loaders first

Rejected because they are projections.

`report::Report::load` reads scheduler, branch registry, journal, and evaluation
files to render a provisional campaign report (`report.rs:76`). `metrics` says
explicitly that rows are projections over current evidence and are not sealed
History entries (`metrics.rs:1`). These modules should consume the shared
unpacked evidence operator. If they become the source locator, selection and
reports will drift into separate parsers and duplicate authority assumptions.

### Expand `loop_graph::ArtifactId`, `PatchId`, or `OperationTarget` first

Rejected as necessary vocabulary but the wrong boundary.

`loop_graph` gives durable graph coordinates for artifacts, patches, runtimes,
and operation targets (`loop_graph.rs:66`, `loop_graph.rs:99`,
`loop_graph.rs:132`). Report `13` also identifies these as useful provenance
vocabulary for semantic patch records. They do not, by themselves, know how to
find a child invocation, attempt result, branch evaluation, run record, protocol
artifact, or journal row. Add these coordinates to the unpacked evidence when
recoverable; do not make them responsible for locating evidence.

### Expand protocol or run artifact refs first

Rejected as too narrow for the parent/child join.

`RunArtifactRefs` is strong for one run attempt's files
(`inner/registry.rs:45`). `ProtocolArtifactRef` is strong for protocol artifacts
inside protocol aggregation (`protocol_aggregate.rs:23`), and report `08`
shows that protocol artifacts can supply high-value selection evidence. But
child selection needs to join run/protocol refs to campaign, node, generation,
runtime, branch, journal, evaluation report, and authority status. The protocol
and run refs should be nested evidence loaded by the store expansion, not the
top-level child evidence operator.

### Expand `successor_selection::SelectionInput` first

Rejected as the most tempting wrong answer.

`SelectionInput` is the closest current selection consumer, but it is deliberately
narrow: candidate, branch disposition, one evaluation artifact path, and
operational run comparisons (`successor_selection/evidence.rs:11`). Its
`CandidateRef` carries only node id, branch id, and generation
(`successor_selection/mod.rs:103`). Report `11` shows that the current selector
is generation-local, direct-child successor selection, not archive traversal or
archive parent selection.

Expanding `SelectionInput` into a filesystem locator would mix three roles:
evidence discovery, policy input, and decision replay. It should instead be fed
from the unpacked evidence operator. Later, selection can accept a richer input
or policy-specific projection derived from the shared child evidence surface.

## Recommended Sequencing

1. Expand `history_preview::EvidenceStore` / `FsEvidenceStore` to produce a
   child evidence grouping over existing `Document` and `EvidencePointer`
   records.
2. Have `metrics` and `report` consume that grouping instead of repeating joins.
3. Derive current `SelectionInput` from the same grouping for generation-local
   continuation.
4. Add run/protocol artifact refs as nested unpacked evidence when report
   fields point to them.
5. Admit selected evidence into sealed History only through an explicit History
   transition, with `EvidenceRef`/`ArtifactRef` citations produced from the
   already-classified source refs.

This keeps the operator in the evidence layer, keeps reports and selection as
projections, and keeps sealed History as authority rather than a filesystem
crawler.
