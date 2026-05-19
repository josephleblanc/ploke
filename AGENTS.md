# Repository Guidelines

## Git Safety

- Requirement: do not use force-style git operations without explicit user permission. This includes `git add --force` / `git add -f`, force pushes, forced checkout/reset/clean operations, and any command that overrides an ignored or protected repository boundary.
- Treat ignored files as intentionally outside normal version control. If an ignored file is edited, report it as a local-only change unless the user explicitly asks to commit that ignored path.

## Command Output Discipline

- Requirement: when running `cargo test`, bound the default output to the useful tail or a targeted error filter, such as `cargo test -p <crate> <test-filter> -- <test-args> 2>&1 | tail -n 20`, `cargo test -p <crate> ... 2>&1 | rg 'E[0-9]+'`, or `cargo test -p <crate> ... 2>&1 | rg '<regex>'`.
- Use fuller `cargo test` output only when the bounded output is insufficient to diagnose the failure, and make that expansion explicit.

## Verification Surface Honesty

- Requirement: when reporting whether a feature was tested, name the exact verified surface in the first sentence. Valid surfaces include `CLI snapshot/export`, `focused egui renderer test`, `native interactive window`, `real-run import`, or `not tested`.
- Requirement: do not let one verification surface stand in for another. In particular, do not describe CLI snapshot inspection as though it proves live egui right-panel behavior, and do not describe focused renderer tests as though they prove native pointer interaction.
- Requirement: for graph or edge rendering changes, distinguish `projected payload visible` from `drawable in the current renderer/readability path`. Do not claim an edge is visible just because `GraphViewDiagnostics.edge_count` or contract reports include it. If the geometry/readability path drops the edge class, or the renderer cannot draw that edge shape (for example self-loops), report it as not visibly rendered until that path is verified.

## Coding Style & Naming Discipline

- Requirement: owned persisted JSON/JSONL data must be read and written through named Rust types. Production code must not parse, inspect, transform, slice, or project owned persisted data through `serde_json::Value`, anonymous field walking, or ad hoc JSON accessors. This applies across `ploke-records`, `ploke-tree`, `ploke-eval`, `ploke-tui`, monitor/debug/projection paths, and future crates. If the project writes the shape, the project owns a `Serialize`/`Deserialize` type for that shape. If a reader needs only part of a record, define a named typed projection struct or enum. Test fixtures may use JSON literals for construction and comparison, but not as production reader/parser precedent.
- Requirement: do not flatten role/state structure into long compound names when a type parameter, enum state, module boundary, or transition carrier can express it. Prefer `Child<Ready>` over `ChildReady`, `RuntimeTraceEntry::ChildReady`, `ChildHeartbeat`, or `observe_child_ready_state`.
- Requirement: typed transition names must preserve structure. If the domain object is a role in a state, model it as `Role<State>` or an equivalent typed carrier; do not invent a new generic layer like "trace", "admission", "claim", "heartbeat", or "progress" unless that layer is actually part of the domain model.
- Requirement: typed transitions should not be trivial public status writes. Prefer private fields, sealed or module-private state markers, move-only transition methods, and journal records produced by those transitions. The durable record should be the projection of an allowed state transition, not an arbitrary string/status update.
- Requirement: do not leave future-only or currently unused Rust code silently. If a type, enum variant, helper, or module is intentionally unused because it preserves a planned invariant or future slice, it must be tracked in `.codex/task-stack.jsonl` and the code must carry a searchable lint reason marker:
  `#[allow(dead_code, reason = "task-stack:<task-id> <short reason>")]`.
  Use `rg 'task-stack:'` to find these markers and verify the referenced task id still exists before preserving or extending the dead code. If there is no task-stack item and no concrete future consumer, delete or defer the code instead of suppressing the lint.
- Do not encode missing structure into long helper names. Repeated prefixes like `prototype1_monitor_*` or `intervention_synthesis_*` usually mean the code wants a narrower module, an enclosing type, or a small context/value carrier.
- If several nearby helpers take the same argument cluster, introduce a local carrier type and make the helpers methods on it. Prefer `ctx.stop_reason(&snapshot)` over `inspect_prototype1_monitor_terminal(manifest_path, prototype_root, snapshot)`.
- If a helper name needs subsystem + command + phase + action to be understandable, first look for the missing boundary. Use module/type context to carry subsystem meaning, then keep local names short and concrete.
- Prefer names that describe the domain result, not the inspection mechanism: `stop_reason`, `changed_paths`, `summary`, `snapshot`, `entry_kind`.
- Do not preserve intent by adding prefixes. Preserve intent with structure: modules, types, enums, traits, and explicit state carriers.

