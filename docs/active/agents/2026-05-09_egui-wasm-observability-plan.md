# 2026-05-09 egui/WASM Run Observability Plan

Plan for building an interactive egui-based WASM frontend that consumes
`ploke-records` / `ploke-tree` / `ploke-tree-browser` typed playback data
to answer operator questions about Prototype 1 self-improvement loop runs.

## Context

### Two Active Tracks

- **Track 1 — Bounded Edit Surface** (`ploke-tui`): The self-edit framework that
  actually performs edits during the loop. Working — the overnight run made it
  through 4+ generations with self-edits.
- **Track 2 — Records / Playback / Observability** (this thread): Making the
  data from Track 1 observable. The typed record pipeline is built and verified:

  | Layer | Crate | Status |
  |---|---|---|
  | Passive schemas | `ploke-records` | All record families typed, no public `serde_json::Value` |
  | Playback vocabulary | `ploke-records::playback` | `RunPlayback<G>`, `RunPlaybackRef<'a, G>`, `Coarse`, `Fine`, `EvidenceStrength` |
  | Projection engine | `ploke-tree::playback` | Coarse (by sealed block) and Fine (by entry/candidate) over sealed History |
  | Renderer-neutral models | `ploke-tree-browser` | `PlaybackBrowserModel` enriched with evaluation/surface/protocol snapshots |
  | CLI export | `ploke-eval history export-browser-model` | Produces enriched JSON from any campaign |
  | **egui/WASM UI** | — | **does not exist yet** |

### Overnight Run Data Snapshot (2026-05-09)

Two runs were launched targeting 20 generations of self-improvement on
`BurntSushi/ripgrep-2209`:

**Run 1** (`p1-bounded-surface-long-20260509-1`):
- 7 nodes, no History directory, no sealed blocks
- Likely crashed early or never reached first seal

**Run 2** (`p1-bounded-surface-long-20260509-2`):
- 16 nodes across 4 generations (max_generation=4)
- 4 sealed History blocks
- 15 evaluations (6 keep, 9 reject)
- 280 transition journal entries (all parsed)
- 22 channel files, 62 envelope lines (all parsed)
- branches.json, scheduler.json present
- 15 treatment branch dirs with BurntSushi__ripgrep-2209 instances
- 1 baseline run with protocol-artifacts

### Enriched Export Verified (Phase 1 output)

```bash
cargo run -p ploke-eval -- history export-browser-model \
  --campaign p1-bounded-surface-long-20260509-2 \
  --output /tmp/browser-model.json
```

Produces 54 fine steps with:
- **46 steps with evaluation snapshots** (disposition, tool_calls_total, tool_calls_failed,
  patch_attempted, patch_apply_state, nonempty_valid_patch, convergence, oracle_eligible,
  aborted, reasons)
- **46 steps with surface snapshots** (target_relpath, source_content_hash,
  proposed_content_hash, source_state_id)
- **Run summary** (node_count=16, generation_count=3, sealed_block_count=4,
  evaluation_count=15, evaluations_kept=6, evaluations_rejected=9, journal_entry_count=280)
- **Protocol snapshots**: not yet populated (need protocol_artifacts_dir wiring)

## Operator Questions the UI Must Answer

### Surface Questions (Fast, After Run Completes)

| Question | Data Source | Status |
|---|---|---|
| Is the run still going? | No single heartbeat; needs process table or liveness file | **Missing** |
| Is the loop healthy? | History block chain, journal completeness | **Available** |
| Did the loop exit with expected state or something else? | Terminal journal entry, last block state | **Available** |
| How many children across how many generations? | Scheduler nodes + History blocks | **Available** |
| What are the metrics telling us? (mechanical + LLM) | Evaluation payloads, protocol artifacts | **Available, now projected** |
| What is the tree state and active ruler? | Scheduler + History blocks + successor records | **Partially available** |
| Is the loop improving on metrics? | Trajectory over generations | **Needs aggregation** |
| Are edits making sense? Is ploke-tui working? | Surface evidence records | **Available, now projected** |
| What went wrong and where are the logs? | Journal entries, diagnostics, protocol artifacts | **Available but scattered** |

## Architecture

### Crate Boundary

```
ploke-records        passive schemas, playback vocabulary (no FS)
ploke-tree           load records, join, build RunPlayback projections
ploke-eval           emit authoritative records; CLI export command
ploke-tree-browser   renderer-neutral browser models (serde); enrichment joins
ploke-tree-egui      egui/eframe app, WASM target (NEXT — Phase 2)
```

### Data Delivery Model

**Option 1 (implemented): Static snapshot.** `ploke-eval history export-browser-model`
produces enriched JSON. The egui app loads it. Works for post-hoc inspection.

