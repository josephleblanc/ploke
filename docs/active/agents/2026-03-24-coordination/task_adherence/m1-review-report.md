# Milestone M.1 Task Adherence Review Report

**Date:** 2026-03-25  
**Reviewer:** Task Adherence Agent  
**Milestone:** M.1 - Survey crates for functions to use for commands  
**Branch:** feature/xtask-commands

---

## Executive Summary

**Recommendation: PROCEED to M.2**

The Milestone M.1 survey work has been completed satisfactorily. All required crates have been surveyed, additional diagnostic functions have been identified, and cross-crate commands have been documented. The work aligns well with the task spec sections A.1-A.4 and C-G. Documentation quality is high and well-structured.

---

## 1. M.1.1 Review: Survey of All Crates (A.1-A.4)

### Status: ✅ COMPLETE

All six crates identified in the task spec have been surveyed:

| Crate | Survey Document | Status | Functions Documented |
|-------|-----------------|--------|---------------------|
| syn_parser | `survey-syn_parser.md` | ✅ Complete | 4 primary functions (A.1) |
| ploke_transform | `survey-ploke_transform.md` | ✅ Complete | 2 primary functions (A.2) |
| ploke_embed | `survey-ploke_embed.md` | ✅ Complete | 2 primary functions (A.3) |
| ploke_db | `survey-ploke_db.md` | ✅ Complete | 8+ primary functions (A.4) |
| ploke_tui | `survey-ploke_tui.md` | ✅ Complete | Headless TUI + Tool functions (A.5-A.6) |
| ploke_test_utils | `survey-test-utils.md` | ✅ Complete | Fixture loading utilities |

### A.1 Parsing Commands (syn_parser) - VERIFIED

All four target functions from README section A.1 are documented:
- ✅ `run_discovery_phase` - Discovery phase for workspace/crate analysis
- ✅ `try_run_phases_and_resolve` - Parse and resolve without merging
- ✅ `try_run_phases_and_merge` - Parse, resolve, and merge graphs
- ✅ `parse_workspace` - Full workspace parsing

Each function includes:
- Full path and signature
- Input parameters with types
- Output/return types
- Error types
- Key types/structs needed
- Special considerations
- Example usage

### A.2 Transform Commands (ploke_transform) - VERIFIED

Target function from README section A.2 is documented:
- ✅ `transform_parsed_graph` - Transform parsed graph to CozoDB
- ✅ `transform_parsed_workspace` - Transform workspace to CozoDB (bonus)
- ✅ `create_schema_all` - Supporting function for schema creation

### A.3 Embedding Commands (ploke_embed) - VERIFIED

Target functions from README section A.3 are documented:
- ✅ `EmbeddingProcessor::new` - Process embeddings
- ✅ `IndexerTask::run` - Run indexing task
- ✅ Environment variable `TEST_OPENROUTER_API_KEY` requirement documented

**Note:** The document correctly notes that `TEST_OPENROUTER_API_KEY` is NOT yet implemented in the codebase but is a requirement for the xtask command implementation. This is per README section A.3 specification.

### A.4 Database Commands (ploke_db) - VERIFIED

All target functions from README section A.4 are documented:
- ✅ `save_db` - Via `db.db.backup_db()` (cozo method)
- ✅ `load_db` - Via `db.db.restore_backup()` (cozo method)
- ✅ `load_fixture` - `fresh_backup_fixture_db` from ploke_test_utils
- ✅ HNSW indexing - `create_index_primary`
- ✅ Count nodes - `count_pending_embeddings` and related functions
- ✅ BM25 rebuild - `Bm25Indexer::rebuild_from_db`
- ✅ Arbitrary query - `run_script` (via Deref to CozoDB)

### A.5 Headless TUI Commands (ploke_tui) - VERIFIED

- ✅ `App` with `TestBackend` documented
- ✅ Simulating user input via synthetic events documented
- ✅ Keypress simulation (keycodes, combinations) documented
- ✅ Event bus subscription for waiting on responses documented

### A.6 Tool Call Commands (ploke_tui) - VERIFIED

- ✅ `NsRead` tool documented
- ✅ `CodeItemLookup` tool documented
- ✅ 7 additional tools documented (bonus coverage)
- ✅ Direct tool execution pattern (`process_tool`) documented

---

## 2. M.1.2 Review: Additional Diagnostic Functions

### Status: ✅ COMPLETE

Two additional survey documents identify valuable diagnostic functions beyond the original A.1-A.4 list:

### Additional syn_parser Functions (`additional-syn_parser.md`)

22+ additional functions identified across categories:
- Graph validation: `validate_unique_rels`, `debug_relationships`, `debug_print_all_visible`
- Node lookup: `find_node_unique`, `find_module_by_path_checked`
- Metadata: `dependency_names`, `CrateContext` accessors
- Discovery: `iter_crate_contexts`, `has_warnings`, `warnings`
- Module tree: `ModuleTree::root`, `modules`, `tree_relations`, `pending_imports`, `pending_exports`
- Workspace: `locate_workspace_manifest`, `try_parse_manifest`, `resolve_workspace_version`
- Graph access: 15+ getter methods for different node types

