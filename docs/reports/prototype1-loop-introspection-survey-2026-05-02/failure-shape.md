# Failure Shape

## Domain

Failure classification, missing transition detection, terminal status, recovery hints, and distinguishing process, transport, projection, authority, and evaluation failures.

## Questions To Answer

- During a 5-10 generation run, what failed: process spawn/exit, transport
  handshake, projection inconsistency, authority/admission, build, evaluation, or
  cleanup?
- Did the current role/state transition complete, or is there a missing
  matching record such as `before` without `after`, `spawned` without
  `ready/observed`, or `result_written` without parent observation?
- Is the node/runtime terminal, still retryable, or ambiguous because the child
  or successor may have written side evidence after the parent stopped waiting?
- What durable source should the operator inspect next: attempt result,
  successor completion, stream log, scheduler projection, branch registry,
  transition journal line, History block, or active checkout state?
- Is the failure inside the selected Artifact/Runtime, or in the parent-side
  transport/controller path around it?
- Did the failure leave cleanup work behind: node worktree, node-local target
  directory, copied child binary, successor stream files, stale ready file, or
  active checkout state?
- For longer runs, which failure classes recur by generation, node lineage,
  branch target, runtime surface, or policy setting such as `stop_on_error`?
- For longer runs, are recovery decisions changing selection pressure, e.g.
  compile failures counted as rejected candidates versus transport failures
  pausing the loop?
- For longer runs, which terminal statuses are projections only, and which have
  authority evidence behind them?

## Already Answered By Persisted Data

- Child node terminal status is persisted in `scheduler.json`,
  `nodes/*/node.json`, latest `runner-result.json`, and attempt-scoped
  `nodes/*/results/<runtime-id>.json`. The scheduler status enum separates
  `planned`, `workspace_staged`, `binary_built`, `running`, `succeeded`, and
  `failed`; runner disposition separates `succeeded`, `compile_failed`, and
  `treatment_failed`.
- Child compile failures persist exit code plus stdout/stderr excerpts in the
  runner result. Treatment failures persist the PrepareError text in `detail`;
  if the child exits without writing an attempt result, the parent converts that
  into an explicit treatment failure with process excerpts.
- Successor transport has journal states for `spawned`, `ready`, `timed_out`,
  `exited_before_ready`, and `completed`, plus standalone
  `successor-ready/<runtime-id>.json` and
  `successor-completion/<runtime-id>.json` records.
- Continuation stop reason is persisted as
  `last_continuation_decision.disposition`, including max-generation, max-node,
  no-selection, first-keep, and rejected-selected-branch stops.
- The transition journal replay code already detects several missing-transition
  shapes: materialization `before` without `after`, build `before` without
  `after`, spawned child without observation, ready without spawned, observed
  without spawned, and child completion observation without terminal result.
- The monitor report already summarizes scheduler failed/completed/frontier
  counts, node status counts, branch status counts, continuation decision,
  transition-journal kind counts, evaluation disposition counts, and failed tool
  calls.
- History and the admission map already mark the authority boundary: scheduler,
  branch registry, reports, invocation JSON, ready files, and CLI output are
  evidence/projections, not Crown authority.

## Partially Derivable

- Failure class can be inferred from current `PrepareError.phase` strings and
  runner dispositions, but only after mapping scattered phase names such as
  `prototype1_runner_build`, `prototype1_successor_spawn`,
  `prototype1_successor_ready`, `prototype1_history_store`,
  `prototype1_child_worktree_cleanup`, and
  `prototype1_successor_checkout_switch`.
- Missing transition detection is derivable from transition journal replays, but
  it is not surfaced as a uniform operator event and does not yet cover every
  successor/authority boundary as strongly as child materialize/build/spawn.
- Terminal status is partially derivable by joining node status, attempt result,
  child `ResultWritten`, parent `ObserveChild`, successor ready/completion, and
  process stream paths. The join is not encoded as one durable terminal fact.
- Recovery hint can often be inferred from the phase: inspect stderr for build
  failures, rerun/retry child for treatment failure, inspect ready path and
  stream logs for successor timeout, inspect History store for authority failure,
  and clean node worktree/build products for cleanup failure. These hints are
  not recorded as structured data.
- Process versus transport failure is partly separable: spawn errors are
  `PrepareError` phases; child process exits are captured through output and
  attempt results; successor timeout/exit-before-ready are typed successor
  states. There is no single `failure_class` field tying them together.
- Projection inconsistency is partly derivable by comparing scheduler/node
  status, branch registry selected/applied state, journal selection records, and
  attempt results. Current reports present counts rather than consistency
  assertions.
- Authority failure is partly derivable from History/store/tree-key/surface
  phases and startup validation errors, but current telemetry must not claim
  sealed authority beyond the implemented History block path.
- Cleanup failure is visible as returned `PrepareError::WriteManifest` or backend
  phases and sometimes from remaining paths on disk, but successful or skipped
  cleanup is not recorded uniformly.

## Requires New Logging

