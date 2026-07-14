# Prototype 1 loop operator-control plan

- Date: 2026-07-13
- Baseline: `de76eaee34e6f4c4c3a19543c4cf91b217aa3bb9`
- Status: active implementation; Stages 0-1 complete, Stage 2 implementation
  verified and awaiting checkpoint/live canary

## Implementation progress

- Stage 0 configuration and readiness evidence is recorded through
  `df5d8ec79` (`docs: record Stage 0 operator findings`). Google eval and
  protocol routes passed live preflight. The explicit OpenRouter Perplexity
  embedding route remains externally blocked by the key-specific monthly
  limit, so no loop mutation was attempted.
- The canonical setup preview landed in `697c7468a` (`Add canonical Prototype 1
  setup preview`). It binds normalized campaign/profile payloads and their
  digests into one reviewed plan.
- The current Stage 1 slice adds receipt-first, resumable setup admission,
  exact completed-state verification, atomic authority-file replacement, and
  an exclusive Git-worktree setup lock. Its focused regressions pass, the full
  `ploke-eval` library suite passes 1007/1007 with 27 ignored tests, and the
  independent review reports no blocker. It is committed as `60c183df0` (`Add
  recoverable Prototype 1 setup admission`).
- Fresh live validation used campaign
  `p1-stage1-hardened-g35f-pplxembed-3g1x3-p3-20260713-1` in a clean worktree at
  that commit. Plan v2 SHA `233a3732be99bb7150560d4d678bf85c0ec1e40c4e0b86d89fd9cf789133f57d`
  admitted Parent(0) at `8c4fbae0e6b3b2b6ac07dd063f80f22a082f9903`; an immediate exact retry
  preserved all campaign file bytes, sizes, mtimes, Git HEAD, and clean status.
  Ordinary doctor, relative-root normalization, headless sparse/BM25 setup, and
  a live Direct Google protocol canary passed. The explicit OpenRouter
  Perplexity embedding preflight still returns the key-specific monthly-limit
  403 (`provider_account`) despite account credits, so the campaign remains
  unadvanced at `baseline_eval`.
- The Stage 2 implementation establishes the protocol-v5 durable controller
  session, cross-process file locking and monotonic fencing, exact attempt and
  cursor certificates, operation idempotency, fail-closed recovery/abandonment,
  generation-zero setup ownership, and recoverable predecessor/successor
  transfer. `prototype1-state`, `prototype1-step`, `prototype1-continue`, and
  walk mutations now pass explicit live-provider/Git capabilities through the
  same session boundary. Successor launch persists the exact predecessor
  attempt and cannot broaden those capabilities through argv or restart.
- Stage 2 includes an artifact-driven June 30 dual-driver regression using the
  real transition journal, child-plan bytes, and saved DB-query response. The
  final focused gate passes 13/13; `cargo test -p ploke-eval` passes 1,168 tests
  with 46 ignored/live tests; and independent authority review reports no
  actionable blocker. The required checkpoint commit and bounded live canary
  are the next gates; no live loop was advanced from the uncommitted tree.

Stage 3 and later remain planned, not implemented. CLI mutation adapters now
share the exclusive durable loop session, but the UI remains an inspection
client until the lossless sibling-client contract and later control-parity
stages are complete.

## Purpose

Turn the current Prototype 1 loop, `loop walk` tooling, and `ploke-walk-ui`
into one understandable operator system that can be inspected and controlled from
either the CLI or the native UI without creating competing loop authorities.

The desired experience includes:

- validate and explain a complete run setup before admitting it;
- start and drive a live run from either UI or CLI;
- see the current R0-R14 phase, the evidence that admits it, the blocker if it
  cannot advance, and the actions currently allowed;
- follow parent agent turns, tool calls, model exchanges, and `ploke-protocol`
  adjudications while they happen;
- hand control from a predecessor runtime to its successor without losing the
  operator session or starting two drivers;
