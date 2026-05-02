# Drift

## Domain

Cross-record consistency across scheduler, branch registry, node records, runner request/result, invocations, results, transition journal, parent identity, and History store.

## Questions To Answer

- During a 5-10 generation run, which record is the strongest current answer
  for "who is the active Parent now": the artifact-carried
  `.ploke/prototype1/parent_identity.json`, the scheduler selected trajectory,
  the latest `ActiveCheckoutAdvanced` journal record, the successor invocation,
  or the sealed History head?
- For the current parent generation, do `parent_identity.node_id`,
  `scheduler.nodes[*].node_id`, `node.json`, `runner-request.json`,
  `branches.json`, and the latest `ParentStarted` journal record agree on
  `campaign_id`, `node_id`, `generation`, `branch_id`, `parent_node_id`, and
  `active_parent_root`?
- For each child runtime attempt, is there exactly one invocation record, one
  child ready/evaluating/result-written journal path, one
  `nodes/*/results/<runtime-id>.json`, and one latest `runner-result.json`
  projection whose `node_id`, `branch_id`, `generation`, `status`, and
  `disposition` match?
- Did the branch registry state for the selected branch match the scheduler node
  that was executed: `source_state_id`, `target_relpath`, `candidate_id`,
  `patch_id`, `derived_artifact_id`, `latest_evaluation`, and selected/active
  branch fields?
- When a successor is selected, did the observed order hold: selection decision,
  active checkout advance, parent identity committed into the selected Artifact,
  History block sealed/appended, successor invocation written, successor spawned,
  ready acknowledgement observed, and successor parent start recorded?
- Are mutable projections behind or ahead of append-only evidence? Examples:
  scheduler says `Succeeded` but no attempt result exists; registry says selected
  but no evaluation or successor-selection journal exists; ready file exists
  without a successor journal record; History head advanced but no matching
  checkout/handoff operational evidence is present.
- Did continuation stop/continue for the reason implied by the selected branch
  evaluation and scheduler policy, or did a projection overwrite make the final
  decision ambiguous?
- Longer runs need lineage-aware questions: when generation, block height,
  parent node, branch id, and artifact/tree key diverge, which lineage head is a
  runtime trying to advance?
- Longer runs also need fork/rollback questions: did two runtimes attempt to
  advance the same lineage head, was a late successor/child observation written
  after Crown lock, and was it imported as ingress rather than silently folded
  into the sealed epoch?
- Longer runs need durability questions: can we recompute a digest bundle for a
  generation/turn from source refs, and can we explain every mismatch without
  reading mutable latest files as authority?

## Already Answered By Persisted Data

- `scheduler.json` answers current controller projection questions: policy,
  frontier/completed/failed ids, last continuation decision, and a mutable copy
  of every node record. The node schema includes node/parent/generation,
  source/branch/candidate ids, target path, node/workspace/binary/request/result
  paths, status, and timestamps.
- `nodes/*/node.json` and `nodes/*/runner-request.json` persist the node-local
  execution plan. They answer what workspace and binary path the parent intended
  for that node and which CLI arguments were planned.
- `branches.json` answers the mutable branch-catalog view: source nodes,
  selected branch per source, active target per relpath, proposed/applied content
  hashes, patch/artifact ids when present, and the latest inline evaluation
  summary.
- `nodes/*/results/<runtime-id>.json` already gives the best attempt-scoped
  child result. `nodes/*/runner-result.json` answers only the latest-result
  projection for the node.
- `nodes/*/invocations/<runtime-id>.json` records role (`child` or
  `successor`), campaign, node, runtime id, journal path, and for successors the
  stable active parent root.
- `nodes/*/successor-ready/<runtime-id>.json` and
  `nodes/*/successor-completion/<runtime-id>.json` answer whether a successor
  acknowledged handoff and whether its bounded controller turn completed.
- `prototype1/transition-journal.jsonl` is the best current ordering evidence
  for materialization/build/spawn/ready/observe-child, parent start, resource
  samples, artifact commit, active checkout advance, successor selection,
  successor spawn, and successor handoff records.
- `.ploke/prototype1/parent_identity.json` is the strongest current
  artifact-carried identity signal for the runtime hydrated from a checkout.
- `prototype1/history/blocks/segment-000000.jsonl` plus
  `prototype1/history/index/{by-hash.jsonl,by-lineage-height.jsonl,heads.json}`
  answers which sealed History block the local store accepted for a lineage and
  what local state root/head proof it was opened from.

## Partially Derivable

