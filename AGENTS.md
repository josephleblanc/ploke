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
- Requirement: before reporting what a targeted test proves, read the test body or the helper it delegates to and name the concrete assertion-backed behavior. A passing test name plus Cargo output is not enough evidence for a behavioral claim.
- Requirement: when the user narrows acceptance to one live surface, do not add skip helpers, cheap substitute tests, or adjacent passing surfaces as progress. Run that surface directly, and if it fails, report provider/tool/state/assertion evidence separately.

## Context Budget / Large File Guardrails

- Context explosion counter:
  - Total incidents: 2
  - Last context explosion: 2026-05-11
  - Days since last context explosion: 0

- Requirement: do not read large logs, journals, generated artifacts, or long documents directly. Before opening any unknown-size file, use metadata guardrails such as `wc -l` and `ls -lh` to estimate both line count and byte size.
- Requirement: for JSONL, journals, traces, or other record-oriented files, a small line count is not enough. Lines may be huge, so content reads must cap both record count and record width, for example `rg -n '<pattern>' <file> | head -n 5 | cut -c 1-400`, `sed -n 'start,endp' <file> | cut -c 1-400`, or `tail -n 3 <file> | cut -c 1-400`.
- Requirement: do not read `transition-journal.jsonl` or similar run/history journals wholesale. Query them only with narrow `rg` patterns, bounded line ranges, or very small tails, always with a width cap such as `cut -c 1-400`, unless the user explicitly asks for a full read.
- Requirement added 2026-05-11: repeated health checks for Prototype 1 campaigns must be metadata-first. Use admitted node counts/statuses, file mtimes, byte sizes, line counts, transition-file mtimes, and disk usage as the default signal. Do not read JSONL content during routine health checks.
- Requirement added 2026-05-11: JSONL content reads during Prototype 1 health checks are anomaly diagnostics only. If an anomaly requires content inspection, read at most one explicitly named file, at most three records, and at most 400 characters per record, for example `tail -n 3 <file> | cut -c 1-400`. Do not run broad `rg` over multiple observation streams or journal files for routine health.
- Requirement added 2026-05-11: if the user asks for repeated loop health checks, preserve context by reporting compact deltas from the last check: node count, generation range, status counts, newest mtime, and disk free. Do not restate or dump raw trace records unless the user explicitly asks for raw evidence.
- Requirement: docs are not exempt from context discipline. When consulting a doc, first identify the needed section with `rg` or inspect a bounded range; avoid dumping whole planning documents into context.
- If a bounded read is insufficient, state what extra range or pattern is needed before expanding it, and keep the expansion targeted.

## Coding Style & Naming Discipline

- Requirement: owned persisted JSON/JSONL data must be read and written through named Rust types. Production code must not parse, inspect, transform, slice, or summarize owned persisted data through `serde_json::Value`, anonymous field walking, or ad hoc JSON accessors. This applies across `ploke-records`, `ploke-tree`, `ploke-eval`, `ploke-tui`, monitor/debug paths, and future crates. If the project writes the shape, the project owns a `Serialize`/`Deserialize` type for that shape.
- Requirement: do not introduce smaller mirror DTOs or one-off subset record types just because a reader needs only part of an existing record. Prefer the existing owner type, borrowed accessors over that type, or an explicit extension of the canonical schema. A new record type is appropriate only when it is a real persisted/wire schema or a reusable domain object with an owning module, not as a convenience slice of another type.
- Requirement: do not flatten role/state structure into long compound names when a type parameter, enum state, module boundary, or transition method can express it. Prefer `Child<Ready>` over `ChildReady`, `RuntimeTraceEntry::ChildReady`, `ChildHeartbeat`, or `observe_child_ready_state`.
- Requirement: typed transition names must preserve structure. If the domain object is a role in a state, model it as `Role<State>` or an equivalent type; do not invent a new generic layer like "trace", "admission", "claim", "heartbeat", or "progress" unless that layer is actually part of the domain model.
- Requirement: typed transitions should not be trivial public status writes. Prefer private fields, sealed or module-private state markers, move-only transition methods, and journal records produced by those transitions. The durable record should be the projection of an allowed state transition, not an arbitrary string/status update.
- Requirement: do not leave future-only or currently unused Rust code silently. If a type, enum variant, helper, or module is intentionally unused because it preserves a planned invariant or future slice, it must be tracked in `.codex/task-stack.jsonl` and the code must carry a searchable lint reason marker:
  `#[allow(dead_code, reason = "task-stack:<task-id> <short reason>")]`.
  Use `rg 'task-stack:'` to find these markers and verify the referenced task id still exists before preserving or extending the dead code. If there is no task-stack item and no concrete future consumer, delete or defer the code instead of suppressing the lint.