### Structural Naming Stop Rule

When adding or renaming a Rust struct, enum, trait, module, or public helper in Prototype 1, History, surface, harness, parent/child, admission, evidence, or runtime code, stop before editing if the proposed name contains three or more semantic tokens such as `Published`, `Checked`, `Admitted`, `Selected`, `Hydrated`, `Pending`, `Awaiting`, `Request`, `Admission`, `Binding`, `Grant`, `Coordinate`, `Surface`, `Harness`, `Parent`, `Child`, `Evidence`, `History`, `Claim`, `Witness`, `Record`, or `Projection`.

Before writing the code, produce the carrier map: axes being collapsed, structural carrier chosen, transition method that mints the advanced state, record/projection boundary if any, active-loop consumer, and compile-time constraint that would have made the related bug impossible. Cargo check passing does not satisfy this rule.

Refusal script: "I will not encode subsystem + phase + authority + provenance into one identifier." If a compound name is only a durable external record shape, put it behind a `record` or projection boundary and keep active loop code on structural carriers.

### Structural Naming Bug Ledger

Maintain this list when a bug is discovered that would have been prevented by preserving role/state or relation structure in names and types. These are not style complaints. They are type-system failures caused by collapsed names. Each entry must name the bug report, the affected files, the collapsed name/shape, the missing structure that should have been modeled, and the type constraint that would have made the invalid state unrepresentable.

- `docs/active/bugs/2026-05-09-prototype1-history-traversal-membership-mismatch.md`
  - Affected files:
    - `crates/ploke-eval/src/successor_selection/traversal.rs`
    - `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs`
    - `crates/ploke-eval/src/cli/prototype1_state/history.rs`
  - Collapsed shape: `candidate_set_membership` / `selected_membership_id` treated source-set membership and final decision-set membership as the same relation.
  - Missing structure: source candidate-set membership vs sealed decision candidate-set membership should be distinct role/state carriers, e.g. `Membership<SourceSet>` and `Membership<DecisionSet>`, or equivalent module/type boundaries.
  - Preventing type constraint: traversal may carry `Membership<SourceSet>` only as evidence; sealed selection constructors must require `Membership<DecisionSet>` minted or resolved from the final candidate set. No API may accept a bare membership id where the set role is not encoded.
- `docs/active/bugs/2026-05-10-prototype1-historical-successor-surface-root-mismatch.md`
  - Affected files:
    - `crates/ploke-eval/src/cli/prototype1_process.rs`
    - `crates/ploke-eval/src/cli/prototype1_state/parent.rs`
    - `crates/ploke-eval/src/cli/prototype1_state/history.rs`
  - Collapsed shape: bare `SurfaceCommitment` and `active_parent_root` let the handoff path treat previous-parent surface roots and selected-successor surface roots as the same relation.
  - Missing structure: the selected Artifact tree key and selected Artifact surface commitment should be one carrier minted by the install/materialize transition, e.g. `SelectedArtifactCommitment` or `SurfaceTransition<CurrentParentBefore, SelectedArtifactAfter>`.
  - Preventing type constraint: History sealing must require a selected-Artifact commitment that bundles Artifact identity, tree key, and surface roots from the same backend transition. No API may seal `ArtifactRef`, `TreeKeyHash`, and `SurfaceCommitment` as independent arguments.
- `docs/active/bugs/2026-05-10-prototype1-successor-hydration-surface-mismatch.md`
  - Affected files:
    - `crates/ploke-eval/src/cli/prototype1_process.rs`
    - `crates/ploke-eval/src/cli/prototype1_state/history.rs`
    - `crates/ploke-eval/src/cli/prototype1_state/identity.rs`
  - Collapsed shape: bare `ArtifactSurface` / `selected_surface` / `InstalledSuccessorArtifact` let the handoff path compare selected-child Artifact surface evidence to the hydrated successor Parent checkout after `parent_identity.json` was committed.
  - Missing structure: selected-child Artifact surface and hydrated-successor Parent surface should be distinct role/state carriers, e.g. `ArtifactSurface<SelectedChild>` and `ArtifactSurface<HydratedSuccessorParent>`, connected only by a `SuccessorHydration<SelectedChild, HydratedSuccessorParent>` transition.
  - Preventing type constraint: selected-child surface validation must happen before parent hydration; post-hydration startup surface must be minted by the hydration transition. No API may compare or seal a bare `ArtifactSurface` without encoding which artifact/parent state it measures.
