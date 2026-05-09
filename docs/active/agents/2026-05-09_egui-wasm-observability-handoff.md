# 2026-05-09 egui/WASM Observability Handoff

Cold-restart handoff for the egui/WASM run observability thread. This thread
builds an interactive egui frontend (native + WASM) that consumes enriched
`PlaybackBrowserModel` JSON to answer operator questions about Prototype 1
self-improvement loop runs.

## Restart Checklist

1. Read this handoff.
2. Read the plan: [`2026-05-09_egui-wasm-observability-plan.md`](2026-05-09_egui-wasm-observability-plan.md).
3. Run the verification commands below to confirm everything still compiles.
4. Start Phase 2.

## Where We Are

We are on **Track 2** (Records / Playback / Observability). Track 1 (Bounded
Edit Surface / `ploke-tui`) is a separate thread — see
[`2026-05-08_bounded-edit-surface-handoff.md`](2026-05-08_bounded-edit-surface-handoff.md).

**Phases 0 and 1 are complete.** The typed record pipeline is built, enriched,
and exportable:

```
ploke-records ──→ ploke-tree ──→ ploke-tree-browser ──→ CLI export ──→ JSON
(passive schemas)  (projection)   (enriched models)      (browser_export.rs)
```

**Phase 2 is next:** Create `crates/ploke-tree-egui` — an `eframe`/`egui` app
that loads the exported JSON and renders an interactive timeline + detail panel.

## What We Built in Phase 1

### Enriched Browser Model (`ploke-tree-browser/src/lib.rs`)

`PlaybackBrowserModel` now carries:

- `run_summary: Option<RunSummary>` — campaign_id, node_count, generation_count,
  sealed_block_count, evaluation_count, evaluations_kept, evaluations_rejected,
  journal_entry_count
- Each `PlaybackBrowserStep` has new fields:
  - Join keys: `node_id`, `branch_id`, `candidate_id`
  - Detail snapshots: `evaluation: Option<EvaluationSnapshot>`,
    `surface: Option<SurfaceSnapshot>`, `protocol: Option<ProtocolSnapshot>`

`EvaluationSnapshot` carries: disposition, tool_calls_total, tool_calls_failed,
patch_attempted, patch_apply_state, nonempty_valid_patch, convergence,
oracle_eligible, aborted, reasons.

`SurfaceSnapshot` carries: target_relpath, patch_id, source_content_hash,
proposed_content_hash, source_state_id.

`ProtocolSnapshot` carries: counts per artifact type, model_id, provider_slug.

### Enrichment Join Path

The join works as follows:

1. `fine_history_browser_model_from_blocks()` builds the base model from sealed
   History blocks (54 fine steps for run 2).
2. `extract_node_id_from_label()` parses `candidate:node-NODEID:plan_index=N`
   to get the short node_id.
3. `build_node_branch_map()` reads `transition-journal.jsonl`, finds all
   `MaterializeBranch` entries, and builds `BTreeMap<String, NodeBranchInfo>`
   keyed by `node-NODEID`.
4. `enrich_fine_browser_model()` joins: short_id → `node-NODEID` →
   NodeBranchInfo (branch_id, candidate_id, target_relpath, hashes) →
   evaluation by branch_id.

### CLI Export Command

```bash
cargo run -p ploke-eval -- history export-browser-model \
  --campaign p1-bounded-surface-long-20260509-2 \
  --output /tmp/browser-model.json
```

Produces enriched JSON. Verified: 46 of 54 fine steps have evaluation and
surface snapshots populated.

### Key Bug Fixed

The initial enrichment tried to join through `branch_registry.source_nodes[].instance_id`,
but `instance_id` is the eval instance name (`BurntSushi__ripgrep-2209`), not
the node_id. The correct join is through the transition journal's
`MaterializeBranch` entries (`refs.node_id` in `node-NODEID` format).

## What's Next: Phase 2 — ploke-tree-egui Crate

### Goal

A native `eframe` app that loads the exported JSON and renders:

```
┌─────────────────────────────────────────────────────┐
│ [Load JSON]  Campaign: p1-bounded-surface-...       │
│ Nodes: 16  Generations: 3  Blocks: 4  Evals: 15     │
├──────────────────────────┬──────────────────────────┤
│ Timeline (scrollable)    │ Detail Panel             │
│                          │                          │
│ ● CandidateConsidered    │ Evaluation: reject       │
│   gen=0 node-263cba...   │ tool_calls: 11/0         │
│   reject                 │ patch: not attempted     │
│                          │ convergence: false       │
│ ● CandidateConsidered    │ oracle_eligible: false   │
│   gen=0 node-20b175...   │                          │
│   reject                 │ Surface:                 │
│                          │   crates/ploke-tui/...   │
│ ● SuccessorSelected      │   code_edit.rs           │
│   gen=0 node-52e144...   │   src hash: 76d1205f...  │
│                          │   proposed: 29ed525e...  │
│ ...                      │                          │
├──────────────────────────┴──────────────────────────┤
│ Warnings: 0                                         │
└─────────────────────────────────────────────────────┘
```

### Steps

1. Create `crates/ploke-tree-egui/Cargo.toml`:
   ```toml
   [package]
   name = "ploke-tree-egui"
   version = "0.1.0"
   edition = "2024"
   rust-version.workspace = true
   description = "egui/eframe UI for browsing Ploke run playback"

   [dependencies]
   ploke-tree-browser = { workspace = true }
   eframe = "0.31"
   egui = "0.31"
   serde = { workspace = true, features = ["derive"] }
   serde_json = { workspace = true }
   ```

2. Add to workspace `Cargo.toml`:
   - Add `"crates/ploke-tree-egui"` to `members`
   - Add `ploke-tree-egui = { path = "crates/ploke-tree-egui" }` to
     `[workspace.dependencies]`

