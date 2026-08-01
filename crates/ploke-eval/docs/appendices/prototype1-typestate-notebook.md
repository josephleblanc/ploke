# Prototype 1 Typestate Notebook

This is the long-form working notebook behind the shorter
[Typestate](../prototype1/typestate.md) overview. It is retained for source
provenance, detailed reasoning, and historical context.

Status: working draft, discovery in progress. Last updated in-place from source/doc reads.

Companion synthesis: [`typestate-invariants.md`](../prototype1/typestate-invariants.md)
collects the current structured explanation of intent, core model, proof layers,
invariants, and known gaps. This file remains the long-form working notebook.

This document is a durable map of the `ploke-eval` Prototype 1 runtime
succession loop and its structural typestate model. It is intentionally written
as an evolving notebook first: claims below should either cite source locations
or be marked as hypotheses until checked against code.

## Purpose of this document

The Prototype 1 loop is complex enough that operator docs alone are not enough.
This page aims to explain:

- the intent behind the runtime succession loop;
- the core type and data model used by the typestate implementation;
- the invariants the loop is trying to enforce;
- where correctness is compile-time, where it is durable/runtime checked, and
  where gaps may remain;
- how the docs in evalnomicon relate to the code in `ploke-eval`.

## Source map

Primary code paths discovered so far:

- `src/cli/prototype1_state/mod.rs` — conceptual module-level model for the
  Prototype 1 state system.
- `src/cli/prototype1_state/typestate/` — structural runtime typestate axes,
  aliases, context bundles, shapes, and transition combinators.
- `src/cli/prototype1_state/live_edges.rs` — canonical direct edge functions
  used by both the batch driver and `loop walk`.
- `src/cli/prototype1_state/driver/advance.rs` — canonical live typed driver
  for one parent turn.
- `src/cli/prototype1_state/driver/reconstruct.rs` — durable reconstruction for
  the `loop walk` debugger.
- `src/cli/prototype1_state/driver/replay.rs` — historical replay cursor.
- `src/cli/prototype1_state/walk/` — operator/debugger phase cursor over the
  same typed edge functions, including explicit gates for effectful edges.
- `src/cli/prototype1_state/run/core.rs` — older operator-facing diagnosis and
  phase advancement surface for `doctor`, `step`, and `continue`.
- `src/cli/prototype1_process.rs` — process seam for child/successor execution,
  successor installation, History sealing/appending, spawn, and ready wait.
- `src/cli/prototype1_state/{c1,c2,c3,c4}.rs` — child-attempt C1-C5 typestate
  scaffold used to model materialize/build/spawn/observe seams.
- `src/cli/prototype1_state/history/` — History block model, projections, and
  seal logic.
- `src/cli/prototype1_state/journal.rs` — transition journal schema and replay
  projections.
- `src/cli/prototype1_state/profile.rs` — admitted run-profile schema and
  validation.

Primary docs discovered so far:

- `crates/ploke-eval/docs/prototype1/operator-map.md` — current canonical
  operator map.
- `crates/ploke-eval/docs/development/source-map.md` — compact
  source anchor companion for the current documentation pass.
- `crates/ploke-eval/docs/prototype1/loop-walk.md` — `loop walk` debugger guide.
- `docs/workflow/evalnomicon/src/prototype1/runtime-loop.md` — succinct
  evalnomicon description of the runtime succession loop.
- `docs/workflow/evalnomicon/src/prototype1/artifact-runtime-model.md` —
  artifact/runtime/tree/lineage separation.
- `docs/workflow/evalnomicon/src/prototype1/runtime-authority.md` —
  role/state-bounded authority model.
- `docs/workflow/evalnomicon/src/prototype1/history-crown.md` and
  `invariant-ledger.md` — History/Crown claim boundaries and status labels.
- `docs/workflow/evalnomicon/src/prototype1/selection-and-evaluation.md` —
  parent-side selection boundary and anti-self-promotion claim.
- `docs/workflow/evalnomicon/src/prototype1/persistence-and-observability.md` —
  evidence family/projection warnings.
- `crates/ploke-eval/src/cli/prototype1_state/typestate/IMPLEMENTATION_PLAN.md`
  — historical implementation plan, still useful for intent and invariants.

Discovery provenance:

- Current source-verified pass read evalnomicon Prototype 1 pages plus the live
  typestate driver, edge functions, parent/inner/History carriers, successor
  handoff path, reconstruction path, walk controller, and child C1-C5 scaffold.
- Earlier graph/intelligence notes should be treated as secondary unless they
  are checked against the source paths above.

## Intent: runtime succession, not flat rerun

The evalnomicon frames Prototype 1 as a runtime succession loop:

```text
active parent runtime
  -> synthesize descendant candidates
  -> realize candidate artifact state
  -> build descendant binary
  -> spawn descendant child runtime
  -> child evaluates itself and records evidence
  -> parent observes child evidence
  -> parent applies selection policy
  -> selected successor artifact is installed into the active checkout
  -> successor runtime validates handoff evidence
  -> successor becomes the next parent if admission succeeds
  -> old parent exits
```

The central distinction is between two axes that are easy to conflate:

- **Artifact state:** files, worktrees, checkouts, branch surfaces, committed
  parent identity, selected descendant code.
- **Runtime state:** an executing binary with a role, phase, and authority to
  perform only certain transitions.

A parent can create and inspect descendant artifacts, but it cannot fully
validate the semantics of a descendant as if it were already running that code.
For that, the loop builds and spawns child/successor runtimes. The typestate
system is an attempt to encode the legal order of this succession pipeline.

## Conceptual data model

The module-level comments in `prototype1_state/mod.rs` are currently one of the
highest-priority sources for intent. The core model is not just a parent process
and a branch name; it is a graph of artifacts, runtimes, operations, records,
and lineage authority.

### Artifact

An artifact is a checkout state: the collection of files that can hydrate a
runtime. A path or branch is only a handle to an artifact. A dirty worktree is a
provisional artifact candidate until it has recoverable identity such as a git
commit, tree id, content hash, or artifact manifest id.

### Runtime

A runtime is an executing process hydrated from an artifact. A runtime may
operate over an artifact surface to produce a patch attempt. The artifact it
operates on need not be the same artifact that hydrated it, so records need more
than git ancestry.

The important operation coordinate is:

```text
OperationCoordinate = (generator Runtime, target Artifact)
```

Prototype 1 therefore needs at least three graph views:

- artifact graph: durable artifacts connected by applied patches;
- runtime derivation graph: which artifact hydrated each runtime;
- operation graph: which runtime operated on which artifact to produce a patch
  attempt.

### History

History is the durable authority substrate over sealed lineage-local blocks. It
is not the scheduler snapshot, branch registry, CLI report, or debug projection.
A History block is one Crown epoch for a lineage: entries are written while a
runtime has parent authority, sealed when succession is locked, and verified by
the successor before next-parent admission.

### Crown

The Crown is lineage-local mutation authority. It is not a pid, branch, path, or
global singleton. The local invariant is at most one valid `Crown<Ruling>` per
lineage. During handoff there may be zero rulers: the predecessor has locked and
retired, while the successor has not yet validated into the next parent.

### Box / message

A cross-runtime message is a typed obligation, not just a JSON file. The local
formula from `mod.rs` is:

```text
Box = (Lock transition, Unlock transition, File schema)
```

The current concrete example is the child-plan box at
`prototype1/messages/child-plan/<parent-node-id>.json`: the parent publishes a
specific candidate set, and the receiver checks the concrete box, parent
identity, and child generation before selecting from it.

## Core type: `Runtime<...>` as a product of evidence axes

The current structural type is centered on
`typestate::runtime::Runtime`:

```rust
Runtime<Phase, Role, Context, Plan, Children, History, Evidence, Continuation, Report>
```

Source-backed axis interpretation:

- `Phase` names the coarse R-stage in the outer parent-turn pipeline; markers
  live in `typestate/axes/phase.rs`.
- `Role` starts as `RuntimeRole<role::Unknown, role::Unresolved>`, then uses
  existing role carriers directly when available, especially `Parent<S>` from
  `parent.rs`.
- `Context` carries command-derived values in `Context<context::Collected<...>>`.
  These are concrete runtime values, not additional type axes.
- `Plan<Authority, Schedule>` separates child-plan authority from child schedule
  readiness.
- `Children<Set, Attempt>` separates parent-level child-set facts from the
  per-child C1-C5 attempt chain.
- `History<Startup, Head, Epoch>` separates startup/admission, lineage-head
  observation/advancement, and in-flight handoff epoch facts.
- `Evidence<ParentStart, Baseline, Policy, Selection, Completion>` carries facts
  that are prerequisites but are not yet first-class role/history/child carriers.
