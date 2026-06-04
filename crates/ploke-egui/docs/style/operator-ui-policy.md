# Operator UI policy (`ploke-egui`)

**Start here** when implementing or reviewing operator-facing UI. This is the canonical “style we want” reference for Prototype 1 run inspection—not a generic debugger, not a raw artifact browser.

## 1. Who this is for

- **Primary user:** operator, data analyst, or reviewer triaging **Prototype 1** eval/protocol runs and, when history exists, **self-improvement trajectory** across generations.
- **Success:** the run explains itself from typed evidence; data bugs, protocol gaps, and model behavior are visible without re-explaining policy in chat.
- **Out of scope:** IDE debugging, unconstrained JSON exploration, or UI that invents meaning the graph does not carry.

## 2. North-star questions

Split questions by what the loaded graph can answer. Never collapse them into one dashboard story.

### (a) Eval / protocol — “what happened on this instance?”

- Did the run close cleanly? What is lifecycle, coverage, failure mix, and patch production?
- Which tool calls failed, mixed, or look redundant? Where is typed call/segment review evidence?
- Can the operator reach proof (record paths, protocol artifacts, rationales) without scrolling an accordion forest?

**Primary surfaces when** `history_blocks` is empty **and** eval/protocol evidence is available: center **Eval & Protocol**, right **Inspector** for selected call/segment/instance detail.

### (b) Trajectory — “did self-edits improve over time?”

- How do generations compare on selection decisions, outcomes, and improvement signals (e.g. `score_child_prop`, `imp@k` when present)?
- Where is Selection Story and the generation table meaningful?

**Primary surface when** `trajectory_generations()` is non-empty: center **Trajectory** preset. **Do not** expect trajectory north-star answers on a protocol-only export (`history_blocks: []`).

### (c) Artifact lineage — “what is selected or promoted, and what is nearby?”

- Which artifact path led here? What siblings or alternatives exist? Where to drill next?

**Primary surface when** neither trajectory nor eval/protocol pane preference applies: center **Artifact tree** canvas (default graph mode `ArtifactTree`), right **Inspector** for selection facts.

## 3. Semantic boundary

- **`ploke_tree::Graph` owns meaning.** UI owns presentation, interaction state, layout geometry, inspector chrome, and render-only diagnostics.
- **Presentation only in `ploke-egui`:** borrow or cache display projections; do not reparse run artifacts, protocol JSON, History JSON, or report text for semantic claims.
- **Pipeline:**

  ```text
  ploke-records / ploke-protocol → ploke-tree::Graph → borrowed inspector view → egui renderer
  ```

- **Never** allocate or clone semantic ids, refs, records, partial records, or row-shaped **mirror types** (`Option<SomeReport>`, DTO vectors rebuilt each frame) for later semantic use. If a claim cannot be proven from the graph, add or expose a graph witness—do not infer in the egui layer.
- **Allowed derivation:** counts, layout coordinates, readability metrics, display labels, and other clearly derived scalars—not renamed copies of graph facts.

## 4. Default surfaces

| Surface | Role | When primary |
|--------|------|----------------|
| **Artifact tree** (center canvas) | Artifact nodes `A`, edges `P_H ∪ P_C`; History as reveal/highlight, not canvas spine | Default when trajectory and eval/protocol preferences do not apply |
| **Eval & Protocol** (center) | Run dashboard: health, coverage, problem slices, scannable call/segment lists | `prefers_eval_protocol_pane`: empty sealed history + eval/protocol evidence |
| **Trajectory** (center) | Generation table, campaign/policy header, Selection Story when backed | `prefers_trajectory_pane`: non-empty `trajectory_generations()` |
| **Inspector** (right) | Selected graph node **or** run-level protocol/call/segment detail | Always the detail pane; must not be empty `not_applicable` when eval/protocol selection exists |
| **Bottom timeline** | Compact sealed/causal order (secondary lane) | Supporting context; not a log dump |

