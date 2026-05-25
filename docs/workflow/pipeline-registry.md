# Pipeline Registry

Status: active index for functions that participate in important project
pipelines.

The registry lives in [`pipeline-registry.jsonl`](pipeline-registry.jsonl).
Use it before editing functions that carry cross-module workflow invariants.

## Purpose

The registry maps pipeline ids to:

- key source functions;
- the stage each function participates in;
- authority docs that explain intended behavior;
- focused tests and known bug reports.

This is a lookup surface for agents and future hooks. It is not a complete
call graph.

## Command

Use `xtask` instead of rediscovering the map by hand:

```bash
cargo xtask pipeline list
cargo xtask pipeline show prototype1.edit_tool_gated_refresh
cargo xtask pipeline find --path crates/ploke-tui/src/rag/tools.rs
cargo xtask pipeline find should_wait_for_settled_edit
cargo xtask pipeline check
```

The command reads `docs/workflow/pipeline-registry.jsonl` by default.

## JSONL Shape

Each line is one JSON object.

Pipeline records:

```json
{"kind":"pipeline","id":"prototype1.edit_tool_gated_refresh","status":"active","docs":["docs/..."]}
```

Function records:

```json
{"kind":"function","pipeline":"prototype1.edit_tool_gated_refresh","stage":"tool_loop_gate","path":"crates/ploke-tui/src/llm/manager/session.rs","symbol":"should_wait_for_settled_edit","role":"...","docs":["docs/..."],"tests":["..."]}
```

## Maintenance Rules

- Add a function when changing it safely requires knowing a larger pipeline.
- Link docs that are authoritative for behavior, not incidental mentions.
- Keep `role` short and behavioral.
- Run `cargo xtask pipeline check` after edits.
- If a pipeline has key functions but weak docs, prioritize doc hygiene there.

## Codex Hooks Note

Codex lifecycle hooks can later call `cargo xtask pipeline find --path <file>`
or inspect prompt/tool inputs and add model-visible context. Keep hooks as a
consumer of this registry, not the registry itself.
