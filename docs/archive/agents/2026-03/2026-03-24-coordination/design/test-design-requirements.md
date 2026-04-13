# Test Design Requirements for xtask Commands

**Date:** 2026-03-25  
**Milestone:** M.2.1 - Design Architecture + Documentation  
**Agent:** Logical and Test Design Agent  
**Branch:** feature/xtask-commands

---

## Executive Summary

This document provides proof-oriented test design requirements for all xtask commands identified in M.1 (Survey phase). For each command, we define the logical conditions necessary for tests to prove correctness, the invariants that must hold, and the failure modes that must be exercised.

**Key Testing Principles:**
1. **Proof-Oriented**: Every test must state precisely what it proves (hypothesis format)
2. **Invariant-Based**: Tests verify that invariants hold under all conditions
3. **Fail-State Coverage**: Tests must exercise known failure modes
4. **Isolation**: Tests should be independent and hermetic where possible

---

## Test Hypothesis Template

For each test, the following must be documented:

```
To Prove: [Precise statement of what correctness property is verified]
Given: [Preconditions and assumptions]
When: [Action or input]
Then: [Expected outcome that proves the hypothesis]

Invariants Verified:
- [Invariant 1]
- [Invariant 2]

Fail States:
- [Failure mode 1]
- [Failure mode 2]

Edge Cases:
- [Edge case 1]
- [Edge case 2]

When This Test Would NOT Prove Correctness:
- [Condition that would invalidate the proof]
```

---

## Command Category A.1: Parsing Commands

### Command: `parse discovery`

**Functions Under Test:**
- `syn_parser::discovery::run_discovery_phase`

**Hypothesis Tests:**

#### Test A.1.1: Discovery Basic Functionality
```
To Prove: That run_discovery_phase correctly identifies crate structure from a valid Cargo.toml
Given: A valid crate path with Cargo.toml
When: Command is executed with path argument
Then: Returns DiscoveryOutput containing at least one CrateContext

Invariants Verified:
- DiscoveryOutput contains non-empty crate_contexts
- Each CrateContext has valid name and version
- Source files are discoverable

Fail States:
- Invalid path (non-existent directory)
- Missing Cargo.toml
- Malformed Cargo.toml
- Permission denied on directory

Edge Cases:
- Empty crate (no source files)
- Workspace root (multiple crates)
- Path with spaces/special characters
- Relative vs absolute paths

Mock/Stub Requirements:
- None - uses real filesystem

Fixture Requirements:
- fixture_test_crate (minimal valid crate)
- ploke-test-utils for temp directory setup

When This Test Would NOT Prove Correctness:
- If the fixture crate structure doesn't represent real-world crates
- If filesystem behavior differs across platforms
```

#### Test A.1.2: Discovery Error Handling
```
To Prove: That the command provides actionable error messages for each failure mode
Given: Various invalid inputs
When: Command is executed with each invalid input
Then: Each error includes recovery path and context

Invariants Verified:
- Error output includes path that failed
- Error type is distinguishable
- Recovery hint is present

Fail States:
- IO errors (permission denied, not found)
- Parse errors (malformed Cargo.toml)
- Logical errors (no Cargo.toml in path)

Edge Cases:
- Very long path names
- Paths with unicode characters
- Symbolic links in path

When This Test Would NOT Prove Correctness:
- If error types are unified/lost in translation layers
```

---

### Command: `parse phases-resolve`

**Functions Under Test:**
- `syn_parser::try_run_phases_and_resolve`

**Hypothesis Tests:**

