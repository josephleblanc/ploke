# Blocker 02 – Embedding Set Lifecycle & Activation Contract

_Last updated: 2025-11-14_

## Problem statement
- The current system assumes a single embedding backend selected at startup (`UserConfig::embedding`, `AppState::embedder`). Switching providers requires editing `config.toml` and restarting.
- The groundwork report introduces `embedding list|use|drop|prune` commands and “versioned embedding sets keyed by provider/model/dimension,” but there is no concrete contract for:
  - What metadata an embedding set tracks and how it relates to database rows.
  - How to switch the active set without re-indexing or restarting.
  - How long-running indexing tasks react when the target set changes mid-run.
  - How RAG/search components know which set to query.

## Goals
1. Define a canonical `EmbeddingSet` metadata model that captures provenance and activation state for every workspace.
2. Specify lifecycle transitions (`staged → active → retired`) and enforce them through typed commands so users cannot corrupt indexes.
3. Design `/embedding …` commands and executor flows for listing, activating, and deleting sets, including IoManager-backed persistence of defaults.
4. Document how runtime components (IndexerTask, RAG service, AppState caches) observe activation changes and what safety checks must occur.
5. Enumerate validation & evidence artifacts needed to prove the lifecycle works.

## Embedding set metadata (ties into Blocker 01 schema)
Rust struct (shared in `ploke-core` or `ploke-db`):
```rust
pub struct EmbeddingSetMeta {
    pub id: Uuid,
    pub workspace: Uuid,
    pub provider_slug: EmbeddingProviderSlug,
    pub model_id: EmbeddingModelId,
    pub dimension: u32,
    pub dtype: EmbeddingDType,
    pub metric: EmbeddingMetric,
    pub status: EmbeddingSetStatus,
    pub batch_size: u32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub sources: EmbeddingSourceFingerprint,
    pub notes: Option<String>,
}
```
- `EmbeddingSourceFingerprint` is the IoManager hash of the embedder config + provider catalog entry used to produce the vectors.
- `status` values:
  - `Staged`: vectors partially populated; not yet active for RAG/search.
  - `Active`: default set used by consumers.
  - `Degraded`: active but missing rows (e.g., indexing aborted). Only transient; triggers UI warnings.
  - `Retired`: kept for history; not used unless explicitly reactivated.
- Each workspace has exactly one `Active` set per provider/model combination, but only one `Active` set is used at a time by the runtime. Think of others as “cached” sets that can be activated quickly.

## Global activation pointers
- `embedding_sets` table stores metadata for every set.
- New table `embedding_set_pointers` maintains the runtime selection:
```
:put embedding_set_pointers {
    workspace => Uuid,
    active_set_id => Uuid,
    rag_override_set_id => Uuid?,
    last_updated => Int
}
```
- `workspace` matches the namespace in ploke-db. Only one row per workspace.
- `rag_override_set_id` allows short-lived experiments (e.g., run RAG with a staged set while indexing continues for the default set). When `None`, both indexing + RAG use `active_set_id`.
- `ploke-tui` caches this pointer in the new `EmbeddingManager` (Blocker 04) but persists changes via the database (not config files).

## CLI surface & executor contract
All commands follow IoManager-safe writes (stage file/script with hash, apply via IoManager).

| Command | Purpose | Behavior |
| --- | --- | --- |
| `/embedding status` | Show active set + indexing progress | Queries `embedding_set_pointers` + `embedding_sets`, prints active & staged sets with provider/model/dim metadata. |
| `/embedding list [--all] [--provider X]` | Inspect sets | `--all` shows retired sets; default lists staged+active. Columns: id (short), provider/model, dim/dtype, status, populated rows, created_at. |
| `/embedding use <set-short-id>` | Activate an existing set | Preconditions: set status is `Active` or `Staged` with full coverage (no null nodes). Executor triggers `EmbeddingManager::activate(set_id)` which performs health checks, updates pointer row, and broadcasts events. |
| `/embedding rebuild [--provider X] [--model Y]` | Create fresh set for current config | Allocates new `EmbeddingSetMeta` row in `Staged` status, kicks off indexing run targeted at the new set, and optionally pins provider/model overrides. |
| `/embedding drop <set-short-id>` | Retire set | Allowed only for `Retired` or `Staged` sets that are not referenced by pointer row. Deletes `embedding_nodes` rows + HNSW indexes + metadata. Writes audit entry. |
| `/embedding prune --max N` | Keep N most recent finished sets per provider | Sorts by `created_at`, retires extras using IoManager-managed transaction. |

