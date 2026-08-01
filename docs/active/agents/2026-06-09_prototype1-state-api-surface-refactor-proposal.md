# Prototype 1 state / tui_adapter API-surface refactor proposal

Date: 2026-06-09
Type: design proposal (plan)
Status: proposal — survey + recommendations only; no production code changed.
Scope: `crates/ploke-eval` (`cli/prototype1_state/**`, `record_emission.rs`,
`operational_metrics.rs`, `successor_selection/**`), with read boundaries into
`crates/ploke-tui/src/tools`, `crates/ploke-records`, `crates/ploke-tree`,
`crates/ploke-db`, and `crates/ploke-selection-score`.

Related docs (read these first for grounding):
- [`2026-06-02_prototype1-state-loop-walkthrough/README.md`](2026-06-02_prototype1-state-loop-walkthrough/README.md) — authoritative end-to-end path map.
- [`2026-06-08_01_prototype1-guided-edit-surface/adr-guided-edit-surface.md`](2026-06-08_01_prototype1-guided-edit-surface/adr-guided-edit-surface.md) — the ADR for the 5 features (PARTIALLY IMPLEMENTED).
- [`2026-06-05_ploke-eval-selection-score-metrics.md`](2026-06-05_ploke-eval-selection-score-metrics.md) and [`2026-06-05_selection-score-ploke-applicability.md`](2026-06-05_selection-score-ploke-applicability.md) — feature 5 inputs.
- [`2026-05-12_tui-adapter-boundary-review/README.md`](2026-05-12_tui-adapter-boundary-review/README.md) — prior boundary review.

> This proposal deliberately does not implement the five follow-up features. It
> proposes the seams that make each feature a localized addition. The five
> features already have an ADR ([guided edit surface](2026-06-08_01_prototype1-guided-edit-surface/adr-guided-edit-surface.md));
> this document is the *enabling cleanup* for that ADR, focused on the
> `tui_adapter` and the `prototype1-state` execution path's API surface.

---

## 0. Executive summary

The `prototype1-state` parent turn works, but its central candidate-generation
boundary — the headless `ploke-tui` adapter (`edit_surface/tui_adapter/`) and the
edit-surface carriers (`edit_surface/{graph,surface,harness_request}.rs`) — has
accreted **three parallel, partially-overlapping models of the same concept**
(an edit surface and a headless attempt), plus a stringly/positional call
boundary into the rest of the parent turn. The follow-up features all want to
attach behavior at exactly this boundary (a planning stage *before* generation,
a graph-restricted policy *during* generation, multi-instance cohorts *around*
generation, a record mirror *under* persistence, and missing-data telemetry
*after* selection). Today each would be a cross-cutting change because the
boundary is implemented as free functions with telescoping signatures rather
than a small set of typed ports.

The good news (confirmed by GitNexus impact analysis): the adapter is
well-isolated. Every refactor target below is **LOW risk** with a small,
mostly-internal blast radius. This is a high-leverage, low-danger cleanup.

Recommended direction, in one sentence: **collapse the three edit-surface
models into one typed `EditSurface` admission path, express the headless attempt
as one typed `AttemptRequest -> AttemptOutcome` port behind a `Harness` trait
(which already exists but is bypassed), replace `PrepareError` stringization at
the boundary with a structured adapter error, and route all record emission
through the existing `EmitRecord` choke point** — then hang each of the five
features off the new seams.

Recommended PR sequence (detail in §6): (1) attempt-request consolidation, (2)
typed adapter error, (3) edit-surface policy unification (`BroadEditPolicy` ->
typed `SurfacePolicy`), (4) planning-stage seam, (5) graph-policy enforcement
seam, (6) record-mirror choke-point completion, (7) cohort carrier, (8)
missing-data telemetry carrier.

---

## 1. Current architecture (concrete map)

### 1.1 The parent turn in one paragraph

`Prototype1StateCommand::run -> run_turn`
(`cli/prototype1_state/cli_facing.rs:6477-7043`) resolves identity/campaign/
profile/closure/journal, establishes a baseline, resolves a child plan via a
candidate generator, fans children out through the C1–C4 state machine
(`c1.rs`–`c4.rs`), compares each child treatment to baseline, runs successor
selection, and may hand off to a successor parent. The full phase-by-phase map
with disk-write boundaries is in the
[state-loop walkthrough](2026-06-02_prototype1-state-loop-walkthrough/README.md)
and is not repeated here. This proposal focuses on the **candidate-generation
boundary** (the broad-harness / headless-TUI path) because that is where all
five features attach.

### 1.2 The candidate-generation boundary, top to bottom

Call chain for the live broad path:

```
run_turn
  -> resolve_child_plan (candidate generator = BroadHarnessRequest)
  -> run_broad_harness_attempt_slot / run_broad_slot_for_admission
     -> run_broad_headless_tui_attempt(_with_options)   cli_facing.rs:1793 / 1920
        -> tui_adapter::run_headless_with_model_capture_responses   tui_bridge.rs:70
           -> run_headless_with_model_inner   tui_bridge.rs:95
              loop { start_attempt_runtime -> run_attempt(TuiHarness) }   tui_bridge.rs:121-204
        -> write_broad_headless_tui_diagnostics   cli_facing.rs:2494
        -> write_broad_headless_tui_turn_live_bundle   cli_facing.rs:2512
        -> finish_broad_headless_tui_attempt -> transaction::Executor
```

The adapter module layout (`edit_surface/tui_adapter/`):

| File | Responsibility | Notable symbols |
|---|---|---|
| `mod.rs` | Module facade, `ModelSelection`, typestate markers `state::{Ready,Running,Done}` (unused by live path), re-exports | `ModelSelection` (`mod.rs:50-117`) |
| `tui_bridge.rs` (2136 lines) | The live driver: telescoping entrypoints, the hand-rolled retry loop, `StateCommand` plumbing, proposal staging/approval/apply, post-apply index/BM25 refresh, cargo validation, provider-failure sniffing | `run_headless_with_model{,_capture_responses}`, `run_attempt`, `start_attempt_runtime`, `validate_applied_batch`, `wait_for_selected`, `wait_for_refresh`, `classify_paths`, `LiveObserver` |
| `harness/mod.rs` | The *intended* trait boundary + neutral event vocabulary | `Harness` trait, `Progress`, `Batch`, `Staged`, `Decision`, `Settled`, `TurnStop`, `SessionSpec` |
| `harness/tui.rs` (807 lines) | The one `Harness` impl: `TuiHarness` drives one attempt as an event loop | `TuiHarness`, `drive_to_attempt_end`, `surface_decision`, `classify_turn_stop` |
| `harness_io.rs` (2288 lines) | `HeadlessRun` (the durable observation accumulator), terminal taxonomy, `Budget`, the *second* typed retry policy, the boundary `Error` | `HeadlessRun`, `HeadlessTerminal`, `Budget`, `Step`/`Terminal`/`Outcome`/`Feedback`, `Error` |

### 1.3 The edit-surface carriers (`edit_surface/`)

These are the typed objects that *should* describe "what may be edited":

- `graph.rs`: `View` trait + `Projection`, `Bounds`, `Rule`, `Target`, `Span`,
  `Hit`, `Delta`, plus a `Mock` view. A clean graph-projection algebra.
- `surface.rs`: `SurfaceRequest -> admit() -> EditableSurface -> Grant ->
  check(Draft) -> Check`. A genuine typestate admission chain with
  `MaterialScope`, `Area`, `ProtectedCore`, `ObjectiveSpec`, `EditObjective`.
- `harness_request.rs`: `PublishedBroadHarnessRequest`, `BroadEditPolicy`,
  `GraphRestriction`, `RequestAdmissionBinding`, `contract::*`, planning carriers.
- `harness_result.rs`: `SubmittedBroadHarnessResult`, the admitted
  `transaction::*` typestate.

---

## 2. The core problem: three parallel models of one concept

This is the single most important finding. There are **three** representations
of "the editable surface / the headless attempt", and they do not share a spine.

### 2.1 Model A — the dormant typed algebra (well-designed, not wired)

`edit_surface/graph.rs` and `edit_surface/surface.rs` define a careful typestate
chain: `SurfaceRequest::broad(...).admit()? -> EditableSurface`, whose `Grant`
exposes `grant.check(Draft) -> Result<Check, surface::Error>` and `narrow()`
that *rejects widening* (`surface.rs:210-216`, `Error::Widens`). Both files open
with `#![allow(dead_code)] // Phase 1 boundary; live adapters are wired in later
phases.` (`graph.rs:1`, `surface.rs:1`). This is the model the project *wants*,
and it already encodes the exact invariant feature 2 needs (a graph `Bounds`
that can only be narrowed, with a protected core). It is essentially unused by
the live path.

### 2.2 Model B — the live string/path enforcement

The live path enforces editability with **path-string matching**, not the typed
algebra:

- `BroadEditPolicy` is a single-variant enum
  (`harness_request.rs:862-865`: only `WorkspaceExceptPlokeEval`).
- `write_scope_for_policy` (`tui_bridge.rs:269-291`) maps that one variant to a
  `ploke_tui::utils::path_scoping::WriteScope` built from
  `WORKSPACE_EXCEPT_AUTHORITY_{PREFIXES,FILENAMES}` string constants.
