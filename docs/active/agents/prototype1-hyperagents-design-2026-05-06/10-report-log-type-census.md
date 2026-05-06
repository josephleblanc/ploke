# Agent 10 Report/Log Type Census

Date: 2026-05-06

Scope: repo-wide within `ploke-eval`, with adjacent protocol surfaces in
`ploke-protocol`, `ploke-llm`, and `ploke-tui` where they produce or explain
records consumed by evaluation. This is a factual census, not a redesign.

Prior surveys used as anchors:

- `docs/reports/prototype1-record-audit/history-admission-map.md`
- `docs/active/agents/history-surface-admission-review-2026-04-30/reviewer-a.md`
- `docs/active/agents/history-surface-admission-review-2026-04-30/reviewer-b.md`
- existing reports `01`, `03`, `06`, and `07` in this design directory

## Concise Synthesis

The persistence surface is fractured along two axes:

- Authority axis: sealed History blocks are authority; transition journal,
  runner results, branch evaluations, run records, protocol artifacts, logs,
  scheduler state, branch registry, metrics, and reports are evidence or
  projections until admitted.
- Scope axis: per-run, per-child-attempt, per-node latest, per-campaign,
  per-lineage, and per-protocol-procedure records all duplicate core identity
  fields without one current child-selection evidence bundle.

The most useful evidence for child selection today is not the most authoritative
evidence. Current generation-local selection uses operational metrics derived
from `RunRecord`s and copied into `Prototype1BranchEvaluationReport`, then
stored in a `SuccessorDecision` embedded in a successor journal record. Sealed
History proves successor handoff authority, but current live blocks do not yet
admit child evaluation evidence. Protocol artifacts and observation logs are
useful process evidence, but they are not in the default successor-selection
domain.

The strongest stale assumption to avoid is "location implies authority".
`runner-result.json`, `scheduler.json`, `branches.json`, monitor reports, and
history preview rows are convenient discovery surfaces; they are not admission
records. Attempt-scoped child result files, branch evaluation reports, and
compressed run records are better evidence, but still need source digests and
History/manifest refs before archive selection can treat them as durable facts.

## Persistence Topography

Current Prototype 1 campaign-local files live under the parent of
`campaign.json`, mainly:

| File surface | Current path convention | Main producer/reader |
| --- | --- | --- |
| Campaign manifest | `<campaign>/campaign.json` | `CampaignManifest`; `campaign_manifest_path`, `save_campaign_manifest`, `load_campaign_manifest` |
| Prototype root | `<campaign>/prototype1/` | scheduler, branch registry, transition journal, history store, node dirs |
| Scheduler projection | `prototype1/scheduler.json` | `Prototype1SchedulerState`; load/modify/write helpers in `intervention/scheduler.rs` |
| Branch registry projection | `prototype1/branches.json` | `Prototype1BranchRegistry`; `intervention/branch_registry.rs` |
| Transition journal | `prototype1/transition-journal.jsonl` | `PrototypeJournal`; append-only JSONL with `sync_data` |
| History store | `prototype1/history/blocks/segment-000000.jsonl`, `prototype1/history/index/*.jsonl`, `prototype1/history/index/heads.json` | `FsBlockStore`; authority-bearing sealed blocks plus rebuildable indexes |
| Branch evaluation | `prototype1/evaluations/<branch-id>.json` | `Prototype1BranchEvaluationReport`; child evaluation writes, selection reads |
| Node mirror | `prototype1/nodes/<node-id>/node.json` | `Prototype1NodeRecord`; scheduler projection/cache |
| Runner request | `prototype1/nodes/<node-id>/runner-request.json` | `Prototype1RunnerRequest`; child execution input |
| Latest runner result | `prototype1/nodes/<node-id>/runner-result.json` | latest `Prototype1RunnerResult`; mutable projection |
| Attempt result | `prototype1/nodes/<node-id>/results/<runtime-id>.json` | attempt-scoped `Prototype1RunnerResult`; better child evidence |
| Invocation | `prototype1/nodes/<node-id>/invocations/<runtime-id>.json` | `Invocation`, `ChildInvocation`, `SuccessorInvocation` |
| Successor ack/completion | `prototype1/nodes/<node-id>/successor-ready/<runtime-id>.json`, `successor-completion/<runtime-id>.json` | successor runtime writes; monitor/preview read mostly as raw JSON |
| Channels | `prototype1/nodes/<node-id>/channels/<runtime-id>/...` | role-indexed parent/child message transport |
| Streams | `prototype1/nodes/<node-id>/streams/<runtime-id>/stdout.log`, `stderr.log` | successor/child process stdout/stderr |
| Parent identity artifact | `.ploke/prototype1/parent_identity.json` inside a checkout | artifact-carried `ParentIdentity` |
| Run record | run/instance directory `record.json.gz` | `RunRecord`; compressed full eval replay record |
| Agent sidecars | `agent-turn-trace.json`, `agent-turn-summary.json`, `llm-full-responses.jsonl` | runner and full-response tracing |
| Protocol artifacts | run-scoped protocol artifacts directory | `StoredProtocolArtifact` files from `write_protocol_artifact` |
| Observation logs | `~/.ploke-eval/logs/prototype1_observation_<run-id>.jsonl` when `PLOKE_PROTOTYPE1_TRACE_JSONL` is set | tracing JSONL; diagnostic observation surface |

