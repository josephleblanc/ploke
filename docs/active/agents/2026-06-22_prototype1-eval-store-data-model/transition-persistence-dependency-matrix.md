# Prototype 1 Eval Store — Transition Persistence Dependency Matrix

Status: active planning note / source-derived test planning aid.

Related files:

- [`live-transition-test-plan.md`](live-transition-test-plan.md)
- [`typestate-persistence-ledger.md`](typestate-persistence-ledger.md)
- [`storage-plan.md`](storage-plan.md)
- [`database-planning-notes.md`](database-planning-notes.md)

## Purpose

We want isolated tests for the full live typestate transition inventory, including live provider/API calls where the real transition uses a provider. A full loop can take roughly 30–35 minutes, so the test plan should avoid waiting for a full run between every storage change.

The risk is that an early transition can appear unchanged locally while failing to persist data that a later transition needs. This matrix lists persisted data produced earlier and consumed later, so tests can be selected by dependency rather than by repeatedly running the whole loop.

Use this as the bridge between:

- the source-checked transition ledger in [`typestate-persistence-ledger.md`](typestate-persistence-ledger.md);
- the live transition test gate in [`live-transition-test-plan.md`](live-transition-test-plan.md);
- the first `fs | database | dual-strict` implementation slices in [`storage-plan.md`](storage-plan.md).

## Testing principle

For each persisted surface, test both sides:

1. **Producer contract:** the transition that writes the surface persists every field/hash/ref later consumers require.
2. **Consumer contract:** each later transition can load only that persisted surface plus its declared pre-state and still enforce the same gates.

This lets us run a live API producer transition once, checkpoint its output, and then run many downstream consumer tests from restored checkpoints without repeatedly paying the full live-loop wall time.

## Dependency matrix

Authority class values:

- **Authority:** later transition must trust this surface as part of the transition gate.
- **Transport authority:** channel/IPC semantics; DB rows may mirror but cannot replace it.
- **MessageBox authority:** lock/unlock/read semantics; DB rows may mirror but cannot replace it.
- **Artifact authority:** checkout/worktree/backend state; DB rows may mirror refs but cannot replace mutation/validation.
- **History authority:** sealed block/hash/lineage semantics; DB rows may cite but cannot replace it.
- **Evidence/projection:** useful for replay, reports, selection evidence, or migration; not the hard authority by itself.
- **Compatibility projection:** legacy mutable/cache surface; keep explicit and low-strength.

