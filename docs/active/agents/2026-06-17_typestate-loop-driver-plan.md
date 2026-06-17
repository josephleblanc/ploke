# 2026-06-17 Typestate Loop Driver Plan

Short description: implementation plan for promoting the Prototype 1 typestate pipeline into the primary parent-loop data model and step driver, while keeping the `walk` server as an operator interface rather than loop authority.

Status: active plan; do not treat as implemented behavior until the per-slice checks listed below pass.

Related planning/code:

- `crates/ploke-eval/src/cli/prototype1_state/typestate/`
- `crates/ploke-eval/src/cli/prototype1_state/live_edges.rs`
- `crates/ploke-eval/src/cli/prototype1_state/walk/`
- `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs`
- [`2026-06-16_walk-server.md`](2026-06-16_walk-server.md)
- [`2026-06-17_walk-command-guide.md`](2026-06-17_walk-command-guide.md)
- [`2026-06-15_prototype1-state-observed-walkthrough/run-prototype1-state-turn-phase-inventory.md`](2026-06-15_prototype1-state-observed-walkthrough/run-prototype1-state-turn-phase-inventory.md)
- [`2026-06-15_prototype1-state-observed-walkthrough/run-prototype1-state-turn-smell-clusters.md`](2026-06-15_prototype1-state-observed-walkthrough/run-prototype1-state-turn-smell-clusters.md)
- [`2026-06-02_prototype1-state-loop-walkthrough/README.md`](2026-06-02_prototype1-state-loop-walkthrough/README.md)
- [`2026-06-02_prototype1-state-loop-walkthrough/terminology-conflicts.md`](2026-06-02_prototype1-state-loop-walkthrough/terminology-conflicts.md)
- `crates/ploke-eval/docs/prototype1-loop-operator.md`
- `crates/ploke-eval/docs/prototype1-proof-ladder.md`
- `crates/ploke-eval/docs/prototype1-child-plan-authority/README.md`
- [`docs/active/ADRs/007-prototype1-parent-identity-in-history.md`](../ADRs/007-prototype1-parent-identity-in-history.md)

## Goal

Make the typed `Runtime<Phase, Role, Context, Plan, Children, History, Evidence, Continuation, Report>` pipeline the primary model for driving `prototype1-state`.

The target shape is:

```text
durable loop evidence
  -> reconstruct current typestate phase
  -> apply one typed edge
  -> persist side effects / evidence
  -> reconstruct next typestate phase
```

The `walk` server should become the operator-facing stepping interface for that driver. It may cache state for convenience, but it must not be the authority for loop progress.

## Non-goals

- Do not make an in-memory socket process the production authority.
- Do not weaken checkout, parent identity, successor, History, import, or schema validation to make stepping smoother.
- Do not silently tolerate stale backup fixtures, stale walk servers, dirty checkouts, missing journals, or schema drift.
- Do not implement destructive undo as the default meaning of “step back.”
- Do not remove the existing `prototype1-state`, `prototype1-step`, or `prototype1-continue` surfaces until they are wrappers over the same typed driver and behavior is proven.

## Authority model

### Authority-bearing inputs

The driver should reconstruct/admit phases from durable evidence, including:

- sealed History blocks and `FsBlockStore` projections where the current implementation has them;
- parent identity as an authority input today, with ADR 007 migration toward History authority;
- campaign manifest and admitted run profile;
- transition journal records as transition evidence, not sealed finality;
- child-plan message boxes (`Received<ChildPlan>` authority);
- per-runtime invocation/channel/result files;
- branch evaluation, selection material, successor invocation/ready/completion records.

### Projections / operator views

These should remain read-side/debug surfaces unless explicitly admitted by a typed edge:

- scheduler snapshots;
- branch registry snapshots;
- CLI tables and JSON output;
- walk server memory;
- monitor-target files;
- run-review reports;
- typed shape/delta displays.

### Server rule

The walk server may own:

- a short-lived operator session;
- cached last phase/delta;
- socket lifecycle and epoch guard;
- display preferences.

The walk server must not own:

- sealed History truth;
- transition-journal truth;
- parent or successor identity;
- child runtime finality;
- continuation policy;
- handoff finality.

A killed server must not lose loop progress. A restarted server should be able to reconstruct the current phase from disk once the durable reconstruction slices exist.

## Step semantics

### Step forward

`walk step` should eventually mean:

```text
read durable state -> reconstruct one current phase -> run one admitted typed edge -> persist -> report delta
```

