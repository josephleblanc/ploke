# Worker 4: Prototype 1 History/Crown Persistence Map

Date: 2026-05-03

Scope: `crates/ploke-eval/src/cli/prototype1_state/history.rs`, `mod.rs`, and adjacent History/Crown persistence readers/writers. This report maps persisted History/Crown authority separately from preview, journal, and metrics projections.

## Authority Store

### Sealed History block stream

- Path pattern: `$(dirname <campaign.json>)/prototype1/history/blocks/segment-000000.jsonl`, where campaign manifests are under `~/.ploke-eval/campaigns/<campaign>/campaign.json` via `campaign_manifest_path` (`crates/ploke-eval/src/campaign.rs:233`).
- Persisted Rust type/schema: JSONL of `Block<block::Sealed>` (`history.rs:2724`) serialized as `{ entries: Vec<Entry<Admitted>>, state: block::Sealed { header: SealedBlockHeader, _private } }`; header fields are `BlockCommon`, `crown_lock_transition`, `selected_successor`, `active_artifact`, `claims`, `sealed_at`, `entry_count`, `entries_root`, `block_hash` (`history.rs:2652`).
- Writer: live handoff constructs `OpenBlock` in `handoff_block_fields` (`prototype1_process.rs:1421`), seals through `Parent<Selectable>::seal_block_with_artifact` (`inner.rs:197`) and `Crown<Locked>::seal` (`history.rs:3127`), then appends with `FsBlockStore::append` (`history.rs:831`). The call site is `history_store.append(&expected_state, &sealed_block)` (`prototype1_process.rs:1176`).
- Reader/CLI: startup predecessor admission reads the current sealed head with `FsBlockStore::sealed_head_block` (`parent.rs:539`) during `loop prototype1-state --handoff-invocation ...`; no `ploke-eval history ...` inspector currently reads the sealed block store. The `history preview` and `history metrics` commands read journals/documents instead.
- Key IDs for joins: `LineageId` is currently campaign id (`prototype1_process.rs:1430`); `BlockHash`, `block_height`, `parent_block_hashes`, `BlockId`, `HistoryStateRoot`, `selected_successor.runtime`, `selected_successor.artifact`, `active_artifact`, `TreeKeyHash` inside artifact claim.
- Evidence status: authoritative local History evidence for one configured filesystem store. It is tamper-evident by block hash and append checks, but local only.
- Safe bounded inspection:

```sh
wc -l ~/.ploke-eval/campaigns/<campaign>/prototype1/history/blocks/segment-000000.jsonl
jq -c '{height:.state.header.common.block_height,lineage:.state.header.common.lineage_id,hash:.state.header.block_hash,parents:.state.header.common.parent_block_hashes,entries:.state.header.entry_count,successor:.state.header.selected_successor}' ~/.ploke-eval/campaigns/<campaign>/prototype1/history/blocks/segment-000000.jsonl | sed -n '1,20p'
```

### History indexes and head projection

- Path patterns: `.../prototype1/history/index/by-hash.jsonl`, `.../prototype1/history/index/by-lineage-height.jsonl`, `.../prototype1/history/index/heads.json`.
- Persisted Rust type/schema: `StoredBlock { block_hash, lineage_id, block_height, location }` (`history.rs:918`), `LineageHeight { lineage_id, block_height, block_hash }` (`history.rs:1288`), and `BTreeMap<LineageId, BlockHash>` for `heads.json` (`history.rs:707`).
- Writer: `FsBlockStore::append` writes the block segment, by-hash index, by-lineage-height index, then updates heads (`history.rs:861`).
- Reader/CLI: `FsBlockStore::lineage_state` reads `heads.json`, builds the sparse map/proof, and checks lineage index consistency (`history.rs:878`); `sealed_head_block` uses the by-hash index to locate the segment line (`history.rs:803`). Used by predecessor/genesis startup (`parent.rs:443`, `parent.rs:500`) and append itself.
- Key IDs for joins: `lineage_id + block_hash` for by-hash; `lineage_id + block_height` for lineage-height; `lineage_id -> block_hash` for heads; `BlockLocation.segment + line_index` joins indexes back to the JSONL block stream.
- Evidence status: rebuildable projection/cache. `append` treats it as checked local state, but comments state heads are not independent authority (`history.rs:593`).
- Safe bounded inspection:

```sh
jq 'keys' ~/.ploke-eval/campaigns/<campaign>/prototype1/history/index/heads.json
jq -c '{lineage_id,block_height,block_hash,location}' ~/.ploke-eval/campaigns/<campaign>/prototype1/history/index/by-hash.jsonl | sed -n '1,20p'
jq -c '.' ~/.ploke-eval/campaigns/<campaign>/prototype1/history/index/by-lineage-height.jsonl | sed -n '1,20p'
```

