# Handoff

## Domain

Successor selection, active checkout advancement, Crown lock, successor launch, acknowledgement, predecessor exit, and cleanup handoff boundaries.

## Questions To Answer

- During a 5-10 generation run, which node/branch was selected as the successor
  candidate, what scheduler policy decision allowed or blocked continuation, and
  was the decision backed by `ContinueReady` or a terminal stop reason?
- Did the predecessor install the selected Artifact into the stable active
  checkout before launching the successor, and which branch/commit/parent
  identity became active?
- Did the predecessor seal and append the History block before successor spawn,
  and which lineage, block height, block hash, active Artifact, selected
  successor runtime, and surface commitment were involved?
- Was the successor launched from the active checkout, with which binary,
  invocation file, pid, streams, and ready path?
- Did the successor acknowledge before timeout or exit, and did the predecessor
  retire/return without trying to keep ruling authority?
- Did the successor later write completion, and did the parent-start record for
  the successor include the same handoff runtime id?
- Were temporary child worktrees and build products removed at the intended
  boundary, and were any cleanup refusals caused by paths outside the node dir?
- For longer runs, where are handoff gaps accumulating: repeated timeouts,
  stale continuation decisions, active checkout mismatches, missing completion,
  failed predecessor sealed-head verification, or growth from uncleaned
  workspaces/build outputs?
- For future parallelism, can we distinguish one lineage handoff from another
  without relying on campaign-global scheduler state, branch names, process ids,
  or active checkout paths as authority?

## Already Answered By Persisted Data

- Successor selection is persisted in the transition journal as
  `successor::Record { state: Selected { decision } }`, including the
  continuation decision payload that was current when selection was recorded.
- Scheduler projection persists `last_continuation_decision` with disposition,
  selected next branch id, selected branch disposition, next generation, and
  total-node count.
- Branch registry projection persists selected and active branch state through
  `selected_branch_id` and `active_targets`.
- Active checkout advancement is persisted in the transition journal through
  `ActiveCheckoutAdvancedEntry`: previous/selected parent identity, active parent
  root, selected branch, and installed commit.
- Successor checkout before/after, spawned, ready, timed-out,
  exited-before-ready, and completed states are persisted as `successor::Record`
  entries in `prototype1/transition-journal.jsonl`.
- Successor invocation, ready, and completion sidecar files are attempt-scoped by
  runtime id under the node directory.
- Parent-start records persist the incoming parent identity, repo root, pid, and
  optional `handoff_runtime_id`.
- The live handoff path now appends a sealed History block before successor
  spawn, and the successor startup path verifies the sealed head's current
  Artifact tree and surface commitment before entering the ready Parent path.

## Partially Derivable

- The end-to-end handoff chain can be reconstructed by joining node id, branch
  id, runtime id, ready path, invocation path, parent identity, and journal order,
  but this requires crossing transition journal entries, scheduler projection,
  branch registry projection, invocation JSON, ready/completion JSON, and History
  block storage.
- Predecessor exit is inferred from command return and lack of further parent
  activity after `Parent<Retired>` is produced; there is no explicit operational
  event saying the predecessor has finished its final turn after handoff.
- Crown lock timing is partly visible from sealed History block append and
  successor spawn order, but a reader must correlate debug logs or code ordering
  with journal records because there is no single operational event spanning
  lock, append, launch, and acknowledgement.
- Cleanup is partly visible through absence of worktree/build paths or error
  phases, but successful cleanup of child workspace/build products is not
  persisted as a first-class operational result.
- Successor-ready and successor-completion files can be read by current code, and
  history preview imports them as degraded evidence, but broad typed reuse is
  still called out as weak in the audits.
- Long-run continuity can be approximated from selected trajectory, active
  checkout advancement, parent identity, and sealed heads, but generation,
  branch id, and path remain projections rather than lineage authority.

## Requires New Logging

- A uniform operational event should record each handoff step with stable
  correlation fields: `campaign_id`, `lineage_id`, `parent_id`,
  `predecessor_node_id`, `successor_node_id`, `generation`,
  `successor_runtime_id`, `transition_id` or `handoff_id`, and
  `attempt_ordinal` when available.
- Selection needs source-strength fields: `selected_branch_id`,
  `selected_node_id`, `selection_policy`, `continuation_disposition`,
  `selection_source`, and references to scheduler/journal/evaluation records.
