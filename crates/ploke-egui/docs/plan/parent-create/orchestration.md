# Parent Create Orchestration

Companion to [`main-plan.md`](main-plan.md). This document is the restart and
delegation contract for implementing the Parent Create Attempt UI. It defines
how the orchestrator evaluates "done", how workers are assigned, and how the
`xtask orchestrate` board keeps the work from drifting.

## Source Anchors

Every worker packet for this effort should point at these docs:

- [`main-plan.md`](main-plan.md): product target and UX ladder.
- [`../../model/default-view-contract.md`](../../model/default-view-contract.md):
  current `ploke-egui` layout, inspector, and timeline contract.
- [`../../model/source-process-graph.md`](../../model/source-process-graph.md):
  Prototype 1 process model and graph source boundary.
- [`../../../../../docs/active/plans/self-improvement-loop/typed-persistence-spine/ui-drilldown-contract.md`](../../../../../docs/active/plans/self-improvement-loop/typed-persistence-spine/ui-drilldown-contract.md):
  UI answer contracts for tool calls, provider attempts, patch evidence, and
  timeline spans.
- [`../../../../../docs/active/plans/self-improvement-loop/typed-persistence-spine/implementation-slices.md`](../../../../../docs/active/plans/self-improvement-loop/typed-persistence-spine/implementation-slices.md):
  current typed-persistence implementation order. The first slice for this
  plan is `run-execution-graph.browser-spine`.
- [`../../../../../docs/workflow/evalnomicon/drafts/edit-surface/model.md`](../../../../../docs/workflow/evalnomicon/drafts/edit-surface/model.md):
  `SurfaceGrant`, proposal, check, apply, and History-admission model.

Use `p1-broad-harness-retry-20260514-1` as the real-run fixture for manual and
ignored smoke verification. Deterministic tests should use synthetic fixtures
unless the test is explicitly marked as environment-dependent.

## Boundary Rule

The implementation must keep this shape:

```text
typed records / RunRecord / trace projections / edit-surface records
  -> ploke-tree RunRecordSet / Graph evidence
  -> borrowed inspector and timeline projections over &Graph
  -> egui render or immediate export
```

`ploke-egui` may own presentation state, layout state, expanded/collapsed
state, hover state, and renderer-local labels. It must not own semantic mirror
records for parent-create attempts, tool calls, provider attempts, chat turns,
surface grants, proposal ids, patch ids, or source refs. Diagnostics and
exports should serialize directly from borrowed projections and typed values;
they should not introduce owned semantic `String` or `Arc<str>` models.

## Done Definition

The feature is done only when the UI and a non-UI verification path answer the
same questions for the same selected child node or parent-to-child patch edge.
The verification path must not read rendered UI text as source truth; it must
start from typed records and graph evidence, then compare that answer key to the
borrowed inspector projection used by the UI.

Each selected parent-create attempt must produce an answer matrix with these
columns. This matrix may be a test fixture, a borrowed contract projection, or
a text report rendered from that projection, but it must not become a
`ploke-egui` semantic mirror record.

| Column | Meaning |
| --- | --- |
| `question` | Stable operator question id, such as `objective`, `surface`, `provider_attempt`, `tools`, `proposal`, `patch_source`, `child_eval`, `source_refs`, or `missing_state`. |
| `ui_field` | Exact inspector section and field where the default UI answers the question. |
| `answer` | Compact user-facing answer, not raw JSON and not a long hash. |
| `state` | `present`, `missing`, `not_applicable`, `failed`, `blocked`, or `telemetry_only`. |
| `evidence` | Typed record or graph evidence class that supports the answer. |
| `detail` | Expand/debug target for full ids, paths, hashes, payload snippets, or source refs. |
| `negative_case` | A concrete thing the UI must not imply for this row. |

For a normal completed attempt, the matrix must answer:

1. What did the parent ask the harness or LLM to do?
2. Which surface was readable, writable, and forbidden?
3. Which LLM/provider attempt happened, how long did it take, and what was the
   outcome?