- `Continuation<Selection, Decision, Handoff>` keeps selected candidate,
  continuation decision, and handoff record distinct.
- `Report<State>` separates report facts from final report emission.

This is a product type: each state is not represented by one giant enum variant,
but by a fixed shape whose type parameters encode which facts are known.
Aliases such as `R0`, `R8`, or `R14bFinalHandoff` name useful points in that
product space.

## Strong typestate islands vs marker axes

The outer `Runtime<...>` aliases are partly marker-driven, but they embed and
reuse stronger move-only carriers where those exist:

- `Parent<S>` has private fields and state-specific methods. For example,
  `Parent<Unchecked>::check` validates checkout identity before producing
  `Parent<Checked>`, and `Parent<Checked>::ready` consumes `Startup<Validated>`
  before producing `Parent<Ready>`.
- `Startup<Validated>` cannot be made from invocation JSON alone. Genesis
  startup requires an absent local History head and generation 0. Predecessor
  startup requires a sealed History head, matching selected parent identity,
  matching clean artifact tree, and matching surface commitment.
- `Crown<crown::Ruling>` has a private constructor. The public transition is
  through `Parent<Selectable>::seal_block_with_artifact`, which consumes the
  selectable parent, admits artifact claims, locks the Crown, seals a block, and
  returns `Parent<Retired>`.
- `Open<M>`, `Locked<M>`, and `Received<M>` model cross-runtime messages as
  move-only obligations. `Open<M>` is `must_use` and panics on drop if it was not
  locked or failed; `Received<M>` is the capability proving the intended receiver
  consumed the message.

This distinction matters for correctness review. A phase alias that carries
`Parent<Ready>` is stronger than a phase alias that only flips a PhantomData
marker, because the parent carrier itself can only be reached through its module's
transition methods.

## Transition model

The historical implementation plan describes the target and current shape as a
pipeline of typed arrows:

```text
Transition<From, To, F>
From -> Result<To, Error>
```

Key intended invariant:

```text
If a function receives R9, the compiler knows every required R0-R8 fact exists.
```

The transition itself is not the authority source. Authority remains in the
consumed state value and in the durable evidence it refers to. Applying a
transition consumes the old runtime state and returns the next state.

This means correctness is layered:

1. **Type-level adjacency:** code cannot compose non-adjacent structural states
   without an explicit bridge.
2. **Runtime validation:** each transition must still check files, process
   status, hashes, provider outputs, History blocks, and profile constraints.
3. **Durable authority:** History blocks, transition journal entries, child-plan
   files, invocation records, and admitted profiles are the cross-process facts
   that make a restarted runtime reconstructable.

## First invariant ledger

These are initial invariants inferred from docs and will be checked against
source as this document evolves.

### Authority boundaries

- History blocks under `history/blocks/*`, appended through the block store, are
  the sealed handoff authority boundary.
- The transition journal is append-only replay evidence, but not the sole
  authority for handoff validity.
- Scheduler/node JSON, branch registry records, CLI tables, and debugger views
  are projections unless a specific transition treats them as admitted input.
- The admitted `run-profile.toml` and its commitment constrain policy knobs for
  generation, selection, execution, and control. It is not only operator
  convenience: `profile::validate` rejects unsupported schema versions, invalid
  child budgets, zero observe/harness timeouts, control parallel caps that widen
  admitted fanout, direct-Google/OpenRouter provider-shape mismatches, and
  oracle/evaluation combinations that require missing MBE evidence.

### Role boundaries

- Parent runtimes may synthesize, materialize, build, spawn, observe, select,
  seal, and hand off within their bounded authority.
- Child runtimes may evaluate assigned candidate artifacts and emit child-shaped
  evidence, but must not promote themselves.
- Successor runtimes become parents only after validating predecessor handoff
  material and acknowledging readiness/admission.

### Selection and evaluation boundaries

- LLM/protocol adjudication answers what to try; mechanized treatment evidence
  answers whether a branch improved under the current eval policy; successor
  selection decides which coordinate, if any, may receive continuation
  authority.
- Child self-report is not promotion. The parent-side `SuccessorDecision`,
  continuation policy, History/Crown handoff, and successor startup validation
  are separate gates.
- Branch-evaluation disposition and successor selection are related but not
  identical. Code comments in `successor_selection::decision` explicitly allow
  traversal policies such as `explore_from_rejected`, where a rejected branch
  can still become the next exploration coordinate. Therefore a selected
  successor must be documented as selected under traversal/continuation policy,
  not automatically as a kept/successful child.
- `selection::Selection<Artifact>` revalidates selected node/resolved-branch
  consistency before handoff material is built from it. That is a runtime guard,
  not a complete proof of evaluation quality.

### Artifact/runtime separation

- Temporary child worktrees are evaluation surfaces, not the next long-lived
  parent checkout.
- The stable active parent checkout is advanced to the selected artifact before
  successor spawn. History handoff can be committed even if successor ready later
  times out, so checkout advancement, History commit, ready acknowledgement, and
  next-parent admission are distinct facts.
- A binary from one checkout should not be used as the authority-bearing runtime
  for another checkout's active parent.

### Sequence boundaries

- Child materialization precedes child build.
- Child build precedes child spawn.
- Child terminal observation precedes successor selection.
- Successor selection precedes History handoff sealing.
- History handoff sealing/appending precedes successor acknowledgement.
- Parent retirement and successor parent admission are distinct facts.

## Early correctness questions / possible gaps to investigate

These are not conclusions yet; they are the current review queue.

1. **Alias strength:** Do the `R*` aliases always carry the precise durable facts
   implied by their names, or do some aliases still carry migration-era context
   bundles that require manual discipline?
2. **Reconstruction completeness:** Does `driver::reconstruct` reject every
   missing/mismatched authority fact, or can it reconstruct states from
   projections that are weaker than live transition requirements?
3. **History authority vs. journal authority:** Are all handoff-critical facts
   sealed into History before successor admission, or are some still only in the
   transition journal or process-local context?
4. **Successor binary provenance:** Is the code preventing a successor spawned
   from the wrong binary/checkout, or is that partly an operator rule?
5. **Branch explicitness:** Are stop-vs-handoff and rejected-only-vs-successful
   child fanout represented as branch types at all authority boundaries, or do
   some paths still encode them as optional fields?
6. **Runtime effects:** Which transitions perform external effects, and are
   those effects gated by typed states plus explicit operator flags in `walk`?

## Status language for claims

Evalnomicon asks Prototype 1 docs to distinguish claim strength. This document
will use the same labels:

- **Implemented:** enforced by current code on the relevant path.
- **Partially implemented:** current code enforces part of the claim but has
  named gaps.
- **Intended:** target architecture, not fully enforced.
- **Not claimed:** a stronger interpretation the project does not rely on.

## Current R-phase map

The live batch path is `driver::advance::run_to_terminal`, which constructs
`R0` and advances through direct edge functions in `live_edges.rs`. The same
edges are shared by the `loop walk` debug surface.

| Phase | Main fact added | Current transition edge |
| --- | --- | --- |
| `R0` | Raw `Prototype1StateCommand`; role unresolved. | `R0::new` |
| `R1` | Repo root, campaign id, manifest path, run shape, campaign config, journal. | `r0_to_r1` |
| `R2a` | Terminal parent-identity initialization branch. | `r1_to_r2a_or_r3` |
| `R3` | Existing parent identity resolved from active checkout or successor invocation. | `r1_to_r2a_or_r3` |
| `R4a` | Concrete `Parent<Unchecked>` loaded. | `r3_to_r4a` |
| `R4b` | Genesis parent checked against active checkout. | `r4a_to_r4b_or_r4c` |
| `R4c` | Unified `Parent<Ready>` after genesis or predecessor startup validation. | `r4b_to_r4c_genesis` or predecessor branch in `r4a_to_r4b_or_r4c` |
| `R5` | Parent-start journal entry and resource sample recorded. | `r4c_to_r5` |
| `R6` | Complete parent baseline established or loaded. | `r5_to_r6` |
| `R7` | Parent policy and child-planning budget resolved. | `r6_to_r7` |
| `R8` | Child-plan authority received; parent becomes `Parent<Selectable>`. | `r7_to_r8` |
| `R9` | Planned children shaped into a budgeted schedule. | `r8_to_r9` |
| `R10` | Active successor-selection strategy ready. | `r9_to_r10` |
| `R11a` | Rejected-surface-attempts-only branch. | `r10_to_r11` |
| `R11` | Child fanout complete and possible successor selection carried. | `r10_to_r11` |
| `R12` | Child/outcome report facts projected. | `r11_to_r12` |
| `R13a` | No successor handoff; continuation stopped. | `r12_to_r13` |
| `R13b` | Selected successor handoff committed; parent is `Parent<Retired>`. | `r12_to_r13` |
| `R14a` | Final stopped report emitted and completion recorded. | `r13_to_r14` |
| `R14b` | Final handoff report emitted and completion recorded. | `r13_to_r14` |

