# 2026-06-22 — Prototype 1 Typestate Persistence Ledger

Status: active companion ledger / first source-checked pass.

Short description: transition-by-transition notes for `ploke-eval loop walk` / `loop prototype1-state` persisted state, with special attention to which data is authoritative, which data is a projection, which later transition reads it as a hard gate, and where current code assumes a shared filesystem.

Related planning files:

- [`storage-plan.md`](storage-plan.md)
- [`relational-data-model.md`](relational-data-model.md)
- [`crates/ploke-eval/docs/prototype1/persistence-inventory-and-cozo-map.md`](../../../../crates/ploke-eval/docs/prototype1/persistence-inventory-and-cozo-map.md)
- [`crates/ploke-eval/docs/prototype1/typestate-invariants.md`](../../../../crates/ploke-eval/docs/prototype1/typestate-invariants.md)
- [`crates/ploke-eval/docs/reference/persisted-files.md`](../../../../crates/ploke-eval/docs/reference/persisted-files.md)
- [`docs/workflow/evalnomicon/src/prototype1/runtime-authority.md`](../../../workflow/evalnomicon/src/prototype1/runtime-authority.md)
- [`docs/workflow/evalnomicon/src/prototype1/history-crown.md`](../../../workflow/evalnomicon/src/prototype1/history-crown.md)
- [`docs/workflow/evalnomicon/drafts/runtime/parent-child-channel.md`](../../../workflow/evalnomicon/drafts/runtime/parent-child-channel.md)

## Scope and method

This is a first pass from code, not a final schema. It follows the live typestate edge layer in:

- `crates/ploke-eval/src/cli/prototype1_state/live_edges.rs`
- `crates/ploke-eval/src/cli/prototype1_state/parent.rs`
- `crates/ploke-eval/src/cli/prototype1_state/c1.rs` through `c4.rs`
- `crates/ploke-eval/src/cli/prototype1_state/child.rs`
- `crates/ploke-eval/src/cli/prototype1_state/channel.rs`
- `crates/ploke-eval/src/cli/prototype1_state/invocation.rs`
- `crates/ploke-eval/src/cli/prototype1_process.rs`
- `crates/ploke-eval/src/intervention/scheduler.rs`
- `crates/ploke-eval/src/intervention/branch_registry.rs`

Use this ledger as a checklist while introducing storage backends. The key guardrail is that backend-agnostic storage must not change authority boundaries.

Terms used below:

- **Hard gate:** missing, malformed, or inconsistent data blocks the transition or produces a rejected typed outcome.
- **Projection:** durable evidence useful for replay, diagnosis, or operator display, but not the authority boundary by itself.
- **MessageBox:** typed lock/unlock buffer. Current concrete example is the child-plan file.
- **Channel:** typed runtime communication authority. Current concrete transport is JSONL files.
- **History/Block:** sealed lineage authority. Do not treat as generic eval records.
- **EvalStore candidate:** ordinary eval evidence/projection that can move behind an `EvalStore` backend if store scope, visibility, source, evidence class, and import/admission context are preserved.

## First-pass storage domains by backend target

| Data family | Current concrete storage | Authority domain | Backend abstraction target |
| --- | --- | --- | --- |
| Node projections, runner requests/results, branch comparison reports, parent reports, resource samples | JSON/JSONL under `prototype1/` plus passive record mirror | Projection / eval evidence | `EvalStore` candidate, with common scope/source/evidence fields |
| Child-plan file | `prototype1/messages/child-plan/<parent-node-id>.json` | `MessageBox` lock/unlock | Message-box backing first; optional EvalStore mirror after receipt |
| Parent/child and parent/successor channel envelopes | `nodes/<node>/channels/<runtime>/*.jsonl` | `Channel<R, T: Transport>` | `Transport`, not `EvalStore` |
| Invocation bootstrap files | `nodes/<node>/invocations/<runtime>.json` | Attempt bootstrap descriptor | Remote bootstrap package/transport; maybe EvalStore mirror |
| Sealed History | `prototype1/history/*` via `FsBlockStore` | History/Crown/BlockStore | Dedicated `HistoryStore` / `BlockStore` |
| Child worktree, active checkout, committed artifact branches | Git worktrees/branches and `.ploke/prototype1/parent_identity.json` | Artifact/backend authority | Artifact transport / checkout backend, not eval record store |
| Treatment run DBs/snapshots/logs | treatment campaign run dirs, `indexing-checkpoint.db`, `final-snapshot.db`, run records | Child/treatment-local eval runtime | Child-local DB/store by default; import only selected refs/evidence |

## Outer parent typestate ledger

### R0 -> R1 — collect command context and open journal

Source: `live_edges::r0_to_r1`.

Writes:

- Updates active monitor target through `record_active_prototype1_monitor_target`, currently `~/.ploke-eval/prototype1-monitor-target.json`.
- Ensures baseline closure state exists via `ensure_prototype1_baseline_closure_state`; if absent this can recompute and write campaign closure state and run artifacts outside the `prototype1/` subtree.
- Constructs `PrototypeJournal` at `prototype1/transition-journal.jsonl`; construction alone does not append.

Later reads / gates:

- Monitor target is operator convenience, not a hard transition gate.
- Baseline closure state is later used by `R5 -> R6` to build the parent baseline. Missing/incomplete closure evidence blocks baseline establishment.
- Journal path is carried to all later transitions and invocation bootstraps.

Remote-runtime note:

- Current code assumes a local `$PLOKE_EVAL_HOME` and local campaign manifest lookup. A remote child does not need this whole context, only a typed bootstrap subset.