- `docs/active/bugs/2026-05-12-prototype1-broad-harness-request-plan-erasure.md`
  - Affected files:
    - `crates/ploke-eval/src/cli.rs`
    - `crates/ploke-eval/src/cli/prototype1_state/profile.rs`
    - `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs`
    - `crates/ploke-eval/src/cli/prototype1_state/edit_surface/harness_request.rs`
    - `crates/ploke-eval/src/cli/prototype1_state/edit_surface/surface.rs`
    - `crates/ploke-eval/src/cli/prototype1_state/edit_surface/harness.rs`
    - `crates/ploke-eval/src/cli/prototype1_state/history.rs`
  - Collapsed shape: `BroadHarness` / `requires_surface_evidence` / `validate_requested_tui_surface_child` treated pending broad harness requests, deterministic TUI child plans, and future Router-backed harness plans as the same generator/provenance relation.
  - Missing structure: published harness request, awaiting Parent state, request-bound child plan, and harness-specific surface evidence should be distinct role/state carriers, e.g. `HarnessRequest<Broad, Published>`, `Parent<AwaitingHarnessPlan<Broad>>`, `ChildPlan<For<HarnessRequest<Broad, Published>>>`, `SurfaceEvidence<DeterministicTuiTools>`, and `SurfaceEvidence<RouterBackedHarness>`.
  - Preventing type constraint: a complete live run may only materialize children from a generator state that has produced typed child-plan evidence for that generator. No API may satisfy `BroadHarness` with an unbound `ChildPlan`, and deterministic TUI evidence must require non-Router provenance unless a separate Router-backed harness carrier is present.

## Anti-Blob Guardrails

- Requirement: do not satisfy a request by piling up ad hoc report, view, info, status, or helper types when the underlying semantic object has not been named. Avoid new `*View`, `*Report`, `*Info`, `*Status`, and `*Output` carriers unless they are downstream projections of an already-defined domain object.
- Requirement: CLI code is dispatch and projection. Do not add semantic authority, active-loop decision logic, or durable state interpretation to broad CLI files such as `cli_facing.rs`; move that logic behind a domain module boundary first.
- Requirement: before adding monitor, timing, debug, or operator-output code, identify the chain explicitly: source facts, semantic fold, projection, renderer. Rendering must not introduce source facts or authority.
- Requirement: do not add a generic trait or type just to make uncertain code look reusable. Traits and generics are appropriate when they enforce an authority boundary, backend adapter, typed state transition, strategy family, or shared semantic interface.
- Requirement: no "minimal slice" that only makes the next command output look right while leaving a worse structure behind. A bounded implementation is fine, but it must establish or preserve the correct boundary.
- Requirement: projection reads must stay behind explicit operator/projection capability boundaries. Active loop paths must use authoritative channels, History, Artifacts, or bootstrap facts, not convenience projection files.
- If names, files, or helper clusters keep growing while solving a local failure, stop and identify the missing semantic object or module boundary before continuing.
- Prefer deleting or isolating stale prototype clutter when it conflicts with the current model. Do not adapt new code around legacy scaffolding merely to preserve it.

## Typed UI Projection Style