**Option 2 (future): Local server + polling.** HTTP server serves JSON endpoints.
WASM app polls. Enables live-ish updates.

**Option 3 (deferred): Full `ploke-tree` in WASM.** Hard — filesystem, async, dep tree.

### What Views (in priority order)

```
Run Dashboard
├── Summary bar (run id, status, generation count, total children, elapsed)
├── Timeline view (fine steps with scores overlaid)
│   └── Expand step → detail panel
├── Tree view (parent → children → successor chain)
│   └── Select node → detail panel
├── Detail panel (drill into one candidate)
│   ├── Evaluation scores (mechanical)
│   ├── Surface evidence (what was edited)
│   └── Protocol artifacts summary
└── Aggregation panel (score distributions, tool failure rates)
```

### egui Architecture (Phase 2)

An `eframe` app (`ploke-tree-egui`):

- **Top bar**: Load JSON button, campaign/run name display, run summary stats
- **Central panel**: Scrollable timeline. Each fine step = a row showing
  kind, evidence strength, block height, label. Click to select.
- **Side panel** (right): Selected step detail — evaluation scores,
  surface evidence, protocol artifacts.
- **Bottom panel**: Warnings/diagnostics strip.

Depends on `ploke-tree-browser` for all model types. Does NOT depend on
`ploke-eval` or `ploke-tree` (JSON is produced server-side).

## Implementation Plan

### ✅ Phase 0: Data Shape Verification

**Done.** Inventory found:
- 13 mechanical metrics in evaluation `RunMetrics`
- 5 protocol artifact types with LLM assessments
- Surface/edit evidence in transition journal `MaterializeBranch` entries
- Join path: candidate label → short node_id → `node-NODEID` → journal MaterializeBranch → branch_id, candidate_id, target_relpath, hashes
- Branch registry `instance_id` is the eval instance (`BurntSushi__ripgrep-2209`), NOT the node_id — wrong join key. Correct join is through journal.

### ✅ Phase 1: Enrich PlaybackBrowserModel

**Done.** Changes:

**`ploke-tree-browser/src/lib.rs`:**
- Added `RunSummary` (campaign_id, node_count, generation_count, sealed_block_count,
  evaluation_count, evaluations_kept, evaluations_rejected, journal_entry_count, terminal_status)
- Added `EvaluationSnapshot` (disposition, tool_calls_total, tool_calls_failed,
  patch_attempted, patch_apply_state, nonempty_valid_patch, convergence, oracle_eligible,
  aborted, reasons)
- Added `SurfaceSnapshot` (target_relpath, patch_id, source_content_hash,
  proposed_content_hash, source_state_id)
- Added `ProtocolSnapshot` (counts per artifact type, model_id, provider_slug)
- Added `NodeBranchInfo` carrier (branch_id, candidate_id, target_relpath,
  source_state_id, source_content_hash, proposed_content_hash, patch_id)
- Added join key fields to `PlaybackBrowserStep`: node_id, branch_id, candidate_id
- Added `enrich_fine_browser_model()` — joins through journal-derived
  `BTreeMap<String, NodeBranchInfo>` keyed by `node-NODEID`
- Added `build_run_summary()` helper
- Added `extract_node_id_from_label()` — parses `candidate:node-NODEID:plan_index=N`
  to extract short node_id

**`ploke-eval/src/cli/prototype1_state/browser_export.rs` (NEW):**
- `export_browser_model()` — loads History blocks, evaluations, transition journal;
  builds node_branches map from `MaterializeBranch` entries; enriches model;
  writes JSON
- `build_node_branch_map()` — parses `transition-journal.jsonl`, extracts
  `MaterializeBranch` entries into `BTreeMap<String, NodeBranchInfo>`

**`ploke-eval/src/cli.rs`:**
- Added `ExportBrowserModel(Prototype1ExportBrowserModelCommand)` variant to
  `HistorySubcommand`
- Added `Prototype1ExportBrowserModelCommand` struct with `--output` flag

**`ploke-eval/src/cli/prototype1_state/cli_facing.rs`:**
- Wired `HistorySubcommand::ExportBrowserModel` dispatch to
  `browser_export::export_browser_model()`

**Key bug found and fixed:** The initial enrichment used `branch_registry.source_nodes[].instance_id`
as the join key, but `instance_id` is the eval instance name (`BurntSushi__ripgrep-2209`),
not the node_id. The correct join is through the transition journal's
`MaterializeBranch` entries, which carry `refs.node_id` (format `node-NODEID`).

### ➡️ Phase 2: Create ploke-tree-egui Crate (NEXT)

- [ ] Add `crates/ploke-tree-egui/Cargo.toml` with deps: `eframe`, `egui`,
  `ploke-tree-browser`, `serde`, `serde_json`.