Backend target:

- Monitor target and baseline closure projections are `EvalStore` candidates only after campaign/run owner scope is explicit.
- Journal remains an append-only transition stream for now.

### R1 -> R2a or R3 — initialize or resolve parent identity

Source: `live_edges::r1_to_r2a_or_r3`, `cli_facing::initialize_prototype1_parent_identity`, `cli_facing::resolve_prototype1_parent_identity`, `prototype1_process::validate_prototype1_successor_continuation`.

Writes on `--init-parent-identity` branch:

- Checks out a fresh parent branch.
- Writes `.ploke/prototype1/parent_identity.json` in the active checkout.
- Commits the parent identity file through the workspace backend.

Reads on normal branch:

- Reads `.ploke/prototype1/parent_identity.json` from active checkout.
- If `--handoff-invocation` is present, reads `nodes/<node>/invocations/<runtime>.json` and validates it as a successor invocation.
- Successor path reads sealed History through `FsBlockStore` to validate the selected successor identity/runtime.

Later reads / gates:

- `Parent<Unchecked>::load*` and `Parent<Unchecked>::check` depend on the identity.
- Successor startup cannot proceed unless the invocation and sealed History agree on campaign, node, runtime, selected artifact, and identity.

Remote-runtime note:

- Parent identity is artifact-carried and must remain attached to the checkout/artifact. A remote successor needs the artifact plus this identity witness, not a shared parent filesystem.

Backend target:

- Artifact identity file is artifact/backend state. It may be mirrored for query, but the hard gate is checkout validation plus History verification.

### R3 -> R4a — load `Parent<Unchecked>`

Source: `live_edges::r3_to_r4a`, `Parent::<Unchecked>::load`, `Parent::<Unchecked>::load_with_runtime_id`.

Writes:

- No transition-owned record file writes. The transition emits structured tracing events when tracing is initialized; those events currently land in log sinks rather than typed eval relations.

Reads:

- In-memory parent identity from prior step.
- For handoff, reloads successor invocation to preserve the runtime id.

Later reads / gates:

- Produces the `Parent<Unchecked>` carrier consumed by checkout/startup validation.

Backend target:

- No storage abstraction needed here except keeping invocation load backend-agnostic for remote successor bootstraps.

### R4a -> R4b or R4c — validate checkout and predecessor startup

Source: `live_edges::r4a_to_r4b_or_r4c`, `Parent<Unchecked>::check`, `Startup::<Predecessor>::from_history`, `record_prototype1_successor_ready`.

Writes:

- Predecessor/successor path writes successor ready evidence:
  - `ToParent::SuccessorReady` envelope to `nodes/<node>/channels/<runtime>/child-to-parent.jsonl` when channel endpoints exist.
  - `JournalEntry::Successor(ready)` to `prototype1/transition-journal.jsonl`.

Reads / hard gates:

- Genesis path validates active checkout against artifact-carried parent identity via `GitWorktreeBackend::validate_parent_checkout`.
- Successor path reads successor invocation, checks `active_parent_root`, validates sealed History head, verifies current active checkout artifact tree and surface against sealed block, then admits `Parent<Ready>`.
- `record_prototype1_successor_ready` is what the retired predecessor waits for during handoff.

Remote-runtime note:

- Current predecessor waits by reading the same file-backed channel path. Remote handoff needs a non-file `Transport` or a replicated channel endpoint.

Backend target:

- Successor-ready channel is `Transport` domain.
- Successor ready journal record is projection/evidence.
- Sealed History remains `BlockStore` domain.

### R4b -> R4c — genesis startup into `Parent<Ready>`

Source: `live_edges::r4b_to_r4c_genesis`, `Startup::<Genesis>::from_history`, `Parent<Checked>::ready`.

Writes:

- No transition-owned record file writes. The startup helpers emit structured tracing/observe events; these should become `eval_trace_event` rows or log refs in a DB-backed observability store.

Reads / hard gates:

- Reads `FsBlockStore::lineage_state` for the campaign lineage.
- Genesis requires generation 0 and an absent History head.
- Validated startup identity must match the checked parent.

Backend target:

- `HistoryStore`/`BlockStore`, not `EvalStore`.

### R4c -> R5 — parent-start evidence

Source: `live_edges::r4c_to_r5`.

Writes:

- Appends `JournalEntry::ParentStarted` to `prototype1/transition-journal.jsonl`.
- Appends parent resource sample via `append_parent_target_sample` to the same journal.

Later reads / gates:

- Used by metrics/playback/traversal diagnostics.
- Not currently the hard authority for becoming `Parent<Ready>`; that gate already happened via startup validation.

Backend target:

- Eval evidence / projection. Good early `EvalStore` candidate, but preserve journal replay semantics during migration.

### R5 -> R6 — establish/load parent baseline

Source: `live_edges::r5_to_r6`, `cli_facing::establish_parent_baseline_for_id`.

Writes:

- Generation 0 can run baseline eval/protocol closure and thereby write closure state, run records, logs, snapshots, and related artifacts outside `prototype1/`.
- Later generations read the selected child branch evaluation report instead of recomputing the parent baseline.

Reads / hard gates:

- Generation 0 requires complete baseline closure evidence.
- Successor generations read `prototype1/evaluations/<parent-branch-id>.json` and promote the selected child treatment as the new parent baseline.
- `CompleteBaseline::validate_for_parent` must match campaign, parent node, and branch.
- Missing metrics/record paths in baseline/treatment evidence blocks child fanout.

Remote-runtime note:

- Baseline evidence must be parent-visible. Child-local treatment run DBs should not be blindly imported; only the selected/comparison evidence needed for baseline promotion crosses the boundary.

Backend target:

- Parent baseline summaries and refs are `EvalStore` candidates.
- Run-local DB snapshots stay run-local unless explicitly imported by ref/hash.

### R6 -> R7 — resolve policy budget

Source: `live_edges::r6_to_r7`, `resolve_parent_policy_budget`.

Writes:

- None observed in this edge.

Reads / hard gates:

- Reads run shape/profile/search policy and persisted node counts as needed by policy helpers.
- Produces complete search policy and child-plan budget carried in memory. Later `R7`, `R8`, and `R10` require these facts to exist.

Backend target:

- Policy commitments are parent-owned eval/policy overlays. They should become explicit persisted relations before remote/distributed execution depends on them.

### R7 -> R8 — resolve, publish, and receive child plan

Source: `live_edges::r7_to_r8`, `resolve_child_plan_for_id`, `create_child_plan`, `receive_child_plan`.

Writes vary by candidate generation mode:

- Legacy path:
  - projects parent node to `Running` via `nodes/<parent>/node.json`;
  - older controller may write scheduler/branch/node projections;
  - writes child-plan box file `prototype1/messages/child-plan/<parent-node-id>.json`.
- Broad harness path:
  - writes parent node `Running`;
  - writes broad harness request JSON and prompt under `prototype1/messages/edit-harness-request/`;
  - writes pre-child-planning prompt/artifact under the request's planning artifact path;
  - admitted broad harness results are converted to child nodes/requests and child-plan payload;
  - below-minimum fatal paths may write a rejected-only child-plan and mark parent failed, or deliberately skip terminal plan persistence for resumable non-direct provider failures.
- Deterministic TUI tools path:
  - writes parent node `Running`;
  - writes child node projections and runner requests with `write_treatment_evaluation_projection`;
  - writes child-plan box file;
  - if insufficient proposals, writes rejected-attempt child-plan and marks parent failed.

MessageBox lock/unlock:

- `Open<ChildPlan>::lock` writes the child-plan file and transitions `Parent<Ready> -> Parent<Planned>`.
- `Locked<ChildPlan>::unlock` reads/validates the same file and transitions `Parent<Planned> -> Parent<Selectable>`.
- Existing child-plan replay uses `Locked::<ChildPlan>::from_box` and then unlocks.

Reads / hard gates:

- Child-plan receiver validates the file path equals the body's message path, parent identity matches, and child generation is exactly parent generation + 1.
- `--node-id` must be a member of the received plan and a direct child of the active parent.
- Candidate generation mode may reject empty/malformed plans or invalid surface evidence.

Remote-runtime note:

- The child-plan file is parent-owned campaign state, not a child-worktree file. For remote operation, this should become a typed message-box backing or parent store record with the same lock/unlock semantics.

Backend target:

- The child-plan authority is `MessageBox` domain.
- Child-plan payload can be mirrored into `EvalStore` after `Received<ChildPlan>` for querying.
- Child node/request projections are `EvalStore` candidates.

### R8 -> R9 — schedule planned children

Source: `live_edges::r8_to_r9`.

Writes:

- None observed.

Reads / hard gates:

- Requires child-plan facts and plan budget from prior steps.
- Computes child budget/schedule mode and truncates in-memory child list for some paths.

Backend target:

- Schedule decision should eventually be persisted as parent-owned eval/policy evidence if it affects replay, fairness, or selection explanations.

### R9 -> R10 — choose successor-selection strategy

Source: `live_edges::r9_to_r10`.

Writes:

- None observed.

Reads / hard gates:

- Reads run shape selection config and metric policy.
- Later fanout/selection requires `selection_strategy` fact.

Backend target:

- Policy/config overlay. Persist as parent-owned evidence if selection reproducibility requires it.

### R10 -> R11 — child fanout and optional selection

Source: `live_edges::r10_to_r11`, `run_child_fanout`, `run_adaptive_child_fanout`, `run_planned_child`.

Writes:

- All nested child path writes listed in the C1-C5 section below.
- Rejected-only path writes no child runtime artifacts, but builds selection projection from rejected surface attempts.

Reads / hard gates:

- Requires parent baseline, child budget, schedule mode, selection strategy, and received child-plan facts.
- Stored child outcome recovery reads existing `node.json`, `runner-result.json`, branch evaluation reports, invocations, channels, and transition journal.
- Complete mode requires child outcomes sufficient for selection policy.

Remote-runtime note:

- Current fanout is local process/task orchestration over shared filesystem paths. Remote fanout needs bootstrap package + channel transport + explicit artifact transfer; parent should not directly inspect child-local DBs.

Backend target:

- Parent-owned child outcome summaries and selected evidence are `EvalStore` candidates.
- Child runtime communication remains `Channel` domain.

### R11 -> R12 — project report facts

Source: `live_edges::r11_to_r12`.

Writes:

- None observed; report facts are in memory.

Reads / hard gates:

- Requires planned child count and child outcomes, except rejected-only path.
- Chooses report child from selected node or last child outcome.

Backend target:

- This is a derived projection. If persisted, mark as report/summary, not authority.

### R12 -> R13 — decide stopped vs successor handoff

Source: `live_edges::r12_to_r13`, `spawn_and_handoff_prototype1_successor`.

Writes in all selected cases:

- Appends `JournalEntry::Successor(selected_with_decision)` to transition journal.

Writes on stopped selected case:

- Appends `JournalEntry::Successor(stopped)` to transition journal.

