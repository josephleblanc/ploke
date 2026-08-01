# Prototype 1 Persistence Port Map

Status: active implementation guardrail / trait-boundary map.

Related files:

- [`storage-plan.md`](storage-plan.md)
- [`implementation-plan.md`](implementation-plan.md)
- [`typestate-persistence-ledger.md`](typestate-persistence-ledger.md)
- [`transition-persistence-dependency-matrix.md`](transition-persistence-dependency-matrix.md)
- [`live-transition-test-plan.md`](live-transition-test-plan.md)

## Purpose

`EvalStore` is intentionally **not** the abstraction for every persisted file in Prototype 1.

It covers Domain-C ordinary eval evidence/projections/refs. The broader migration still has to track History, channels, message boxes, bootstrap files, artifact/worktree state, run profiles, transition journals, logs, snapshots, and compatibility mirrors. Some already have typed ports; some need future ports; some should remain file-backed authority for now but may be mirrored into `EvalStore` for query.

This map prevents the implementation from narrowing the plan to only `EvalStore` and accidentally dropping the broader persisted-file surfaces.

## Dispatch pattern

Use the same high-level pattern for all backend-able domains:

1. **Domain-specific port** expresses the semantic authority or evidence contract.
2. **Filesystem implementation** preserves the current behavior first.
3. **Configured wrapper** is introduced at runtime boundaries only when a domain becomes configurable.
4. **Backend-specific implementations** use static dispatch and focused tests.
5. **High-level callsites** depend on the domain port or configured wrapper, not on raw paths or Cozo scripts.

Do not create one global `PersistenceStore` trait. The authority contracts differ too much.

## Port inventory

| Domain / persisted surface | Current source shape | Existing or planned port | First file/db migration status | Tests / guardrails |
| --- | --- | --- | --- | --- |
| Sealed History blocks, head/index projections | `FsBlockStore` under `prototype1/history/*`; source has `BlockStore` in `history/stored/mod.rs` | Existing `BlockStore`; future `HistoryStore`/distributed block store may refine it | **Not `EvalStore`.** Keep file-backed authority during first pass. Eval rows may cite block refs only. | History seal/verify/hash/state-root tests remain decisive. DB row present but invalid/missing History block must fail successor startup. |
| Crown / lineage authority | Typestate carriers in `inner.rs`; History seal append through `BlockStore` | Crown/History typestate, not a storage record trait | Not part of Domain-C migration | Handoff must cross `Parent<Selectable> -> Parent<Retired>` and append sealed block before successor spawn. |
| Parent/child and parent/successor channels | `Channel<R, T: Transport>` with `FileTransport` in `channel.rs` | Existing `Transport` trait | **Not `EvalStore`.** Channel envelopes may be mirrored as `eval_channel_message` / receipt rows later. | Parent readiness/result/SuccessorReady must come from channel transport. DB mirror cannot satisfy missing/invalid envelope. |
| Message boxes / child-plan lock-unlock file | `Message`, `MessageBox`, `Open<M>`, `Locked<M>`, `Received<M>` in `inner.rs`; child plan file under `prototype1/messages/child-plan/` | Existing `MessageBox` type contract; possible future `MessageBoxBacking` for non-file boxes | **Not generic `EvalStore`.** Payload may be mirrored after `Received<ChildPlan>`. | Lock/unlock path/body/parent/generation validation remains decisive. DB payload row cannot replace the box. |
| Artifact/worktree/active checkout mutation | `WorkspaceBackend` with `GitWorktreeBackend` in `backend/` | Existing `WorkspaceBackend` | **Not `EvalStore`.** Store only artifact refs/surfaces/evidence in eval rows. | Active checkout, worktree, tree key, surface, and commit validation remain backend authority. DB row cannot install or prove artifact alone. |
| Parent identity in active checkout | `.ploke/prototype1/parent_identity.json`, committed via workspace backend | Artifact/backend state plus parent identity loader | Keep file/artifact-backed in first pass; optional query mirror later | Startup and successor validation load identity from artifact/checkout and History, not DB mirror. |
| Run profile / profile commitment / campaign config | `run-profile.toml`, `run-profile.commitment.json`, campaign manifest/config | Planned config/admission surface; may remain direct profile loader initially | Add storage backend selector here, but do not turn profile admission into `EvalStore` | Profile parsing/default/validation/commitment tests. Scheduler fallback must not override admitted profile. |
| Transition journal | `PrototypeJournal` JSONL at `prototype1/transition-journal.jsonl` | Current append stream; possible future `TransitionJournal`/`JournalSink` port | Preserve JSONL replay semantics. Selected entries may mirror into `EvalStore`. | Replay/walk/metrics still work from JSONL while migration proceeds. DB row cannot hide missing journal entry when consumers require it. |
| Ordinary eval records/projections/refs | `node.json`, runner request/result, branch/evaluation records, parent reports, resource samples, structured observations | Planned `EvalStore` | **Primary target** for `fs | database | dual-strict` first pass | Producer/consumer dependency tests, dual-strict row/file parity, common scope/source/evidence/hash validation. |
| Invocation/bootstrap files | `nodes/<node>/invocations/<runtime>.json`; child/successor executable bootstrap | Planned `BootstrapPackage` / `InvocationTransport` or `InvocationStore` port; may be mirrored by `EvalStore` | Keep executable file/bootstrap behavior first. Mirror only for query. | Child/successor process must be launchable from bootstrap; DB mirror cannot replace invocation delivery. |
| Logs, stdout/stderr streams, full LLM responses | stream files, tracing logs, response sidecars | Planned `LogRefStore` / `BlobRefStore`; large blobs maybe object store later | First pass should store refs/hashes, not large payloads inline | Hash/ref verification, sensitivity/redaction labels, no silent missing logs where lifecycle expects them. |
| Treatment run DBs/snapshots | run-local `indexing-checkpoint.db`, `final-snapshot.db`, run artifacts | Planned `EvalSnapshotStore` / child-local store; not parent ambient DB | Child/treatment-local by default. Parent imports explicit refs/evidence only. | Parent-visible facts must cross channel/import edge; child code graph facts do not merge into parent scope by default. |
| Passive record mirror | `records/mirror.cozo.sqlite` relation `prototype1_record` from `JsonRecordFile::emit` | Compatibility/debug mirror only | Not `DbEvalStore`; may help backfill/comparison | Mirror lacks owner/runtime/artifact/import authority context. Do not use as proof of typed DB parity. |
| Code graph overlays | future `eval_code_*` relations | Follow-on query/overlay traits, not first storage pass | Deferred until file/db parity for ordinary evidence | Historical joins require snapshot/as-of/hash coordinates; no base code graph mutation. |

