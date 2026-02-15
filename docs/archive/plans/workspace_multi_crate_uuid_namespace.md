# Plan: Workspace + Dependency-Safe UUID Namespacing

## Context
We currently generate UUIDv5 IDs for nodes and types in `ploke-core`, and `syn_parser` drives the crate namespace used as a key input to those IDs. This works for a single crate, but we want to ingest **multiple crates in a workspace** (and later dependencies) into the same graph without namespace collisions. The high-level architecture is described in `docs/plans/uuid_refactor/00_overview_batch_processing_model.md`.

## Goals
- Deterministic, unique namespaces for **every crate instance** in a workspace (and dependencies).
- IDs stay stable for a crate even when multiple crates are parsed together.
- Make namespace derivation explicit, testable, and versioned (so we can evolve without silent collisions).
- Maintain compatibility with existing `NodeId`, `TypeId`, `CanonId`, and `PubPathId` generation.

## Non-Goals
- Full cross-crate name resolution in this plan (only namespace correctness).
- Database migration strategy beyond minimal schema updates (note risks, but defer deep migration design).

## Current State (Key Files)
- Namespace constant and ID generation:
  - `crates/ploke-core/src/lib.rs`
- Workspace and crate identity helpers:
  - `crates/ploke-core/src/workspace.rs`
- Crate discovery and namespace derivation:
  - `crates/ingest/syn_parser/src/discovery/single_crate.rs`
  - `crates/ingest/syn_parser/src/discovery/workspace.rs`
- Parser usage of crate namespace:
  - `crates/ingest/syn_parser/src/parser/visitor/state.rs`
  - `crates/ingest/syn_parser/src/parser/visitor/mod.rs`
  - `crates/ingest/syn_parser/src/parser/graph/parsed_graph.rs`
  - `crates/ingest/syn_parser/src/resolve/id_resolver.rs`
- DB transform and schema touchpoints:
  - `crates/ingest/ploke-transform/src/transform/crate_context.rs`
  - `crates/ingest/ploke-transform/src/schema/crate_node.rs`
  - `crates/ploke-db/src/database.rs`

## Design Decisions Needed (Callouts)
1. **Crate identity inputs** for namespace derivation:
   - Must disambiguate same `name` across:
     - multiple versions
     - different sources (registry/git/path)
     - multiple workspace members with same name (path-dependent)
2. **Workspace identity** for grouping:
   - Decide whether we need a `WorkspaceId` distinct from crate namespace for filtering and queries.
3. **Determinism across machines**:
   - Path-based identifiers are stable locally but not portable. Decide if we accept this for workspace crates or normalize with workspace-relative paths.

## Proposed Namespace Model (Draft)
### 1) Root namespace (constant)
- Keep `PROJECT_NAMESPACE_UUID` in `ploke-core`.

### 2) Workspace namespace (new)
- Define `WORKSPACE_NAMESPACE = v5(PROJECT_NAMESPACE_UUID, workspace_fingerprint)`.
- `workspace_fingerprint` could be:
  - canonical workspace root path **relative to repo root** (if available), or
  - hash of workspace manifest path + workspace package metadata.

### 3) Crate namespace (new, explicit)
- Derive `CRATE_NAMESPACE = v5(WORKSPACE_NAMESPACE, crate_fingerprint)`.
- `crate_fingerprint` should include:
  - `name`
  - `version`
  - `source` (registry/git/path)
  - `manifest_path` (normalized; workspace-relative for path deps)
- This allows:
  - same crate name in different workspaces
  - multiple versions of same crate
  - both workspace members and external dependencies

### 4) Logical crate identity (optional, for embeddings)
- If we want a stable ID across workspace instances, define:
  - `LOGICAL_CRATE_ID = v5(PROJECT_NAMESPACE_UUID, name + version + source)`
- Use this only where cross-workspace stability is desired (e.g., embeddings or external caching).