### State root and predecessor observation

- Path pattern: persisted inside each sealed block header at `.state.header.common.opened_from_state`; no separate proof file is written.
- Persisted Rust type/schema: `HistoryStateRoot(HistoryHash)` (`history.rs:994`) in `BlockCommon` (`history.rs:2434`). `LineageState { root, proof, head }` is serializable (`history.rs:1152`) but is used in memory as the expected append state, not persisted as its own artifact.
- Writer: `handoff_block_fields` reads `FsBlockStore::lineage_state`, copies `state.root()` into `OpenBlock.opened_from_state`, and carries `expected_state` to append (`prototype1_process.rs:1429`, `prototype1_process.rs:1469`).
- Reader/CLI: `LineageState::verify_append` verifies the sparse proof, checks the sealed block opened from the expected root, then delegates head checks (`history.rs:1187`). This is exercised by `FsBlockStore::append`.
- Key IDs for joins: `lineage_id`, current `StoreHead::{Absent,Present}`, predecessor `BlockHash`, and `opened_from_state`.
- Evidence status: authoritative append precondition for the local store, not a global consensus root.
- Safe bounded inspection:

```sh
jq -c '{height:.state.header.common.block_height,opened_from_state:.state.header.common.opened_from_state,parents:.state.header.common.parent_block_hashes}' ~/.ploke-eval/campaigns/<campaign>/prototype1/history/blocks/segment-000000.jsonl | sed -n '1,20p'
```

### Crown lock, opening authority, admitted artifact claim

- Path pattern: sealed block header in `.../prototype1/history/blocks/segment-000000.jsonl`.
- Persisted Rust type/schema: `OpenBlock` fields are folded into `BlockCommon` (`history.rs:2405`); `SealBlock` fields become `SealedBlockHeader` (`history.rs:2452`); `OpeningAuthority::{Genesis,Predecessor}` stores either `GenesisAuthority { bootstrap_policy, tree_key, parent_identity }` or `PredecessorAuthority { predecessor_block_hash }` (`history.rs:2053`, `history.rs:2086`, `history.rs:2100`); `block::Claims` stores optional flat claims for policy, surface, manifest, artifact (`history.rs:2515`).
- Writer: `handoff_block_fields` chooses genesis vs predecessor opening authority from `LineageState.head()` (`prototype1_process.rs:1441`); `SealBlock::from_handoff` creates crown lock header material (`history.rs:2476`); live code admits the artifact claim with `Crown<Ruling>::admit_claim` inside the handoff seal closure (`prototype1_process.rs:1144`).
- Reader/CLI: predecessor startup verifies the current clean checkout tree against the sealed artifact claim (`parent.rs:547`, `history.rs:2950`) and verifies current surface roots against the sealed header (`parent.rs:569`, `history.rs:2983`).
- Key IDs for joins: `TreeKeyHash`, `Digest<Artifact>`, `ArtifactRef`, `ParentIdentityRef`, `SuccessorRef.runtime`, `SuccessorRef.artifact`, `crown_lock_transition`, `policy_ref`, `SurfaceCommitment` roots.
- Evidence status: authoritative local sealed-head evidence for successor admission. Current live claim population appears limited to the artifact claim; policy/surface/manifest claim slots can remain `None`.
- Safe bounded inspection:

```sh
jq -c '{height:.state.header.common.block_height,opening:.state.header.common.opening_authority,policy_ref:.state.header.common.policy_ref,claims:.state.header.claims,surface:.state.header.common.surface}' ~/.ploke-eval/campaigns/<campaign>/prototype1/history/blocks/segment-000000.jsonl | sed -n '1,10p'
```

### History entries and ingress import