| Producer | Persisted data | Authority class | Later consumers / gates | Test implication |
| --- | --- | --- | --- | --- |
| Setup / admitted profile | `campaign.json`, `run-profile.toml`, `run-profile.commitment.json` | Authority/evidence depending on field | `R0 -> R1` resolves campaign/run shape; `R6 -> R7` policy budget; `R9 -> R10` selection strategy; `R12 -> R13` continuation decision. | Build a stable fixture with admitted profile and commitment. Storage changes must not fall back to stale scheduler policy when admitted profile exists. |
| Setup / root registration | `prototype1/scheduler.json` | Compatibility projection | `resolve_parent_policy_budget` fallback, preview/metrics/recovery tools. Active typed loop should not use it for selection or History admission. | Consumer tests should warn/fail if scheduler projection becomes stronger than admitted profile/typed facts. |
| `R0 -> R1` | active monitor target | Projection | Operator monitor/watch surfaces. Not a hard transition gate. | Exclude from correctness gates; verify only operator metadata if touched. |
| `R0 -> R1` / generation-0 setup | baseline closure state, baseline run artifacts, logs, snapshots | Evidence, partly treatment/run-local | `R5 -> R6` establishes generation-0 parent baseline; child comparison later uses this baseline. | If storage slice touches baseline refs, test `R5 -> R6` and `C5 -> ParentCompared` consumers from restored baseline artifacts. |
| `R0 -> R1` | `prototype1/transition-journal.jsonl` path and append stream | Evidence/projection | Child/successor invocations carry journal path; replay, walk, metrics, reports, recovery read the journal; many later transitions append. | Journal must stay append-compatible in `fs`; DB rows can mirror but cannot remove JSONL before replay consumers migrate. |
| `R1 -> R2a` parent initialization | `.ploke/prototype1/parent_identity.json` committed in active checkout | Artifact authority | Normal `R1 -> R3`, `R3 -> R4a`, `R4a -> R4b`, successor validation, active checkout checks. | Producer test must mutate/commit the artifact identity. Consumer tests should load identity from checkout, not DB mirror. |
| `R1 -> R3` successor path / predecessor handoff | successor invocation `nodes/<node>/invocations/<runtime>.json` | Bootstrap authority | `R3 -> R4a` preserves runtime id; `R4a -> R4c` validates invocation campaign/node/root; successor startup. | Handoff fixtures must include invocation JSON; DB mirror is not executable bootstrap until a bootstrap backend exists. |
| `R4a -> R4c` predecessor path | successor-ready channel envelope and successor-ready journal entry | Transport authority + evidence | Predecessor handoff wait in `R12 -> R13`; operator/replay. | Channel test must read transport envelope/cursor; DB `eval_channel_message` is only mirror. |
| `R4c -> R5` | `ParentStarted` journal entry and parent-start resource sample | Evidence/projection | Walk/replay/metrics; final run review. | Good early `EvalStore` slice. Consumer test can query/report without live API. |
| `R5 -> R6` | parent baseline summary/ref in memory; generation-0 baseline artifacts if recomputed | Evidence | `R10 -> R11` requires `parent_baseline`; `C5 -> ParentCompared` compares child treatment against it. | Isolated `R10 -> R11` fixtures must include a complete baseline. Missing metrics/record refs should fail before fanout. |
| `R6 -> R7` | complete search policy / child-plan budget currently in memory; admitted policy/profile persisted before run | Authority/evidence | `R7 -> R8`, `R8 -> R9`, `R10 -> R11`, `R12 -> R13` continuation. | When persisted later, assert profile commitment/hash and distinguish from scheduler fallback. |
| `R7 -> R8` | parent `node.json` status updates | Compatibility projection | Preview/recovery/report; some path joins. | Keep as `eval_record_ref`/candidate event; do not make node status projection selection authority. |
| `R7 -> R8` | child-plan MessageBox file `prototype1/messages/child-plan/<parent-node-id>.json` | MessageBox authority | Same edge unlock/receive; `--node-id` membership validation; child plan facts drive `R8`, `R10`, nested child C1. | Live producer test must verify lock/unlock path/body/parent/generation. DB payload mirror only after `Received<ChildPlan>`. |
| `R7 -> R8` legacy/deterministic paths | child node projections and `runner-request.json` files | Compatibility projection / bootstrap evidence | C1 validates child payload; C2 updates workspace; C3/C4 spawn uses request; child invocation is built from request. | Changing these writers requires C1/C2/C3 consumer tests from restored child-plan fixture. |
| `R7 -> R8` broad harness path | edit-harness request JSON, prompt/planning artifacts, admitted harness results | Evidence; provider-facing | Admitted results become child nodes/requests/plan; rejected-only path can drive R10 rejected branch. | This is a live API producer transition. Checkpoint after received plan so schedule/fanout tests do not rerun planning API unnecessarily. |
| `R8 -> R9` | child budget/schedule mode currently in memory | Evidence once persisted | `R10 -> R11` fanout. | Producer has no filesystem write today; if persisted later, test truncation/schedule semantics against child-plan membership. |
| `R9 -> R10` | successor selection strategy currently in memory | Evidence once persisted | `R10 -> R11` selection and `R12 -> R13` continuation explanation. | If persisted later, bind to profile commitment/metrics policy. |
| `C1 -> C2` | materialize journal entries; child worktree/workspace; updated `node.json`; updated `runner-request.json`; passive mirror rows | Artifact authority + evidence/projection | `C2 -> C3` builds workspace; `C3` commits child artifact; reports use workspace paths. | Consumer test must build from persisted workspace/request, not in-memory candidate only. DB row cannot replace workspace artifact. |
| `C2 -> C3` | build journal; target/bin dirs; promoted child binary; terminal build status in `node.json`; child artifact commit/surface evidence | Artifact/build authority + evidence | `C3 -> C4` spawns binary; selection/handoff uses child artifact refs; History seal binds selected artifact. | Expensive but local. Checkpoint after build to test spawn/observe without rebuilding. |
| `C3 -> C4` parent side | child invocation JSON; stdout/stderr stream files; channel endpoint files; spawn journal | Bootstrap + transport authority + evidence | Child process reads invocation; parent waits for `Ready`; logs/replay use streams/journal. | Spawn tests must prove invocation is complete and channel endpoints work. DB mirrors cannot replace invocation/channel. |
| `C3 -> C4` child side | `ToParent::Ready` channel envelope; child ready journal | Transport authority + evidence | Parent `SpawnChild` observes ready; failure/timeout paths update node. | Ready test should assert channel cursor/order/body hash, not just node status. |
| `Child<Ready> -> Child<Evaluating> -> Child<ResultWritten>` | child evaluating/result journal; `ToParent::Evaluating`; terminal `ToParent::Result`; attempt result `results/<runtime>.json`; latest `runner-result.json`; terminal `node.json`; treatment artifacts/logs/snapshots | Transport authority for parent visibility; evidence/projection for result files; treatment-local evidence | `C4 -> C5` consumes terminal channel result; `C5 -> ParentCompared` requires treatment evidence; reports/recovery read result files. | Main live API producer transition. Checkpoint terminal channel/result/treatment evidence so compare/selection tests do not rerun child provider calls. |
| `C4 -> C5` | observe journal before/after; observed child outcome in memory | Evidence | `C5 -> ParentCompared`; `R11 -> R12` report facts through child outcomes. | Consumer test should fail if success-shaped channel result lacks treatment evidence. |
| `C5 -> ParentCompared` | `prototype1/evaluations/<branch-id>.json`; `prototype1/branches.json` comparison record | Evidence/projection, strong selection input | `R10 -> R11` selection/recovery; `R11 -> R12` report; `R12 -> R13` successor material; next generation `R5 -> R6` promotes selected child as parent baseline. | Critical early-late dependency. Any storage migration here requires same-generation selection tests and next-generation baseline promotion tests. |
| `R10 -> R11` rejected-only branch | rejected surface attempts projected into selection/report facts; may rely on broad harness rejection artifacts | Evidence | `R11 -> R12` rejected-only report; `R12 -> R13` stopped/no-selection path. | Test separately from child fanout so no child API/spawn is needed. |
| `R12 -> R13` selected/stopped path | successor selection/stopped journal records | Evidence; History may later cite selection material | `R13 -> R14` report; replay/run review. | DB rows can mirror decision evidence but must not be the sealed History authority. |
| `R12 -> R13` handoff path | active checkout advanced; successor parent identity committed; selected artifact installed | Artifact authority | Successor `R1 -> R4c` validates active checkout and identity; sealed block binds artifact/surface. | Handoff fixture must include actual checkout mutation, not just DB row. |
| `R12 -> R13` handoff path | sealed History block and head/index projections under `prototype1/history/*` | History authority | Successor startup validates sealed head, selected identity/runtime/artifact/surface; later lineage traversal. | History tests remain separate. EvalStore tests may cite block refs only. |
| `R12 -> R13` handoff path | successor invocation JSON; successor streams; successor channel endpoints; spawned/handoff journal | Bootstrap + transport authority + evidence | Successor process reads invocation; predecessor waits for `SuccessorReady`; final report records handoff status. | Checkpoint after spawn/ready to test successor finalization without replaying selection API. |
| `R13 -> R14` | parent-complete resource sample; final `prototype1_state_report_path(...)` JSON; successor completion channel/journal if launched by handoff | Evidence/projection + transport for completion | Operator status, run review, predecessor/operator observability. | Final report tests should be local from R13 fixture. Completion channel remains transport. |
| Passive record emission | `records/mirror.cozo.sqlite` relation `prototype1_record` | Compatibility mirror | Debug/backfill only; should not drive live gates. | Do not use mirror parity as proof of `DbEvalStore`; typed rows need owner/scope/import context. |

