# Progress

## Domain

Current generation, turn phase, semantic step, and phase-to-phase progression.

## Questions To Answer

- During a 5-10 generation run, what generation is the active Parent executing, and which candidate child generation is it trying to produce?
- For the current Parent turn, what phase is active now: planning child candidates, materializing a child workspace, building the child runtime, spawning/evaluating the child, observing the result, selecting a successor, installing the selected Artifact, launching handoff, waiting for successor readiness, or cleanup?
- Which semantic step last completed, which step is currently in progress, and which durable evidence proves that boundary?
- For a selected node, did progression cross the expected path: create child plan -> materialize -> build -> child ready/evaluating/result written -> observe child -> select successor -> update active checkout -> seal/handoff -> successor ready/completed -> cleanup?
- If the loop stopped, was the stop caused by policy, build failure, treatment failure, missing result, successor timeout, successor exit before ready, or a cleanup/install failure?
- Which node, branch, runtime, parent identity, and generation should an operator inspect first when the loop appears stuck?
- For longer runs, can we distinguish generation from lineage-local History height after branching, rejected children, retries, and future merge/fork cases?
- For longer runs, can we reconstruct dwell time and failure rate by semantic phase without treating mutable scheduler state as History authority?
- For longer runs, can an LLM resume from a compact "where is the loop now?" event without scanning every scheduler, journal, invocation, ready, result, stream, and History file?

## Already Answered By Persisted Data

- Parent turn start is persisted in `prototype1/transition-journal.jsonl` as `ParentStartedEntry` with `campaign_id`, `parent_identity`, `repo_root`, optional `handoff_runtime_id`, and `pid`.
- Generation is persisted in multiple projection/evidence records: scheduler nodes and runner requests/results, journal materialize/build/spawn/child/observe records, child artifact commit records, and parent identity. The admission map explicitly treats it as a projection coordinate or entry metadata, not as block height.
- Node status is persisted in `scheduler.json` and mirrored into `nodes/*/node.json` as `planned`, `workspace_staged`, `binary_built`, `running`, `succeeded`, or `failed`.
- Child runtime state is persisted structurally in `JournalEntry::Child(Record { state })` as `ready`, `evaluating`, and `result_written`, with a legacy `ChildReady` projection still accepted.
- Materialize/build/spawn/observe phases are persisted in the transition journal with before/after or starting/spawned/observed phase records and replay helpers that can detect missing counterpart phases.
- Child result evidence is persisted as attempt-scoped `nodes/*/results/<runtime-id>.json`, with a mutable latest copy at `nodes/*/runner-result.json`.
- Successor selection is persisted as `JournalEntry::Successor(State::Selected { decision })` and also as scheduler `last_continuation_decision`; the journal record is the stronger ordering source, while scheduler state is a mutable projection.
- Active checkout update is persisted as `Successor(State::Checkout { phase: before/after })` plus legacy `ActiveCheckoutAdvancedEntry` carrying selected identity, active root, selected branch, and installed commit.
- Handoff progress is persisted as successor journal records for spawned/ready/timed-out/exited/completed, standalone successor ready/completion JSON files, and legacy `SuccessorHandoffEntry`.
- Current report and preview surfaces already read parts of this data. The monitor report reads scheduler, branch registry, transition journal, and evaluation files; the history preview imports transition journal plus adjacent node, invocation, attempt result, successor ready/completion, runner request, runner result, scheduler, and branch registry documents with degraded/projection labels.

## Partially Derivable

- "Current generation" is derivable for the simple current path from parent identity, scheduler max/selected trajectory, child plan generation, and selected successor node. It is not a safe substitute for lineage-local History height.
- "Current phase" can be inferred by joining the latest journal entry, scheduler node status, runner result presence, successor ready/completion files, and process stream paths. This requires a precedence rule because records are split across append-only and mutable surfaces.
- Phase-to-phase progression is mostly reconstructable for a single selected child using journal replay: materialize before/after, build before/after, spawn starting/spawned/observed plus ready, observe before/after, successor selection, checkout before/after, spawned, ready/completed.
- Create/select/update/build/handoff/cleanup can be reconstructed at coarse granularity, but cleanup is weak: child worktree and build-product removal have code paths and errors, not a durable success event.
- Dwell time by phase is partially derivable from `recorded_at`, `created_at`, `updated_at`, and attempt result timestamps. The data model does not consistently preserve separate occurred/observed/recorded times, so latency analysis is approximate.
- The semantic step name is derivable by mapping storage variants into structural facts such as `Child<Ready>`, `Child<Evaluating>`, `Checkout<Advanced>`, and `Successor<Ready>`. This mapping is not one uniform persisted field today.
- Stop reason is partially derivable from scheduler continuation decisions, runner result disposition, build failure results, successor timeout/exit records, and terminal CLI report outcome strings. It is not one consistently recorded operational event.
- Whether a child belongs to the active Parent turn is partially derivable through `ChildPlanFiles`, which binds parent node, child generation, scheduler, branches, node, and runner-request files. That box is stronger than scanning scheduler state, but the operational report still needs to join it manually.