4. Which tools ran, what did they retrieve or produce, and did any fail?
5. Was a proposal produced, checked, applied, rejected, or missing?
6. Was the material patch placeholder-generated or LLM-derived?
7. Did the resulting child artifact run and produce evaluation evidence?
8. Which typed records support each displayed claim?
9. Which expected facts are missing, not applicable, failed, blocked by a
   missing projection, or telemetry-only?

For `p1-broad-harness-retry-20260514-1`, the matrix must make the known
placeholder behavior falsifiable:

- `llm_request`: present if the typed harness/LLM request exists.
- `proposal`: present, missing, or blocked according to typed proposal evidence;
  the UI must not infer proposal success from the child artifact alone.
- `patch_source`: placeholder if the material delta came from the placeholder
  path; this must be visually separate from the LLM request.
- `check_apply`: present, missing, or blocked from typed check/apply evidence.
- `child_eval`: present only if child run/evaluation records support it.

Passing `cargo check` is not enough. A completed implementation must satisfy
all evaluation gates below, including the answer-key comparison and a blind
review of the rendered inspector output.

## Validation Surfaces

Any CLI or text-report verification added for this work must stay a renderer,
not a second application:

- Source facts: typed `RunRecordSet`, agent-turn/tool/provider records,
  edit-surface records, and graph evidence.
- Semantic object: the parent-create attempt reachable from a selected child
  node or parent-to-child patch edge.
- Projection: borrowed inspector/timeline projections over `&Graph`, plus a
  borrowed answer matrix for tests and contract reports.
- Renderer: egui rows, `--inspect-node`, `--contract-report`, or a future
  narrow `--inspect-edge` report.

The useful verification shape is:

```text
typed source answer key
  compared with
borrowed inspector projection rendered as UI/text contract
  reviewed by
an independent worker who is not told the expected fixture answer
```

The reviewer should receive the fixture path, selection label/key, and rendered
contract output. A separate worker or the main thread should hold the typed
answer key and verify whether the reviewer could correctly answer the matrix
questions from the UI output alone.

## Evaluation Gates

### Gate 1: Graph Evidence Landing

`ploke-tree` must expose already-typed tool-call, tool-result, provider
attempt, timeout, retry, surface-attempt, and run-attempt facts as evidence
reachable from the selected graph object. If a fact cannot be joined, the graph
must preserve a typed unavailable reason rather than letting `ploke-egui` infer
from filenames, logs, or copied text.

Acceptance artifacts:

- A synthetic full-attempt fixture where the selected edge or child joins to
  objective, surface, provider attempt, tool summary, proposal, check/apply,
  patch source, child eval, and source-ref evidence.
- A synthetic placeholder fixture where the request exists but material patch
  source is placeholder, and proposal/check/apply are independently
  `present`, `missing`, or `blocked`.
- A negative fixture where similarly named files or child artifacts exist but
  the typed join is absent; the graph must report unavailable evidence instead
  of inferring success from paths.
- A same-count graph refresh fixture where record identities or evidence
  content change without node/edge count changes; the selected evidence must
  update or become unavailable rather than reusing stale widget payloads.

The tests should assert the answer matrix values, not just that graph loading
succeeds. They should include at least these expected rows:

```text
objective -> present/blocked with source record class
surface -> readable/writable/forbidden counts plus expandable grant ref
provider_attempt -> count, status, duration class, telemetry/source status
tools -> count, failed count, retrieved/produced item classes
proposal -> present/missing/blocked and source record class
patch_source -> placeholder/llm_derived/unknown
check_apply -> present/missing/failed/blocked
child_eval -> present/missing/not_applicable
```

Minimum verification commands after implementation:

```bash
cargo test -p ploke-tree parent_create_evidence 2>&1 | tail -n 120
cargo test -p ploke-tree fs_run_store_loads_record_set 2>&1 | tail -n 80
```

### Gate 2: Borrowed Inspector Projection

`ploke-egui` must resolve selected graph objects into a borrowed inspector
projection over `&Graph`. The projection may contain typed small values and
references; it must not store owned semantic rows as UI state.