#### Test A.1.3: Phase Resolution Success
```
To Prove: That try_run_phases_and_resolve produces valid ParserOutput with resolved graphs
Given: A valid crate path
When: Command executes with valid path
Then: Returns ParserOutput containing ParsedCodeGraphs with resolved relations

Invariants Verified:
- Output contains at least one ParsedCodeGraph
- Graphs have valid module trees
- Relations reference existing nodes
- No unresolved imports in final output

Fail States:
- Discovery phase fails
- Parse phase fails (syntax errors)
- Resolution fails (circular dependencies)

Edge Cases:
- Crate with no modules
- Crate with circular module dependencies
- Crate with external dependencies

Fixture Requirements:
- fixture_test_crate
- Complex fixture with nested modules

When This Test Would NOT Prove Correctness:
- If resolution logic changes for edge cases not in fixtures
```

#### Test A.1.4: Phase Resolution Output Formats
```
To Prove: That command output can be formatted correctly in all supported formats
Given: Successful phase resolution
When: Command executed with --format json, --format table, default
Then: Output is valid and complete in each format

Invariants Verified:
- JSON output is valid JSON
- Table output has consistent columns
- All formats contain same data

Edge Cases:
- Very large output (truncation)
- Unicode in output
- Empty results
```

---

### Command: `parse phases-merge`

**Functions Under Test:**
- `syn_parser::try_run_phases_and_merge`

**Hypothesis Tests:**

#### Test A.1.5: Phase Merge Produces Merged Graph
```
To Prove: That try_run_phases_and_merge correctly merges multiple ParsedCodeGraphs into one
Given: A workspace or crate with multiple modules
When: Command executes
Then: Returns ParserOutput with merged graph containing all nodes

Invariants Verified:
- Merged graph node count equals sum of input graph nodes
- No duplicate node IDs
- Relations are preserved after merge
- Module tree is consistent

Fail States:
- ID collision during merge
- Inconsistent module trees
- Memory exhaustion on huge graphs

Edge Cases:
- Single module (no actual merge)
- Many modules (stress test)
- Modules with same name in different paths

When This Test Would NOT Prove Correctness:
- If merge logic has special cases for specific node types not exercised
```

---

### Command: `parse workspace`

**Functions Under Test:**
- `syn_parser::parse_workspace`

**Hypothesis Tests:**

#### Test A.1.6: Workspace Parsing
```
To Prove: That parse_workspace discovers and parses all crates in a workspace
Given: A valid workspace with multiple crates
When: Command executes with workspace path
Then: Returns ParsedWorkspace containing all crate graphs

Invariants Verified:
- All crates in workspace are parsed
- Each crate graph is valid
- Cross-crate references are tracked
- Workspace metadata is captured

Fail States:
- Missing workspace Cargo.toml
- Individual crate parse failures
- Mixed workspace/virtual manifest issues

Edge Cases:
- Single-crate workspace
- Workspace with many crates
- Workspace with path dependencies
- Selective crate parsing (--crates flag)

Fixture Requirements:
- ws_fixture_01_canonical
- Current ploke workspace (self-reference)

When This Test Would NOT Prove Correctness:
- If workspace structure differs from Cargo.toml patterns tested
```

---

## Command Category A.2: Transform Commands

### Command: `transform graph`

**Functions Under Test:**
- `ploke_transform::transform_parsed_graph`

**Hypothesis Tests:**

#### Test A.2.1: Graph Transformation Success
```
To Prove: That transform_parsed_graph correctly inserts all graph data into CozoDB
Given: Valid ParsedCodeGraph and ModuleTree, initialized database
When: Function is called
Then: Database contains all nodes and relations from graph

Invariants Verified:
- Node count in DB matches graph node count
- All relations are inserted with correct sources/targets
- Schema is created before data insertion
- IDs are preserved/transformed consistently

Fail States:
- Database not initialized
- Schema creation fails
- Duplicate ID insertion
- Constraint violations

Edge Cases:
- Empty graph (no nodes)
- Graph with only relations
- Graph with many node types

Fixture Requirements:
- ParsedCodeGraph from fixture_test_crate
- ploke_test_utils::Database setup

When This Test Would NOT Prove Correctness:
- If CozoDB has different behavior with large datasets
- If transaction behavior differs
```

