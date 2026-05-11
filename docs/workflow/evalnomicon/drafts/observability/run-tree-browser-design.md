# Prototype 1 Run Tree Browser Design

Status: draft, 2026-05-08.

## Purpose

Define a shared passive record crate and a read-only tree projection for
browsing Prototype 1 run progress as a tree of runtime, artifact, child,
successor, and History nodes. The first renderer may be `egui`/`eframe`
compiled to WebAssembly, but the durable design object is the record/projection
boundary, not the UI toolkit.

The design starts from
`crates/ploke-eval/src/cli/prototype1_state/PROTOTYPE1_LOOP_OPERATOR.md`. That
operator appendix is a useful map of current files and traits. Two points must
be sharpened for a browser:

- `nodes/<node-id>/successor-ready/<runtime-id>.json` and
  `successor-completion/<runtime-id>.json` are defined typed paths and preview
  import surfaces, but current live successor liveness is carried through
  channel envelopes and journal records. A browser must not assume those JSON
  files exist.
- History/Crown remains the authority boundary. Channel envelopes are
  first-class live transport for successor readiness/completion, while the
  transition journal is a durable replay/projection surface.

## Design Claim

This is not a GUI over CLI output. It is a typed, provenance-preserving
projection over existing Prototype 1 records.

The proposed split is:

- `ploke-records`: shared recording and report schemas for persisted Prototype 1
  files and loop-command outputs. Public record/projection types should derive
  `Serialize` and `Deserialize`, carry fields and format versions, and have no
  authority to open, admit, seal, append, select, or advance the loop. Public
  record fields should be typed domain fields, not `serde_json::Value`,
  `JsonRecordValue`, or equivalent opaque JSON placeholders.
- `ploke-eval`: runtime and authority crate. It may wrap `ploke-records` values
  in typestate carriers such as `Parent<S>`, `Crown<S>`, `Block<S>`, and
  `Entry<S>`, and it owns the transition methods that turn records/evidence into
  authority.
- `ploke-tree`: read-only tree/projection crate that loads `ploke-records`
  values and produces `RunForest` snapshots for operator interfaces.
- `ploke-tree-browser`: optional `egui`/`eframe` renderer that consumes those
  snapshots natively or through HTTP/JSON when compiled to WebAssembly.

`ploke-tui` should not be the browser substrate. It is a terminal application
with `ratatui`, `crossterm`, clipboard, native request, and full-runtime
assumptions that make browser compilation a separate migration problem.

## Record vs Authority

The split must preserve this distinction:

- a **record** is a passive persisted schema that can be serialized,
  deserialized, displayed, diffed, and transported;
- an **authority carrier** is a private-state wrapper or transition result that
  proves the record has passed the checks required for the loop to rely on it.

For example, `ploke-records` may expose a `SealedBlockRecord` that the UI can
deserialize. `ploke-eval` should expose a separate `Block<block::Sealed>` that
contains or borrows that record only after verification:

```rust
pub struct SealedBlockRecord {
    pub header: SealedBlockHeaderRecord,
    pub entries: Vec<AdmittedEntryRecord>,
}

pub struct Block<S> {
    record: BlockRecord,
    state: S,
}

impl Block<block::Sealed> {
    pub fn verify_from_record(record: SealedBlockRecord) -> Result<Self, HistoryError> {
        // hash, entry-root, lineage, and schema checks happen here
        Ok(Self {
            record: BlockRecord::Sealed(record),
            state: block::Sealed::new(),
        })
    }
}
```

The exact API can differ, but the invariant should not: deserializing a record
does not grant `Block<Sealed>`, `Crown<Locked>`, `Parent<Ready>`, or append
authority. The transition into an authority carrier remains in `ploke-eval`.

This means `Block<S>` and `Entry<S>` should not move wholesale into
`ploke-records` in the first implementation. They are not just schemas; they are
the local carriers for admissible History transitions. Their private fields,
state markers, constructors, and move-only methods are part of the correctness
boundary. What can move is the passive data they project to or hydrate from:
headers, entry payloads, hashes, lineage refs, schema versions, timestamps, and
other serialized fields.

