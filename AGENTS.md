# Repository Guidelines

## Git Safety

- Requirement: do not use force-style git operations without explicit user permission. This includes `git add --force` / `git add -f`, force pushes, forced checkout/reset/clean operations, and any command that overrides an ignored or protected repository boundary.
- Treat ignored files as intentionally outside normal version control. If an ignored file is edited, report it as a local-only change unless the user explicitly asks to commit that ignored path.

## Coding Style & Naming Discipline

- Requirement: do not flatten role/state structure into long compound names when a type parameter, enum state, module boundary, or transition carrier can express it. Prefer `Child<Ready>` over `ChildReady`, `RuntimeTraceEntry::ChildReady`, `ChildHeartbeat`, or `observe_child_ready_state`.
- Requirement: typed transition names must preserve structure. If the domain object is a role in a state, model it as `Role<State>` or an equivalent typed carrier; do not invent a new generic layer like "trace", "admission", "claim", "heartbeat", or "progress" unless that layer is actually part of the domain model.
- Requirement: typed transitions should not be trivial public status writes. Prefer private fields, sealed or module-private state markers, move-only transition methods, and journal records produced by those transitions. The durable record should be the projection of an allowed state transition, not an arbitrary string/status update.
- Do not encode missing structure into long helper names. Repeated prefixes like `prototype1_monitor_*` or `intervention_synthesis_*` usually mean the code wants a narrower module, an enclosing type, or a small context/value carrier.
- If several nearby helpers take the same argument cluster, introduce a local carrier type and make the helpers methods on it. Prefer `ctx.stop_reason(&snapshot)` over `inspect_prototype1_monitor_terminal(manifest_path, prototype_root, snapshot)`.
- If a helper name needs subsystem + command + phase + action to be understandable, first look for the missing boundary. Use module/type context to carry subsystem meaning, then keep local names short and concrete.
- Prefer names that describe the domain result, not the inspection mechanism: `stop_reason`, `changed_paths`, `summary`, `snapshot`, `entry_kind`.
- Do not preserve intent by adding prefixes. Preserve intent with structure: modules, types, enums, traits, and explicit state carriers.

## Prototype 1 Caution

- Recent Prototype 1 code may contain agent-introduced scaffolding, duplicated records, overlong helpers, and weak abstractions created while chasing local failures. Do not treat nearby Prototype 1 patterns as authoritative just because they exist.
- When working in `crates/ploke-eval/src/cli/prototype1_state` or adjacent Prototype 1 code, recover the intended model from module docs and current user direction before following local precedent:
  - Every checkout is an Artifact.
  - Every Artifact is a dehydrated Runtime.
  - Every Runtime is a potential Parent.
  - Parent-ness is a role/state, not a backend operation.
  - Parent/child protocol state should be modeled structurally, e.g. `Child<Ready>`, not as flattened event names such as `ChildReady`, `ChildProgress`, or `ChildHeartbeat`.
- If a local Prototype 1 pattern conflicts with that model, patch toward the model. Do not preserve agent-created clutter for consistency.

## Prototype 1 History Audit

- Operational policy: audit the Prototype 1 History/Crown implementation at least once per week with combined human and LLM review.
- The audit must check that documentation and status claims do not overpromise what the current implementation proves, especially around tamper evidence, Crown authority, and compiler-enforced transition validity.
- The audit must inspect the actual type barriers: private fields, sealed or module-private state markers, constructor visibility, move-only transition methods, and the durable records emitted by those transitions.
- Treat any drift between claimed invariants and implemented constraints as a correctness issue. Fix the implementation, narrow the claim, or record the gap explicitly before relying on the History model for longer runs.
