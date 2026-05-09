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
  | Renderer-neutral models | `ploke-tree-browser` | `PlaybackBrowserModel` with coarse/fine, serde-serializable |
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

**Playback pipeline verified on run 2:**
```
coarse_history blocks=4 steps=4 warnings=0
block 0 selected candidate:node-52e1449c2e9248d1:plan_index=5 candidates 6 warnings 0
block 1 selected candidate:node-cf3d3023fe7433b9:plan_index=0 candidates 9 warnings 0
block 2 selected candidate:node-66b0882c17912ae7:plan_index=0 candidates 12 warnings 0
block 3 selected candidate:node-66b0882c17912ae7:plan_index=0 candidates 15 warnings 0
```

The pipeline works. Playback is thin — block hashes, candidate counts, and
selected successor ids — but the spine is solid and the typed records are
parseable.

## Operator Questions the UI Must Answer

### Surface Questions (Fast, After Run Completes)

| Question | Data Source | Status |
|---|---|---|
| Is the run still going? | No single heartbeat; needs process table or liveness file | **Missing** |
| Is the loop healthy? | History block chain, journal completeness | **Available** |
| Did the loop exit with expected state or something else? | Terminal journal entry, last block state | **Available** |
| How many children across how many generations? | Scheduler nodes + History blocks | **Available** |
| What are the metrics telling us? (mechanical + LLM) | Evaluation payloads, protocol artifacts | **Available but not projected** |
| What is the tree state and active ruler? | Scheduler + History blocks + successor records | **Partially available** |
| Is the loop improving on metrics? | Trajectory over generations | **Needs aggregation** |
| Are edits making sense? Is ploke-tui working? | Surface evidence records | **Available but not projected** |
| What went wrong and where are the logs? | Journal entries, diagnostics, protocol artifacts | **Available but scattered** |

### Trajectory Questions (Aggregation Over Time)

| Question | Status |
|---|---|
| Score distributions over generations | Needs joining evaluation payloads onto playback steps |
| Tool failure probability distributions | Needs protocol artifact aggregation |
| Timing bottlenecks per phase | **Broken — needs redevelopment on new record infra** |
| Lineage attribution (which parent caused which improvement) | Needs surface evidence → evaluation score join |

## Architecture

### Crate Boundary (from plan)

```
ploke-records        passive schemas, playback vocabulary (no FS)
ploke-tree           load records, join, build RunPlayback projections
ploke-eval           emit authoritative records (not in WASM)
ploke-tree-browser   renderer-neutral browser models (serde)
ploke-tree-egui      egui/eframe app, WASM target (NEW)
```

### Data Delivery Model for WASM

Three options, in order of implementation:

1. **Static snapshot (first slice)**: The WASM app loads a pre-serialized
   `PlaybackBrowserModel` JSON bundle. The bundle is produced by a small CLI
   command in `ploke-eval` or `ploke-tree` that loads a campaign through
   `ploke-tree`, enriches the projection, and writes JSON. Simplest path to a
   working interactive UI.

2. **Local server + polling (second slice)**: A small HTTP server serves JSON
   endpoints backed by `ploke-tree`; the WASM app polls. Enables live-ish
   updates. The server could be `ploke-eval` itself, emitting updated snapshots
   as blocks seal.

3. **Full `ploke-tree` in WASM (deferred)**: Compile `ploke-tree` (with
   `FsRunStore`) to WASM. Hard — filesystem access, async, large dependency
   tree. Only pursue if static snapshots prove insufficient.

For the first slice, we use option 1: a `ploke-eval export browser-model`
command that produces a JSON file, and the egui app loads it.

### What Views First

The data wants progressive disclosure. A natural hierarchy:

```
Run Dashboard
├── Summary bar (run id, status, generation count, total children, elapsed)
├── Timeline view (coarse/fine steps with scores overlaid)
│   └── Expand step → detail panel
├── Tree view (parent → children → successor chain)
│   └── Select node → detail panel
├── Detail panel (drill into one candidate)
│   ├── Evaluation scores (mechanical + LLM)
│   ├── Protocol artifacts summary
│   ├── Surface evidence (what was edited)
│   └── Warnings / diagnostics
└── Aggregation panel
    ├── Score distribution over generations
    ├── Tool failure rates
    └── Timing breakdown (when redeveloped)
```

Rule: **one semantic promise per slice**. First slice: **timeline + detail
panel** because it maps directly onto `FineStep`/`CoarseStep` playback we
already have.

### egui Architecture

An `eframe` app (`ploke-tree-egui`):

- **Top bar**: Campaign/run selector (load JSON), metadata summary
- **Central panel**: Scrollable timeline. Each fine step = a row showing
  kind, evidence strength, block height, label. Click to expand.
