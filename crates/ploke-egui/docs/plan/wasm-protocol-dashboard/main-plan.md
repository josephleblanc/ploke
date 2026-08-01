# ploke-egui WASM Protocol Dashboard Implementation Plan

> **For Hermes:** Use subagent-driven-development skill to implement this plan task-by-task. Do not combine tasks unless an explicit gate says the previous task is complete.

**Goal:** Make `ploke-egui` the typed, WASM-friendly Prototype 1 protocol/debug frontend without violating the graph reference, allocation, performance, or UI-contract rules.

**Architecture:** Add typed borrowed dashboard/inspector projections over `&ploke_tree::Graph` first, then render those projections in egui, then add a real `ploke-egui` WASM/Trunk shell and browser smoke loop. `ploke_tree::Graph` remains the semantic source; egui rows/text are render-boundary artifacts only.

**Tech Stack:** Rust 2024, `ploke-tree`, `ploke-records`, `eframe`/`egui`, `egui_tiles`, `trunk`, `wasm32-unknown-unknown`, existing `ploke-egui` dev/native-benchmark/profiling features.

---

## Non-Negotiable Rules

1. `ploke-egui` must not derive semantic facts from CLI output, monitor text, raw run JSON, compressed records, copied path strings, copied labels, or widget strings.
2. Protocol/eval/selection/run facts must enter through typed records folded into `ploke_tree::Graph` or graph-owned typed projections.
3. Typed dashboard/inspector structs must borrow from `Graph` where possible. Owned `String` is allowed only for true UI artifacts, not semantic witnesses.
4. Visual rows are renderer-local only. No `InspectorRow`, `Vec<Row>`, or text-shaped carrier may become the intermediate semantic model.
5. Missing evidence must render as explicit `missing`, `blocked`, or `not_applicable` status, never as an empty panel.
6. Render-path work must not introduce recurring `to_string`, `format!`, clone, serde field-walking, or JSON parsing churn without a measured reason and review approval.
7. Default `ArtifactTree` invariants remain intact: center nodes are `A`, center edges are `P_H union P_C`, debug/protocol records do not become default canvas nodes.
8. Every code task uses TDD: failing test first, minimal implementation, passing test, then review.
9. Every task has two reviews: spec compliance first, then quality/perf/reference review.
10. WASM compatibility is checked before browser dogfood; browser dogfood starts only after a typed UI path exists.

## Source Documents To Keep Open

- `crates/ploke-egui/docs/model/graph-pipeline.md`
- `crates/ploke-egui/docs/model/default-view-contract.md`
- `crates/ploke-egui/docs/model/view-set-contract.md`
- `crates/ploke-egui/docs/model/source-process-graph.md`
- `crates/ploke-egui/docs/model/debugger-claim-workflow.md`
- `crates/ploke-egui/docs/model/protocol-and-evaluation-data-locations.md`
- `crates/ploke-egui/docs/profiling/setup.md`
- `crates/ploke-egui/docs/profiling/benchmarks/README.md`
- `crates/ploke-eval/docs/prototype1-loop-operator.md`
- `crates/ploke-eval/docs/prototype1-run-profile.md`
- `crates/ploke-eval/docs/knobs/timeouts.md`

## Gate Taxonomy

| Gate | Type | Trigger | Failure behavior | Resume point |
|---|---|---|---|---|
| Repo pre-flight | Pre-flight | Before any task | Stop if not `/home/brasides/code/ploke` or intended repo missing. Do not switch checkout. | User fixes repo path or approves different path. |
| Dirty-worktree scope | Pre-flight | Before edits | Continue only if new edits are limited to declared files and do not overwrite unrelated dirty files. | Re-check `git status --short`. |
| TDD red gate | Revision | Each code task | If the test does not fail for the expected reason before implementation, rewrite the test. | Same task, test step. |
| Spec gate | Revision | After implementer finishes | Reviewer checks task must-haves. Fix only gaps. Max 3 loops. | Same task, implementer/fix subagent. |
| Reference/provenance gate | Revision | Quality review | Fail on row-shaped semantic carriers, semantic string copies, raw parsing, or UI-owned semantics. | Same task, targeted fix. |
| Allocation/perf gate | Revision / Escalation | Quality review and benchmark-relevant edits | Fail on new per-frame string/clone/serde churn unless moved to cache/projection or measured. Escalate if tradeoff is intentional. | Same task or user decision. |
| WASM gate | Pre-flight / Revision | Before browser work and after wasm-sensitive edits | Fix compile errors or dependency leaks. | Same task. |
| UI contract gate | Revision | After render/diagnostic edits | Fail if default-view contract regresses or missing statuses vanish. | Same task. |
| Browser dogfood gate | Pre-flight / Revision | Browser task only | Verify Trunk target/listener/canvas/console/screenshot; fix or report exact blocker. | Browser task. |
| Context pressure gate | Abort | Orchestrator context degrades | Checkpoint and stop rather than continue sloppily. | Fresh session with plan + status. |

