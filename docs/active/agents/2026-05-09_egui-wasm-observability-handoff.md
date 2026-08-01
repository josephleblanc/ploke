# 2026-05-09 egui/WASM Observability Handoff

Cold-restart handoff for the egui/WASM run observability thread. This thread
builds an interactive egui frontend (native + WASM) that consumes enriched
`PlaybackBrowserModel` JSON to answer operator questions about Prototype 1
self-improvement loop runs.

## Restart Checklist

1. Read this handoff.
2. Read the plan: [`2026-05-09_egui-wasm-observability-plan.md`](2026-05-09_egui-wasm-observability-plan.md).
3. Run the verification commands below to confirm everything still compiles.
4. Continue Phase 3 browser serving/manual test unless manual testing finds
   another native UI blocker.

## Where We Are

We are on **Track 2** (Records / Playback / Observability). Track 1 (Bounded
Edit Surface / `ploke-tui`) is a separate thread — see
[`2026-05-08_bounded-edit-surface-handoff.md`](2026-05-08_bounded-edit-surface-handoff.md).

**Phases 0, 1, 2, and 2.1 are complete.** The typed record pipeline is built,
enriched, exportable, and viewable in the native egui app:

```
ploke-records ──→ ploke-tree ──→ ploke-tree-browser ──→ CLI export ──→ JSON
(passive schemas)  (projection)   (enriched models)      (browser_export.rs)
```

**Phase 3 is in progress:** the WASM compile path exists. Next is serving the
Trunk app and testing the same exported JSON in a browser.

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
surface snapshots populated, and 43 of 54 have protocol snapshots populated.

### Key Bug Fixed

The initial enrichment tried to join through `branch_registry.source_nodes[].instance_id`,
but `instance_id` is the eval instance name (`BurntSushi__ripgrep-2209`), not
the node_id. The correct join is through the transition journal's
`MaterializeBranch` entries (`refs.node_id` in `node-NODEID` format).

## What We Built in Phase 2

`crates/ploke-tree-egui` is now a native `eframe` app that loads the exported
JSON and renders:

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

Implemented:

- `crates/ploke-tree-egui/Cargo.toml` with `eframe`, `egui`,
  `ploke-tree-browser`, `serde`, and `serde_json`.
- Workspace registration in `Cargo.toml`.
- `src/main.rs` with `PlokeTreeApp`:
  - path input defaulting to `/tmp/browser-model.json`
  - load button and status/error message
  - top summary from `PlaybackBrowserModel.run_summary`
  - scrollable timeline rows with kind, block height, label, evidence,
    disposition, and join ids
  - selectable rows feeding a right-side detail panel
  - bottom diagnostics strip with warning and missing-data counts
- `PlaybackBrowserModel.schema_version` is now an owned `String`, matching the
  persisted-record pattern and avoiding an egui-side JSON buffer leak.

Run it natively with:

```bash
cargo run -p ploke-tree-egui
```

### Manual Test Findings (2026-05-09)

Verified manually:

- Loading `/tmp/browser-model.json` works.
- Bad paths fail gracefully, clear the display, and show a small useful error
  under the load path.
- Timeline hover makes rows feel interactive.
- Candidate rows show keep/reject.
- Candidate surface sections show the target path.
- Non-candidate rows do not look broken just because evaluation/surface data is
  absent.

Limitations found and addressed in Phase 2.1:

- The right detail panel can overrun the visible viewport and needs its own
  scroll area.
- Hashes and raw ids are visually noisy. They are provenance/audit handles, not
  first-order operator signals, and should move behind an Advanced/Provenance
  section unless there is a copy/open/search affordance.
- `target_relpath` is useful and should remain promoted in the Surface section.
- The footer's `missing eval`, `missing surface`, and `missing protocol` counts
  are too coarse. They conflate "not applicable" with "expected but absent".
- Fine granularity means the playback is decomposed into fine History-derived
  steps; it does not mean every row should have every evidence family. The UI
  should make that distinction explicit through applicability-aware diagnostics.
