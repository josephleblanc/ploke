# Repository Guidelines

## Git Safety

- Requirement: do not use force-style git operations without explicit user permission. This includes `git add --force` / `git add -f`, force pushes, forced checkout/reset/clean operations, and any command that overrides an ignored or protected repository boundary.
- Treat ignored files as intentionally outside normal version control. If an ignored file is edited, report it as a local-only change unless the user explicitly asks to commit that ignored path.

## Command Output Discipline

- Requirement: when running `cargo test`, bound the default output to the useful tail or a targeted error filter, such as `cargo test -p <crate> <test-filter> -- <test-args> 2>&1 | tail -n 20`, `cargo test -p <crate> ... 2>&1 | rg 'E[0-9]+'`, or `cargo test -p <crate> ... 2>&1 | rg '<regex>'`.
- Use fuller `cargo test` output only when the bounded output is insufficient to diagnose the failure, and make that expansion explicit.

## Token Budget / Sub-Agent Model Routing

- Prefer fewer sub-agents first. A cheaper sub-agent still spends tokens if it reads too broadly or duplicates work already done in the main thread.
- Model routing should account for both model capability and plan-credit cost. Treat output as especially expensive, so ask sub-agents for compact reports rather than long explanations.
- Prefer `gpt-5.3-codex-spark` for bounded discovery and very small mechanical tasks when it is available.
- Use `gpt-5.4-mini` as the cheap fallback for summarization, search narrowing, compile-error triage, mechanical edits, and tests from an obvious pattern.
- Use `gpt-5.3-codex` for bounded implementation work where Spark or mini is likely too weak but the task is still local and code-shaped.
- Use `gpt-5.4` for harder review, diagnosis, or multi-file reasoning when 5.5 is not clearly justified.
- Reserve `gpt-5.5` for architectural changes, Prototype 1 History/Crown/Runtime/Artifact invariants, subtle Rust type/lifetime/async/concurrency reasoning, multi-file edits where semantic authority matters, diagnosing agent/tool workflow failures, and final review before applying or accepting a high-risk patch.
- Use Spark or mini for finding where a symbol, file, command, or behavior is defined; summarizing a module or narrow file cluster; small refactors with obvious local patterns; mechanical edits; writing tests from an existing nearby pattern; checking a compile error and proposing a narrow fix; and quick diff review before escalating.
- Ask Spark and mini agents for exact file paths and line ranges, compact findings, commands used, and the smallest verification command. Do not ask them to dump large logs, artifacts, JSON records, prompts, responses, benchmark payloads, or broad search output.
- Escalate only after a cheaper model has narrowed the search surface, unless the task is already known to require invariant-sensitive reasoning.

## Coding Style & Naming Discipline

- Requirement: do not flatten role/state structure into long compound names when a type parameter, enum state, module boundary, or transition carrier can express it. Prefer `Child<Ready>` over `ChildReady`, `RuntimeTraceEntry::ChildReady`, `ChildHeartbeat`, or `observe_child_ready_state`.
- Requirement: typed transition names must preserve structure. If the domain object is a role in a state, model it as `Role<State>` or an equivalent typed carrier; do not invent a new generic layer like "trace", "admission", "claim", "heartbeat", or "progress" unless that layer is actually part of the domain model.
- Requirement: typed transitions should not be trivial public status writes. Prefer private fields, sealed or module-private state markers, move-only transition methods, and journal records produced by those transitions. The durable record should be the projection of an allowed state transition, not an arbitrary string/status update.
- Do not encode missing structure into long helper names. Repeated prefixes like `prototype1_monitor_*` or `intervention_synthesis_*` usually mean the code wants a narrower module, an enclosing type, or a small context/value carrier.
- If several nearby helpers take the same argument cluster, introduce a local carrier type and make the helpers methods on it. Prefer `ctx.stop_reason(&snapshot)` over `inspect_prototype1_monitor_terminal(manifest_path, prototype_root, snapshot)`.
- If a helper name needs subsystem + command + phase + action to be understandable, first look for the missing boundary. Use module/type context to carry subsystem meaning, then keep local names short and concrete.
- Prefer names that describe the domain result, not the inspection mechanism: `stop_reason`, `changed_paths`, `summary`, `snapshot`, `entry_kind`.
- Do not preserve intent by adding prefixes. Preserve intent with structure: modules, types, enums, traits, and explicit state carriers.

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

## Prototype 1 History Audit

- Operational policy: audit the Prototype 1 History/Crown implementation at least once per week with combined human and LLM review.
- The audit must check that documentation and status claims do not overpromise what the current implementation proves, especially around tamper evidence, Crown authority, and compiler-enforced transition validity.
- The audit must inspect the actual type barriers: private fields, sealed or module-private state markers, constructor visibility, move-only transition methods, and the durable records emitted by those transitions.
- Treat any drift between claimed invariants and implemented constraints as a correctness issue. Fix the implementation, narrow the claim, or record the gap explicitly before relying on the History model for longer runs.