- Requirement: `ploke-egui` is a typed projection of `ploke-tree::Graph`, not a string-rendering layer. Preserve semantic graph facts as typed borrowed values until the egui render boundary.
- Requirement: for `ploke-egui` graph, inspector, timeline, source-ref, default-view, plan, or profiling work, start from `crates/ploke-egui/docs/README.md` and follow the relevant directory README. Do not bypass the README indexes by hard-coding a stale doc path from memory.
- Requirement: for graph/view/inspector semantics, start from `crates/ploke-egui/docs/model/README.md` and follow its current links. For bounded UI plans, start from `crates/ploke-egui/docs/plan/README.md`. For performance work, start from `crates/ploke-egui/docs/profiling/README.md`.
- Requirement: before adding, changing, or explaining a UI-facing semantic claim with ambiguous identity/provenance carriers, use both project skills `ploke-debugger-claim-workflow` and `ui-claim-archaeology`.
- Requirement: before editing an archaeology report inside an existing `docs/active/archaeology/` group, re-read the local archaeology set: `README.md`, `INDEX.md`, the group `README.md`, and at least one sibling report in that group. Do not update one report in isolation and call the archaeology pass complete.
- Requirement: archaeology report carrier tables must use concrete type/path references in `where it appears`. Do not use vague buckets like `scheduler records`, `child-plan record`, or `sealed History fields` when a real type/field path is available.
- Requirement: do not cite a file, type, helper, or field in archaeology work unless you actually read it in the current pass. If an entry is not yet grounded, mark it `unverified` and stop instead of filling it with plausible text.
- Requirement: do not collapse Artifact, Runtime, role, History, candidate, or evidence facts into `InspectorRow`, string labels, or row-shaped carriers. If the UI fact means `Parent`, `Child`, selected Artifact, Runtime role, or evidence relation, carry that meaning in the type.
- Requirement: visual row layout may exist only inside the renderer. Rows are not semantic objects, cache entries, inspector facts, or projection carriers. Text-only rendering belongs at the egui/text boundary after typed facts have already been selected.
- Requirement: prefer graph-derived typed witnesses such as `Badge::Child(&ArtifactId)` or `Badge::Parent(&ArtifactId)` over labels like `"child"` or `"parent"`. The carried id is a binding: it proves the rendered item is attached to an underlying semantic object and should make detached role labels hard to construct.
- Use lifetimes intentionally. UI projections may borrow from `Graph` during the app render pass instead of cloning ids or inventing owned DTOs. Only introduce owned cached forms at an explicit cache boundary with graph invalidation/keying.
- Render objects such as `egui::RichText` are boundary projections. They may be cached for performance, but they must not replace the typed semantic source.
- Traits are appropriate when they preserve this pattern ergonomically across semantic objects such as Artifact and Runtime. Do not add a trait just to hide uncertainty about the source fact.
- Keep styling centralized when practical. Role colors and badge styling should come from a shared theme/profile surface rather than scattered per-call literals.

## Prototype 1 Caution

- Recent Prototype 1 code may contain agent-introduced scaffolding, duplicated records, overlong helpers, and weak abstractions created while chasing local failures. Do not treat nearby Prototype 1 patterns as authoritative just because they exist.
- When working in `crates/ploke-eval/src/cli/prototype1_state` or adjacent Prototype 1 code, recover the intended model from module docs and current user direction before following local precedent:
  - Every checkout is an Artifact.
  - Every Artifact is a dehydrated Runtime.
  - Every Runtime is a potential Parent.
  - Parent-ness is a role/state, not a backend operation.
  - Parent/child protocol state should be modeled structurally, e.g. `Child<Ready>`, not as flattened event names such as `ChildReady`, `ChildProgress`, or `ChildHeartbeat`.
- If a local Prototype 1 pattern conflicts with that model, patch toward the model. Do not preserve agent-created clutter for consistency.

## Collaboration Incident Ledger

- When the user expresses anger, frustration, alarm, or trust loss caused by agent behavior, create or update an incident log under `docs/active/agents/collaboration-incidents/`.
- Group incidents by failure family in subdirectories, keep every directory indexed with a `README.md`, and add the new area to `docs/active/agents/readme.md` when needed.
- Each incident entry must record: trigger, user-visible failure, touched code surface, skipped docs/skills/instructions, why the behavior was risky, and the concrete prevention rule for next time.
- If the incident reveals a durable repo-level workflow gap, update `AGENTS.md` or the relevant skill in the same turn when feasible.
- Use the `collaboration-incident-logging` skill when this trigger fires.
- When the user signals that an architecture explanation became confusing,
  reset to a concrete recommendation in the local subsystem's terms before
  continuing with broader authority, protocol, or formal-model language.
- For broad-harness edit-surface work, do not solve safety by defaulting to a
  one-edit harness. Preserve the goal of broad model autonomy and put safety at
  the admission, batching, refresh, and evidence-counting boundaries.

## Bounded Edit Surface Plan