- Path pattern: sealed block `.entries[]` in `.../prototype1/history/blocks/segment-000000.jsonl`.
- Persisted Rust type/schema: `Entry<Admitted>` (`history.rs:2326`) with `EntryCore` plus `Admitted` state fields including observer/recorder/proposer/admitting authority/ruling authority/lineage/block IDs (`history.rs:2275`, `history.rs:2313`). Ingress import is represented by `EntryPayload::IngressImport` (`history.rs:2208`) and `Ingress<ingress::Imported>` state (`history.rs:3161`).
- Writer: `Crown<Ruling>::admit_entry` routes entry admission to private `Block<Open>::admit` (`history.rs:3064`, `history.rs:2782`); `Ingress<Open>::import` creates a proposed entry with import chain-of-custody (`history.rs:3213`). Live handoff currently seals minimal blocks before successor spawn; I did not find a live path admitting non-empty entries into the sealed block stream.
- Reader/CLI: `Block<Sealed>::verify_hash` recomputes `entries_root` and `entry_count` (`history.rs:2901`). `FsBlockStore::sealed_head_block` currently rejects stored non-empty blocks (`history.rs:797`, `history.rs:950`), so startup admission can only load zero-entry sealed heads.
- Key IDs for joins: `EntryId`, `payload_hash`, `payload_ref`, `lineage_id`, `block_id`, `block_height`, `prior_block_hash`, `ingress_id`.
- Evidence status: intended authoritative block contents once admitted and sealed; currently mostly type surface plus tests, not live persisted evidence beyond zero-entry roots.
- Safe bounded inspection:

```sh
jq -c '{height:.state.header.common.block_height,entry_count:.state.header.entry_count,entries_len:(.entries|length),entries_root:.state.header.entries_root}' ~/.ploke-eval/campaigns/<campaign>/prototype1/history/blocks/segment-000000.jsonl | sed -n '1,20p'
```

## Evidence And Projections

### Transition journal

- Path pattern: `$(dirname <campaign.json>)/prototype1/transition-journal.jsonl`.
- Persisted Rust type/schema: JSONL of `JournalEntry` variants (`journal.rs:332`), including parent start, resource samples, materialize/build/spawn/ready/observe child, successor, active checkout, and successor handoff records.
- Writer: `PrototypeJournal::append` (`journal.rs:655`); path helper `prototype1_transition_journal_path` (`journal.rs:647`); many live call sites use `append_prototype1_journal_entry` (`prototype1_process.rs:168`).
- Reader/CLI: `PrototypeJournal::load_entries` and replay helpers (`journal.rs:370`); `history_preview::FsEvidenceStore::transition_journal` (`history_preview.rs:65`); metrics build (`metrics.rs:73`). CLI readers are `ploke-eval history preview`, `ploke-eval history metrics`, `ploke-eval loop prototype1-monitor history-preview`, and `ploke-eval loop prototype1-monitor history-metrics` (`cli_facing.rs:1492`, `cli_facing.rs:1533`).
- Key IDs for joins: `transition_id`, `runtime_id`, `campaign_id`, `node_id`, `generation`, `refs.branch_id`, `refs.candidate_id`, `refs.instance_id`, `refs.source_state_id`, `paths.*`.
- Evidence status: append-only evidence source and replay stream, not sealed History authority.
- Safe bounded inspection:

```sh
jq -c '{kind,generation,transition_id,runtime_id,campaign_id,node_id,refs}' ~/.ploke-eval/campaigns/<campaign>/prototype1/transition-journal.jsonl | sed -n '1,40p'
```

### History-shaped preview

- Path pattern: no default persisted file. It is emitted to stdout by CLI unless redirected.
- Persisted Rust type/schema if captured: `HistoryPreview { schema_version: "prototype1-history-preview.v1", generated_at, campaign_id, manifest_path, prototype_root, sources, blocks, entries, deferred, diagnostics }` (`history_preview.rs:297`); entries are `PreviewEntry` (`history_preview.rs:385`).
- Writer: `history_preview::run` prints a built preview (`history_preview.rs:231`); `cli_facing::run_history_preview` can slice `entries`, `diagnostics`, or a single entry (`cli_facing.rs:1570`).
- Reader/CLI: user/operator via `ploke-eval history preview ...` and `ploke-eval loop prototype1-monitor history-preview ...`.
- Key IDs for joins: preview preserves `EvidencePointer { class, ref_id, path, line, hash }` (`history_preview.rs:434`), plus generation, subject, payload hash, input/output refs.
- Evidence status: projection/cache only. It deliberately marks journal and raw document imports as degraded/pre-History (`history_preview.rs:700`).
- Safe bounded inspection:

```sh
cargo run -p ploke-eval -- history --campaign <campaign> preview --entries 20 --diagnostics 10
cargo run -p ploke-eval -- history --campaign <campaign> preview --format json --entries 5 | jq '{entries:.entries,diagnostics:.diagnostics}'
```

### Metrics projection

