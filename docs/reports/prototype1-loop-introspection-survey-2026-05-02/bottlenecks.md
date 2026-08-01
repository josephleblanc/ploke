# Bottlenecks

## Domain

Phase durations and slow-path diagnosis across synthesis, materialization, cargo, child execution, evaluation, selection, handoff, successor startup, and cleanup.

## Questions To Answer

- During a 5-10 generation run, which phase is consuming wall time: baseline
  eval/protocol, issue detection, intervention synthesis, materialization,
  cargo check/build, child startup, child evaluation, evaluation readback,
  successor selection, active checkout update, History seal/append, successor
  build/launch/startup, or cleanup?
- For a slow child candidate, did time go to Cargo, the spawned child runtime,
  the eval/protocol/compare procedure inside that child, or parent-side readback
  and result observation?
- Are slow `cargo check` and slow `cargo build` distinguishable, and are they
  correlated with node-local `target/` growth, cold caches, failed builds, or
  cleanup not running?
- For synthesis, was latency in deterministic issue detection/input assembly,
  LLM adjudication, protocol artifact writeback, or branch-registry projection?
- For materialization, was latency in git worktree realization, target-file
  write, node/request status updates, surface validation, or child Artifact
  commit/identity commit?
- For successor handoff, was latency in selecting/recording the successor,
  installing the selected Artifact into the active checkout, building the active
  successor binary, sealing/appending History, spawning the successor, or waiting
  for readiness?
- When a run stops or appears stuck, what is the last completed phase, what is
  the current long-running phase, and which record/ref proves that phase
  boundary?
- For longer runs, which phases trend upward by generation or node count, and
  which phases produce repeated tail latency outliers?
- For longer runs, can operators distinguish durable bottleneck facts from
  mutable projections and stderr timing lines without replaying every artifact?

## Already Answered By Persisted Data

- The transition journal persists before/after materialize records and
  before/after build records with `recorded_at`, transition ids, generation,
  refs, paths, world, hashes, and build result. This is enough for coarse
  materialize/build elapsed time when both sides are present.
- The typed state path persists child spawn phases: starting, spawned,
  observed, plus child-ready evidence. This answers whether the child reached
  readiness, exited early, or timed out, and gives a coarse spawn/ready gap.
- The child role records persist `Child<Ready>`, `Child<Evaluating>`, and
  `Child<ResultWritten>` as journal records, so the child-side evaluation
  interval can be approximated from ready/evaluating/result-written timestamps.
- Parent-side observe-child before/after journal entries persist whether a
  runner result was read and whether it pointed at a successful evaluation
  artifact or a failed runner disposition.
- Attempt-scoped child results are persisted at
  `nodes/*/results/<runtime-id>.json`; `runner-result.json` keeps a mutable
  latest copy. These records include disposition, exit code, stdout/stderr
  excerpts for failures, evaluation artifact path on success, and `recorded_at`.
- Evaluation reports are persisted under `prototype1/evaluations/*.json`, and
  branch-registry summaries keep compared/rejected counts, overall disposition,
  and `evaluated_at`.
- Scheduler/node projections persist coarse status timestamps:
  `created_at`/`updated_at` on nodes and scheduler `updated_at`. These are
  useful for detecting gross dwell, but they conflate multiple operations.
- Successor records persist selected, checkout before/after, spawned, ready,
  timed-out, exited-before-ready, and completed states. The timeout record
  carries `waited_ms`.
- Active checkout advancement is persisted as a journal entry with previous and
  selected parent identity, active root, selected branch, and installed commit.
- The live handoff path now seals/appends a History block before successor
  spawn; the stored block metadata gives lineage height/hash and append
  evidence, but not an operational duration.
- `TimingTrace` prints start/end wall times for several controller and child
  evaluation scopes, including synthesis, materialization, eval, protocol,
  compare, and report persistence. These lines are useful during a live run but
  are not a structured persisted data source.

## Partially Derivable

- End-to-end child-candidate duration is derivable by joining materialize,
  build, spawn, child lifecycle, observe-child, runner result, evaluation
  report, and cleanup-adjacent records, but the join crosses append-only
  journal records and mutable sidecars.
- Cargo check versus cargo build duration is only partially derivable in the
  typed state path. `BuildChild` runs `cargo check` and then `cargo build`, but
  the journal records only bracket the whole build transition; check/build are
  distinguishable by failure result, not by successful per-subphase timings.
- The legacy child helper runs `cargo build` directly and normalizes compile
  failure as a runner result, but it does not persist a separate build start/end
  event or distinguish check/build.
- Child execution latency can be approximated from spawn/ready/evaluating/result
  written/observe timestamps. It does not consistently separate child startup,
  evaluation proper, report persistence, parent polling, and parent readback.