## Requires New Logging

- A uniform operational event is needed for the current semantic step, with fields such as `campaign_id`, `parent_id`, `parent_node_id`, `generation`, `node_id`, `branch_id`, `runtime_id`, `role`, `state`, `phase`, `step`, `status`, `started_at`, `finished_at`, `duration_ms`, `source_ref`, and `error`.
- Record phase boundaries for create/select/update/build/handoff/cleanup as telemetry, not History authority. The event should say what the runtime is doing and cite durable evidence; it must not imply Crown admission by itself.
- Record cleanup completion/failure explicitly for child worktree removal and node-local build-product removal. Today cleanup has side effects and errors, but no durable "cleanup finished" progress fact.
- Record successor build start/success/failure and active-checkout install start/success/failure in the same operational event shape. The current code records checkout before/after and active checkout advanced, but build and install failures surface mainly as `PrepareError` phases.
- Record phase timing with consistent role-specific timestamps instead of relying on mixed `created_at`, `updated_at`, `recorded_at`, and report generation time.
- Record a bounded "waiting" state for long waits: child ready wait, child process wait, successor ready wait, and policy wait/stop. Avoid per-poll events; one start and one result event is enough.
- Record source refs/digests for the event's evidence bundle when the event summarizes multiple records, especially for operator-facing "current progress" snapshots.

## Natural Recording Surface

- Use one shared tracing-backed JSONL operational event stream for liveness/progress, probably emitted through small transition-boundary helpers such as `.log_step()` / `.log_result()` or a local `Progress` context near existing journal writes.
- The natural code surfaces are the transition boundaries that already know the structural state: `run_parent_target_selection` / child-plan lock and unlock, `MaterializeBranch`, `BuildChild`, `SpawnChild`, `ObserveChild`, `record_prototype1_child_ready`, child `evaluating` / `result_written`, `decide_node_successor_continuation`, `spawn_and_handoff_prototype1_successor`, checkout install before/after, successor ready/completion, and cleanup helpers.
- The event should live beside, not inside, sealed History. It may cite `transition-journal.jsonl`, History block refs, scheduler/node paths, invocation paths, result paths, stream paths, and checkout commits as evidence.
- Prefer one uniform JSONL record with optional fields over new domain-specific files. Example semantic fields: `domain="progress"`, `role="parent|child|successor"`, `state="ready|planned|materializing|built|running|selected|checkout|handoff|cleanup|completed|failed"`, `phase="before|after|starting|observed|result"`, `step="create|select|update|build|handoff|cleanup|evaluate|observe"`.
- The History boundary remains separate: History may later admit selected progress events as evidence, but the operational event does not by itself admit a child, lock a Crown, advance a lineage head, or prove successor authority.

## Essential

- Current Parent generation and parent node identity.
- Candidate child node, branch, runtime id, and intended child generation.
- Current semantic step and phase result across create/select/update/build/handoff/cleanup.
- Terminal or blocking reason with source evidence: build failed, treatment failed, missing result, rejected/stop policy, successor timed out, successor exited, install/checkout failed, cleanup failed.
- Event timestamp and duration at phase boundary.
- Evidence refs to the transition journal line or file path/digest that supports the progress event.
- Explicit authority label: `operational_projection` / `transition_evidence`, not sealed History authority unless separately admitted.

## Nice To Have

- Dwell-time summaries by generation, node, and phase.
- Per-generation compact resume record for operators/LLMs: active parent, selected child, last completed step, next expected step, stop reason if terminal, and evidence refs.
- Retry/attempt ordinal for repeated builds or child invocations of the same node.
- Resource samples correlated with phase, especially cargo target size at parent start/complete and child build scratch size before cleanup.
- Source digest bundle for derived progress snapshots, matching the preview/metrics need for repeatable projection artifacts.
- Stable distinction between selected-by-policy, selected-by-temporary-test-short-circuit, selected-by-mutable-projection, and selected-by-sealed-History.

## Too Granular Or Noisy

- Per-poll successor or child wait events.
- Raw stdout/stderr streaming into progress events; keep paths/digests/excerpts only when terminal failure needs them.
- Per-file deletion events during cleanup; record cleanup subject and aggregate result instead.
- Every Cargo compiler line or build artifact path; record build start/result, binary path, target dir, exit code, and log refs.
- Re-emitting whole scheduler, branch registry, invocation, runner result, or evaluation JSON payloads in each progress event.
- Treating every low-level helper call as a step. Operators need semantic phase boundaries, not implementation call traces.

## Source Notes