- `classify_paths` (`tui_bridge.rs:2077-2115`) re-checks applied proposal paths
  against `prototype_surface_for_broad_edit_policy` + `path_matches_surface_policy`.

### 2.3 Model C — the advisory graph-restriction metadata

`GraphRestriction { mode: ToolNeighborhood, nearest_items, seed_modules:
[crates/ploke-tui/src/tools/mod.rs], source: CodeGraphCozo }`
(`harness_request.rs:876-892`) is serialized into the published request and
rendered into the planner/patcher prompt. But it is **advisory only**: the ADR
records that "final broad harness admission still rejects only against the
existing `WorkspaceExceptPlokeEval` protected-core surface"
([changelog](2026-06-08_01_prototype1-guided-edit-surface/changelog.md), lines
87-92). Nothing enforces the N-nearest graph neighborhood.

### 2.4 Why this matters

Feature 2 ("graph-restricted mutable surface") is *defined in terms of Model A*
(`graph::Bounds`, `surface::Grant`) by the ADR, *configured in terms of Model C*
(`GraphRestriction`), and *enforced by Model B* (path strings). To implement it,
someone must reconcile all three. Until they are unified, every edit-surface
change is a three-place edit with a silent correctness gap (the prompt promises
a neighborhood the harness never enforces — a permissiveness footgun the project
guardrails explicitly warn against).

There is a **fourth, smaller** duplication of the same shape: two retry state
machines. `harness_io.rs:1986-2059` defines a typed `Step`/`Terminal`/`Outcome`/
`Feedback` policy; the live loop in `tui_bridge.rs:121-204` ignores it and
hand-rolls retry with `AttemptEnd` + `advance_turn`.

---

## 3. API-surface problems that make the 5 features hard

Beyond the three-models problem, the boundary has concrete ergonomic and
coupling defects. Each is cited so it can be verified.

**P1 — Telescoping entrypoints + positional `Option`/slice args.**
`run_headless`, `run_headless_with_model`, `run_headless_with_model_capture_responses`
(`tui_bridge.rs:31-93`) are three overloads that funnel into
`run_headless_with_model_inner` with **8 positional parameters** including two
`Option`s, a `&[EvidenceRoot]`, and a `&[contract::Command]`
(`tui_bridge.rs:95-104`). `run_attempt` takes **10 positional parameters**
(`tui_bridge.rs:398-409`) and carries a `// TODO: I think this actually wants to
be a method of HeadlessRun` (`tui_bridge.rs:397`). Adding feature 1's planning
context or feature 2's graph bounds means threading yet more positional params
through every layer.

**P2 — The `Harness` trait exists but the live path bypasses it.**
`harness/mod.rs:109-121` defines a clean port (`next/decide/settle/run`), and
`TuiHarness` implements it. But the live driver (`run_headless_with_model_inner`)
and `run_attempt` call free functions in `tui_bridge.rs` instead of programming
to the trait, and `drive_to_attempt_end` (`harness/tui.rs:101-165`) re-imports a
dozen free helpers. The abstraction is declared but leaky: the policy that
*should* live above the trait (retry, validation gating, terminal
classification) is smeared across `tui_bridge.rs` free functions and
`TuiHarness` methods.

**P3 — Boundary error is stringized into `PrepareError`.**
The adapter has a precise `Error` enum (`harness_io.rs:2061-2077`:
`EmptyBudget`, `HeadlessSetup{phase,detail}`, `HeadlessEvent`, `Surface(...)`),
but `run_broad_headless_tui_attempt_with_options` discards it: on failure it
either projects `setup_failure()` or formats `format!("broad headless-tui
attempt failed: {source}")` into `PrepareError::InvalidBatchSelection`
(`cli_facing.rs:1992-2004`). `PrepareError` is the catch-all eval error;
downstream callers cannot match on *why* an attempt failed (provider down vs.
surface violation vs. timeout vs. no-edit). Features 1 and 5 both want to
*record* and *route on* failure cause, so this lossy boundary is directly in the
way.

**P4 — Deep coupling into `ploke-tui` internal state.**
`tui_bridge.rs` reaches into `ploke_tui::app_state::StateCommand` variants,
`runtime.state.proposals` / `create_proposals` maps, `EditProposalStatus`,
`ploke_tui::tools::Ctx` + `cargo::CargoTool as Tool` direct execution
(`tui_bridge.rs:637-695`), and BM25 (`bm25_status_with_timeout`). The neutral
event vocabulary in `harness/mod.rs` was meant to contain this, but `tui_bridge`
short-circuits it. This coupling is the reason the harness is hard to test
without a full `WorkspaceTuiRuntime` and hard to reuse for the planning model
(feature 1), which needs a *different* call shape (one structured JSON response,
no edit loop).

