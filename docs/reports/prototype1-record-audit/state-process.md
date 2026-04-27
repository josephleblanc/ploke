# Prototype 1 State/Process Record Audit

Scope: `crates/ploke-eval/src/cli/prototype1_state/**` and
`crates/ploke-eval/src/cli/prototype1_process.rs`.

This audit treats a record type as in-scope when these files define it or when
these files clearly write/read a persisted Prototype 1 process/state file whose
Rust type is imported from another module.

## Summary Findings

- Prototype 1 has two persistence models in active use: mutable JSON records
  under `prototype1/` and an append-only typed transition journal at
  `prototype1/transition-journal.jsonl`.
- The transition journal is the better semantic home for role/state transitions,
  but several facts are still duplicated into scheduler/node/runner files for
  controller convenience.
- Child readiness is duplicated as both legacy `ChildReady(ReadyEntry)` and
  typed `Child(Record { state: Ready })`; replay code already accepts both.
- Successor state is split across standalone ready/completion JSON files and
  `JournalEntry::Successor(successor::Record)`.
- Parent identity is persisted as an artifact file and then copied into multiple
  journal records. That is useful for replay, but it risks drift unless the
  artifact identity remains the authority.

## Record Types

### `identity::ParentIdentity`

- Rust type/path: `prototype1_state::identity::ParentIdentity`
- Persisted path(s): `<checkout-root>/.ploke/prototype1/parent_identity.json`
- Writer(s): `identity::write_parent_identity`; called by
  `prototype1_process::install_prototype1_successor_artifact`,
  `prototype1_process::persist_prototype1_buildable_child_artifact`, and
  `cli_facing::initialize_prototype1_parent_identity`
- Reader(s): `identity::load_parent_identity`,
  `identity::load_parent_identity_optional`, `parent::Parent<Unchecked>::load`
  via caller-provided identity, `cli_facing::resolve_prototype1_parent_identity`,
  `prototype1_process::install_prototype1_successor_artifact`
- Semantic role: artifact-carried identity for a checkout that can act as a
  `Parent`; establishes campaign, node, generation, branch, predecessor, and
  artifact branch facts
- Duplicate/overlapping facts: overlaps with external `Prototype1NodeRecord`
  (`node_id`, `generation`, `branch_id`, parent node), branch registry selected
  branch, and journal records embedding full `ParentIdentity`
- Gaps/risks: `parent_id` and `node_id` are currently identical; the artifact
  file is authoritative but journal snapshots can become stale if interpreted as
  live identity
- Cleanup recommendation: keep this as the authoritative parent identity
  artifact; make journal records store identity snapshots only as evidence and
  prefer `Parent<Checked>`/`Parent<Ready>` validation over ad hoc field checks

### `invocation::Invocation` / `ChildInvocation` / `SuccessorInvocation`

- Rust type/path: `prototype1_state::invocation::Invocation` with authority
  wrappers `ChildInvocation`, `SuccessorInvocation`, and classifier
  `InvocationAuthority`
- Persisted path(s): `prototype1/nodes/<node-id>/invocations/<runtime-id>.json`
- Writer(s): `invocation::write_child_invocation`,
  `invocation::write_successor_invocation`; called by
  `prototype1_process::spawn_prototype1_child_runner`,
  `prototype1_process::spawn_prototype1_successor`, and typed
  `c3::SpawnChild`
- Reader(s): `invocation::load`, `load_authority`, `load_executable`;
  consumed by `prototype1_process::execute_prototype1_runner_invocation`,
  `cli_facing::acknowledge_prototype1_state_handoff`,
  `cli_facing::record_failed_successor_turn`
- Semantic role: attempt-scoped authority token for one runtime; distinguishes
  leaf child evaluation from selected successor bootstrap
- Duplicate/overlapping facts: duplicates `campaign_id`, `node_id`,
  `runtime_id`, and `journal_path` that also appear in journal entries and
  node/runner state; successor invocation also duplicates active parent root
- Gaps/risks: schema validation is minimal beyond role classification; child
  invocation does not carry workspace/request facts by design, so correctness
  depends on node/request files remaining consistent