Four branch types make important forks explicit:

- `R1Branch` distinguishes identity initialization from a normal parent turn.
- `R10FanoutBranch` distinguishes rejected-only child planning from completed
  child fanout.
- `R12ContinuationBranch` distinguishes stopped continuation from committed
  successor handoff.
- `R14FinalBranch` distinguishes final stopped report from final handoff report.

## Effectful edge inventory

The R-phase map is easiest to audit when each edge is classified by the kind of
world effect it is allowed to perform.

| Edge | Main effects | Current guard / authority shape |
| --- | --- | --- |
| `R0 -> R1` | Resolve repo/campaign/run context, ensure baseline closure state, open transition journal handle. | No parent authority yet; only command-derived context. |
| `R1 -> R2a` | Initialize parent identity and commit/write active checkout identity. | Terminal setup branch, not a normal parent turn. Future docs should avoid implying this already creates full genesis History authority. |
| `R1 -> R3` | Resolve existing parent identity from active checkout or successor invocation. | Rejects child invocations when successor handoff invocation is required; successor path validates sealed continuation first. |
| `R3 -> R4a` | Load `Parent<Unchecked>`, preserving successor runtime id when the run starts from handoff invocation. | Checks invocation campaign and node identity against resolved parent identity. |
| `R4a -> R4b/R4c` | Genesis validates active checkout; predecessor validates active root, sealed successor identity, current tree, and surface before ready. | Strong `Parent`/`Startup<Validated>` carrier boundary. |
| `R4c -> R5` | Append parent-start journal entry and resource sample. | Requires `Parent<Ready>`. Journal is evidence/projection, not History authority. |
| `R5 -> R6` | Establish or load complete parent baseline. | Async runtime work; baseline carried as value-level fact. |
| `R6 -> R7` | Resolve complete search policy and child-planning budget. | Admitted run profile/campaign policy constrains the values. |
| `R7 -> R8` | Resolve, publish, load, or receive child-plan authority; may run provider/headless-harness work. | Produces `Parent<Selectable>` plus `Received<ChildPlan>` evidence; `walk` requires `--watch`. |
| `R8 -> R10` | Shape child schedule/budget and selection strategy. | Pure-ish projection over collected plan/policy facts; still checks required facts at runtime. |
| `R10 -> R11/R11a` | Run child fanout or rejected-only projection and produce selection evidence. | Requires baseline, budget, schedule, strategy, and child-plan facts; `walk` requires `--watch`. |
| `R11 -> R12` | Project child/rejected outcomes into report facts. | Does not decide/commit continuation by itself. |
| `R12 -> R13a` | Record stopped/no-successor continuation. | Explicit stopped branch; keeps `Parent<Selectable>`. |
| `R12 -> R13b` | Install selected artifact into active checkout, build successor, seal and append History, retire parent, write invocation, spawn successor, wait for ready/timeout. | Highest-risk edge: consumes `Parent<Selectable>` into `Parent<Retired>`, routes through `seal_block_with_artifact` and `FsBlockStore::append`; `walk` requires `--watch --allow git-changes`. |
| `R13 -> R14` | Emit final report and record successor completion if this runtime was itself a successor. | Completion/report projection; not a successor admission proof. |

## Compile-time vs runtime vs durable correctness

### Compile-time adjacency: implemented

`Transition`, `Step`, `AsyncStep`, `Chain`, `StepInput`, and `AsyncStepInput`
make typed edge composition ergonomic while preserving adjacency. For example,
`r8.advance(r8_to_r9.then(r9_to_r10))` composes because the first edge outputs
`R9` and the second consumes `R9`.

The important limit: Rust only proves the values have the requested alias type.
It does not independently verify that each constructor performed every semantic
check implied by the alias name. That remains transition code responsibility.

### Runtime transition checks: implemented / partially implemented

The live edges perform concrete checks and effects, for example:

- `r1_to_r2a_or_r3` rejects a child invocation when a successor handoff
  invocation is required.
- `r3_to_r4a` checks successor invocation campaign and node identity against the
  resolved parent identity.
- `r4a_to_r4b_or_r4c` checks active parent root path, validates sealed successor
  continuation, and records successor readiness before predecessor startup enters
  `Parent<Ready>`.
- `r7_to_r8` resolves or publishes the child plan before `Parent<Selectable>`.
- `r10_to_r11` requires baseline, budget, schedule, selection strategy, and
  child-plan facts before fanout.
- `r12_to_r13` requires selection material before handoff and records successor
  selection/stopped decisions in the journal.

The named gap is that many `from_collected_parent` constructors set type-axis
markers with `PhantomData`; they rely on only live edge functions calling them
after the checks. This is still useful internal typestate, but not a proof that
arbitrary code inside the module cannot forge a stronger alias.

### Durable reconstruction: partially implemented

The loop has several durable substrates:

- admitted run profile and commitment;
- active checkout parent identity;
- child-plan message boxes;
- transition journal;
- child/successor invocation, channel, ready, result, completion, and stream
  files;
- sealed History blocks and rebuildable indexes.

The intended authority hierarchy is that sealed History is the handoff authority
surface, while scheduler/branch/CLI/debugger records are projections or evidence.
This is only partially consolidated: the live loop still uses a mixture of
History, journal, node records, branch/evaluation artifacts, and process-local
`context::Facts`.

### Sealed History rigor: implemented locally

The History store is one of the most rigorous parts of the current model:

- `BlockStore::append` is documented as the only semantic operation that may
  advance the lineage head.
- `FsBlockStore::append` verifies the sealed block hash, reads the current
  lineage state, rejects stale expected heads, checks `expected.verify_append`,
  appends to the sealed block stream, and only then updates rebuildable indexes
  and `heads.json`.
- `Block<Sealed>::verify_hash` recomputes the entries root and block hash from
  the sealed preimage, including selected successor, selected parent identity,
  active artifact, claims, and seal timestamp.
- `Block<Sealed>::verify_current_artifact_tree` and `verify_current_surface`
  recompute successor-side artifact/surface facts instead of trusting invocation
  JSON.
- `validate_prototype1_successor_continuation` requires a sealed History head,
  checks that the sealed selected successor runtime matches the invocation
  runtime, checks selected artifact equals active artifact, and validates the
  sealed selected parent identity against the invocation campaign/node.

Current boundary: this is a local, transition-checked, tamper-evident History
model. It is not distributed consensus, global process uniqueness, or proof that
LLM/model judgment was correct.

## Known caveats discovered so far

### Startup path is intentionally weaker than final History model

Status: **partially implemented / intended stronger model**.

`typestate/mod.rs` documents a caveat: the intended History path is closer to
`Startup<Observed> -> Startup<Genesis | Predecessor> -> Startup<Validated> ->
Parent<Ruling>`, while the current live path advances through `Parent<Ready>`
and later `Parent<Selectable>`. The `History<Startup, Head, Epoch>` axis keeps
that stronger model visible without claiming it is fully enforced today.

### R-state aliases still carry migration-era value bundles

Status: **partially implemented**.

`context::Collected` carries many `Option` facts (`parent_baseline`, policy,
child plan, selection strategy, outcomes, report facts, etc.). The type aliases
mark when those facts should exist, while the live edges still check the
corresponding `Option`s at runtime. This is a transitional design: it gives
compile-time phase adjacency but still needs runtime missing-fact errors inside
later transitions.

### Rejected-only and stop/handoff branches are explicit, but not all semantics are type-level

Status: **partially implemented**.

The major control forks have sum types, which is good. However, the payloads
inside those branches still rely on `context::Facts` for details such as
selection material, rejected attempt counts, and report facts. The branch type
proves which path was taken, not every detail of the evidence carried by that
path.

### Timeout and replay hardening items

Status: **partially implemented with remaining gap**.

Current `c3` child-spawn code records `ReadyTimedOut` and
`TerminatedBeforeAcknowledged` as explicit `SpawnObservation`s, so the older
spawn-timeout-not-journaled concern appears fixed for the C3 spawn edge.

Current `c4` child-result observation still records a `Before` entry, then
returns a timeout error without an `After` observation when the child result does
not arrive before `execution.observe_child_stale_after_secs`. Journal replay can
classify that pending observation as `StaleOrHung` from elapsed time, but it does
not preserve the stronger fact that a particular controller already fired the
configured timeout policy.