- Active checkout advancement needs before/after fields:
  `active_parent_root`, previous/selected parent identity refs,
  `selected_branch`, `installed_commit`, observed clean tree key, and surface
  digest roots.
- Crown/History boundary needs operational fields: `block_height`, `block_hash`,
  `parent_block_hashes`, `opened_from_state`, `crown_lock_transition_ref`,
  `active_artifact_ref`, `selected_successor_ref`, and whether append succeeded.
- Successor launch and acknowledgement need `binary_path`, `invocation_path`,
  `ready_path`, `pid`, `stdout`, `stderr`, wait outcome, wait duration, timeout,
  exit code, and acknowledgement record digest/path.
- Predecessor exit needs an explicit terminal handoff step such as
  `predecessor_finished_after_handoff` with retired parent identity, result, and
  any cleanup outcome. This should remain telemetry, not a second authority
  transition.
- Cleanup needs bounded result fields: cleaned child worktree path, build-product
  paths, status, bytes if cheaply available, and failure/refusal reason. Path
  details should be capped to avoid turning cleanup into noisy filesystem logs.

## Natural Recording Surface

- Use one shared tracing-backed JSONL operational event stream, not additional
  handoff-specific files. Natural emission points are the existing transition
  boundaries in `spawn_and_handoff_prototype1_successor`,
  `install_committed_successor_artifact`,
  `record_prototype1_successor_ready`,
  `record_prototype1_successor_completion`, and successor startup
  acknowledgement.
- The event should be a projection beside current journal/History writes. It may
  cite sealed block hashes, transition journal paths, invocation/ready/completion
  paths, and source digests, but it must not be read as History authority.
- A compact shape is enough: `timestamp`, `event_family = "prototype1.handoff"`,
  `step`, `status`, common correlation fields, optional path/ref fields, optional
  timing fields, optional `error`, and optional `authority_boundary` fields that
  name the sealed block or Crown-lock reference being observed.
- The current `successor::Record` and legacy journal entries are good source
  material, but new tracing labels should avoid carrying legacy flattened names
  forward as the domain model. Prefer step labels such as `selected`,
  `checkout_advanced`, `crown_locked`, `history_appended`, `launched`, `ready`,
  `predecessor_finished`, and `cleanup_finished`.

## Essential

- Selection decision and stop/continue reason, with source refs and source
  strength.
- Active checkout before/after, installed commit, selected parent identity, clean
  tree key, and surface commitment.
- Crown/History handoff boundary: lineage id, block height/hash, selected
  successor ref, active artifact ref, append result, and lock/append/spawn order.
- Successor launch attempt: runtime id, invocation path, binary path, active root,
  pid, stream paths, ready path, and spawn result.
- Acknowledgement outcome: ready/timed-out/exited-before-ready, wait duration,
  exit code if any, ready record path/digest, and parent-observed handoff result.
- Successor parent start and completion linkage back to the same runtime id.
- Cleanup success/failure at the workspace/build-product boundary, with refusal
  reasons for unsafe paths.

## Nice To Have

- Per-step monotonic timing deltas from selection through ready/completion.
- Source digests for scheduler, branch registry, invocation, ready, completion,
  journal line, and sealed block records.
- Bounded disk-byte deltas for child workspace and active checkout build output.
- Human-readable short summaries for monitor output, derived from structured
  events rather than stored as the primary data.
- A single handoff id carried across all records once the typed handoff box exists.

## Too Granular Or Noisy

- Poll-loop samples for every ready-path check; keep only final wait outcome and
  elapsed duration unless debugging a specific hang.
- Full stdout/stderr payloads in the operational event; store paths/digests and
  bounded excerpts only for failures.
- Full file listings of worktree or `target/` cleanup; record aggregate status,
  capped path refs, and byte totals when useful.
- Repeating the full scheduler, branch registry, parent identity, or sealed block
  JSON in every event. Use refs/digests.
- Treating every `debug!` phase string as a durable event kind. Keep event steps
  stable and domain-level.

## Source Notes

- `crates/ploke-eval/src/cli/prototype1_state/mod.rs:166` documents the intended
  single-successor loop: select child Artifact, update stable active checkout,
  launch successor, hand off, exit, cleanup. Lines 189-239 describe Crown
  succession, sealed block validation targets, and the warning not to add another
  ad hoc acknowledgement file.
