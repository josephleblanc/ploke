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
| R5 | Parent-start evidence recorded. | parent-start and resource journal entries. | Appends transition journal entries. | Replay shows parent-start evidence; back cursor can move before it but does not remove journal entries. | Current walk reconstructs R5 from matching journal evidence and admits R5 -> R6. |
| R6 | Parent baseline established. | baseline closure state or selected-child promoted baseline evidence; campaign closure artifacts. | May advance eval/protocol closure for baseline. | Replay shows baseline source and completeness. | Current walk reaches/reconstructs R6 when durable baseline evidence exists and admits R6 -> R7. |
| R7 | Policy and child budget ready. | admitted run profile, search policy, child budget, schedule mode, generation/node counts. | Reads profile and projections; may reserve/log budget if current code does. | Replay displays policy derivation. | Current walk reaches/reconstructs R7; live R7 -> R8 requires explicit `walk step --watch`. |
| R8 | Child-plan authority received; parent becomes selectable. | `messages/child-plan/<parent-node-id>.json`, `Received<ChildPlan>`, child files, rejected attempts. | May publish broad-harness requests, collect/receive plan, write child-plan message. | Replay shows plan authority and rejected attempts. | Current walk reconstructs R8 from existing child-plan message evidence and can live-enter via blocking `--watch`. |
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
  - This is kind of a vague question. I leave it to your judgement. Prefer more context over less I guess.
2. **ADR 007 sequencing:** should genesis History admission be implemented before or after durable R0–R5 reconstruction?
  - Hmm. Let's do it after. I don't want to get sidetracked with this just yet. I think one of the benefits of this approach is that implementing the genesis History admission will be a lot easier to debug and implement once we have our typestate setup nicely. If things are broken around parent/successor handoff, which I think they actually are after the first generation because of exactly this issue, we can deal with it when that comes up. But I think this only becomes a concern after we have implemented a lot of other steps, so we can punt for now.
3. **Transition journal idempotency:** which journal entries can be detected as already written, and by what stable key?
  - Hm, I haven't looked at this for a while. I'd say just check whatever `prototype1-state` already does and do that. Same with detection.
4. **Async edge API:** which R6+ edges should use `advance_async`, and how should walk display pending/blocked async work?
  - We should have the default be that the walk returns that the async step started if it takes longer than one second, by default.
  - We should also add a command like `--watch` to not return control until the command completes, and essentially pipe the output of the what the current approach in `prototype1-state` does for these steps. This should also work with `--debug-tools` to show the output. I think it basically just shows the tracing steps for these so the operator has something to look at while the loop is progressing, and it has been helpful in the past to catch some issues mid-run in the past, so we want to keep the option. But we don't want that to be the default, since taking control of a longer-running async command is bad UX. So by default keep it to 1s before returning, and if it isn't done in that 1s also include a command, maybe something like `walk show progress` that will print the progress of the output so far, basically the same thing we print for `prototype1-state` already, and make it possible to do something like `walk show progress --watch` so the operator can choose to leave control of the shell with the server and have the server just keep printing as new stuff arrives, should also work with `--debug-tools`.
5. **Live/replay gates:** exact CLI flags for allowing provider calls, child spawns, checkout installs, and History sealing from `walk`.
  - provider calls should be allowed by default. we don't want to be shy with live api calls here.
  - same with child spawns, that's not really an issue, since they are on an execution path that is part of the immutable surface and operate under essentially a short-lived DAG.
  - history sealing should also be allowed from the walker.
  - checkout installs should include an additional `--allow git-changes` just to make it explicit so it doesn't surprise the operator/us in the future if we forget how this works.
  - the only thing we really really want to be careful about is that we are following all the already existing checks around the parent/successor handoff, which is where there is the possibility of accidentally spawning an unbounded number of detached and long-lived processes, which needless to say would be bad and is not desired. It's not a huge deal, its just that we want to be precise and careful with that handoff. We should already be checking the `History` digests for mutable/immutable surface changes multiple times in both the tool-calling patching loops and then again in the successor checkout/compile/run handoff. For whatever reason you talk a lot about "authority" and this is really the one place where it matters, since this is where we want to be able to prove that there is really only one active runtime on this lineage - by which I mean this succession of parent -> selected child as successor -> next parent handoffs - that has access to the "permissions" to do things like add a new block, create a next parent detached process at handoff, change git state of pwd dir and child node dirs, etc. And since we want to be able to drive the loop from the walker, we want to be able to do this handoff, that's all well and good. We just want to be especially conscientious about this typestate transition is all.
