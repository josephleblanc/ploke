# SetupPhase Implementation Plan

**Status:** In Progress - Type Refactoring Complete  
**Created:** 2026-04-11  
**Updated:** 2026-04-12  
**Related:** [AUDIT_SYNTHESIS.md](./phase-1-audit/AUDIT_SYNTHESIS.md), [eval-design.md](../plans/evals/eval-design.md)

---

## Recent Changes (2026-04-12)

### Completed: CrateContext Enhancement

**Changes Made:**
1. **Added `id: CrateId` to `CrateContext`** (`syn_parser/src/discovery/single_crate.rs`)
   - `CrateContext` now has both:
     - `id: CrateId` (path-based UUID for AppState maps)
     - `namespace: Uuid` (name-based UUID for DB)
   - This subsumes `CrateInfo` - `CrateContext` is now the canonical crate representation

2. **Updated `LoadedCrateState`** (`ploke-tui/src/app_state/core.rs`)
   - Changed from `info: CrateInfo` to `context: CrateContext`
   - Updated all usages (`.info.` → `.context.`)

3. **Kept `CrateInfo` for `WorkspaceRoots`**
   - `ploke-core` can't depend on `syn_parser`
   - `CrateInfo` remains for workspace-level tracking
   - `CrateContext` is used for loaded crate state

**Test Results:** All workspace tests pass (verified via `cargo test --workspace`)

---

## Executive Summary

---

## Executive Summary

The `SetupPhase` struct in `RunRecord` is **never populated** (always `null` in output). This blocks A2 (parsing fidelity) validation and A5 (replay/introspection) because we cannot determine:
- Which crates were indexed
- Whether parsing succeeded or failed
- Node counts for coverage analysis

This plan details how to populate `SetupPhase` with indexed crate information, enabling post-run introspection.

---

## Current State Analysis

### The Problem

```rust
// crates/ploke-eval/src/record.rs:470
pub struct RunPhases {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub setup: Option<SetupPhase>,  // ← Always None today
    ...
}

// Verified from actual run artifact:
// $ zcat record.json.gz | jq '.phases.setup'
// null
```

### Existing Artifacts (Data Exists, Not Aggregated)

| Artifact | File Location | Contents | Serializable? |
|----------|---------------|----------|---------------|
| `RepoStateArtifact` | `repo-state.json` | git SHA, paths | ✅ Yes |
| `IndexingStatusArtifact` | `indexing-status.json` | status message | ✅ Yes |
| `ParseFailureArtifact` | `parse-failure.json` | errors, diagnostics | ✅ Yes |
| `CrateContext` | In memory only | name, version, namespace, files | ✅ Yes |
| Node counts | DB only | per-crate node counts | ❌ Needs query |

### Existing Types with Serialize/Deserialize

```rust
// crates/ingest/syn_parser/src/discovery/single_crate.rs:570-601
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CrateContext {
    pub name: String,
    pub version: String,
    pub namespace: Uuid,
    pub root_path: PathBuf,
    pub files: Vec<PathBuf>,
    pub targets: Vec<TargetSpec>,
    pub features: Features,
    pub dependencies: Dependencies,
    pub dev_dependencies: DevDependencies,
    #[serde(skip)]
    pub workspace_path: Option<PathBuf>,
}
```

### Types Missing Serialize/Deserialize (Easy to Add)

```rust
// crates/ploke-core/src/workspace.rs:66-72
#[derive(Debug, Clone, PartialEq, Eq)]  // Add: Serialize, Deserialize
pub struct CrateInfo { ... }

// crates/ploke-db/src/database.rs:87-93  
#[derive(Debug, Clone, PartialEq, Eq)]  // Add: Serialize, Deserialize
pub struct CrateContextRow { ... }
```

---

## Design Goals

1. **Capture indexed crate list** — What crates exist in the DB after indexing
2. **Capture node counts** — How many nodes per crate (total + embedded)
3. **Capture parse status** — Success/failure per crate with diagnostics
4. **Enable introspection API** — Simple methods for common questions
5. **Minimal ingestion changes** — Query DB post-indexing, don't instrument indexing

---

## Proposed Types

### IndexedCrateSummary

New type for `SetupPhase.indexed_crates`:

```rust
// crates/ploke-eval/src/record.rs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexedCrateSummary {
    // Identity (from CrateContext/CrateContextRow)
    pub name: String,
    pub version: String,
    pub namespace: Uuid,
    pub root_path: PathBuf,
    
    // File inventory (from CrateContext.files)
    pub file_count: usize,
    pub source_files: Vec<PathBuf>,  // Or skip if too large?
    
    // Node statistics (from DB queries post-indexing)
    pub node_count: usize,           // Total nodes in crate namespace
    pub embedded_count: usize,       // Nodes with embeddings
    
    // Status (from parse failure tracking)
    pub status: CrateIndexStatus,
    pub parse_error: Option<ParseErrorSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CrateIndexStatus {
    Success,
    Partial,      // Some files parsed, others failed
    Failed,
    Skipped,      // Cached DB used
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParseErrorSummary {
    pub message: String,
    pub target_dir: PathBuf,
    pub occurred_at_ms: i64,
    // Note: Full diagnostics in ParseFailureArtifact separately
}
```

### Enhanced SetupPhase

```rust
// crates/ploke-eval/src/record.rs:487-506 (existing, needs population)
pub struct SetupPhase {
    pub started_at: String,              // ISO 8601
    pub ended_at: String,
    pub repo_state: RepoStateArtifact,   // Already exists
    pub indexing_status: IndexingStatusArtifact,  // Already exists
    
    // NEW FIELD:
    pub indexed_crates: Vec<IndexedCrateSummary>,
    
    // RENAME: parse_failure → parse_failures (plural, Vec)
    pub parse_failures: Vec<ParseFailureRecord>,
    
    pub db_timestamp_micros: i64,
    
    // NEW FIELD: version tracking per eval-design.md §VIII
    pub tool_schema_version: String,
}
```

---

## Data Flow

### Post-Indexing Capture (runner.rs)

After indexing completes (~line 1327), before `run_benchmark_turn()`:

```rust
// 1. Get crate contexts from DB
let crate_rows = state.db.list_crate_context_rows()?;

// 2. For each crate, query node counts
let indexed_crates: Vec<IndexedCrateSummary> = crate_rows
    .into_iter()
    .map(|row| {
        let node_count = count_nodes_for_namespace(&state.db, row.namespace)?;
        let embedded_count = count_embedded_for_namespace(&state.db, row.namespace)?;
        let status = determine_status(&row, &parse_failures);
        
        IndexedCrateSummary {
            name: row.name,
            version: /* from CrateContext or query */,
            namespace: row.namespace,
            root_path: PathBuf::from(row.root_path),
            file_count: /* from CrateContext or count */,
            node_count,
            embedded_count,
            status,
            parse_error: get_parse_error(&row, &parse_failures),
        }
    })
    .collect();

// 3. Populate SetupPhase
let setup_phase = SetupPhase {
    started_at: setup_start_time.to_rfc3339(),
    ended_at: chrono::Utc::now().to_rfc3339(),
    repo_state,           // Already captured
    indexing_status,      // Already captured
    indexed_crates,       // NEW
    parse_failures: collect_parse_failures(&state).await,  // NEW
    db_timestamp_micros: state.db.current_validity_micros()?,
    tool_schema_version: manifest.tool_schema_version.clone(),  // NEW
};

run_record.phases.setup = Some(setup_phase);
```

### DB Queries Needed

```rust
// In ploke-db/src/database.rs (new methods)

/// Count all nodes for a crate namespace
pub fn count_nodes_for_namespace(&self, namespace: Uuid) -> Result<usize, DbError> {
    let script = format!(
        r#"?[count] := agg(count = node_count(), node_count() = count()),
           *file_mod{{ namespace, owner_id: mod_id }},
           *nodes{{ id, mod_id @ 'NOW' }},
           namespace = uuid("{}")"#,
        namespace
    );
    // ... query and parse result
}

/// Count embedded nodes for a crate namespace  
pub fn count_embedded_for_namespace(&self, namespace: Uuid) -> Result<usize, DbError> {
    // Join nodes with embedding table
    // ... similar pattern
}
```

---

## SetupPhase Introspection API

Methods on `SetupPhase` for common questions:

```rust
impl SetupPhase {
    /// "How many crates were indexed?"
    pub fn crate_count(&self) -> usize {
        self.indexed_crates.len()
    }
    
    /// "How many total nodes across all crates?"
    pub fn total_node_count(&self) -> usize {
        self.indexed_crates.iter().map(|c| c.node_count).sum()
    }
    
    /// "How many nodes have embeddings?"
    pub fn total_embedded_count(&self) -> usize {
        self.indexed_crates.iter().map(|c| c.embedded_count).sum()
    }
    
    /// "Which crates failed to parse?"
    pub fn failed_crates(&self) -> Vec<&IndexedCrateSummary> {
        self.indexed_crates.iter()
            .filter(|c| matches!(c.status, CrateIndexStatus::Failed))
            .collect()
    }
    
    /// "What's the parse coverage percentage?"
    pub fn coverage_percentage(&self) -> f64 {
        let total = self.indexed_crates.len();
        if total == 0 { return 0.0; }
        let successful = self.indexed_crates.iter()
            .filter(|c| matches!(c.status, CrateIndexStatus::Success))
            .count();
        (successful as f64 / total as f64) * 100.0
    }
    
    /// "Does this crate exist in the index?"
    pub fn has_crate(&self, name: &str) -> bool {
        self.indexed_crates.iter().any(|c| c.name == name)
    }
    
    /// "Get info for a specific crate"
    pub fn crate_info(&self, name: &str) -> Option<&IndexedCrateSummary> {
        self.indexed_crates.iter().find(|c| c.name == name)
    }
    
    /// "How many nodes in a specific crate?"
    pub fn node_count_for_crate(&self, name: &str) -> Option<usize> {
        self.crate_info(name).map(|c| c.node_count)
    }
}
```

---

## Implementation Tasks

### Phase 1: Type Preparation (1-2 hours)

- [ ] Add `Serialize, Deserialize` to `CrateInfo` (ploke-core)
- [ ] Add `Serialize, Deserialize` to `CrateContextRow` (ploke-db)
- [ ] Create `IndexedCrateSummary` type (ploke-eval)
- [ ] Create `CrateIndexStatus` enum (ploke-eval)
- [ ] Add `indexed_crates` field to `SetupPhase` (ploke-eval)
- [ ] Change `parse_failure: Option<ParseFailureRecord>` to `parse_failures: Vec<ParseFailureRecord>`
- [ ] Add `tool_schema_version` field to `SetupPhase`

### Phase 2: DB Queries (2-3 hours)

- [ ] Implement `count_nodes_for_namespace()` in ploke-db
- [ ] Implement `count_embedded_for_namespace()` in ploke-db
- [ ] Add unit tests for new queries

### Phase 3: Runner Integration (2-3 hours)

- [ ] Capture `setup_start_time` at beginning of `run()`
- [ ] Query DB for crate list after indexing completes
- [ ] Query DB for node counts per crate
- [ ] Collect parse failures from state
- [ ] Build `SetupPhase` and assign to `run_record.phases.setup`
- [ ] Test with real run

### Phase 4: Validation (1 hour)

- [ ] Verify `record.json.gz` contains populated `setup`
- [ ] Verify `setup.indexed_crates` has expected data
- [ ] Test introspection API methods

---

## Open Questions

1. **Version field in IndexedCrateSummary**: Where do we get crate version?
   - Option A: Query from `CrateContext` stored in AppState (need to expose)
   - Option B: Add to DB `crate_context` table
   - Option C: Skip for now (not critical)

2. **Source files list**: Include `Vec<PathBuf>` or just count?
   - Including full list: Accurate but verbose (could be 100s of files)
   - Just count: Simpler but loses file-level detail
   - Recommendation: Start with count only

3. **Cached DB path**: How to handle `using_cached_starting_db`?
   - All crates show `CrateIndexStatus::Skipped`?
   - Or query the cached DB metadata?
   - Recommendation: Mark as `Skipped` with note in detail

4. **Multiple parse failures**: Currently only `last_parse_failure()` is stored
   - Need to collect all failures during indexing
   - May require changes to indexing event handling
   - Recommendation: Start with single failure, extend later

---

## Dependencies

### Files to Modify

| File | Changes |
|------|---------|
| `crates/ploke-core/src/workspace.rs` | Add derives to `CrateInfo` |
| `crates/ploke-db/src/database.rs` | Add derives to `CrateContextRow`, add count queries |
| `crates/ploke-eval/src/record.rs` | Add `IndexedCrateSummary`, update `SetupPhase` |
| `crates/ploke-eval/src/runner.rs` | Populate `SetupPhase` after indexing |

