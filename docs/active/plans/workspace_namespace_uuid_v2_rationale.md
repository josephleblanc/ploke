# Rationale: Error Sources and Mitigations (v2 Namespacing)

## Purpose
This document explains why the v2 plan makes specific choices, highlights potential error sources, and describes how each risk is mitigated. It also calls out additional pitfalls that could still cause unnecessary re-ingest or collisions.

## Core Tension: Sensitivity vs. Collisions
- **Too sensitive**: IDs change on repo moves, workspace layout changes, or machine differences, forcing full re-ingest.
- **Too coarse**: Different crates collapse into the same namespace and corrupt graph relationships.

The v2 plan solves this by:
- Scoping identity to **workspace + crate**, not just crate name.
- Making workspace identity **repo-relative** when possible.
- Using **Cargo metadata source identity** to avoid crate collisions.

## Primary Error Sources and How They Are Addressed

### 1) Absolute paths in namespaces
**Risk**: Moving a workspace changes all IDs.  
**Mitigation**:
- Workspace identity uses **repo-relative path**.
- Crate identity uses **workspace-relative manifest path**.
- Absolute paths are only used as a fallback, with warnings.

### 2) Missing crate version in namespace
**Risk**: Two versions of the same crate collide.  
**Mitigation**:
- `version` is required in `CrateIdentity`.
- Namespace versioning (`NAMESPACE_IDENTITY_VERSION`) is explicit.

### 3) Source ambiguity (registry vs git vs path)
**Risk**: Same name+version from different sources collides.  
**Mitigation**:
- Use `cargo metadata` `package_id` and `source`.
- Include both `source` and `source_id` in fingerprint.

### 4) Workspace context dropped in merges
**Risk**: `ParsedCodeGraph::merge_new` preserves only one crate context; multi-crate namespaces get lost.  
**Mitigation**:
- New multi-crate container or explicit `Vec<CrateContext>`.
- Merge logic must preserve *all* crate contexts.

### 5) Non-deterministic string concatenation
**Risk**: Changes in formatting or ordering change namespace inputs.  
**Mitigation**:
- Fixed key-value ordering in fingerprint strings.
- Explicit delimiters and version tags.

### 6) Git repo detection edge cases
**Risk**: `.git` as a file (submodules) not detected; workspace identity becomes path-sensitive.  
**Mitigation**:
- Use robust git root detection (handle `.git` file).
- If missing, warn and optionally allow override.

## Additional Potential Errors (and Corrections)

### A) Workspace membership changes causing namespace churn
**Risk**: If workspace identity depends on members, adding/removing a member invalidates all IDs.  
**Correction**:
- Workspace identity is based only on workspace location (repo-relative), not members.

### B) Path dependencies outside the workspace
**Risk**: These often resolve to absolute paths that are machine-specific.  
**Correction**:
- Treat as path-sensitive and warn.
- Optional future: allow user-defined stable anchors or overrides.

### C) Windows path normalization
**Risk**: Backslashes, drive letters, and case sensitivity can diverge.  
**Correction**:
- Normalize to forward slashes.
- Lowercase drive letter when present.

### D) Cargo metadata unavailability
**Risk**: Without metadata, dependency sources become ambiguous.  
**Correction**:
- Fallback to TOML parsing only for workspace members.
- Emit warning for dependencies; mark source as `unknown`.

### E) Namespace version migrations
**Risk**: Silent changes in inputs can cause collisions or accidental re-ingest.  
**Correction**:
- Store `namespace_version` in DB and CrateContext.
- Require explicit version bump for identity changes.

### F) DB query assumptions about single crate
**Risk**: Queries might return mixed results across crates.  
**Correction**:
- Add `workspace_id` filters where queries assume single-crate data.
- Extend `crate_context` schema to carry identity fields.

## Summary
The v2 plan emphasizes **stable, repo-relative identity**, **source-aware crate fingerprints**, and **explicit versioning**. It avoids accidental re-ingest on simple edits while preventing collisions across crates and dependency sources. The remaining risks are explicitly logged and can be mitigated further with optional overrides for path dependencies or non-git workspaces.