3. Implement `src/main.rs`:
   - `PlokeTreeApp` struct holding `model: Option<PlaybackBrowserModel>`,
     `selected_step: Option<usize>`, `file_path: String`
   - `eframe::App::update()` rendering:
     - Top panel: file path input, load button, run summary stats
     - Central: `egui::ScrollArea` with step rows
     - Side: detail panel for selected step
     - Bottom: warning count

4. Each timeline row shows:
   - Step kind as colored text (CandidateConsidered=blue, SuccessorSelected=green,
     HistoryEntryAdmitted=gray, HistoryBlockSealed=purple)
   - Block height, label (candidate id or block hash), evidence badge
   - If evaluation exists: disposition badge (keep=green, reject=red)
   - Click to select → populates detail panel

5. Detail panel shows:
   - Evaluation section: all metrics in a grid
   - Surface section: target_relpath, content hashes
   - Protocol section: artifact counts (will be empty until wired)

### Design Constraints

- The egui crate depends ONLY on `ploke-tree-browser`. It does NOT depend on
  `ploke-eval` or `ploke-tree`.
- JSON is loaded at runtime via a file picker or path input.
- All model types come from `ploke_tree_browser::*`.
- Keep it simple — one file (`main.rs`) is fine for the first slice.
- The app should work as a native binary first. WASM comes in Phase 3.

## Key Files

| File | What It Does |
|---|---|
| `crates/ploke-tree-browser/src/lib.rs` | All model types: `PlaybackBrowserModel`, `RunSummary`, `EvaluationSnapshot`, `SurfaceSnapshot`, `ProtocolSnapshot`, `NodeBranchInfo`, `enrich_fine_browser_model()`, `build_run_summary()`, `extract_node_id_from_label()` |
| `crates/ploke-eval/src/cli/prototype1_state/browser_export.rs` | CLI export: `export_browser_model()`, `build_node_branch_map()` |
| `crates/ploke-eval/src/cli.rs` | `HistorySubcommand::ExportBrowserModel`, `Prototype1ExportBrowserModelCommand` |
| `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs` | Dispatch for `ExportBrowserModel` (line ~1965) |
| `crates/ploke-records/src/playback.rs` | `FineStepKind` enum, `EvidenceStrength` enum |
| `crates/ploke-records/src/evaluation.rs` | `Artifact`, `RunMetrics`, `InstanceComparison` |
| `crates/ploke-records/src/journal.rs` | `JournalEntry::MaterializeBranch(TransitionRecord)`, `Refs`, `Paths`, `Hashes` |
| `crates/ploke-records/src/history.rs` | `SealedBlockRecord`, authority surface test (playback.rs excluded at line ~533) |
| `crates/ploke-tree/src/playback/fine.rs` | `fine_run_playback_from_sealed_history()` |
| `/tmp/browser-model.json` | Latest enriched export from run 2 |

## Overnight Run Fixture

**Campaign:** `p1-bounded-surface-long-20260509-2`
**Root:** `/home/brasides/.ploke-eval/campaigns/p1-bounded-surface-long-20260509-2/prototype1`

Stats: 16 nodes, 4 generations, 4 sealed blocks, 15 evaluations (6 keep, 9 reject),
280 journal entries.

## Verification Commands

Run these on restart to confirm nothing is broken:

```bash
# Compile check
cargo fmt --all
cargo check -p ploke-records
cargo check -p ploke-tree
cargo check -p ploke-tree-browser
cargo check -p ploke-eval

# Tests
cargo test -p ploke-records 2>&1 | tail -n 5
cargo test -p ploke-tree 2>&1 | tail -n 5
cargo test -p ploke-tree-browser 2>&1 | tail -n 5

# Export enriched model (confirms pipeline works end-to-end)
cargo run -p ploke-eval -- history export-browser-model \
  --campaign p1-bounded-surface-long-20260509-2 \
  --output /tmp/browser-model.json

# Verify enrichment populated
python3 -c "
import json
with open('/tmp/browser-model.json') as f:
    m = json.load(f)
eval_steps = [s for s in m['steps'] if s.get('evaluation')]
surface_steps = [s for s in m['steps'] if s.get('surface')]
assert len(eval_steps) == 46, f'expected 46 eval steps, got {len(eval_steps)}'
assert len(surface_steps) == 46, f'expected 46 surface steps, got {len(surface_steps)}'
print('OK: enrichment verified')
print('run_summary:', json.dumps(m.get('run_summary'), indent=2))
"
```

## Recent Commits

```
87a96155 feat(ploke-eval): add 'history export-browser-model' CLI command
7943cf67 feat(ploke-tree-browser): enrich PlaybackBrowserModel with evaluation, surface, and protocol snapshots
6c7209d4 docs: add egui/WASM run observability plan; add real-campaign browser model serialization test
5584d96d fix(ploke-records): exclude playback.rs from authority surface check
```

## Watchouts

- The `public_api_has_no_authority_surface` test in `ploke-records/src/history.rs`
  excludes `playback.rs` because `RunPlayback::new` is a passive data constructor.
  If you add new `pub fn new` (or `verify`, `seal`, `admit`, etc.) in other
  `ploke-records` modules, that test will catch them.
- `ploke-eval` emits ~196 warnings. These are pre-existing and not from our changes.
- The `ContentHash` type (in `ploke-records/src/ids.rs`) is a `string_id!` macro
  newtype — access the inner value with `.0`.
- `PatchId` is also a `string_id!` newtype — same `.0` access pattern.
- The `JournalEntry::MaterializeBranch` variant wraps a `TransitionRecord`, not
  named fields. Pattern match as `JournalEntry::MaterializeBranch(record)`.