- replay historical transitions without mutating the historical run;
- create an explicitly derived live branch from historical evidence;
- query persisted run data through a backend-neutral application boundary;
- preserve the existing correctness boundaries around History, identity,
  MessageBox/channel authority, worktrees, immutable evidence, and typestate.

This is not primarily a UI project. The UI is the clearest consumer of a larger
missing object: an authority-preserving loop execution session.

## Assumptions and decisions to confirm

This plan proceeds with the following recommendations:

1. The crate present at this commit, `ploke-walk-ui`, is the implementation
   vehicle. `ploke-loop-ui` is treated as the product concept, not a requested
   crate rename.
2. UI and CLI are sibling clients. Neither frontend executes R-state edges
   directly.
3. There is at most one admitted mutating controller for a loop session. Any
   number of clients may inspect it.
4. “Back” and “rewind” are read-only replay-cursor operations. Alternative
   execution from the past creates a new branch; it does not undo a live run.
5. “One source of truth” means one application-level authority and one coherent
   snapshot for clients. It does not yet mean that every authority-bearing fact
   must be forced into one physical database file.
6. Filesystem authority is migrated to database-backed adapters one domain at a
   time, with parity and negative tests. DB rows do not silently replace History,
   MessageBox/channel, invocation, artifact, or worktree authority.
7. Near-term default validation should prefer configuration/profile changes.
   If a current-code blocker is found, stop, preserve a reproducer, and plan the
   smallest correctness-preserving fix instead of weakening validation.

If these decisions are accepted, they remove several architectural forks from
the implementation sequence. In particular, destructive rollback and a generic
“put everything in the DB” rewrite are out of scope.

## What exists at the reset baseline

### Three different meanings of “advance”

The current code has three control models:

- `prototype1-state` is the canonical typed batch driver. It carries R0-R14
  values and calls the direct edges in `driver/advance.rs` and `live_edges.rs`.
- `loop walk` keeps one concrete R-state in a server-local `WalkController` and
  invokes those same direct edges incrementally.
- `prototype1-step` and `prototype1-continue` reconstruct a separate
  `DiagnosedPhase` and dispatch phase-specific operations. They do not carry the
  same R0-R14 value.

The first architectural goal is therefore not “add UI step buttons.” It is to
make every mutation pass through one admitted driver boundary with one meaning
of step.

### Useful capabilities already present

- Direct typed transitions cover the complete R0-R14 batch path.
- `WalkController::step_once` covers the principal phase branches and uses the
  direct edges.
- The walk protocol has health, start, step, stop, status, audit, files, LLM
  debugging, replay cursor, and branch-provenance operations.
- Mutations carry a server epoch, and source/binary drift fails closed.
- The walk server tracks outer jobs and suppresses duplicate outer start/step
  submissions.
- Reconstruction is side-effect free and intentionally strict.
- Replay back/forward already moves an immutable journal cursor without undoing
  effects.
- The eval store has normalized run-policy, parent-start, agent-turn,
  model-exchange, message-event, and tool-event relations.
- The UI already has run/socket selection, an online indicator, a current-phase
  rail, Health/Show, and read-only Cozo queries.

These are foundations to preserve, not replace.

### Current gaps that constrain the order of work

1. `WalkController::refresh_from_disk` does not refresh while it holds any
   in-memory state. External `prototype1-state`, `prototype1-step`, or
   `prototype1-continue` activity can therefore diverge from a running walk
   server.
2. The public `WalkClient` is intentionally read-only and flattens structured
   `Job` and `Status` responses into prose. The UI cannot render reliable job,
   blocker, transition, or cancellation state from that facade.
3. A fresh continuous walk has no admitted continuation after the R2a identity
   initialization branch.
4. Reconstruction omits several intermediate carriers, including R8, R9, and
   R11, and may rebuild early command data from defaults. It is strict recovery,
   not exact replay of every in-memory edge.
5. `branch-live` writes provenance only. It does not create a worktree, run
   identity, controller session, or live provider call.
6. R13b handoff launches a successor `prototype1-state` process rather than a
   successor walk controller. The predecessor server epoch is also invalidated
   by the checkout change.