- **Side panel** (right): Selected step detail — evaluation scores,
  protocol artifacts, surface evidence, raw record view.
- **Bottom panel**: Warnings/diagnostics strip.

The app crate depends on `ploke-tree-browser` for `PlaybackBrowserModel`,
`PlaybackBrowserStep`, and `BrowserGranularity`. It does not depend on
`ploke-eval` or `ploke-tree` directly (those are used server-side to produce
the JSON).

## Implementation Plan

### Phase 0: Data Shape Verification (delegate, cheap)

Before writing egui code, verify the data shapes we'll need:

- [ ] Load the overnight run 2 through the existing `ploke-tree` projection and
  serialize a real `PlaybackBrowserModel` JSON snapshot.
- [ ] Inspect one evaluation JSON to see what score fields are available
  (mechanical metrics, LLM-adjudicated metrics).
- [ ] Inspect one protocol-artifacts dir to see what typed payloads exist.
- [ ] Check whether surface evidence records (edits) are findable in the run
  data and what shape they have.
- [ ] Report: what fields exist in the real data that `PlaybackBrowserModel`
  does not yet carry, and what would need to be added to `ploke-tree-browser`.

Output: a compact inventory of available enrichment fields, so the browser
model can be extended before the egui crate is built.

**Delegate to**: `gpt-5.4-mini` sub-agent.

### Phase 1: Enrich PlaybackBrowserModel

Based on Phase 0 findings:

- [ ] Add evaluation score fields to `PlaybackBrowserStep` (mechanical metrics,
  LLM metrics, disposition).
- [ ] Add protocol artifact summary fields (intent count, review count,
  tool calls?).
- [ ] Add surface evidence fields (edited paths, patch status).
- [ ] Add a `run_summary` field to `PlaybackBrowserModel` (node count, generation
  count, evaluation counts, status).
- [ ] Wire up `ploke-tree` to produce enriched models from real data.
- [ ] Add a CLI export command: `ploke-eval export browser-model --campaign <name>`
  that writes enriched JSON.

### Phase 2: Create ploke-tree-egui Crate

- [ ] Add `crates/ploke-tree-egui/Cargo.toml` with deps: `eframe`, `egui`,
  `ploke-tree-browser`, `serde`, `serde_json`.
- [ ] Add to workspace `Cargo.toml` members.
- [ ] Implement `PlokeTreeApp` struct with `eframe::App` trait.
- [ ] **Top bar**: Load JSON button, campaign/run name display.
- [ ] **Timeline panel**: Scrollable list of fine playback steps. Each row
  shows step kind icon, block height, label, evidence badge. Click to select.
- [ ] **Detail panel**: Selected step detail. Shows evaluation scores,
  protocol artifact summary, surface evidence if available.
- [ ] **Summary bar**: Run metadata from `PlaybackBrowserModel`.

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

Each phase stops when its one semantic promise is validated:

**Phase 0**: Real data inventory is complete and gaps are documented.
**Phase 1**: `ploke-eval export browser-model` produces enriched JSON that
  contains evaluation scores, protocol summaries, and surface evidence for
  the overnight run 2.
**Phase 2**: `ploke-tree-egui` native app renders timeline + detail panel from
  the exported JSON.
**Phase 3**: WASM build runs in browser with the same data.
**Phase 4**: Each new view (tree, aggregation) is implemented and validated
  against real data.

## No-Goals

- Do not move `ploke-eval`'s write/authority paths into the UI.
- Do not parse rendered CLI output.
- Do not make the UI a control surface for the loop (read-only first).
- Do not introduce a separate JSON format that diverges from typed record shape.
- Do not add live filesystem watching in the first slice.

## Open Questions

- Should the browser model carry raw record data for "drill into raw JSON" views,
  or only projected/typed fields?
- How should multiple eval instances (when we add them) be displayed in the
  detail panel — per-instance rows or aggregated?
- Should the egui app support loading multiple runs for comparison, or single-run
  first?
- Should we add a "diff from baseline" view in the evaluation detail panel?

## Verification Commands (Baseline)

Before starting Phase 0, confirm everything still passes:

```bash
cargo fmt --all
cargo test -p ploke-records
cargo test -p ploke-tree
cargo test -p ploke-tree-browser
cargo check -p ploke-eval
PLOKE_TREE_RUN_ROOT=/home/brasides/.ploke-eval/campaigns/p1-bounded-surface-long-20260509-2/prototype1 \
  cargo test -p ploke-tree fs_run_store_loads_real_campaign -- --ignored --nocapture 2>&1 | tail -n 40
```
