# Prototype 1 Eval Store — Authority Boundaries and Backend Model

Status: active boundary/backend plan; implementation sequencing lives in [`implementation-plan.md`](implementation-plan.md).

Short description: authority-boundary plan for making Prototype 1 ordinary eval evidence configurable between the current filesystem layout and owner-scoped Cozo-backed `ploke_db::Database` handles. This file explains what `EvalStore` may own and what must remain under History, Channel, MessageBox, bootstrap, journal, artifact/worktree, log/blob, or snapshot-specific ports.

Related planning files:

- [`implementation-plan.md`](implementation-plan.md)
- [`persistence-port-map.md`](persistence-port-map.md)
- [`typestate-persistence-ledger.md`](typestate-persistence-ledger.md)
- [`relational-data-model.md`](relational-data-model.md)
- [`codegraph-eval-join-model.md`](codegraph-eval-join-model.md)
- [`crates/ploke-eval/docs/prototype1/persistence-inventory-and-cozo-map.md`](../../../../crates/ploke-eval/docs/prototype1/persistence-inventory-and-cozo-map.md)
- [`crates/ploke-eval/docs/reference/persisted-files.md`](../../../../crates/ploke-eval/docs/reference/persisted-files.md)
- [`crates/ploke-eval/docs/prototype1/typestate-invariants.md`](../../../../crates/ploke-eval/docs/prototype1/typestate-invariants.md)
- [`docs/workflow/evalnomicon/src/prototype1/artifact-runtime-model.md`](../../../workflow/evalnomicon/src/prototype1/artifact-runtime-model.md)
- [`docs/workflow/evalnomicon/src/prototype1/runtime-authority.md`](../../../workflow/evalnomicon/src/prototype1/runtime-authority.md)
- [`docs/workflow/evalnomicon/src/prototype1/history-crown.md`](../../../workflow/evalnomicon/src/prototype1/history-crown.md)
- [`docs/workflow/evalnomicon/src/prototype1/persistence-and-observability.md`](../../../workflow/evalnomicon/src/prototype1/persistence-and-observability.md)
- [`docs/workflow/evalnomicon/drafts/runtime/parent-child-channel.md`](../../../workflow/evalnomicon/drafts/runtime/parent-child-channel.md)
- [`docs/active/agents/2026-05-15_prototype1-record-pipeline-map.md`](../2026-05-15_prototype1-record-pipeline-map.md)
- [`docs/active/agents/2026-06-09_prototype1-state-api-surface-refactor-proposal.md`](../2026-06-09_prototype1-state-api-surface-refactor-proposal.md)

## User intent captured

The goal is **not** to first migrate sealed History or runtime channels into a database.

See [`persistence-port-map.md`](persistence-port-map.md) for the broader implementation trait/port map. `EvalStore` is only the Domain-C port; other persisted surfaces are tracked under existing or future domain-specific ports.

The desired split is:

1. Keep `History` / sealed `Block` semantics separate.
2. Keep `Channel<R, T: Transport>` semantics separate.
3. Move the remaining Prototype 1 persisted records/projections/evidence/log references into a Cozo-backed `ploke_db::Database` at the relevant owner scope. For parent-owned evidence, that means the same database handle used for parent code graph queries.

The motivation for using the relevant code-graph `Database` is that eval evidence can later be joined to code graph facts without losing artifact/runtime ownership. For example, the loop could label or group code item nodes as immutable/mutable, or attach policy/evidence overlays to code nodes. That enables finer-grained patch permissions, protected regions, and queryable evidence about which code items were cited, edited, failed, proposed, or selected.

First-pass boundary: the first implementation pass should not require code-graph overlay relations. It should prove configurable `fs | database | dual-strict` storage for ordinary eval evidence, while choosing scopes/ids that will not block later code graph joins.

## Remote-runtime and shared-filesystem assumption

Current Prototype 1 execution assumes a shared filesystem. Parent, child, and successor processes usually see the same `$PLOKE_EVAL_HOME` campaign tree, and parent-controlled paths are passed through invocation records, channel endpoint roots, and direct file paths. That is an implementation convenience, not the intended long-term authority model.

Future runs should be able to place parent and child runtimes on different machines, VMs, or size-constrained devices. In that shape, the child may have a local checkout, local database, local embeddings, and local diagnostics that the parent cannot see directly. Cross-runtime visibility must then flow through explicit protocol surfaces:

- a typed `MessageBox` lock/unlock edge, such as the current child-plan box;
- a role-indexed `Channel<R, T: Transport>` message or verifiable reference;
- sealed History material and successor validation bundles;
- an explicit import/admission edge that names the producing runtime/artifact and validates hashes or identities.

Do not design the database backend as if every runtime can safely write to one ambient shared database. The database handle used by an `EvalStore` has explicit store scope/visibility, and cross-runtime movement of records must be explicit.

## Current file visibility categories

| Category | Current filesystem location | Current producer/consumer | Remote-runtime implication |
| --- | --- | --- | --- |
| Parent-owned campaign state | `campaign.json`, `prototype1/run-profile.*`, `scheduler.json`, `branches.json`, `transition-journal.jsonl`, `evaluations/*`, `history/*` | Parent writes/reads; successor validates selected History material. | Parent store owns these. Remote successors need an admitted handoff bundle or replicated parent store view, not ambient path access. |
| Child-plan message box | `prototype1/messages/child-plan/<parent-node-id>.json` | Parent locks and unlocks before child runtimes exist. | This is a typed box, not a copied child-worktree file. A remote backend must preserve lock/unlock semantics, not merely move JSON. |
| Child invocation/bootstrap | `nodes/<node>/invocations/<runtime>.json` | Parent writes; child reads to learn role, campaign/node/runtime ids, journal path, channel root, request, and resolved branch. | Remote execution needs a bootstrap transport or launch package containing the same facts. The file path alone is not authority. |
| Runtime channel | `nodes/<node>/channels/<runtime>/{parent-to-child,child-to-parent}.jsonl` | Child sends `Ready`, `Evaluating`, terminal `Result`; parent observes. Successor sends `SuccessorReady`. | Replaceable by another `Transport`. Parent decisions should depend on channel messages or channel-carried verifiable refs, not shared result files alone. |
| Child workspace/artifact | `nodes/<node>/worktree/` or broad harness workspace roots | Parent currently materializes/builds/spawns locally; child runs against that artifact. | Remote execution may require artifact packaging, git commit/tree transfer, or child-side checkout. Parent should not assume direct workspace reads except where the current FS backend says so. |
| Child-local eval DB and snapshots | treatment run `indexing-checkpoint.db`, `final-snapshot.db`, cached starting DBs | Child/treatment run initializes or restores a run-local code graph DB and writes snapshots/records. | Generally child-local. Do not import into parent graph by default; code item ids may refer to the child artifact, not the parent artifact. |
| Logs and diagnostic blobs | run dirs, stream logs, `~/.ploke-eval/logs`, full-response traces | Mixed producers; useful for replay/diagnosis. | Store as refs/hashes first. Import full blobs only under explicit policy and owner scope. |

## Persistence domains

### Domain A: sealed History authority

Keep separate for now.

Examples:

- `History`
- `Block<Sealed>`
- lineage state
- sealed block append authority
- `BlockHash`, `entries_root`, `HistoryStateRoot`
- `verify_hash()` semantics

Reason: this domain has independent authority, append, hash-preimage, and lineage invariants. It should not be blurred with general record storage.

### Domain B: runtime channel semantics

Keep separate for now.

Examples:

- `Channel<R, T: Transport>`
- `FileTransport`
- parent-to-child / child-to-parent transport
- runtime readiness and IPC handshakes

Reason: channels are transport/session semantics, not just persisted records. A future DB transport may exist, but that is a separate design from eval evidence storage.

### Domain B2: typed message boxes / lock-unlock buffers

Keep separate from generic eval-record storage.

Examples:

- `MessageBox`
- `Open<M>` / `Locked<M>` / `Received<M>`
- `ChildPlanFile`
- `LockChildPlan` / `UnlockChildPlan`

Reason: a message box is a typed access rule, not an arbitrary persisted record. The current child-plan file is a filesystem-backed implementation of `Box = (Lock transition, Unlock transition, File schema)`. A database backend may mirror the payload for queries, but it must not replace the lock/unlock authority unless it implements an equivalent message-box backing.

### Domain C: eval evidence / records / projections / diagnostics

This is the only domain targeted by the first `EvalStore` backend work.

Examples:

- parent-start/resource evidence, transition evidence, and ordinary report/projection evidence;
- compatibility refs for scheduler/node/latest-result files without promoting those projections to decision authority;
- runner request/result refs split by attempt/runtime and import boundary;
- child-plan payload mirrors after typed receipt, not the message-box authority itself;
- harness requests/results and branch/evaluation records;
- selected transition journal records mirrored as evidence while JSONL replay remains intact;
- structured tracing / `observe::Step` events;
- diagnostic log references, LLM response references, and hashes;
- future code graph overlay labels/edges after file/db parity.

This domain should be configurable between filesystem and database backends. In the first pass, focus on non-code-graph evidence rows and compatibility refs. Legacy projections such as `scheduler.json`, `node.json`, and latest `runner-result.json` stay low-strength refs or typed facts derived from stronger evidence; do not create authority-shaped scheduler/node tables.

## Backend model

Use a capability trait for Domain C, not a single global store abstraction for all Prototype 1 state.

The first implementation must be narrow: add only the method needed for the fixed `R4c -> R5` parent-start/resource evidence slice, as specified in [`implementation-plan.md`](implementation-plan.md) and [`database-planning-notes.md`](database-planning-notes.md). Do not begin with a generic `put_record` / `put_blob_ref` / `put_edge` API or with `JsonRecordFile::emit`; those are broader than the first proof point and make authority boundaries harder to review.

The trait should express eval evidence semantics, not files. Filesystem paths are one backend representation.

## Static and dynamic dispatch shape

Use static dispatch inside backend implementations and tests, plus a thin runtime-selected wrapper at loop boundaries.

Sketch:

```rust
enum ConfiguredEvalStore {
    Fs(FsEvalStore),
    Db(DbEvalStore<ploke_db::Database>),
    DualStrict(DualEvalStore),
}

impl EvalStore for ConfiguredEvalStore {
    // match and delegate
}
```

This keeps high-level callsites agnostic to the configured backend without spreading generic type parameters through the whole loop driver. Backend-specific helper functions can still be generic over the narrow `EvalStore` trait where useful.

## Cozo / Database handling

The database backend should be built around the existing `ploke_db::Database` abstraction, because the point is to join eval evidence to the code graph.

Sketch:

```rust
struct DbEvalStore<D> {
    db: Arc<D>,
    campaign_id: CampaignId,
}

trait EvalDb {
    fn run_eval_mut(&self, script: &str, params: Params) -> Result<Rows, DbError>;
    fn run_eval_immut(&self, script: &str, params: Params) -> Result<Rows, DbError>;
}

impl EvalDb for ploke_db::Database {
    // delegate to existing Cozo execution methods
}
```

The backend should not treat Cozo as an unrelated passive mirror database. It should write eval relations into the same graph database handle used for code graph queries.

Open implementation detail: current code paths may use an in-memory `Database` restored from backups or a persistent SQLite-backed Cozo instance elsewhere. The store design should target the `Database` capability first, then decide how the configured loop run obtains and persists that database handle.

### Database ownership and visibility

`Database` must be treated as owner-scoped. At minimum, distinguish:

- parent runtime database / parent code graph;
- child runtime database over a child artifact;
- treatment run database and snapshots;
- cached starting database snapshots;
- passive record mirror database;
- future shared or replicated code-graph database.

Do not merge child code graph facts into the parent code graph merely because both are Cozo databases. A child may index changed files that are useful only inside the child artifact. Those code item ids may not exist in the parent artifact, and importing them as parent facts can create false joins.

Parent-owned overlays should store parent-relevant facts: policy labels, immutable/mutable/protected surface groups, candidate refs, channel evidence, selection evidence, artifact refs, and child-result summaries. Child-local overlays may store child-only embeddings and diagnostics. When a child becomes the selected successor, the successor must either inherit, recompute, or validate the admitted policy overlays needed for safe editing under the new artifact.

A useful split:

- **local child facts:** embeddings and code graph nodes for child-only changed files; usually stay child-local unless the artifact is selected or explicitly imported;
- **cross-runtime evidence:** terminal result payloads, treatment metrics, artifact refs, hashes, selected candidate membership, and diagnostics refs; move via Channel/import edges;
- **policy overlays:** immutable/mutable/protected code groups and allowed edit-surface grants; must be available to parent and to any admitted successor runtime;
- **History/seal facts:** selected artifact, successor identity, surface commitments, and admitted evidence hashes; remain History/Crown domain.