- Cleanup recommendation: keep as the narrow authority contract; avoid adding
  runner request fields here, and route all executable runtime entrypoints
  through `InvocationAuthority`

### `invocation::SuccessorReadyRecord`

- Rust type/path: `prototype1_state::invocation::SuccessorReadyRecord`
- Persisted path(s): `prototype1/nodes/<node-id>/successor-ready/<runtime-id>.json`
- Writer(s): `invocation::write_successor_ready_record`; called by
  `prototype1_process::record_prototype1_successor_ready`
- Reader(s): `invocation::load_successor_ready_record`; polled by
  `prototype1_process::wait_for_prototype1_successor_ready`
- Semantic role: filesystem acknowledgement that a detached successor process
  has started as the next parent
- Duplicate/overlapping facts: same readiness is also appended as
  `JournalEntry::Successor(successor::Record::Ready)` and summarized by
  `SuccessorHandoffEntry`
- Gaps/risks: file existence is part of the handshake; content is only loaded to
  prove parseability and is not deeply checked against the invocation in the
  waiter
- Cleanup recommendation: keep the ready file as the interprocess latch, but
  make the journal record the durable transition projection and validate loaded
  ready records against invocation/runtime before treating them as acknowledged

### `invocation::SuccessorCompletionRecord`

- Rust type/path: `prototype1_state::invocation::SuccessorCompletionRecord`
- Persisted path(s):
  `prototype1/nodes/<node-id>/successor-completion/<runtime-id>.json`
- Writer(s): `invocation::write_successor_completion_record`; called by
  `prototype1_process::record_prototype1_successor_completion` from successful
  and failed successor turns
- Reader(s): no scoped structured reader found; monitor watches/excerpts the
  file path
- Semantic role: terminal status for one successor parent turn
- Duplicate/overlapping facts: mirrored by
  `JournalEntry::Successor(successor::Record::Completed)`
- Gaps/risks: no scoped load helper is used after writing, so completion files
  are mostly evidence rather than controller state
- Cleanup recommendation: either add a structured reader/replay use or demote
  the standalone file to a process-local artifact with the journal as durable
  authority

### `journal::PrototypeJournal` / `journal::JournalEntry`

- Rust type/path: `prototype1_state::journal::PrototypeJournal` storing
  `prototype1_state::journal::JournalEntry`
- Persisted path(s): `prototype1/transition-journal.jsonl`
- Writer(s): `PrototypeJournal::append`; called throughout `c1`, `c2`, `c3`,
  `c4`, `child`, `prototype1_process`, and `cli_facing`
- Reader(s): `PrototypeJournal::load_entries`, `replay_*`, `c3::Handoff`,
  `c4::child_result_path`, monitor summary/watch code in `cli_facing`
- Semantic role: append-only durable event stream for typed Prototype 1
  transitions and replay
- Duplicate/overlapping facts: intentionally projects facts from parent identity,
  scheduler/node records, runner results, successor latch files, and branch
  evaluation reports
- Gaps/risks: mixed old/new variants make the journal partly event-sourced and
  partly snapshot/evidence; replay covers materialize/build/spawn/observe but
  not all parent/successor/artifact variants
- Cleanup recommendation: keep the journal as the transition projection layer;
  complete replay/classification for parent, artifact, and successor variants,
  and phase out legacy duplicate variants where typed variants exist

### `journal::Entry` as `JournalEntry::MaterializeBranch`

- Rust type/path: `prototype1_state::journal::Entry`
- Persisted path(s): `prototype1/transition-journal.jsonl`
- Writer(s): `c1::MaterializeBranch::transition` before/after materialization
- Reader(s): `PrototypeJournal::replay_materialize_branch`, monitor summaries
- Semantic role: before/after record for `C1 -> C2` branch materialization
- Duplicate/overlapping facts: overlaps with node status/workspace root updates
  and branch registry branch/source facts
- Gaps/risks: path/hash snapshots are correct evidence, but live workspace state
  can diverge after the entry