Current in-memory stepping through R5 is a useful prototype, but it should be replaced phase-by-phase with durable reconstruction.

### Step back

Default “back” should not mean “undo side effects.” It should mean moving an operator/replay cursor to a previous durable point.

Initial semantics:

```text
walk back = move/replay cursor backward; do not mutate live campaign state
```

If destructive undo is ever needed, it should be a separate explicit command with a clear destructive name and additional admission checks.

### Historical replay

Historical replay should be read-only by default:

```text
sealed/recorded evidence -> reconstruct typed phase snapshots -> show/diff/step cursor
```

Replay should not call providers, spawn children, write parent identity, alter worktrees, append journals, or seal History unless the operator explicitly branches into live mode.

### Branch historical to live

Branching from historical replay to live should create a new live line with explicit provenance:

- chosen replay cursor / block / transition evidence;
- source campaign/run identifiers;
- new branch/campaign/worktree identity;
- operator admission record;
- reason for leaving read-only replay.

This is not a first slice. It depends on durable reconstruction and replay cursor semantics.

## Current phase map and migration notes

The table below treats R0–R14 as the driver spine. The exact durable evidence column is intentionally conservative: if a row says “TBD,” the first implementation slice for that phase should inventory the existing files before adding live behavior.

| Phase | Typestate meaning | Durable reconstruction evidence | Live side effects | Replay/back behavior | Migration note |
| --- | --- | --- | --- | --- | --- |
| R0 | Raw `Prototype1StateCommand` captured. | CLI request/session config. For a restarted driver, enough command/context must be rebuilt from active context and durable campaign/parent identity. | None. | Reconstructable from operator command or saved driver context. | Current walk constructs in memory. Keep as thin command carrier. |
| R1 | Repo/campaign/run context collected. | repo root, campaign id, campaign manifest, admitted run profile, run shape, transition journal path, monitor target path. | May write/update active monitor target. | Read-only replay can display collected coordinates. | Current `r0_to_r1` is live and canonical. Add durable `R1` reconstruction first. |
| R2a | Parent identity initialization branch. | Generated/committed parent identity today; future genesis History block per ADR 007. | Writes/commits parent identity today. Future: seal/appends genesis History block. | Replay shows initialization evidence; back does not uncommit. | Treat as terminal startup branch. Do not weaken identity validation. |
| R3 | Existing parent identity resolved. | checkout parent identity today or successor handoff invocation; later sealed History head. | None beyond reads/validation. | Replay can show identity source and predecessor links. | Needs clear “authority input vs projection” display. |
| R4a | Parent loaded as `Parent<Unchecked>`. | parent identity, node record, repo root, manifest, optional handoff invocation. | Reads checkout and parent/node files. | Replay can inspect unchecked parent coordinates. | Current R4a failure on wrong checkout is valid and should become a typed blocker, not a loose DB error. |
| R4b | Genesis checkout validated as `Parent<Checked>`. | generation-zero parent identity, active branch/artifact branch match, absent startup History head today. | May inspect/switch checkout depending existing live path. | Replay shows validation inputs. | Future ADR 007 should replace “absent History head + checkout file” with genesis History admission. |
| R4c | Startup validated as `Parent<Ready>`. | genesis validation or predecessor sealed head + successor invocation/ready evidence. | Predecessor path records successor ready/ack evidence. | Replay can show startup validation path. | Genesis and predecessor converge to one ready state; branch kind should be value metadata, not separate post-ready type. |
| R5 | Parent-start evidence recorded. | parent-start and resource journal entries. | Appends transition journal entries. | Replay shows parent-start evidence; back cursor can move before it but does not remove journal entries. | Current walk reaches R5; make R5 idempotent/reconstructable before admitting R6. |
| R6 | Parent baseline established. | baseline closure state or selected-child promoted baseline evidence; campaign closure artifacts. | May advance eval/protocol closure for baseline. | Replay shows baseline source and completeness. | First async/possibly expensive phase after R5. Needs no-live/replay guard. |
| R7 | Policy and child budget ready. | admitted run profile, search policy, child budget, schedule mode, generation/node counts. | Reads profile and projections; may reserve/log budget if current code does. | Replay displays policy derivation. | Normalize policy cluster into typed carrier rather than loose locals. |
| R8 | Child-plan authority received; parent becomes selectable. | `messages/child-plan/<parent-node-id>.json`, `Received<ChildPlan>`, child files, rejected attempts. | May publish broad-harness requests, collect/receive plan, write child-plan message. | Replay shows plan authority and rejected attempts. | This is the child-plan authority boundary; retry must recover from message box. |
| R9 | Child schedule shaped. | planned children, budget, schedule mode, runnable subset. | Usually no durable side effect beyond display/projection unless current code persists schedule choice. | Replay displays runnable set. | Avoid carrying schedule as loose locals. |
| R10 | Selection strategy ready. | successor-selection policy/strategy, seed, metrics mode, History traversal inputs. | Reads History/projections to build strategy. | Replay shows strategy inputs. | `ActiveSelectionStrategy` is still private to `cli_facing`; either move carrier or add typed public equivalent. |
| R11a | Rejected-only branch projected. | rejected surface attempts and selection material. | Writes/report selection evidence if current path does. | Replay shows no-child selection evidence. | Branch from R10; must not require fake child outcomes. |
| R11b/R11c | Child fanout complete. | per-child C1-C5 evidence: materialized artifact, binary, invocation, channel ready/result, runner result, branch evaluation. | Materialize/build/spawn/observe children; may call providers via child runner. | Replay should use channel/result evidence, not spawn. Back cursor does not kill/undo completed child work. | Admit one child sub-edge at a time; reuse existing C1-C5 carriers directly. |
| R12 | Report-child/outcome projection ready. | child outcomes, fallback/report child, branch evaluation report, selection material. | Assembles report facts/projections. | Replay shows outcome projection. | Keep report facts separate from final emitted report. |
| R13a | Continuation stopped/no successor. | successor decision/outcome, read History head, continuation decision stopped. | May append stopped successor/journal records. | Replay shows stop reason. | Branch from R12; no handoff side effects. |
| R13b | Successor handoff committed. | selected child, selection seal material, open/locked/sealed/appended History block, retired parent, successor invocation/ready record. | Install selected artifact, seal/append History, retire parent, spawn successor, wait ready. | Replay can inspect sealed handoff. Branch-to-live later can start from here only with explicit new provenance. | Highest-risk slice; require focused proof before live full loop. |
| R14a | Final report after stopped/no-selection. | completion resource sample and emitted `Prototype1StateReport`. | Appends completion sample and prints/writes report. | Replay displays final report. | No handoff continuation. |
| R14b | Final report after successor handoff. | completion sample, emitted report, successor completion record if bounded successor observed. | Records successor completion and final report. | Replay displays parent/successor closure. | Parent server should stop/become read-only; successor starts fresh context. |