7. A successor-ready timeout can still produce the live R13b type whose name
   claims a committed handoff, while reconstruction requires both durable
   handoff and ready evidence. Live and reconstructed semantics disagree.
8. A June 30 run demonstrated that an autonomous successor and a manually
   stepped walk server can both drive the same generation. The resulting
   child-plan file/DB collision was correctly rejected by `dual-strict`.
9. Stop aborts a running task, but effectful edge cancellation has no explicit
   transaction/recovery boundary.
10. The server accept loop is serial. Some long operations still hold the
    controller lock, and a response-write failure can terminate the serve loop.
11. The owner Cozo file is restored into an in-memory database and replaced
    under a process-local mutex. That is a useful query mirror, not yet a safe
    independent multi-process write authority.
12. Agent-turn database writes are primarily completed bundles. The detailed
    `walk llm` surface reads filesystem checkpoints and protocol artifacts, so
    live trace observability is not yet one coherent read model.

## The semantic object to build

The target is a durable loop execution session, not a larger walk DTO.

```text
                         read snapshots / submit commands
                 +---------------------------------------------+
                 |                                             |
          +------v------+                               +------v------+
          | native UI   |                               | CLI client  |
          +------+------+                               +------+------+
                 |                                             |
                 +--------------------+------------------------+
                                      |
                             typed control service
                                      |
                         +------------v-------------+
                         | loop execution session   |
                         | controller lease         |
                         | expected phase + epoch   |
                         +------+------------+------+
                                |            |
                       one typed edge     immutable receipts
                                |            |
                         +------v-----+  +---v----------------+
                         | R0-R14     |  | operator read model |
                         | driver     |  | DB/query projections|
                         +------+-----+  +---------------------+
                                |
                    typed authority/domain ports
                 History, identity, messages, channel,
                 artifacts, invocation, protocol, eval store
```

The exact Rust carriers should be chosen only after a type-reuse audit. The
semantic responsibilities are:

### Loop session

A loop session identifies one controllable lineage position and its lifecycle.
It relates:

- one campaign and current node/generation;
- zero or one active controller lease;
- one admitted configuration commitment;
- an ordered sequence of transition attempts and committed receipts;
- zero or more observer clients;
- zero or one predecessor session and zero or one successor session;
- zero or more replay cursors and derived branch origins.

The session must survive a UI restart, CLI disconnect, and walk-server restart.

### Controller lease

The controller lease grants exclusive mutation authority. It needs enough
durable identity to reject a second driver and diagnose a stale owner:

- session identity and campaign/node/generation coordinates;
- run mode: stepped or continuous;
- controller process/binary epoch;
- acquired/heartbeat/expiry or explicit terminal state;
- predecessor/successor link during handoff;
- takeover/recovery reason when ownership changes.

A lease is not permission to weaken the underlying edge checks. It prevents two
otherwise-valid processes from applying them concurrently.

The exclusion mechanism must itself be atomic across processes. The current
owner Cozo query mirror, protected only by a process-local mutex around backup
replacement, is not eligible to serve as the lease store. The first local
implementation should choose an actual OS-level ownership primitive held by one
authoritative control process, or use a store with proven transactional lease
semantics. It must issue a monotonic fencing token carried by every mutation so a
stale former owner cannot continue after takeover. Socket bind/takeover must
prove ownership and liveness before replacing an endpoint. Persisted lease
metadata is evidence about the owner; it is not by itself the exclusion
primitive.

### Transition command and receipt

Every mutation should carry:

- intended action;
- expected session, phase, controller epoch, and relevant evidence version;
- an idempotency key;
- explicit admission flags for high-risk edges;
- operator/client provenance.

Every attempt should return a typed receipt with:

- accepted, running, committed, blocked, failed, cancelled, or recovered state;
- phase before and after;
- transitions crossed;
- evidence written or consulted;
- blocker/error category and recovery advice;
- job identity and timing;
- controller and binary provenance.

The UI and CLI must render this same receipt. Neither should infer capabilities
by searching display text such as `--watch`.