6. **Step-back vocabulary:** command name and UX for read-only back cursor versus any future destructive undo.
  - name not that important. keep it short and simple. use `--force` and similar git-inspired semantics for any future destructive modes, but I don't think we want to add those yet if at all. The intent here is to make it easy to inspect and step forward and backwards through an implementation for debugging or to try tracking why a given loop run did whatever it did, not to prune branches or anything. So for example, we might be looking at a given loop run, which let's suppose has run through multiple generations and then failed at some point, suppose it failed because there was a bug in the way our tool calls for submitting patches via ns_patch or apply_code_edit are defined. We want to be able to start the server, probably list what steps were taken on that run, see the failure, jump to that step with the surrounding context/logging/profile/etc loaded so we have basically the same typestate, and walk it forward to replay the failure the same as it was in the logs. Then maybe we do some digging, find out the cause of the failure, and fix it. We want to be able to start the server and jump to that state, then do a live branch from the step right before the failure, observe the outcome, and see if it succeeded. Suppose our fix didn't work, now we know before needing to do a whole live loop run, which saves us a lot of time. We iterate on the fix, figure out what changed, and run the step again with a new branch from the historical state, and this time it succeeds. That's good, we were able to validate our fix relatively quickly. I think you get the idea.
7. **Server lifecycle at handoff:** stop parent server, mark it read-only, or transfer context? The current preference is stop/read-only parent and fresh successor server.
  - the parent runtime handoff is the right model to track here. server should also terminate and then start again from the new binary. operator-facing should essentially be the same as if the server never died, other than info stating the server change happened, but the operator shouldn't need to add an extra handoff step specifically for the server or anything. We don't want the original server to live because we could be running this loop for something like 100+ generations, and since the loop is self-editing it could change the server semantics, or at least we don't want to preclude that possibility, and the rules/semantics around that are already essentially covered in the mutable/immutable surfaces stuff.
8. **Old `prototype1-step`/`continue`:** when to keep compatibility wrappers versus hiding/deprecating old behavior.
  - just deprecate/feature-gate these. I don't want to spend time migrating/depracating. We'll punt until after "long-term" below achieved. Most likely once we have the long-term version working I'll just delete `prototype1-step`/`continue`.
9. **Graph/replay integration:** when to feed typed driver snapshots into `ploke-tree` playback or egui surfaces.
  - punting on this until the "long-term" below is implemented.

Something you didn't ask about, but that I'd like to clarify, is the behavior of some of the longer-running commands, the worktree setup sequence, and adapter traits for git/files.

### Long Rambling User Notes

#### On longer-running commands.

I'm talking here about the tui_harness and protocol stuff. We already have two kinds of replay semantics around this. I think one is kind of broken, and one works more or less, I don't really recall which, and they need some work, which I don't want to do a deep dive on here but it touches on the typestate and replay/step semantics so I'll touch on it.

Essentially I want to model this whole loop as something that can be stepped through at a granular or coarse level. The coarsest level is what you're implementing now in the type-state transitions. I know its not really that coarse, but its the coarsest level we care about.

The general idea is that the whole loop has these steps, which we've modeled as the typestate module and the transitions etc. But within the loop, we are also calling in to the ploke-tui test harness, which uses all this ploke-llm and ploke-tui machinery for operating the harness, parsing the codebase, doing embeddings, and importantly for this point, manages the llm tool-calling loop and patch staging/acceptance. That tool-calling loop is used by ploke-eval in the loop because we are evaluating the ploke-tui harness in the loop so we can feed back the metrics around that performance back to the ploke-eval loop so it can generate patches that ideally wil lead to improvements in the harness itself. And essentially this tool-calling loop is composed of llm calls, which call the tools, and the tool output, which has logs of what the tool did and whether it succeeded. Each of those api calls to the llm could be modeled as step-through semantics, which is what we've tried to do with some of the replay semantics we have set up I think through ploke-tree.

Ideally, we want to have the coarse-grained loop take each step through the full loop, and then be able to "step into" the llm calls. Think of debuggers like (it's been a while, I think I'm remembering the name right) gcc in C. We essentially want to be able to "step into" the loop and observe each llm tool-call like you would in a gcc debugger, and then - and here is where the metaphor might break with gcc - branch out into a live run starting from whatever that point was, similar to what I've described above in the open design checkpoints.

We already have this, more or less, its just been implemented somewhat poorly, and there are all these record types that are kind of a mess. But the core idea is kind of there already.

I just don't want us to get distracted with implementing that right now. I mean the focus should remain on the coarse level first, and just getting that set up correctly. I mean ideally we would just do this all at once, and as you were implementing the coarse-level typestate stuff, and you reached the llm tool-calling stages like when the parent is generating the patches to apply to the child nodes via I think `ChildPlan`, you would stop and take a side-quest to add the step-through semantics for tool calling. I think we even already have some stuff in ploke-tui to facilitate running ploke-tui and pausing after every tool call - it's really like 90% of the way there. But I'm somewhat concerned that this would end up introducing too much complexity for you to track, and things might end up getting kind of confused.