- Do not encode missing structure into long helper names. Repeated prefixes like `prototype1_monitor_*` or `intervention_synthesis_*` usually mean the code wants a narrower module, an enclosing type, or a small context/value type.
- If several nearby helpers take the same argument cluster, introduce a small local type and make the helpers methods on it. Prefer `ctx.stop_reason(&snapshot)` over `inspect_prototype1_monitor_terminal(manifest_path, prototype_root, snapshot)`.
- If a helper name needs subsystem + command + phase + action to be understandable, first look for the missing boundary. Use module/type context to carry subsystem meaning, then keep local names short and concrete.
- Prefer names that describe the domain result, not the inspection mechanism: `stop_reason`, `changed_paths`, `summary`, `snapshot`, `entry_kind`.
- Do not preserve intent by adding prefixes. Preserve intent with modules, types, enums, traits, and explicit states.

### Structural Naming Stop Rule

When adding or renaming a Rust struct, enum, trait, module, or public helper in Prototype 1, History, surface, harness, parent/child, admission, evidence, or runtime code, stop before editing if the proposed name contains three or more semantic tokens such as `Published`, `Checked`, `Admitted`, `Selected`, `Hydrated`, `Pending`, `Awaiting`, `Request`, `Admission`, `Binding`, `Grant`, `Coordinate`, `Surface`, `Harness`, `Parent`, `Child`, `Evidence`, `History`, `Claim`, `Witness`, `Record`, or `Projection`.

Before writing the code, write the concrete map: what role/state/policy facts are being mixed, which existing or new type owns each fact, which transition method creates the next state, which persisted record is written if any, which active-loop code consumes it, and which compile-time rule would have made the related bug impossible. Cargo check passing does not satisfy this rule.

Refusal script: "Do not cram subsystem, phase, policy, and source into one identifier." If a compound name is only a durable external record shape, keep it at the record boundary and keep active loop code on explicit role/state types.

### Structural Naming Bug Ledger

Maintain this list when a bug is discovered that would have been prevented by preserving role/state or relation structure in names and types. These are not style complaints. They are type-system failures caused by collapsed names. Each entry must name the bug report, the affected files, the collapsed name/shape, the missing structure that should have been modeled, and the type constraint that would have made the invalid state unrepresentable.

- `docs/active/bugs/2026-05-09-prototype1-history-traversal-membership-mismatch.md`
  - Affected files:
    - `crates/ploke-eval/src/successor_selection/traversal.rs`
    - `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs`
    - `crates/ploke-eval/src/cli/prototype1_state/history.rs`
  - Collapsed shape: `candidate_set_membership` / `selected_membership_id` treated source-set membership and final decision-set membership as the same relation.
  - Missing structure: source candidate-set membership vs sealed decision candidate-set membership should be distinct role/state types, e.g. `Membership<SourceSet>` and `Membership<DecisionSet>`, or equivalent module/type boundaries.
  - Preventing type constraint: traversal may carry `Membership<SourceSet>` only as evidence; sealed selection constructors must require `Membership<DecisionSet>` minted or resolved from the final candidate set. No API may accept a bare membership id where the set role is not encoded.
- `docs/active/bugs/2026-05-10-prototype1-historical-successor-surface-root-mismatch.md`
  - Affected files:
    - `crates/ploke-eval/src/cli/prototype1_process.rs`
    - `crates/ploke-eval/src/cli/prototype1_state/parent.rs`
    - `crates/ploke-eval/src/cli/prototype1_state/history.rs`
  - Collapsed shape: bare `SurfaceCommitment` and `active_parent_root` let the handoff path treat previous-parent surface roots and selected-successor surface roots as the same relation.
  - Missing structure: the selected Artifact tree key and selected Artifact surface commitment should be one value minted by the install/materialize transition, e.g. `SelectedArtifactCommitment` or `SurfaceTransition<CurrentParentBefore, SelectedArtifactAfter>`.
  - Preventing type constraint: History sealing must require a selected-Artifact commitment that bundles Artifact identity, tree key, and surface roots from the same backend transition. No API may seal `ArtifactRef`, `TreeKeyHash`, and `SurfaceCommitment` as independent arguments.