## Runtime composition shape

Long term, the parent loop should not receive raw paths as its only persistence boundary. It should receive or construct a small bundle of domain ports.

Conceptual shape:

```rust
struct Prototype1Ports {
    eval: ConfiguredEvalStore,
    history: FsBlockStore,              // later: ConfiguredBlockStore
    channel: FileTransport,             // later: ConfiguredTransport
    workspace: GitWorktreeBackend,      // existing WorkspaceBackend impl
    // future: bootstrap, message_box_backing, log/blob refs, snapshot store
}
```

Do not implement this full struct first if it causes churn. The initial implementation can introduce only `ConfiguredEvalStore`, while this map keeps the other domains visible and tested.

## Implementation sequencing implication

The first `fs | database | dual-strict` implementation slice is allowed to touch only the `EvalStore` row of this map.

However, every slice must still run tests for the dependent broader domains listed in [`transition-persistence-dependency-matrix.md`](transition-persistence-dependency-matrix.md). For example:

- changing child-plan payload persistence requires MessageBox lock/unlock tests;
- changing channel-result mirrors requires channel transport tests;
- changing selection/evaluation rows requires History/handoff negative tests if those rows are cited by sealed material;
- changing artifact refs requires workspace/backend validation tests.

## Existing code anchors

- `crates/ploke-eval/src/cli/prototype1_state/history/stored/mod.rs` — `BlockStore`, `FsBlockStore`.
- `crates/ploke-eval/src/cli/prototype1_state/channel.rs` — `Transport`, `FileTransport`, role-indexed `Channel`.
- `crates/ploke-eval/src/cli/prototype1_state/inner.rs` — `File`, `MessageBox`, `Message`, `Open`, `Locked`, `Received`, Crown carriers.
- `crates/ploke-eval/src/cli/prototype1_state/backend/mod.rs` — `WorkspaceBackend`.
- `crates/ploke-eval/src/cli/prototype1_state/profile.rs` — run profile/config storage shape.
- `crates/ploke-eval/src/cli/prototype1_state/journal.rs` — transition journal append/replay surface.
- `crates/ploke-eval/src/record_emission.rs` — current passive record mirror and JSON emission choke point.

## Open implementation questions

- Should `Prototype1Ports` be introduced early as a test harness shape, or only after `ConfiguredEvalStore` is proven?
- Should `TransitionJournal` become a formal port before or after first `EvalStore` slice?
- Does MessageBox need a `MessageBoxBacking` trait before remote execution, or can current file-backed `MessageBox` remain until after DB parity?
- Should invocation/bootstrap be its own port before DB-only mode is considered?
- Which log/blob refs belong in `EvalStore` directly versus a separate `BlobStore` with only refs in eval rows?