Current `journal::replay_spawn_child` still accepts a group containing spawned,
observed, and ready records without checking whether the observed result is
incompatible with ready. That means a contradictory
`Spawned + Observed(TerminatedBeforeAcknowledged) + ChildReady` history still
appears normalizable instead of being rejected.

### Operator binary provenance is partly policy/documentation

Status: **partially implemented**.

The operator docs warn to use the `ploke-eval` binary built inside the active
parent checkout for authority-bearing live parent execution. The successor
invocation path checks active parent root and sealed identity, but this document
has not yet confirmed a full binary-provenance check for every operator entry.
Treat cross-checkout binary use as a known audit topic.

### `loop walk` gates effectful live edges

Status: **implemented operator guard**.

`walk` is not authority by itself; it consumes the same typed states and calls
the same `live_edges` functions. It adds explicit operator gates around long or
mutating edges:

- R7 -> R8 child-plan authority requires `--watch`.
- R10 -> R11 child fanout/rejected-only projection requires `--watch`.
- R12 -> R13b selected-successor handoff requires both `--watch` and
  `--allow git-changes`, because it installs the selected successor into the
  active checkout, seals/appends History, retires the parent, and spawns/waits
  for successor readiness.
- Nested live LLM/tool-loop stepping requires `--watch --allow
  workspace-mutation`.

This guard is ergonomic and safety-relevant, but it is not a substitute for the
underlying transition checks. It prevents accidental operator advancement through
known effectful boundaries.

### Successor handoff commits History before successor readiness

Status: **implemented; semantics need operator clarity**.

`spawn_and_handoff_prototype1_successor` installs/builds the selected successor
artifact in the active parent checkout, opens/seals a History handoff block,
appends it through `FsBlockStore`, writes the successor invocation, and only then
spawns the successor and waits for ready. On ready timeout, it records a
successor timeout and returns `Parent<Retired>` with no ready handoff object.

This matches the History/Crown model that there may be zero rulers during
handoff, but it means `R13bHandoffCommitted` should not be read as "successor is
now a live parent." It means the predecessor committed the handoff authority and
retired. The successor still has to validate sealed History and acknowledge
before it becomes the next parent.

### Durable reconstruction is strict but still projection-heavy

Status: **partially implemented**.

`driver::reconstruct` explicitly promises not to replay side effects. It reads
parent identity, campaign/run-profile data, journal entries, child-plan messages,
channel/result evidence, successor handoff records, and parent-complete evidence.
When evidence is missing or inconsistent, it returns the latest safe typestate
plus blockers instead of fabricating evidence.

The strictness is good, but the reconstruction path still derives several
higher phases from projections and recomputed live helper logic:

- `R8` is reconstructed from an existing child-plan message after validation.
- `R11` is reconstructed from store/channel-derived child outcomes.
- `R12` report facts are rebuilt by calling the same report projection edge.
- `R13b` is reconstructed from successor-handoff journal evidence plus ready
  path existence, not solely from an in-memory handoff carrier.

This is a reasonable transitional model, but it should not be described as pure
History reconstruction. It is reconstruction from the current durable evidence
families under strict blockers.

## Discovery pass 2026-06-20: operational-semantics lens

The evalnomicon core notes distinguish expressions that evaluate in a state from
commands that execute to change state. The Prototype 1 typestate loop is best
read in that same operational style:

```text
configuration = one Runtime<...> value plus its durable environment
edge         = typed transition function consuming one configuration
execution    = R0 -> ... -> R14, with branch rules at explicit sum types
```

`driver::advance::run_to_terminal` is the current direct execution relation for
a parent turn. It constructs `R0`, applies `r0_to_r1`, branches through
`R1Branch`, validates parent startup, advances through parent work, branches
through fanout/continuation, and ends at `R14FinalBranch`. `walk` is not a second
semantics: it is an incremental/debug projection over the same `live_edges`
functions, with operator gates around expensive or mutating edges.

This makes each edge look like a proof rule with obligations:

1. consume the predecessor carrier so it cannot be reused accidentally;
2. check any external facts that Rust types cannot know, such as files,
   worktree state, process/channel records, provider output, and History heads;
3. write or read durable evidence at the boundary where the edge claims it;
4. return only the successor alias whose axes correspond to the facts just
   established.

The correctness question is therefore not just "does `R8` have the right type?"
It is "is the only production path for `R8` the rule that actually establishes
child-plan authority, and can a restarted/runtime peer reconstruct or verify that
fact from durable evidence?"

### Authority/evidence strength matrix

| Layer | Current strength | What it proves | Boundary / gap |
| --- | --- | --- | --- |
| `Step` / `AsyncStep` adjacency | **Implemented** | A composed pipeline cannot skip from a non-matching input alias to an unrelated output alias. | It proves type adjacency only, not that the edge body did all semantic checks. |
| Strong move-only carriers (`Parent<S>`, `Startup<Validated>`, `Crown<S>`, `Block<S>`, `Open/Locked/Received<M>`) | **Implemented locally** | Callers outside defining modules cannot normally mint these states from raw strings/paths. | The guarantee is local to module visibility and existing constructors; tests may have special constructors. |
| `Runtime<Phase, Role, Context, ...>` aliases | **Partially implemented** | Later functions can demand a structurally named state such as `R10` or `R13b`. | Many aliases are marker axes plus `context::Facts`; `from_collected_parent` can set markers after runtime checks rather than carrying fully typed payloads. |
| `context::Facts` value bundle | **Partially implemented** | Live edges pass concrete data needed by older controller logic. | Many required facts are still `Option`; missing-fact checks happen inside later transitions. |
| Child-plan message box | **Implemented for this box** | `Received<ChildPlan>` proves a `Parent<Planned>` consumed the concrete child-plan file addressed to the same parent and next generation. | Other cross-runtime buffers are not all represented as typed boxes yet. |
| Sealed History and `FsBlockStore::append` | **Implemented locally** | Handoff-critical block material is hash-checked, appended from an expected lineage state, and used by successor startup validation. | Not distributed consensus, global process uniqueness, or proof that model/eval judgment was correct. |
| Transition journal / node files / branch registry / reports | **Evidence/projection** | Useful replay, debugging, and operator context; some transitions require records from them. | They must not be described as Crown/History authority unless admitted by a typed transition. |
| `loop walk` gates | **Operator guard** | Prevents accidental advancement through known expensive/mutating edges. | It is not an authority source; the live edge still must enforce correctness. |

### Selection and continuation evidence

Selection has two separable questions:

1. **Which candidate coordinate did parent-side policy select?**
2. **May that coordinate receive successor/continuation authority now?**

`SuccessorDecision` answers the first question at the selection-procedure level.
It records candidate node id, selected branch id, branch-evaluation disposition,
outcome, findings, and rationale. Its comments explicitly warn that
`branch_disposition = "reject"` is not the same as "must not be selected" when
traversal policy allows exploration from rejected children.

`live_successor_continuation_decision` answers the second question. It applies
search policy constraints such as direct-child shape, historical traversal guard,
`require_keep_for_continuation`, `explore_from_rejected`, max generation, and max
total nodes. Only `ContinueReady`, `ContinueExploreFromRejected`, and
`ContinueHistoricalTraversal` allow successor handoff.

The sealed selection payload is stronger than a plain `SuccessorDecision`. A
`SelectionDecisionEntry` commits to:

- the selection procedure/policy and scope;
- the selected candidate, occurrence id, and membership id when available;
- the ordered considered payload list plus a domain-separated order hash;
- a candidate-set commitment/membership proof over the considered universe;
- projection failures that were known at seal time;
- traversal replay evidence and child-count context;
- selection-time metrics, including a metric-set id tied to the same considered
  order and candidate-set root;
- the final `SuccessorDecision`.

This gives good replay and tamper-evidence for what the selector considered.
It still does not prove that the scoring rule is the right one, that model
judgment was correct, or that a selected rejected child is semantically an
improvement. Those are policy/evaluation claims, not typestate facts.

### What looks most rigorous today

The most rigorous parts share three properties: private constructors, move-only
state consumption, and durable verification.

- `Parent<Unchecked> -> Parent<Checked> -> Parent<Ready>` uses private `Parent`
  fields plus startup validation. Genesis startup requires local absent History
  state and generation 0; predecessor startup requires a sealed head, selected
  identity match, current clean artifact tree match, and surface match before
  entering `Parent<Ready>`.
- The child-plan box routes parent planning through `Open<ChildPlan>`,
  `Locked<ChildPlan>`, and `Received<ChildPlan>`. `Open<M>` is `must_use` and
  asserts if dropped while still armed; receipt validates the concrete box path,
  parent node id, and child generation.