#### Test A.2.2: Transform Idempotency
```
To Prove: That running transform twice on same data produces consistent results
Given: Valid graph and initialized database
When: Transform is run twice
Then: Second run either fails gracefully or is idempotent

Invariants Verified:
- No data corruption on duplicate run
- Appropriate error if duplicates not allowed
- Clear error message if operation fails

When This Test Would NOT Prove Correctness:
- If transaction isolation levels differ
```

---

### Command: `transform workspace`

**Functions Under Test:**
- `ploke_transform::transform_parsed_workspace`

**Hypothesis Tests:**

#### Test A.2.3: Workspace Transform
```
To Prove: That transform_parsed_workspace transforms all crates in workspace
Given: Valid ParsedWorkspace with multiple crates
When: Function is called with database
Then: All crate graphs are in database with proper cross-references

Invariants Verified:
- All crate graphs transformed
- Cross-crate relations preserved
- Crate metadata in database
- Namespace consistency

Edge Cases:
- Workspace with single crate
- Empty workspace
- Workspace with dependency cycles

When This Test Would NOT Prove Correctness:
- If workspace crates have complex interdependencies not in fixtures
```

---

## Command Category A.3: Ingestion Pipeline

### Command: `ingest embed`

**Functions Under Test:**
- `ploke_embed::EmbeddingProcessor::new`
- Related embedding pipeline functions

**Hypothesis Tests:**

#### Test A.3.1: Embedding Processor Creation
```
To Prove: That EmbeddingProcessor::new initializes with correct configuration
Given: Valid configuration parameters
When: Processor is created
Then: Processor has expected state and configuration

Invariants Verified:
- Configuration is stored correctly
- Required resources are allocated
- Invalid config rejected with error

Fail States:
- Missing TEST_OPENROUTER_API_KEY when needed
- Invalid backend specification
- Resource allocation failure

Edge Cases:
- Mock backend (no API key needed)
- Local backend
- OpenRouter backend with test key

Mock/Stub Requirements:
- Mock embedding backend for unit tests
- Stub API responses

When This Test Would NOT Prove Correctness:
- If real API behavior differs from mocks
```

---

### Command: `ingest index`

**Functions Under Test:**
- `ploke_embed::IndexerTask::run`

**Hypothesis Tests:**

#### Test A.3.2: Indexer Task Execution
```
To Prove: That IndexerTask::run processes pending embeddings correctly
Given: Database with nodes needing embeddings, initialized embedding processor
When: Indexer task runs
Then: Embeddings are generated and stored for eligible nodes

Invariants Verified:
- Pending count decreases (or reaches zero)
- Embeddings are stored with correct dimensions
- Progress updates are sent
- Task can be cancelled gracefully

Fail States:
- Database connection lost
- API errors (rate limit, invalid key)
- Embedding dimension mismatch
- Progress channel closed

Edge Cases:
- No pending embeddings
- All nodes already have embeddings
- Mixed embedding sets
- Very large batch size

Mock/Stub Requirements:
- Mock embedding provider
- Controlled progress channel

When This Test Would NOT Prove Correctness:
- If real embedding generation has different latency/error patterns
```

#### Test A.3.3: API Key Handling
```
To Prove: That TEST_OPENROUTER_API_KEY is loaded and used correctly
Given: Environment variable set or not set
When: Command executes with different backend options
Then: Correct key is used, errors if required key missing

Invariants Verified:
- TEST_OPENROUTER_API_KEY used, not OPENROUTER_API_KEY
- No overrides accepted (security invariant)
- Clear error when key missing but required

Fail States:
- Key not set but OpenRouter backend requested
- Key invalid (401 from API)

When This Test Would NOT Prove Correctness:
- If environment variable handling differs by platform
```

---

## Command Category A.4: Database Commands

### Command: `db save`

**Functions Under Test:**
- CozoDB `backup_db` via `ploke_db::Database`

**Hypothesis Tests:**