## High-risk producer/consumer chains

### Profile and policy chain

```text
run-profile + commitment
-> R0/R6/R7/R9/R12
-> child budget, selection strategy, continuation decision
```

Risk: a storage change appears fine through planning but later selection/continuation silently uses scheduler fallback or stale profile data.

Test shape: alter or remove scheduler projection in a fixture and prove admitted profile still drives policy; separately prove missing admitted profile fails or explicitly falls back only where allowed.

### Child-plan chain

```text
R7 child-plan MessageBox
-> R8 schedule
-> R10 fanout
-> C1 child payload validation
```

Risk: payload mirror exists in DB, but MessageBox path/body/parent/generation lock-unlock semantics are broken.

Test shape: producer test validates lock/unlock; consumer tests restore received plan facts plus file and prove member/non-member selection behavior.

### Child runtime chain

```text
C1 workspace/request
-> C2 build
-> C3 invocation/channel/spawn
-> child ready/evaluating/result
-> C4 observe terminal
```

Risk: early filesystem-to-store migration preserves JSON-looking records but omits workspace path, binary path, runtime id, channel root, or attempt result fields needed later.

Test shape: checkpoint at C2/C3/C4 boundaries. For any writer in this chain, run all downstream consumers from the nearest checkpoint rather than a full loop.

### Treatment/evaluation/selection chain

```text
terminal channel result + treatment evidence
-> C5 comparison report
-> R10/R11 selection material
-> R12 continuation/handoff
-> next-generation R5 baseline promotion
```

