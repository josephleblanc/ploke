# Plan: Stable Multi-Crate Namespacing (v2)

## Summary
Define a stable, collision-resistant namespace strategy that allows incremental updates without forcing full re-ingestion. This plan fixes the weaknesses in the prior draft by: (1) making workspace identity stable across moves, (2) using Cargo metadata for source identity, (3) versioning the namespace algorithm, and (4) preserving multi-crate context throughout parse/transform/DB layers.

## Requirements (Invariants)
- **Deterministic IDs** for the same workspace + crate identity inputs.
- **No global re-ingest on ordinary edits** (file edits must not change namespace).
- **No collisions** across crates with same name but different version/source/path.
- **Stable across machines** when cloning the same repo (no absolute path dependence).
- **Namespace algorithm is versioned** and can be upgraded with explicit migration.

## Corrections to Previous Plan
1. **Workspace fingerprint** must not rely on absolute paths (too sensitive).
2. **Crate identity** must include version and source (previous code ignores version).
3. **Cargo metadata** is required for reliable source identity.
4. **Multi-crate context** must not be dropped during `ParsedCodeGraph::merge_new`.
5. **Namespace versioning** must be explicit (not just implied).

## Identity Model

### WorkspaceIdentity
Use a stable, repo-relative identifier when possible.

**Rule**:
1. Locate workspace root (Cargo.toml with `[workspace]`).
2. If inside a git repo, compute `workspace_rel_path = workspace_root relative to repo root`.
3. `workspace_id = v5(PROJECT_NAMESPACE_UUID, "ws|v1|repo:" + workspace_rel_path)`.
4. If no git root found:
   - Fallback to canonical path **and** emit a warning that IDs are path-sensitive.
   - Optional: allow an override via config file (future extension).

This keeps IDs stable across clones/moves when the workspace is in a repo.

### CrateIdentity
Use Cargo metadata for deterministic, unique crate identity.

**Inputs** (normalized strings):
- `name`
- `version`
- `source` (registry/git/path/unknown)
- `source_id` (from Cargo metadata package id)
- `manifest_path_rel` (workspace-relative path to Cargo.toml)

**Fingerprint**:
```
crate_fingerprint =
  "crate|v2|" +
  "name:" + name +
  "|ver:" + version +
  "|src:" + source +
  "|srcid:" + source_id +
  "|manifest:" + manifest_path_rel
```

### CrateNamespace
```
crate_namespace = v5(PROJECT_NAMESPACE_UUID, "ns|v2|ws:" + workspace_id + "|" + crate_fingerprint)
```

### Namespace Version
Introduce `NAMESPACE_IDENTITY_VERSION = "ns|v2"` in `ploke-core`.
This allows intentional namespace changes and explicit migrations.

## Normalization Rules (Critical)
- Use forward slashes for all paths.
- Normalize `manifest_path_rel` to be workspace-relative.
- For path dependencies outside workspace root:
  - Use canonical path only as a last resort and log a warning.
- For git dependencies:
  - Prefer `source` from Cargo metadata (includes URL + rev).
- For registry dependencies:
  - Use `source` from Cargo metadata (registry URL included).

## Implementation Steps

### Phase A: Core identity helpers
1. **Add new types to `ploke-core`**:
   - `WorkspaceIdentity { workspace_id, workspace_root_rel, is_repo_scoped }`
   - `CrateIdentity { name, version, source, source_id, manifest_path_rel }`
   - `CrateNamespace::from_identity(WorkspaceIdentity, CrateIdentity)`
2. **Add `NAMESPACE_IDENTITY_VERSION` constant** and ensure it is part of the namespace input string.
3. **Update `CrateId` / `CrateInfo`** to use new identity (not root path hashing).

Files:
- `crates/ploke-core/src/workspace.rs`
- `crates/ploke-core/src/lib.rs`

### Phase B: Discovery and metadata
4. **Use Cargo metadata for identity inputs**:
   - Pull `package_id`, `source`, `manifest_path` from metadata.
   - Fall back to `Cargo.toml` parsing only if metadata is unavailable.
5. **Extend `CrateContext`**:
   - Add: `workspace_id`, `manifest_path_rel`, `source`, `source_id`, `namespace_version`.
6. **Replace `derive_crate_namespace(name, version)`**:
   - Use `CrateIdentity` + `WorkspaceIdentity` to derive namespace.

Files:
- `crates/ingest/syn_parser/src/discovery/single_crate.rs`
- `crates/ingest/syn_parser/src/discovery/workspace.rs`

### Phase C: Multi-crate graph preservation
7. **Preserve all crate contexts**:
   - Create `ParsedWorkspaceGraph` (new) or add `Vec<CrateContext>` to `ParsedCodeGraph`.
   - Update `ParsedCodeGraph::merge_new` to avoid discarding crate contexts.
8. **Ensure namespace is threaded per crate**:
   - `VisitorState` and `CanonIdResolver` must be tied to the correct crate namespace.

Files:
- `crates/ingest/syn_parser/src/parser/graph/parsed_graph.rs`
- `crates/ingest/syn_parser/src/parser/visitor/state.rs`
- `crates/ingest/syn_parser/src/resolve/id_resolver.rs`

### Phase D: Transform + DB updates
9. **Update schema to store identity fields**:
   - Extend `crate_context` relation with workspace_id, manifest_path_rel, source, source_id, namespace_version.
10. **Update transform**:
   - Write new fields from `CrateContext`.
11. **DB query safeguards**:
   - Add workspace_id filters where queries assume single-crate data.

Files:
- `crates/ingest/ploke-transform/src/schema/crate_node.rs`
- `crates/ingest/ploke-transform/src/transform/crate_context.rs`
- `crates/ploke-db/src/database.rs`
- `crates/ploke-db/src/helpers.rs`

### Phase E: Tests
12. **Identity tests**:
   - Same crate name/version in different sources => different namespace.
   - Same workspace + crate => stable namespace across runs.
   - Workspace move inside git repo => same workspace_id.
13. **Fixture tests**:
   - Multi-crate workspace fixture with path + registry deps.

Files:
- `crates/ingest/syn_parser/tests/*`
- `crates/ploke-db/tests/*`

## Migration & Compatibility
- Gate all changes under a feature flag (e.g., `uuid_ids` or new `namespace_v2`).
- Store `namespace_version` in DB so old data can coexist.
- Require explicit re-ingest only when `NAMESPACE_IDENTITY_VERSION` changes.

## Risks and Mitigations
- **Missing cargo metadata**: fallback to TOML parsing, log warning.
- **Path deps outside workspace**: log warning, accept path sensitivity.
- **Workspace not in git**: warn about path sensitivity, allow override later.
- **Performance**: cache metadata results across runs.

