# Handoff: Dual Syn Version Implementation + SetupPhase/Introspection

**Date:** 2026-04-11 (Updated 2026-04-12)  
**Workstream:** A2 Data Fidelity / Parser + Phase 1 Introspection  
**Gate:** H0 Interpretation (enables A2 validation)  
**Status:** SETUPPHASE COMPLETE, INTROSPECTION API PARTIAL - Needs fixes for DB queries  
**Branch:** Main  

---

## Part 1: Dual Syn Implementation (COMPLETE)

### Problem
`syn` 2.x hard-rejects Rust 2015 bare trait objects (e.g., `Arc<Fn(...)>`), blocking evaluation on ripgrep and other Rust 2015 crates.

### Solution Implemented
Dual syn version support with unified processing via conversion layer:
- **syn 1.x** for Rust 2015 edition crates (accepts bare trait objects)
- **syn 2.x** for Rust 2018+ edition crates (default behavior)
- **Conversion layer** syn1→syn2 enables code reuse

### Key Files
- `utils.rs` - Type/attribute conversion functions
- `type_processing_syn1.rs` - Thin adapter (21 lines)
- `attribute_processing_syn1.rs` - Thin adapter (75 lines)

**Commits:**
- `83b97568` - wip: dual syn version support
- `f5ee41ed` - wip: refactor attribute_processing_syn1 to convert and delegate

---

## Part 2: SetupPhase & Introspection Implementation (COMPLETE/PARTIAL)

### What Was Implemented

Following the plan in `/home/brasides/.kimi/plans/iceman-white-tiger-wolverine.md`:

| Task | Status | Implementation |
|------|--------|----------------|
| Task 1: DB Historical Queries | ✅ | `raw_query_at_timestamp()` in `ploke-db/src/database.rs` |
| Task 2: SetupPhase Types | ✅ | `IndexedCrateSummary`, `CrateIndexStatus`, enhanced `SetupPhase` in `ploke-eval/src/record.rs` |
| Task 3: Node Count Queries | ✅ | `count_nodes_for_namespace()`, `count_embedded_for_namespace()` in `ploke-db/src/database.rs` |
| Task 4: Populate SetupPhase | ✅ | `build_setup_phase()` in `ploke-eval/src/runner.rs` - called after indexing |
| Task 5: Introspection API | ⚠️ PARTIAL | `DbState`, `lookup()`, `replay_query()` implemented but query syntax needs fixes |
| Task 6: Integration Tests | ✅ | 6 tests in `ploke-eval/tests/setup_phase_integration.rs` all passing |

### Verified Working

**SetupPhase Population:**
```bash
$ zcat ~/.ploke-eval/runs/BurntSushi__ripgrep-2209/record.json.gz | jq '.phases.setup'
{
  "started_at": "2026-04-12T03:04:17.656219146+00:00",
  "ended_at": "2026-04-12T03:04:18.706270546+00:00",
  "repo_state": { ... },
  "indexing_status": { ... },
  "indexed_crates": [
    {
      "name": "globset",
      "namespace": "8c80172b-43df-52d7-ba55-975c370989c0",
      "root_path": "/home/brasides/.ploke-eval/repos/BurntSushi/ripgrep/crates/globset",
      "node_count": 103,
      "embedded_count": 67,
      "status": "Skipped"
    },
    ... 9 crates total
  ],
  "parse_failures": [],
  "db_timestamp_micros": 1775963199624424
}
```

**API Methods Exist:**
- `turn.db_state()` - Returns `DbState` with timestamp
- `db_state.lookup(&db, name)` - Queries for node by name at historical timestamp  
- `record.replay_query(turn, &db, query)` - Executes arbitrary query at turn timestamp

### Known Issues (Needs Fix)

**Query Syntax Errors:**
```
Error: Cozo("Cannot find requested stored relation 'nodes'")
```

The `lookup()` and `replay_query()` methods use incorrect relation names. The actual Cozo DB uses different relation names than `*nodes`. Need to:
1. Identify correct relation names from DB schema
2. Update `lookup()` query to use correct relations (e.g., `function`, `struct`, `method`)
3. Fix `replay_query()` query parser error

**Test in `ploke-eval/tests/test_introspection.rs` shows:**
- ✅ SetupPhase populated with 9 crates
- ✅ Turn timestamp captured: `1775963199624424`
- ✅ DbState created with correct timestamp
- ⚠️ `lookup("GlobSet")` → `Cannot find requested stored relation 'nodes'`
- ⚠️ `replay_query()` → Query parser error

---

## Next Steps

### Immediate: Fix Introspection Queries

**File:** `crates/ploke-eval/src/record.rs`

1. **Fix `DbState::lookup()`** (around line 737):
   - Current query uses `*nodes{name: $name, ...}` - relation doesn't exist
   - Need to query actual relations: `function`, `struct`, `method`, etc.
   - May need to search across multiple relation types

2. **Fix `RunRecord::replay_query()`** (around line 401):
   - Query parser error at position 22 - likely syntax issue with `@ timestamp`
   - Verify Cozo `@ timestamp` syntax works with raw_query_at_timestamp

3. **Add proper relation discovery:**
   - Add method to list available relations in DB
   - Or query union of all node-type relations

### After Fix: CLI Integration

The user wants to "wire that in to our eval structure to expose the commands for introspection." Options:

1. **Add `ploke-eval introspect` subcommand:**
   ```bash
   cargo run -p ploke-eval -- introspect --run BurntSushi__ripgrep-2209 --query "..."
   ```

2. **Add methods to existing commands:**
   ```bash
   cargo run -p ploke-eval -- transcript --lookup "GlobSet"
   cargo run -p ploke-eval -- replay --run BurntSushi__ripgrep-2209 --turn 1
   ```

3. **Python/CLI tool using record library:**
   - External script that loads `record.json.gz` and uses the Rust API

---

## Test Command

To verify introspection after fixes:
```bash
cargo test -p ploke-eval --test test_introspection -- --nocapture
```

---

## Related Documents

- [Plan: iceman-white-tiger-wolverine.md](/home/brasides/.kimi/plans/iceman-white-tiger-wolverine.md)
- [Bug Report: syn 2.x fails on Rust 2015 bare trait objects](../../bugs/2026-04-10-syn-2-fails-on-rust-2015-bare-trait-objects.md)
- [CURRENT_FOCUS.md](../../CURRENT_FOCUS.md)