## Standard Commands

Use narrow commands first, then broader commands after green.

```bash
cargo test -p ploke-egui eval_protocol_dashboard -- --nocapture
cargo test -p ploke-egui --features dev default_view_contract
cargo check -p ploke-egui --target wasm32-unknown-unknown
cargo check -p ploke-egui
cargo run -p ploke-egui --features dev -- --run-root /home/brasides/.ploke-eval/campaigns/p1-five-gen-1x3-20260516-1/prototype1 --contract-report
cargo run -p ploke-egui --features dev -- --perf-log --run-root /home/brasides/.ploke-eval/campaigns/p1-five-gen-1x3-20260516-1/prototype1
```

Native benchmark only for render-path/layout/cache changes with plausible performance impact:

```bash
cargo run -p ploke-egui --features "dev native-benchmark" -- \
  --run-root /home/brasides/.ploke-eval/campaigns/p1-five-gen-1x3-20260516-1/prototype1 \
  --benchmark-suite standard
```

Browser dogfood only after WASM shell work:

```bash
cargo build -p ploke-egui --target wasm32-unknown-unknown
trunk build --config <ploke-egui-trunk-config>
trunk serve --address 127.0.0.1 --port 8080 --config <ploke-egui-trunk-config>
```

Readiness must be verified with `ss` and `curl`, not Trunk's `success` log alone.

## Static Slop Scan

Every quality review must inspect changed files for these patterns and explain each occurrence:

```text
.to_string(
.to_owned(
format!(
clone(
serde_json::Value
serde_json::to_value
serde_json::from_
std::fs
File::open
read_to_string
InspectorRow
Vec<.*Row
String,
Arc<str>
```

These are not automatically forbidden everywhere. They are suspect in projection, inspector, and per-frame render paths. A reviewer must decide whether each is a final-render artifact, a cached artifact, or a semantic/reference violation.

## Task 0: Plan And Scope Check

**Objective:** Land this plan without touching unrelated dirty files.

**Files:**
- Create: `crates/ploke-egui/docs/plan/wasm-protocol-dashboard/README.md`
- Create: `crates/ploke-egui/docs/plan/wasm-protocol-dashboard/main-plan.md`
- Modify: `crates/ploke-egui/docs/plan/README.md`

**Verification:**

```bash
git status --short crates/ploke-egui/docs/plan
git diff -- crates/ploke-egui/docs/plan
```

**Done when:** The plan exists, the gate matrix is explicit, and unrelated existing dirty docs are untouched.

## Task 1: Add Typed Eval/Protocol Dashboard Projection

**Objective:** Create a pure, typed projection over `&ploke_tree::Graph` that reports eval/protocol availability and summary counts without egui rows or semantic string copies.

**Files:**
- Create: `crates/ploke-egui/src/ui/eval_protocol.rs`
- Modify: `crates/ploke-egui/src/ui/mod.rs`
- Do not modify: `crates/ploke-egui/src/ui/app/shell.rs` except in later render task.

**Design:**

Introduce a small borrowed projection, names may be adjusted during implementation but must preserve the shape:

```rust
pub(crate) struct EvalProtocolDashboard<'g> {
    evidence: ploke_tree::EvalProtocolEvidence<'g>,
    availability: EvalProtocolAvailability,
    protocol_stats: ploke_tree::ProtocolReviewStats,
    patch_stats: ploke_tree::EvalPatchStats,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum EvidenceState {
    Available,
    Missing,
    NotApplicable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct EvalProtocolAvailability {
    pub closure: EvidenceState,
    pub run_records: EvidenceState,
    pub protocol_artifacts: EvidenceState,
}
```

If `ProtocolReviewStats` / `EvalPatchStats` cloning is too costly or semantically wrong, use borrowed accessors and computed scalar methods instead. Do not introduce owned strings to label protocol verdicts in this task.

**Step 1: Write failing tests**