#### Test A.4.1: Database Backup
```
To Prove: That db save creates a valid backup file containing all database data
Given: Initialized database with data
When: Save command executes with path
Then: Backup file exists and can be restored

Invariants Verified:
- Backup file is created
- File is not empty
- Backup format is valid (can be loaded)
- Original database is unchanged

Fail States:
- Invalid path (directory doesn't exist)
- Permission denied
- Disk full
- Database locked

Edge Cases:
- Empty database backup
- Large database backup
- Path with special characters

When This Test Would NOT Prove Correctness:
- If backup format changes in future CozoDB versions
```

---

### Command: `db load`

**Functions Under Test:**
- CozoDB `restore_backup` via `ploke_db::Database`

**Hypothesis Tests:**

#### Test A.4.2: Database Restore
```
To Prove: That db load restores database to state at backup time
Given: Valid backup file
When: Load command executes
Then: Database contains all data from backup

Invariants Verified:
- All relations restored
- All nodes restored
- Counts match pre-backup state
- Schema is correct

Fail States:
- Invalid backup file
- Corrupted backup
- Incompatible backup version
- Target database not empty (if not allowed)

Edge Cases:
- Loading into new database
- Loading with existing data (overwrite/merge)
- Concurrent access during load

When This Test Would NOT Prove Correctness:
- If backup contains schema versions not supported
```

---

### Command: `db load-fixture`

**Functions Under Test:**
- `ploke_test_utils::fresh_backup_fixture_db`

**Hypothesis Tests:**

#### Test A.4.3: Fixture Loading
```
To Prove: That load-fixture correctly loads known fixture databases
Given: Valid fixture ID from registry
When: Command executes with fixture ID
Then: Database contains fixture data

Invariants Verified:
- Fixture is found in registry
- Database is initialized
- Fixture data is loaded correctly
- Fixture contract is validated

Fail States:
- Invalid fixture ID
- Fixture file missing
- Fixture corrupted
- Schema version mismatch

Edge Cases:
- All available fixtures load correctly
- Fixture with embeddings
- Fixture without embeddings

Fixture Requirements:
- FIXTURE_NODES_CANONICAL
- FIXTURE_NODES_LOCAL_EMBEDDINGS
- PLOKE_DB_PRIMARY
- WS_FIXTURE_01_CANONICAL

When This Test Would NOT Prove Correctness:
- If fixture schema differs from current code expectations
- See BACKUP_DB_FIXTURES.md for fixture review process
```

---

### Command: `db count-nodes`

**Functions Under Test:**
- `ploke_db::Database::count_pending_embeddings`
- `ploke_db::Database::count_unembedded_nonfiles`
- `ploke_db::Database::count_unembedded_files`
- Related counting functions

**Hypothesis Tests:**

#### Test A.4.4: Node Counting Accuracy
```
To Prove: That count commands return accurate counts matching actual database state
Given: Database with known node population
When: Count command executes
Then: Returned count matches expected count

Invariants Verified:
- Count is non-negative
- Sum of category counts equals total
- Counts match direct query results

Fail States:
- Database not initialized
- Query syntax error
- Database locked

Edge Cases:
- Empty database (count = 0)
- Database with one node
- Database with many nodes

When This Test Would NOT Prove Correctness:
- If CozoDB has different counting semantics
```

---

### Command: `db hnsw-build`

**Functions Under Test:**
- `ploke_db::index::hnsw::create_index_primary`

**Hypothesis Tests:**

#### Test A.4.5: HNSW Index Creation
```
To Prove: That create_index_primary builds valid HNSW index from embeddings
Given: Database with embeddings, initialized schema
When: HNSW build command executes
Then: Index is created and queryable

Invariants Verified:
- Index relation is created
- Index contains entries for all embedded nodes
- Index is registered in metadata
- Subsequent similarity queries work

Fail States:
- No embeddings in database
- Schema not initialized
- Insufficient memory
- Invalid embedding dimensions

Edge Cases:
- Single embedding
- Many embeddings
- Mixed dimension embeddings (should fail)

When This Test Would NOT Prove Correctness:
- If HNSW parameters affect behavior not tested
```