### Loop snapshot

The primary read model should answer, without parsing prose:

- what session/run is selected;
- whether a controller is online and owns mutation authority;
- current reconstructed phase and confidence/admission evidence;
- current/last job and its progress;
- last committed transition and evidence delta;
- next allowed actions and their gates;
- active blocker and where to inspect it;
- effective configuration values and provenance;
- links to parent trace, tool calls, protocol reviews, child runs, selection,
  History, and handoff records.

Raw files, raw JSON, and raw CozoScript remain expert escape hatches, not the
primary operator story.

### Replay cursor and branch origin

Keep three operations separate:

1. **Historical replay** reconstructs and displays immutable evidence. It never
   calls providers or writes live state.
2. **Response fork** re-runs a recorded model/tool interaction with a new binary
   or implementation and stores new provenance without pretending the outer
   loop moved.
3. **Loop branch** creates a new campaign/worktree/session from an admitted
   historical checkpoint and records its source snapshot.

This separation prevents “branch-live” from becoming an ambiguous command that
mixes LLM experimentation, worktree derivation, and outer-loop mutation.

## Configuration model

“Defaults” must be split into three explicit layers:

| Layer | Meaning | Required behavior |
| --- | --- | --- |
| Schema defaults | Values Rust applies when fields are absent | Versioned, tested, and identical wherever the profile is parsed |
| Recommended setup | An explicit operator-facing template known to work for a stated environment | No hidden provider, embedding, concurrency, or storage choices |
| Effective setup | Fully resolved values admitted for one campaign | Includes provenance: default, template, campaign, CLI, environment, or registry resolution |

The effective setup is composed from more than `run-profile.toml`. Today,
embedding model/provider and eval budgets are setup inputs persisted in
`campaign.json`; they are not run-profile fields. The UI must not imply that the
profile alone is the complete configuration.

There is also a type-conformance problem to resolve before a profile editor is
safe: runtime `Prototype1RunProfile` owns validation, while
`ploke_records::run_profile::RunProfileRecord` is a passive mirror with different
field coverage/defaults. In particular, protocol defaults and model/tool-review
fields are not equivalent. A UI must not deserialize into the passive record and
then silently drop or rewrite runtime fields.

The first configuration implementation should therefore provide a shared typed
preview/admission operation that:

- parses through the canonical runtime owner;
- resolves setup-only fields and provider/embedding choices;
- shows the source of every effective value;
- shows validation constraints and explanatory copy for UI tooltips;
- returns an exact normalized profile and setup commitment preview;
- commits the previewed configuration identity atomically or idempotently where
  the current stores permit;
- reports each later setup side effect with staged receipts and an explicit
  recovery/abandon path rather than claiming one cross-filesystem/DB/Git
  transaction;
- compares the preview, admitted profile, campaign manifest, and commitment;
- refuses unapproved drift.

Do not rebuild a `Prototype1LoopCommand` from UI-local strings, JSON values, and
copied defaults. The discarded post-reset setup-form implementation is useful as
negative evidence for this rule.

## Controller modes and successor handoff

The existing stepped/continuous distinction should become an explicit ownership
contract.

### Stepped mode

- The session controller is the only driver.
- UI or CLI submits one transition command at a time.
- Long edges run as supervised jobs while reads remain available.
- At R13b, the successor starts in service-ready mode and acquires the successor
  session lease; it does not independently run to completion.
- The predecessor becomes read-only or exits after the successor is ready and
  ownership has been durably transferred.
- Clients resolve/reconnect to the successor endpoint from the session record.

### Continuous mode

- One controller owns autonomous advancement.
- UI and CLI observe, pause/cancel at admitted boundaries, or request an explicit
  takeover. They cannot manually step alongside the controller.
- Successor startup transfers the continuous lease; it does not create a second
  independent driver.

### Handoff lifecycle

R13b must expose durable sub-states rather than claiming “committed” too early.
The exact names should follow existing carriers, but the lifecycle must
distinguish at least:

1. selected successor and handoff admitted;
2. artifact installed and identity prepared;
3. predecessor/History retirement committed;
4. successor process started;
5. successor ready and lease acquired;
6. predecessor relinquished;
7. recovered or terminally blocked.

Each irreversible action needs an idempotent receipt and a documented recovery
path. The implementation does not need magical rollback, but it must never hide
which irreversible boundary was crossed.

## Dependency-ordered implementation roadmap

### Stage 0 — Establish a configuration-first live baseline

Goal: determine whether a current loop can pass setup/readiness and a bounded
walk using an explicit, evidence-backed setup before changing loop code.

Actions:

1. Use the June 30 admitted profile as the primary candidate template:
   `p1-gated-parent-3g1x3-p3-20260630-174316`.
2. Use the June 23 Perplexity-embedding run as the setup-only-field reference:
   `p1-dbdual-broad3g1x3-pplxembed-g25p-p25f-20260623-174553`.
3. Keep the June 15 filesystem-backed two-target run as a control.
4. Preserve the July 1 `p1-walkctx-...` embedding failure as a negative
   regression case.
5. Copy into a fresh disposable campaign; do not select a profile by mtime.
6. Make every model, route, embedding, storage, child count, parallelism,
   generation limit, timeout, and protocol setting explicit.
7. Diff source and candidate setup; reject every unexplained change.
8. Run setup, verify normalized profile/commitment/campaign values, then run
   ordinary doctor plus its current headless-TUI and live-protocol preflights.
   There is no standalone operator embedding execution preflight at this
   baseline. Verify the explicit embedding route and credential resolution, but
   record actual embedding execution as unproven unless a narrow preflight is
   approved or the first baseline attempt is deliberately used as the probe.
9. Advance only through a predeclared bounded transition and inspect after each
   step.

Success criteria:

- one fresh admitted campaign with exact config provenance;
- all currently preflightable provider dependencies proven before baseline
  work, with embedding execution explicitly marked proven or unproven;
- no unapproved profile drift;
- a bounded CLI-only walk reaches its declared stopping phase;
- any failure is classified as config/environment versus code contract, with a
  reproducer and no weakened invariant.

Stage 0 makes no source-code changes unless a code blocker is explicitly
approved for the next stage. Fresh disposable setup necessarily creates campaign,
closure, database, branch, identity, and commit artifacts; those effects must be
kept inside the new run and recorded.

### Stage 1 — Make configuration and admission one canonical contract

Goal: CLI and future UI preview and admit the same complete setup.

Deliverables:

- a type-reuse inventory of runtime profile, passive record, setup command,
  campaign manifest, setup report, and commitment carriers;
- conformance tests for field coverage, default equivalence, validation, and
  round-trip preservation;
- one effective-setup resolver with per-field provenance;
- one preview/admit service used first by the CLI;
- a narrow embedding execution/readiness preflight so a baseline run is not the
  first proof of its embedding route;
- structured explanations/constraints suitable for CLI help and UI tooltips;
- an explicit resolution for the documented full-batch parallelism drift:
  source currently derives `min(3, children.max)` while docs say
  `children.max`.

Acceptance:

- the same input produces the same normalized profile, campaign values, and
  commitment through every public setup path;
- missing/implicit embedding and provider readiness is visible before admit;
- no parser can successfully reserialize a profile while dropping runtime
  fields;
- preview has no side effects, configuration admission persists exactly the
  previewed values, and later setup stages expose receipts/recovery for partial
  filesystem, database, identity, branch, or commit effects.

### Stage 2 — Establish one durable controller/session authority

Goal: remove competing drivers before exposing mutations in the UI.

Deliverables:

- a historical regression reproducing the June 30 dual-driver collision through
  production replay/session code;
- a durable session identity and exclusive controller lease;
- an atomic cross-process ownership/fencing primitive plus a crash-recoverable
  attempt/receipt journal chosen explicitly; the current owner query mirror must
  not be used as the lease store;
- explicit stepped/continuous mode enforcement;
- expected-phase, epoch, evidence-version, and idempotency admission on mutation;
- one driver operation meaning “reconstruct, admit one typed edge, persist,
  produce receipt”;
