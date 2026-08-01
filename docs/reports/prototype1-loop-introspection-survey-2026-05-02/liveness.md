# Liveness

## Domain

Freshness, stalled runtime detection, last durable record, heartbeat-like observations, and process/record ambiguity.

## Questions To Answer

- During a 5-10 generation run, what is the freshest durable sign of life for
  the active loop: latest transition journal entry, scheduler/node `updated_at`,
  ready/completion latch, stream append, or monitor-observed file mtime?
- Which runtime is currently expected to be alive, in which role/state, and what
  was its last durable transition? Prefer `Parent`/`Child<State>`/incoming
  successor wording over a flat "process status".
- Is a child or successor stalled before readiness, after readiness, while
  evaluating, while waiting for a result, or during handoff?
- Did the parent observe a terminal result or acknowledgement, or did only the
  child/successor write a local record?
- If the monitor says "quiet", is it quiet because the runtime is legitimately
  waiting, because the process exited, because no durable record is emitted for
  the current phase, or because stream output is not normalized?
- For longer runs, which generations have widening gaps between durable
  transitions, repeated ready timeouts, repeated result-pending states, or
  accumulating process/record mismatches?
- For longer runs, can operators separate "no process visible" from "no ruling
  Parent authority" without treating pid liveness as Crown authority?

## Already Answered By Persisted Data

- Scheduler and node mirrors persist coarse freshness: `Prototype1SchedulerState`
  has campaign-level `updated_at`, frontier/completed/failed node ids, and
  `last_continuation_decision`; each `Prototype1NodeRecord` has status,
  `created_at`, and `updated_at`
  (`crates/ploke-eval/src/intervention/scheduler.rs:75`,
  `:157`, `:515`, `:676`).
- The append-only transition journal persists durable transition observations
  with `recorded_at` timestamps for parent start, spawn, ready, child
  lifecycle, observe-child, successor, checkout, and handoff records
  (`crates/ploke-eval/src/cli/prototype1_state/journal.rs:126`, `:154`,
  `:180`, `:198`, `:311`, `:332`).
- Child runtime liveness has typed durable projections for `Child<Ready>`,
  `Child<Evaluating>`, and `Child<ResultWritten>`; these include runtime id,
  generation, pid, paths, and recorded time
  (`crates/ploke-eval/src/cli/prototype1_state/child.rs:1`, `:18`, `:30`,
  `:144`, `:151`, `:160`, `:183`).
- Child spawn waiting already records enough to distinguish acknowledged,
  exited-before-ready, and ready-timeout outcomes for the initial spawn
  handshake (`crates/ploke-eval/src/cli/prototype1_state/c3.rs:540`, `:608`,
  `:729`).
- Successor readiness and completion are persisted both as attempt-scoped JSON
  latch files and as successor journal records
  (`crates/ploke-eval/src/cli/prototype1_state/invocation.rs:402`, `:426`,
  `:444`; `crates/ploke-eval/src/cli/prototype1_process.rs:321`, `:348`;
  `crates/ploke-eval/src/cli/prototype1_state/successor.rs:19`).
- The monitor already knows the expected volatile locations and treats
  scheduler, branch registry, journal, nodes, invocations, attempt results,
  successor ready/completion, and streams as distinct observation surfaces
  (`crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:1732`).

## Partially Derivable

- "Last durable record" can be derived by replaying or scanning
  `transition-journal.jsonl` and comparing `recorded_at`, but current monitor
  code mostly counts and prints new entries rather than exposing a normalized
  last-record summary (`crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:2141`,
  `:2150`, `:2166`).
- Freshness can be inferred from scheduler/node `updated_at`, journal
  `recorded_at`, successor ready/completion `recorded_at`, attempt result
  `recorded_at`, and filesystem `modified` times, but these clocks are not
  normalized into one freshness model (`history_preview.rs` currently falls back
  across `recorded_at`, `created_at`, and `updated_at` at
  `crates/ploke-eval/src/cli/prototype1_state/history_preview.rs:1170`).