## Proposed driver modules

Names are placeholders; keep final names short and avoid loaded/overloaded terms.

```text
crates/ploke-eval/src/cli/prototype1_state/driver/
  mod.rs
  cursor.rs          # durable phase cursor / replay cursor
  reconstruct.rs     # durable evidence -> typed R-state admission
  advance.rs         # one typed edge runner
  effects.rs         # side-effect admission/idempotency helpers
  replay.rs          # read-only reconstruction over historical evidence
  branch.rs          # later: replay-to-live branching provenance
```

The driver should call canonical direct edge functions from `live_edges.rs` instead of duplicating transition semantics.

The walk server should call the driver rather than owning the driver’s authority. Its controller can keep cached display state, but `show` and `step` should prefer reconstruction once a phase supports it.

## Implementation slices

Each slice should be small enough to validate independently and commit before moving on.

### Slice 0 — Plan and inventory checkpoint

Deliverables:

- this plan;
- code inventory of current direct edges and missing R6+ edge shape;
- phase-by-phase durable evidence checklist beside the code or in this doc.

Verification:

```text
cargo check -p ploke-eval --all-targets
cargo test -p ploke-eval typestate --all-targets
```

### Slice 1 — Durable R0–R5 reconstruction

Goal: `walk show` can recover R0–R5 phase/display after server restart, using disk state where possible.

Steps:

1. Introduce `driver::Cursor` / `driver::Snapshot` with phase, source evidence, and display deltas.
2. Reconstruct R1 coordinates from active context, campaign manifest, admitted profile, and parent identity.
3. Reconstruct R3/R4a from parent identity and parent records without consuming live typestate values.
4. Reconstruct R5 from parent-start journal evidence.
5. Make `walk show` prefer reconstruction; leave in-memory state as cache.

Verification:

```text
cargo check -p ploke-eval --all-targets
cargo test -p ploke-eval loop_walk_ --all-targets
cargo test -p ploke-eval typestate --all-targets
```