One stale-location risk: older comments and docs describe `RunRecord` as
`runs/{date}/{task_id}/record.json.gz`. The current code also routes runs
through campaign `instances_root`/`batches_root` and run registry identity. Use
the recorded `record_path`/run registry, not a hardcoded date path.

## Type Census

| Family | Main types | Files | Evidence role |
| --- | --- | --- | --- |
| Campaign manifests | `CampaignManifest`, `EvalCampaignPolicy`, `ProtocolCampaignPolicy`, `ResolvedCampaignConfig` | `crates/ploke-eval/src/campaign.rs` | Campaign/eval/protocol context; useful policy context, not child outcome evidence by itself. |
| Run records | `RunRecord`, `TurnRecord`, `LlmResponseRecord`, `RawFullResponseRecord`, `ToolExecutionRecord`, `ToolCallRecord`, `SubmissionArtifactState` | `crates/ploke-eval/src/record.rs` | Best per-run replay and metric source; directly useful for child selection through operational metrics. |
| Runner artifacts | `RunArtifactPaths`, `AgentRunArtifactPaths`, `ExecutionLog`, `RepoStateArtifact`, `IndexingStatusArtifact`, `AgentTurnArtifact`, `ObservedTurnEvent`, `PatchArtifact`, `ToolRequestRecord`, `ToolCompletedRecord`, `ToolFailedRecord` | `crates/ploke-eval/src/runner.rs` | Fragmented sidecars and event captures; useful for diagnostics and derived metrics. |
| Protocol procedure artifacts | `StoredProtocolArtifact`, `StoredProtocolArtifactFile`, `ProtocolArtifactRef`, `ProtocolAggregate`, `ProtocolAggregateReport`, `ProtocolCampaignTriageReport` | `crates/ploke-eval/src/protocol_artifacts.rs`, `protocol_aggregate.rs`, `protocol_report.rs`, `protocol_triage_report.rs` | Adjudicated process evidence; useful for future selection domains, currently mostly projected/reporting. |
| Formal protocol algebra | `StepArtifact`, `SequenceArtifact`, `FanOutArtifact`, `MergeArtifact`, `ProcedureArtifact`, `ProcedureRun`, `StateDisposition`, `EvidencePolicy` | `crates/ploke-protocol/src/core.rs`, `procedure.rs`, `step.rs` | Generic typed procedure carrier; adjacent vocabulary, not directly wired into Prototype 1 selection authority. |
| Tool-call trace | `Trace`, `Call`, `ToolCallNeighborhood`, `ToolCallSequence` | `crates/ploke-protocol/src/tool_calls/trace.rs` | Structured tool-use evidence for protocol review; useful when protocol metrics enter selection. |
| Scheduler/node records | `Prototype1SchedulerState`, `Prototype1NodeRecord`, `Prototype1RunnerRequest`, `Prototype1RunnerResult`, `Prototype1ContinuationDecision`, `Prototype1SearchPolicy` | `crates/ploke-eval/src/intervention/scheduler.rs` | Node discovery and mutable state; latest runner result is a projection, attempt result is stronger evidence. |
| Branch registry | `Prototype1BranchRegistry`, `InterventionSourceNode`, `TreatmentBranchNode`, `TreatmentBranchEvaluationSummary`, `ActiveInterventionTarget` | `crates/ploke-eval/src/intervention/branch_registry.rs` | Branch/source graph projection; useful for discovery and summaries, not sufficient for admission. |
| Branch evaluation reports | `Prototype1BranchEvaluationReport`, compared-instance rows, `Prototype1ComparedInstanceReport` | `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs` | Primary current child evaluation artifact; directly feeds `SelectionInput`, but copies metrics without source digests. |
| Successor selection | `SelectionInput`, `SuccessorDecision`, `SuccessorOutcome`, `DomainFinding`, `Verdict`, operational domain metrics | `crates/ploke-eval/src/successor_selection/*` | Current child-selection decision evidence; generation-local, operational-only by default. |
| Transition journal | `PrototypeJournal`, `JournalEntry`, `Entry`, `BuildEntry`, `SpawnEntry`, `ReadyEntry`, `CompletionEntry`, `ParentStartedEntry`, `ChildArtifactCommittedEntry`, `ActiveCheckoutAdvancedEntry`, `SuccessorHandoffEntry`, `ReplayLog` | `crates/ploke-eval/src/cli/prototype1_state/journal.rs` | Append-only transition evidence; best ordering source, but legacy variant names are storage labels. |
| Typed child records | `Child<State>`, `child::Record`, states `Ready`, `Evaluating`, `ResultWritten` | `crates/ploke-eval/src/cli/prototype1_state/child.rs` | Structural child lifecycle projection into the journal; useful for attempt provenance. |
| Typed successor records | `successor::Record`, successor `State::{Selected, Spawned, Checkout, Ready, TimedOut, ExitedBeforeReady, Completed}` | `crates/ploke-eval/src/cli/prototype1_state/successor.rs` | Handoff path projection; embeds `SuccessorDecision` when selected. |
| Invocation records | `Invocation`, `InvocationAuthority`, `ChildInvocation`, `SuccessorInvocation`, `SuccessorReadyRecord`, `SuccessorCompletionRecord` | `crates/ploke-eval/src/cli/prototype1_state/invocation.rs` | Runtime bootstrap descriptors and acknowledgements; attempt-scoped identity evidence. |
| Parent identity | `ParentIdentity` | `crates/ploke-eval/src/cli/prototype1_state/identity.rs` | Artifact-carried parent identity witness; strongest current artifact identity signal, not a full provenance manifest. |
| History authority | `BlockStore`, `FsBlockStore`, `Block<Open/Sealed>`, `SealedBlockHeader`, `StoredBlock`, `LineageState`, `StoreHead`, `HistoryStateRoot`, `EntryKind`, `EvidenceRef`, `ArtifactRef`, `SurfaceCommitment`, `TreeKeyHash`, `Manifest` | `crates/ploke-eval/src/cli/prototype1_state/history.rs` | Only implemented authority surface for lineage admission and successor handoff. Current live blocks do not yet admit child evaluation evidence. |
| Crown/protocol carriers | `Crown<Ruling>`, `Crown<Locked>`, `LockCrown`, `LineageKey`, `MessageBox`, `LockBox`, `Transition`, `At<File>` | `crates/ploke-eval/src/cli/prototype1_state/inner.rs` | Structural authority and cross-runtime protocol carriers. |
| History preview | `EvidenceStore`, `FsEvidenceStore`, `Document`, `JournalProjection`, `EvidenceClass`, preview source summaries | `crates/ploke-eval/src/cli/prototype1_state/history_preview.rs` | Read-only degraded/provisional import; useful census/reporting, not authority. |
| Metrics/report projections | `metrics::Decision`, `DecisionState`, `report::Report`, `JournalView`, CLI report structs | `crates/ploke-eval/src/cli/prototype1_state/metrics.rs`, `report.rs`, `cli_facing.rs` | Human/operator projections; useful for inspection, not source facts. |
| Observation/log traces | `LoggingGuards`, `ProviderAttempt`, `ProviderRetryDecision`, `ChatHttpErrorTrace`, TUI `FullResponseTraceRecord`, `ChatSessionReport`, `ChatStepReport` | `crates/ploke-eval/src/tracing_setup.rs`, `crates/ploke-llm/src/manager/*`, `crates/ploke-tui/src/llm/manager/*` | Runtime/provider diagnostics; useful for child health classification, not currently selection evidence. |
| Generic artifact ids | `ArtifactId`, `PatchId`, `OperationTarget`, `ProtocolArtifactRef`, `IssueArtifactRef`, `RunArtifactRefs` | `crates/ploke-eval/src/loop_graph.rs`, `protocol_aggregate.rs`, `intervention_issue_aggregate.rs`, `inner/registry.rs` | Provenance hints and graph coordinates; fragmented across subsystems. |