- Cleanup recommendation: treat as immutable evidence and derive recovery from
  replay plus filesystem checks, not by trusting node status alone

### `journal::BuildEntry`

- Rust type/path: `prototype1_state::journal::BuildEntry`
- Persisted path(s): `prototype1/transition-journal.jsonl`
- Writer(s): `c2::BuildChild::transition` before/after build
- Reader(s): `PrototypeJournal::replay_build_child`, monitor summaries
- Semantic role: before/after record for `C2 -> C3` child binary build
- Duplicate/overlapping facts: overlaps with node status `BinaryBuilt`/`Failed`,
  binary path existence, and compile-failure runner results in the older path
- Gaps/risks: build failure details are embedded here while legacy build failure
  can also be normalized into `Prototype1RunnerResult`
- Cleanup recommendation: keep build failure as build-transition data; avoid
  treating compile failures as child runner results once typed flow is primary

### `journal::SpawnEntry`

- Rust type/path: `prototype1_state::journal::SpawnEntry`
- Persisted path(s): `prototype1/transition-journal.jsonl`
- Writer(s): `c3::SpawnChild::transition` through `HandoffTxn`
- Reader(s): `PrototypeJournal::replay_spawn_child`, `c3::Handoff::find_ready`,
  monitor summaries
- Semantic role: parent-side `C3 -> C4` process spawn, stream paths, child pid,
  and handshake outcome
- Duplicate/overlapping facts: overlaps with invocation JSON, stream log paths,
  `ChildReady`/`child::Record`, and node status `Running`/`Failed`
- Gaps/risks: `Starting`, `Spawned`, and `Observed` are phase labels inside one
  record family rather than typed `Child<State>` carriers
- Cleanup recommendation: continue replay support, but converge naming around
  parent-observed spawn state and typed child readiness instead of adding more
  flattened phase/event variants

### `journal::ReadyEntry` as `JournalEntry::ChildReady`

- Rust type/path: `prototype1_state::journal::ReadyEntry`
- Persisted path(s): `prototype1/transition-journal.jsonl`
- Writer(s): `c3::record_child_ready` and legacy handoff paths; also synthesized
  from `child::Record::ready_entry` during replay
- Reader(s): `c3::Handoff::find_ready`,
  `PrototypeJournal::replay_spawn_child`, monitor summaries
- Semantic role: legacy child-side ready witness
- Duplicate/overlapping facts: duplicates `child::Record { state: Ready }`
- Gaps/risks: two ready encodings mean replay and monitoring must remember both
- Cleanup recommendation: stop writing `ChildReady` once all child readiness
  producers use `Child<Ready>`; retain read compatibility until old journals age
  out

### `child::Record` / `child::State`

- Rust type/path: `prototype1_state::child::Record` with
  `prototype1_state::child::State`
- Persisted path(s): `prototype1/transition-journal.jsonl` via
  `JournalEntry::Child`
- Writer(s): `child::Child<S>::record`; called by `record_prototype1_child_ready`
  and `execute_prototype1_runner_invocation` for `Ready`, `Evaluating`, and
  `ResultWritten`
- Reader(s): `Record::ready_entry`, `Record::result_path`,
  `c3::Handoff::find_ready`, `c4::child_result_path`,
  `PrototypeJournal::replay_spawn_child`, monitor summaries
- Semantic role: durable projection of typed `Child<Ready>`,
  `Child<Evaluating>`, and `Child<ResultWritten>`
- Duplicate/overlapping facts: ready duplicates `ReadyEntry`; result-written
  duplicates attempt result file path and latest runner result path
- Gaps/risks: fields are private, which is good, but journal entries can still
  be deserialized from arbitrary JSON; `State` is a projection rather than a
  sealed construction path
- Cleanup recommendation: make this the only child lifecycle journal variant
  and preserve construction through move-only `Child<State>` transitions

### `journal::CompletionEntry`

- Rust type/path: `prototype1_state::journal::CompletionEntry`
- Persisted path(s): `prototype1/transition-journal.jsonl`
- Writer(s): `c4::ObserveChild::transition` before/after observing a child
  runner result