The intended shape is therefore:

- `ploke-records::history::BlockRecord` and related record structs describe the
  persisted bytes;
- `ploke-eval::...::history::Block<S>` owns the verified/open/sealed state and
  may contain a `BlockRecord`;
- `ploke-records::history::EntryRecord` and related record structs describe
  persisted entry payloads;
- `ploke-eval::...::history::Entry<S>` owns draft/observed/proposed/admitted
  state and the transition methods that produce admitted entries.

If a current `Block` or `Entry` field is only a serialized fact, it is a
candidate to move into a record. If a field or method controls whether an entry
may be admitted, a block may be sealed, or a lineage head may advance, it stays
inside `ploke-eval` behind the authority carrier.

Mirror records are allowed when the UI needs to read something that corresponds
to a typestate carrier. A mirror record is the recorded shape of a carrier, not
the carrier itself. For example, a `SealedBlockRecord`, `AdmittedEntryRecord`, or
`ParentRuntimeRecord` may expose the fields needed by reports and browser views,
but deserializing one only produces recorded data. It does not recreate the
functional typestate value unless `ploke-eval` explicitly verifies it and wraps
it back into the appropriate authority carrier.

Mirror records must still be typed. They are not permission to store untyped JSON
subtrees in `ploke-records`. If a persisted value matters to the tree UI, its
fields should be modeled in the shared record crate. If the source type cannot
move because it also carries private constructors, typestate, or runtime
authority, add a public typed mirror with the same serialized facts and leave
the authority-bearing transition in `ploke-eval`.

For producer normalization, `ploke-records` may define record metadata such as a
record family, schema version, and wire format. `ploke-eval` should own the
emission trait or store adapter, for example an `EmitRecord<R>` implementation
over `R: ploke_records::Record`. That keeps write authority with the runtime
crate while making every UI-facing persisted object structurally searchable as a
typed shared schema.

The target user-facing flow is:

1. `ploke-eval` loop commands continue to run the authoritative Prototype 1
   process and persist or emit typed records.
2. Those persisted files and command outputs use types defined in
   `ploke-records`.
3. `ploke-tree` reads those shared types and assembles a friendly view of the
   ongoing multi-generational loop.
4. The browser or native UI depends on `ploke-tree` and `ploke-records`, not on
   `ploke-eval`, while still having typed access to all persisted data that
   `ploke-eval` intentionally exposes for reporting.

## Source Boundaries

`ploke-records` should contain passive source schemas. `ploke-tree` should load
those records through typed or deliberately labeled read boundaries:

| Source | Shared record shape | Use in tree |
|--------|---------------|-------------|
| Campaign manifest | `campaign.json` next to `prototype1/` | run anchor and configured campaign id |
| Parent checkout | `.ploke/prototype1/parent_identity.json` | active parent artifact identity |
| Scheduler | `Prototype1SchedulerState` / `Prototype1NodeRecord` | search-wave nodes, frontier, completed and failed sets |
| Branch registry | `Prototype1BranchRegistry` | branch status and selected branch projections |
| Transition journal | `JournalEntry` via `RecordStore` | durable operational replay: parent start, spawn, observe, selection, resource samples |
| Runtime channel | channel envelope records | live successor ready/completion and parent-child message evidence |
| History | block and entry records | admitted lineage facts and sealed block heads, labeled raw or verified |
| Child evidence | `FsEvidenceStore` / `ChildEvidenceSet` | typed joins over node, invocation, result, evaluation, and successor evidence |
| Metrics and scores | read-only projections | derived operator views with source refs |
| Observation logs | tracing/full-response/provider logs | optional degraded evidence with explicit attribution warnings |

`ploke-tree` must not parse human-oriented monitor text. It may reuse current
builders after their passive record inputs are available from `ploke-records`,
or it may move shared projection code out of `cli_facing.rs` into a library
module. `ploke-tree` should never call `ploke-eval` authority constructors just
to render a UI.

