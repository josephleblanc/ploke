# Repository Guidelines

## Git Safety

- Requirement: do not use force-style git operations without explicit user permission. This includes `git add --force` / `git add -f`, force pushes, forced checkout/reset/clean operations, and any command that overrides an ignored or protected repository boundary.
- Treat ignored files as intentionally outside normal version control. If an ignored file is edited, report it as a local-only change unless the user explicitly asks to commit that ignored path.

## Command Output Discipline

- Requirement: when running `cargo test`, bound the default output to the useful tail or a targeted error filter, such as `cargo test -p <crate> <test-filter> -- <test-args> 2>&1 | tail -n 20`, `cargo test -p <crate> ... 2>&1 | rg 'E[0-9]+'`, or `cargo test -p <crate> ... 2>&1 | rg '<regex>'`.
- Use fuller `cargo test` output only when the bounded output is insufficient to diagnose the failure, and make that expansion explicit.

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

## Anti-Blob Guardrails

- Requirement: do not satisfy a request by piling up ad hoc report, view, info, status, or helper types when the underlying semantic object has not been named. Avoid new `*View`, `*Report`, `*Info`, `*Status`, and `*Output` carriers unless they are downstream projections of an already-defined domain object.
- Requirement: CLI code is dispatch and projection. Do not add semantic authority, active-loop decision logic, or durable state interpretation to broad CLI files such as `cli_facing.rs`; move that logic behind a domain module boundary first.
- Requirement: before adding monitor, timing, debug, or operator-output code, identify the chain explicitly: source facts, semantic fold, projection, renderer. Rendering must not introduce source facts or authority.
- Requirement: do not add a generic trait or type just to make uncertain code look reusable. Traits and generics are appropriate when they enforce an authority boundary, backend adapter, typed state transition, strategy family, or shared semantic interface.
- Requirement: no "minimal slice" that only makes the next command output look right while leaving a worse structure behind. A bounded implementation is fine, but it must establish or preserve the correct boundary.
- Requirement: projection reads must stay behind explicit operator/projection capability boundaries. Active loop paths must use authoritative channels, History, Artifacts, or bootstrap facts, not convenience projection files.
- If names, files, or helper clusters keep growing while solving a local failure, stop and identify the missing semantic object or module boundary before continuing.
- Prefer deleting or isolating stale prototype clutter when it conflicts with the current model. Do not adapt new code around legacy scaffolding merely to preserve it.

## Prototype 1 Caution

- Recent Prototype 1 code may contain agent-introduced scaffolding, duplicated records, overlong helpers, and weak abstractions created while chasing local failures. Do not treat nearby Prototype 1 patterns as authoritative just because they exist.
- When working in `crates/ploke-eval/src/cli/prototype1_state` or adjacent Prototype 1 code, recover the intended model from module docs and current user direction before following local precedent:
  - Every checkout is an Artifact.
  - Every Artifact is a dehydrated Runtime.
  - Every Runtime is a potential Parent.
  - Parent-ness is a role/state, not a backend operation.
  - Parent/child protocol state should be modeled structurally, e.g. `Child<Ready>`, not as flattened event names such as `ChildReady`, `ChildProgress`, or `ChildHeartbeat`.
- If a local Prototype 1 pattern conflicts with that model, patch toward the model. Do not preserve agent-created clutter for consistency.

## Bounded Edit Surface Plan

- Before implementing bounded edit-surface work, read `docs/active/agents/2026-05-08_bounded-edit-surface-implementation-orientation.md`.
- Treat `docs/workflow/evalnomicon/drafts/2026-05-08-bounded-edit-harness-adapter-plan.md` as the active implementation plan for evidence-directed bounded `ploke-tui` edit surfaces.
- Current first route: `invalid_candidate_generation -> semantic_edit_resolution -> ploke-tui semantic edit resolver surface`.
- Use the stable core vocabulary from the orientation doc. Do not add slice-local nouns or objective-specific type families when a field, enum variant, module boundary, or existing carrier can express the structure.
- Implement this plan through vertical slices with splice tests: `recorded/mock upstream output A' -> new implementation B* -> downstream consumer contract C'`.
- For this plan, `ploke-eval` owns grants, checks, History admission, runtime hydration, and successor selection; `ploke-tui` is a harness/executor behind a trait boundary.
- TUI proposal state, CLI output, logs, mutable reports, and monitor views are not source truth. A Parent may diagnose and choose surfaces only from typed evidence or explicitly admitted projections.

## Prototype 1 History Audit

- Operational policy: audit the Prototype 1 History/Crown implementation at least once per week with combined human and LLM review.
- The audit must check that documentation and status claims do not overpromise what the current implementation proves, especially around tamper evidence, Crown authority, and compiler-enforced transition validity.
- The audit must inspect the actual type barriers: private fields, sealed or module-private state markers, constructor visibility, move-only transition methods, and the durable records emitted by those transitions.
- Treat any drift between claimed invariants and implemented constraints as a correctness issue. Fix the implementation, narrow the claim, or record the gap explicitly before relying on the History model for longer runs.

## Prototype 1 Run Playback / Observability Plan

- Before implementing `RunPlayback`, `RunPlaybackRef`, playback iterators, replay CLI commands, `ploke-tree` run projections, or front-facing UI/WebAssembly observability surfaces, start from the shared track index at `docs/active/plans/self-improvement-loop/handoffs.md`.
- Treat `docs/active/agents/2026-05-09_run-playback-typed-observability-plan.md` as the typed playback contract, not necessarily the latest operational handoff.
- Playback is a read/projection layer over persisted typed records. It must not become active loop authority, and it must not parse rendered CLI output.
- Preserve granularity structurally with typestates such as `RunPlayback<Coarse>` and `RunPlayback<Fine>` rather than string modes or report flags.
- Provide both owned and borrowed playback forms. UI-facing paths should be able to use borrowed `RunPlaybackRef<'a, G>` projections over an already-loaded record store without cloning large payloads.
- Coarsening, filtering, or rendering must not upgrade evidence authority. Steps should carry evidence strength separately from playback granularity.