- Stalled runtime detection is partially implemented in monitor watch:
  non-`ContinueReady` scheduler decisions, failed nodes, failed successor
  completion files, incomplete materialization after a quiet snapshot, and
  latest parent pid disappearance after quiet time
  (`crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:2319`,
  `:2372`, `:2417`, `:2446`, `:2481`).
- Process/record ambiguity is visible but not resolved: monitor uses `/proc/<pid>`
  as a liveness hint, while the History model explicitly says process
  uniqueness is not Crown authority (`crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:2493`;
  `crates/ploke-eval/src/cli/prototype1_state/history.rs:136`).
- Stream files give a weak heartbeat by mtime/length, and spawn records carry
  stream paths, but streams are raw artifacts with no structured reader or
  schema (`crates/ploke-eval/src/cli/prototype1_process.rs:856`;
  `docs/reports/prototype1-record-audit/state-process.md:447`).
- Successor ready is parse-checked by the waiter, but not deeply validated
  against the invocation/runtime before acknowledgement; completion has weaker
  structured readback (`docs/reports/prototype1-record-audit/state-process.md:80`,
  `:100`).

## Requires New Logging

- A uniform operational event should record runtime-step boundaries that are not
  currently durable unless the step completes: wait start/end, timeout start/end,
  quiet-duration decisions, process-exit observations, stream open/close, and
  bounded "still waiting for result" observations.
- Each event should carry minimal shared fields: `event_schema`,
  `occurred_at`, `recorded_at`, `campaign_id`, `generation`, `node_id`,
  `runtime_id`, `role`, `state`, `phase`, `transition_id`, `pid`,
  `process_status`, `source_path`, `source_mtime`, `last_durable_kind`,
  `last_durable_recorded_at`, `duration_ms`, `threshold_ms`, `outcome`,
  `detail`, and evidence refs/hashes where available.
- The missing distinction is not another authority record; it is an
  observation record. Events should make clear whether the observer is the
  runtime itself, the predecessor parent, or monitor code. That separates
  child/successor self-report from parent-observed progress.
- Add explicit "pending but alive" observations only at bounded intervals or
  phase changes. Do not log every poll iteration.
- Add a normalized "last durable record snapshot" event at monitor/report
  generation time so an LLM can ask what changed last without replaying every
  record family.

## Natural Recording Surface

- The natural producer surface is the existing transition-boundary code, with a
  shared helper such as `log_step`/`log_result` layered on `tracing` and backed by
  one campaign-scoped JSONL operational stream. It should not be a new History
  block, scheduler field, or per-domain sidecar file.
- Child runtime events belong near the `Child<State>` transition methods and
  parent-side spawn wait boundary
  (`crates/ploke-eval/src/cli/prototype1_state/child.rs:144`,
  `crates/ploke-eval/src/cli/prototype1_state/child.rs:151`,
  `crates/ploke-eval/src/cli/prototype1_state/child.rs:160`;
  `crates/ploke-eval/src/cli/prototype1_state/c3.rs:729`), because those
  already separate self-observed child state from parent-observed
  readiness/timeout.
- Successor events belong near `record_prototype1_successor_ready`,
  `record_prototype1_successor_completion`, `wait_for_prototype1_successor_ready`,
  and `append_successor_record`
  (`crates/ploke-eval/src/cli/prototype1_process.rs:321`, `:348`, `:897`,
  `:1036`).
- Parent/controller freshness events belong where scheduler/node status and
  continuation decisions are updated, but should describe operational freshness
  rather than strengthening mutable scheduler authority
  (`crates/ploke-eval/src/intervention/scheduler.rs:515`, `:676`).
- Monitor observations belong at the watch/report projection boundary, especially
  after `terminal_state` and `snapshot_quiet_for`, and should be marked
  `observer=monitor` so they remain diagnostics rather than runtime facts
  (`crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:2319`, `:2481`).

## Essential

- One normalized "last durable record" view per campaign/runtime, derived from
  journal entries plus attempt-scoped ready/result/completion records.
- Distinguish runtime self-observed events from parent-observed and
  monitor-observed events.
- Record bounded wait outcomes with `duration_ms`, threshold, pid, runtime id,
  expected state, and observed result for child ready, child result, successor
  ready, and successor completion.