- Protocol snapshots now populate when the branch evaluation points to a
  treatment run with persisted protocol artifacts. For the run 2 fixture this is
  currently 43 of 54 fine steps.

## What We Built in Phase 2.1

Native UI cleanup and upstream protocol enrichment are done:

- The selected-step detail panel is scrollable.
- Evaluation disposition, tool-call counts, patch state, convergence, oracle
  eligibility, aborted, reasons, and surface `target_relpath` are promoted.
- Raw ids and hashes are behind collapsed Advanced/Provenance sections.
- Footer diagnostics distinguish not applicable, expected absent, and unavailable
  upstream evidence instead of showing one coarse missing count.
- `history export-browser-model` now builds `ProtocolSnapshot` values from each
  branch evaluation's treatment run protocol artifacts.

## What We Built in Phase 3 So Far

WASM compile support is wired, but browser serving/manual test is still open:

- `ploke-tree-egui` has a `wasm32` startup path using `eframe::WebRunner`.
- The browser UI loads JSON by pasted text or dropped JSON files instead of
  filesystem paths.
- `crates/ploke-tree-egui/index.html` is present for Trunk.
- Root `Trunk.toml` points `trunk build` at the egui crate entrypoint.
- `ploke-tree-browser` keeps browser DTO fields backed by `ploke-records`
  passive types. Do not duplicate record enums for the WASM path.
- Native-only dependency edges are gated:
  - `ploke-core/cozo` gates Cozo ID conversion impls.
  - `ploke-records/protocol` gates protocol artifact payloads so the WASM
    viewer does not pull `ploke-protocol`/`ploke-llm`/Tokio networking.
- Verified: `cargo check -p ploke-tree-egui --target wasm32-unknown-unknown`.
- Not yet verified: `trunk build`/`trunk serve` from the workspace root and browser loading of
  `/tmp/browser-model.json`. A plain `trunk build` hit Trunk's `NO_COLOR=1`
  parsing issue in this shell; do not work around it by setting env without
  user approval.

### Phase 2.1 UI Cleanup (DONE)

- [x] Make the selected-step detail panel scrollable.
- [x] Promote operator-facing fields:
  - evaluation disposition
  - tool calls total/failed
  - patch attempted/apply state
  - convergence, oracle eligibility, aborted
  - reasons
  - surface `target_relpath`
- [x] Move raw ids and hashes into collapsed Advanced/Provenance sections.
- [x] Replace footer missing counts with applicability-aware diagnostics:
  - not applicable
  - expected but absent
  - unavailable upstream enrichment
- [x] Keep the UI read-only and projection-only.
- [x] Do not add visible explanatory copy for protocol enrichment absence yet; the
  code comment near the protocol diagnostics is the reminder for the next patch.

### Design Constraints

- The egui crate depends ONLY on `ploke-tree-browser`. It does NOT depend on
  `ploke-eval` or `ploke-tree`.
- JSON is loaded at runtime via a native path input, or in WASM via pasted text
  / dropped JSON file.
- All model types come from `ploke_tree_browser::*`.
- Keep it simple — one file (`main.rs`) is fine for the first slice.
- The app works as a native binary first. The WASM compile path is present;
  browser serving/manual test remains.

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
| `crates/ploke-tree-egui/src/main.rs` | Native/WASM egui viewer for exported `PlaybackBrowserModel` JSON |
| `crates/ploke-tree-egui/index.html` | Trunk entrypoint for the WASM viewer |
| `Trunk.toml` | Workspace-root Trunk config targeting the egui WASM entrypoint |
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
cargo check -p ploke-tree-egui
cargo check -p ploke-tree-egui --target wasm32-unknown-unknown
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
protocol_steps = [s for s in m['steps'] if s.get('protocol')]
assert len(eval_steps) == 46, f'expected 46 eval steps, got {len(eval_steps)}'
assert len(surface_steps) == 46, f'expected 46 surface steps, got {len(surface_steps)}'
assert len(protocol_steps) == 43, f'expected 43 protocol steps, got {len(protocol_steps)}'
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