Expected shape:

```text
GraphSelectionRef
  -> ParentCreateInspection<'graph>
  -> ParentCreateSnapshot<'selection, 'graph>
  -> renderer / contract report
```

Acceptance artifacts:

- A unit test that resolves the same synthetic full-attempt fixture from Gate 1
  through the inspector path and produces the same answer matrix values.
- A placeholder-fixture test proving the inspector displays both "LLM/harness
  request happened" and "patch source: placeholder" without merging them.
- A missing-join test proving the inspector renders explicit `missing` or
  `blocked` rows instead of omitting the field or synthesizing text from a
  label/path.
- A stale-selection test that loads graph A, selects a child, replaces the graph
  with graph B, and proves the inspector re-resolves against B or clears the
  selection. It must not resolve graph A's owned label/detail against graph B.
- A diagnostics/export test proving the serialized snapshot contains source
  status and compact answers rendered from borrowed projections, not owned
  semantic copies of chat/tool/provider/proposal rows.

Minimum verification commands after implementation:

```bash
cargo test -p ploke-egui --lib parent_create_inspector 2>&1 | tail -n 120
cargo test -p ploke-egui --lib inspector 2>&1 | tail -n 120
```

The existing dev CLI should be extended only as a thin renderer over the same
projection, for example:

```bash
cargo run -p ploke-egui --features dev -- \
  /home/brasides/.ploke-eval/campaigns/p1-broad-harness-retry-20260514-1/prototype1 \
  --inspect-node <child-label-or-key> 2>&1 | tail -n 120
```

The rendered report must include stable field names sufficient for scripted
checks and blind review, such as:

```text
Patch Generation
Outcome:
Objective:
Surface:
LLM Run:
Tools:
Proposal:
Check/Apply:
Patch Source:
Child Eval:
Source Status:
```

### Gate 3: Progressive UX Contract

The default inspector must be compact and question-led:

- Show status, objective, surface, proposal/check/apply, LLM run, tools, child
  eval, and source status as grouped sections.
- Abbreviate or hide long ids and hashes by default.
- Put full ids, hashes, raw response refs, paths, and raw-ish payloads behind a
  debug/detail affordance.
- Render clickable or expandable rows with hover/active styling.
- Use `missing`, `not_applicable`, `failed`, `blocked`, or `telemetry_only`
  explicitly instead of omitting expected facts.

Acceptance artifacts:

- A contract-render test for the default/compact inspector output. It must
  assert that the first-level `Patch Generation` section contains the required
  field names and enough compact values to answer the matrix questions.
- A hash/id suppression test. Default rows must not contain full SHA-like
  strings such as `[a-f0-9]{40,64}` except inside an expanded/debug/evidence
  section.
- An affordance test at the projection level: every row with hidden detail must
  carry an expand/debug action, and rows that navigate or highlight graph
  objects must carry an interaction class that the egui renderer maps to hover
  and active styling.
- A usefulness test: every default-visible row must map to one matrix question
  and one source-status class. Rows that cannot name an operator question should
  be removed, moved behind debug, or renamed.
- A missing-state test: if proposal, check/apply, provider timing, or tool
  retrieval evidence is absent, the field remains visible with an explicit
  state rather than disappearing.

Minimum verification commands after implementation:

```bash
cargo test -p ploke-egui --lib parent_create_ux_contract 2>&1 | tail -n 120
cargo test -p ploke-egui --lib default_view 2>&1 | tail -n 80
cargo check -p ploke-egui --target wasm32-unknown-unknown 2>&1 | tail -n 80
```

Expected visual/manual check against `p1-broad-harness-retry-20260514-1`:

- The initial inspector view does not fill the panel with SHA-like ids.
- The placeholder patch path is clearly separate from LLM/harness evidence.
- Tool/provider details are discoverable without being dumped by default.
- A fresh reviewer who only sees the UI or text contract output can answer the
  matrix questions, and another reviewer comparing against typed source facts
  finds no material mismatch.