- adapters so `prototype1-state`, `prototype1-step`, `prototype1-continue`, and
  walk mutation cannot bypass or disagree with the authority boundary;
- generation-zero identity bootstrap remains owned by `prototype1-setup`, whose
  completed checkout/identity authority creates the controller session at R3;
  walk and batch adapters reject R0/R1/R2a as mutable session targets while
  exposing setup/bootstrap evidence read-only for operator observability;
- cancellation/recovery semantics for effectful jobs;
- an R13b regression and corrected live/reconstruction meaning;
- atomic or recoverable predecessor-to-successor lease transfer.

Acceptance:

- two clients can inspect one session;
- two attempted drivers cannot mutate it concurrently;
- a repeated command is idempotent or rejected with a typed conflict;
- server restart reports the last committed boundary plus any explicit
  indeterminate/reconciliation state; it claims the same admitted phase only
  where sufficient durable carriers exist;
- a successor-ready timeout never reports a committed/ready state;
- old and new runtimes cannot both own generation mutation;
- checkout/binary epoch changes result in a clear reconnect/handoff receipt, not
  silent divergence.

This is the hard gate before UI Start/Step/Stop controls.

### Stage 3 — Publish a lossless sibling-client contract

Goal: make CLI and UI consumers of the same typed application service.

Deliverables:

- public typed request/response carriers for session snapshot, job state,
  transition receipt, blockers, allowed actions, and structured errors;
- operation-specific typed payloads for existing success responses that still
  use `Ok { message: String }`;
- a nonblocking snapshot path that does not wait behind a long effectful job;
- a backend-neutral, revision-tagged query/read endpoint so clients do not each
  restore an owner-DB snapshot at potentially different revisions;
- every effectful action, including nested live LLM work, supervised as a job;
- reliable server handling of client disconnects and response-write failures;
- client reconnection/session resolution across server and successor handoff;
- CLI rendering moved onto the public contract before UI mutation is added.

Acceptance:

- CLI and UI receive equivalent structured snapshots and receipts;
- no client parses human-readable messages to determine state or capability;
- session snapshots, evidence queries, and raw-query results identify the
  revision they observed;
- status/health remains responsive during live jobs;
- a timed-out/disconnected client cannot kill the server;
- public carriers preserve all `Job`, `Status`, `Audit`, and error structure.

Transport can remain framed JSON over a Unix socket for this stage. Streaming or
remote transport should be added only when the application contract is stable.

### Stage 4 — Build the operator observability read model

Goal: answer loop questions through typed summaries and evidence links while
preserving authority distinctions.

Deliverables:

- a canonical session snapshot and ordered transition-attempt/receipt view;
- explicit evidence references for phase admission and blockers;
- config provenance and derived-policy views;
- normalized parent/child/model/message/tool timelines using existing record
  types where possible;
- protocol adjudication/review records linked to the model/tool events they
  evaluate;
- incremental, ordered telemetry for live agent turns where post-attempt bundles
  are insufficient;
- a typed “what changed in this transition?” view. Do not assume existing
  `eval_transition_event` or `eval_walk_event_transition` is a generic relation
  delta ledger;
- raw immutable query retained as an expert surface.

Acceptance:

- for every phase, an operator can identify current state, admitting evidence,
  prior transition, next allowed action, and blocker;
- parent evaluation traces expose prompts, exchanges, tool calls/results, and
  linked protocol reviews without filesystem archaeology;
- incomplete/in-flight work is distinguishable from missing persistence;
- projections identify their source authority and integrity/provenance fields;
- the existing 57-question DB/filesystem parity worksheet is used as the
  answerability test rather than replaced by a new ad hoc checklist.

### Stage 5 — Make the UI excellent at inspection

Goal: provide an intuitive native operator view before adding full control.

Deliverables:

- persistent online/controller/job indicator;
- phase rail showing completed, active, blocked, and future phases;
- current-state card with evidence, last transition, blocker, and next actions;
- setup/effective-config inspector with shared explanatory tooltips;
- expandable parent/child agent trace, tool-call details, and protocol reviews;
- transition evidence/delta and provenance navigation;
- clear links or copyable coordinates for files, records, nodes, and reviews;
- raw response and raw query panels as secondary tools.

Acceptance uses three access tiers:

- **Full access:** source/files/CLI/DB can validate the UI claim.
- **DB/service only:** normalized/queryable evidence can answer the claim without
  browsing arbitrary files.
- **UI only:** a person can answer the question through actual UI interaction.

UI-only validation should use `egui_kittest`, real navigation, and screenshots.
The target is not merely that data exists; it is that the UI explains it.

### Stage 6 — Add UI/CLI control parity

Goal: complete a parent turn through UI only, CLI only, or mixed sequential
clients over one controller.

Deliverables:

- setup preview/admit, start, step, pause/stop, resume/recover, and explicit
  high-risk confirmations through the shared service;
- job progress and cancellation/recovery presentation;
- allowed actions derived from typed capabilities, not duplicated frontend
  policy;
- UI controls disabled with an explanatory blocker when another continuous
  controller owns the session;
- equivalent CLI JSON and human-readable receipts.

Acceptance:

- a fresh run can be set up and driven through one full parent turn CLI-only;
- the same can be done UI-only;
- UI and CLI may alternate commands without state divergence;
- a second simultaneous mutation is attached, rejected, or idempotently
  coalesced—it never launches another driver;
- all correctness failures remain visible and fail closed.

### Stage 7 — Implement exact replay and derived live branches

Goal: inspect history and test fixes without corrupting or pretending to rewind
the source run.

Deliverables:

- exact historical snapshots/receipts through production reconstruction and
  replay code;
- typed back/forward/jump cursor operations with no live side effects;
- response-fork operation for a recorded LLM/tool boundary;
- loop-branch operation creating a new campaign, worktree, session, and explicit
  source provenance;
- historical replay tests built from real persisted incident artifacts;
- comparison UI/CLI views between original and derived results.

Acceptance:

- replay performs no provider calls, worktree writes, History append, or live
  mutation;
- a live branch has a new identity and cannot write into the source run;
- source cursor, artifact hashes, code/binary version, config, and reason are
  durable;
- a historical bug can be replayed, a fixed binary can create a derived result,
  and both outcomes remain comparable.

### Stage 8 — Migrate authority to database-backed adapters by domain

Goal: support selectable database backends without erasing domain semantics.

Approach:

1. Keep the operator read model/query store backend-neutral.
2. Define backend capabilities and concurrency/transaction behavior explicitly.
3. Migrate one existing authority port at a time, reusing its domain types:
   transition receipts/journal, protocol/trace artifacts, MessageBox/channel,
   History, invocation/bootstrap, and other stores only after their contracts are
   inventoried.
4. For each surface, provide import/migration, dual-write parity, restart,
   corruption, and negative tests before claiming DB authority.
5. Keep filesystem implementations as adapters where useful; do not make schema
   drift permissive.

Acceptance:

- clients do not depend on a specific database backend;
- each migrated domain states which store is authority and which is projection;
- concurrent UI/CLI reads and controller writes have tested isolation semantics;
- a two-generation `dual-strict` canary proves parity before DB-only claims;
- “DB-only” is claimed surface by surface, never because one owner file exists.

## Verification ladder

Implementation should advance through the cheapest proof that can falsify each
slice. All test commands should be run by a test sub-agent and summarized back to
the primary implementation thread.

1. Static/type tests: profile field/default conformance, serde round-trip,
   capability vocabulary, session invariants.
2. Contract tests: preview/admit identity, command/receipt idempotency, two-client
   conflicts, typed snapshot equivalence.
3. Historical replay regressions: June 30 dual-driver collision, July 1 embedding
   fallback failure, R13b timeout semantics, known server/LLM incidents.
4. Temp-socket integration: disconnected clients, concurrent status, job
   cancellation, server restart, stale epoch, successor endpoint resolution.