- Evaluation readback time is partially derivable from observe-child before/after
  and runner/evaluation artifact timestamps, but there is no dedicated readback
  event with byte count, source digest, or parse duration.
- Synthesis duration is visible through ephemeral `TimingTrace` stderr and
  protocol artifact timestamps/records, but the branch registry only persists
  synthesized candidates and `updated_at`, not synthesis start/end/duration.
- Selection duration is effectively absent. Selection outcome is persisted
  through branch evaluation summaries, branch registry active selection,
  scheduler continuation decision, and successor selected journal records, but
  the calculation itself is not timed.
- Active checkout update duration can be roughly estimated from successor
  checkout before/after records and `ActiveCheckoutAdvancedEntry`, but install,
  identity validation, and parent checkout validation are not separated.
- History seal/append duration is not directly persisted. Ordering is visible in
  code and stored History block output, but no operational event brackets open,
  admit claim, seal, append, and fsync.
- Successor startup latency is partly derivable from successor spawned/ready or
  timed-out records; active successor build latency and invocation write/stream
  setup latency are not separately persisted.
- Cleanup success is weakly derivable from absence of node worktree/build paths
  and lack of cleanup errors. There is no durable cleanup result event, so slow
  cleanup and skipped cleanup are hard to distinguish after the fact.
- Long-run trend analysis can be approximated from `recorded_at`, node
  `updated_at`, runner result `recorded_at`, and evaluation `evaluated_at`, but
  the clocks mix occurred, observed, recorded, and projection update times.

## Requires New Logging

- A uniform tracing-backed JSONL operational event should bracket each slow-path
  phase with `started_at`, `finished_at`, `duration_ms`, `status`, `error`,
  `phase`, `step`, `role`, and source refs. This is telemetry, not History
  authority.
- Required common fields: `campaign_id`, `lineage_id` when known,
  `parent_id`, `parent_node_id`, `generation`, `node_id`, `branch_id`,
  `runtime_id`, `transition_id` or stable operation id, `pid`, `attempt_ordinal`
  when available, `source_path`, `source_digest`, `stdout_path`,
  `stderr_path`, `exit_code`, and `authority_label`.
- Add explicit timings for synthesis: issue detection, synthesis input assembly,
  LLM procedure/adjudication, artifact write, and branch-registry record.
- Add materialization timings for worktree realize/reuse, target write,
  scheduler/node/request update, surface validation, child Artifact commit, and
  parent identity commit.
- Add cargo timings that separate `cargo check`, `cargo build`, binary promotion,
  target-dir path, and build failure classification. Keep compiler output as
  stream refs or bounded excerpts, not inline payload.
- Add child timings for invocation write, spawn, ready wait, evaluating start,
  result written, parent observe/readback, evaluation report load, and terminal
  classification.
- Add successor timings for continuation validation, branch selection, active
  checkout install, active successor build, History open/admit/seal/append,
  invocation write, spawn, ready wait, completion, and predecessor finish.
- Add cleanup events for worktree removal and node-local build-product removal,
  with aggregate status, path refs, optional byte counts, and refusal/error
  reason. Avoid per-file deletion logs.
- Add a derived bottleneck summary event per generation or parent turn with
  phase durations and top slow phase, citing the source event refs used to
  derive it.

## Natural Recording Surface

- Use one shared operational event stream emitted through small transition
  boundary helpers, for example `.log_step()` and `.log_result()` on a local
  context, rather than adding per-domain JSON files.
- Synthesis events belong around `persist_issue_detection_for_record`,
  `persist_intervention_synthesis_for_record`, and `record_synthesized_branches`
  because those boundaries already know the run record, model/provider,
  candidate set, branch ids, and artifact write path.
- Materialization and cargo events belong in the typed transitions
  `MaterializeBranch` and `BuildChild`, plus the legacy helper
  `stage_prototype1_runner_node` / `build_prototype1_runner_binary` while that
  path remains live.
- Child execution events belong at `SpawnChild`, `Child<State>` transition
  methods, `run_prototype1_branch_evaluation`, and `ObserveChild`. Those
  surfaces already separate parent-observed spawn/readback from child-observed
  ready/evaluating/result-written.
- Evaluation readback events belong where the parent loads the attempt runner
  result and evaluation report, not in the evaluation report schema itself.
- Selection events belong where branch evaluations are reduced into
  `selected_next_branch_id`, where `select_treatment_branch` mutates the branch
  projection, and where `Successor::selected` is appended.
- Active checkout, History, successor launch, and cleanup events belong in
  `spawn_and_handoff_prototype1_successor`,
  `install_committed_successor_artifact`, the History seal/append boundary, the
  successor ready/completion writers, and cleanup helpers.