- Path pattern: no default persisted file. It is emitted to stdout by CLI unless redirected.
- Persisted Rust type/schema if captured: `Dashboard { schema_version: "prototype1-metrics-projection.v1", generated_at, campaign_id, manifest_path, derivation, rows, generations, cohorts, trajectory, selected_by_generation, diagnostics }` (`metrics.rs:112`).
- Writer: `metrics::run` prints the projection (`metrics.rs:50`); `metrics::build` reads the same evidence store as preview plus transition journal (`metrics.rs:73`).
- Reader/CLI: `ploke-eval history metrics ...` and `ploke-eval loop prototype1-monitor history-metrics ...` via `run_metric_slice` (`cli_facing.rs:1553`).
- Key IDs for joins: source refs from `EvidencePointer`, document paths, journal variant IDs, node/generation/branch/runtime IDs.
- Evidence status: projection/dashboard only; module docs explicitly say metrics do not strengthen authority of mutable files (`metrics.rs:1`).
- Safe bounded inspection:

```sh
cargo run -p ploke-eval -- history --campaign <campaign> metrics --rows 20 --view summary
cargo run -p ploke-eval -- history --campaign <campaign> metrics --format json --rows 10 --view trajectory | jq '{schema_version,trajectory,diagnostics}'
```

### Preview evidence documents read by History preview and metrics

- Path patterns: `.../prototype1/{scheduler.json,branches.json,evaluations/*.json,nodes/*/invocations/*.json,nodes/*/results/*.json,nodes/*/successor-ready/*.json,nodes/*/successor-completion/*.json,nodes/*/{node.json,runner-request.json,runner-result.json}}`.
- Persisted Rust type/schema: heterogeneous JSON documents loaded as `Document { class, path, pointer, value }` (`history_preview.rs:473`); classes are `EvidenceClass` (`history_preview.rs:573`).
- Writer: not owned by History; scheduler/branch/invocation/runner modules write them. History preview only reads with `FsEvidenceStore::documents` (`history_preview.rs:70`).
- Reader/CLI: `history preview` imports some as raw/degraded preview entries (`history_preview.rs:1071`); metrics applies node/request/invocation/result/evaluation/selection projections (`metrics.rs:87`).
- Key IDs for joins: `node_id`, `branch_id`, `runtime_id`, `generation`, `journal_path`, source/base/patch IDs, evaluation and runner result paths.
- Evidence status: evidence source or projection, never sealed authority until admitted into a block.
- Safe bounded inspection:

```sh
find ~/.ploke-eval/campaigns/<campaign>/prototype1 -maxdepth 4 -type f \( -name '*.json' -o -name '*.jsonl' \) | sed -n '1,80p'
jq 'keys' ~/.ploke-eval/campaigns/<campaign>/prototype1/scheduler.json
```

## Type Barriers Observed

- `Block<Open>::open`, `Block<Open>::admit`, and `Block<Open>::seal` are private to `history.rs`; crate-visible construction is routed through `Crown<Ruling>`/`Crown<Locked>` methods (`history.rs:2730`, `history.rs:3023`, `history.rs:3127`).
- `Crown<Ruling>` constructor is private; sibling modules can use `LockCrown` when they hold `Parent<Selectable>`, but cannot mint a ruling Crown from a string (`inner.rs:134`, `inner.rs:158`).
- `block::Claims` has no public constructor or `Default`; flat stored claims are filled/extracted through nested admitted/witnessed/verifiable boundaries (`history.rs:2515`).
- `FsBlockStore::append` verifies block hash, current expected state, state root, lineage, height, and parent hash before advancing heads (`history.rs:831`, `history.rs:1187`, `history.rs:1234`).

## Gaps / Unknowns / Overclaims To Avoid

- The store is local filesystem authority, not distributed consensus, global fork choice, or OS-process uniqueness. Module docs say this explicitly; reports should keep that qualifier.
- Bootstrap/genesis is still weaker than the intended uniform startup admission carrier. Gen0 setup writes parent identity, but the first sealed genesis block is produced at first live handoff if no configured History head exists.
- `Crown<Ruling>` proves possession of a lineage Crown but still accepts actor/ruler identity as data in `OpenBlock` and `admit_claim`; comments call out the missing `Parent<Ruling>` structural identity carrier (`history.rs:3024`, `history.rs:3094`).
- Startup can load only zero-entry sealed heads because `sealed_head_block` rejects non-empty entries. That is a clear implementation gap before admitted entry History can be used for successor startup.
- Ingress has typestate and payload support but I did not find a filesystem ingress journal/store or live import path.
- `history preview` and `history metrics` do not inspect the sealed History block store. They project transition journals and mutable evidence documents; their output should not be described as Crown authority.
- `heads.json` and index JSONL files are rebuildable projections. They are checked by append/read paths, but the sealed block stream is the authority-bearing record.