Smoke:

```text
ploke-eval loop walk use <clean-parent-worktree>
ploke-eval loop walk start
ploke-eval loop walk step --until r5
ploke-eval loop walk stop
ploke-eval loop walk status
ploke-eval loop walk show   # after restart/autospawn, should reconstruct
```

### Slice 2 — Typed blocker display for R4a checkout mismatch

Goal: valid R4a guard failures should be reported as typed blockers with recovery commands.

Current example:

```text
parent checkout '<dev-checkout>' does not match parent identity active branch
```

Desired display:

```text
blocked edge: r4a_to_r4b_or_r4c
reason: active checkout branch does not match parent identity artifact_branch
repo_root: ...
active_branch: ...
expected_branch: ...
recovery:
  ploke-eval loop walk use <actual-parent-worktree>
  ploke-eval loop walk reset
  ploke-eval loop walk start
```

This remains a hard stop. Do not make checkout validation permissive.

### Slice 3 — R6 baseline edge

Goal: admit `r5 -> r6` through the walk driver.

Requirements:

- establish/reconstruct `CompleteBaseline`;
- separate live baseline advancement from read-only replay;
- mark provider/eval/protocol live side effects explicitly;
- ensure repeated `walk step` after R6 does not duplicate baseline work incorrectly.

Validation:

- focused baseline tests;
- historical baseline fixture where possible;
- no live provider call unless behind existing `live_api_tests` or explicit operator command.

### Slice 4 — R7 policy and budget edge

Goal: move run policy/budget out of loose locals and into the typed driver.

Requirements:

- admitted run-profile load and validation;
- search policy and child budget derivation;
- schedule mode carried structurally or as value metadata;
- display policy provenance.

### Slice 5 — R8 child-plan authority edge

Goal: child-plan publication/recovery is driven by `Received<ChildPlan>` authority.

Requirements:

- recover existing child-plan message before minting/generating new slots;
- preserve zero-admission child-plan persistence invariant;
- keep rejected surface attempts in selection evidence;
- `walk step` should stop after writing/recovering the child-plan authority.

Proofs:

- existing `step_persists_zero_admission_plan`;
- focused retry from existing message box;
- `walk files` shows child-plan and request/result paths.

### Slice 6 — R9/R10 scheduling and selection strategy

Goal: schedule shaping and strategy construction become typed edges.

Requirements:

- runnable child set as value metadata;
- budget/schedule mode in `Plan<..., schedule::Ready<...>>`;
- selection strategy carrier moved out of private `cli_facing` if needed;
- History traversal reads stay explicit and read-only until selection/handoff.

### Slice 7 — R11 rejected-only and child fanout

Goal: admit the R10 branch:

```text
R10 -> R11aRejectedOnly
R10 -> R11FanoutComplete
```

Approach:

1. Implement rejected-only path first.
2. Then admit child fanout one child sub-edge at a time:
   - `ChildFiles -> C1`
   - `C1 -> C2`
   - `C2 -> C3`
   - `C3 -> C4`
   - `C4 -> C5`
3. Keep parent fanout orchestration typed, but reuse C1-C5 carriers directly.

Guardrails:

- no fake successful child results;
- no scheduler-only finality;
- terminal child evidence comes from channel/result files;
- live child model/provider tests stay gated.

### Slice 8 — R12 report facts

Goal: report-child/outcome projection becomes typed and reconstructable.

Requirements:

- select report child / fallback node deterministically;
- carry report facts separately from final emitted report;
- display missing evidence without inventing success.

### Slice 9 — R13a stopped continuation

Goal: no-successor/stopped continuation is a typed terminal branch before final report.

Requirements:

- read current History head without advancing it;
- record stopped/no-selection continuation evidence if live path does;
- `walk step` can reach R14a without handoff.

### Slice 10 — R13b successor handoff

Goal: full successor handoff through typed driver.

Requirements:

- selection material is sealed into History;
- open block -> locked Crown -> sealed block -> appended block is explicit;
- parent becomes retired;
- successor invocation/ready is recorded;
- parent server stops or becomes read-only;
- successor starts a fresh server context from the successor checkout/binary.

Proof before live full loop:

- focused historical handoff reconstruction;
- local no-provider handoff fixture if possible;
- one gated live proof only after historical/local proofs pass.

### Slice 11 — R14 final report

Goal: final report emission is a typed edge for both stopped and handoff branches.

Requirements:

- completion resource sample recorded exactly once;
- final `Prototype1StateReport` emitted from `Report<Facts>`;
- successor completion record handled if bounded successor run completes;
- `prototype1-state` return path uses the typed final state.

### Slice 12 — Replace old control surfaces with driver adapters

Goal: existing commands become frontends over the same typed driver:

```text
prototype1-state
prototype1-step
prototype1-continue
walk step
walk show
```

Migration order:

1. `walk` uses driver for supported phases.
2. `prototype1-step` calls the same driver for one phase.
3. `prototype1-continue` loops over driver steps with existing guard.
4. `prototype1-state` full run becomes a driver loop until final R14.
5. Keep old functions as implementation helpers only where they still own isolated side effects.

### Slice 13 — Replay/back/branch

Only after durable live stepping is stable:

- add read-only replay cursor over journal/History/channel evidence;
- add `walk back` as cursor movement, not undo;
- add historical `show delta` over cursor movement;
- add explicit branch-to-live command with provenance record.

## Testing and use-testing protocol

For each slice:

1. State expected phase/edge and side effects before coding.
2. Add or update focused tests first where practical.
3. Run focused checks:

```text
cargo check -p ploke-eval --all-targets
cargo test -p ploke-eval typestate --all-targets
cargo test -p ploke-eval loop_walk_ --all-targets
cargo test -p ploke-eval prototype1_state --all-targets
```

4. Smoke through `walk` with an isolated runtime socket.
5. If the slice touches backup fixtures, check `docs/testing/BACKUP_DB_FIXTURES.md` before changing fixtures.
6. Use live provider tests only behind existing feature flags or explicit ignored tests.
7. Commit after each working slice.

Operator smoke pattern:

```text
ploke-eval loop walk use <clean-parent-worktree>
ploke-eval loop walk start
ploke-eval loop walk step
ploke-eval loop walk show
ploke-eval loop walk show delta --no-color
ploke-eval loop walk files
ploke-eval loop walk stop
```

For side-effectful phases, run in a prepared parent worktree, not the development checkout.

## Worktree and server setup notes

Before live stepping:

1. Use a clean parent worktree whose active branch matches parent identity / History admission.
2. Build `ploke-eval` in the checkout whose binary will run that checkout.
3. Use `walk use <parent-worktree>` to avoid repeated `--repo-root`.
4. Restart the walk server after code changes; epoch/protocol guards should fail closed on stale servers.
5. For isolated smoke tests, set `PLOKE_EVAL_WALK_SOCKET_DIR` to a temp runtime dir.

Do not use a development checkout with a mismatched branch as the parent runtime checkout unless you are intentionally testing a typed blocker.

## Open design checkpoints

These should be resolved at the slice where they first matter:

1. **R0/R1 persistence:** how much operator command context should be saved for restart reconstruction?
2. **ADR 007 sequencing:** should genesis History admission be implemented before or after durable R0–R5 reconstruction?
3. **Transition journal idempotency:** which journal entries can be detected as already written, and by what stable key?
4. **Async edge API:** which R6+ edges should use `advance_async`, and how should walk display pending/blocked async work?
5. **Live/replay gates:** exact CLI flags for allowing provider calls, child spawns, checkout installs, and History sealing from `walk`.
6. **Step-back vocabulary:** command name and UX for read-only back cursor versus any future destructive undo.
7. **Server lifecycle at handoff:** stop parent server, mark it read-only, or transfer context? The current preference is stop/read-only parent and fresh successor server.
8. **Old `prototype1-step`/`continue`:** when to keep compatibility wrappers versus hiding/deprecating old behavior.
9. **Graph/replay integration:** when to feed typed driver snapshots into `ploke-tree` playback or egui surfaces.

## Success criteria

Short-term:

- `walk` can reconstruct R0–R5 after server restart.
- R4a checkout mismatch displays as a typed blocker with recovery commands.
- R6 is admitted without duplicating baseline side effects.

Medium-term:

- `walk step` can drive R0–R14 through stopped/no-successor paths.
- Child-plan and child fanout use typed authority and C1-C5 carriers without loose local clusters.
- `prototype1-step` and `prototype1-continue` call the same driver as `walk`.

Long-term:

- Full parent/successor handoff is driven by typed edges.
- Server lifecycle is correct across handoff.
- Historical replay and step-back are read-only cursor operations.
- Branching from historical replay to live creates explicit provenance.
- The old loose `run_prototype1_state_turn` path is either removed or reduced to thin wrappers over the typed driver.