- The event should cite transition-journal lines, History block hashes,
  scheduler/node/registry paths, invocation/result/evaluation paths, stream
  paths, and checkout commits as evidence. It must not be treated as a Crown
  lock, History admission, or selected-successor authority.

## Essential

- Phase duration for each parent turn and child candidate:
  synthesis/materialization/cargo/child startup/child evaluation/readback/
  selection/active checkout/History append/successor startup/cleanup.
- Separate cargo check, cargo build, and binary promotion outcomes where the
  typed path has both check and build.
- Slow-path classification with `status`, `exit_code`, `error`, and evidence
  refs for build failure, treatment failure, missing result, parse/readback
  failure, successor timeout/exit, History append failure, install failure, and
  cleanup failure.
- Correlation fields that let an operator join parent, child, successor, node,
  branch, runtime, generation, and History block without treating any projection
  as authority.
- Wall-clock `started_at`, `finished_at`, and `duration_ms` on operational
  events, with clear observer/source role.
- Evidence refs/digests instead of copied payloads.
- Authority label: operational telemetry / transition evidence / mutable
  projection / sealed History reference.

## Nice To Have

- Per-generation bottleneck summary with top slow phase, p50/p95 by phase for
  long runs, and regression from previous generation.
- Disk-byte deltas for node-local Cargo target dirs before/after build and
  cleanup, especially to diagnose target growth.
- Cache/cold-start hint for cargo and successor build, if cheaply derivable from
  target-dir existence and build output refs.
- Attempt ordinal for retries or repeated child invocations of the same node.
- Model/provider/timeout fields for synthesis latency correlation.
- Readback byte counts and parse duration for runner result, evaluation report,
  and History block reads.
- Derived monitor view: "currently slow in phase X for N seconds, last durable
  record Y, source refs Z".

## Too Granular Or Noisy

- Per-poll events for child-ready/result or successor-ready loops. Record wait
  start and terminal outcome, with optional bounded interval summaries only when
  debugging a hang.
- Full Cargo stdout/stderr, compiler diagnostics, or every build artifact path
  in the operational event. Store paths, digests, exit code, and bounded
  excerpts on failure.
- Per-file materialization or cleanup entries. Record one aggregate operation
  with relevant root/path refs and capped stats.
- Re-emitting full scheduler, branch registry, runner result, evaluation report,
  protocol artifact, or History block payloads in every bottleneck event.
- Timing every helper that only formats ids, constructs paths, or wraps errors.
  Operators need phase boundaries, not call tracing.
- New flattened event families such as `CargoBuildBottleneck`,
  `ChildEvaluationSlowPath`, or `SuccessorStartupDelay`. Use structured fields
  on one event shape so role/state remains explicit.

## Source Notes

- `crates/ploke-eval/src/cli/prototype1_state/mod.rs:166` describes the
  intended loop order: create child checkout, hydrate/evaluate child, select,
  update active checkout, launch successor, hand off, exit, and cleanup. Lines
  189-239 keep Crown/History authority separate from transport/debug evidence.
- `crates/ploke-eval/src/cli/prototype1_state/history.rs:52-65` defines History
  versus Projection; `history.rs:81-100` gives the intended authority sequence.
  Lines 255-270 list current enforcement gaps, so bottleneck telemetry must not
  be framed as authority.
- `crates/ploke-eval/src/cli/prototype1_state/history.rs:3023-3145` routes
  block open/admit/seal through `Crown<Ruling>`/`Crown<Locked>` methods. Lines
  3363-3482 define append/store error cases, but no operational timing fields.
- `crates/ploke-eval/src/cli/prototype1_state/inner.rs:158-220` locks a
  selectable Parent, admits an Artifact claim, seals the block, and returns
  `Parent<Retired>`. This is the natural History boundary to time without
  weakening the typestate.
- `crates/ploke-eval/src/cli/prototype1_state/journal.rs:62-106` defines
  materialize/build entries; `journal.rs:108-145` defines spawn phases and
  observations; `journal.rs:154-196` defines ready and observe-child entries.
- `crates/ploke-eval/src/cli/prototype1_state/journal.rs:404-594` replays
  materialize/build/spawn/observe groups, making it the best existing surface
  for deriving coarse phase boundaries.
- `crates/ploke-eval/src/cli/prototype1_state/c1.rs:539-569` records
  materialize-before, realizes the workspace, and then updates node/workspace
  state; `c1.rs:637-644` records materialize-after, giving coarse duration but
  not subphase timing.
- `crates/ploke-eval/src/cli/prototype1_state/c2.rs:367-453` records
  build-before and runs `cargo check`; `c2.rs:455-506` runs `cargo build` and
  records build failure. Successful check/build are bracketed together rather
  than timed separately.