### External Dependencies

- `cargo_toml::Manifest` — may not be serializable, might need wrapper or skip
- Cozo DB queries — need to verify query syntax for node counting

---

## References

### Existing Types

- `CrateContext`: `crates/ingest/syn_parser/src/discovery/single_crate.rs:570`
- `CrateInfo`: `crates/ploke-core/src/workspace.rs:66`
- `CrateContextRow`: `crates/ploke-db/src/database.rs:87`
- `ParseFailureArtifact`: `crates/ploke-eval/src/runner.rs:159`
- `SetupPhase`: `crates/ploke-eval/src/record.rs:487`

### Key Functions

- `list_crate_context_rows()`: `crates/ploke-db/src/database.rs:2259`
- `persist_parse_failure_artifact()`: `crates/ploke-eval/src/runner.rs:211`
- `RunMsbSingleRequest::run()`: `crates/ploke-eval/src/runner.rs:849`
- `wait_for_indexing_completion()`: `crates/ploke-eval/src/runner.rs:1920`

### Documentation

- `eval-design.md` §VII (Introspection API)
- `eval-design.md` §VIII (Immutable Run Manifest)
- `AUDIT_SYNTHESIS.md` (Phase 1 Audit findings)

---

## Success Criteria

1. After an eval run, `record.json.gz` contains non-null `.phases.setup`
2. `.phases.setup.indexed_crates` lists all crates that were indexed
3. Each crate entry has: name, namespace, file_count, node_count, status
4. `SetupPhase::failed_crates()` returns crates that failed to parse
5. `SetupPhase::coverage_percentage()` reports % of crates successfully parsed

---

## Appendix: Example Output

```json
{
  "phases": {
    "setup": {
      "started_at": "2026-04-11T10:00:00Z",
      "ended_at": "2026-04-11T10:02:15Z",
      "repo_state": {
        "repo_root": "/tmp/ripgrep",
        "requested_base_sha": "abc123",
        "checked_out_head_sha": "abc123"
      },
      "indexing_status": {
        "status": "completed",
        "detail": "Indexing completed through the full app command path."
      },
      "indexed_crates": [
        {
          "name": "ripgrep",
          "version": "14.1.0",
          "namespace": "a1b2c3d4-...",
          "root_path": "/tmp/ripgrep",
          "file_count": 45,
          "node_count": 1523,
          "embedded_count": 1523,
          "status": "Success"
        },
        {
          "name": "globset",
          "version": "0.4.14",
          "namespace": "e5f6g7h8-...",
          "root_path": "/tmp/ripgrep/crates/globset",
          "file_count": 12,
          "node_count": 0,
          "embedded_count": 0,
          "status": "Failed",
          "parse_error": {
            "message": "Rust 2015 bare trait object syntax not supported",
            "target_dir": "/tmp/ripgrep/crates/globset",
            "occurred_at_ms": 1744369215000
          }
        }
      ],
      "parse_failures": [...],
      "db_timestamp_micros": 1744369215000000,
      "tool_schema_version": "v2.3.1"
    },
    "agent_turns": [...]
  }
}
```

---

## Next Steps (Post-Compaction)

With the `CrateContext` refactoring complete, we can now proceed with the remaining implementation:

### Phase 2: DB Queries (Ready to Start)

**Files to modify:**
- `crates/ploke-db/src/database.rs`

**Add methods:**
```rust
/// Count all nodes for a crate namespace
pub fn count_nodes_for_namespace(&self, namespace: Uuid) -> Result<usize, DbError>

/// Count embedded nodes for a crate namespace  
pub fn count_embedded_for_namespace(&self, namespace: Uuid) -> Result<usize, DbError>
```

### Phase 3: Runner Integration (Ready to Start)

**Files to modify:**
- `crates/ploke-eval/src/record.rs` - Add `IndexedCrateSummary` type
- `crates/ploke-eval/src/runner.rs` - Populate `SetupPhase` after indexing

**Key location:** After `wait_for_indexing_completion()` (~line 1327), before `run_benchmark_turn()`

### Simplified Approach

Since `CrateContext` now has all the data we need:
1. Get crate contexts from `AppState.loaded_crates`
2. Query DB for node counts per namespace
3. Build `SetupPhase` with enriched crate data

No need to query `crate_context` table - we already have the contexts in memory!
