# Authority

## Domain

Crown, Parent role/state, lineage coordinate, and ruling authority health.

## Questions To Answer

- During a 5-10 generation run, which Runtime is allowed to act as
  `Parent<Ruling>` for a given lineage right now?
- What lineage coordinate is being advanced, and what sealed head/root was
  observed before the Parent tried to advance it?
- Did the outgoing Parent retire its ruling role at the same boundary where it
  locked and sealed successor authority?
- Was the sealed block appended before successor spawn, and did the appended
  block cite the expected predecessor head or a valid local genesis absence?
- Did the incoming successor verify the current clean Artifact tree and surface
  commitment against the sealed head before entering the parent path?
- Is the system in a valid zero-ruler handoff window, a stale-head conflict, or
  an accidental overlapping-ruler condition for the same lineage?
- Are scheduler/branch/ready/invocation records only being used as evidence and
  transport, or is some monitor path accidentally treating them as Crown
  authority?
- For longer runs: can two lineages reference the same Artifact without being
  collapsed into one authority chain?
- For longer runs: when a block append is rejected as stale or wrong-parent, can
  operators see the competing observed head and the proposed block identity?
- For longer runs: which late successor-ready/completion observations arrived
  after Crown lock and therefore need ingress/import treatment rather than
  sealed-block mutation?
- For longer runs: when the policy-bearing surface changes, can the run explain
  whether this was an ordinary invalid successor, an explicit protocol upgrade,
  or an out-of-system execution?

## Already Answered By Persisted Data

- Sealed History blocks persist the lineage coordinate, lineage-local height,
  parent block hashes, opened-from state root, opening authority, opened/ruling
  actor refs, opened Artifact ref, policy/procedure label, surface commitment,
  selected successor, active Artifact, Crown-lock transition ref, sealed time,
  entry count/root, and block hash.
- The filesystem block store persists sealed blocks to a JSONL segment and
  rebuildable indexes, and advances `heads.json` only after append validation.
  This answers "what sealed head did this local store accept?"
- Append validation already rejects stale observed state, wrong lineage,
  duplicate genesis, non-consecutive height, and wrong predecessor head.
- Successor startup from predecessor History verifies the current clean Artifact
  tree and current surface commitment against the sealed head before converting
  startup evidence into `Parent<Ready>`.
- Parent identity is persisted as an Artifact-carried identity file and records
  campaign, node, generation, branch, and predecessor-ish context. It is useful
  identity evidence, not a standalone Crown.
- Transition journal records already show parent-started, child Artifact
  committed, active checkout advanced, successor spawned/ready/handoff, and
  related paths. These answer "what evidence did the controller write?", not
  "what did History admit?"

## Partially Derivable

- Current likely ruler can be inferred from parent identity, successor
  invocation/ready/completion files, journal records, and the latest sealed
  History head, but there is no durable operational event that says
  "this Runtime entered `Parent<Ruling>` for lineage L at block H."
- Handoff ordering can be reconstructed from block `sealed_at`, block-store
  append location, successor spawn record, ready file, and
  `SuccessorHandoffEntry`, but the zero-ruler interval and parent-retired moment
  are not first-class operational facts.
- Lineage is currently represented through campaign-derived `LineageId` /
  `LineageKey`. That is enough for the single-lineage prototype but only a
  placeholder for multi-lineage authority; generation and parent-node chains are
  projections, not lineage identity.
- Selection can be correlated through branch registry, scheduler continuation
  decision, journal entries, selected successor, and Artifact refs, but
  scheduler/registry selections remain mutable projections unless admitted into
  sealed History.
- Stale-head and wrong-parent conflicts are enforced by `BlockStore::append`,
  but operators get the error path rather than a compact persisted operational
  event with observed head, proposed block, and conflict reason.
- Successor-ready and successor-completion timing is visible as files/journal
  facts, but deciding whether they are in-epoch evidence or ingress depends on a
  Crown-lock boundary that is not logged as a uniform operational event.

## Requires New Logging

- Durable operational event for startup validation: `lineage_id`,
  `parent_id`, `parent_node_id`, `generation`, `runtime_id` when available,
  `startup_kind` (`genesis` or `predecessor`), observed `store_head`,
  `history_state_root`, current Artifact tree hash, current surface roots,
  outcome, and rejection reason.
- Durable operational event for authority transition:
  `Parent<Selectable> -> Parent<Retired> + Crown<Locked> -> Block<Sealed>`,
  including `lineage_id`, outgoing parent actor, selected successor runtime,
  selected successor Artifact, active Artifact/tree key, block height,
  predecessor hash, Crown-lock transition ref, sealed block hash, and outcome.