Loop-facing `ploke-eval` commands should prefer emitting `ploke-records` values
as JSON when they need to expose active run state for another interface. Human
text remains a renderer. It should not be the interchange format for the tree
browser.

## Projection Model

The core snapshot should be a forest because Prototype 1 has at least three
tree-like axes that should not be flattened into one id:

- search expansion: scheduler node parentage and generation;
- runtime succession: Parent, Child, Successor, and Runtime attempts;
- authority lineage: sealed History block height and lineage head.

Suggested public DTOs:

```rust
pub struct RunForest {
    pub schema_version: String,
    pub generated_at: String,
    pub campaign: CampaignRef,
    pub roots: Vec<TreeNode>,
    pub lanes: Lanes,
    pub diagnostics: Vec<Diagnostic>,
}

pub struct TreeNode {
    pub id: NodeKey,
    pub parent: Option<NodeKey>,
    pub label: String,
    pub kind: NodeKind,
    pub progress: Progress,
    pub authority: AuthorityLabel,
    pub refs: Vec<EvidenceRef>,
    pub children: Vec<NodeKey>,
}

pub enum NodeKind {
    Campaign,
    Lineage,
    HistoryBlock,
    ParentRuntime,
    Candidate,
    ChildRuntime,
    SuccessorRuntime,
    Artifact,
    Branch,
}

pub enum AuthorityLabel {
    SealedHistory,
    TypedEvidence,
    MutableProjection,
    LiveTransport,
    DegradedObservation,
}
```

`Progress` should be a small value object, not a status string bag. It can carry
phase, terminality, result class, timestamps, and counts. The UI can then render
consistent glyphs/colors without inventing semantics. When `ploke-tree` has only
raw records, authority labels should say so; when `ploke-eval` exports a
verified snapshot, the corresponding nodes can be labeled as verified.

## Data API

Minimum API for `ploke-tree`:

```rust
pub trait RunStore {
    type Error;

    fn list_campaigns(&self) -> Result<Vec<CampaignRef>, Self::Error>;
    fn load_forest(&self, campaign: &CampaignRef) -> Result<RunForest, Self::Error>;
    fn load_node(&self, key: &NodeKey) -> Result<TreeNodeDetail, Self::Error>;
}

pub trait WatchRuns {
    type Error;
    type Event;

    fn poll_changes(&mut self) -> Result<Vec<Self::Event>, Self::Error>;
}
```

Native implementations can read the filesystem directly. Browser builds should
consume exported snapshots or a local read-only service:

- native: `FsRunStore` loads from `$PLOKE_EVAL_HOME` and a repo-root checkout;
- wasm static: `HttpSnapshotStore` fetches `run-forest.json`;
- wasm live: `HttpSnapshotStore` polls `/campaigns/<id>/forest` and optional
  `/events` Server-Sent Events.

The service/export layer is projection transport only. It must not write
campaign state, modify the parent checkout, or advance History.

`ploke-records` should be simpler: public serde structs, schema version
constants, stable field names, and small helpers for non-authoritative parsing.
Its public data-carrying types should derive `Serialize` and `Deserialize`.
It should not expose `verify_from_record`, `seal`, `append`, or transition
methods. Those belong to `ploke-eval`.

## Assembly Rules

1. Build the base search tree from scheduler nodes when available.
2. Attach typed child evidence by `node_id`, `branch_id`, and `runtime_id`.
3. Attach channel-derived live liveness only as `LiveTransport`.
4. Attach journal records as replay/projection facts with source refs.
5. Attach History block records as lineage nodes, separate from scheduler
   generation. If they were only deserialized by `ploke-tree`, label them raw or
   locally checked; if they came from a `ploke-eval` verified export, label them
   verified.
6. Attach score/metrics rows as derived lanes with derivation ids.
7. Preserve unplaced evidence and join conflicts as diagnostics rather than
   first/last-writer-wins behavior.

The implementation should fail closed on malformed declared records crossing a
typed record boundary. Degraded observation logs may be best-effort, but their
nodes must carry `AuthorityLabel::DegradedObservation`.