### Gate 4: Timeline And Ordering Status

The bottom lane should either render parent-create spans or explicitly report
that typed span projection is not yet available. If spans render, they must
separate sealed History order from telemetry or timestamp-only timing.

Minimum expected rows:

```text
surface/objective -> LLM/provider attempt -> tool calls -> proposal/check/apply
  -> child plan -> child eval -> selection/handoff
```

If `timeline.concurrency` is still queued, this gate may pass with explicit
`blocked_by_missing_projection` timeline rows, but the UI must not fake causal
nesting from timestamps alone.

### Gate 5: Fixture Answer Check

For `p1-broad-harness-retry-20260514-1`, a manual or ignored smoke command
should demonstrate that selecting at least one generated child can answer:

- LLM/harness request happened.
- Proposal/check/apply evidence is present, missing, or blocked.
- Material patch source is placeholder or LLM-derived.
- Tool/provider counts and timing are visible when typed evidence exists.
- Child/evaluation/selection status is visible.
- Source refs identify typed records or telemetry classes.

If the fixture is absent on a machine, the smoke test should skip with a clear
message. It should not silently pass.

## Orchestrator Workflow

The main thread owns `.orchestrator/`. Sub-agents do not run board commands.
They read generated packet files, work only inside their assigned write
surface, and report changed files, tests, blockers, and handoff points.

Before spawning workers:

```bash
target/debug/xtask orchestrate --format json status | jq '{
  board,
  blockers: (.blockers | length),
  lanes: [.lanes[] | {id, owned_edit}],
  workers: [.workers[] | {id, role, active}],
  tasks_by_state: (.tasks | group_by(.state.state) | map({
    state: .[0].state.state,
    count: length
  }))
}'
target/debug/xtask orchestrate lane validate
target/debug/xtask orchestrate packet <worker-id>
```

For a fresh wave:

```bash
target/debug/xtask orchestrate init
target/debug/xtask orchestrate worker retainer --role retainer
target/debug/xtask orchestrate worker worker-tree --role worker
target/debug/xtask orchestrate worker worker-egui --role worker
target/debug/xtask orchestrate worker reviewer --role reviewer
```

If `target/debug/xtask` is missing or stale, build `xtask` first. Do not use
the legacy task stack as the live queue for this effort.

## Lanes

Use lane ownership to keep parallel work safe. Adjust exact file lists after
the first discovery packet, then run `lane validate` before spawning.

| Lane | Role | Owned edit surface | Notes |
|---|---|---|---|
| `retainer` | read-only discovery | none | Keeps source anchors, fixture paths, and prior-art locations fresh. |
| `tree-store` | worker | `crates/ploke-tree/src/store/`, `crates/ploke-tree/src/tests.rs` | Loads typed evidence into `RunRecordSet` when missing. |
| `tree-graph` | worker | `crates/ploke-tree/src/graph/`, `crates/ploke-tree/src/browser.rs` | Attaches parent-create/tool/provider evidence to graph objects. |
| `egui-inspector` | worker | `crates/ploke-egui/src/ui/inspector.rs` | Builds borrowed inspector projection and snapshot types. |
| `egui-render` | worker | `crates/ploke-egui/src/ui/app/shell.rs`, `crates/ploke-egui/src/ui/id_display/` | Renders compact sections, affordances, and id display behavior. |
| `egui-diagnostics` | worker | `crates/ploke-egui/src/diagnostics/`, `crates/ploke-egui/src/cli/` | Keeps contract reports in parity with borrowed inspector projection. |
| `docs` | worker or main | `crates/ploke-egui/docs/plan/parent-create/`, related contract docs | Updates durable docs only when the contract changes. |
| `review` | reviewer | none | Independent review; no writes unless explicitly reassigned. |

Do not assign `egui-inspector` and `egui-render` to the same file in parallel.
Do not edit `ploke-eval` for the `run-execution-graph.browser-spine` lane
without explicit reauthorization; that slice is currently scoped to
`ploke-records`, `ploke-tree`, and `ploke-egui`.