Add tests in `eval_protocol.rs` under `#[cfg(test)]`:

```rust
#[test]
fn eval_protocol_dashboard_reports_all_sources_missing_for_empty_graph() {
    let graph = ploke_tree::Graph::default();
    let dashboard = EvalProtocolDashboard::from_graph(&graph);
    assert_eq!(dashboard.availability().closure, EvidenceState::Missing);
    assert_eq!(dashboard.availability().run_records, EvidenceState::Missing);
    assert_eq!(dashboard.availability().protocol_artifacts, EvidenceState::Missing);
    assert!(!dashboard.is_available());
}
```

Add a second test that constructs the projection and asserts it does not expose row/string carriers. This can be a behavioral API test: access typed states/counts only; no `rows()` or `labels()` method should be needed.

**Step 2: Verify RED**

```bash
cargo test -p ploke-egui eval_protocol_dashboard_reports_all_sources_missing_for_empty_graph -- --nocapture
```

Expected: fail because module/type does not exist.

**Step 3: Minimal implementation**

- Add `pub(crate) mod eval_protocol;` to `src/ui/mod.rs`.
- Implement `EvalProtocolDashboard::from_graph(&Graph)`.
- Implement typed availability accessors.
- Keep construction cheap and non-rendering.

**Step 4: Verify GREEN**

```bash
cargo test -p ploke-egui eval_protocol_dashboard -- --nocapture
cargo check -p ploke-egui --target wasm32-unknown-unknown
```

**Task-specific gates:**

- Reference gate: projection must be built from `graph.eval_protocol_evidence()` only.
- Row gate: no row/text/vector-of-row semantic API.
- Allocation gate: no `format!`, no per-frame labels, no raw JSON parsing.
- WASM gate: `cargo check -p ploke-egui --target wasm32-unknown-unknown` passes.

## Task 2: Move Existing Eval/Protocol Rendering To Projection Input

**Objective:** Update `render_eval_protocol_for_graph` so it first builds/borrows `EvalProtocolDashboard` and renders from typed projection accessors.

**Files:**
- Modify: `crates/ploke-egui/src/ui/app/shell.rs`
- Modify: `crates/ploke-egui/src/ui/eval_protocol.rs`

**Step 1: Write failing tests**

Add projection tests for the exact facts the renderer currently shows:

- empty graph => dashboard not available;
- protocol stats accessor returns typed counts;
- patch stats accessor returns typed counts;
- candidate protocol dirs status is exposed as typed status if implemented in this task, otherwise explicitly deferred.

**Step 2: Verify RED**

Run the specific new tests and confirm missing accessors fail.

**Step 3: Minimal implementation**

- Renderer may create final labels in `shell.rs` only.
- Any `format!` for wall-clock or count-map labels stays at render boundary and should be documented in code review.
- Do not move `serde_json::to_value` label generation into the dashboard. If label generation is needed, prefer graph/tree typed helpers or renderer-only cached label functions.

**Step 4: Verify GREEN**

```bash
cargo test -p ploke-egui eval_protocol_dashboard -- --nocapture
cargo test -p ploke-egui --features dev default_view_contract
cargo check -p ploke-egui --target wasm32-unknown-unknown
```

**Task-specific gates:**

- UI contract gate: `Eval & Protocol` still renders `not_available` for empty evidence.
- Allocation gate: new render-boundary formatting is no worse than existing and is isolated.
- Reference gate: no direct file/JSON reads in `shell.rs`.

## Task 3: Add Contract Report Fields For Eval/Protocol Availability

**Objective:** Make protocol dashboard availability visible in a headless contract/diagnostic report before browser testing.

**Files:**
- Inspect first: `crates/ploke-egui/src/diagnostics/default_view/mod.rs`
- Inspect first: `crates/ploke-egui/src/diagnostics/default_view/components.rs`
- Inspect first: `crates/ploke-egui/src/diagnostics/default_view/text.rs`
- Modify only the minimal diagnostic files needed.

**Test-first behavior:**

Add a test that an empty graph reports eval/protocol evidence as missing, and a loaded graph report can include counts/status without the UI owning semantics.

**Verification:**

```bash
cargo test -p ploke-egui --features dev default_view_contract -- --nocapture
cargo run -p ploke-egui --features dev -- --run-root /home/brasides/.ploke-eval/campaigns/p1-five-gen-1x3-20260516-1/prototype1 --contract-report
```

**Gates:**