- `Parent<Selectable>::seal_block_with_artifact` is the current Crown boundary:
  it consumes the selectable parent, creates a private `Crown<Ruling>`, admits
  artifact/selection material, locks/seals the block, and returns
  `Parent<Retired>` plus `Block<Sealed>`.
- `FsBlockStore::append` is the only semantic lineage-head advancement path in
  the local store. It verifies the sealed block hash, checks the current lineage
  state against the expected observed state, verifies append legality, appends to
  the block stream, and only then updates rebuildable head indexes.
- Successor startup rechecks sealed History instead of trusting only invocation
  JSON: it loads the sealed head, checks selected successor runtime and active
  artifact, validates selected parent identity, and recomputes current tree and
  surface before entering the parent path.

### What remains mostly discipline rather than proof

The outer `Runtime<...>` map is valuable even where it is not yet a full proof,
because it names the proof obligations. The weak points are where the alias name
implies a fact that is still carried by convention, option fields, or a legacy
projection.

- `context::Facts` is a staging bundle, not a dependent record. The type says
  `R10`; the concrete baseline, budget, schedule, strategy, and child plan still
  live behind `Option` fields checked by `r10_to_r11`.
- Some state constructors are internal but broad. For example,
  `from_collected_parent` can produce an alias with changed marker axes as long
  as the caller already has the right `Parent<S>` role carrier. That is safer
  than public struct literals, but it is weaker than constructors that require
  each semantic payload explicitly.
- The branch sum types prove which high-level fork occurred, not every payload
  property inside that fork. `R12ContinuationBranch::HandoffCommitted` proves the
  path returned a retired parent and handoff-shaped context; it does not mean the
  successor is already admitted as next parent.
- Durable reconstruction is strict about blockers, but it is reconstruction from
  several evidence families, not from sealed History alone. Describing it as
  "replay of the authoritative ledger" would overclaim.
- Binary provenance for operator-started parent runs is still partly an operator
  discipline. Successor build/spawn happens from the active checkout, but this
  pass has not found a universal check that every authority-bearing entrypoint is
  running the binary built from the active artifact it claims.

### Test coverage landmarks from this pass

This pass only skimmed test names and focused cases; treat this as a starting
index, not a complete coverage report.

- `typestate/tests.rs` covers `Transition`, `Step`, `AsyncStep`, composition,
  short-circuiting, and a few payload-carrying R aliases. It does not currently
  prove invalid compositions fail to compile.
- `inner.rs` tests cover message open/lock/unlock behavior and the panic-on-drop
  guard for unconsumed `Open<M>`.
- `parent.rs` tests cover genesis startup for generation 0, rejection of genesis
  startup for later generations, preservation of explicit runtime id during
  unchecked-parent load, and child-plan receipt rejection for the wrong parent.
- History tests are comparatively rich: append/projection behavior, duplicate
  genesis rejection, non-genesis-without-head rejection, stale expected-head
  rejection, different state-root rejection, missing projection rejection,
  sealed-head artifact-tree mismatch, surface mismatch, candidate-set membership
  checks, selection-decision mismatch checks, decision-grade eligibility, and
  bootstrap-authority rejection for child blocks all have named tests.
- `c4` tests confirm success sidecar/result-written projections alone do not
  advance `C4`; a successful channel result must include treatment evidence;
  failed channel results can advance as failed observations.
- `journal` tests cover replay classifications such as ready-without-observed,
  terminal-result-written-unobserved, and stale/hung child-result observation.
  The noted gap remains: current spawn replay normalizes
  `Spawned + Observed + ChildReady` without checking that `Observed` is
  semantically compatible with `ChildReady`.

### Relation to `doctor` / `step` / `continue`

There are two operator surfaces over related state, and they should not be
collapsed in documentation:

- `driver::advance::run_to_terminal` is the structural R0-R14 parent-turn
  execution relation. It keeps child fanout behind the large `R10 -> R11` edge.
- `run/core.rs` diagnosis is a file/projection-driven control loop used by
  `doctor`, `step`, and `continue`. It can stop and resume inside the child
  fanout by inspecting node status, child-plan files, result/channel records, and
  successor markers.

The diagnosed phases are a recovery/operator decomposition of the same conceptual
loop, not one-to-one aliases for R states:

| Diagnosed phase | Approximate typed-loop location | Notes |
| --- | --- | --- |
| `baseline_eval`, `baseline_protocol` | prerequisites before complete parent baseline / planning | Generation-0 closure work must complete before child planning. |
| `child_plan` | near `R7 -> R8` | Builds/receives the child-plan message and enters selectable parent state. |
| `materialize` | inside `R10 -> R11` | Child-attempt `C1 -> C2`; realizes temporary child artifact/worktree. |
| `build` | inside `R10 -> R11` | Child-attempt `C2 -> C3`; builds/promotes child binary. |
| `spawn` | inside `R10 -> R11` | Child-attempt `C3 -> C4`; spawns child and observes ready. |
| `observe` | inside `R10 -> R11` | Child-attempt `C4 -> C5`, plus parent comparison when treatment evidence exists. |
| `select` | around `R11/R12 -> R13` | Records parent-side successor selection/stopped marker; not handoff by itself. |
| `handoff` | `R12 -> R13b` | Installs selected artifact, seals/appends History, retires parent, spawns successor. |
| `complete` | `R14` or no selectable successor | Terminal report/projection state, not new authority. |
| `blocked` | no valid R edge should advance | A required evidence/projection check failed. |

This split is useful: `step`/`continue` can resume a partial child attempt after a
previous stop, while the R-phase driver gives a compact proof skeleton for a
whole parent turn. The risk is documentation drift. If a child-attempt invariant
is enforced in C1-C5 but not visible in R10/R11, docs should cite both layers.
If a `run/core.rs` diagnosis accepts a projection that the R edge would reject,
that is a correctness gap to audit rather than a harmless UI difference.

### Working definition of loop invariants

For later review, the loop invariants can be grouped by the kind of state they
protect.

**Ordering invariants:** parent identity precedes parent load; startup validation
precedes parent-start evidence; baseline/policy/plan precede fanout; fanout or
rejected-only projection precedes report facts; selection decision precedes
handoff; History append precedes successor-ready wait; successor ready precedes
successor admission on the next process.

**Authority invariants:** only a ready/selectable parent may publish and consume
its child-plan authority; only a selectable parent may lock Crown authority for
handoff; after handoff the predecessor is retired; the successor must validate
sealed History before entering the parent path; records and reports are not
promotional authority by themselves.

**Artifact/runtime invariants:** a child worktree is a temporary evaluation
surface, not the next long-lived parent root; selected artifact installation,
successor binary build, successor invocation, ready acknowledgement, and next
parent admission are separate facts; policy-bearing surface preservation is part
of ordinary successor admission until an explicit protocol-upgrade rule exists.

**Evidence invariants:** selection/evaluation records must name the evaluated
artifact/runtime and policy/oracle context; child self-report is evidence, not
promotion; late or backchannel evidence must be appended/imported under an
explicit rule rather than rewriting sealed History.

## Next reading pass

Planned next source reads / doc work:

- Add line-level anchors or a compact appendix for the key code claims above.
- Deepen the `successor_selection/` and `score/` summary with the exact
  evidence payloads sealed into History and the score-selection review outputs.
- Deepen the first-pass `run/core.rs` vs `driver/advance.rs` comparison with
  line-level anchors and known divergence tests.
- Read focused tests around History, replay, and `walk` to separate tested
  invariants from code-intent comments.
- Continue auditing C1-C5 child-attempt code against the live `run_child_fanout`
  path, especially timeout/replay contradictions and which C-state transitions
  are actually used by current parent turns.

## Working log: 2026-06-20 typestate documentation continuation

This section is intentionally an in-progress scratchpad for the next durable
pass. Promote or delete bullets as they become source-anchored above.

Initial orientation for this pass:

- GitNexus index for `/home/brasides/code/ploke` is current at commit `fed271b`.
  Query hits point at `run_prototype1_loop_controller`, `driver::advance`,
  `driver::reconstruct`, `walk::controller`, `live_edges`, and the `R12 -> R13`
  handoff/continuation edge as high-signal source paths.
- `crates/ploke-eval/docs/prototype1/operator-map.md` is the current operator
  map; `src/cli/prototype1_state/PROTOTYPE1_LOOP_OPERATOR.md` only redirects to
  it.
- Evalnomicon's consolidated Prototype 1 pages describe the loop as runtime
  succession, not a flat rerun. The core claim to preserve is the separation of
  artifact state, runtime state, lineage authority, and evidence/projection
  records.
