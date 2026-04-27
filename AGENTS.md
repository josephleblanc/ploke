# Repository Guidelines

## Coding Style & Naming Discipline

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
- If a local Prototype 1 pattern conflicts with that model, patch toward the model. Do not preserve agent-created clutter for consistency.