## Browser UI

`egui`/`eframe` is the most plausible Rust-first browser renderer because it can
run native and wasm from the same widget code. The first usable interface should
be operational, not decorative:

- left pane: campaign/run selector and filters by generation, authority label,
  terminal state, selected successor, and diagnostics;
- center: expandable run tree with stable row heights, phase icons, timestamps,
  and progress counters;
- right pane: selected node detail, evidence refs, source files, diagnostics,
  and related History block or scheduler node;
- bottom lane: timeline of parent, child, successor, selection, and History
  events.

The browser renderer should never read raw Prototype 1 files directly. It should
consume `RunForest` snapshots or record bundles produced by `ploke-tree` or a
read-only local service. In wasm, the file system and home directory are not
available in the same way; pushing a snapshot or polling a local endpoint keeps
that constraint explicit.

Alternative renderer: a TypeScript/D3 or Svelte frontend over the same snapshot
JSON may be faster for graph layout, but it should not replace the Rust data
crate. The stable contract is `RunForest`, not `egui`.

## Crate Layout

Recommended first slice:

```text
crates/ploke-records/
  Cargo.toml
  src/lib.rs
  src/history.rs
  src/journal.rs
  src/scheduler.rs
  src/channel.rs
  src/invocation.rs
  src/identity.rs
  src/metrics.rs

crates/ploke-tree/
  Cargo.toml
  src/lib.rs
  src/model.rs
  src/store/fs.rs
  src/assemble.rs
  src/export.rs

crates/ploke-tree-browser/
  Cargo.toml
  src/lib.rs
  src/app.rs
  src/native.rs
  src/wasm.rs
```

For the first implementation, `ploke-records` should not depend on `ploke-eval`.
Instead, `ploke-eval` depends on `ploke-records` and wraps record structs in its
functional typestate carriers. `ploke-tree` also depends on `ploke-records`, but
not on `ploke-eval`.

Avoid adding generic `*Report`, `*Info`, or `*Status` carriers as authority
objects. Reports are downstream projections from `RunForest`, `ChildEvidenceSet`,
`ScoreSet`, or sealed History facts.

Avoid hand-maintained field duplication where possible. Prefer moving the
persisted fields into `ploke-records` and changing authority types in
`ploke-eval` to contain records plus private state markers. Where a current
functional type has extra live-only fields, split those into runtime context
types in `ploke-eval` rather than putting them into the persisted record.

## Implementation Sequence

1. Add `ploke-records` with passive record structs for the smallest useful
   subset: scheduler nodes, parent identity, journal entries, channel envelopes,
   invocation/result records, and passive History block/entry record shapes.
   Do not move `Block<S>` or `Entry<S>` themselves in this step.
2. Refactor `ploke-eval` authority types to contain or convert from
   `ploke-records` structs without making authority constructors public. For
   History, this means adapting `Block<S>` and `Entry<S>` around passive records,
   not exporting their typestate transitions to the shared crate.
3. Add `ploke-tree` with `RunForest` DTOs and `FsRunStore` over
   `ploke-records`.
4. Add snapshot export JSON and a small golden fixture from synthetic records.
5. Add a native `egui` prototype that loads one exported snapshot.
6. Add wasm build using `eframe` and HTTP snapshot loading.
7. Add optional live polling/SSE after the static snapshot contract is stable.

## Open Questions

- Which current `ploke-eval` structs are already pure persisted records and can
  move to `ploke-records` first without changing behavior?
- Should `ploke-records` include only raw schemas, or also non-authoritative
  local consistency checks such as schema-version and hash-shape validation?
- Should channel envelopes be folded into the same `RunForest` snapshot, or into
  a short-lived liveness overlay refreshed independently?
- What is the first browser deployment target: local `trunk serve`, static file
  plus JSON export, or a read-only local HTTP server?
- How much of provider/full-response observation belongs in the first tree, given
  the current attribution bridge can misattribute multi-campaign logs?