- Child attempt consistency is mostly derivable by joining
  `runtime_id` across invocation path, child/successor journal records, ready
  files, and attempt-result filenames, then joining `node_id` back to
  scheduler/node/request/registry records. The weak point is that
  `Prototype1RunnerResult` does not carry `runtime_id`; for attempt results the
  runtime is currently inferred from the filename.
- Parent/successor continuity is partly derivable by joining selected branch,
  node id, parent identity, active checkout journal, successor invocation,
  successor ready/completion records, and History head. It is not one atomic
  persisted fact.
- Generation continuity is derivable for most records through direct generation
  fields or node/branch indexes, but this is a projection coordinate, not block
  height or lineage identity.
- Selection drift is partly derivable from scheduler `last_continuation_decision`,
  branch registry selected/active branch state, successor selection journal
  records, evaluation reports, and runner results. The source strength differs:
  scheduler/registry are mutable projections, while journal/evaluation/attempt
  results are better evidence.
- Active checkout drift is partly derivable from `ActiveCheckoutAdvanced`,
  successor checkout journal records, current parent identity, and History
  sealed-head tree/surface validation. Existing records do not provide one
  compact "checkout advanced from artifact A to artifact B under History head H"
  event.
- Report and history-preview code can discover many existing classes and infer
  lightweight facts, but preview still marks scheduler as projection, branch
  registry as projection plus refs, node as projection/degraded evidence, and
  latest runner-result as projection unless attempt result is missing.

## Requires New Logging

- A single tracing-backed JSONL operational event should record each
  transition-boundary write as one fact with source refs, not as another
  domain-specific sidecar file. The minimum fields are:
  `schema_version`, `occurred_at`, `recorded_at`, `campaign_id`, optional
  `lineage_id`, optional `history_head_before`, optional `history_head_after`,
  `parent_id`, `node_id`, `generation`, `runtime_id`, `role`, `state_before`,
  `state_after`, `operation`, `disposition`, `source_refs`, `output_refs`,
  `payload_hashes`, `active_parent_root`, `workspace_root`, `artifact_ref`,
  `branch_id`, `patch_id`, `evaluation_ref`, `runner_result_ref`,
  `invocation_ref`, `journal_ref`, and `projection_writes`.
- Log projection-write bundles so operators can tell that a scheduler update,
  node update, runner-result latest copy, branch-registry update, and journal
  append belong to the same logical transition.
- Log History append correlation: block hash/height, opened state root,
  predecessor head, selected successor runtime/artifact, active checkout
  root/tree/surface, and the invocation/ready paths used as transport evidence.
- Log explicit drift checks at boundaries: mismatched scheduler/node/request,
  missing attempt result for a succeeded node, latest runner result disagreeing
  with attempt result, registry selection disagreeing with evaluated branch,
  successor ready without matching spawn/handoff, or current checkout identity
  disagreeing with History head validation.
- Log late/backchannel classification after Crown lock: ready/completion/result
  evidence observed after the sealed boundary should be recorded as ingress
  candidate evidence rather than in-epoch evidence.
- Do not log full prompts, stdout/stderr bodies, proposed file contents, or full
  evaluation artifacts inline; record path/digest/excerpt refs.

## Natural Recording Surface

- The natural surface is a small operational introspection helper used at typed
  transition boundaries, likely backed by `tracing` events plus one shared JSONL
  sink under `prototype1/`. It should live beside the existing transition
  boundary code, not inside every ad hoc file writer.
- Good call sites are where code already crosses role/state or durable boundary:
  parent start; child plan lock/unlock; node registration; workspace staged;
  child binary built/failed; child spawned/ready/result-written/observed;
  evaluation report persisted; branch selected; continuation decision recorded;
  active checkout advanced; History block appended; successor invocation
  written; successor spawned/ready/completed; cleanup completed/failed.
- The helper should accept structural carriers such as `Parent<Ready>`,
  `Parent<Selectable>`, `Parent<Retired>`, `Child<Ready>`/result-written, and
  History store append results where available. It should not introduce new
  flattened ontology such as "drift heartbeat" or "handoff trace entry".
- Existing `transition-journal.jsonl` remains useful transition evidence, but
  it mixes legacy storage labels with semantic events. The uniform operational
  event should cite journal refs and projection writes while keeping History
  admission separate.
- The History store remains the authority surface for sealed lineage facts. The
  operational event can point to block hash/height and source refs; it must not
  be described as admitting a child, advancing a Crown, or proving handoff.

## Essential

- One event per transition boundary with enough refs to reconcile all mutated
  records touched by that transition.