- The next useful documentation improvement is not another phase table alone.
  It should explain the proof boundary for each layer: structural `Runtime` axes,
  stronger move-only carriers, durable History/message records, operator gates,
  and projection-only files.

Current questions to answer from source in this pass:

1. Which modules are allowed to construct or strengthen `Runtime<...>` aliases,
   and where are broad constructors such as `from_collected_parent` still a
   discipline boundary rather than a proof boundary?
2. How exactly do `driver::advance`, `live_edges`, and `walk::controller` share
   edge semantics, and where does `run/core.rs` still expose a different resume
   decomposition?
3. Which handoff facts are sealed into History, which are only journaled, and
   which are only process-local or projection facts at the moment `R13b` is
   returned?
4. Which child-attempt C1-C5 invariants are actually on the current fanout path,
   and which remain scaffold/replay-only?

Source findings from the first continuation read:

- The structural carrier is intentionally small and product-shaped:
  `Runtime<Phase, Role, Context, Plan, Children, History, Evidence,
  Continuation, Report>` in `typestate/runtime.rs`. The fields and the
  `Private` marker keep raw struct construction inside the typestate module
  tree, but the alias constructors in `typestate/aliases.rs` are still
  `pub(crate)`. Current call sites are concentrated in `live_edges.rs`,
  `driver/reconstruct.rs`, and typestate tests. This is a useful discipline
  boundary, not a fully closed proof boundary.
- `driver::advance::run_to_terminal` is the compact batch semantics for the
  typed parent turn. It creates `R0`, then calls the direct functions from
  `live_edges.rs`, with explicit branch handling at `R1`, `R4a`, `R10`, `R12`,
  and `R14`. `walk::controller::step_once` calls the same edge functions, so
  the debugger and batch path share transition bodies instead of duplicating the
  semantics.
- `walk` adds operator gates but not separate authority. It blocks `R7 -> R8`
  and `R10 -> R11` unless `--watch` is supplied, and it blocks selected-successor
  `R12 -> R13b` unless both `--watch` and `--allow git-changes` are supplied.
  These gates protect live provider/tool/fanout work and active-checkout
  mutation, but the underlying edge must still enforce semantic checks.
- The live child fanout path does use the C1-C5 child-attempt chain:
  `run_planned_child` constructs `C1` from `ChildFiles`, then applies
  `MaterializeBranch`, `BuildChild`, `SpawnChild`, and `ObserveChild` before the
  parent compares treatment evidence. Some source comments in C1/C2 still say
  the scaffold is not wired into the live controller; treat those comments as
  stale until refreshed.
- C3 spawn timeout handling is now explicit: `SpawnChild` records an observed
  spawn entry for `ExitedBeforeReady` and `ReadyTimedOut` before returning a
  rejected outcome. C4 completion timeout remains weaker: `ObserveChild` writes
  the `Before` completion entry, times out, and returns an error without writing
  an `After` result. Replay can classify the pending entry as stale/hung from
  elapsed time, but the journal does not preserve that the controller's timeout
  policy fired for that attempt.
- The handoff edge `r12_to_r13` delegates the dangerous path to
  `spawn_and_handoff_prototype1_successor`. That function installs the selected
  artifact into the active checkout, builds the successor binary from that
  checkout, seals a History block through `Parent<Selectable>::seal_block_with_artifact`,
  appends it via `FsBlockStore::append`, writes the successor invocation, then
  spawns and waits for successor ready. Therefore `R13b` means predecessor
  handoff was committed and the parent is retired; it does not by itself mean
  the successor has completed a later parent turn.
- `driver/reconstruct.rs` is strict about missing evidence and avoids replaying
  side effects, but the R13b reconstruction path is not a pure History replay:
  it currently reconstructs handoff from successor-handoff journal evidence,
  invocation identity, ready-path existence, and parent-complete resource
  evidence. That should be documented as strict durable reconstruction from
  multiple evidence families, not as sealed-History-only reconstruction.

Selection/continuation findings from the next read:

- `SuccessorDecision` records the parent-side selector's chosen coordinate and
  branch-evaluation disposition; its own docs warn not to read
  `branch_disposition = "reject"` as "cannot be selected." History traversal may
  bind `selected_branch_id` for an exploratory coordinate even when the
  candidate-local outcome is `Stop`.
- `Candidates::traverse_with_policy` makes selection replay stricter than a raw
  list walk: it filters decision-grade candidates, verifies selection-input
  bindings and candidate-set membership proofs, records projection failures
  separately, excludes candidates that already have successful children, binds
  selection metrics to the considered order/candidate-set root, and returns
  selected membership evidence when available.
- `SelectionDecisionEntry::new_with_traversal_identity_metrics` is the payload
  that gets admitted into the History block for selection. It commits to the
  considered order hash, candidate-set commitment, projection failures,
  traversal replay evidence, metrics, optional score formula rows, and the final
  `SuccessorDecision`; it also validates that metrics and decision identity bind
  back to the same considered universe.
- Continuation authority is a separate edge after selection. `r12_to_r13` calls
  `live_successor_continuation_decision`, which applies search-policy gates
  such as direct-child shape, historical traversal guard, keep requirement,
  `explore_from_rejected`, max generation, and max total nodes. Only decisions
  whose disposition allows successor handoff are passed to the History/Crown
  handoff path.

Data-model finding:

- `loop_graph.rs` is the crate-level vocabulary for the artifact/runtime model
  that evalnomicon describes. It defines durable `RuntimeId`, backend-neutral
  `ArtifactId`, `PatchId`, `OperationTarget`, and `Coordinate { runtime_id,
  target }`. The module docs explicitly warn that many live paths still carry
  only text-file surface identities or absent runtime coordinates. This supports
  the documentation distinction between the intended graph model and the current
  narrower live records.

`doctor` / `step` / `continue` comparison finding:

- `run/core.rs` owns the diagnosed-phase operator path. It reconstructs the
  active parent context from checkout files and admitted profile, diagnoses
  phases such as `baseline_eval`, `child_plan`, `materialize`, `build`, `spawn`,
  `observe`, `select`, and `handoff`, then advances one phase or loops up to a
  guard. This is intentionally more resumable inside child fanout than the
  compact R0-R14 batch relation.
- The diagnosed child phases rehydrate C2/C3/C4 from node/runner/channel
  projections and then apply `BuildChild`, `SpawnChild`, or `ObserveChild`.
  Handoff recomputes selection from terminal child snapshots and calls the same
  `spawn_and_handoff_prototype1_successor` process seam, but it does so from a
  diagnosed/projection context rather than by consuming an in-memory `R12`.
  Documentation should present this as an operator resume decomposition of the
  same conceptual loop, not as a second typestate proof skeleton.

Admitted-profile finding:

- `profile.rs` is part of the loop invariant surface, not just CLI config. Its
  validation rejects unsupported schema versions, invalid child budgets, invalid
  route/provider combinations, relative-oracle settings that require missing MBE
  evidence, zero broad-TUI limits/timeouts, disabled metric persistence when
  metrics are used for scoring, and `control.parallel_cap` values that would
  widen admitted fanout. The typestate docs should treat an admitted
  `run-profile.toml` plus commitment as policy evidence feeding R6/R7/R10/R12,
  not as optional operator preference.

## Working log: 2026-06-20 current continuation

Goal for this continuation: turn the existing working draft into a more durable
explanation of *why* the loop is shaped this way, what each proof layer can and
cannot guarantee, and how evalnomicon's intent maps onto the current code.

Immediate assumptions to verify from source rather than preserve as folklore:

1. `Runtime<...>` is a structural proof skeleton over an older value bundle, not
   a complete proof object by itself.
2. The rigorous boundaries are the smaller move-only carriers and durable
   admission/seal APIs: `Parent<S>`, `Startup<Validated>`, message boxes,
   `Crown<S>`, sealed `Block<S>`, and `FsBlockStore::append`.
3. `driver::advance` and `walk` share edge bodies, while `run/core.rs` is a
   projection/recovery controller over the same conceptual loop but not the same
   typestate proof skeleton.
4. The hardest correctness boundary is handoff: selection evidence, continuation
   policy, active-checkout installation, History append, successor ready, and
   successor admission are distinct facts and should not be collapsed.

Next reads in this continuation:

- evalnomicon Prototype 1 pages: artifact/runtime model, runtime authority,
  invariant ledger, selection/evaluation, persistence/observability.
- `typestate/runtime.rs`, `aliases.rs`, `context.rs`, `shape.rs`, and
  `transition.rs` for the core type/data model.
- `live_edges.rs`, `driver/advance.rs`, `driver/reconstruct.rs`, and
  `walk/controller.rs` for shared edge semantics and reconstruction gaps.