- A uniform operational event should record the failure class directly:
  `process`, `transport`, `projection`, `authority`, `build`, `evaluation`,
  `cleanup`, plus `io` or `serialization` when the failure is record-surface
  maintenance rather than loop semantics.
- Record transition boundary and phase in one shape:
  `operation`, `role`, `from_state`, `to_state`, `phase`, `status`, and
  `terminal`. Keep role/state structural; do not create new flattened event
  kinds such as `child_ready_failure` or `successor_handoff_progress`.
- Record missing-transition classification when an expected matching event is
  absent: `expected_event`, `observed_event`, `transition_id` or `runtime_id`,
  `absence_kind`, and `checked_at`.
- Record operator recovery hints as data, not prose-only error strings:
  `recovery_action` such as `inspect_stream`, `inspect_attempt_result`,
  `retry_child`, `resume_from_scheduler`, `repair_projection`, `clean_workspace`,
  `reject_artifact`, or `manual_authority_review`.
- Record source refs and evidence strength: journal line, attempt-result path,
  runner-result path, invocation path, ready/completion path, stream paths,
  scheduler path, branch-registry path, History block hash/head, and whether the
  event is authority, evidence, projection, or diagnostic.
- Record terminal/retryability fields: `retryable`, `blocks_successor`,
  `blocks_selection`, `cleanup_required`, `manual_intervention_required`, and
  optional `next_safe_command`.
- Record bounded excerpts/digests consistently: stderr/stdout path and digest,
  bounded excerpt, exit code, signal/termination if available, child pid as
  environment detail, runtime id as actor/executor.
- Record cross-file consistency failures: expected node id, generation, branch
  id, runtime id, active parent root, artifact/surface/tree key, and actual
  observed values.

## Natural Recording Surface

- One shared tracing-backed JSONL operational event should be emitted at
  transition boundaries and error exits, colocated with the existing Prototype 1
  operational surface rather than adding per-domain files. The natural current
  location is beside `prototype1/transition-journal.jsonl`, because the journal
  is already append-only, cross-runtime, and deserializable for replay, while the
  event must remain telemetry/evidence rather than sealed History authority.
- The event should be written by narrow helpers at existing boundaries:
  child materialize/build/spawn/observe, child `Child<State>` transitions,
  successor selected/checkout/spawn/ready/completion, History store append and
  startup validation, branch evaluation stages, scheduler status changes, and
  cleanup attempts.
- The recording API should look like a small transition/result helper, e.g.
  `op.log_step(...)` and `op.log_result(...)`, so call sites supply the domain
  carrier, status, and source refs without constructing large ad hoc JSON.
- The event should carry optional fields for operation coordinates:
  `campaign_id`, `lineage_id`, `generation`, `parent_node_id`, `node_id`,
  `branch_id`, `runtime_id`, `pid`, `repo_root`, `active_parent_root`,
  `artifact_ref`, `surface_commitment_ref`, `history_block_hash`, and
  `source_refs`.
- History import may later cite these operational events as evidence or ingress,
  but they should not become the authority surface and should not replace
  typed History transitions.

## Essential

- Direct `failure_class` with values that distinguish process, transport,
  projection, authority, build, evaluation, and cleanup.
- `operation`, `phase`, structural `role`/`state`, `status`, `terminal`, and
  `retryable`.
- Stable correlation keys: `campaign_id`, `generation`, `node_id`, `branch_id`,
  `runtime_id`, `transition_id` where present, and source path/line/digest refs.
- Missing-transition classifications for before/after, spawn/ready/observed,
  result-written/observed, successor spawned/ready/completed, and History
  open/seal/append/startup-verify.
- Terminal status and stop reason, including continuation disposition and
  successor completion status.
- Recovery hint and next evidence pointer, especially stream logs, attempt
  result, completion record, scheduler projection, branch registry, or History
  store.
- Authority/projection/evidence distinction on every surfaced status.
- Cleanup-required and cleanup-result fields for worktrees, node target dirs,
  copied binaries, streams, stale ready/completion files, and active checkout.

## Nice To Have

- Failure aggregation by generation, node lineage, branch target, runtime
  surface digest, and policy setting.
- Duration fields per phase and time since last event for stuck-run detection.
- Dashboard-level source digest/root for any derived failure summary.
- Normalized `PrepareError.phase` taxonomy so older string phases can be mapped
  into the uniform failure classes.
- Exit signal and resource counters around process failures when the platform
  can provide them.
- Structured severity: `info`, `warning`, `error`, `fatal`, plus
  `operator_action_required`.
- Recovery outcome tracking: whether a hinted action was attempted and whether
  it cleared the failure.
- Linkage from evaluation-level metrics such as failed tool calls, aborts,
  partial patch failures, and missing submission artifacts into the same
  operational event stream by source ref.

## Too Granular Or Noisy

- Full stdout/stderr payloads inline. Store stream refs, digests, and bounded
  excerpts only.
- Every polling iteration while waiting for successor ready. Record start,
  timeout, ready, or exit-before-ready; do not log each 50 ms poll.