Risk: same-run selection passes, but next-generation baseline promotion fails because evaluation report refs/metrics/branch ids were not persisted correctly.

Test shape: after a live child result, checkpoint terminal evidence; run compare, selection, and next-generation baseline promotion as separate consumer tests.

### Successor handoff chain

```text
selected artifact
-> active checkout mutation + successor identity
-> sealed History append
-> successor invocation
-> successor ready channel
-> successor startup validation
```

Risk: predecessor reports handoff success while successor only worked due to shared in-memory state or stale files.

Test shape: checkpoint after predecessor handoff, then launch/validate successor from persisted invocation, active checkout, and sealed History only.

## Checkpoint ladder for minimizing live API time

Create reusable fixtures/checkpoints at transition boundaries. A checkpoint must include the full campaign tree, active checkout/artifact state, relevant run artifacts, and enough metadata to verify hashes/ids before use.

Suggested ladder:

| Checkpoint | Boundary | Main use | Live API needed to create? |
| --- | --- | --- | --- |
| `F0_setup` | campaign/profile admitted, root parent identity ready or init path prepared | R0/R1/R2/R3 startup tests | No |
| `F1_ready_parent` | after `R4b -> R4c` or predecessor `R4a -> R4c`, before `R4c -> R5` | parent-start/baseline tests | No, unless predecessor fixture came from live handoff |
| `F2_baseline_complete` | after `R5 -> R6` | policy/schedule/fanout tests | Maybe, if generation-0 baseline computation calls providers |
| `F3_child_plan_received` | after `R7 -> R8` | schedule/fanout/C1 tests | Yes for broad/provider child planning; no for deterministic modes |
| `F4_child_materialized_built` | after `C2 -> C3` plus artifact commit | spawn/ready tests | No |
| `F5_child_terminal_result` | after child `ResultWritten` with terminal channel/result/treatment evidence | observe/compare/selection tests | Yes |
| `F6_child_compared` | after `C5 -> ParentCompared` | report/selection/continuation tests | No if built from F5 |
| `F7_selection_ready` | after `R11 -> R12`, before `R12 -> R13` | stopped/handoff branch tests | No if built from F6 |
| `F8_handoff_committed` | after `R12 -> R13` handoff with sealed History and successor invocation | successor startup/finalization tests | No if built from F7, but may spawn successor locally |

Regenerate live checkpoints intentionally when provider-facing producer behavior changes. Otherwise, use restored checkpoints for downstream storage and consumer tests.

## Test selection algorithm

When a storage change touches a writer:

1. Identify the persisted surface in the matrix.
2. Run the producer transition test in `fs` mode.
3. Run the producer transition test in the new configured mode (`fs` after abstraction or `dual-strict` after DB slice).
4. Run every downstream consumer listed for that surface from the nearest checkpoint.
5. Run authority-negative tests for the authority class:
   - DB row present but MessageBox file invalid must fail MessageBox consumers.
   - DB row present but channel envelope missing/invalid must fail channel consumers.
   - DB row present but sealed History invalid must fail successor startup.
   - DB row present but artifact checkout/root invalid must fail artifact consumers.
6. Run the full live loop only as a canary when producer/consumer contract coverage changes, when cross-chain behavior changes, or before declaring a migration slice complete.

## Live API minimization without lowering validity

Live API calls remain required for provider-facing producer transitions. The time savings come from not repeating those calls for downstream consumers.

Recommended tiers:

1. **Local compile/unit/contract tests:** no API, fastest, run constantly.
2. **Isolated transition tests from checkpoints:** no API unless the target transition itself calls providers.
3. **Live producer tests:** API-enabled tests only for transitions that produce provider-dependent evidence, e.g. broad planning or child treatment execution.
4. **Full live loop canary:** API-enabled end-to-end run, manual or pre-merge, not between every narrow test iteration.

Do not replace live producer tests with mocks in the confidence suite. Recorded/checkpointed outputs are for consumer coverage and time savings, not for pretending the provider-facing producer was exercised.

## Open items

- Generate and freeze the source-derived transition/outcome inventory, including the exact current count expected by the migration suite.
- For each transition outcome, assign one checkpoint fixture and one producer/consumer contract assertion set.
- Decide checkpoint serialization/restoration mechanics: copy campaign tree, git worktree refs, run artifacts, and any DB snapshots with hash verification.
- Decide which provider/model/profile is the default live confidence target.
- Define artifact retention policy for live checkpoints so failed storage migrations are reviewable without rerunning providers immediately.