---

### Command: `db hnsw-rebuild`

**Functions Under Test:**
- `ploke_db::index::hnsw::create_index_primary_with_index`

**Hypothesis Tests:**

#### Test A.4.6: HNSW Index Rebuild
```
To Prove: That hnsw-rebuild recreates index with updated embeddings
Given: Existing HNSW index, database with new/updated embeddings
When: Rebuild command executes
Then: Index reflects current state of embeddings

Invariants Verified:
- Old index is replaced
- New index contains current embeddings
- No orphaned index entries

When This Test Would NOT Prove Correctness:
- If rebuild has different semantics than initial build
```

---

### Command: `db bm25-rebuild`

**Functions Under Test:**
- `ploke_db::bm25_index::Bm25Indexer::rebuild_from_db`

**Hypothesis Tests:**

#### Test A.4.7: BM25 Index Rebuild
```
To Prove: That bm25-rebuild creates searchable text index from database content
Given: Database with code/text content
When: BM25 rebuild command executes
Then: BM25 index is created and text search works

Invariants Verified:
- Index is created for all text fields
- BM25 scoring is functional
- Index metadata is updated

Fail States:
- No text content to index
- BM25 service unavailable
- Invalid configuration

When This Test Would NOT Prove Correctness:
- If BM25 configuration affects scoring
```

---

### Command: `db query`

**Functions Under Test:**
- CozoDB `run_script` via `ploke_db::Database` Deref

**Hypothesis Tests:**

#### Test A.4.8: Arbitrary Query Execution
```
To Prove: That db query executes CozoDB queries and returns results correctly
Given: Valid CozoDB query string, initialized database
When: Query command executes
Then: Query results are returned in specified format

Invariants Verified:
- Valid queries return results
- Invalid queries return errors with context
- Results are formatted correctly (JSON/table)
- Original query is echoed in error messages

Fail States:
- Syntax error in query
- Query references non-existent relations
- Database not initialized
- Timeout on long-running query

Edge Cases:
- Empty result set
- Very large result set
- Query with parameters
- Recursive/multi-line queries

When This Test Would NOT Prove Correctness:
- If CozoDB query semantics change
```

---

## Command Category A.5: Headless TUI Commands

### Command: `tui headless`

**Functions Under Test:**
- `ploke_tui::App` with `ratatui::backend::TestBackend`

**Hypothesis Tests:**

#### Test A.5.1: Headless TUI Initialization
```
To Prove: That App can be initialized in headless mode with TestBackend
Given: Valid configuration
When: Headless command initializes App
Then: App runs without display, backend is TestBackend

Invariants Verified:
- App initializes successfully
- Backend is TestBackend type
- No display server required
- App state is valid

Fail States:
- Configuration invalid
- Required resources unavailable
- Actor initialization fails

Mock/Stub Requirements:
- Mock LLM client for tool calls
- Stub embedding service

When This Test Would NOT Prove Correctness:
- If TestBackend behavior differs from real backend
```

---

### Command: `tui input`

**Functions Under Test:**
- TUI input simulation (custom xtask implementation)
- Event sending to App

**Hypothesis Tests:**

#### Test A.5.2: Input Simulation
```
To Prove: That simulated user input is processed by App correctly
Given: Running headless App
When: Input string is sent
Then: App processes input and produces expected output

Invariants Verified:
- Input reaches App input handler
- App state updates appropriately
- Response/Output is captured
- Timeout handles slow responses

Fail States:
- App not running
- Input channel closed
- Timeout exceeded
- App panics during processing

Edge Cases:
- Empty input
- Very long input
- Unicode input
- Special characters
- Input with newlines

When This Test Would NOT Prove Correctness:
- If timing differences affect behavior
- If App has race conditions only visible with real backend
```

---

### Command: `tui key`

**Functions Under Test:**
- TUI keycode simulation (custom xtask implementation)

