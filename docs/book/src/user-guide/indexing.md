# Indexing a Rust Workspace

Indexing is the bridge between a local Rust project and Ploke's code-aware chat workflow.

At a high level, indexing does four things:

1. discovers the target crate or workspace,
2. parses Rust source into a typed code graph,
3. stores graph rows and relations in Cozo,
4. computes retrieval indexes for sparse and dense code search.

## Start indexing

From inside the TUI:

```text
/index start
```

To target a specific path:

```text
/index start path/to/crate-or-workspace
```

## Pause, resume, or cancel

```text
/index pause
/index resume
/index cancel
```

## Save and load an indexed graph

```text
/save db
/load <crate-or-workspace-name>
```

Saved graphs are useful when returning to the same project and avoiding a full re-index.