**Layout contract:** graph canvas remains the largest horizontal region (≥50% of initial width with side panels at max). Left sidebar summarizes and filters; it is **not** a semantic source.

**Mode rules:**

- Default canvas shows **artifacts**, not debug/runtime/tool/provider nodes as primary geometry.
- Full composed graph / record inventory belongs in explicit debug or drilldown modes—not the default tree.
- Eval & Protocol: **overview first**, problem slices second, typed evidence third, raw payloads last (see analyst surface plan).

## 5. Inspector and disclosure

Follow [`inspector-ux.md`](inspector-ux.md) for patterns; policy summary:

- **Evidence first, not rows first.** Every row must help answer what happened, why, and where proof lives.
- **Progressive disclosure.** Long prose, payloads, rationales, summaries, and traces live behind obvious collapsibles—not clipped one-line previews.
- **Headers stay meaningful.** Do not replace section titles with `details`, `not_applicable`, or truncated preview text.
- **Typed fields over generic parsing.** Use `ploke-records`, `ploke-protocol`, and graph witnesses. **Never** build a `serde_json::Value` field walker in `ploke-egui`.
- **Raw / Fields modes** when payloads exist: prefer **Fields** when typed decoders succeed; **Raw** copyable and secondary; explain truncation (e.g. `result_preview` is not full tool result).
- **State discipline:** missing, truncated, parse-failed, not-applicable, not-recorded, and **zero** are distinct—label each explicitly.
- **Provenance is UI data:** paths, artifact keys, run ids—compact by default, expandable, **copyable** (tail display + full copy affordance).
- **Performance:** no per-frame reconstruction of strings, parsed payloads, or row vectors; borrow graph facts or cache behind stable keys.
- **Analyst affordances:** charts and summaries must preserve drilldown to underlying evidence; dense-table hover stays lightweight—full LLM prose in the selected detail pane.

**Inspector resolution path:**

```text
selection → GraphSelectionRef → borrowed typed graph answer → renderer
```

Unavailable drilldowns show `missing`, `blocked`, or `not_applicable` with contract id—not empty panels.

## 6. Visual cues and drilldown ladders

**Goal:** more **at-a-glance** structure so operators see status before reading prose. Every cue must either (a) answer a north-star question in &lt;3 seconds, or (b) invite one obvious next click into typed drilldown—not decoration.

### Drilldown ladder (default pattern)

Use the same four layers everywhere; do not skip layers by dumping proof in the center pane.

```text
Glance (dashboard / header / lane / chip)
  → Slice (filter, sort, problem bucket, table row)
    → Row summary (collapsed inspector header or list cell)
      → Proof (Inspector: Fields / Raw, paths, trace chain)
```

- **Glance** must work when children are collapsed (verdict color, bar length, segment color, count badge).
- **Slice** changes *what* is selected, not *what* the graph means—filters come from graph witnesses or precomputed dashboard counts.
- **Row summary** carries semantic labels (`helpful`, `recoverable detour`, `failed`) with token-backed emphasis—not repeated plain strings.
- **Proof** stays in the **Inspector** (or an explicit expand), especially LLM prose and stack traces.

### Cue types that fit today (extend, do not replace)

| Cue | Glance question | Typical drilldown |
|-----|-----------------|-------------------|
| **Horizontal bar charts** (Analyst Snapshot) | How heavy was the run? How much protocol coverage? What outcome mix? | Click bar segment or legend → filter Call Review / instance list → Inspector call or segment |
| **Verdict chips + emphasis** (usefulness / redundancy / recoverability / overall) | Is this call worth opening? | Chip row in Eval spotlight or scan grid → Inspector typed assessment → Raw only if Fields fails |
| **Outcome / severity tags** on collapsed rows | Pass, warn, error, failed tool—without expanding | Row click → Inspector tool step or protocol detail |
| **Bottom timeline lanes** (turns / tool calls / protocol) | Where do failures cluster in time? | Segment click → select turn or call index → Inspector LLM trace or run record turn |
| **Trajectory generation table** | Which gen improved? trend label | Row select → Selection Story + metric witness in Inspector |
| **Call Review Scan grid** (warn/error cell tint) | Which calls need eyes? | Cell or row → Inspector review artifact (coordinate-aware) |
| **Lifecycle / coverage status rows** (dashboard header) | Is eval/protocol/closure done? anything missing? | Status line → collapsible run records / protocol section → path copy |