- `crates/ploke-eval/src/cli/prototype1_state/mod.rs:220-239` states that invocation and ready files remain transport/debug evidence and that History/Crown handoff should use typed boxes/transitions, not ad hoc files.
- `crates/ploke-eval/src/cli/prototype1_state/mod.rs:261-285` defines the child-plan box and its generation check, including the stricter `parent_identity.generation + 1` rule.
- `crates/ploke-eval/src/cli/prototype1_state/history.rs:52-65` defines History, Entry, Ingress, Regime, and Projection; `history.rs:81-100` gives the intended authority sequence. `history.rs:136-149` warns that execution is not Crown authority and process uniqueness is not proven.
- `docs/reports/prototype1-record-audit/history-admission-map.md:31-48` classifies transition journal, evaluation reports, attempt results, invocations, successor records, scheduler, registry, node records, and CLI reports by evidence/projection role.
- `docs/reports/prototype1-record-audit/history-admission-map.md:58-77` covers field ownership for campaign/node/generation/runtime/status/timestamp/stop-reason data; line 60 is the key warning that generation is not block height.
- `docs/reports/prototype1-record-audit/history-admission-map.md:151-165` says the preview imports adjacent documents but keeps node records as projection evidence that cannot override the journal or future sealed History.
- `crates/ploke-eval/src/intervention/scheduler.rs:45-54` defines continuation decision fields; `scheduler.rs:56-65` defines node status; `scheduler.rs:75-174` defines node, runner result/request, and scheduler state fields.
- `crates/ploke-eval/src/intervention/scheduler.rs:515-559` mutates node status and scheduler frontier/completed/failed sets; `scheduler.rs:627-685` decides and records continuation.
- `crates/ploke-eval/src/intervention/scheduler.rs:688-833` registers treatment evaluation nodes with generation, parent node, branch, candidate, runner request, and paths.
- `crates/ploke-eval/src/intervention/branch_registry.rs:216-350` records synthesized/selected branch candidates; `branch_registry.rs:452-544` marks a selected branch; `branch_registry.rs:682-710` writes evaluation summaries into the registry projection.
- `crates/ploke-eval/src/cli/prototype1_state/journal.rs:62-145` defines materialize, build, and spawn progress records; `journal.rs:154-196` defines ready and completion records.
- `crates/ploke-eval/src/cli/prototype1_state/journal.rs:198-212` defines parent-start records; `journal.rs:273-330` defines child artifact commit, active checkout advancement, and successor handoff legacy evidence; `journal.rs:332-353` lists all journal variants.
- `crates/ploke-eval/src/cli/prototype1_state/journal.rs:404-594` has replay grouping for materialize/build/spawn/observe, which is the current best derivation surface for phase-to-phase progression.
- `crates/ploke-eval/src/cli/prototype1_state/child.rs:18-40` defines structural child states; `child.rs:144-167` records allowed `Child<Ready>`, `Child<Evaluating>`, and `Child<ResultWritten>` transitions.
- `crates/ploke-eval/src/cli/prototype1_state/successor.rs:19-56` defines selected/spawned/checkout/ready/timed-out/exited/completed successor states; `successor.rs:191-205` gives monitor labels.
- `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:331-386` runs target selection and writes/receives the child-plan message; `cli_facing.rs:518-570` validates child-plan generation and children.
- `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:3335-3368` appends parent-start and parent-start resource samples; `cli_facing.rs:3384-3496` executes materialize/build/spawn/observe transitions; `cli_facing.rs:3547-3578` records successor selection, continuation decision, and handoff attempt; `cli_facing.rs:3637-3662` emits the final state report.
- `crates/ploke-eval/src/cli/prototype1_process.rs:20-25` states the current-generation vs candidate-next-generation boundary; `prototype1_process.rs:39-50` states the intended successor handoff flow.
- `crates/ploke-eval/src/cli/prototype1_process.rs:666-714` performs cleanup without a durable cleanup event; `prototype1_process.rs:716-787` persists buildable child Artifact evidence.
- `crates/ploke-eval/src/cli/prototype1_process.rs:930-1107` seals/appends the History handoff block, spawns the successor, records spawned/ready/timeout/exit evidence, and returns a retired parent.
- `crates/ploke-eval/src/cli/prototype1_process.rs:1352-1427` builds the child binary and records compile failure as runner result; `prototype1_process.rs:1727-1907` documents and executes the parent-side staged child path including cleanup after failure/success.
- `docs/reports/prototype1-record-audit/2026-04-29-monitor-report-coverage-audit.md:7-16` summarizes monitor report reads and derived identifiers; lines 18-33 list missing artifacts from that report surface.
- `docs/reports/prototype1-record-audit/2026-04-29-record-emission-sites-audit.md:15-27` inventories emitted record families; lines 28-35 call out duplicated mutable projections and weak successor/stream handling.
- `docs/reports/prototype1-record-audit/2026-04-29-history-crown-introspection-audit.md:105-125` identifies structural naming and authority/provenance risks relevant to progress events.