## Duplicate And Fractured Surfaces

1. Attempt result versus latest result.
   `nodes/<node-id>/results/<runtime-id>.json` is attempt scoped, while
   `nodes/<node-id>/runner-result.json` is a latest mutable projection. Both
   carry `Prototype1RunnerResult`; the type itself does not include
   `runtime_id`, so readers must recover attempt identity from path or journal.

2. Child lifecycle has both structural and legacy journal shapes.
   `child::Record` models `Child<Ready/Evaluating/ResultWritten>`, while
   `ReadyEntry`, `CompletionEntry`, and legacy variants such as
   `ChildReady`/`ObserveChild` still exist for replay and compatibility.
   Journal comments correctly warn not to make those variant names the History
   ontology.

3. Successor handoff is split across `successor::Record`, invocation files,
   ready/completion files, old `SuccessorHandoffEntry`, stream logs, and
   `scheduler.last_continuation_decision`. The selected successor decision can
   be embedded in the journal, but the scheduler also stores a mutable singleton
   continuation decision.

4. Branch evaluation is duplicated between the full evaluation report and the
   branch registry summary. The report has compared instances and run record
   paths; the registry summary only has compact disposition counts and is a
   projection.

5. Run evidence is split between `record.json.gz` and runner sidecars. The
   compressed record is intended to consolidate `run.json`, turn trace, repo
   state, indexing status, and conversation, but sidecars remain present and
   useful. Selection should prefer the record or a digest-backed ref, not
   scraped summaries.