- Diagnostic report is render/contract projection only, not a semantic mirror of `Graph`.
- Missing states are explicit.

## Task 4: Add Performance/Allocation Guardrail Note For Protocol Projection

**Objective:** Record whether the first protocol projection/render changes plausibly affect performance and what was measured.

**Files:**
- Create: `crates/ploke-egui/docs/profiling/benchmarks/YYYYMMDD-eval-protocol-dashboard-benchmark-note.md`
- Modify: `crates/ploke-egui/docs/profiling/benchmarks/README.md`
- Modify if measured impact is meaningful: `crates/ploke-egui/docs/profiling/performance-change-log.md`

**Verification:**

At minimum:

```bash
cargo run -p ploke-egui --features dev -- --perf-log --run-root /home/brasides/.ploke-eval/campaigns/p1-five-gen-1x3-20260516-1/prototype1
```

If render path changed materially, run native benchmark standard suite.

**Gates:**

- Note names exact verification surface.
- Note states whether baseline comparison is valid.
- Note states residual risk, especially wasm/browser not measured if true.

## Task 5: Add ploke-egui WASM Trunk Shell

**Objective:** Serve the real `ploke-egui` app in the browser rather than root `ploke-tree-egui`.

**Files:**
- Inspect first: `crates/ploke-egui/src/web.rs`
- Create likely: `crates/ploke-egui/index.html`
- Create likely: `crates/ploke-egui/Trunk.toml`
- Modify only if required: `crates/ploke-egui/Cargo.toml`

**Test-first behavior:**

For config-only shell files, TDD may be a scripted smoke instead of a unit test:

```bash
cargo build -p ploke-egui --target wasm32-unknown-unknown
trunk build --config crates/ploke-egui/Trunk.toml
```

**Gates:**

- Root `Trunk.toml` target confusion must be documented in browser test notes.
- No `ploke-tree-egui` model or `PlaybackBrowserModel` dependency is introduced.
- Browser page title/labels identify `ploke-egui`.

## Task 6: Browser Dogfood Smoke For Typed Protocol Surface

**Objective:** Verify the real `ploke-egui` browser target loads, has a healthy canvas/WebGL context, and shows the typed eval/protocol dashboard state.

**Files:**
- Modify only docs/benchmark/browser note unless a bug is found.

**Procedure:**

Follow `ploke-wasm-egui-dogfood` exactly:

1. Start tracked `trunk serve` for `crates/ploke-egui` target.
2. Verify listener with `ss` and `curl`.
3. Navigate browser.
4. Check console.
5. Inspect title/body/canvas dimensions.
6. Check WebGL context if screenshot is blank.
7. Capture screenshot/vision evidence.
8. Stop server and verify port is free.

**Gates:**

- Do not claim browser success from build success alone.
- Do not claim blank UI until console/canvas/WebGL/repaint are checked.
- Server cleanup is mandatory unless user asks to leave it running.

## Task 7: Continue With Protocol Drilldown Slices

Only after Tasks 1-6 are green, add further slices:

- selected protocol artifact summary;
- provider/model/token-budget context;
- unsupported structured-output status;
- candidate/selection protocol metrics;
- issue-detection and intervention-synthesis drilldowns;
- timeline availability/status for protocol/eval spans.

Each slice gets its own typed projection test, render-boundary integration, perf/reference review, wasm check, and browser evidence if user-visible.

## Per-Task Review Prompt Template

Spec reviewer must check:

```text
- Did the task implement exactly the named files and behavior?
- Did it preserve `Graph` as semantic source?
- Did it keep missing/blocked/not_applicable explicit?
- Did it avoid unrelated cleanup/refactors?
- Did it run the required commands?
Verdict: PASS or specific gaps.
```

Quality/perf/reference reviewer must check:

```text
- Any semantic data carried as String/rows/widget payloads?
- Any raw JSON/CLI/file parsing in ploke-egui UI code?
- Any per-frame `to_string`, `format!`, clone, serde field walking, or map rebuild?
- Any wasm-incompatible dependency or cfg leak?
- Any default ArtifactTree contract drift?
- Are tests focused and behavior-first?
- Are performance notes/benchmarks sufficient for changed surface?
Verdict: APPROVED or REQUEST_CHANGES with blocking issues.
```

## Current Pre-flight Observation

At plan creation time, the repo had unrelated dirty docs outside `crates/ploke-egui`. The workflow must not overwrite or depend on those files. Use path-limited diffs for this plan and implementation.
