# 2026-05-09 Run Playback Typed Observability Plan

Plan for the Prototype 1 run playback / live observability surface that should
feed CLI debugging, `ploke-tree`, and a future UI or WebAssembly front end.

## Goal

Build a typed, iterable playback model over persisted Prototype 1 run records.
The model should support an interactive visual timeline/graph that can step
through a multi-generation self-improvement run at multiple levels of
granularity:

- parent/ruler startup and handoff;
- child candidate materialization, build, spawn, observe, and evaluation;
- scoring and successor selection;
- sealed History block admission;
- successor parent continuation;
- detailed evidence drill-down for UI, CLI, and internal debugging.

The playback layer is a read/projection model. It must not become active loop
authority.

## Current Evidence Shape

Known shared record material is sufficient for an initial coarse replay, but not
yet sufficient for the full observability app without more normalization.

Strongest current spine:

- sealed History blocks and admitted selection payloads;
- scheduler node/request/result records;
- evaluation and protocol artifacts;
- branch, invocation, channel, journal, selection, and History payload DTOs.

Known weak points:

- not every replay-relevant passive DTO has explicit `Record` metadata;
- the run chain is split across several record families rather than one joined
  object;
- some surfaces are projection or transport records, not authority;
- exact successor-handoff micro-order needs bounded verification before being
  promised in UI semantics.

## Authority Tiers

Playback steps should carry evidence strength explicitly. Coarsening or visual
rendering must not upgrade weak evidence.

Suggested tiers:

- `Projection`: scheduler, branch registry, monitor/cache, and other read-only
  convenience surfaces.
- `TypedRecord`: passive records with stable typed schema and join keys.
- `AdmittedHistory`: records admitted into History as entries or payloads.
- `SealedHistory`: sealed block/header/hash evidence.

Active execution must continue to use authoritative eval/runtime/History paths,
not the playback projection.

## Core Types

The core model should be Rust-native and iterable.

```rust
pub struct RunPlayback<G = Fine> {
    steps: Vec<G::Step>,
    _granularity: PhantomData<G>,
}

pub struct RunPlaybackRef<'a, G = Fine> {
    steps: Vec<G::StepRef<'a>>,
    _granularity: PhantomData<G>,
}

pub trait PlaybackGranularity {
    type Step;
    type StepRef<'a>
    where
        Self: 'a;
}
```

`RunPlayback` is the owned snapshot form. It is appropriate for CLI output,
tests, export, cache snapshots, worker boundaries, and WebAssembly handoff.

`RunPlaybackRef` is the borrowed form. It is appropriate for fast UI projection
over an already-loaded record store, live refresh, and detail panes that should
avoid cloning large payloads.

Both forms should implement normal iterator ergonomics:

```rust
impl<'a, G> IntoIterator for &'a RunPlayback<G>
where
    G: PlaybackGranularity,
{
    type Item = &'a G::Step;
    type IntoIter = std::slice::Iter<'a, G::Step>;
}

impl<G> IntoIterator for RunPlayback<G>
where
    G: PlaybackGranularity,
{
    type Item = G::Step;
    type IntoIter = std::vec::IntoIter<G::Step>;
}
```

Equivalent borrowed implementations should exist for `RunPlaybackRef<'a, G>`.

## Granularity Typestates

Granularity should be represented structurally, not by string flags.

Possible type states:

- `Coarse`: generation or sealed-decision timeline.
- `ParentTurn`: one parent runtime turn and its selected successor.
- `ChildAttempt`: candidate-level materialize/build/spawn/observe/evaluate
  timeline.
- `Fine`: canonical detailed step sequence.
- `Evidence`: detail/provenance-oriented view for selected records.

Example:

```rust
let fine: RunPlayback<Fine> = load_or_project(...);
let coarse: RunPlayback<Coarse> = fine.coarsen();

for step in &coarse {
    // step: &CoarseStep
}
```

Granularity is a projection level, not an authority level.

## Playback Ordering

Playback order must come from causal records, not timestamps, filesystem order,
or scheduler projection order.

Ordering priority:

1. Sealed History order:
   `lineage_id`, `block_height`, block parent hash/hash links, and entry order
   inside the sealed block.
2. Journal or transition append order for fine-grained parent/child/handoff
   steps.
3. Invocation or runtime-local ordering for per-attempt details.
4. Typed artifacts attach as evidence to the step they support.
5. Scheduler node/request/result records are context only. They may provide
   labels, ids, paths, and status, but must not define playback causality.

A canonical phase order for fine playback should be explicit:

```text
ParentResumeOrStart
ParentStarted
ChildPlanned
ChildMaterialized
ChildBuilt
ChildSpawned
ChildObserved
EvaluationRecorded
CandidateConsidered
SuccessorSelected
HistoryEntryAdmitted
HistoryBlockSealed
SuccessorHandoffPrepared
SuccessorReadyAck
ParentCompleted
SuccessorCompletion
```

Coarse playback may collapse those phases, but it should still be derived from
the same causal order:

```text
ParentStarted
ChildrenEvaluated
SuccessorSelected
HistoryBlockSealed
SuccessorHandoff
NextParentStarted
```

