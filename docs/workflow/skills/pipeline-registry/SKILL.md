---
name: pipeline-registry
description: Use when editing, reviewing, documenting, or diagnosing source functions that may participate in a project workflow pipeline, or when adding pipeline/function records so agents can find authority docs, tests, bugs, and invariants before changing behavior.
---

# Pipeline Registry

Use this skill before editing a function that may belong to a larger project
pipeline.

## Workflow

1. Query the registry:

   ```bash
   cargo xtask pipeline find --path <source-file>
   cargo xtask pipeline find <symbol-or-keyword>
   ```

2. If a record matches, read the linked docs before editing behavior.
3. Run the listed focused tests when your change touches that pipeline stage.
4. If the function is pipeline-critical but missing from the registry, add a
   `function` record to `docs/workflow/pipeline-registry.jsonl`.
5. Run:

   ```bash
   cargo xtask pipeline check
   ```

## Registry Files

- `docs/workflow/pipeline-registry.md`
- `docs/workflow/pipeline-registry.jsonl`

The registry is an index, not a call graph. Add key functions where changing
local behavior safely requires understanding cross-module invariants.