Writes on handoff case:

- Installs selected Artifact into active checkout.
- Writes `.ploke/prototype1/parent_identity.json` for the selected successor in the active checkout and commits it.
- Appends `JournalEntry::Successor(checkout before/after)` and `JournalEntry::ActiveCheckoutAdvanced`.
- Seals a History block via Crown/History code and appends it through `FsBlockStore` before successor spawn.
- Writes successor invocation `nodes/<node>/invocations/<runtime>.json`.
- Creates successor stdout/stderr stream files under `nodes/<node>/streams/<runtime>/`.
- Appends `JournalEntry::Successor(spawned)`.
- On ready, appends `JournalEntry::SuccessorHandoff`.
- On timeout/exit-before-ready, appends corresponding successor journal records.

Reads / hard gates:

- `live_successor_continuation_decision` reads policy, persisted node count, traversal guard, and selection material.
- Handoff requires selected artifact material, active checkout install, artifact target verification, parent identity validation, History seal/append, successor spawn, and successor ready channel observation.
- Successor startup later verifies the sealed History block against the active checkout artifact/surface.

Remote-runtime note:

- This is the densest shared-filesystem assumption: predecessor mutates active checkout, writes invocation, spawns process, opens stream files, and waits on file-backed channel. Remote successor needs artifact transfer/checkout plus non-file channel and an invocation delivery mechanism.

Backend target:

- Selection/stopped/spawned journal records are eval evidence/projections.
- History block append is `HistoryStore` domain.
- Active checkout install is artifact/backend domain.
- Successor invocation is bootstrap transport domain.
- Successor ready is `Channel` domain.

### R13 -> R14 — final report and completion side effects

Source: `live_edges::r13_to_r14`, `emit_final_report_from_parts`.

Writes:

- Appends parent-complete resource sample to transition journal.
- Writes final report JSON at `prototype1_state_report_path(...)`.
- If this process was launched by a successor handoff invocation, sends `ToParent::SuccessorCompletion` and appends `JournalEntry::Successor(completed)`.

Reads / hard gates:

- Requires parent identity and report facts.
- Successor completion is useful to predecessor/operator observability, but the predecessor handoff wait currently gates on ready, not completion.

Backend target:

- Report and completion projections are `EvalStore` candidates; successor completion message remains `Channel` domain.

## Nested child execution ledger

These transitions happen inside `run_planned_child` after a child-plan payload has been received.

### ChildFiles -> C1 — validate candidate payload against active parent artifact

Source: `run_planned_child`, `C1::from_child_plan`.

Writes:

- None observed.

Reads / hard gates:

- Reads child `node`, `runner_request`, and `resolved` from the received child-plan payload.
- Resolves active parent repo root.
- Reads target source file from the active parent artifact and requires it to match `resolved.source_content`.
- Requires node/request/resolved identities to match.

Remote-runtime note:

- Parent currently validates from local active checkout. For remote child creation, the parent still owns the source artifact identity and must validate the candidate against it before bootstrap.

Backend target:

- Payload mirror can be `EvalStore`; hard gate remains typed validation against artifact/backend.

### C1 -> C2 — materialize child artifact/workspace

Source: `MaterializeBranch::transition`, `MaterializeBranch::transition_with_harness`.

Writes:

- Appends `JournalEntry::MaterializeBranch` before/after to transition journal.
- Non-harness path realizes backend-managed workspace under `nodes/<node>/worktree/`.
- Harness path validates an existing candidate workspace from harness evidence.
- Removes stale child binary if present.
- Updates `nodes/<node>/node.json` to `WorkspaceStaged` and workspace root.
- Updates `nodes/<node>/runner-request.json` with workspace root.
- Each JSON record write also goes through `JsonRecordFile::emit`, which mirrors passive records into `records/mirror.cozo.sqlite`. For real eval-home campaign paths under `~/.ploke-eval/campaigns/<campaign>/prototype1`, the mirror is currently rooted at `~/.ploke-eval/records/mirror.cozo.sqlite`; local/test prototype roots mirror under that local root.

Reads / hard gates:

- Non-harness path reads current target source and requires source hash/content match.
- Harness path requires workspace, artifact ids, changed paths, source root, and artifact surface to match admitted harness evidence.
- Later build reads the workspace root and runner request.

Remote-runtime note:

- Current parent materializes local child workspace. Remote child execution may instead receive a git tree/commit or artifact package. Parent should store artifact refs/hashes, not assume direct workspace reads after launch.

Backend target:

- Node/request projections and materialize journal are `EvalStore` candidates.
- Workspace creation is artifact/backend domain.

### C2 -> C3 — build child runtime

Source: `BuildChild::transition`.

Writes:

- Appends `JournalEntry::BuildChild` before/after.
- Creates scratch target dir `nodes/<node>/target/`.
- Creates child binary dir `nodes/<node>/bin/`.
- Runs `cargo check` / `cargo build` with `CARGO_TARGET_DIR` under node dir.
- Copies/promotes built `ploke-eval` binary to `nodes/<node>/bin/ploke-eval`.
- Removes scratch target dir after successful build.
- Updates `nodes/<node>/node.json` to `BinaryBuilt` on success or `Failed` on check/build rejection.

Reads / hard gates:

- Requires no pre-existing promoted child binary.
- Requires built binary exists before promotion.
- Build/check failures are recorded as rejected outcomes, not silent transition loss.
- Later spawn requires the promoted binary and node status path.

Additional side effect after successful C3 in `run_planned_child`:

- `persist_prototype1_buildable_child_artifact` commits child artifact target, appends `JournalEntry::ChildArtifactCommitted`, validates child surface/target, and returns artifact surface used for later selection/History evidence.

Remote-runtime note:

- Building locally under node dirs assumes parent can run the child build on the same machine. Remote execution may build on the child VM and return artifact refs; parent must then validate refs/hashes before selection.

Backend target:

- Build journal/node projection is `EvalStore` candidate.
- Binary and scratch dirs are artifact/build cache, not durable eval authority.
- Committed child artifact refs are artifact/backend evidence; selection may mirror refs into EvalStore.

### C3 -> C4 — spawn child and observe ready

Source: `SpawnChild::transition`, `invocation::write_child_invocation`, `Channel<Child<Starting>>::send_ready` on child side.

Parent writes before/around spawn:

- `nodes/<node>/invocations/<runtime>.json` child invocation.
- `JournalEntry::SpawnChild(Starting)` and `JournalEntry::SpawnChild(Spawned)`.
- Creates stdout/stderr stream files under `nodes/<node>/streams/<runtime>/`.
- Opens file-backed channel endpoints under `nodes/<node>/channels/<runtime>/`.

Child writes during bootstrap:

- Appends `JournalEntry::Child(state=ready)`.
- Sends `ToParent::Ready` to `child-to-parent.jsonl`.

Parent reads / hard gates:

- Parent polls child-to-parent channel for `ToParent::Ready`.
- Exited-before-ready and timeout become rejected spawn outcomes and update node to `Failed`.
- On ready, parent updates `nodes/<node>/node.json` to `Running` and appends observed spawn journal.

Remote-runtime note:

- Invocation file and file-backed channel are currently shared filesystem assumptions. Remote child launch needs invocation delivery plus channel endpoint allocation over a non-file transport.

Backend target:

- Invocation can be mirrored, but executable bootstrap is a launch/transport contract.
- Channel messages stay `Transport` domain.
- Spawn journal/node projection are `EvalStore` candidates.

### Child<Ready> -> Child<Evaluating> -> Child<ResultWritten>

Source: `execute_prototype1_runner_invocation`, `Child::ready`, `Child::evaluating`, `Child::result_written`, `run_prototype1_resolved_branch_treatment`.

Writes:

- Child appends `JournalEntry::Child(state=ready/evaluating/result_written)`.
- Child sends `ToParent::Ready`, `ToParent::Evaluating`, and terminal `ToParent::Result { runner_result, treatment }` when channel exists.
- Child writes attempt result `nodes/<node>/results/<runtime>.json`.
- Child writes latest node result `nodes/<node>/runner-result.json`.
- Child updates `nodes/<node>/node.json` to terminal `Succeeded` or `Failed`.
- Treatment execution writes treatment campaign artifacts/run records/logs/snapshots outside the parent `prototype1/` subtree, including run-local DB snapshots.

Reads / hard gates:

- Child reads its invocation file for campaign/node/runtime/journal/channel/request/resolved data.
- Treatment success requires complete treatment metrics and patch projection validation before returning success-shaped evidence.
- Parent selection-grade success depends on terminal channel `Result` carrying treatment evidence, not merely `runner-result.json`.

Remote-runtime note:

- Treatment DBs and embeddings are child/treatment-local by default. The parent should receive terminal summary/evidence refs over the channel and should not import child code graph facts unless the artifact is selected and an explicit import/revalidation occurs.

Backend target:

- Attempt result and node result are `EvalStore` candidates at the owning runtime scope.
- Terminal child facts cross to parent through `Channel` and then become parent-owned projections.
- Treatment run DBs remain child-local/run-local unless imported by policy.

### C4 -> C5 — observe terminal child result

Source: `ObserveChild::transition`.

Writes:

- Appends `JournalEntry::ObserveChild` before/after.
- Does not write runner result itself; it consumes the channel payload and returns observed evidence.

Reads / hard gates:

- Reads child-to-parent channel and looks for `ToParent::Result`.
- A succeeded runner disposition without treatment evidence is a hard error.
- Timeout waiting for terminal result is a hard transition error.
- Failed runner dispositions produce `ObservedChild::Failed` with after journal evidence.

Remote-runtime note:

- This is the parent-visible boundary for child treatment facts. Non-file transport should preserve identity, body hash, runtime id, and ordered cursor semantics.

Backend target:

- Observe journal and child outcome summary are `EvalStore` candidates.
- Channel remains `Transport` domain.

### C5 -> ParentCompared — compare child treatment against parent baseline

Source: `compare_observed_child_treatment` in `cli_facing.rs`.

Writes:

- Writes `prototype1/evaluations/<branch-id>.json` branch evaluation report.
- Appends parent comparison record to `prototype1/branches.json`.

Reads / hard gates:

- Requires treatment evidence campaign/branch to match the child and parent baseline.
- Later selection uses evaluation report / selection input.
- Later successor generation may promote this evaluation report as the new parent baseline.
- Missing branch evaluation reports can block direct re-entry or force observe recovery from channel.

Remote-runtime note:

- This comparison is parent-owned and should persist in the parent store. It should refer to child/treatment evidence by stable ids/hashes rather than assuming treatment run directories are locally readable forever.

Backend target:

- Strong `EvalStore` candidate: parent-owned comparison report, branch record, selection input, and treatment evidence refs.

## Successor handoff ledger

### Parent<Selectable> -> Parent<Retired> / Crown<Locked>

Source: `Parent<Selectable>::into_retired_and_lineage`, `LockCrown`, `spawn_and_handoff_prototype1_successor`.

Writes:

- The state transition itself logs authority movement.
- Actual durable authority write is the sealed History block appended after `seal_block_with_artifact`.

Reads / hard gates:

- Handoff cannot create executable successor invocation until the parent has crossed into retired state.
- Sealed block binds selected runtime, selected successor identity, active artifact, surface, and selection evidence.

Backend target:

- Crown/History authority is not `EvalStore`.

### History seal/append before successor spawn

Source: `spawn_and_handoff_prototype1_successor`, `handoff_block_fields`, `FsBlockStore::append`.

Writes:

- Sealed block under `prototype1/history/*` via filesystem block store.
- History store head/index projections.

Reads / hard gates:

- Successor startup reads the sealed head and verifies selected identity/runtime/artifact/surface.
- Parent observes append failure as a hard handoff failure before spawn.

Backend target:

- Dedicated `HistoryStore` / `BlockStore`.

### Successor invocation, ready, and completion

Source: `spawn_and_handoff_prototype1_successor`, `record_prototype1_successor_ready`, `record_prototype1_successor_completion`.

Writes:

- Predecessor writes successor invocation JSON and stream files, appends `Successor(spawned)`.
- Successor sends `ToParent::SuccessorReady` and appends `Successor(ready)`.
- Final report path for successor-launched process sends `ToParent::SuccessorCompletion` and appends `Successor(completed)`.

Reads / hard gates:

- Predecessor waits for `SuccessorReady`; timeout/exit paths are recorded.
- Successor validates invocation against sealed History and active checkout before ready.

Remote-runtime note:

- This is an obvious future non-file channel/bootstrap seam.

Backend target:

- Successor-ready/completion channel messages are `Transport` domain.
- Journal projections are `EvalStore` candidates.

## Passive record mirror and observability inventory

Follow-up pass source files:

- `crates/ploke-eval/src/record_emission.rs`
- `crates/ploke-eval/src/layout.rs`
- `crates/ploke-eval/src/intervention/scheduler.rs`
- `crates/ploke-eval/src/runner/msb_single.rs`
- `crates/ploke-eval/src/runner/artifacts.rs`
- `crates/ploke-eval/src/record.rs`
- `crates/ploke-eval/src/protocol_artifacts.rs`
- `crates/ploke-eval/src/tracing_setup.rs`
- `crates/ploke-eval/src/cli/prototype1_state/observe.rs`
- `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs`

### Current `records/mirror.cozo.sqlite` behavior

`JsonRecordFile::emit` is the only current writer that automatically writes the passive Cozo mirror. It:

1. serializes a `ploke_records::record::Record` as pretty JSON;
2. writes that JSON to the requested filesystem path;
3. opens/creates `records/mirror.cozo.sqlite` for the record layout;
4. creates relation `prototype1_record` when missing;
5. inserts one row keyed by `(family, record_path, content_sha256)`.

Current mirror relation:

```text
prototype1_record {
  family: String,
  record_path: String,
  content_sha256: String =>
  schema_version: String,
  record_format: String,
  mirror_schema: String,
  recorded_at: String,
  payload_json: String
}
```

For records under the normal eval-home campaign layout:

```text
~/.ploke-eval/campaigns/<campaign>/prototype1/...
```

the mirror path resolves to:

```text
~/.ploke-eval/records/mirror.cozo.sqlite
```

For local/test Prototype 1 roots that are not under `.../campaigns/<campaign>/prototype1`, the mirror resolves to `<local-root>/records/mirror.cozo.sqlite`.

Design conclusion: this mirror is useful as a compatibility/debug index and dual-write comparison target, but it is not the future `DbEvalStore`. It lacks owner/runtime/artifact scope, authority domain, import/channel receipts, and hard-gate semantics. `DbEvalStore` should supersede it with typed eval relations; the mirror can remain as a passive compatibility mirror during migration.

### Passive `RecordFamily` status