6. Protocol evidence has three layers: generic `ploke-protocol` artifacts,
   persisted `StoredProtocolArtifact` JSON files, and projected
   `ProtocolAggregate`/`ProtocolAggregateReport`/triage summaries. The report
   layer is lossy and should not become the source of protocol facts.

7. History has both sealed authority and preview projections. `FsBlockStore`
   appends sealed blocks and maintains indexes; `history_preview` imports
   current files as degraded evidence. Archive selection should cite sealed
   blocks or sealed ingress, not preview rows.

8. Manifest naming is overloaded. `CampaignManifest` is campaign configuration;
   `history::Manifest` is an artifact-local provenance-manifest marker; run
   manifests and batch manifests are separate run/closure surfaces. These should
   not be treated as interchangeable because the names overlap.

9. Logs/traces are diagnostic and path-sensitive. Observation JSONL, full
   response logs, stdout/stderr streams, and provider attempt traces contain
   important failure evidence, but they require span/path joins and are not
   durable admission objects.

## Write And Read Paths By Evidence Strength

Strongest current authority:

- `FsBlockStore::append` writes sealed blocks to
  `prototype1/history/blocks/segment-000000.jsonl`, indexes by hash and
  lineage-height, then updates `heads.json`. Append verifies sealed block hash,
  expected `LineageState`, and local head shape. This is the authority spine.
- `sealed_head_block` can load a sealed head and verify its hash, but currently
  rejects stored blocks with non-empty entries. That means child evidence is not
  currently recoverable from sealed block entries.

Best current child-selection evidence:

- `record_attempt_runner_result` writes the attempt result first, then mirrors
  to latest `runner-result.json`.
- Child evaluation writes `prototype1/evaluations/<branch-id>.json`, then writes
  a compact evaluation summary into `branches.json`.
- `SelectionInput` is built from the child node, branch disposition, evaluation
  artifact path, and parent-vs-child operational metric comparison.
- `SuccessorDecision` records the selected candidate/outcome/findings and is
  embedded in `successor::Record::Selected` in the transition journal.

Useful corroborating evidence:

- `PrototypeJournal::append` writes transition JSONL with `sync_data`.
- `child::Record` and `successor::Record` add structural lifecycle/handoff facts
  to the journal.
- `ParentIdentity` is written into the checkout at
  `.ploke/prototype1/parent_identity.json` and committed as artifact evidence.
- Protocol artifacts are written through `write_protocol_artifact` after run
  identity resolution and subject validation.
- Observation JSONL is written only when `PLOKE_PROTOTYPE1_TRACE_JSONL` is set;
  it is useful for provider/runtime classification.

Mutable or lossy projections:

- `scheduler.json`, `node.json`, `runner-result.json`, `branches.json`,
  `history metrics`, `history preview`, monitor timing reports, CLI state
  reports, and protocol aggregate reports are discovery/reporting surfaces.
  They may carry useful paths or summaries, but should not be treated as
  authority without source refs and admission policy.

## Provenance Carriers

Frequently duplicated identity/provenance fields:

- `campaign_id`, across campaign manifest, scheduler, journal, invocations,
  runner results, branch evaluation reports, protocol/run summaries.
- `node_id`, across scheduler, node records, runner request/result, invocation,
  journal, selection decision, parent identity.
- `generation`, across scheduler, node records, journal entries, parent
  identity, branch evaluation and metrics projections.
- `runtime_id`, across invocation filenames, child/successor journal records,
  ready/completion files, attempt results by path, channels, streams, telemetry.
- `branch_id`, `candidate_id`, `source_state_id`, `parent_branch_id`, across
  branch registry, scheduler, runner request/result, selection, evaluation.
- `record_path`, `run_id`, `subject_id`, across run records, protocol
  artifacts, protocol aggregate/report, branch evaluation rows.
- `model_id`, `provider_slug`, endpoint provenance, across campaign manifest,
  run metadata, protocol artifacts, provider traces.
- artifact/tree identity via `ParentIdentity`, `ArtifactRef`, `TreeKeyHash`,
  `SurfaceCommitment`, `OperationTarget`, `ArtifactId`, `PatchId`.
- evidence refs via raw paths, `EvidenceRef`, `ProtocolArtifactRef`, stream
  paths, observation log paths, full-response trace paths.

Child selection currently consumes only a narrow subset: node/branch/generation,
branch disposition, evaluation artifact path, and operational metric deltas
derived from run records. It does not yet consume sealed History citation,
protocol artifact reliability, provider attempt health, or source record
digests as first-class selection evidence.

## Stale Or Risky Location Assumptions

- Do not assume `runner-result.json` is attempt evidence. It is the latest copy;
  use `results/<runtime-id>.json` when available.
- Do not assume successor completion has a typed read path. It has a typed
  writer and schema, but monitor currently parses completion JSON as raw
  `serde_json::Value`; history preview also imports it as a raw document.
- Do not assume branch registry evaluation summaries are the evaluation
  artifact. They summarize `Prototype1BranchEvaluationReport`.
- Do not assume `History preview` output is History. The preview is explicitly
  read-only and degraded.
- Do not assume `heads.json` is independent authority. It is an index
  projection maintained by `FsBlockStore` after accepted sealed-block append.
- Do not assume `campaign_id`/`generation` uniquely identify lineage or parent
  identity. Current docs and code treat generation as a projection coordinate;
  lineage/head authority lives in History/Crown.
- Do not assume logs can be joined by request id alone. Provider request ids are
  process-local and need span fields/path context for campaign/node/runtime
  attribution.
- Do not assume an artifact-local provenance manifest exists because
  `history::Manifest` exists as a marker. The prior audits and current History
  comments still mark the full manifest as future/missing.

## Evidence Usefulness For Child Selection

Use now:

- Attempt-scoped `Prototype1RunnerResult` for terminal child status.
- `Prototype1BranchEvaluationReport` for baseline/treatment comparison and
  paths to run records.
- `RunRecord` operational metrics for tool failures, patch state, convergence,
  repair-loop behavior, and oracle eligibility.
- `SuccessorDecision` in the successor journal record for the actual
  generation-local selection outcome.
- Transition journal child/successor records for ordering and runtime lifecycle
  corroboration.

Useful but not wired into default selection:

- `StoredProtocolArtifact` and protocol aggregates for process-quality evidence.
- Provider attempt traces and observation JSONL for distinguishing candidate
  quality from runtime/provider failure.
- Full-response sidecars and agent turn artifacts for prompt/response and tool
  replay.
- Parent identity and History sealed block material for proving the candidate
  belonged to the admitted transition system.

Avoid as primary selection facts:

- `scheduler.json` frontier/latest fields.
- `branches.json` compact summaries without the full evaluation report.
- `runner-result.json` when an attempt-scoped result exists.
- CLI report strings, monitor output, metrics tables, and preview rows.

## Final Fact Pattern

There is no single implemented "child selection evidence record" or
"archive admission record" that joins:

```text
campaign/node/generation/runtime/branch
-> child invocation and attempt result
-> branch evaluation report
-> baseline/treatment RunRecord paths and digests
-> protocol artifacts and provider/runtime health
-> SuccessorDecision and continuation decision
-> sealed History/head/surface/artifact citation
```

The pieces exist in enough detail to reconstruct the chain manually, but the
chain crosses at least eight persistence families. For HyperAgents design, the
important fact is that selection-quality evidence and authority evidence are
currently separate: selection is operational and generation-local; History is
local sealed authority for handoff; archive admission over child evidence is
still missing.