- Durable operational event for block append:
  expected `LineageState`, accepted/rejected store head, proposed block hash,
  block height, parent hashes, append location, and stale/wrong-head error.
- Durable operational event for successor launch and acknowledgement:
  successor runtime id, pid, invocation path, ready path, stream refs, active
  parent root, binary ref, whether the ready record parsed and matched the
  invocation/runtime, and whether the outgoing parent is already retired.
- Durable operational event for successor entry into Parent role:
  verified sealed head hash, current Artifact/surface verification result,
  parent identity used, and `Parent<Ready>`/`Parent<Ruling>` admission outcome.
- Durable operational event for late/backchannel authority evidence:
  event source, observed time, prior sealed block hash, target lineage/block if
  known, and whether it was deferred to ingress/import.
- These should be one shared operational JSONL event shape, ideally backed by
  structured `tracing`, with optional fields. Do not add separate Crown,
  Parent, lineage, ready, scheduler, and ingress files.

## Natural Recording Surface

- A small transition-boundary helper on the operational introspection layer,
  e.g. `log_step()` / `log_result()` over one JSONL event type, called from the
  authority transitions rather than hand-built at every call site.
- Startup surfaces: `Startup<Genesis>::from_history`,
  `Startup<Predecessor>::from_history`, and `Parent<Checked>::ready`, because
  these are where transport/identity evidence becomes Parent readiness.
- Crown/seal surface: `LockCrown::seal_block_with_artifact`, because it is the
  structural boundary where `Parent<Selectable>` retires, `Crown<Ruling>` admits
  Artifact evidence, `Crown<Locked>` seals, and `Block<Sealed>` is produced.
- Store surface: `BlockStore::append` / `FsBlockStore::append`, because it is
  the only semantic operation that advances the local lineage head.
- Process handoff surface: `spawn_and_handoff_prototype1_successor`, because it
  sequences sealed block append, invocation creation, successor spawn, ready
  wait, and journal handoff evidence.
- Successor startup surface: predecessor startup verification, because it is
  where the incoming Runtime checks sealed Artifact/surface commitments before
  entering the Parent path.

## Essential

- `lineage_id` and explicit authority coordinate; do not substitute branch,
  worktree, process id, generation, or Artifact id alone.
- `role` and `state` as structured fields, for example Parent/Selectable,
  Parent/Retired, Crown/Locked, Startup/Predecessor, rather than flattened event
  names.
- `event_kind` / `transition_kind`, timestamp, outcome, and error class.
- `parent_id`, `parent_node_id`, `generation`, `runtime_id`, and pid only as
  actor/environment fields.
- observed `HistoryStateRoot`, `StoreHead` before append, block height,
  predecessor hash, proposed/accepted block hash, and append location.
- selected successor runtime and Artifact, active Artifact/tree key, and compact
  surface commitment roots.
- source refs to invocation, ready, journal, block segment, parent identity, and
  binary/stream paths when applicable.
- authority status field that distinguishes `sealed_history`, `transport`,
  `mutable_projection`, `degraded_evidence`, and `ingress_deferred`.

## Nice To Have

- Duration fields for startup validation, block sealing, append, successor
  spawn, and ready wait.
- Compact before/after checkout refs and git commit refs for operator
  correlation.
- Branch/scheduler selection refs as evidence/source-strength fields, clearly
  marked projection unless admitted.
- Span ids or parent event ids so an operator can join startup, seal, append,
  spawn, ready, and successor-admission events without loading every JSON file.
- Digest bundle for the operational event source refs, so a report can cite a
  stable evidence set.
- Human-readable summary text, bounded and derived from typed fields.

## Too Granular Or Noisy

- Every poll iteration while waiting for successor ready; log start, timeout,
  ready, and exit-before-ready outcomes instead.
- Full stdout/stderr, prompts, responses, patches, or file contents inside the
  operational event; keep refs/digests/excerpts elsewhere.
- Every scheduler/node mirror mutation unless it changes the authority-relevant
  question being asked.
- Raw branch registry snapshots as authority events.
- Duplicate ready facts in three forms unless they are joined by a single
  event/source-ref list.
- Per-file surface hash rows in the authority stream; record partition roots and
  refs, then leave detailed manifests to evidence artifacts.

## Source Notes

- `crates/ploke-eval/src/cli/prototype1_state/mod.rs:62` defines History as
  sealed authority blocks, not scheduler/report projections; `:189` defines the
  Crown as lineage authority rather than process id, branch, or path; `:221`
  lists what the successor should verify before unlocking succession.