| Record family | Current production writer in this pass | Current mirror status | Producer role/scope | Remote-runtime treatment |
| --- | --- | --- | --- | --- |
| `SchedulerNode` | `save_node_record` via `write_node_projection`, root/child registration, C1/C2/C3 status updates, child terminal status updates | Mirrored through `JsonRecordFile::emit` | Mixed: parent writes planned/running/build states; child writes terminal result status in child process | Split by owner. Parent should import/admit child terminal status from channel before writing parent-owned node projection. |
| `RunnerRequest` | `save_runner_request` via registration/projection helpers | Mirrored through `JsonRecordFile::emit` | Mostly parent-owned bootstrap/planning projection | Parent store or bootstrap package. Remote child should receive request facts through invocation/bootstrap, not by path lookup. |
| `RunnerResult` | `write_runner_result_at`; child writes attempt result and latest node result | Mirrored through `JsonRecordFile::emit` | Child attempt output, later parent-consumed projection | Child-local until terminal `ToParent::Result` is observed. Parent may write/import a parent-owned projection from channel payload. |
| `SchedulerState` | `save_scheduler_state` writes `prototype1/scheduler.json` directly | Not mirrored by current production code | Parent legacy scheduler projection | EvalStore candidate, but must remain non-authoritative for selection/History unless derived from typed transitions. |
| `ChildPlan` | Current child-plan box writes `ChildPlanFiles` via raw JSON helper, not `ChildPlanRecord` | Not mirrored by current production code | Parent-owned `MessageBox` authority | Do not replace with generic record write. Mirror payload only after `Received<ChildPlan>`. |
| `ClosureState` | `recompute_closure_state` writes `closure-state.json` directly | Not mirrored by current production code | Baseline/treatment closure computation | Store parent/treatment summaries and refs; do not collapse child treatment closure into parent graph without import scope. |
| `EvaluationArtifact` | Evaluation and patch projection artifacts are written by runner/projection helpers as ordinary JSON | Not mirrored by current `JsonRecordFile` path in this pass | Run/treatment artifact evidence | Store refs/hashes first; selected/imported evidence may become parent-owned eval records. |
| `ProtocolArtifact` | `protocol_artifacts.rs` writes protocol artifacts directly under eval-home protocol layout | Not mirrored by current production code | Run/protocol evidence | Store refs/hashes and decoded summaries; full artifact import should preserve run/procedure identity. |
| `RunProfile` | Profile files/commitments are admitted by profile helpers | Not mirrored by current production code | Parent/campaign policy input | Parent-owned policy/config relation candidate. |
| `RunProfileCommitment` | Profile commitment is loaded into successor invocation when available | Not mirrored by current production code | Parent/campaign policy commitment | Must be available to successor by bootstrap or replicated parent store view. |
| `AgentTurnTrace` | Runner writes `agent-turn-trace.json`; broad headless-TUI writes turn-live trace | Not mirrored by current production code | Run-local or broad harness attempt evidence | DB should store structured turn rows or path+hash refs; child-local traces cross to parent only by channel/import evidence. |
| `AgentTurnSummary` | Runner writes `agent-turn-summary.json`; broad headless-TUI writes turn-live summary | Not mirrored by current production code | Run-local or broad harness attempt evidence | Summary is good import candidate; preserve model/request/runtime scope. |
| `LlmFullResponseTrace` | Global tracing sink writes `llm_full_response_<run-id>.log`; runner copies slices to `llm-full-responses.jsonl`; broad TUI writes turn-live JSONL | Not mirrored by current production code | Local process/run LLM evidence | Treat as sensitive blob/log evidence. Store refs/hashes and optionally chunked payloads under policy. |
| `RunRecord` | `write_compressed_record` writes `record.json.gz` | Not mirrored by current production code | Run-local aggregate evidence | Store run summary + artifact refs in DB; keep compressed record as compatibility blob until typed ingestion exists. |

Immediate implication: despite the broad `RecordFamily` enum, the current automatic Cozo mirror only covers scheduler node/request/result records in production paths observed here. Most important Prototype 1 artifacts still bypass it.

### Observability and trace/log sinks

Trace and log output are persisted evidence too. A transition that has “no record writes” may still emit structured events into one or more tracing sinks.

| Sink/artifact | Current writer | Current path | Producer role/scope | DB treatment |
| --- | --- | --- | --- | --- |
| Main eval tracing log | `tracing_setup::init_tracing` file layer | `~/.ploke-eval/logs/ploke_eval_<run-id>.log` | Local process: parent, child, successor, or ordinary eval runner | Store as `eval_log_ref` with path/hash; optionally parse into `eval_trace_event` later. |
| Full response global log | `tracing_setup::init_tracing` full-response layer for `FULL_RESPONSE_TARGET` | `~/.ploke-eval/logs/llm_full_response_<run-id>.log` | Local process LLM traffic | Sensitive blob/log. Store ref/hash by default; import payload only under explicit policy. |
| Prototype 1 observation JSONL | Optional tracing layer enabled by `PLOKE_PROTOTYPE1_TRACE_JSONL` | default `~/.ploke-eval/logs/prototype1_observation_<run-id>.jsonl`, or env-provided path | Local process structured events for `ploke_exec`, `chat_http`, `chat-loop` | Best source for first `eval_trace_event` ingestion because it is JSONL and includes span/event fields. |
| `observe::TransitionBuilder` events | `observe::transition::<T>()` | tracing sinks above | Parent child-plan authority transitions | Should become typed `eval_trace_event` rows with transition, authority, record access, duration, outcome, error. |
| `observe::Step` / `observe::result` / `observe::future` / `observe::command_output` | `observe.rs` helpers | tracing sinks above | Parent/child/successor step timing and command outcomes | Should become typed `eval_trace_event` rows. Command stdout/stderr excerpts are small event fields, not full stream logs. |
| Child/successor stdout/stderr streams | Parent-created process stdio files | `prototype1/nodes/<node>/streams/<runtime>/{stdout,stderr}.log` | Child/successor process output captured by parent transport setup | Store as `eval_log_ref` tied to runtime/channel invocation; remote child should upload or return verifiable refs. |
| Runner execution log | `runner/msb_single.rs` | run dir `execution-log.json` | Eval/treatment run | Store structured run summary and log ref. Child treatment logs should stay child/treatment scoped unless imported. |
| Runner setup/status artifacts | `runner/msb_single.rs` | `repo-state.json`, `indexing-status.json`, `parse-failure.json`, `snapshot-status.json`, validation audit | Eval/treatment run | Store refs/hashes and selected summary fields; DB snapshot paths are child/run-local refs. |
| Agent turn artifacts | `write_agent_turn_trace`, `write_agent_turn_summary`, broad TUI live bundle | run dir or `*.turn-live/agent-turn-*.json` | Run-local or broad harness attempt | Store summary rows and trace refs. Parent can import summaries for selection evidence if channel/admission names them. |
| Full response run slice | `persist_full_response_trace_slice` | run dir `llm-full-responses.jsonl` | Run-local LLM evidence copied from global log | Store ref/hash; payload import policy-sensitive. |
| Compressed run record | `write_compressed_record` | run dir `record.json.gz` | Run-local aggregate | Compatibility blob/ref plus derived typed rows over time. |
| TimingTrace stderr markers | `TimingTrace::scope` | stderr of the current process; captured only if that process stderr is redirected | Local process timing marker | Prefer replacing/duplicating with structured `observe::Step` events before relying on it as evidence. |