- Reader(s): `PrototypeJournal::replay_observe_child`, monitor summaries
- Semantic role: parent-side observation of terminal child result and reduction
  to succeeded/failed branch evaluation outcome
- Duplicate/overlapping facts: overlaps with attempt-scoped runner result,
  latest runner result, branch evaluation report, and node status
- Gaps/risks: the before entry stores an expected result path; actual result
  discovery depends on `child::Record::ResultWritten`
- Cleanup recommendation: keep as parent observation evidence, but make the
  dependency on `Child<ResultWritten>` explicit in replay/transition APIs

### `journal::ParentStartedEntry`

- Rust type/path: `prototype1_state::journal::ParentStartedEntry`
- Persisted path(s): `prototype1/transition-journal.jsonl`
- Writer(s): `cli_facing::Prototype1StateCommand::run_turn`
- Reader(s): monitor terminal-state logic and summaries
- Semantic role: records that a checked parent began one typed loop turn
- Duplicate/overlapping facts: embeds `ParentIdentity`, repo root, pid, and
  optional handoff runtime already present in invocation/identity files
- Gaps/risks: currently monitored mostly by pid liveness; not part of replay
  aggregate
- Cleanup recommendation: add replay semantics for parent turns or keep it
  clearly monitor-only; avoid using it as alternate parent identity authority

### `journal::ChildArtifactCommittedEntry`

- Rust type/path: `prototype1_state::journal::ChildArtifactCommittedEntry`
- Persisted path(s): `prototype1/transition-journal.jsonl`
- Writer(s): `prototype1_process::persist_prototype1_buildable_child_artifact`
- Reader(s): monitor summaries only in scoped code
- Semantic role: evidence that a child worktree was committed into a
  parent-capable artifact with parent identity
- Duplicate/overlapping facts: duplicates parent identity artifact content,
  node generation/target path, backend branch, and git commit ids
- Gaps/risks: not replayed; may be mistaken for the authority on artifact state
  even though git plus parent identity file are the actual artifact
- Cleanup recommendation: classify as artifact-admission evidence; add replay
  or reporting semantics if it becomes controller state

### `journal::ActiveCheckoutAdvancedEntry`

- Rust type/path: `prototype1_state::journal::ActiveCheckoutAdvancedEntry`
- Persisted path(s): `prototype1/transition-journal.jsonl`
- Writer(s): `prototype1_process::install_prototype1_successor_artifact`
- Reader(s): monitor summaries only in scoped code
- Semantic role: records active checkout advancement to the selected successor
  parent artifact
- Duplicate/overlapping facts: overlaps with `successor::Record::Checkout`,
  parent identity file, branch registry selection, and installed git commit
- Gaps/risks: duplicated with successor checkout before/after records and not
  replayed
- Cleanup recommendation: merge with structured successor checkout transition or
  keep it as a single high-level audit projection, not both

### `journal::SuccessorHandoffEntry`

- Rust type/path: `prototype1_state::journal::SuccessorHandoffEntry`
- Persisted path(s): `prototype1/transition-journal.jsonl`
- Writer(s): `prototype1_process::spawn_and_handoff_prototype1_successor`
  after ready acknowledgement
- Reader(s): monitor summaries only in scoped code
- Semantic role: previous parent observed that successor handoff was
  acknowledged
- Duplicate/overlapping facts: overlaps with successor invocation,
  `SuccessorReadyRecord`, `successor::Record::Spawned`, and
  `successor::Record::Ready`
- Gaps/risks: it is another acknowledgement projection in addition to the latch
  file and successor state record
- Cleanup recommendation: keep either this high-level handoff summary or derive
  it from `successor::Record` plus ready file; avoid three durable ready facts

### `successor::Record` / `successor::State`

- Rust type/path: `prototype1_state::successor::Record` with
  `prototype1_state::successor::State`
- Persisted path(s): `prototype1/transition-journal.jsonl` via
  `JournalEntry::Successor`