- Every file stat/read used by monitor/report projection unless it changes the
  classification or detects inconsistency.
- Every scheduler mirror write when it does not change semantic status.
- Duplicating full scheduler, branch registry, node, or evaluation JSON inside
  the operational event.
- Per-tool-call failure rows in the loop-level event stream when the run record
  already owns request/result pairing. Use aggregate counts and refs unless a
  tool failure changes node terminal status.
- Treating `pid` churn as identity. Pid is environment detail; `runtime_id` and
  typed role/state are the correlation surface.

## Source Notes

- `crates/ploke-eval/src/cli/prototype1_state/mod.rs:166` defines the intended
  Parent -> child checkout -> child runtime -> selection -> active checkout ->
  successor -> handoff -> cleanup loop. Lines 221-239 state that invocation and
  ready files are transport/debug evidence, not authority; lines 409-437 call
  scheduler reports non-authoritative, temporary child worktrees cleanup
  targets, and degraded runtime provenance something to record explicitly.
- `crates/ploke-eval/src/cli/prototype1_state/history.rs:52` defines History as
  sealed lineage-local authority, not scheduler/report/metrics projections.
  Lines 203-208 say mutable files may be evidence/projections but are not
  authority until admitted. Lines 255-270 list current enforcement and gaps,
  including missing uniform startup admission and missing ingress capture.
- `docs/reports/prototype1-record-audit/history-admission-map.md:29` classifies
  current sources: transition journal, evaluation JSON, attempt results,
  invocations, successor ready/completion, scheduler, registry, node records,
  and streams. Lines 71-77 identify stop reason, stream refs, timestamps, and
  statuses/outcomes as deduped field families. Lines 94-101 name missing
  failure evidence refs, latest-result pointer cleanup, and successor completion
  typed-reader gaps.
- `crates/ploke-eval/src/intervention/scheduler.rs:56` defines node statuses and
  runner dispositions. Lines 107-129 define runner-result terminal fields.
  Lines 515-559 mutate scheduler/node status and failed/completed/frontier
  lists. Lines 627-685 compute and persist continuation stop/continue
  decisions.
- `crates/ploke-eval/src/cli/prototype1_process.rs:79` documents process
  recursion fallout and cleanup risks. Lines 439-478 classify successor build
  failures. Lines 653-714 implement cleanup with path guards. Lines 891-928
  classify successor wait as ready, timeout, or exit-before-ready. Lines
  1219-1272 build persisted child compile/treatment failure results. Lines
  1738-1742 state compile, treatment, and missing runner-result behavior.
- `crates/ploke-eval/src/cli/prototype1_state/journal.rs:76` defines build
  failure info and build result. Lines 117-145 define spawn observations,
  including acknowledged, terminated-before-acknowledged, and ready timeout.
  Lines 165-196 define observed child terminal results. Lines 404-644 replay
  materialize/build/spawn/completion and detect missing phase pairs. Lines
  865-918 classify pending materialization/build/completion. Lines 949-1036
  define recovery-relevant pending dispositions.
- `crates/ploke-eval/src/cli/prototype1_state/child.rs:18` records
  `Child<Ready>`, `Child<Evaluating>`, and `Child<ResultWritten>` as typed
  child-state projections; lines 144-166 emit the transition records.
- `crates/ploke-eval/src/cli/prototype1_state/successor.rs:19` records
  successor selected/spawned/checkout/ready/timed-out/exited/completed states;
  lines 191-205 provide monitor labels for those states.
- `crates/ploke-eval/src/cli/prototype1_state/invocation.rs:402` defines
  successor-ready and lines 416-438 define successor completion with
  succeeded/failed status, trace path, and detail.
- `crates/ploke-eval/src/cli/prototype1_state/report.rs:56` shows the monitor
  report is a provisional aggregate. Lines 148-182 summarize scheduler terminal
  counts and continuation. Lines 303-353 summarize journal kinds. Lines 371-469
  summarize evaluation dispositions and failed tool calls. Lines 606-653 show
  the report currently loads only scheduler, registry, journal, and evaluation
  JSON, not attempt results or successor records.
- `crates/ploke-eval/src/cli/prototype1_state/history_preview.rs:62` imports
  transition journal and adjacent JSON evidence classes, including attempt
  result, ready, completion, runner request, latest result, scheduler, registry,
  and node records. Lines 606-619 classify source treatment and keep scheduler
  and latest runner result projection/degraded.
- `docs/reports/prototype1-record-audit/2026-04-29-monitor-report-coverage-audit.md:20`
  records that monitor report omits node/request/result/invocation/ready/
  completion payloads. Lines 24-33 describe weak coverage and projection-only
  limits.
- `docs/reports/prototype1-record-audit/2026-04-29-record-emission-sites-audit.md:15`
  inventories emitted record families. Lines 28-34 call out duplicated latest
  runner result, successor ready/completion typed-reader limits, stream-log
  refs, and mutable scheduler/node projections.