- Stable correlation fields: `campaign_id`, `lineage_id` when known,
  `parent_id`, `node_id`, `generation`, `runtime_id`, `role`, and the
  transition/state name.
- Source/output refs with payload digests for scheduler, branch registry, node,
  runner request/result, attempt result, invocation, ready/completion, journal
  entry, parent identity, active checkout, and History block.
- Explicit source-strength/authority classification: projection, degraded
  evidence, attempt-scoped evidence, transition evidence, sealed History.
- Drift diagnostics for mismatched identities, missing paired records, stale
  mutable projections, and out-of-order successor/handoff evidence.
- History correlation fields for successor handoff: expected state root,
  predecessor head, appended block hash/height, selected successor runtime,
  selected artifact/tree/surface refs, and active checkout root.
- Role-specific times: observed/occurred/recorded and block opened/sealed where
  available, not one overloaded timestamp.

## Nice To Have

- A digest bundle for each generation/parent turn that summarizes all source
  refs used by report/preview/metrics projections.
- A compact operator-facing drift summary: latest parent, active checkout,
  selected successor, open/missing pairs, stale projection count, and History
  head correlation.
- Per-transition latency fields for time between invocation, ready, result,
  parent observation, checkout advance, History append, and successor ready.
- Cleanup/resource observations tied to node/workspace/runtime refs, especially
  retained worktrees, node-local targets, binaries, and streams.
- Projection derivation ids for report/metrics/history-preview rows so old
  dashboards can be compared without implying authority.
- Optional JSON pointers into large artifacts for the exact evaluation/report
  fields that drove selection.

## Too Granular Or Noisy

- Full scheduler/registry/node JSON payloads embedded into every event. Store
  source refs and hashes instead.
- Per-poll successor-ready wait events. Record start, timeout, ready, or
  exited-before-ready once.
- Every stdout/stderr line, cargo diagnostic, LLM token chunk, or full prompt
  body. Use stream paths, digest, bounded excerpts, and run-record refs.
- File-by-file content hashes for all unrelated repo files during each drift
  check. Use artifact/tree/surface commitments at transition boundaries.
- Repeating static policy/config fields in every event when a campaign/block
  environment ref would identify them.
- Treating every mutable projection write as a History candidate. Most are
  useful only to reconcile the operational state of the loop.

## Source Notes

- `crates/ploke-eval/src/cli/prototype1_state/mod.rs:33-76` defines Artifact,
  Runtime, Journal, and History, and separates authority from paths, process
  ids, scheduler snapshots, branch registry, CLI reports, and other mutable
  projections.
- `crates/ploke-eval/src/cli/prototype1_state/mod.rs:161-211` defines the
  stable active checkout / temporary child checkout / successor handoff shape
  used by this drift survey.
- `crates/ploke-eval/src/cli/prototype1_state/history.rs:24-46` states that the
  live loop still uses transition journals, invocation files, ready files, and
  mutable projections as evidence, not History authority.
- `crates/ploke-eval/src/cli/prototype1_state/history.rs:325-329` requires
  private typestate carriers and loader transitions rather than implicit
  deserialization constructors.
- `crates/ploke-eval/src/cli/prototype1_state/history.rs:352-372` records the
  remaining live-wiring gaps and says existing journals/reports must be imported
  with degraded provenance.
- `crates/ploke-eval/src/cli/prototype1_state/history.rs:587-618` defines
  `BlockStore::append` as the only semantic operation that may advance a lineage
  head; `heads.json` is a projection.
- `crates/ploke-eval/src/cli/prototype1_state/history.rs:831-875` appends sealed
  blocks to the segment and indexes, then updates the local heads projection.
- `crates/ploke-eval/src/cli/prototype1_state/history.rs:878-897` derives
  `LineageState` and absent/present head proofs from the local heads map.
- `crates/ploke-eval/src/cli/prototype1_state/parent.rs:72-87` keeps
  `Startup<Validated>` fields private so transport evidence cannot be converted
  into parent readiness by convention.
- `crates/ploke-eval/src/cli/prototype1_state/parent.rs:386-424` checks parent
  identity against active checkout and scheduler node fields.
- `crates/ploke-eval/src/cli/prototype1_state/parent.rs:481-521` verifies
  successor startup against the sealed History head, current clean Artifact
  tree, and surface commitment.
- `crates/ploke-eval/src/intervention/scheduler.rs:75-155` defines node,
  runner-result, runner-request, and scheduler persisted fields.