### Additional ploke_db Functions (`additional-ploke_db.md`)

19+ additional functions identified across categories:
- Statistics: `count_complete_embeddings`, `count_embeddings_for_set`, `count_common_nodes`, `count_edges_by_kind`
- Index status: `is_hnsw_index_registered`, `is_embedding_set_registered`, `list_embedding_sets`, `Bm25Indexer::doc_count`
- Namespace: `list_crate_context_rows`, `collect_namespace_inventory`, `get_crate_files`, `get_path_info`
- Introspection: `relations_vec`, `relations_vec_no_hnsw`, `rel_names_with_tracking_hash`, `get_file_data`, `list_primary_nodes`
- Validation: `validate_namespace_import_conflicts`, `restore_embedding_set`, `with_active_set`

**Assessment:** The additional function identification significantly exceeds requirements. The documents provide suggested command mappings and priority recommendations that will be valuable for M.2 architecture design.

---

## 3. M.1.3 Review: Cross-Crate Commands

### Status: ✅ COMPLETE

The `cross-crate-commands.md` document comprehensively identifies 14 cross-crate commands organized into 5 categories:

### Pipeline Commands (3)
| Command | Crates | Priority |
|---------|--------|----------|
| `pipeline parse-transform` | syn_parser, ploke_transform, ploke_db | P1 |
| `pipeline full-ingest` | syn_parser, ploke_transform, ploke_db, ploke_embed | P3 |
| `pipeline workspace` | syn_parser, ploke_transform, ploke_db | P2 |

### Validation Commands (3)
| Command | Crates | Priority |
|---------|--------|----------|
| `validate parse-integrity` | syn_parser | P2 |
| `validate db-health` | ploke_db | P2 |
| `validate end-to-end` | syn_parser, ploke_transform, ploke_db | P3 |

### Diagnostic Commands (4)
| Command | Crates | Priority |
|---------|--------|----------|
| `diagnostic db-report` | ploke_db | P2 |
| `diagnostic embedding-status` | ploke_db | P3 |
| `compare parse-transform` | syn_parser, ploke_transform, ploke_db | P4 |
| `debug graph-inspect` | syn_parser | P4 |

### Setup Commands (2)
| Command | Crates | Priority |
|---------|--------|----------|
| `setup test-env` | ploke_test_utils, ploke_db | P1 |
| `setup dev-workspace` | syn_parser, ploke_transform, ploke_db | P2 |

### Workflow Commands (2)
| Command | Crates | Priority |
|---------|--------|----------|
| `workflow reindex` | All 4 ingest crates + ploke_db | P3 |
| `workflow regenerate-fixture` | ploke_test_utils, syn_parser, ploke_transform, ploke_db | P4 |

Each command includes:
- Complete workflow/order of operations
- Input parameters table
- Output description
- Error handling patterns
- Crate dependencies

The document also includes:
- Dependency graph visualization
- Execution order rules
- Error handling patterns (fail-fast, aggregate, cleanup)
- Recommended implementation priority

---

## 4. Documentation Quality Assessment

### Structure and Organization

**Rating: EXCELLENT**

All survey documents follow a consistent structure:
1. Header with date, task, agent, branch
2. Files touched during survey
3. Detailed function documentation with standardized fields
4. Types and structs summary tables
5. Issues encountered
6. Notes for future milestones (M.2)

### Completeness

**Rating: EXCELLENT**

- All A.1-A.4 functions documented with signatures, parameters, error types
- Tracing instrumentation status tracked
- Async/sync nature of functions clearly indicated
- Example usage provided from actual codebase
- Special considerations and prerequisites noted

### Cross-References

**Rating: GOOD**

- Command matrix links to survey documents
- Cross-crate commands reference specific functions
- Progress tracker links to all deliverables
- Survey documents note integration points

### Actionability for M.2

**Rating: EXCELLENT**

Each survey document includes a "Notes for M.2 Architecture Design" section with:
- Command structure recommendations
- Error handling guidance
- Output format suggestions
- Performance considerations
- Tracing integration notes

---

## 5. Task Spec Alignment Assessment

### Section A (Desired Functionality) - ALIGNED

| Requirement | Status | Notes |
|-------------|--------|-------|
| A.1 Parsing commands | ✅ | All 4 functions documented |
| A.2 Transform commands | ✅ | Primary function documented |
| A.3 Embedding commands | ✅ | Functions + TEST_OPENROUTER_API_KEY noted |
| A.4 Database commands | ✅ | All 8 functions documented |
| A.5 Headless TUI | ✅ | Comprehensive coverage |
| A.6 Tool calls | ✅ | Multiple tools documented |