## Implementation Plan
### Phase A: Align crate identity + namespace derivation
1. **Introduce identity structs in `ploke-core`**
   - Add `WorkspaceId` + `CrateIdentity` (name, version, source, manifest_path).
   - Centralize namespace derivation in `ploke-core` (new helper module).
   - Files: `crates/ploke-core/src/workspace.rs`, `crates/ploke-core/src/lib.rs`.

2. **Update `syn_parser` discovery to produce crate identity**
   - Extend `CrateContext` to include identity fields (source + manifest_path).
   - Replace `derive_crate_namespace(name, version)` with a call that uses `CrateIdentity`.
   - Files: `crates/ingest/syn_parser/src/discovery/single_crate.rs`,
     `crates/ingest/syn_parser/src/discovery/workspace.rs`.

3. **Decide source normalization strategy**
   - For workspace members: use workspace-relative `manifest_path`.
   - For registry dependencies: use Cargo metadata source string (e.g., `registry+...`).
   - For git/path deps: normalize to stable string where possible; otherwise accept local path.

### Phase B: Multi-crate graph support
4. **Represent multiple crate contexts in parsed graphs**
   - Decide between:
     - `ParsedWorkspaceGraph` (new wrapper around `Vec<ParsedCodeGraph>`), or
     - extending `ParsedCodeGraph` to hold a map of crate contexts.
   - Ensure `merge_new` and downstream usage avoid silently dropping contexts.
   - Files: `crates/ingest/syn_parser/src/parser/graph/parsed_graph.rs`,
     `crates/ingest/syn_parser/src/parser/visitor/mod.rs`.

5. **Ensure `crate_namespace` threads through parsing**
   - Update construction of `VisitorState` to be explicit per-crate.
   - Verify `CanonIdResolver` and type resolution use the crate namespace tied to each graph.
   - Files: `crates/ingest/syn_parser/src/parser/visitor/state.rs`,
     `crates/ingest/syn_parser/src/resolve/id_resolver.rs`.

### Phase C: Transform + DB affordances
6. **Crate context schema update**
   - Add fields to store workspace/crate identity (manifest path, source, logical crate id).
   - Update transform to write these fields.
   - Files: `crates/ingest/ploke-transform/src/schema/crate_node.rs`,
     `crates/ingest/ploke-transform/src/transform/crate_context.rs`.

7. **Query updates for multi-crate**
   - Ensure `ploke-db` queries filter by namespace/workspace where needed.
   - Validate joins that assume single crate (e.g., `file_mod` + `crate_context`).
   - Files: `crates/ploke-db/src/database.rs`, `crates/ploke-db/src/helpers.rs`.

### Phase D: Tests + fixtures
8. **Namespace derivation tests**
   - New tests for:
     - same name, different version -> different namespace
     - same name/version, different source -> different namespace
     - stable namespace for same crate identity
   - Files: `crates/ingest/syn_parser/src/discovery/single_crate.rs` tests or new test module.

9. **Multi-crate integration fixtures**
   - Add a workspace fixture with:
     - two workspace crates
     - a dependency with same name but different source/version
   - Validate non-colliding namespaces and deterministic IDs.
   - Files: `crates/ingest/syn_parser/tests/*`, `crates/ploke-db/tests/*`.

## Open Questions
- Do we want **path-based stability** for workspace crates across machines?
  - If yes, we should hash workspace-relative paths (not absolute).
- Should `PROJECT_NAMESPACE_UUID` change when we change namespace inputs?
  - If so, treat this as a "namespace version bump" and plan DB migration.
- Do we need both `CrateId` and `CrateNamespace`, or can we collapse them into one typed ID?

## Acceptance Criteria
- Parsing multiple crates in a workspace yields **unique namespaces** for each crate.
- UUIDs remain deterministic across runs for the same workspace/identity inputs.
- No accidental collisions when same crate name appears via multiple sources.