**P5 — Test seams baked into production functions.**
`run_broad_headless_tui_attempt_with_options` contains `#[cfg(test)] if let
Some(result) = broad_headless_tui_fixture_attempt(slot) { return result; }`
(`cli_facing.rs:1949-1952`). Planner sequencing similarly branches on a
`test_stub` artifact in production code
([ADR implementation note](2026-06-08_01_prototype1-guided-edit-surface/adr-guided-edit-surface.md), lines 57-61).
This makes the production path's control flow depend on `cfg(test)`, which is
fragile and obscures the real seam (a provider port that can be swapped).

**P6 — Persistence choke point exists but is ~3-call adopted.**
`EmitRecord`/`JsonRecordFile` (`record_emission.rs:36-73`) is the designed single
write+mirror path, but only `save_node_record`/`save_runner_request`/
`save_runner_result` use it (`intervention/scheduler.rs:615,779,852`). Scheduler
state, run profile, closure, agent turns, compressed run records, and protocol/
eval artifacts all bypass it with ad-hoc `fs::write`. Feature 4 ("parallel Cozo
mirror of *all* record families") cannot land cleanly until emission is funneled.

**P7 — Single-instance assumptions baked into node identity.**
`Prototype1NodeRecord.instance_id` and `Prototype1RunnerRequest.instance_id` are
scalar `String` (`scheduler.rs:283-312`, `340-362`); `ParentIdentity::from_node`
copies one scalar (`identity.rs:117-129`); broad/TUI children set `instance_id =
parent.runtime_id().to_string()` (`cli_facing.rs:2589-2590`, `2981`). The cohort
exists at the campaign/slice layer (`Target::eval_instances()`,
`profile.rs:248-258`) and MBE already supports N via `CohortRequest`
(`mbe/mod.rs:129-142`), but the loop spine collapses to one primary instance.
Feature 3 is blocked at the node-identity layer, not the harness layer.

---

## 4. Proposed refactor of the boundary

Design principles (from the project's `AGENTS.md` + semantic-architecture
skill): typed boundaries over stringly/boolean flags; encode structure in types,
not in long names (≤3 semantic parts); structured errors; typestate/enum
carriers; surgical, reversible steps. The refactor below is staged so each step
compiles and passes tests independently.

### 4.1 One typed attempt request (fixes P1, P5)

Replace the three telescoping entrypoints and the 8/10-arg inner functions with
a single owned request struct and one entrypoint:

```rust
// edit_surface/tui_adapter/mod.rs
pub(crate) struct Attempt {
    pub workspace: PathBuf,
    pub prompt: String,
    pub budget: Budget,
    pub surface: SurfacePolicy,          // see §4.3 (replaces BroadEditPolicy)
    pub evidence: Vec<EvidenceRoot>,
    pub validation: Vec<contract::Command>,
    pub model: Option<ModelSelection>,
    pub capture: Capture,                // enum { Off, Responses } (replaces the bool-by-overload)
}

impl Attempt {
    pub(crate) async fn run(self, harness: &mut impl Harness) -> Result<HeadlessRun, Error>;
}
```

Notes:
- `Capture` is an enum, not a third overload; the response-tap install moves
  inside `Attempt::run` keyed on `self.capture`.
- The retry loop becomes the one in `harness_io.rs` (`Step`/`Terminal`),
  deleting the hand-rolled `advance_turn` loop (resolves the §2.4 duplication and
  the `tui_bridge.rs:397` TODO).
- Test injection (P5) becomes a `Harness` chosen by the caller (a `FixtureHarness`
  in tests, `TuiHarness` in prod) rather than a `#[cfg(test)]` early return.

Name discipline: fields are single-token roles; the structure carries the
meaning, so we avoid names like `headless_tui_response_capture_mode`.

### 4.2 Make the `Harness` trait the real port (fixes P2, P4)

The trait in `harness/mod.rs` is already the right shape. The refactor *moves
policy above the trait and mechanics below it*:

- Above the trait (in `Attempt::run` / a small `AttemptDriver`): retry policy,
  validation gating, terminal classification (`classify_turn_stop`,
  `classify_applied_terminal`).
- Below the trait (in `TuiHarness` / a new `tui_bridge` that is *only* the
  ploke-tui mechanic): everything that touches `StateCommand`, `proposals`,
  `EditProposalStatus`, BM25/index refresh, cargo execution.

This lets feature 1 introduce a *different* implementor (a `PlanHarness` that
returns one structured response and no edit batches) reusing the same driver
vocabulary, and lets tests use a `FixtureHarness` without `WorkspaceTuiRuntime`.

Tradeoff/alternative: a full trait-object (`dyn Harness`) port is not required;
static dispatch (`impl Harness`) keeps zero-cost and matches the project's
static-dispatch preference. The cost is that `Attempt::run` is generic; that is
acceptable given only 2–3 implementors.

### 4.3 Unify the three edit-surface models (fixes the §2 core problem)

This is the keystone change. Promote the dormant typed algebra (Model A) to the
single source of truth and make Models B and C *projections of it*:

1. Replace `BroadEditPolicy` (Model B, single-variant) with a typed
   `SurfacePolicy` enum that names the *intent*, each variant carrying its typed
   scope:

   ```rust
   pub(crate) enum SurfacePolicy {
       WorkspaceExceptCore(ProtectedCore),       // today's WorkspaceExceptPlokeEval
       GraphNeighborhood(surface::Grant),        // feature 2: a narrowed Grant
   }
   ```

2. `write_scope_for_policy` and `classify_paths` become **derivations** of the
   `SurfacePolicy`: a `WriteScope` is projected from the policy's `Area`/
   `ProtectedCore`, and the post-apply path check calls `grant.check(Draft)`
   (`surface.rs:304-313`) instead of re-deriving from strings. One admission
   model, two read-projections.

3. `GraphRestriction` (Model C) stops being advisory: it becomes the *recipe* for
   building the `GraphNeighborhood(Grant)` (seeds -> graph `Bounds` -> `Grant`).
   The prompt still renders it, but admission now enforces the same object.

Correctness note (flagged per guardrails): step 2 *tightens* enforcement — it
closes the current gap where the prompt promises a neighborhood the harness
never checks. It must fail closed: if the graph lookup is unavailable, the policy
must error, not silently widen to the workspace (ADR Compliance line:
"Do not let graph lookup failure silently widen the mutable surface"). This is a
deliberate non-permissive change and should be called out in its PR.

### 4.4 Structured boundary error (fixes P3)

Stop collapsing adapter `Error` into `PrepareError` strings at
`cli_facing.rs:1992-2004`. Either (a) add a `PrepareError::HeadlessAttempt(#[from]
tui_adapter::Error)` variant, or (b) have the broad-attempt function return
`Result<AttemptOutcome, tui_adapter::Error>` and let the *caller* decide how to
fold it. Option (b) is preferred: it keeps `AttemptOutcome` (applied / rejected /
provider-unavailable / timed-out / no-edit) as a typed value the planning stage
(feature 1) and missing-data telemetry (feature 5) can record without re-parsing
strings. `AttemptOutcome` should reuse the existing `HeadlessTerminal` taxonomy
rather than introduce a new enum (type-reuse discipline).

### 4.5 Keep the durable artifacts; just move the seams

`HeadlessRun` (the observation accumulator), the `.headless-tui.json` diagnostics,
and the `.turn-live/` bundle are good and load-bearing for replay; this refactor
does **not** change their schemas. It only changes *who calls them*: the
diagnostics/turn-live writes move to run after `Attempt::run` returns a typed
outcome, which they already effectively do (`cli_facing.rs:2012-2013`).

---

## 5. Per-feature seams the refactor exposes

For each feature: where it attaches, which new/existing type hosts it, and why
the refactor makes it localized.

### Feature 1 — Pre-generation planning stage (high-capacity structured review)

- Seam: a new phase **between** child-plan resolution and broad-harness
  publication, i.e. inside `resolve_child_plan` / just before
  `run_broad_harness_attempt_slot`. The ADR already added a planner prompt +
  artifact here ([changelog](2026-06-08_01_prototype1-guided-edit-surface/changelog.md) lines 33-37).
- Host types: reuse `surface::{ObjectiveSpec, EditObjective}` for the structured
  output (it already models target metric, writable intent, constraints, success
  criteria, evidence refs via `EvidenceRef`). The planner result becomes a typed
  `PlanReview { objective: EditObjective, evidence: Vec<EvidenceRef>, pipeline:
  String, intent: String, missing: Vec<MissingDatum> }` — note `target pipeline`,
  `evidence` (with locations via `EvidenceRef`), `pipeline scope`, and `intent`
  map 1:1 to the feature's (a)–(d).
- Why localized after refactor: the planning model call is *another `Harness`
  implementor* (`PlanHarness`) driven by the same `Attempt`-style request, so it
  reuses model routing, response capture, and error taxonomy instead of a bespoke
  call path. Without §4.2 it would be a fourth ad-hoc ploke-tui call site.
- Persistence: the `PlanReview` is a new `ploke-records` `Record` family emitted
  through `EmitRecord` (§Feature 4), so it is mirrored automatically.

### Feature 2 — Graph-restricted mutable surface (N-nearest from tool modules)

- Seam: the `SurfacePolicy::GraphNeighborhood(Grant)` variant from §4.3.
- Host types: `edit_surface::graph::{Projection, Bounds, Rule}` to compute the
  neighborhood, `surface::{SurfaceRequest, Grant, EditableSurface}` to admit it,
  `harness_request::GraphRestriction` as the configuration recipe. Seeds are the
  10 `impl Tool` types in `crates/ploke-tui/src/tools/` (enumerated:
  `RequestCodeContextGat`, `GatCodeEdit`, `InsertRustItem`, `CreateFile`,
  `NsPatch`, `NsRead`, `CodeItemLookup`, `CodeItemEdges`, `CargoTool`, `ListDir`),
  resolved from `seed_modules = [crates/ploke-tui/src/tools/mod.rs]`.
- New port needed: a `GraphView` impl of the existing `graph::View` trait backed
  by `ploke-db` (the Cozo code graph) to replace the `Mock` view — "N nearest
  items" becomes a `Rule`-driven `bounds()` query. This is the one genuinely new
  adapter; everything else is wiring.
- Why localized after refactor: enforcement already routes through
  `grant.check(Draft)`, so adding a new `Grant` source does not touch the harness
  driver or the apply loop. Without §4.3 it is a three-place change (policy enum,
  `WriteScope`, `classify_paths`) plus an unclosed correctness gap.
- Risk to flag: depends on code-graph freshness; must fail closed (§4.3).

### Feature 3 — Multiple multi-SWE-bench targets per loop

- Seam: the node-identity layer, not the harness. `Prototype1NodeRecord` /
  `Prototype1RunnerRequest` / `ParentIdentity` need a cohort carrier instead of a
  scalar `instance_id`.
- Host type: introduce a `Cohort` (typed `Vec<InstanceId>` with a designated
  `primary`) and thread it where the scalar is read; child self-eval applies/
  evaluates the candidate across the cohort. MBE is already cohort-ready
  (`CohortRequest`, `mbe/mod.rs:129-142`; aggregated
  `multi-swe-bench-submission.jsonl`); branch evaluation already produces
  per-instance `compared_instances` (`cli_facing.rs:8699-8713`).
- Decision to surface (do not pick silently): cohort vs. primary semantics when a
  treatment is *partial* (patch fixes some instances, not all).
  `maybe_attach_treatment_oracle` currently hard-requires the full configured
  cohort (`prototype1_process.rs:2191-2207`). Options: (a) require-all (current,
  strict); (b) score partial cohorts with explicit `missing_treatment_instance_ids`
  (the carrier already exists in `Prototype1EvalSetIdentity`). This is a policy
  choice with correctness implications and must be decided with the user.
- Why localized after the carrier lands: most of the multi-instance scaffolding
  (slice, baseline closure, branch comparison) already iterates the cohort; the
  blocker is the scalar identity. Keep `legacy` generation single-instance
  (profile validation already enforces this, `profile.rs:62-72`).

### Feature 4 — Parallel persistent Cozo records mirror

- Seam: the `EmitRecord`/`JsonRecordFile` choke point (`record_emission.rs`).
- Host work: route the remaining ad-hoc writers (scheduler state, run profile,
  closure, agent turns, compressed run records, protocol/eval artifacts) through
  `EmitRecord` so the existing `mirror_record` (`record_emission.rs:76-166`)
  captures all 14 `RecordFamily` variants, not 3. The `prototype1_record` relation
  and process-local mutex already exist.
- Decision to surface: the mirror currently opens raw `cozo::DbInstance`
  (`record_emission.rs`) and duplicates low-level Cozo usage rather than reusing
  `ploke-db::Database` / `ploke-tree`'s `Graph::from_records`. Two paths:
  (a) keep the mirror a thin payload table (current), and let `ploke-tree` read
  `payload_json` -> `RunRecordSet` -> `Graph::from_records` (zero new schema);
  (b) give the mirror a richer relational schema. The ADR says keep it
  projection-only and separate "for now" — recommend (a): the mirror stays a
  durable JSON envelope table, and `ploke-tree` remains the typed projection
  engine. Avoid a second authority.
- Backup-fixture guardrail: per `AGENTS.md`, any relation add/rename must
  regenerate fixtures or add a migration; treat `tests/backup_dbs/` as
  schema-coupled.

### Feature 5 — Missing-data tracking for selection-score mechanisms

- Seam: the score-selection review carrier `MissingDataSummary`
  (`score/select.rs:169-176`) and the field-tagged `ScoreSelectionDiagnostic`
  (`score/select.rs:478-508`), which already exist and aggregate missing fields.
- Host work: extend the diagnostic field taxonomy to the lanes
  `ploke-selection-score` declares but the loop never populates —
  `ploke/evidence.rs:7-16` `Lane::{Cost, Trace, Handoff}` and the metric families
  from [the metrics note](2026-06-05_ploke-eval-selection-score-metrics.md):
  route cost, episode status, eval-set identity, evaluator trust, artifact diff,
  scheduler distribution. Each missing carrier is recorded as a
  `ScoreSelectionDiagnostic::missing(field, ...)` with the *mechanism name* and
  *required carrier* named (ADR guardrail: "Do not let missing selection-score
  telemetry silently score as zero").
- Posture: report-only first ([applicability map](2026-06-05_selection-score-ploke-applicability.md));
  do not let `ploke-selection-score` drive selection until carriers are validated.
- Why localized: the missing-data summary is already a typed aggregation; this is
  additive field tagging, not a new selector. Each new metric carrier (route cost,
  etc.) is a new `ploke-records` field emitted through `EmitRecord` (feature 4),
  so the two features compound cleanly.

---

## 6. Risks, blast radius, and PR sequencing

### 6.1 Blast radius (GitNexus impact, upstream)

| Symbol | Risk | Direct callers (d=1) |
|---|---|---|
| `run_headless_with_model` (`tui_bridge.rs`) | LOW | `run_headless`, replay `run_prefix_then_live_probe`, replay `run_self_edit_probe`, 1 test |
| `run_broad_headless_tui_attempt_with_options` (`cli_facing.rs`) | LOW | `run_broad_headless_tui_attempt`, `run_broad_harness_attempt_slot` (internal) |
| `BroadEditPolicy` enum (`harness_request.rs`) | LOW | none in the graph (callers are within the same module) |
| `HeadlessRun` struct (`harness_io.rs`) | LOW | tests only at d=1 |
| `JsonRecordFile` (`record_emission.rs`) | LOW (3 callers) | scheduler save fns |

Interpretation: the adapter is well-isolated; the only cross-module upstream
reach is into `replay/` (probe + self-edit), which already calls
`run_headless_with_model`. Any signature change there must be reflected in the
two replay probe entrypoints — both should adopt the new `Attempt` request.

### 6.2 Concurrency / async hazards to respect

- **Process-global response tap.** `ploke_tui::llm::install_response_tap`
  (`tui_bridge.rs:81`) returns a *process-global* guard. The capture mode must
  keep RAII guard lifetime tied to the single attempt; do not let two attempts
  install taps concurrently. The current design serializes child fanout per slot,
  but feature 3's cohort fanout must not parallelize attempts that share the tap.
- **Record-mirror SQLite lock.** The mirror already needed a process-local mutex
  for `database is locked`
  ([changelog](2026-06-08_01_prototype1-guided-edit-surface/changelog.md) lines 58-66).
  Funneling *all* families through `EmitRecord` (feature 4) increases mirror write
  pressure; keep the single-writer mutex and consider batching, but do not weaken
  crash-recovery (file authority is written *before* the mirror — preserve that
  order, `record_emission.rs:54-72`).
- **Blocking work in async.** `run_child_fanout` uses blocking tasks around
  `run_planned_child` (`cli_facing.rs:5123-5245`); cargo build/check (C2),
  `dir_size_limited`, and `fs::*` in the adapter are blocking. Keep them on
  blocking executors; do not move them onto the cooperative runtime.
- **Broadcast lag.** `next_event_with_deadline` already handles
  `RecvError::Lagged` (`tui_bridge.rs:1965-1990`); the trait-port move must keep
  lag recording, or a lag becomes a silent hang-until-deadline.

### 6.3 Recommended PR sequence

Each PR is independently shippable and test-gated. Run `gitnexus_impact` on the
edited symbol and `gitnexus_detect_changes` before each commit (per `CLAUDE.md`).

1. **PR1 — Attempt request consolidation (P1, P5).** Introduce `Attempt` +
   `Capture` enum; collapse the three entrypoints; replace the hand-rolled retry
   loop with `harness_io::Step`/`Terminal`. Move the `#[cfg(test)]` fixture
   injection to a `Harness` choice. Update the two `replay/` callers. No behavior
   change. *Verify:* existing `tui_adapter` + `replay` tests pass.
2. **PR2 — Structured boundary error (P3).** Return `tui_adapter::Error` /
   `AttemptOutcome` from the broad-attempt fn; stop stringizing into
   `PrepareError`. *Verify:* error-path tests assert typed variants.
3. **PR3 — Harness port cleanup (P2, P4).** Move policy above the trait,
   mechanics below; shrink `tui_bridge` to ploke-tui mechanics only. *Verify:*
   add a `FixtureHarness` and a driver-level unit test that needs no
   `WorkspaceTuiRuntime`.
4. **PR4 — SurfacePolicy unification (§4.3, Model A/B merge).** Replace
   `BroadEditPolicy` with `SurfacePolicy`; derive `WriteScope`/path-check from the
   typed scope; keep behavior identical for the existing workspace policy. *Verify:*
   path-classification tests unchanged.
5. **PR5 — Graph neighborhood enforcement (feature 2 seam).** Add the `ploke-db`
   `graph::View` impl and `SurfacePolicy::GraphNeighborhood(Grant)`; admission now
   enforces the neighborhood and **fails closed** on missing graph data. *Verify:*
   a test that a changed path outside the neighborhood is rejected, and that
   graph-unavailable errors (does not widen). *Flag the non-permissive change.*
6. **PR6 — Planning stage as a Harness (feature 1 seam).** Add `PlanHarness` +
   `PlanReview` (reusing `EditObjective`); emit through `EmitRecord`. *Verify:*
   the existing planner-sequencing test, now driven by the port not a `cfg(test)`
   stub.
7. **PR7 — Record-mirror choke-point completion (feature 4).** Route remaining
   writers through `EmitRecord`. *Verify:* a test that each family appears in the
   mirror relation; backup-fixture review if any relation changes.
8. **PR8 — Cohort carrier (feature 3) + missing-data taxonomy (feature 5).** Two
   small additive PRs that can run in parallel once 1–7 land: the `Cohort`
   identity carrier (with the partial-treatment policy decision made first), and
   the extended `ScoreSelectionDiagnostic` field taxonomy.

PRs 1–4 are pure cleanup (no feature). PRs 5–8 are the feature seams. PR4 is the
keystone; PRs 5–8 depend on it (or on PR7 for emission).

---

## 7. Tradeoffs and alternatives (do not silently pick)

- **Unify on Model A vs. delete Model A.** I recommend promoting the dormant
  typed algebra. The alternative is to delete `graph.rs`/`surface.rs` as
  speculative dead code and keep string-path enforcement. Rejected because
  feature 2 *requires* exactly the invariant Model A already encodes
  (`Bounds::covers`, `narrow` rejects widening, `ProtectedCore`); rebuilding it
  stringly would reintroduce the correctness gap. But this is a real fork worth
  the user's confirmation, since Model A is currently unused.
- **`impl Harness` (static) vs. `dyn Harness` (dynamic).** Static dispatch
  matches the project's preference and there are few implementors; dynamic would
  simplify storing heterogeneous harnesses in one collection (e.g. if planning
  and patching attempts were scheduled together). Recommend static.
- **Mirror as thin payload table vs. relational schema (feature 4).** Thin table
  + `ploke-tree` projection avoids a second authority and reuses
  `Graph::from_records`; a richer schema enables direct Cozo queries but risks
  schema drift and fixture coupling. Recommend thin table now, per ADR.
- **Multi-instance partial-treatment policy (feature 3).** Strict require-all
  (current oracle behavior) vs. partial scoring with explicit missing-instance
  tracking. This changes what "useful branch" means and must not be relaxed
  without explicit approval (correctness guardrail). Flagged for decision.
- **How far to push the boundary error (P3).** Returning `AttemptOutcome` to the
  caller (option b) is more invasive than adding a `PrepareError` variant
  (option a) but gives features 1 and 5 typed failure causes for free. Recommend
  (b); (a) is the lower-effort fallback.

---

## 8. What I verified vs. assumed

Verified by reading code (this session): the tui_adapter module layout and
entrypoints; `BroadEditPolicy` single variant; `GraphRestriction` advisory-only;
the dormant `graph.rs`/`surface.rs` typestate; the dual retry machines; the
`PrepareError` stringization; `EmitRecord` 3-call adoption; scalar `instance_id`;
the `MissingDataSummary`/`ScoreSelectionDiagnostic` carriers; the 10 tool seed
types; LOW-risk blast radius via GitNexus impact on the five primary targets.

Assumed / not exhaustively traced (verify before implementing): the exact
`ploke-db` query surface for "N nearest graph items" (only the `graph::View`
trait shape is confirmed, not a Cozo neighborhood query); the precise set of
remaining ad-hoc record writers (the survey found the main ones but not
necessarily all); and whether any non-`replay` crate consumes
`run_headless_with_model` (impact showed none, but confirm with
`gitnexus_detect_changes` at edit time).