### Cues to add (natural next wave)

Prefer these over new text sections or wider accordions.

| Cue | Why it helps | Drilldown target |
|-----|--------------|------------------|
| **Problem-slice chips** with counts (failed tools, mixed, redundant thrash, truncated preview, missing review) | Points investigation; empty slice shows `0` or `missing` explicitly | Slice click → filtered call/segment list → Inspector |
| **Sparkline or mini-bar per trajectory row** for `score_child_prop` / `imp@k` when witness exists | Trend at a glance across generations | Row → Selection Story formula rows |
| **Call-index heat strip** (one pixel/color per tool call by outcome class) | Failure density without scrolling 47 rows | Click band → scroll list + select call |
| **Icon + color on collapsible headers** (pass / warn / error / missing) | Collapsed state communicates status per inspector-ux | Expand → Fields |
| **Comparison glyph** (▲/▼/=) beside generation scores | Self-improvement north star without reading numbers | → metric witness + branch rationale |
| **Packaging tension badge** (e.g. aborted turn + nonempty submission) | Mechanical vs useful in one glance | → run record packaging + disk hint for validation-audit |
| **Provider vs recorded parity strip** (counts only, from audit summary at export) | Catches ledger drift early | → link/copy run root; optional future ingest panel |
| **Artifact-tree edge handles** (`P1`, `P2`) + subtle promotion highlight | Lineage legibility on canvas | Node select → Inspector artifact identity + derivation drilldown |

### Representation rules

- **Prefer length, position, and hue** over new numeric columns when the question is comparative (outcome mix, failure rate, trend).
- **Use theme semantic tokens** only (`warn`, `error`, `success`, `muted`)—see [`theme.md`](theme.md); cues must stay readable in both light and dark.
- **Charts must preserve drilldown** (eval-protocol plan): a bar segment or slice without a selection action is incomplete.
- **Hover stays light:** no full rationale on hover; at most tooltip with count + label—prose in Inspector.
- **Do not** use pie charts for many categories, animated transitions on large tables, or per-frame chart rebuilds without cache keys.
- **Missing data gets a visual role:** dashed outline, `—`, or `not_recorded` chip—not empty space identical to zero.

### Anti-patterns (visual)

- Color-only status with no collapsed label (accessibility + canvas limits).
- Giant accordion lists with no slice or chart above them.
- Repeating the same verdict string forty times without emphasis hierarchy.
- Timeline lanes that look decorative but do not change selection on click.

## 7. Error and provenance lanes

Failures must name **which layer** produced the evidence so operators do not confuse snapshot load errors, WASM decode limits, and recorded harness tool failures.

| Lane chip | Operator question | Trustworthy action |
|-----------|-------------------|-------------------|
| `UI · snapshot load` | Did the UI fail to load the graph? | Fix URL/file picker/import; ignore tool trees until load succeeds |
| `UI · decode (WASM)` | Why is decode missing? | Read raw JSON; this is not a harness failure |
| `UI · decode failed` / `decode ok` | Did native UI decode tool JSON? | Use Fields when ok; use Raw when failed |
| `Recorded · tool failed` | Did the agent’s tool actually fail? | Trust harness headline + typed error wire |
| `Recorded · completed` | Did the tool succeed in the export? | Drill into payload/result as needed |
| `Recorded · typed error wire` | What structured error was exported? | Prefer user line; keep system/LLM wire in drilldown |
| `Recorded · raw payload` | What bytes were persisted? | Copy raw; do not reinterpret in UI |
| `Graph · not in export` | Is a graph witness missing? | Do not infer; load richer snapshot or disk witness |