- [ ] Add to workspace `Cargo.toml` members.
- [ ] Implement `PlokeTreeApp` struct with `eframe::App` trait.
- [ ] **Top bar**: Load JSON button, campaign/run name display, run summary.
- [ ] **Timeline panel**: Scrollable list of fine playback steps. Each row
  shows step kind icon, block height, label, evidence badge, evaluation disposition.
  Click to select.
- [ ] **Detail panel**: Selected step detail. Shows evaluation scores,
  surface evidence, protocol artifact summary.
- [ ] **Summary bar**: Run metadata from `PlaybackBrowserModel.run_summary`.

### Phase 3: WASM Target

- [ ] Add `wasm-bindgen` and configure `eframe` for WASM build.
- [ ] Add `index.html` and build script.
- [ ] Verify with `trunk serve`.
- [ ] Test with the overnight run 2 JSON snapshot.

### Phase 4: Enrichment Iteration

- [ ] Add tree view (parent → children → successor) using scheduler node
  records joined with History blocks.
- [ ] Add aggregation panel (score distributions, tool failure rates).
- [ ] Add live-update polling (local server endpoint).

## Stop Conditions Per Phase

**Phase 0**: ✅ Real data inventory complete, gaps documented.
**Phase 1**: ✅ `ploke-eval history export-browser-model` produces enriched JSON
  with evaluation scores and surface evidence for run 2.
**Phase 2**: `ploke-tree-egui` native app renders timeline + detail panel from
  the exported JSON.
**Phase 3**: WASM build runs in browser with the same data.
**Phase 4**: Each new view (tree, aggregation) is implemented and validated
  against real data.

## Key Files for Restart

| File | Role |
|---|---|
| `crates/ploke-tree-browser/src/lib.rs` | `PlaybackBrowserModel`, `RunSummary`, `EvaluationSnapshot`, `SurfaceSnapshot`, `ProtocolSnapshot`, `NodeBranchInfo`, `enrich_fine_browser_model()`, `build_run_summary()` |
| `crates/ploke-eval/src/cli/prototype1_state/browser_export.rs` | CLI export command: loads campaign, builds enriched model, writes JSON |
| `crates/ploke-eval/src/cli.rs` | `Prototype1ExportBrowserModelCommand` and `HistorySubcommand::ExportBrowserModel` |
| `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs` | Dispatch wiring for `ExportBrowserModel` |
| `crates/ploke-records/src/playback.rs` | `RunPlayback`, `FineStep`, `FineStepKind`, `EvidenceStrength` |
| `crates/ploke-tree/src/playback/fine.rs` | `fine_run_playback_from_sealed_history()` |
| `crates/ploke-records/src/evaluation.rs` | `Artifact`, `RunMetrics`, `InstanceComparison` |
| `crates/ploke-records/src/journal.rs` | `JournalEntry::MaterializeBranch(TransitionRecord)`, `Refs`, `Paths`, `Hashes` |
| `crates/ploke-records/src/history.rs` | `SealedBlockRecord`, authority surface test (playback.rs excluded) |
| `/tmp/browser-model.json` | Latest enriched export from run 2 (54 steps, 46 with eval+surface) |

## No-Goals

- Do not move `ploke-eval`'s write/authority paths into the UI.
- Do not parse rendered CLI output.
- Do not make the UI a control surface for the loop (read-only first).
- Do not introduce a separate JSON format that diverges from typed record shape.
- Do not add live filesystem watching in the first slice.

## Verification Commands

```bash
# Baseline tests
cargo fmt --all
cargo test -p ploke-records
cargo test -p ploke-tree
cargo test -p ploke-tree-browser
cargo check -p ploke-eval

# Real campaign load
PLOKE_TREE_RUN_ROOT=/home/brasides/.ploke-eval/campaigns/p1-bounded-surface-long-20260509-2/prototype1 \
  cargo test -p ploke-tree fs_run_store_loads_real_campaign -- --ignored --nocapture 2>&1 | tail -n 40

# Export enriched browser model
cargo run -p ploke-eval -- history export-browser-model \
  --campaign p1-bounded-surface-long-20260509-2 \
  --output /tmp/browser-model.json

# Inspect enrichment
python3 -c "
import json
with open('/tmp/browser-model.json') as f:
    m = json.load(f)
eval_steps = [s for s in m['steps'] if s.get('evaluation')]
surface_steps = [s for s in m['steps'] if s.get('surface')]
print(f'steps: {m[\"step_count\"]}')
print(f'with evaluation: {len(eval_steps)}')
print(f'with surface: {len(surface_steps)}')
print('run_summary:', json.dumps(m.get('run_summary'), indent=2))
"
```