- `crates/ploke-eval/src/cli/prototype1_state/c3.rs:545-608` records spawn
  starting/spawned and starts the child process; `c3.rs:729-770` waits for ready
  with timeout and records `waited_ms` only for timeout.
- `crates/ploke-eval/src/cli/prototype1_state/c4.rs:274-388` records
  observe-child before/after and loads runner result/evaluation report, but does
  not separately time readback/parse.
- `crates/ploke-eval/src/cli/prototype1_state/child.rs:18-40` defines
  `Child<Ready>`, `Child<Evaluating>`, and `Child<ResultWritten>` records;
  `child.rs:144-167` writes those state transitions.
- `crates/ploke-eval/src/cli/prototype1_state/successor.rs:19-56` defines
  selected, spawned, checkout, ready, timed-out, exited-before-ready, and
  completed successor states.
- `crates/ploke-eval/src/intervention/scheduler.rs:75-174` defines node,
  runner result/request, and scheduler projection fields; `scheduler.rs:515-613`
  mutates status and writes latest runner result; `scheduler.rs:627-685` decides
  and records continuation.
- `crates/ploke-eval/src/intervention/branch_registry.rs:216-350` records
  synthesized/selected candidates; `branch_registry.rs:452-544` records selected
  branch projection; `branch_registry.rs:682-710` records evaluation summary.
- `crates/ploke-eval/src/cli.rs:1143-1173` implements `TimingTrace` as stderr
  start/end prints with elapsed seconds, not durable structured records.
- `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:745-771` wraps
  issue detection and synthesis in `TimingTrace`; `cli_facing.rs:919-931` wraps
  branch child evaluation; `cli_facing.rs:959-994` selects a branch and records
  continuation decision without timing the selection itself.
- `crates/ploke-eval/src/cli.rs:1321-1374` runs LLM synthesis and writes a
  protocol artifact; `crates/ploke-eval/src/intervention/synthesize.rs:387-400`
  returns procedure output/artifact but no duration field.
- `crates/ploke-eval/src/cli/prototype1_process.rs:439-493` builds the active
  successor binary after install prep; `prototype1_process.rs:496-650` commits
  and installs the selected Artifact into the active checkout and journals
  checkout before/after plus active checkout advanced.
- `crates/ploke-eval/src/cli/prototype1_process.rs:666-714` removes child
  build products and workspace without a durable success event.
- `crates/ploke-eval/src/cli/prototype1_process.rs:716-787` persists child
  Artifact evidence; `prototype1_process.rs:803-819` writes child invocation and
  waits for child output in the legacy helper; `prototype1_process.rs:1299-1310`
  writes attempt and latest runner results.
- `crates/ploke-eval/src/cli/prototype1_process.rs:1438-1547` wraps child
  evaluation substeps in `TimingTrace`: materialize, prepare campaign, eval,
  protocol, compare, persist report, and persist summary.
- `crates/ploke-eval/src/cli/prototype1_process.rs:1727-1907` documents and
  executes the parent-side child path, including materialize/build/spawn/wait,
  result readback, evaluation report load, and cleanup.
- `crates/ploke-eval/src/cli/prototype1_process.rs:930-1107` seals/appends
  History, spawns the successor, records spawned/ready/timeout/exit, and returns
  a retired parent. The path has ordering evidence but not phase durations.
- `docs/reports/prototype1-record-audit/history-admission-map.md:31-48`
  classifies existing records as transition evidence, attempt evidence,
  projections, or CLI projections. Lines 58-77 cover field ownership for ids,
  statuses, stop reasons, timestamps, and stream refs.
- `docs/reports/prototype1-record-audit/history-admission-map.md:84-106` lists
  missing weak fields including sealed actor, artifact provenance, evaluation
  schema/source digests, registry evaluation refs, latest runner result pointer,
  and metric derivation/source digests.
- `docs/reports/prototype1-record-audit/2026-04-29-monitor-report-coverage-audit.md:7-16`
  says monitor report reads scheduler, branches, journal, and evaluations only;
  lines 18-33 list omitted node-local results, invocations, ready/completion,
  streams, and run artifacts.
- `docs/reports/prototype1-record-audit/2026-04-29-record-emission-sites-audit.md:15-27`
  inventories current emitted record families; lines 28-35 call out duplicated
  latest runner result, registry summary limits, weak successor typed readers,
  stream refs, and mutable scheduler/node mirrors.
- `crates/ploke-eval/src/cli/prototype1_state/report.rs:1-7` states that the
  report is provisional, not sealed History; `report.rs:76-106` loads scheduler,
  branch registry, transition journal, and evaluations.