## Filesystem backend

`FsEvalStore` should preserve the current on-disk behavior initially.

It may internally write the same JSON/TOML/JSONL files, but callsites should transition toward semantic store methods/envelopes instead of ad hoc path writes.

This lets `backend = "fs"` remain the default while DB parity is built.

## Database relations: initial overlay shape

The database should use eval-specific overlay relations keyed by campaign/run/node/runtime ids. First-pass relations should be ordinary eval evidence/projection/ref rows, not code-graph overlay rows.

Potential first-pass relation families are defined canonically in [`relational-data-model.md`](relational-data-model.md) and sliced physically in [`database-planning-notes.md`](database-planning-notes.md). The near-term families are:

```text
eval_transition_event      -- first slice: R4c -> R5 parent-start/resource evidence
eval_trace_event           -- structured observe/log events
eval_record_ref            -- legacy/small record refs, not legacy authority
eval_log_ref / eval_blob_ref
eval_runtime / eval_attempt
eval_invocation
eval_channel_message / eval_channel_receipt / eval_import_event
eval_candidate_event
eval_evaluation / eval_selection_decision
```

Legacy projections such as scheduler/node/latest-result files may be retained through `eval_record_ref` or derived candidate/attempt events; do not create authority-shaped scheduler/node tables in the first pass.

Follow-on relation families, after file/db parity:

```text
eval_code_snapshot
eval_code_ref
eval_code_link
eval_code_label
eval_policy_surface / eval_policy_grant
eval_code_touch
eval_validation_cover
eval_retrieval_hit
```

Important design preference: keep base code graph facts distinct from eval/policy overlays unless an explicit migration says otherwise. For example, immutable/mutable classification can be represented as an overlay label/group relation rather than mutating the source ingest relations directly.

Conceptual code-graph edges are documented in [`codegraph-eval-join-model.md`](codegraph-eval-join-model.md), but are not part of the first storage migration pass.

## Config shape

Extend run profile storage config with a Domain C backend selector.

Possible TOML shape:

```toml
[storage.eval]
backend = "fs" # fs | database | dual-strict

[storage.eval.database]
target = "code-graph"
```

Possible backend modes:

- `fs`: current file layout remains authoritative for Domain C.
- `database`: Domain C records are written to the configured owner-scoped `Database` for that store.
- `dual-strict`: write both filesystem and database forms and fail on mismatch or DB write failure.

Default should remain `fs` until parity is proven.

## Migration boundary

The canonical phased sequence is [`implementation-plan.md`](implementation-plan.md). This file only records the boundary rules that sequence must preserve:

1. Establish the isolated live transition suite before moving writers behind `EvalStore`.
2. Keep `backend = fs` as the default and preserve current filesystem bytes/paths for migrated slices.
3. Move only the fixed first writer, `R4c -> R5` ParentStarted/resource evidence, behind the first narrow `EvalStore` method.
4. Add DB rows only for that slice, with common axes and deterministic envelope/hash comparison.
5. Enable `dual-strict` for that slice and fail loudly on DB write errors or semantic mismatches.
6. Expand later by dependency chain, using [`transition-persistence-dependency-matrix.md`](transition-persistence-dependency-matrix.md) to select downstream consumer tests.

Later slices may retain legacy files as `eval_record_ref` compatibility evidence, but should not add first-class authority-shaped relations for `scheduler.json`, `node.json`, latest `runner-result.json`, or passive mirror rows. Code graph overlay relations remain follow-on work after file/db parity for ordinary eval evidence.

## Things explicitly out of scope for this plan

- Replacing sealed History storage.
- Replacing `Channel<R, T: Transport>`.
- Replacing typed `MessageBox` lock/unlock semantics with generic record writes.
- Making Cozo the transport layer for parent/child messages.
- Assuming parent and child share a filesystem in the long-term design.
- Making child-local DB writes automatically visible in the parent database.
- Merging child artifact code graph facts into the parent graph without an explicit artifact/runtime/import relation.
- Weakening History hash, raw preimage, or `verify_hash()` semantics.
- Silently tolerating filesystem/DB schema drift during dual-write validation.

## Open refinement slots

Current unresolved questions are tracked in [`open-questions.md`](open-questions.md). Keep this file focused on authority and backend boundaries; do not add competing implementation sequences or relation catalogs here.