## Initial Board Tasks

Use task ids like these so reports and packets stay searchable:

| Task | Lane | Acceptance |
|---|---|---|
| `pc-discovery-map` | `retainer` | Report exact current code paths for RunRecordSet, graph evidence, inspector rows, diagnostics, and fixture availability. |
| `pc-tree-evidence-landing` | `tree-graph` | Selected child/edge graph objects can expose typed tool/provider/run-attempt evidence or typed unavailable reasons. |
| `pc-inspector-projection` | `egui-inspector` | Borrowed parent-create inspector projection answers the done-definition questions without owned semantic rows. |
| `pc-render-ladder` | `egui-render` | Right panel renders compact progressive sections, hides long hashes by default, and shows affordances for expandable rows. |
| `pc-diagnostics-parity` | `egui-diagnostics` | CLI/default-view diagnostics serialize from borrowed projections and expose parent-create contract signals. |
| `pc-fixture-smoke` | `tree-store` or `tree-graph` | Synthetic and optional real fixture checks prove placeholder-vs-LLM evidence is distinguishable. |
| `pc-review` | `review` | Reviewer signs off on typed boundary, stale-selection safety, UX ladder, and tests. |

Example add/assign pattern:

```bash
target/debug/xtask orchestrate lane set tree-graph \
  --own crates/ploke-tree/src/graph/ \
  --doc crates/ploke-egui/docs/plan/parent-create/orchestration.md

target/debug/xtask orchestrate add pc-tree-evidence-landing \
  --lane tree-graph \
  --title "Attach parent-create evidence to graph objects" \
  --allow crates/ploke-tree/src/graph/ \
  --forbid crates/ploke-egui/src/ \
  --doc crates/ploke-egui/docs/plan/parent-create/orchestration.md \
  --accept "Selected child or patch edge exposes typed tool/provider/run-attempt evidence or typed unavailable reasons."

target/debug/xtask orchestrate assign pc-tree-evidence-landing worker-tree --active
target/debug/xtask orchestrate lane validate
target/debug/xtask orchestrate packet worker-tree
```

## Model Slots

When spawning sub-agents:

- Discovery and retainer work: use `gpt-5.3-codex-spark` with high reasoning.
  If the UI names this as "spark mini", treat it as the fast discovery slot.
- Implementation workers: use `gpt-5.4` with high reasoning.
- Independent reviews: use `gpt-5.5` with xhigh reasoning.

Worker prompts should be short and packet-based:

```text
Read your generated packet first. You are not alone in the codebase; do not
revert or overwrite changes outside your assigned surface. Do not run
orchestrator board commands. Keep semantic data borrowed from Graph or typed
records; do not introduce UI-owned semantic mirrors. Report changed files,
tests, blockers, and exact handoff points.
```

## Join And Review Rules

The main thread joins worker results in dependency order:

```text
discovery
  -> tree store/graph evidence
  -> borrowed inspector projection
  -> renderer and diagnostics
  -> fixture smoke
  -> independent review
```

Parallelism is allowed whenever write surfaces are disjoint. Reviews may run as
soon as a worker has a coherent completed surface; downstream workers should
not rely on a completed upstream task until the main thread has reviewed the
reported diff or assigned a reviewer.

Mark implementation completion with:

```bash
target/debug/xtask orchestrate complete <task-id> --report <path>
target/debug/xtask orchestrate review <task-id> --report <path>
```

Use blockers instead of loose notes:

```bash
target/debug/xtask orchestrate block <task-id> <blocker-id> --kind boundary \
  --summary "<short blocker>" \
  --evidence <path> \
  --unblock "<next concrete action>"
```

## Resume Protocol

After compaction or interruption:

1. Read this file and [`main-plan.md`](main-plan.md).
2. Run a bounded board status projection; do not dump the full board.
3. Check `git status --short` and preserve unrelated worktree changes.
4. Regenerate packets for active workers if the docs or board changed.
5. Continue from the active board task, not from memory.

If the board no longer matches the worktree, pause and repair the board before
spawning more workers.