- `history/seal`, `history/stored`, and `history/projection` around the exact
  sealed handoff payload.

Continuation findings after evalnomicon/source read:

- Evalnomicon's Prototype 1 section explicitly sets source priority: current
  code comments in `prototype1_state/mod.rs` and `history/mod.rs` first, then
  consolidated book pages, then drafts. It also requires status labels
  (`Implemented`, `Partially implemented`, `Intended`, `Not claimed`). This
  document should preserve those labels rather than turning the typestate map
  into a single-strength architecture claim.
- The formal-procedure notes are a useful lens for the loop: keep step-local
  input/output boundaries typed, but do not encode the whole dynamic search tree
  as one global compile-time type. The current implementation follows that:
  `Runtime<...>` gives a compact parent-turn skeleton, while child fanout,
  selection traversal, and durable evidence remain runtime graphs/projections.
- The central carrier in `typestate/runtime.rs` has nine axes and private fields:
  phase, role, context, plan, children, history, evidence, continuation, and
  report. `RuntimeShape` mirrors those aliases for the walk debugger so humans
  can see axis deltas without maintaining a second phase table by hand.
- `transition.rs` gives a precise guarantee and no more: `Step::then` and
  `advance` prove edge adjacency (`R8 -> R9 -> R10` can compose, `R8 -> R12`
  cannot) but they do not prove the edge body performed every external check.
  Semantic truth still lives in edge functions and durable validation.
- `aliases.rs` makes the proof boundary visible but also shows the migration
  seam. Constructors like `from_collected_parent` are crate-visible and mostly
  change marker axes around a `Parent<S>` plus `context::Collected`. This is
  stronger than loose locals and public struct literals, but weaker than a fully
  payload-specific constructor for each fact.
- `context::Facts` is the main transitional bundle. It carries baseline, policy,
  child-plan, selection strategy, child outcomes, selection material, report
  facts, and parent identity behind `Option`s. Later edges re-check those facts
  and fail if they are missing. Therefore R aliases should be read as phase
  obligations plus checked facts, not as dependent records that make missing
  facts impossible.
- The strong islands are much more rigorous. `Parent<Unchecked>`,
  `Parent<Checked>`, `Parent<Ready>`, and `Parent<Selectable>` have private
  fields and state-specific transitions. `Startup<Genesis>::from_history`
  requires local absent History and generation 0; `Startup<Predecessor>`
  requires a sealed head, selected parent identity match, clean tree match, and
  surface match before `Parent<Ready>`.
- Message boxes are rigorous for the child-plan case. `Open<ChildPlan>` consumes
  `Parent<Ready>` and is `must_use`; dropping an armed open message panics.
  `Locked<ChildPlan>::unlock` validates the concrete box path, parent node id,
  and direct child generation before returning `Parent<Selectable>` plus
  `Received<ChildPlan>`.
- The History store has a clear local append contract. `FsBlockStore::append`
  verifies the sealed block hash, reads the current lineage state, rejects stale
  expected state, checks state-root/proof/head append legality, appends the block
  line, then writes rebuildable indexes and `heads.json`. This is rigorous local
  tamper evidence, not a global consensus or process-uniqueness proof.
- The exact handoff sequence in `spawn_and_handoff_prototype1_successor` is:
  install selected artifact into the active checkout, build `ploke-eval` from
  that checkout, open/seal a History block with artifact and selection decision
  evidence, append it, write successor invocation, spawn successor, then wait
  for ready/timeout/early-exit. So `R13b` means predecessor handoff authority is
  committed and the predecessor is retired; successor readiness and next-parent
  admission remain later evidence.
- `run/core.rs` intentionally has a different shape from the compact R driver.
  It diagnoses durable projections into operator phases and can resume individual
  C2/C3/C4 child attempts. Its `active_parent_ready` path validates checkout and
  History startup, but does not require a live `SuccessorInvocation` runtime-id
  binding the way `live_edges` does for the handoff-started batch path. This
  reinforces the narrow claim: current admission is primarily History/artifact
  local, not proof that a unique successor OS process is the only runner.

Negative design constraints from `prototype1_state/mod.rs` worth promoting into
stable documentation:

- Do not assume one global active parent; the current single-parent path is a
  constrained prototype, not the semantic model.
- Do not store a singleton "current best branch"; selection must preserve
  candidate scope, policy, and evidence so future traversal can replay it.
- Do not overwrite evaluation state; append observations, decisions, handoffs,
  and failures.
- Do not store scores without evaluator, eval-set/oracle, and policy identity.
- Do not make the analysis engine part of the trusted root.
- Do not let a runtime self-report become promotion without parent-side policy,
  History/Crown checks, and successor admission.
- Do not couple worktree layout to semantic artifact/tree identity.
- Do not make successor authority imply global authority; authority is
  lineage-scoped.

These are useful because they reveal the intended shape by exclusion: the loop is
not a branch picker, not a mutable scheduler status machine, not a process lock,
and not a self-reporting child promotion scheme. It is a typed succession
protocol over artifact/runtime pairs with local History authority and explicit
projection/evidence boundaries.

Prior audit reconciliation notes:

- Older `docs/reports/prototype1-history-v2-audit` and
  `prototype1-history-typestate-review` findings must be treated as historical.
  Several high-severity concerns from 2026-04-29 appear fixed or narrowed in
  current source: handoff now seals and appends History before successor spawn;
  successor continuation validation now reads the sealed head; and the generic
  `Crown<S>::for_lineage` constructor is no longer visible as described there.
- The same audits are still useful for themes: separate cloneable transport JSON
  from move-only executable authority; avoid treating invocation/ready files as
  Crown authority; and distinguish parent retirement from successor admission.
- A remaining audit question from current source: predecessor-side ready wait
  checks for a `ToParent::SuccessorReady` message on the expected runtime channel
  but does not appear to inspect the record payload fields in that wait loop.
  Because successor startup performs the stronger History/artifact validation
  before sending readiness, this is evidence quality rather than the primary
  authority gate, but docs should not describe predecessor ready wait as an
  independent proof of successor identity.

Additional test-coverage findings from this continuation:

- `typestate/tests.rs` exercises the combinator surface (`Transition`, direct
  function `Step`, `then`, short-circuiting, async edges) and a few payload
  carriers (`R2a`, `R3`, `R4a`). It does not act as compile-fail coverage for
  invalid R-edge composition.
- History tests now cover more than simple hash recomputation. The current suite
  includes append/index projection behavior, stale head rejection, wrong state
  root rejection, missing head-projection rejection, verified loading of stored
  sealed blocks, and preservation of stored JSON field order for block/selection
  hashes. That strengthens the claim that History is a local tamper-evident
  append substrate rather than just an in-memory block type.
- Selection/History tests cover candidate-set commitments and traversal entries,
  including cross-generation considered sets and avoiding re-ingestion of prior
  traversal considered payloads as fresh candidates. This supports the claim
  that selection evidence is replay-oriented and scope-bound.
- C4 tests exercise success/failure terminal observations and explicitly reject
  successful runner results that omit treatment evidence. Timeout tests confirm
  the current behavior described above: timeout is returned as an error after a
  `Before` observation rather than as a committed `After` timeout record.

## Working log: 2026-06-20 intent/model pass

This pass is continuing the durable typestate documentation rather than starting
from an empty map. Initial source priority follows evalnomicon's Prototype 1
section: current `prototype1_state` code comments first, consolidated
`evalnomicon/src/prototype1/*` pages second, drafts third, and older reports only
as historical audit context.

Immediate working thesis to check against code:

- The loop is a typed succession protocol over runtime/artifact coordinates, not
  a scheduler status machine and not a flat `baseline -> patch -> rerun` script.
- The outer `Runtime<...>` type is a proof skeleton for local edge ordering. Its
  strongest correctness comes when an axis carries a smaller move-only proof
  object (`Parent<S>`, `Startup<Validated>`, `Received<ChildPlan>`, sealed
  `Block<S>`) instead of only a marker plus an optional value in `context::Facts`.
- The authority story is deliberately local and lineage-scoped: History/Crown can
  make handoff tamper-evident and restart-checkable, but it does not claim global
  process uniqueness or correctness of model/evaluator judgment.
- The major documentation risk is overclaiming: `R13b` must mean predecessor
  handoff committed and predecessor retired, not successor already admitted as a
  future parent; `walk` gates must mean operator safety guard, not authority; and
  transition journal/projection files must not be described as Crown authority.

Next source reads for this pass: the module-level intent comments in
`prototype1_state/mod.rs`, the crate-level graph vocabulary in `loop_graph.rs`,
the structural carrier/alias/context files under `typestate/`, and the live edge
and History append/seal code paths used by handoff.