**Hypothesis Tests:**

#### Test A.5.3: Keycode Simulation
```
To Prove: That simulated keycodes trigger correct App behavior
Given: Running headless App in appropriate state
When: Keycode (e.g., Esc, Ctrl+f) is sent
Then: App responds as if real key was pressed

Invariants Verified:
- Key event reaches App
- Correct handler is invoked
- App state changes appropriately

Keycodes to Test:
- <Esc> - Cancel/exit
- <Enter> - Submit
- Ctrl+f - Find
- Arrow keys - Navigation
- Ctrl+c - Interrupt (if handled)

When This Test Would NOT Prove Correctness:
- If key handling differs by terminal type
- If modifier key handling varies
```

---

## Command Category A.6: Tool Call Commands

### Command: `tool ns-read`

**Functions Under Test:**
- `ploke_tui::tools::ns_read::NsRead`

**Hypothesis Tests:**

#### Test A.6.1: Namespace Read Tool Execution
```
To Prove: That NsRead tool executes correctly with provided arguments
Given: Initialized database, valid namespace path
When: Tool is called with JSON arguments
Then: Returns namespace contents as expected

Invariants Verified:
- Tool receives correct arguments
- Database is accessible
- Output format is correct
- Errors include context

Fail States:
- Invalid namespace path
- Database not initialized
- Tool not registered
- Invalid JSON arguments

Mock/Stub Requirements:
- Minimal actor setup for tool context
- Stub database with known content

When This Test Would NOT Prove Correctness:
- If full TUI has additional context not available in minimal setup
```

---

### Command: `tool code-lookup`

**Functions Under Test:**
- `ploke_tui::tools::code_item_lookup::CodeItemLookup`

**Hypothesis Tests:**

#### Test A.6.2: Code Item Lookup Tool Execution
```
To Prove: That CodeItemLookup tool finds code items correctly
Given: Database with code graph, valid query parameters
When: Tool is called with JSON arguments
Then: Returns matching code items

Invariants Verified:
- Query parameters parsed correctly
- Search executes correctly
- Results contain expected items
- Rank/score is included if applicable

Fail States:
- Invalid query parameters
- No matching items
- Database error during search

When This Test Would NOT Prove Correctness:
- If ranking algorithm changes
```

---

## Cross-Crate Command Test Strategies

### Pipeline Commands (pipeline parse-transform, pipeline full-ingest, etc.)

**Testing Strategy:** Layered Verification

```
Layer 1: Component Verification
- Test each stage independently (covered in A.1-A.4)
- Verify outputs are valid inputs for next stage

Layer 2: Integration Testing
- Test stage chaining with known good data
- Verify data flows correctly between stages

Layer 3: End-to-End Testing
- Test complete pipeline with real fixtures
- Verify final output matches expected state
```

#### Test CC.1: Pipeline parse-transform Integration
```
To Prove: That parse and transform stages chain correctly
Given: Valid crate path
When: Pipeline parse-transform executes
Then: Database contains transformed graph without intermediate failures

Invariants Verified:
- Parse output is valid transform input
- Transform produces valid database state
- No data loss between stages
- Progress reported at each stage

Fail States:
- Parse fails (stop, cleanup)
- Transform fails (cleanup partial DB)
- Database init fails (early exit)

When This Test Would NOT Prove Correctness:
- If stage outputs are mocked/artificial
```

#### Test CC.2: Pipeline full-ingest Integration
```
To Prove: That full pipeline completes successfully with embedding generation
Given: Valid crate, embedding backend
When: Pipeline full-ingest executes
Then: Database contains graph and embeddings, HNSW index built

Invariants Verified:
- All stages complete in order
- Embeddings are generated for nodes
- Index is built and queryable
- Progress reported throughout

Fail States:
- Any stage fails (fail-fast)
- Embedding API errors
- Timeout during indexing

When This Test Would NOT Prove Correctness:
- If embedding generation is mocked
- If API latency makes test flaky
```