**Run context strip** (Source / Host / Decode) appears before run-record and LLM tool trees. **Top-level banner** mirrors `GraphCatalog` snapshot load errors on WASM (not sidebar-only).

Headline selection for failed tools: `error.user` → `ui_payload.summary` → `result.error`, labeled **Harness error (recorded)**.

## 8. Labels and trust

Counters and labels must not lie about what the operator is viewing.

- **Sidebar `selections`** counts **sealed selection-generation history** (`graph.selections`), not “how interesting the run is.” **`selections 0` on a protocol export is expected** when `history_blocks` is empty—it does **not** mean an empty run or missing eval/protocol data.
- **Sidebar `artifacts`** counts graph artifact registry size, not eval-instance richness (e.g. 47 reviewed calls with `artifacts 0` is a labeling bug, not operator error).
- **Dashboard “registry”** reflects **campaign registry closure status**, not per-run `registries/runs/*.json` content—do not imply file-level registry inspection.
- **Trajectory `score_child_prop` / `trend`:** show `-` or explicit not-recorded when formula rows are absent; never fabricate scores from instance-level protocol success alone.
- **WASM snapshot vs native `--run-root`:** label provenance when paths are embedded absolutes in exports; operator must know snapshot dogfood vs live disk layout.

Prefer renaming sidebar metrics (“sealed selections”, “graph artifacts”) over ambiguous single words when counts stay graph-scoped.

## 9. Run-review ergonomics

Run reviews join graph evidence with disk-only witnesses. UI policy:

- **Evidence roots strip (goal):** copyable primary paths from closure/refs (`record_path`, `run_root`, `protocol_artifacts_dir`, registration paths)—secondary visually, not buried in accordions. Snapshot exports may embed absolute paths; surface them for review workflows even when the browser cannot read disk.
- **In graph + UI today:** parsed `RunRecord`, protocol artifact bodies, Analyst Snapshot / Call Review Scan, closure status, turn/tool lifecycle from record, patch diff when on `RunRecord.phases.patch`.
- **Graph partial / UI gap:** consolidated provenance strip; `validation-audit.json`; `llm-full-responses.jsonl`; full benchmark/MSB projection bodies; checkout `git diff`; `run_trace_audit.py`; per-run registry JSON semantics.
- **Aborted vs useful tension:** protocol reviews can be rich while validation or patch witnesses remain disk-only—show submission/projection **states** and explicit “needs disk” for audit/checkout diff, not silent success.
- **Mechanical closure ≠ useful progress:** distinguish protocol coverage, tool failures, patch attempted/applied, and selection outcomes; do not let green closure badges subsume failed tools or empty trajectory metrics.

## 10. WASM vs native

| Context | Load path | Default fixture |
|---------|-----------|-----------------|
| **WASM dogfood** | `trunk serve` → fetch URL | `/` → `protocol-graph.json`; trajectory QA → `?graph=trajectory-multi-gen.json` |
| **Native dev** | `--run-root`, file picker, dev features | Same fixture basenames under `benchmark-fixtures/`; live campaign roots for full evidence |

- **Trunk auto-start (agents):** if `http://127.0.0.1:8080` is not up, start trunk from `crates/ploke-egui` before dogfood or browser review — do not skip WASM checks or mark them **N/A** because the server was down:

  ```bash
  cd crates/ploke-egui && env -u NO_COLOR trunk serve --config Trunk.toml --address 127.0.0.1 --port 8080
  ```

  Run in background; wait until `curl -sf http://127.0.0.1:8080/` returns HTTP 200 (max ~90s).