It's essentially the same story with the protocols, which are the post-tool-call analyses of the given patch generation phase, and are intended to provide some feedback over what is and is not working for the tool calls, where things might be broken in our harness, where things might be failing do to llm model failure, and provide an llm-adjudicated touchstone and starting point to understand where we might want to implement improvements. These are for both us as developers and for the ploke-eval loop to provide as records that we'll want to make available to the llm so it can determine what most needs attention - so far we've been doing that mostly in the HyperAgents style (see .agents/hyper-agents.txt for the research paper from Meta/Stanford), but we'll likely change that to be more directed once we have this coarse-grained level better set up and can more easily iterate on different parts of our pipeline such that we use the protocol output to direct agents in our ploke harness towards areas that are clearly broken or in need of improvements or something, but thats a future problem.

I mention all the protocol stuff because it is intentionally highly automated and produces a mixture of natural language reasoning and machine-readable enums that we can use to show, e.g. "the tool call with the most issues is the code_context tool with 12 `Mixed` usefulness scores and 7 unclear recoverable scores". However, it is exactly because it is ao completely automated - as a strict DAG with guarenteed terminal state - that we need to have an ergonomic way to inspect it more closely so we can realize "oh, yeah this protocol review step totally doesn't work, we should be doing X instead", but that's hard to see by reading dozens or hundreds of json files. So we want to be able to step through that at a fine level as well.

And in the grand, "this is my feature wishlist" version, we want to show all of this in the UI of ploke-egui in a way that lets us step through a historical run and have an intuitive and easy to follow interface that steps through the whole run at coarse or fine-grained levels and (potentially) branch off to debug or test new features, potentially even using ploke-egui as the control plane via the server. There is a lot more I could say there, but I don't want to get into it now. What I do want to say is that this is how the typestate I'm asking you to implement relates to the bigger picture of where we are going with this ploke application as a whole - composed of all the various crates and ploke-tui frontend and ploke-eval benchmarks + self-evolving loop and the ploke-egui frontend for the loop. Its a huge project so sometimes its hard to keep in mind how the big picture relates to the fine details like this, but we care about typestate because it has to interact with all of these other layers of the application, both downward into llm tool calls and agent traces and probably eventually even more fine-graned parts like the code graph, as well as upward into the ploke-egui frontend that displays a graph view of the parent/child nodes and allows the user to have a way of seeing what is happening in the self-evolving loop, and we need it to be very very correct because we want to be able to run this safely for a large number of self-evolving iterations and have confidence that the whole thing stays consistent and dodesn't accidentally or even maliciously somehow attempt to edit its own harness to spawn arbitrarily many detached processes.

So the tl;dr version is: basically we want more step-through semantics to be implemented for tool calling and protocols at some point, and later probably that will be part of the loop, but don't let it distract you from getting this coarser typestate implemented first.

#### worktree setup sequence

you basically have this correct below I think, but my one note is that we would like to have just one command to set up a worktree such that we can then run the server from that repo-root. we've taken some steps in that direction, but the codebase has been worked on since then and teh setup is not as simple as a single command right now, but we'd like it to be. It might make sense to do that as part of working on this typestate stuff, and if you run into issues with repeatedly setting up worktrees, maybe do that here, but otherwise don't sweat it, we'll get to it at some point and the more improtant thing is getting the typestate stuff fully implemented.

#### adapter traits

We have a few adapter traits for storing data, currently using a file system backend but want to leave the door open as much as posible for an agnostic backend, maybe cozo, maybe something else, for all the runtime data. I don't want to get into designing the schema and everything for that here, but I do want to make it clear that this is the intended long-view implementation so we want to use that more.

Basically same idea for the git-backend. We are using git semantics pretty freely, and thats fine for now, but we might want to transition to something else down the road and so we are using a trait adapter for that. Maybe it will be jj, maybe gitbutler, maybe our own thing, who knows, but there is a reason the trait adapter exists.

Basically the same for the harness. This is under-used, but I want to leave the door open for the possibility of making the editor harness swappable someday, because amybe we want to make it possible to use different versions of harnesses we build or maybe we want to be able to run the self-evolving loop on harnesses other people build. I'm not saying work on it now, but it exists for a reason, so just keep that in mind.

## Success criteria

Short-term:

- `walk` can reconstruct R0–R8 after server restart where durable baseline/child-plan evidence exists.
- R4a checkout mismatch displays as a typed blocker with recovery commands.
- R6/R7 are admitted without duplicating baseline side effects when evidence already exists.

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
