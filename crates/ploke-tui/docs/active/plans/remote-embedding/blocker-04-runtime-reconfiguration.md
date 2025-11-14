# Blocker 04 – Runtime Reconfiguration & Embedding Manager Workflow

_Last updated: 2025-11-14_

## Problem statement
- `AppState::embedder` is constructed once during startup (`crates/ploke-tui/src/lib.rs:171-217`) and shared by the indexer, RAG, and tools.
- `/model load` merely warns the user to restart because there is no mechanism to rebuild the embedder and hot-swap dependent services.
- The new embedding set lifecycle (Blocker 02) requires runtime switching between sets/providers, coordinating running `IndexerTask`s, `RagService`, and other caches without tearing down the TUI.

## Goals
1. Introduce an `EmbeddingManager` responsible for constructing, swapping, and broadcasting embedding contexts (provider/model/dimension/set_id) at runtime.
2. Ensure configuration changes (`/embedding use`, `/embedding rebuild`, config file edits) stage via IoManager and are applied atomically.
3. Avoid inconsistent states: long-running indexing jobs must either target the requested set or be paused during activation; RAG must never read from a stale embedder after a switch.
4. Provide observable events/logs for every reconfiguration (per AGENTS.md evidence requirements).

## Proposed architecture
```
AppState
 ├─ config: RuntimeConfig
 ├─ embedder: Arc<EmbeddingProcessor>        // replaced by manager handle
 ├─ embedding_manager: Arc<EmbeddingManager>
 ├─ rag: Option<Arc<RagService>>
 └─ indexer_task: Arc<IndexerTask>
```

### `EmbeddingManager`
```rust
pub struct EmbeddingManager {
    ctx: RwLock<EmbeddingContext>,
    factory: Arc<dyn EmbeddingServiceFactory>,
    db: Arc<Database>,
    io: IoManagerHandle,
    event_bus: EventBus<RuntimeEmbeddingEvent>,
}

pub struct EmbeddingContext {
    pub set_id: Uuid,
    pub provider_slug: EmbeddingProviderSlug,
    pub model_id: EmbeddingModelId,
    pub shape: EmbeddingShape,
    pub service: Arc<dyn EmbeddingService>,
}
```
- `EmbeddingServiceFactory` builds `Arc<dyn EmbeddingService>` from registry records + secrets; backed by the trait stack (Blocker 03).
- `EmbeddingManager` is the single source of truth for the active embedder. AppState stores a clone of `Arc<EmbeddingManager>` instead of `Arc<EmbeddingProcessor>`.

### Event flow
`RuntimeEmbeddingEvent` enum:
```rust
enum RuntimeEmbeddingEvent {
    ContextChanged(EmbeddingContextSummary),
    ActivationFailed { set_id: Uuid, reason: ActivationError },
    IndexingRequested { set_id: Uuid },
}
```
Consumers (RAG, indexing overlay, CLI overlays) subscribe and update local state when events arrive.

## Reconfiguration sequences
### 1. `/embedding use <set>` (hot swap existing set)
1. Parser emits `Command::EmbeddingUse`.
2. Executor calls `EmbeddingManager::activate_set(set_id)`.
3. Manager workflow:
   - Acquire `RwLock` write guard with timeout (prevents deadlocks while indexing uses the embedder).
   - Query `embedding_sets` to load metadata + set status.
   - If a different set is already active, instruct running `IndexerTask` (if any) to pause/resume via `IndexerCommand::Pause` before swapping.
   - Build new embedding service via factory (lazy, uses provider slug + registry metadata + stored secrets). This may involve HTTP requests (e.g., verifying remote API key) – wrap in IoManager-managed future for observability.
   - Update `context.service` pointer and store `EmbeddingContextSummary` (set_id, provider, model, dimension) for UI.
   - Persist pointer row in DB (Blocker 02) and emit `RuntimeEmbeddingEvent::ContextChanged`.
   - Resume paused indexer if needed.

### 2. `/embedding rebuild …` (create new set + reconfigure when ready)
1. Executor requests `EmbeddingManager::spawn_set(params)`.
2. Manager inserts metadata row, returns new set_id.
3. Manager spawns `IndexerTask::run_for_set(set_id, target_shape)` using `EmbeddingContext.service.clone()` **or** a temporary service derived from CLI flags.
4. Upon completion, manager updates set status to `Active` or `Degraded` based on row counts, optionally auto-activates if the user passed `--and-use`.

### 3. Config file changes
- `UserConfig` still stores default provider preferences. When IoManager writes a new `config.toml`, the file watcher (existing config reload path) notifies `EmbeddingManager`.
- Manager diff-checks the new config; if only defaults changed, no immediate action is required. If `auto_apply` flag is set, manager triggers a rebuild/activation using the new defaults.

## Safety guarantees
- `EmbeddingManager` is the only component allowed to hand out `Arc<dyn EmbeddingService>` references. Consumers call `EmbeddingManager::context()` to clone the latest handle.
- Before swapping contexts, manager ensures there are no outstanding `EmbeddingJobHandle`s referencing the old service. Implementation: track an `ArcSwap<EmbeddingContext>` plus reference counting; new contexts wait until old `Arc`s drop or until a timeout triggers cancellation.
- Indexer and RAG both take `EmbeddingContextGuard` objects (RAII) that release references when work completes.

## Persistence
- Activation pointer updates use IoManager scripts to update `embedding_set_pointers` (Blocker 02). Validation: query the row afterwards and ensure `last_updated` changed.
- Manager writes textual logs to `crates/ploke-tui/docs/logs/embedding_manager/*.md` (or rotates) summarizing each swap (set_id, provider/model, reason) to satisfy “impl logs” requirement.

## Tests & instrumentation
- **Unit tests**: Simulate concurrent RAG + indexing loads while triggering `activate_set`. Ensure no race/hard crash occurs and old service is dropped once guards release.
- **Integration tests** (`test_harness`): Launch TUI harness, run `/embedding rebuild --provider openai`, issue `/embedding use` while an indexing job is mid-flight, assert manager blocks activation until job completes or user confirms cancellation.
- **Telemetry**: Manager emits events recorded under `target/test-output/embedding/runtime_switch_<ts>.json` capturing before/after context, active commands, and durations.

## Open items for review
1. Determine whether manager should restart the entire `IndexerTask` when swapping contexts or simply update its embedder reference (leaning restart to avoid half-embedded sets).
2. Decide how CLI exposes conflict resolution (“Indexer currently running for set X; pause and activate Y?”). Proposed: prompt user with `--force` flag.
3. Evaluate whether IoManager should own the activation pointer update end-to-end (submit script, wait for hashed apply) vs. letting DB handle it directly. Recommendation: IoManager-managed script for auditability.

This workflow ensures runtime changes are consistent, observable, and testable, unlocking a non-restart UX for remote embeddings.
