# ADR 005: Crate Identity and Coarse Dependency Invalidation

## Status
Proposed (2025-12-27)

## Context
- We need a single, stable notion of crate identity for indexing, focus, and querying.
- Path policy for IO must be derived from focus to avoid multiple sources of truth.
- The graph can become stale when a dependency changes; we need coarse invalidation now, with a path toward finer-grain dependency tracking later.
- Multi-agent and concurrent reads can surface stale results unless staleness is visible at query time.

## Decision
- Introduce a stable CrateId and store crate metadata as CrateInfo { id, namespace, root_path }.
- Track workspace crates in WorkspaceRoots, and track focus as CrateFocus { crate_id }.
- Derive PathPolicy from CrateFocus plus explicit extra_read_roots. Do not store PathPolicy independently.
- Track CrateVersionMap (monotonic per CrateId), Deps (CrateId -> Vec<CrateId>), and InvalidationSet (stale crates).
- On reindex of crate X:
  - bump CrateVersionMap[X]
  - mark dependents of X as stale in InvalidationSet
- On graph query for crate Y:
  - if Y is stale, surface staleness (warn/deny or reindex based on correctness policy).
- Keep dependency tracking modular so it can later be refined to edge-level without changing focus/policy structures.

## Consequences
### Positive
- Single source of truth for crate identity and focus; paths are metadata, not identity.
- Coarse invalidation prevents silently serving stale graph results.
- Path policy remains derived, avoiding divergence between IO scope and focused crate.
- Future refinement to item-level dependency tracking is possible without reshaping core state.

### Negative
- Additional bookkeeping (versions, deps, invalidation set) in core state.
- Requires wiring staleness visibility into query surfaces and UI/logs.

### Neutral
- Coarse invalidation may be conservative (marking more stale than necessary) until fine-grain edges are implemented.

## Notes
- Implementation plan is tracked in docs/active/todo/2025-12-27-crate-invalidation-state.md.