- Writer(s): `prototype1_process::append_successor_record`,
  `cli_facing::Prototype1StateCommand::run_turn`; constructors cover
  `Selected`, `Checkout`, `Spawned`, `Ready`, `TimedOut`,
  `ExitedBeforeReady`, and `Completed`
- Reader(s): monitor summaries; no scoped replay reducer found
- Semantic role: typed projection of selected-successor handoff and bounded
  parent-turn lifecycle
- Duplicate/overlapping facts: overlaps with scheduler continuation decision,
  branch selection, active checkout advancement, invocation JSON, ready file,
  completion file, and handoff entry
- Gaps/risks: `runtime_id` is optional because selection/checkout happen before
  spawn; that mixes candidate selection and runtime lifecycle in one record
  family
- Cleanup recommendation: split static successor selection/checkout from
  `Successor<RuntimeState>` lifecycle, or introduce structural state carriers so
  optional runtime identity is not needed

### `cli_facing::Prototype1LoopReport`

- Rust type/path: `prototype1_state::cli_facing::Prototype1LoopReport`
- Persisted path(s): `prototype1/prototype1-loop-trace.json`
- Writer(s): `cli_facing::run_prototype1_loop_controller` via
  `write_json_file_pretty`
- Reader(s): monitor/excerpt paths; no scoped structured reader found
- Semantic role: overwritten controller trace/report for one legacy loop run
- Duplicate/overlapping facts: summarizes scheduler, branch registry, selected
  targets, staged nodes, branch evaluations, and closure state paths
- Gaps/risks: mutable trace can look like state but is overwritten per run and
  has no typed replay semantics
- Cleanup recommendation: keep as CLI report/trace only; do not use for
  controller decisions

### `cli_facing::Prototype1BranchEvaluationReport`

- Rust type/path: `prototype1_state::cli_facing::Prototype1BranchEvaluationReport`
- Persisted path(s): `prototype1/evaluations/<branch-id>.json`
- Writer(s): `prototype1_process::run_prototype1_branch_evaluation` via
  `write_json_file_pretty`
- Reader(s): `prototype1_process::load_prototype1_branch_evaluation_report`,
  `c4::load_report`, comparison/reporting helpers
- Semantic role: durable treatment-vs-baseline evaluation artifact for a branch
- Duplicate/overlapping facts: summarized into branch registry treatment
  evaluation summaries and runner result `evaluation_artifact_path`
- Gaps/risks: path is branch-id scoped under the baseline campaign, so reruns for
  the same branch replace the report
- Cleanup recommendation: decide whether branch evaluation is latest-by-branch
  or attempt-scoped; if attempt-scoped, include runtime/evaluation id in path

### External `Prototype1SchedulerState`

- Rust type/path: `crate::intervention::Prototype1SchedulerState` imported and
  used by scoped files
- Persisted path(s): `prototype1/scheduler.json`
- Writer(s): external helpers called in scope:
  `update_scheduler_policy`, `register_treatment_evaluation_node`,
  `update_node_status`, `record_continuation_decision`
- Reader(s): `load_scheduler_state`, `load_or_default_scheduler_state`,
  monitor summaries, continuation validation
- Semantic role: mutable frontier/completion/continuation mirror for the search
  controller
- Duplicate/overlapping facts: node ids/statuses overlap node records and
  journal transitions; continuation overlaps `successor::Record::Selected`
- Gaps/risks: mutable scheduler can disagree with append-only journal after
  partial failures
- Cleanup recommendation: treat scheduler as a projection/cache and define
  reconciliation from journal plus node records

### External `Prototype1NodeRecord`

- Rust type/path: `crate::intervention::Prototype1NodeRecord` imported and used
  by scoped files
- Persisted path(s): `prototype1/nodes/<node-id>/node.json`
- Writer(s): external helpers called in scope:
  `register_treatment_evaluation_node`, `update_node_status`,
  `update_node_workspace_root`
- Reader(s): `load_node_record` across process/state commands
- Semantic role: durable scheduler-owned node mirror: branch, generation,
  workspace, binary, runner paths, and status