- `docs/active/bugs/2026-05-10-prototype1-successor-hydration-surface-mismatch.md`
  - Affected files:
    - `crates/ploke-eval/src/cli/prototype1_process.rs`
    - `crates/ploke-eval/src/cli/prototype1_state/history.rs`
    - `crates/ploke-eval/src/cli/prototype1_state/identity.rs`
  - Collapsed shape: bare `ArtifactSurface` / `selected_surface` / `InstalledSuccessorArtifact` let the handoff path compare selected-child Artifact surface evidence to the hydrated successor Parent checkout after `parent_identity.json` was committed.
  - Missing structure: selected-child Artifact surface and hydrated-successor Parent surface should be distinct role/state types, e.g. `ArtifactSurface<SelectedChild>` and `ArtifactSurface<HydratedSuccessorParent>`, connected only by a `SuccessorHydration<SelectedChild, HydratedSuccessorParent>` transition.
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
  - Missing structure: published harness request, awaiting Parent state, request-bound child plan, and harness-specific surface evidence should be distinct role/state types, e.g. `HarnessRequest<Broad, Published>`, `Parent<AwaitingHarnessPlan<Broad>>`, `ChildPlan<For<HarnessRequest<Broad, Published>>>`, `SurfaceEvidence<DeterministicTuiTools>`, and `SurfaceEvidence<RouterBackedHarness>`.
  - Preventing type constraint: a complete live run may only materialize children from a generator state that has produced typed child-plan evidence for that generator. No API may satisfy `BroadHarness` with an unbound `ChildPlan`, and deterministic TUI evidence must require non-Router source information unless a separate Router-backed harness type is present.

## Anti-Blob Guardrails

- Requirement: do not satisfy a request by piling up ad hoc report, view, info, status, or helper types when the real domain object has not been named. Avoid new `*View`, `*Report`, `*Info`, `*Status`, and `*Output` types unless they are render/output forms of an existing domain type.
- Requirement: CLI code is dispatch and projection. Do not add semantic authority, active-loop decision logic, or durable state interpretation to broad CLI files such as `cli_facing.rs`; move that logic behind a domain module boundary first.
- Requirement: before adding monitor, timing, debug, or operator-output code, identify the chain explicitly: input record, domain decision, output format, renderer. Rendering must not invent source facts or authority.
- Requirement: do not add a generic trait or type just to make uncertain code look reusable. Traits and generics are appropriate when they enforce an authority boundary, backend adapter, typed state transition, strategy family, or shared semantic interface.
- Requirement: no "minimal slice" that only makes the next command output look right while leaving a worse structure behind. A bounded implementation is fine, but it must establish or preserve the correct boundary.
- Requirement: operator-output reads must stay behind explicit read-only boundaries. Active loop paths must use authoritative channels, History, Artifacts, or bootstrap facts, not convenience output files.
- If names, files, or helper clusters keep growing while solving a local failure, stop and identify the missing domain type or module boundary before continuing.
- Prefer deleting or isolating stale prototype clutter when it conflicts with the current model. Do not adapt new code around legacy scaffolding merely to preserve it.

## Typed UI Data Style

- Requirement: `ploke-egui` reads from `ploke-tree::Graph`; it is not a string-rendering or mirror-DTO layer. Preserve graph facts as typed borrowed values until the egui render boundary.
- Requirement: do not collapse Artifact, Runtime, role, History, candidate, or evidence facts into `InspectorRow`, string labels, or row-shaped types. If the UI fact means `Parent`, `Child`, selected Artifact, Runtime role, or evidence relation, carry that meaning in the type.
- Requirement: visual row layout may exist only inside the renderer. Rows are not domain objects, cache entries, or inspector facts. Text-only rendering belongs at the egui/text boundary after typed facts have already been selected.
- Requirement: prefer graph-derived typed witnesses such as `Badge::Child(&ArtifactId)` or `Badge::Parent(&ArtifactId)` over labels like `"child"` or `"parent"`. The carried id is a binding: it proves the rendered item is attached to an underlying semantic object and should make detached role labels hard to construct.
- Use lifetimes intentionally. UI views may borrow from `Graph` during the app render pass instead of cloning ids or inventing owned DTOs. Only introduce owned cached forms at an explicit cache boundary with graph invalidation/keying.
- Render objects such as `egui::RichText` are renderer artifacts. They may be cached for performance, but they must not replace the graph/source data.
- Traits are appropriate when they preserve this pattern ergonomically across domain objects such as Artifact and Runtime. Do not add a trait just to hide uncertainty about the source fact.
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