Source findings from this pass so far:

- `prototype1_state/mod.rs` explains the system by negative constraints: do not
  assume one global parent, do not store a singleton current-best branch, do not
  overwrite evaluation state, do not store scores without evaluator/eval-set/
  policy identity, do not trust the analysis engine as root, and do not let a
  runtime self-report become promotion. These constraints are not incidental;
  they define the intended model as a succession protocol over runtime/artifact
  pairs with append-only evidence and lineage-scoped authority.
- `loop_graph.rs` is intentionally forward vocabulary. `RuntimeId`,
  `ArtifactId`, `PatchId`, `OperationTarget`, and `Coordinate` express the
  intended graph, while the module docs explicitly admit that many live paths
  still carry only text-file surface identities or absent runtime coordinates.
  Documentation should therefore separate the intended graph model from the
  current live provenance quality.
- `typestate/runtime.rs` confirms the core carrier is a nine-axis product type,
  not a monolithic enum. `typestate/shape.rs` mirrors the same axes for `walk`
  rendering, which reduces doc/UI drift by deriving human-visible axis deltas
  from the alias declarations.
- `typestate/transition.rs` states the exact compile-time guarantee: `Step` and
  `AsyncStep` enforce typed adjacency and consume the prior state. They do not
  themselves grant authority or prove the transition body checked the filesystem,
  History, process, or provider facts it claims to establish.
- `typestate/aliases.rs` makes both the value and the gap visible. `R8` carries
  a real `Received<ChildPlan>` capability and `Parent<Selectable>`, while many
  later facts still live in `context::Facts` behind `Option` fields. Constructors
  such as `from_collected_parent` are crate-visible migration seams: useful for
  concentrating construction, but not as strong as payload-specific private
  constructors for every semantic fact.
- `Parent<S>` and `Startup<Validated>` are stronger than most marker axes.
  `Parent<Unchecked>::check`, `Parent<Checked>::ready`, and
  `Parent<Unchecked>::ready_from_predecessor_startup` consume role carriers and
  private startup evidence. Genesis startup requires a generation-0 parent and
  an absent local History head; predecessor startup requires a present sealed
  head, matching selected parent identity, clean current tree, and matching
  surface commitment.
- The child-plan box is a concrete rigorous message example: `Open<ChildPlan>`
  consumes the sender state and panics if dropped while armed; `Locked<ChildPlan>`
  reads a concrete file; `unlock` validates the receiver against the box path,
  parent node id, and direct child generation before yielding `Received<ChildPlan>`
  plus `Parent<Selectable>`.
- The handoff path is the main authority boundary. `spawn_and_handoff_prototype1_successor`
  installs the selected artifact in the active checkout, builds the successor
  binary from that checkout, seals a block through
  `Parent<Selectable>::seal_block_with_artifact`, appends it with
  `FsBlockStore::append`, writes the successor invocation, spawns the successor,
  and waits for ready/timeout/early exit. `FsBlockStore::append` verifies the
  sealed block hash, current lineage state, append legality, and state root
  before updating rebuildable heads. This supports the `R13b = predecessor
  committed and retired` reading, not `successor is already admitted`.
- `driver::advance::run_to_terminal` is the compact batch execution relation.
  `walk::controller::step_once` calls the same `live_edges` functions and adds
  operator gates around R7->R8, R10->R11, and selected-successor R12->R13b.
  `run/core.rs` is a different projection/recovery controller that diagnoses
  persisted files into resumable child phases and rehydrates C2/C3/C4 from
  projections. It is operationally important but should not be described as the
  same proof skeleton as the R0-R14 driver.
- Selection has a deliberately split proof story. `SuccessorDecision` records
  the parent selector's chosen coordinate and the candidate's branch disposition;
  it explicitly warns that `reject` can still be a selected traversal coordinate.
  `live_successor_continuation_decision` then applies continuation policy gates
  such as direct-child shape, historical traversal budget, keep requirement,
  `explore_from_rejected`, generation limit, and total-node limit. The sealed
  `SelectionDecisionEntry` binds the considered order, candidate-set commitment,
  projection failures, traversal evidence, metrics, formula rows, and final
  decision together before History admission.
- The C1-C5 child-attempt chain is now on the live `run_planned_child` path even
  though some file headers still say it was only scaffold. The chain establishes
  materialized child artifact, built child binary, ready child runtime, terminal
  child observation, and then parent-side treatment comparison. C3 spawn timeout
  is explicitly recorded as an observed spawn result; C4 result timeout still
  records only the `Before` observation plus a timeout error.
- The admitted run profile is part of the invariant surface. `profile.rs` rejects
  unsupported schema versions, invalid budgets/timeouts/provider route shapes,
  relative-oracle configurations without required MBE/target evidence,
  non-persisted metrics used for scoring, and control caps that widen fanout.
  Treating `run-profile.toml` as mere operator preference would understate its
  role in R6/R7 planning, R10 selection, and R12 continuation.

## Working log: current exploration pass

This pass is continuing the in-progress document by checking the implementation
against the evalnomicon intent, not by assuming the existing draft is already
canonical. The main sources read in this pass were:

- consolidated evalnomicon Prototype 1 pages: `runtime-loop`,
  `artifact-runtime-model`, `runtime-authority`, `history-crown`,
  `selection-and-evaluation`, `persistence-and-observability`, and
  `invariant-ledger`;
- implementation intent comments in `prototype1_state/mod.rs` and
  `history/mod.rs`;
- core typestate files: `runtime.rs`, `transition.rs`, `context.rs`, and
  `aliases.rs`;
- live execution seams: `driver/advance.rs`, `live_edges.rs`,
  `walk/controller.rs`, `run/core.rs`, `parent.rs`, `inner.rs`,
  `prototype1_process.rs`, and the C1-C5 child-attempt files.

Fresh synthesis from this pass:

- The best short description of the system is: **a typed local succession
  protocol over runtime/artifact coordinates, with sealed History as the local
  authority surface and many other files as projections or evidence sources**.
  That phrasing avoids the two main overclaims: it is not a flat rerun loop, and
  it is not global consensus/process uniqueness.
- The outer R0-R14 typestate is rigorous primarily as an ordering and review
  scaffold. Its strongest facts come when an R alias embeds a smaller carrier
  that cannot be freely minted, such as `Parent<Ready>`,
  `Parent<Selectable>`, `Received<ChildPlan>`, or a sealed History block. The
  R alias itself is weaker when the claimed fact lives only in `context::Facts`.
- The implementation intentionally follows evalnomicon's formal-procedure
  advice: keep step-local boundaries typed, but leave the larger branching
  search/evidence graph as runtime data with stable identities and durable
  records. Trying to encode the whole campaign graph in one compile-time type
  would be the wrong target.
- There are three operator/execution views that should stay distinct in docs:
  `driver::advance::run_to_terminal` is the compact batch R-edge semantics;
  `walk::controller::step_once` steps those same edge bodies with explicit
  operator gates; `run/core.rs` diagnoses persisted projections into resumable
  operator phases and can resume inside child fanout. They overlap in purpose,
  but they are not the same proof object.
- Handoff remains the most important correctness boundary to explain without
  collapsing facts. Selection material, continuation policy, active-checkout
  installation, successor binary build, History seal/append, successor
  invocation, predecessor-side ready wait, and successor startup admission are
  separate facts. `R13b` names the predecessor-side committed/retired state, not
  a proof that the successor has completed or will complete the next turn.

Current open questions for the next pass:

1. **Constructor closure:** Should alias constructors such as
   `from_collected_parent` remain `pub(crate)`, or should more semantic facts
   move into private payload-specific constructors so R aliases become less
   dependent on caller discipline?
2. **Ready acknowledgement evidence:** The predecessor ready wait currently
   looks for a `SuccessorReady` channel message; successor startup performs the
   stronger sealed History/artifact/surface checks before sending readiness.
   Document whether predecessor-side ready should remain transport evidence or
   should also validate payload fields explicitly.
3. **Projection/import boundary:** Which current journal, node, branch, and
   report facts are intended to become admitted History entries or ingress, and
   which should remain only replay/operator projections?
4. **Timeout semantics:** C3 spawn timeout is now explicit in observations; C4
   result timeout still returns an error after a `Before` journal entry. Decide
   whether that is an acceptable replay-derived stale/hung state or whether the
   controller firing timeout should be committed as its own `After` fact.
5. **Binary provenance:** Successor handoff builds from the active checkout, but
   authority-bearing operator invocations still rely partly on using the binary
   from the active parent checkout. The docs should keep this as operator policy
   until every authority-bearing entrypoint verifies binary/artifact provenance.