Design conclusion: trace events are not “less persisted” than JSON records when tracing sinks are active. They are currently outside the passive mirror and should be modeled separately from `eval_record_ref` payload refs.

### Recommended DB treatment from this pass

`DbEvalStore` should have at least three ordinary-evidence lanes, separate from History, Channel, and MessageBox authority:

```text
eval_record_ref    -- typed small records/projections with family/schema/ref/hash and common axes
eval_trace_event   -- structured tracing/observe events with transition/span fields
eval_log_ref       -- path/object-store refs, hashes, byte ranges, redaction/sensitivity flags
```

Use the canonical common axes from [`relational-data-model.md`](relational-data-model.md):

```text
campaign_id
producer_role          -- parent | child | successor | eval_runner | harness | operator | provider | unknown
producer_id            -- runtime_id when known
node_id
run_id / treatment_campaign_id
artifact_id / tree_hash
store_scope            -- parent | child_runtime | successor_runtime | treatment_run | campaign | artifact | external
visibility_scope       -- local | parent_visible | successor_visible | imported | public_debug
source_class           -- direct_write | channel_payload | channel_ref | message_box_mirror | passive_mirror | log_parse | compatibility_import
evidence_class         -- sealed_history | admitted_channel | typed_transition | passive_record | diagnostic | compatibility | unverified
validation_status      -- unchecked | valid | invalid | rejected | degraded | imported
recorded_at / ingested_at
source_ref
content_sha256
```

Do not use the existing mirror as the only database relation because it cannot answer whether a row is child-local, parent-admitted, channel-imported, or merely a local debug projection.

## Shared-filesystem assumptions found in this pass

1. Parent and child both address the same campaign tree under `$PLOKE_EVAL_HOME`.
2. Parent writes child invocation JSON; child opens that path directly.
3. Parent and child communicate through file-backed JSONL channel endpoints.
4. Parent opens child stdout/stderr stream files under the node dir.
5. Parent materializes child workspaces locally and spawns child binaries locally.
6. Child writes `runner-result.json`, attempt results, and node projection files where parent can later read them.
7. Child/treatment run artifacts and run-local DB snapshots are reachable by parent path in some recovery/report paths.
8. Successor handoff writes invocation locally, spawns a local process, and waits on file-backed channel.
9. Active checkout mutation and successor identity commit happen on the predecessor machine before successor launch.

These assumptions are acceptable for the current filesystem backend but should be named explicitly in interfaces. Remote execution should replace them with artifact transfer, bootstrap transport, channel transport, and explicit import/admission records.

## Hard gates to preserve during storage abstraction

- Startup History gates:
  - genesis requires absent lineage head and generation 0;
  - predecessor startup requires sealed head, selected identity, active artifact tree, and surface match.
- Child-plan gates:
  - typed lock/unlock path must match the message body;
  - parent id and child generation must match;
  - selected child must be a direct member of the received plan.
- Child materialization/build gates:
  - source content/hash must match parent artifact;
  - harness surface/artifact ids must match admitted harness evidence;
  - build failures must persist as rejected outcomes, not disappear.
- Runtime channel gates:
  - readiness and terminal result must be observed on the runtime channel;
  - successful terminal result requires treatment evidence or future verifiable refs.
- Comparison/baseline gates:
  - treatment evidence must match campaign/branch;
  - complete metrics/record paths are required before success-shaped comparisons;
  - selected child evaluation report becomes successor parent baseline input.
- Successor handoff gates:
  - active checkout must be advanced to selected artifact;
  - successor parent identity must be written/committed;
  - sealed History block must be appended before successor spawn;
  - successor must validate sealed History before ready.

## Initial EvalStore slices suggested by this ledger

The fixed first slice is parent-owned `R4c -> R5` `ParentStarted` / resource sample evidence. After that, safer slices that do not change authority boundaries are:

1. Structured `eval_trace_event` ingestion from `observe::TransitionBuilder` / `observe::Step` events, or from the optional Prototype 1 observation JSONL sink.
2. Node status projection writes (`node.json`) with common scope/source/evidence fields.
3. Runner request/result projection writes, split by child-local attempt result vs parent-imported terminal result.
4. Parent comparison reports and branch comparison records.
5. Final parent report.
6. Child-plan payload mirror after `Received<ChildPlan>`, explicitly marked as mirror of MessageBox authority.

Do **not** start with:

- sealed History blocks;
- channel messages as ordinary records;
- child-local code graph/embedding DB imports;
- active checkout / artifact mutation;
- invocation delivery as if it were just a record lookup.

## Open follow-up passes

- Fill exact path for `prototype1_state_report_path` and report consumers.
- Split broad harness request/result artifacts into their own mini-ledger.
- Decide whether `records/mirror.cozo.sqlite` remains always-on compatibility mirror, profile-gated mirror, or retired after `DbEvalStore` reaches parity.
- Add exact common-axis values proposed for each EvalStore candidate row: parent runtime, child runtime, successor runtime, treatment run, imported evidence, artifact id/tree, source class, evidence class, and visibility.
- Decide which trace/log payloads are stored inline versus path/object-store refs with hashes.
- Decide which channel-carried child evidence is imported into parent store immediately versus retained as a child-owned ref.
- Define how policy overlays for mutable/immutable/protected edit surfaces are inherited or revalidated by selected successors.