## Bounded Edit Surface Plan

- Before implementing bounded edit-surface work, read `docs/active/agents/2026-05-08_bounded-edit-surface-implementation-orientation.md`.
- Treat `docs/workflow/evalnomicon/drafts/2026-05-08-bounded-edit-harness-adapter-plan.md` as the active implementation plan for evidence-directed bounded `ploke-tui` edit surfaces.
- Current first route: `invalid_candidate_generation -> semantic_edit_resolution -> ploke-tui semantic edit resolver surface`.
- Use the stable core vocabulary from the orientation doc. Do not add slice-local nouns or objective-specific type families when a field, enum variant, module boundary, or existing type can express the structure.
- Implement this plan through vertical slices with splice tests: `recorded/mock upstream output A' -> new implementation B* -> downstream consumer contract C'`.
- For this plan, `ploke-eval` owns grants, checks, History admission, runtime hydration, and successor selection; `ploke-tui` is a harness/executor behind a trait boundary.
- TUI proposal state, CLI output, logs, mutable reports, and monitor views are not source truth. A Parent may diagnose and choose surfaces only from typed evidence or explicitly admitted source facts.

## Agent-Turn Persisted Record Work

- Before extending `agent-turn-trace.json`, `agent-turn-summary.json`, or tool UI/error payload persistence, read `docs/active/agents/2026-05-12_agent-turn-record-projection-handoff.md`.
- Treat `ploke-records::agent_turn` as the canonical owner of the persisted `agent-turn` schema.
- Treat `crates/ploke-eval/src/runner.rs` as the current live-to-record writer boundary for `agent-turn` artifacts unless the crate graph is intentionally restructured first.
- Do not expand the feature-gated `ploke-records <-tool_contracts-> ploke-tui` bridge to solve new persisted-schema ownership needs. Also do not create smaller records-owned subset DTOs as a shortcut; use the canonical `agent-turn` owner types or add a real reusable schema there.

## Prototype 1 History Audit

- Operational policy: audit the Prototype 1 History/Crown implementation at least once per week with combined human and LLM review.
- The audit must check that documentation and status claims do not overpromise what the current implementation proves, especially around tamper evidence, Crown authority, and compiler-enforced transition validity.
- The audit must inspect the actual type barriers: private fields, sealed or module-private state markers, constructor visibility, move-only transition methods, and the durable records emitted by those transitions.
- Treat any drift between claimed invariants and implemented constraints as a correctness issue. Fix the implementation, narrow the claim, or record the gap explicitly before relying on the History model for longer runs.

## Prototype 1 Run Playback / Observability Plan

- Before implementing `RunPlayback`, `RunPlaybackRef`, playback iterators, replay CLI commands, `ploke-tree` run projections, or front-facing UI/WebAssembly observability surfaces, start from the shared track index at `docs/active/plans/self-improvement-loop/handoffs.md`.
- Before changing the default `ploke-egui` tree graph projection or graph view mode, read `docs/active/agents/ploke-ui-task-readability/artifact-tree-default/README.md`.
- The default `ploke-egui` graph is an artifact-first tree/DAG: Artifact states are primary nodes, applied patch / derivation relations are primary edges, and History is reveal/dimming/highlight state rather than the canvas spine.
- Do not make the default graph a full record graph, History-block chain, candidate inventory pile, runtime/tool/agent-turn graph, or synthetic-anchor debug surface. Those belong in explicit debug modes, side diagnostics, or typed drilldown.
- Treat `docs/active/agents/2026-05-09_run-playback-typed-observability-plan.md` as the typed playback contract, not necessarily the latest operational handoff.
- Playback is a read/projection layer over persisted typed records. It must not become active loop authority, and it must not parse rendered CLI output.
- Preserve granularity structurally with typestates such as `RunPlayback<Coarse>` and `RunPlayback<Fine>` rather than string modes or report flags.
- Provide both owned and borrowed playback forms. UI-facing paths should be able to use borrowed `RunPlaybackRef<'a, G>` projections over an already-loaded record store without cloning large payloads.
- Coarsening, filtering, or rendering must not upgrade evidence authority. Steps should carry evidence strength separately from playback granularity.
