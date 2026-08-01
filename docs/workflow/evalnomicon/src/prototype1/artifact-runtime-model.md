# Artifact and Runtime Model

Prototype 1 distinguishes artifact state, runtime state, tree structure, and lineage authority.

## Core terms

- **Artifact:** a checkout state: the collection of files that can hydrate a runtime. A worktree path is a handle to an artifact, not the artifact's semantic identity.
- **Runtime:** an executing process hydrated from an artifact. A runtime may operate over an artifact surface to produce a patch attempt.
- **Tree:** the artifact substrate: durable artifact states connected by applied patches.
- **Lineage:** a History authority coordinate over admitted artifact continuity. It is not identical to a git branch, worktree path, process id, or artifact id.
- **Patch attempt:** a proposed or realized mutation from a base artifact toward a derived artifact.

## Three graph views

Current code comments describe the future model as three related graphs rather than one git tree:

```text
artifact graph            durable Artifact states connected by applied patches
runtime derivation graph  which Artifact hydrated each Runtime
operation graph           which Runtime operated over which Artifact to create a patch attempt
```

The important operation coordinate is:

```text
OperationCoordinate = (generator Runtime, target Artifact)
```

Git ancestry can describe that one artifact descends from another, but it cannot by itself describe which runtime generated the patch attempt. Prototype 1 records therefore need generator runtime, target artifact, selected artifact, selected runtime, candidate scope, oracle identity, evaluation policy, and attempt identity when available.

## Identity cautions

- Branch names and worktree paths are handles, not semantic identity.
- Dirty worktrees are provisional candidates until they have a recoverable identity such as a git commit, git tree id, content hash, or artifact manifest id.
- Runtime provenance may be degraded, but it should not be erased. If a runtime exists but its source artifact is missing, records should say so directly instead of implying a recoverable source node.

## Canonical sources

- `crates/ploke-eval/src/cli/prototype1_state/mod.rs`
- Draft source: `docs/workflow/evalnomicon/drafts/runtime/artifact-runtime-lineage.md` is historical/background only unless refreshed against current code.