- `crates/ploke-eval/src/cli/prototype1_state/mod.rs:145` separates Tree and
  History lineage; same Artifact/tree key is not an authority conflict unless
  the same lineage head is advanced without the required authority rule.
- `crates/ploke-eval/src/cli/prototype1_state/history.rs:52` states that
  History is the durable authority surface and other records are projections or
  evidence; `:124` states the one-`Crown<Ruling>`-per-lineage invariant.
- `crates/ploke-eval/src/cli/prototype1_state/history.rs:187` says Crown is
  lineage authority and Parent is a Runtime role; `:193` warns that Artifact
  identity alone is not the authority coordinate.
- `crates/ploke-eval/src/cli/prototype1_state/history.rs:255` lists current
  enforcement and gaps; live handoff appends a minimal sealed block, but
  uniform bootstrap/predecessor admission, live `Parent<Ruling>` as sole open
  block writer, ingress, signatures, and consensus remain gaps.
- `crates/ploke-eval/src/cli/prototype1_state/history.rs:312` lists implemented
  typestate carriers; `:325` notes private fields and transition-only advanced
  states; `:359` says open block construction/admission are Crown-gated but
  actor identity is still supplied as data.
- `crates/ploke-eval/src/cli/prototype1_state/history.rs:575` defines
  `LineageId`; `:587` says `BlockStore::append` is the only semantic operation
  that may advance a lineage head; `:831` shows append verifying hash, stale
  state, and predecessor rules before updating projections.
- `crates/ploke-eval/src/cli/prototype1_state/history.rs:1160` describes
  `LineageState` as local single-ruler store proof, not distributed consensus;
  `:1199` defines `StoreHead::Absent` as local store-scoped absence.
- `crates/ploke-eval/src/cli/prototype1_state/history.rs:2405` lists the data
  needed to open a block; `:2452` describes sealed header material carried by
  `Crown<Locked>`; `:2652` shows the sealed header fields.
- `crates/ploke-eval/src/cli/prototype1_state/history.rs:2950` verifies current
  Artifact tree against the sealed head; `:2983` verifies current surface roots;
  `:3023`, `:3064`, and `:3127` are the Crown-gated open/admit/seal boundaries.
- `crates/ploke-eval/src/cli/prototype1_state/inner.rs:47` defines Crown state
  markers; `:84` defines `Crown<S>` as exclusive lineage authority; `:158`
  defines the transition that retires a selectable Parent and locks authority.
- `crates/ploke-eval/src/cli/prototype1_state/parent.rs:72` defines
  `Startup<Validated>` as the local single-ruler startup gate; `:481` verifies
  predecessor startup against sealed History head, current Artifact tree, and
  current surface; `:553` checks the validated startup against parent identity.
- `crates/ploke-eval/src/cli/prototype1_process.rs:930` sequences successor
  handoff; `:966` seals via `Parent<Selectable>::seal_block_with_artifact`;
  `:989` appends the sealed block before successor spawn; `:1022` and `:1045`
  record spawn evidence; `:1058` waits for ready and writes handoff evidence.
- `crates/ploke-eval/src/cli/prototype1_process.rs:1125` builds handoff block
  fields from current store state, clean active checkout tree key, opening
  authority, parent actor, surface commitment, and expected state.
- `crates/ploke-eval/src/cli/prototype1_state/journal.rs:20` warns that
  flattened legacy journal names are storage labels, not History ontology.
- `crates/ploke-eval/src/cli/prototype1_state/history_preview.rs:1` says the
  preview is read-only and not sealed History; `:835` projects successor
  handoff with missing generation; `:1598` marks preview blocks
  `provisional_unsealed`.
- `crates/ploke-eval/src/cli/prototype1_state/report.rs:107` still reports
  missing/weak fields such as `sealed_by`, predecessor block verification in
  current records, and distributed evidence.
- `docs/reports/prototype1-record-audit/history-admission-map.md:29` classifies
  existing record families; `:56` assigns duplicated field ownership; `:84`
  lists weak fields, including missing `sealed_by`; `:173` repeats that preview
  imports do not become authoritative History.
- `docs/reports/prototype1-record-audit/2026-04-29-history-crown-introspection-audit.md:7`
  is useful for presentation-drift risk, but some findings predate later live
  handoff wiring. Treat it as historical caution, not the current implementation
  baseline where it conflicts with `history.rs` and `prototype1_process.rs`.