---

### Validation Commands (validate parse-integrity, validate db-health, etc.)

**Testing Strategy:** Aggregate Error Collection

```
Strategy: Run all validation checks, collect all results, report aggregate
- Unlike pipeline commands, validation should not fail-fast
- All checks should run even if some fail
- Report provides overview of all issues
```

#### Test CC.3: Validation Aggregate Reporting
```
To Prove: That validation commands report all issues, not just first
Given: Database with multiple issues
When: Validation command executes
Then: All issues are reported, not just the first

Invariants Verified:
- All checks execute
- Results are aggregated
- Report includes all findings
- Exit code reflects overall status

When This Test Would NOT Prove Correctness:
- If validation checks have side effects
```

---

### Setup Commands (setup test-env, setup dev-workspace)

**Testing Strategy:** Idempotency and Cleanup

```
Strategy: Commands should be safe to re-run, cleanup after failures
- Setup should check for existing state
- Failed setup should not leave system in broken state
- Cleanup should be thorough
```

#### Test CC.4: Setup Idempotency
```
To Prove: That setup commands can be run multiple times safely
Given: Environment after first setup
When: Setup command runs again
Then: Either succeeds idempotently or fails gracefully with clear message

Invariants Verified:
- No corruption on re-run
- Clear status of what was done
- No resource leaks

When This Test Would NOT Prove Correctness:
- If concurrent setup occurs
```

---

## Integration Test Organization

### Directory Structure

```
xtask/tests/
├── integration/
│   ├── mod.rs                    # Shared integration test utilities
│   ├── parsing_tests.rs          # A.1 commands
│   ├── transform_tests.rs        # A.2 commands
│   ├── ingest_tests.rs           # A.3 commands
│   ├── database_tests.rs         # A.4 commands
│   ├── tui_tests.rs              # A.5 commands
│   ├── tool_tests.rs             # A.6 commands
│   ├── pipeline_tests.rs         # Cross-crate pipeline tests
│   ├── validation_tests.rs       # Cross-crate validation tests
│   └── setup_tests.rs            # Cross-crate setup tests
├── fixtures/
│   ├── mod.rs                    # Fixture loading helpers
│   └── test_data/                # Small test-specific fixtures
└── test_matrix.md                # Generated test tracking
```

### Test Categories

| Category | File | Commands Covered |
|----------|------|------------------|
| Parsing | `parsing_tests.rs` | parse discovery, parse phases-resolve, parse phases-merge, parse workspace |
| Transform | `transform_tests.rs` | transform graph, transform workspace |
| Ingest | `ingest_tests.rs` | ingest embed, ingest index |
| Database | `database_tests.rs` | db save, db load, db load-fixture, db count-nodes, db hnsw-*, db bm25-rebuild, db query |
| TUI | `tui_tests.rs` | tui headless, tui input, tui key |
| Tools | `tool_tests.rs` | tool ns-read, tool code-lookup |
| Pipeline | `pipeline_tests.rs` | pipeline parse-transform, pipeline full-ingest, pipeline workspace |
| Validation | `validation_tests.rs` | validate parse-integrity, validate db-health, validate end-to-end |
| Setup | `setup_tests.rs` | setup test-env, setup dev-workspace |

---

## Mock and Stub Requirements Summary

### Required Mocks

| Component | Mock For | Purpose |
|-----------|----------|---------|
| EmbeddingProvider | ploke_embed backends | Unit tests without API calls |
| TestBackend | ratatui display | Headless TUI testing |
| LLMClient | ploke_tui tool calls | Tool testing without LLM |
| FileSystem (optional) | std::fs operations | Deterministic file tests |

### Required Stubs

| Component | Stubs For | Purpose |
|-----------|-----------|---------|
| Database | ploke_db::Database | Pre-populated test databases |
| Event Channels | tokio::broadcast | Controlled event testing |
| Progress Channels | tokio::mpsc | Progress reporting tests |