- Before implementing bounded edit-surface work, read `docs/active/agents/2026-05-08_bounded-edit-surface-implementation-orientation.md`.
- Treat `docs/workflow/evalnomicon/drafts/2026-05-08-bounded-edit-harness-adapter-plan.md` as the active implementation plan for evidence-directed bounded `ploke-tui` edit surfaces.
- Current first route: `invalid_candidate_generation -> semantic_edit_resolution -> ploke-tui semantic edit resolver surface`.
- Use the stable core vocabulary from the orientation doc. Do not add slice-local nouns or objective-specific type families when a field, enum variant, module boundary, or existing carrier can express the structure.
- Implement this plan through vertical slices with splice tests: `recorded/mock upstream output A' -> new implementation B* -> downstream consumer contract C'`.
- For this plan, `ploke-eval` owns grants, checks, History admission, runtime hydration, and successor selection; `ploke-tui` is a harness/executor behind a trait boundary.
- TUI proposal state, CLI output, logs, mutable reports, and monitor views are not source truth. A Parent may diagnose and choose surfaces only from typed evidence or explicitly admitted projections.

## Agent-Turn Record Projection Work

- Before extending `agent-turn-trace.json`, `agent-turn-summary.json`, or tool UI/error payload persistence, read `docs/active/agents/2026-05-12_agent-turn-record-projection-handoff.md`.
- Treat `ploke-records::agent_turn` as the canonical owner of the persisted `agent-turn` schema.
- Treat `crates/ploke-eval/src/runner.rs` as the current live-to-record writer boundary for `agent-turn` artifacts unless the crate graph is intentionally restructured first.
- Do not expand the feature-gated `ploke-records <-tool_contracts-> ploke-tui` bridge to solve new persisted-schema ownership needs when an explicit records-owned projection will do.

## Prototype 1 History Audit

- Operational policy: audit the Prototype 1 History/Crown implementation at least once per week with combined human and LLM review.
- The audit must check that documentation and status claims do not overpromise what the current implementation proves, especially around tamper evidence, Crown authority, and compiler-enforced transition validity.
- The audit must inspect the actual type barriers: private fields, sealed or module-private state markers, constructor visibility, move-only transition methods, and the durable records emitted by those transitions.
- Treat any drift between claimed invariants and implemented constraints as a correctness issue. Fix the implementation, narrow the claim, or record the gap explicitly before relying on the History model for longer runs.

## ploke-egui Graph / Observability Docs

- Before changing the default `ploke-egui` graph projection, graph view mode, inspector source model, source-ref handling, timeline shell, or UI/WebAssembly observability surface, start from `crates/ploke-egui/docs/README.md`, then follow `crates/ploke-egui/docs/model/README.md`. The README indexes are the current routing surface for how `ploke-egui` turns `ploke_tree::Graph` into an interactive inspection UI.
- Before changing default artifact-graph semantics, follow the `crates/ploke-egui/docs/model/README.md` reading path and restate the intended default node set and edge set before editing.
- The default `ploke-egui` graph is the artifact-first `ArtifactTree` projection over `ploke_tree::Graph`: rendered nodes are resolved Artifact identities, rendered default edges are `P_H union P_C`, and History/candidate/runtime/tool/provider records are context, marks, diagnostics, or drilldown material unless a non-default projection admits them explicitly.
- Do not make the default graph a full record graph, History-block chain, candidate inventory pile, runtime/tool/agent-turn graph, or synthetic-anchor debug surface. Those belong in explicit debug modes, side diagnostics, timeline lanes, or typed drilldown.
- `ploke-egui` is read-only inspection. It does not choose, admit, seal, or advance successors. Runtime records, reports, protocol artifacts, and benchmark outputs must enter through named typed records or typed graph projections before the UI uses them.
- For evaluation, protocol, and run-record source locations, start from `crates/ploke-egui/docs/model/README.md` and follow its data-location entry. Do not make `ploke-egui` parse run JSON, compressed records, rendered CLI output, or copied path strings directly.
- For performance-affecting `ploke-egui` edits, read `crates/ploke-egui/docs/profiling/README.md` and keep the benchmark/report requirements from the local `ploke-egui-benchmarking` skill. Treat allocation churn in the right inspector and graph canvas as a first-class regression surface.
- Use `docs/active/plans/self-improvement-loop/typed-persistence-spine/operating-console.md` and its implementation queue when the task is a typed-persistence or graph-ingestion slice. Use older observability notes only when routed by a current README/index or explicitly requested.