- Preserve authority boundary labels: telemetry/projection/evidence vs sealed
  History authority. Pid, file mtime, ready file existence, and monitor output
  must not be treated as Crown authority.
- Include source refs to the persisted record or stream path that caused the
  observation.

## Nice To Have

- A per-generation freshness summary: newest record kind, age, current expected
  role/state, and oldest pending transition.
- Gap metrics between `Spawned -> Ready`, `Ready -> Evaluating`,
  `Evaluating -> ResultWritten`, `ResultWritten -> ObserveChild`, and
  `Successor Spawned -> Ready -> Completed`.
- A bounded stream activity summary with byte deltas and mtime, without parsing
  or importing raw stdout/stderr as state.
- A reconciliation warning when scheduler/node status disagrees with journal
  replay or attempt result state.
- A digestible monitor observation event emitted when `watch` decides a terminal
  or stalled state, so operator-facing conclusions can be cited later.

## Too Granular Or Noisy

- Per-poll heartbeat events inside 50ms successor-ready polling or child-ready
  polling loops. Log the wait start, bounded interval summary if needed, and
  terminal outcome instead.
- Full stdout/stderr line ingestion into the operational event. Store stream
  refs, byte deltas, mtimes, and short excerpts only when attached to a failure.
- Rewriting scheduler/node records as heartbeat ticks. Their mutable `updated_at`
  fields are useful but should not become a synthetic liveness clock.
- New flattened event kinds such as `ChildHeartbeat` or
  `RuntimeProgressUpdate`. Use role/state/phase fields on one shared event
  instead.
- Duplicating successor readiness across more durable record families. Existing
  ready file, successor journal record, and handoff summary are already
  overlapping evidence.

## Source Notes

- History claim boundary: mutable scheduler, branch registry, CLI reports,
  invocation/ready files, and transition journals are evidence/projections until
  admitted by sealed History
  (`crates/ploke-eval/src/cli/prototype1_state/history.rs:6`,
  `crates/ploke-eval/src/cli/prototype1_state/history.rs:52`,
  `crates/ploke-eval/src/cli/prototype1_state/history.rs:203`;
  `crates/ploke-eval/src/cli/prototype1_state/mod.rs:62`,
  `crates/ploke-eval/src/cli/prototype1_state/mod.rs:221`).
- Current code explicitly warns that execution/process uniqueness is distinct
  from Crown authority; multiple runtimes may execute, and pid/process identity
  cannot decide ruling authority
  (`crates/ploke-eval/src/cli/prototype1_state/history.rs:136`,
  `crates/ploke-eval/src/cli/prototype1_state/history.rs:140`;
  `crates/ploke-eval/src/cli/prototype1_state/mod.rs:191`,
  `crates/ploke-eval/src/cli/prototype1_state/mod.rs:212`).
- Admission map classifies `transition-journal.jsonl` as the best ordering
  source, scheduler as mutable projection, successor ready/completion as
  evidence, process streams as evidence refs only, and monitor output as
  projection (`docs/reports/prototype1-record-audit/history-admission-map.md:29`,
  `:56`, `:79`).
- Monitor/report coverage is intentionally narrow: report reads scheduler,
  branch registry, journal, and evaluations, but not node-local invocations,
  attempt results, ready/completion files, or stream payloads
  (`docs/reports/prototype1-record-audit/2026-04-29-monitor-report-coverage-audit.md:9`,
  `:18`).
- State/process audit records the main liveness ambiguity: child readiness has
  legacy and typed encodings; successor readiness is duplicated across latch file,
  successor journal record, and handoff summary; scheduler/node are mutable
  mirrors; streams are raw artifacts
  (`docs/reports/prototype1-record-audit/state-process.md:171`, `:185`,
  `:200`, `:285`, `:359`, `:447`).
- Current history/crown audit warns that successor ready/completion cannot be
  classified as in-epoch or ingress without a live Crown lock boundary
  (`docs/reports/prototype1-record-audit/2026-04-29-history-crown-introspection-audit.md:47`,
  `:96`, `:114`).