- **Must** show load-in-progress or explicit empty state during large snapshot fetch (~8 MB protocol export)—bare `/` must not look broken while ingesting.
- **Prefer** in-app pointer to multi-gen fixture when trajectory table is empty on protocol export.
- Native smoke: `cargo check -p ploke-egui`, `timeout 15 cargo run -p ploke-egui` (no panic) for non-UI verification; WASM visual checklist after trunk is serving `:8080` (start trunk if needed).

Theme tokens: see [`theme.md`](theme.md)—semantic colors from palettes only, not ad-hoc `Color32::from_rgb` for status roles.

**Theme switch:** changing the top-strip palette must not show garbled or corrupted text (stale font atlas / cached galleys). The app clears inspector and diff render caches, requests a single multipass discard via `on_theme_changed`, and **returns early** from the frame so pass 1 does not paint the dashboard until pass 2 repaints with the new visuals.

## 11. Anti-patterns

- **Inventory-only UI:** default canvas or side panel as a flat record class list instead of artifact lineage or analyst questions.
- **Raw JSON soup:** generic walkers, clipped one-line payloads as primary view, walls of absolute paths without tail+copy.
- **Mirror semantics:** inspector rows or snapshots that re-own graph ids/records for convenience; `PlaybackBrowserModel` copies as source of truth.
- **History as default spine:** sealed History chain replacing artifact tree on first paint.
- **Silent absence:** empty panel where drilldown is missing; zero conflated with N/A; truncated preview presented as full record.
- **Per-frame allocation** in hot inspector/table paths without measurement on large surfaces.
- **Misleading counters** (sidebar `artifacts` / `selections` vs populated Eval & Protocol).
- **Background browser UX review subagents** with trunk + MCP—they time out; run checklist in **foreground** per [`.cursor/rules/ploke-egui-ux-review.mdc`](../../../../.cursor/rules/ploke-egui-ux-review.mdc).

## 12. Related docs

| Doc | Use |
|-----|-----|
| [`inspector-ux.md`](inspector-ux.md) | Disclosure patterns, copy rules, anti-patterns checklist |
| [`theme.md`](theme.md) | Palette tokens and theme switching |
| [`../model/default-view-contract.md`](../model/default-view-contract.md) | Frame layout, artifact-tree invariants, testable contract |
| [`../plan/eval-protocol-analyst-surface/main-plan.md`](../plan/eval-protocol-analyst-surface/main-plan.md) | Dashboard slices, call/segment lists, chart drilldown intent |
| [`../refs/selection-metrics.md`](../refs/selection-metrics.md) | Selection-time metrics and graph propagation gaps |
| [`../../../../docs/active/agents/ploke-ui-task-readability/README.md`](../../../../docs/active/agents/ploke-ui-task-readability/README.md) | Graph-owns-meaning coordination |
| [`../../../../docs/active/agents/ploke-ui-task-readability/artifact-tree-default/README.md`](../../../../docs/active/agents/ploke-ui-task-readability/artifact-tree-default/README.md) | Artifact-first default geometry |
| [`../../../../docs/active/agents/2026-06-03_ploke-egui-wasm-ux-review/README.md`](../../../../docs/active/agents/2026-06-03_ploke-egui-wasm-ux-review/README.md) | Dogfood findings (labels, load affordance, trajectory columns) |
| [`../../../../docs/active/agents/2026-06-03_ploke-egui-run-review-evidence-access/README.md`](../../../../docs/active/agents/2026-06-03_ploke-egui-run-review-evidence-access/README.md) | Evidence vs graph vs UI matrix |
| [`../../../../docs/active/agents/2026-06-03_ploke-egui-wasm-ux-checklist/README.md`](../../../../docs/active/agents/2026-06-03_ploke-egui-wasm-ux-checklist/README.md) | Full UX review checklist |

**Review before merge (UI):** Does the change answer an operator north-star question? Are labels truthful? Are missing states explicit? Is meaning still graph-owned?