---

## Fixture Requirements Summary

### Existing Fixtures (from ploke-test-utils)

| Fixture | Used For | Test Categories |
|---------|----------|-----------------|
| fixture_test_crate | Basic parsing tests | A.1, A.2 |
| FIXTURE_NODES_CANONICAL | Database operations | A.4, A.5, A.6 |
| FIXTURE_NODES_LOCAL_EMBEDDINGS | Embedding tests | A.3, A.4 |
| PLOKE_DB_PRIMARY | Real-world data | A.4, CC |
| WS_FIXTURE_01_CANONICAL | Workspace tests | A.1, CC |

### Required Test-Specific Fixtures

| Fixture | Purpose | Scope |
|---------|---------|-------|
| minimal_crate | Minimal valid crate | Unit tests |
| circular_deps_crate | Circular module dependencies | Edge case tests |
| no_modules_crate | Empty/edge case | Edge case tests |
| large_crate | Performance/stress tests | Stress tests |
| invalid_cargo_toml | Error handling tests | Fail state tests |

---

## Test Coverage Targets

### Coverage by Command Category

| Category | Target Coverage | Priority |
|----------|-----------------|----------|
| A.1 Parsing | 90% | P1 |
| A.2 Transform | 85% | P1 |
| A.3 Ingest | 80% | P2 |
| A.4 Database | 90% | P1 |
| A.5 Headless TUI | 75% | P2 |
| A.6 Tool Calls | 80% | P2 |
| Cross-Crate Pipeline | 85% | P1 |
| Cross-Crate Validation | 80% | P2 |
| Cross-Crate Setup | 85% | P1 |

### Coverage Metrics

```
Line Coverage Target: 80% minimum, 90% for P1
Branch Coverage Target: 75% minimum, 85% for P1
Function Coverage Target: 90% for all public APIs
Error Path Coverage: All documented fail states must have tests
```

---

## Test Quality Checklist

For each test written in M.3.2, verify:

- [ ] Hypothesis is stated clearly (To Prove: ...)
- [ ] Preconditions are documented (Given: ...)
- [ ] Action is specific (When: ...)
- [ ] Expected outcome proves hypothesis (Then: ...)
- [ ] Invariants are listed
- [ ] Fail states are tested OR documented why not
- [ ] Edge cases are identified
- [ ] Mock/stub requirements are stated
- [ ] Fixture requirements are stated
- [ ] Conditions where test would NOT prove correctness are documented

---

## Documentation Requirements for Test-Writing Agents

When writing tests in M.3.2, agents must:

1. **Reference this document** for each command being tested
2. **Follow the hypothesis format** in test doc comments
3. **Update test_matrix.md** with test links and status
4. **Use shared fixtures** from ploke-test-utils where available
5. **Create test-specific fixtures** only when necessary
6. **Document any deviations** from this design with rationale

---

## Notes for M.3.2 Implementation

1. **Start with A.1 and A.4** - These have the most existing test infrastructure
2. **Use ploke-test-utils** - Many utilities already exist
3. **Check existing tests** - syn_parser and ploke_db have comprehensive tests to reference
4. **Mock external APIs** - Never call real OpenRouter API in tests
5. **Clean up resources** - Use temp directories, drop databases after tests
6. **Parallel safety** - Tests should not interfere with each other
7. **Tracing integration** - Use tracing-test for log capture in tests

---

## Related Documents

- [Command Matrix](../2026-03-25-command-matrix.md) - All commands with functions
- [Test Matrix](../2026-03-25-test_matrix.md) - To be filled by test-writing agents
- [Cross-Crate Commands](../sub-agents/cross-crate-commands.md) - Pipeline details
- [BACKUP_DB_FIXTURES.md](../../../../testing/BACKUP_DB_FIXTURES.md) - Fixture registry
- [PRIMARY_TASK_SPEC.md](../PRIMARY_TASK_SPEC.md) - Main specification