### Section B (Documentation) - ALIGNED

The survey documents lay groundwork for:
- In-line documentation (functions documented)
- Help command content (command mappings provided)
- Recovery paths (error types identified)
- Usage patterns (example usage provided)

### Section C (Invariants) - ADDRESSED

The cross-crate commands document identifies:
- Feedback to stdout requirements
- Argument patterns
- Help entry needs
- New enum construction for command input

### Section D (Error Handling) - ADDRESSED

All survey documents identify:
- Error types for each function
- Recovery paths
- Special error conditions

### Section E (Tests) - NOT YET APPLICABLE

Test matrix exists but is empty. This is appropriate for M.1 (survey phase). Test documentation will be addressed in M.2.

### Section F (Out of Scope) - RESPECTED

No evidence of:
- REPL implementation attempts
- Underlying crate logic changes
- Type re-implementation (all imports noted)

### Section G (Organization) - ADDRESSED

Survey documents note the planned `xtask-db`, `xtask-parse`, etc. module structure.

---

## 6. Deviations from Task Spec

### Minor Observations (Non-Blocking)

1. **M.1.4 skipped in progress tracker**: The progress tracker marks M.1.4 as "Skipped (covered in M.1.3)". The README specifies M.1.4 should "gather required function information" for cross-crate commands. This work is indeed present in M.1.3's cross-crate-commands.md document. This consolidation is reasonable and doesn't impact quality.

2. **A.5-A.6 in single survey**: The ploke_tui survey combines A.5 (Headless TUI) and A.6 (Tool Calls) into one document. This is a reasonable organizational choice given both are in the same crate.

3. **Extra functions documented**: Survey documents include more functions than strictly required by A.1-A.4. This is beneficial, not a deviation.

### No Significant Deviations Found

The work completed for M.1 aligns well with the task spec. No corrective action is required.

---

## 7. Recommendations

### Recommendation: PROCEED to M.2

**Rationale:**
1. All required crates surveyed comprehensively
2. All A.1-A.4 functions documented with sufficient detail
3. Additional diagnostic functions identified exceeding requirements
4. Cross-crate commands documented with workflows and dependencies
5. Documentation quality is high and consistent
6. No significant deviations from task spec
7. Strong foundation for M.2 architecture design

### Action Items for M.2

1. **Architecture Design Documents** (M.2.1):
   - Create 3 parallel architecture agent documents
   - Create logical test design agent document
   - Review against survey documents for accuracy

2. **Design Consolidation** (M.2.2):
   - Create skeleton review document
   - Spawn 5 sub-agents for architecture evaluation
   - Consolidate recommendations

3. **Key Considerations from M.1**:
   - Address `TEST_OPENROUTER_API_KEY` implementation
   - Plan for async/sync function handling in commands
   - Design tracing instrumentation additions
   - Plan module structure per Section G (`xtask-db`, etc.)
   - Consider error type consolidation (`DbError`, `TransformError`, `SynParserError`)

4. **Test Matrix Population** (M.2.1):
   - Populate `2026-03-25-test_matrix.md` with test conditions
   - Reference survey documents for underlying functions

---

## 8. Appendix: Document Registry

### Primary Documents
| Document | Purpose | Lines |
|----------|---------|-------|
| `README.md` | Task specification | 525 |
| `progress.md` | Progress tracker | 72 |
| `2026-03-25-command-matrix.md` | Command-function mapping | 379 |

### Survey Documents (sub-agents/)
| Document | Crate/Area | Lines | Functions |
|----------|-----------|-------|-----------|
| `survey-syn_parser.md` | syn_parser | 401 | 4 primary + types |
| `survey-ploke_transform.md` | ploke_transform | 367 | 2 primary + schema |
| `survey-ploke_embed.md` | ploke_embed | 780 | 2 primary + runtime |
| `survey-ploke_db.md` | ploke_db | 619 | 8+ primary |
| `survey-ploke_tui.md` | ploke_tui | 1000+ | Headless + 9 tools |
| `survey-test-utils.md` | ploke_test_utils | 653 | 12+ utilities |
| `additional-syn_parser.md` | syn_parser extras | 621 | 22+ diagnostic |
| `additional-ploke_db.md` | ploke_db extras | 531 | 19+ diagnostic |
| `cross-crate-commands.md` | Cross-crate | 628 | 14 commands |

**Total Documentation:** ~5,297 lines across 9 survey documents

---

## 9. Conclusion

Milestone M.1 has been completed successfully. The survey work provides a comprehensive foundation for the architecture design phase (M.2). All required functions are documented, additional valuable functions have been identified, and cross-crate workflows are well-defined. The documentation is of high quality and ready to support the next phase of development.

**Final Verdict: APPROVED for transition to M.2**

---

*Report generated by Task Adherence Agent*  
*Review completed: 2026-03-25*