Timestamps are audit data, not ordering authority. If timestamps contradict the
causal order, playback should keep causal order and emit a warning.

```rust
pub enum TimeConsistency {
    Aligned,
    MissingTimestamp,
    OutOfOrder {
        earlier_step: StepId,
        later_step: StepId,
        earlier_timestamp: String,
        later_timestamp: String,
    },
    ClockSkewSuspected,
}
```

Rule: causal records define order; timestamps audit order.

## Step Shape

Steps should be light enough for UI iteration. Prefer stable ids, labels,
ordering keys, timestamps if available, evidence strength, and references into a
record store. Large payloads should stay in the store and be loaded for detail
panels only.

Representative step families:

- `ParentStarted`
- `ChildMaterialized`
- `ChildBuilt`
- `ChildSpawned`
- `ChildObserved`
- `EvaluationRecorded`
- `SelectionConsidered`
- `SuccessorSelected`
- `HistoryBlockSealed`
- `SuccessorHandoff`
- `ParentCompleted`

Do not flatten role/state structure into long names. If a step needs to express
role and state, use structural carriers such as `Role<State>` or a typed
subrecord rather than names like `ChildReadyEvent`.

## Crate Boundary

Recommended split:

- `ploke-records`: passive schemas, `Record` metadata, evidence-strength
  vocabulary, playback step vocabulary, borrowed/owned playback container
  traits if they require no filesystem access.
- `ploke-tree`: loads/indexes record files, joins typed records, builds
  `RunPlaybackRef` and `RunPlayback` projections.
- `ploke-eval`: emits authoritative records and may use playback for debugging
  CLI projection only.
- front-facing UI crate: renders playback/timeline/graph/detail panels from
  `ploke-tree` projections. It should not parse CLI text or own authority.

If `ploke-records` exposes any builder, it should accept already-loaded passive
records or a neutral record index. Directory walking belongs outside
`ploke-records`.

## First Slice

1. Add explicit `Record` metadata for replay-relevant passive DTOs that are
   already stable enough to share.
2. Define minimal `EvidenceStrength`, `PlaybackGranularity`, `RunPlayback`, and
   `RunPlaybackRef` vocabulary without filesystem access.
3. In `ploke-tree`, build a coarse `RunPlaybackRef<Coarse>` from sealed History
   selection entries plus scheduler/evaluation context.
4. Add tests over small synthetic records before using a real campaign.
5. Add a CLI/debug projection only after the typed playback model exists.

## Tight Validation Loop

Ground each slice against the last live run with bounded, compact checks. Do not
dump raw JSON or logs into agent context.

Loop:

1. Pick one fixed live campaign fixture.
2. Define one expected semantic chain for the current slice.
3. Add or update a narrow synthetic test for edge behavior.
4. Run one bounded real-run check that prints counts, ids, and warnings only.
5. Compare against the known handoff chain or expected invariant.
6. Fix the model or record a gap.
7. Repeat for exactly one projection field, join, or warning class at a time.

After each loop, run the abstraction/module checkpoint:

```text
Did this introduce a second consumer, invariant, or lawful operation?
If yes, promote the concept into a named data structure.

Did this file gain a second responsibility or force long helper names?
If yes, split by semantic boundary before adding the next feature.
```

The first coarse real-run check should prove only:

- one coarse step per sealed successor-selection decision;
- selected successor ids match sealed payloads;
- block heights are monotonic;
- parent hashes link;
- each step has considered-candidate count;
- timestamp warnings are reported without reordering playback.

Example bounded command shape:

```bash
cargo test -p ploke-tree coarse_playback_real_run -- --ignored 2>&1 | tail -n 80
```

If a diagnostic command is used before a test exists, it should print a compact
table like:

```text
block 0 parent <id> selected <id> candidates <n> warnings <n>
```

## Stop Conditions

Stop an implementation slice when one semantic promise is implemented and
validated. Do not let a successful coarse replay expand directly into UI,
fine-grained playback, graph layout, WebAssembly, or live refresh.

For the first coarse slice, done means:

- `RunPlaybackRef<Coarse>` is built from typed records, not CLI text;
- sealed History is the ordering spine;
- scheduler records are not used for ordering;
- there is one step per sealed successor-selection decision;
- block heights are monotonic;
- block parent hashes link correctly;
- selected successor id and considered-candidate count are present;
- missing optional context becomes a warning, not a panic;
- timestamp disagreement becomes a warning, not a reorder;
- the compact real-run check matches the known 12-block chain from the handoff.

General stop rule:

```text
one semantic promise implemented
one synthetic test proves edge behavior
one live-run check proves real compatibility
warnings expose known gaps
no new authority claim introduced
```

If a slice needs new concepts to explain itself, stop earlier and record the
gap instead of expanding scope.

## Abstraction and Module Pressure

Generalize only when a data structure preserves a real semantic relation across
multiple consumers or prevents a known class of bugs. Do not generalize merely
because two call sites have similar syntax.

Good abstraction signals:

- the same shape is needed by more than one consumer, such as CLI replay and UI
  timeline;