5. Fake/local provider tests: setup readiness and handoff without live cost.
6. Bounded live gates: R0-R5, R5-R7, R7-R8, R10-R12, then R13b separately.
7. One full parent-turn canary in stepped mode.
8. Two-generation handoff canary in continuous mode.
9. UI access-tier tests with `egui_kittest`, screenshots, and interaction.
10. Backend parity and DB-only tests only after the relevant domain adapter exists.

Every stage should have a committed checkpoint before live advancement. A live
failure should result in a durable bug/reproducer and a fresh run after repair;
do not mutate a failed campaign into appearing valid.

## Immediate work packet

The next implementation effort should be deliberately narrower than this full
roadmap:

1. Produce a read-only effective-config comparison for the four historical
   candidates listed in Stage 0.
2. Choose and copy one explicit candidate setup into a fresh disposable campaign.
3. Validate profile normalization, commitment, campaign/setup-only fields,
   derived parallelism, and provider/embedding readiness.
4. Run doctor and bounded preflights without advancing the loop.
5. If green, start one walk server and advance only to an agreed early boundary,
   using inspect-after-each-step discipline.
6. Classify any blocker. Prefer a config/profile correction; if it is a code
   contract, stop and create a reproducing regression/repair slice.
7. Do not add UI mutation controls during this packet.

Expected output:

- selected recommended explicit setup and why;
- exact effective values and provenance;
- readiness/preflight evidence;
- bounded phase ledger;
- blocker report or clean handoff into Stage 1.

## Existing documents to preserve and reuse

- [`2026-06-17_typestate-loop-driver-plan.md`](2026-06-17_typestate-loop-driver-plan.md):
  phase/evidence map and the rule that walk is an operator interface, not
  authority.
- [`2026-06-22_prototype1-eval-store-data-model/`](2026-06-22_prototype1-eval-store-data-model/README.md):
  domain-aware persistence model and authority/projection distinctions.
- [`2026-06-25_cleanup-before-db-parity-decision.md`](2026-06-22_prototype1-eval-store-data-model/2026-06-25_cleanup-before-db-parity-decision.md):
  current DB-read and domain-port gaps.
- [`2026-06-25_remote-safe-db-persistence-implementation-plan.md`](2026-06-22_prototype1-eval-store-data-model/2026-06-25_remote-safe-db-persistence-implementation-plan.md):
  owner-scoped DB and remote-safe import direction.
- [`2026-06-30_p1-db-filesystem-parity-live-run-template/`](2026-06-30_p1-db-filesystem-parity-live-run-template/README.md):
  57-question answerability matrix and live step ledger.
- [`2026-06-30_p1-gated-live-run-handoff.md`](2026-06-30_p1-gated-live-run-handoff.md):
  dual-driver/handoff incident evidence.

This document integrates those lanes. It should not replace their detailed
inventories or rewrite their historical findings.

## Explicit non-goals

- No broad reorganization of `ploke-eval` before the controller and data
  contracts are proven.
- No UI-local copy of run defaults or transition policy.
- No second executor hidden inside the UI.
- No destructive rewind of live evidence.
- No generic storage trait that erases History, MessageBox, channel, artifact, or
  protocol semantics.
- No permissive loading to disguise schema/config drift.
- No claim that the current DB mirror is already the shared runtime authority.
- No user-facing polish that masks missing typed blocker/evidence data.

## Definition of the desired end state

The work is complete only when:

- one durable session and controller can drive the direct typed loop edges;
- UI and CLI show the same current state and submit the same typed commands;
- configuration is explicit, validated, explained, and provenance-bearing;
- predecessor/successor handoff transfers one controller authority safely;
- phase evidence, traces, tools, protocol reviews, and blockers are navigable;
- replay is exact and read-only, while branching creates a new explicit lineage;
- database backends can be selected behind domain-preserving adapters;
- CLI-only, UI-only, mixed-client, restart, handoff, and historical-fix workflows
  are all proven without weakening correctness guarantees.