- Duplicate/overlapping facts: overlaps `ParentIdentity`, journal `Refs/Paths`,
  runner request, branch registry, and scheduler node lists
- Gaps/risks: status is mutable and can flatten multi-role state into one enum
  value
- Cleanup recommendation: keep node record as a projection of allowed
  transitions; prefer transition methods to public status writes

### External `Prototype1RunnerRequest`

- Rust type/path: `crate::intervention::Prototype1RunnerRequest` imported and
  used by scoped files
- Persisted path(s): `prototype1/nodes/<node-id>/runner-request.json`
- Writer(s): external registration/workspace update helpers; scoped code reads
  and validates it before running
- Reader(s): `load_runner_request` in child runner execution and typed spawn
- Semantic role: stable per-node execution request consumed by the child runtime
- Duplicate/overlapping facts: overlaps node record workspace/binary/branch and
  invocation campaign/node/runtime context
- Gaps/risks: consistency with node record is checked partly (`binary_path`) but
  remains a separate mutable file
- Cleanup recommendation: either make runner request a derived view of node
  state or enforce full validation before every child execution

### External `Prototype1RunnerResult`

- Rust type/path: `crate::intervention::Prototype1RunnerResult` imported and
  used by scoped files
- Persisted path(s): latest path `prototype1/nodes/<node-id>/runner-result.json`
  and attempt path `prototype1/nodes/<node-id>/results/<runtime-id>.json`
- Writer(s): `prototype1_process::record_attempt_runner_result` writes attempt
  result through `write_runner_result_at` and latest result through
  `record_runner_result`; older path also writes compile failures directly to
  latest result
- Reader(s): `load_runner_result`, `load_runner_result_at`,
  `c4::ObserveChild`, process parent readback, monitor paths
- Semantic role: child attempt outcome and latest node outcome
- Duplicate/overlapping facts: attempt result overlaps `child::Record`
  `ResultWritten`, `CompletionEntry`, branch evaluation report, and node status
- Gaps/risks: latest result is overwritten/cleared while attempt result is
  retained; compile failures can be represented as runner results even when no
  child runtime ran
- Cleanup recommendation: make attempt-scoped result the authority; keep latest
  result as a projection/cache or remove it after readers use attempt ids

### External Branch Registry Records

- Rust type/path: branch registry types imported through
  `crate::intervention` helpers such as `load_or_default_branch_registry`,
  `record_synthesized_branches`, `select_treatment_branch`,
  `mark_treatment_branch_applied`, and `record_treatment_branch_evaluation`
- Persisted path(s): `prototype1/branches.json`
- Writer(s): scoped controller/process calls to the helpers above
- Reader(s): `load_or_default_branch_registry`, `resolve_treatment_branch`,
  branch status/show/apply/report helpers
- Semantic role: mutable synthesized-branch registry, selected branch state, and
  branch evaluation summaries
- Duplicate/overlapping facts: overlaps scheduler nodes, continuation decision,
  successor selection, branch evaluation report, and node branch ids
- Gaps/risks: selection and evaluation summaries are mutable branch-level facts,
  not attempt-scoped transition records
- Cleanup recommendation: keep registry as branch catalog/projection; move
  parent/successor selection decisions into typed journal records with explicit
  parent context

### Process Stream Logs

- Rust type/path: `prototype1_state::journal::Streams`
- Persisted path(s): `prototype1/nodes/<node-id>/streams/<runtime-id>/stdout.log`
  and `stderr.log`
- Writer(s): `c3::open_streams`, `prototype1_process::open_runtime_streams`
- Reader(s): no scoped structured reader; paths are embedded in journal records
- Semantic role: raw process output files for spawned child/successor runtimes
- Duplicate/overlapping facts: stdout/stderr excerpts can also be embedded in
  `FailureInfo` or runner results
- Gaps/risks: log files are not schema records and may be cleaned/rotated
  independently of journal evidence
- Cleanup recommendation: keep streams as raw artifacts referenced by journal
  entries; do not promote them into state records