- the shape carries stable domain meaning, such as `StepId`,
  `EvidenceStrength`, `PlaybackOrder`, `RunPlaybackRef`, `RecordRef`, or
  `JoinKey`;
- the shape prevents weak joins, authority upgrades, timestamp ordering, or
  other known playback errors;
- the shape has lawful operations such as iterate, filter, coarsen, attach
  warnings, or dereference evidence;
- callers should not be trusted to manually keep the fields in sync;
- naming the concept makes nearby code shorter and clearer.

Bad abstraction signals:

- it exists only to make one command output easier;
- it is named after a view, report, or output rather than a domain object;
- it duplicates record fields without adding structure, ordering, authority, or
  provenance;
- it is generic before there are real variants;
- it hides uncertainty instead of exposing warnings or gaps.

Stop expanding a file when it starts mixing layers. Line count is a useful smell
but responsibility mixing is the correctness risk.

Hard stop signs:

- one file contains record loading, semantic joining, causal ordering, warning
  production, and rendering;
- helper names grow long because the file has no local module context;
- several helper clusters pass the same arguments around;
- adding one feature requires navigating unrelated command, rendering, state,
  and IO code;
- tests cannot target the semantic object without invoking CLI or filesystem
  behavior;
- the file owns more than one of record IO, indexing, playback construction,
  rendering, and command dispatch.

Preferred module direction:

```text
ploke-records/src/playback/types.rs      // RunPlayback, RunPlaybackRef, granularity traits
ploke-records/src/playback/order.rs      // PlaybackOrder, phase ordering
ploke-records/src/playback/warning.rs    // timestamp/join/authority warnings
ploke-tree/src/playback/index.rs         // loaded record index
ploke-tree/src/playback/build.rs         // joins records into playback
ploke-tree/src/playback/coarse.rs        // coarse projection
ploke-tree/src/playback/fine.rs          // fine projection
ploke-eval/src/cli/replay.rs             // command dispatch/rendering only
```

Abstraction is justified by repeated semantic responsibility, not repeated
syntax. Modularity is forced by mixed authority/projection/rendering layers, not
by line count alone.

## Drift Review Triggers

Do not run heavyweight reviews on a rigid schedule. Pause for review when the
code starts compensating for a weak model.

Trigger a short drift review when any of these symptoms appear:

- a file keeps growing because new behavior has nowhere better to live;
- names repeat the same structural nouns in different orders;
- several types share the same words because a missing carrier should represent
  the relation directly;
- tests only assert construction, serialization, or formatting rather than the
  semantic invariant;
- tests require large unrelated fixtures to prove a small rule;
- a test failure is being patched around instead of questioning the model;
- no bounded real-run check has been run after several structural changes;
- the implementation starts using scheduler/projection data because it is
  convenient;
- UI or CLI rendering starts shaping the playback model;
- an agent report says "probably" or "seems" around authority, ordering, or join
  claims;
- the next change cannot be explained in terms of this plan's stop conditions.

Drift review questions:

```text
What semantic object are we implementing?
What invariant is this slice supposed to prove?
Which file/type is absorbing too much responsibility?
Which names indicate missing structure?
What is the smallest live-run check that reconnects this to reality?
Should we split, delete, or stop?
```

The review outcome should be one of: continue with the current slice, split a
module, promote a missing carrier, delete/defer an ad hoc type, or stop and
record the gap.

## Incorrectness Criteria

Treat these as correctness failures, not cosmetic issues:

- Authority error: a projection/control file determines selection, handoff, or
  successor truth instead of sealed History or an appropriate authoritative
  record.
- Ordering error: playback is ordered by timestamp, path order, file mtime, or
  incidental load order instead of History/journal/runtime causal order.
- Join error: evaluation, branch, child, runtime, or artifact evidence is
  attached by weak keys such as generation alone.
- Completeness error: required causal evidence is missing but the projection
  silently presents a complete step.
- Granularity error: a coarse step claims fine detail it does not contain.
- Timestamp consistency error: timestamps contradict causal order and no warning
  is emitted.
- UI/data-shape error: a front end requires a flattened format that differs from
  persisted typed records instead of using playback projections.
- Real-run error: synthetic tests pass but the last live run cannot produce the
  expected sealed chain or exposes missing joins.

Validation rule: History/journal causality decides correctness; timestamps and
projections audit it; UI convenience never defines it.

## No-Goals

- Do not parse rendered CLI output.
- Do not make scheduler or mutable projection records active authority.
- Do not move write authority into `ploke-records`.
- Do not introduce a UI-only replay format that differs from persisted record
  shape.
- Do not promise live observability fields until the typed records and join keys
  exist.

## Open Questions

- Which replay-relevant DTOs should gain `RecordFamily` first?
- Should `RunPlaybackRef` store a `Vec<StepRef<'a>>`, an indexed iterator over a
  record store, or both?
- Should coarse playback be derived directly from sealed History first, with
  scheduler/evaluation data attached as optional context?
- What graph renderer should the front-facing crate use: egui-native graph
  widgets, a small graph layout crate, or a separate visualization layer for
  WebAssembly?
- How should live updates enter the model: filesystem polling, append-only
  record stream, or a `ploke-tree` watch/index refresh layer?