Parser additions mirror `model …` commands (extend `Command` enum + parser).

## Activation flow
1. **User runs `/embedding use <id>`**
   - Parser emits `Command::EmbeddingUse { id }`.
   - Executor resolves `id` to full UUID via `embedding_sets` query (short-id matching identical to model overlay: prefix match, fail on ambiguity).
   - `EmbeddingManager::prepare_activation(set_id)` performs:
     - Ensure set status ∈ {`Active`, `Staged`}.
     - Ensure row counts == node counts (if not, prompt to run `/embedding rebuild` and refuse).
     - Ensure HNSW indexes exist for `(node_type, set_id)`; if missing, schedule creation via `IoManager` job.
   - On success, `EmbeddingManager::commit_activation` updates `embedding_set_pointers.active_set_id` inside a DB transaction and emits `RuntimeEmbeddingEvent::Activated { set_id }` on the event bus.
   - AppState listeners (RAG, Indexing overlay, future Observability) update UI to show the new provider/model/dim.

2. **Indexer run targeting new set**
   - `/embedding rebuild --provider openai --model text-embedding-3-large` triggers `EmbeddingManager::create_set(params)` which writes metadata row with `status=Staged`, `dimension=dyn`, `sources=hash(embedder-config)`.
   - `IndexerTask` receives `IndexerStart { set_id }`, writes vectors only into `embedding_nodes` for that set, and updates set status to `Active` when row counts match total nodes.
   - If indexing fails, set stays `Staged` and `/embedding status` shows error summary referencing `target/test-output/embedding/indexing_<ts>.json`.

3. **Dropping/pruning**
   - `drop` and `prune` wrap a DB transaction that ensures no HNSW job currently runs for the set. IoManager collects the Cozo scripts (drop indexes, delete rows) and validates file hashes before applying.

## Runtime consumers
- **RAG / search**: Accept `EmbeddingContext { active_set_id }` from `EmbeddingManager`. All queries add `embedding_set_id = active_set_id` filter.
- **IndexerTask**: Must always run against an explicit set id; never implicit. When `/embedding use` switches to a different set while indexing is in progress, the manager either (a) blocks activation until the run completes, or (b) allows activation but leaves task running on `staged` set. Decision: activation is rejected if `set.status == Staged && set.indexing_job_active`. This keeps HNSW indexes consistent.
- **AppState config**: Remove `EmbeddingConfig` from the long-lived runtime pointer once the manager owns activation. `UserConfig` still records provider defaults but active set selection is stored in DB to survive restarts without editing config files.

## Safety + evidence requirements
- Every state transition writes an audit row to `target/test-output/embedding/lifecycle_<ts>.json` summarizing:
  - command executed,
  - actor,
  - prior/next set ids,
  - validation checks performed,
  - row/index counts sampled.
- Unit tests: `EmbeddingManager` state machine tests (simulate set creation, activation, drop) verifying constraints.
- Integration tests: `cargo test -p ploke-tui --features test_harness` scenario that (a) creates two sets, (b) activates second, (c) ensures RAG queries read from new set, (d) ensures drop refuses while set is active.
- Live gate: when remote embedding is enabled, `/embedding use` triggers actual tool-call traces stored under `target/test-output/embedding/live/…` proving that remote provider vectors were fetched before activation.

## Open review questions
1. Should activation pointer live purely in DB or also mirror to config (for offline CLI use)? – Recommendation: DB-first, but provide `UserConfig::embedding.default_set` fallback for cold boot when DB is empty.
2. How do we expose partial sets (Degraded) to users? – Proposed approach: block activation; require `/embedding rebuild --resume <id>` that restarts indexing from last cursor.
3. Do we need workspace-scoped permissions before changing active set? – Not today (single-user TUI), but the command executor should still prompt for confirmation if dropping/pruning would delete the currently active set.

Answering these items unblocks Blocker 04 (runtime manager mechanics) and ensures CLI work can target a well-defined lifecycle.