- `crates/ploke-eval/src/cli/prototype1_state/history.rs:81` documents the
  intended authority sequence from `Parent<Ruling>` through `Crown<Locked>` and
  successor verification. Lines 255-270 list current enforcement gaps, including
  missing full live `Parent<Ruling>` writer enforcement, uniform startup carrier,
  ingress, signatures, and consensus.
- `crates/ploke-eval/src/cli/prototype1_state/history.rs:2419` defines
  `OpenBlock` fields; lines 2452-2488 define `SealBlock` fields committed by the
  Crown lock material. Lines 2824-2856 seal selected successor, active artifact,
  claims, lock transition ref, entry root, and block hash. Lines 2950-2995 verify
  current checkout Artifact tree and surface against a sealed head.
- `crates/ploke-eval/src/cli/prototype1_state/inner.rs:158` keeps the lock/seal
  transition behind `LockCrown`; lines 197-220 move `Parent<Selectable>` into
  retired state, open/admit/seal the block, and return `Parent<Retired>`.
- `crates/ploke-eval/src/cli/prototype1_state/parent.rs:72` keeps startup
  evidence private so invocation JSON cannot become Parent readiness by
  convention. Lines 481-521 verify predecessor sealed head, current clean tree,
  and surface before successor startup can become validated.
- `crates/ploke-eval/src/cli/prototype1_process.rs:481` validates continuation,
  selects/resolves the treatment branch, installs the selected Artifact into the
  active checkout, and builds the successor binary. Lines 577-649 verify/install
  the selected branch and journal checkout advancement. Lines 930-1099 seal and
  append History, create invocation/ready paths, spawn, record spawned/ready or
  timeout/exit, and return a retired parent plus optional handoff result.
- `crates/ploke-eval/src/cli/prototype1_process.rs:321` writes successor ready
  and journal state; lines 348-380 write successor completion. Lines 404-437
  validate successor continuation from scheduler state. Lines 653-713 bound
  cleanup to node-local paths and child workspace/build products.
- `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:3073` acknowledges
  handoff on successor startup, validates invocation/campaign/node/active root,
  verifies predecessor History, writes ready, and enters `Parent<Ready>`. Lines
  3335-3345 record parent start with optional handoff runtime id. Lines
  3547-3578 record selection and launch handoff; lines 3708-3718 record successor
  completion after the bounded turn.
- `crates/ploke-eval/src/cli/prototype1_state/journal.rs:20` warns that flattened
  legacy record names are storage labels. Lines 294-329 define
  active-checkout-advanced and successor-handoff evidence, and lines 332-353 list
  the journal envelope variants.
- `crates/ploke-eval/src/cli/prototype1_state/successor.rs:19` defines projected
  successor states: selected, spawned, checkout, ready, timed out,
  exited-before-ready, and completed.
- `crates/ploke-eval/src/cli/prototype1_state/invocation.rs:242` keeps successor
  invocation creation downstream of `Parent<Retired>`; lines 402-461 define
  ready/completion schemas and attempt-scoped paths.
- `crates/ploke-eval/src/intervention/scheduler.rs:45` defines persisted
  continuation decision fields. Lines 627-684 compute and persist stop/continue
  decisions.
- `crates/ploke-eval/src/intervention/branch_registry.rs:452` marks a selected
  treatment branch and active target in the mutable branch registry projection.
- `docs/reports/prototype1-record-audit/history-admission-map.md:31` classifies
  transition journal as append-only transition evidence, successor ready/completion
  as process-bound evidence, scheduler/branch registry as projections, and CLI
  reports as projection only. Lines 84-102 list weak/missing fields including
  `sealed_by`, startup validation gaps in older wording, typed successor
  completion loading, and artifact provenance.
- `docs/reports/prototype1-record-audit/2026-04-29-record-emission-sites-audit.md:17`
  inventories current persisted handoff files and notes successor ready/completion
  are dual-written but still need stronger typed loading before broad reliance.
- `docs/reports/prototype1-record-audit/2026-04-29-history-crown-introspection-audit.md:52`
  warns that successor ready/completion cannot be classified without Crown lock
  timing, and lines 91-101 repeat that current projection records are not sealed
  History or Crown authority.