- `crates/ploke-eval/src/intervention/scheduler.rs:372-443` writes
  `scheduler.json`, `node.json`, `runner-request.json`, and `runner-result.json`.
- `crates/ploke-eval/src/intervention/scheduler.rs:515-613` mutates node
  status and records latest runner result, making scheduler/node/result useful
  projections but possible drift sites.
- `crates/ploke-eval/src/intervention/scheduler.rs:627-685` computes and
  persists continuation decisions.
- `crates/ploke-eval/src/intervention/branch_registry.rs:28-108` defines
  branch/source/active-target registry fields.
- `crates/ploke-eval/src/intervention/branch_registry.rs:216-349`,
  `:352-449`, `:452-544`, and `:682-710` write synthesized, applied,
  selected, and latest-evaluation branch registry state.
- `crates/ploke-eval/src/cli/prototype1_state/journal.rs:198-212` records
  parent start; `:281-330` records child artifact commit, active checkout
  advance, and successor handoff legacy storage labels.
- `crates/ploke-eval/src/cli/prototype1_state/journal.rs:332-353` lists the
  JSONL journal envelope variants; `:655-688` appends and fsyncs the journal.
- `crates/ploke-eval/src/cli/prototype1_state/invocation.rs:93-105` defines the
  persisted invocation fields; `:316-334` defines invocation and attempt-result
  paths.
- `crates/ploke-eval/src/cli/prototype1_state/invocation.rs:389-400` keeps
  successor invocation writing downstream of the retired-parent boundary.
- `crates/ploke-eval/src/cli/prototype1_state/invocation.rs:402-511` defines
  successor ready/completion records and write/read paths.
- `crates/ploke-eval/src/cli/prototype1_state/identity.rs:16-38` defines the
  artifact-carried parent identity; `:62-95` validates command/campaign/node
  identity; `:140-158` writes it into a checkout.
- `crates/ploke-eval/src/cli/prototype1_process.rs:496-650` installs the
  selected successor Artifact into the stable active checkout and appends
  successor checkout / active checkout advanced journal records.
- `crates/ploke-eval/src/cli/prototype1_process.rs:933-1099` seals/appends the
  History block before successor spawn, writes successor invocation, records
  spawned/ready/timeout/exit evidence, and returns a retired parent.
- `crates/ploke-eval/src/cli/prototype1_process.rs:1300-1310` writes both the
  attempt-scoped result and latest runner-result projection.
- `crates/ploke-eval/src/cli/prototype1_process.rs:1628-1724` loads child
  invocation, records child ready/evaluating/result-written, and persists the
  attempt result.
- `crates/ploke-eval/src/cli/prototype1_state/history_preview.rs:70-123`
  discovers scheduler, branch registry, evaluations, invocations, attempt
  results, successor ready/completion, node records, runner requests, and latest
  runner results.
- `crates/ploke-eval/src/cli/prototype1_state/history_preview.rs:606-619`
  classifies scheduler as projection, branch registry as projection plus refs,
  node records as projection/degraded evidence, runner request as raw evidence,
  and latest runner-result as projection unless attempt result is missing.
- `crates/ploke-eval/src/cli/prototype1_state/history_preview.rs:835-850`
  explicitly notes that `SuccessorHandoffEntry` lacks generation.
- `crates/ploke-eval/src/cli/prototype1_state/history_preview.rs:1284-1335`
  prefers attempt-scoped results and labels latest runner-result as mutable
  projection.
- `docs/reports/prototype1-record-audit/history-admission-map.md:31-48`
  classifies each existing record family by History treatment.
- `docs/reports/prototype1-record-audit/history-admission-map.md:56-77`
  lists duplicated field families and intended History ownership.
- `docs/reports/prototype1-record-audit/history-admission-map.md:84-106`
  lists missing/weak fields, including `sealed_by`, registry evaluation refs,
  latest runner-result pointer behavior, successor completion reader, and source
  digests.
- `docs/reports/prototype1-record-audit/2026-04-29-record-emission-sites-audit.md:15-27`
  inventories emitted record families; `:28-35` identifies dual writes and
  mutable projection drift risks.
- `docs/reports/prototype1-record-audit/2026-04-29-monitor-report-coverage-audit.md:18-31`
  notes that monitor report omits node/request/result/invocation/successor and
  parent identity payloads.
- `docs/reports/prototype1-record-audit/2026-04-29-history-crown-introspection-audit.md:114-125`
  lists authority/provenance gaps relevant to drift, including missing
  `sealed_by`, late/backchannel successor classification, and absent stable
  lineage ids.
