# Table of Contents: xtask Commands Feature

**Date:** 2026-03-25  
**Milestone:** M.3 — implement architecture foundation (partial; see [PROJECT_SUMMARY.md](./PROJECT_SUMMARY.md))  
**Branch:** feature/xtask-commands

---

## Overview

This directory (`docs/active/agents/2026-03-24-coordination/`) serves as the central coordination hub for the multi-agent effort to build `xtask` commands for the ploke workspace. The documentation here spans survey work (M.1), architecture design (M.2), and the current implementation phase (M.3+).

### Quick Navigation

| Need To... | Go To |
|------------|-------|
| Understand the task requirements | [PRIMARY_TASK_SPEC.md](./PRIMARY_TASK_SPEC.md) |
| Check current progress | [progress.md](./progress.md) |
| See command-function mappings | [2026-03-25-command-matrix.md](./2026-03-25-command-matrix.md) |
| See test matrix vs `xtask` tests | [2026-03-25-test_matrix.md](./2026-03-25-test_matrix.md) |
| M.5 / A.5–A.6 planning entry | [m5-planning.md](./m5-planning.md) → [design/m6-planning.md](./design/m6-planning.md) |
| Review architecture decision | [design/architecture-decision.md](./design/architecture-decision.md) |
| Find test requirements | [design/test-design-requirements.md](./design/test-design-requirements.md) |
| Look up a crate's functions | [sub-agents/](#sub-agent-survey-documents) |

---

## Document Status Legend

| Status | Meaning |
|--------|---------|
| 🟢 **Final** | Document is complete and approved |
| 🟡 **Draft** | Document is work-in-progress or under review |
| 🔵 **Archived** | Document is historical/background reference |
| ⚪ **Empty** | Document exists but needs content |

---

## Root Documents

### Core Coordination

| Document | Status | Description | Lines |
|----------|--------|-------------|-------|
| [PRIMARY_TASK_SPEC.md](./PRIMARY_TASK_SPEC.md) | 🟢 Final | **PRIMARY TASK SPEC** - Sections A-G defining all requirements | 525 |
| [progress.md](./progress.md) | 🟡 Draft | Progress tracker with milestone status and assignments | - |
| [PROJECT_SUMMARY.md](./PROJECT_SUMMARY.md) | 🟡 Draft | Executive summary, codebase truth, PRIMARY_TASK_SPEC adherence snapshot | - |
| [TABLE_OF_CONTENTS.md](./TABLE_OF_CONTENTS.md) | 🟡 Draft | This document - comprehensive navigation guide | - |
| [m5-planning.md](./m5-planning.md) | 🟡 Draft | Entry pointer for PRIMARY_TASK_SPEC **M.5** (A.5–A.6); body in design/m6-planning.md | - |

### Supporting Documents

| Document | Status | Description | Lines |
|----------|--------|-------------|-------|
| [TASK_ADHERENCE_PROMPT.md](./TASK_ADHERENCE_PROMPT.md) | 🟢 Final | Instructions for task adherence agents | 41 |
| [2026-03-25-command-matrix.md](./2026-03-25-command-matrix.md) | 🟡 Draft | Survey mappings + **`xtask` impl** column (A.1–A.6); cross-crate section | - |
| [2026-03-25-test_matrix.md](./2026-03-25-test_matrix.md) | 🟡 Draft | Test matrix per PRIMARY_TASK_SPEC §E; links `xtask/tests/*.rs` | - |
| [execution_path_trace.md](./execution_path_trace.md) | 🟢 Final | Research tracing execution from `/index start` to `CodeVisitor` | 395 |

---

## Milestone M.1: Survey Crates

**Status:** ✅ Complete  
**Purpose:** Document all functions from workspace crates for xtask command implementation

### Sub-Agent Survey Documents

| Document | Status | Crate/Area | Functions | Lines |
|----------|--------|------------|-----------|-------|
| [sub-agents/survey-syn_parser.md](./sub-agents/survey-syn_parser.md) | 🟢 Final | syn_parser (A.1) | 4 primary + types | 401 |
| [sub-agents/survey-ploke_transform.md](./sub-agents/survey-ploke_transform.md) | 🟢 Final | ploke_transform (A.2) | 2 primary + schema | 367 |
| [sub-agents/survey-ploke_embed.md](./sub-agents/survey-ploke_embed.md) | 🟢 Final | ploke_embed (A.3) | 2 primary + runtime | 780 |
| [sub-agents/survey-ploke_db.md](./sub-agents/survey-ploke_db.md) | 🟢 Final | ploke_db (A.4) | 8+ primary | 619 |
| [sub-agents/survey-ploke_tui.md](./sub-agents/survey-ploke_tui.md) | 🟢 Final | ploke_tui (A.5-A.6) | Headless + 9 tools | 1000+ |
| [sub-agents/survey-test-utils.md](./sub-agents/survey-test-utils.md) | 🟢 Final | ploke_test_utils | 12+ utilities | 653 |

### Additional Function Surveys (M.1.2)

| Document | Status | Area | Functions | Lines |
|----------|--------|------|-----------|-------|
| [sub-agents/additional-syn_parser.md](./sub-agents/additional-syn_parser.md) | 🟢 Final | syn_parser diagnostic | 22+ diagnostic | 621 |
| [sub-agents/additional-ploke_db.md](./sub-agents/additional-ploke_db.md) | 🟢 Final | ploke_db diagnostic | 19+ diagnostic | 531 |

### Cross-Crate Analysis (M.1.3)

| Document | Status | Content | Lines |
|----------|--------|---------|-------|
| [sub-agents/cross-crate-commands.md](./sub-agents/cross-crate-commands.md) | 🟢 Final | 14 cross-crate pipeline/validation commands | 628 |

### Task Adherence Reports

| Document | Status | Milestone Reviewed | Lines |
|----------|--------|-------------------|-------|
| [task_adherence/m1-review-report.md](./task_adherence/m1-review-report.md) | 🟢 Final | M.1 Survey | 366 |

**M.1 Total Documentation:** ~5,297 lines across 9 survey documents

---

## Milestone M.2: Design Architecture

**Status:** ✅ Complete  
**Purpose:** Design architecture for xtask commands and select final approach

### Architecture Proposals (M.2.1)

| Document | Status | Agent | Approach | Lines |
|----------|--------|-------|----------|-------|
| [design/architecture-proposal-1.md](./design/architecture-proposal-1.md) | 🔵 Archived | Architecture Agent 1 | Layered Modular with Traits | 939 |
| [design/architecture-proposal-2.md](./design/architecture-proposal-2.md) | 🔵 Archived | Architecture Agent 2 | Clap-based with Formatters | 1000+ |
| [design/architecture-proposal-3.md](./design/architecture-proposal-3.md) | 🟢 Final | Architecture Agent 3 | **Executor Pattern with Resources** (SELECTED) | 1000+ |
| [design/test-design-requirements.md](./design/test-design-requirements.md) | 🟢 Final | Logical Test Design Agent | Proof-oriented test requirements | 1000+ |

### Design Consolidation (M.2.2)

| Document | Status | Content | Lines |
|----------|--------|---------|-------|
| [design/consolidated-reviews.md](./design/consolidated-reviews.md) | 🟢 Final | Review Agent 3's detailed analysis | 148 |
| [design/architecture-decision.md](./design/architecture-decision.md) | 🟢 Final | **FINAL DECISION** - Proposal 3 with modifications | 164 |
| [design/m6-planning.md](./design/m6-planning.md) | 🟡 Draft | PRIMARY_TASK_SPEC **M.5** (A.5–A.6) implementation planning; entry [m5-planning.md](./m5-planning.md) | - |

### Task Adherence Reports

| Document | Status | Milestone Reviewed | Lines |
|----------|--------|-------------------|-------|
| [task_adherence/m2-review-report.md](./task_adherence/m2-review-report.md) | 🟢 Final | M.2 Design | 282 |

**M.2 Total Documentation:** ~4,533 lines across 6 design documents

---

## Milestone M.3: Implement Architecture Foundation

**Status:** 🟡 In progress (partial)  
**Started:** 2026-03-25  
**Codebase snapshot:** [PROJECT_SUMMARY.md](./PROJECT_SUMMARY.md) — *Codebase truth (`xtask`)*

### M.3.1 — Types + planning

| Role | Status | Output / location |
|------|--------|-------------------|
| Bookkeeping | Complete | This TOC, PROJECT_SUMMARY, progress updates |
| Planning (PRIMARY_TASK_SPEC M.5) | Complete (draft) | [m5-planning.md](./m5-planning.md), [design/m6-planning.md](./design/m6-planning.md) |
| Engineering | Partial | `xtask/src`: `error`, `context`, `executor`, `usage`, `cli`, `commands/{mod,parse,db}`, `test_harness`; `main.rs` not wired to `Cli` |
| Binary entry | Gap | Legacy string dispatch only |

**Note:** PRIMARY_TASK_SPEC milestone **M.5** names the ploke-tui expansion. The planning file is stored as `design/m6-planning.md` for stable links; the title inside matches **M.5**.

### M.3.2 — Tests + matrix

| Deliverable | Status |
|-------------|--------|
| Integration tests under `xtask/tests/` | Partial (several `todo!(M.4)` placeholders) |
| [2026-03-25-test_matrix.md](./2026-03-25-test_matrix.md) | Seeded (canonical per PRIMARY_TASK_SPEC §E.1) |
| [xtask/tests/test_matrix.md](../../../../xtask/tests/test_matrix.md) | Pointer to coordination matrix |
| Strict TDD “fail until impl” | Not uniform across suite |

---

## Document Organization by Topic

### Parsing Commands (A.1)

**Primary Functions:**
- `run_discovery_phase` - Discovery phase for workspace/crate analysis
- `try_run_phases_and_resolve` - Parse and resolve without merging
- `try_run_phases_and_merge` - Parse, resolve, and merge graphs
- `parse_workspace` - Full workspace parsing

**Reference:** [sub-agents/survey-syn_parser.md](./sub-agents/survey-syn_parser.md)  
**Additional Functions:** [sub-agents/additional-syn_parser.md](./sub-agents/additional-syn_parser.md)

### Transform Commands (A.2)

**Primary Functions:**
- `transform_parsed_graph` - Transform parsed graph to CozoDB
- `transform_parsed_workspace` - Transform workspace to CozoDB

**Reference:** [sub-agents/survey-ploke_transform.md](./sub-agents/survey-ploke_transform.md)

### Embedding Commands (A.3)

**Primary Functions:**
- `EmbeddingProcessor::new` - Create embedding processor
- `IndexerTask::run` - Run indexing task

**Environment Variable:** `TEST_OPENROUTER_API_KEY`

**Reference:** [sub-agents/survey-ploke_embed.md](./sub-agents/survey-ploke_embed.md)

### Database Commands (A.4)

**Primary Functions:**
- `save_db` / `backup_db` - Database backup
- `load_db` / `restore_backup` - Database restore
- `load_fixture` - Load fixture database
- `count_*` - Various node counting functions
- `create_index_primary` - HNSW indexing
- `Bm25Indexer::rebuild_from_db` - BM25 rebuild
- `run_script` / `query` - Arbitrary CozoDB queries

**Reference:** [sub-agents/survey-ploke_db.md](./sub-agents/survey-ploke_db.md)  
**Additional Functions:** [sub-agents/additional-ploke_db.md](./sub-agents/additional-ploke_db.md)  
**Fixtures:** [sub-agents/survey-test-utils.md](./sub-agents/survey-test-utils.md)

### Headless TUI (A.5)

**Components:**
- `App` with `TestBackend` - Headless TUI execution
- Synthetic input events - Simulate user input
- Keycode simulation - Send key combinations
- Event bus subscription - Wait for responses

**Reference:** [sub-agents/survey-ploke_tui.md](./sub-agents/survey-ploke_tui.md)

### Tool Commands (A.6)

**Available Tools:**
- `NsRead` - Non-semantic file read
- `CodeItemLookup` - Code item lookup
- `CodeItemEdges` - Get relationship edges
- `RequestCodeContextGat` - Hybrid semantic + BM25 search
- `GatCodeEdit` - Apply canonical code edits
- `CreateFile` - Create new files
- `NsPatch` - Non-semantic patches
- `ListDir` - Safe directory listing
- `CargoTool` - Run cargo commands

**Reference:** [sub-agents/survey-ploke_tui.md](./sub-agents/survey-ploke_tui.md)

### Cross-Crate Commands

**Pipeline Commands:**
- `pipeline parse-transform` - Parse and transform in one step
- `pipeline full-ingest` - Full pipeline with embeddings
- `pipeline workspace` - Parse entire workspace

**Validation Commands:**
- `validate parse-integrity` - Check graph consistency
- `validate db-health` - Database diagnostics
- `validate end-to-end` - Verify pipeline integrity

**Setup Commands:**
- `setup test-env` - Load fixture database
- `setup dev-workspace` - Setup workspace for dev

**Workflow Commands:**
- `workflow reindex` - Full pipeline rebuild
- `workflow regenerate-fixture` - Regenerate fixtures

**Reference:** [sub-agents/cross-crate-commands.md](./sub-agents/cross-crate-commands.md)

---

## Key Decisions and References

### Architecture Selection (M.2.2)

**Selected:** Proposal 3 (Executor Pattern with Resources)  
**Document:** [design/architecture-decision.md](./design/architecture-decision.md)

**Key Components:**
- `Command` trait with associated types
- `CommandExecutor` with registry pattern
- `CommandContext` with lazy resource initialization
- `UsageTracker` for statistics and suggestions
- `CommandTestHarness` for TDD

**Modifications from Other Proposals:**
- From Proposal 1: Per-crate module organization (implemented as `commands/parse.rs`, `commands/db.rs` under one crate, not separate `xtask-db` dirs)
- From Proposal 2: structured errors (`ErrorCode`, `RecoveryHint`), clap derive macros; full `OutputFormatter` trait split across files is approximated by `OutputFormat` in `commands/mod.rs`

### Test Design Approach

**Method:** Proof-oriented testing with hypothesis format  
**Document:** [design/test-design-requirements.md](./design/test-design-requirements.md)

**Template:**
```
To Prove: [Precise correctness statement]
Given: [Preconditions]
When: [Action/input]
Then: [Expected outcome]
Invariants Verified: [...]
Fail States: [...]
Edge Cases: [...]
```

---

## Quick Reference: File Links

### By Milestone

```
M.1 (Survey):
  sub-agents/survey-*.md
  sub-agents/additional-*.md
  sub-agents/cross-crate-commands.md
  task_adherence/m1-review-report.md

M.2 (Design):
  design/architecture-proposal-{1,2,3}.md
  design/architecture-decision.md
  design/consolidated-reviews.md
  design/test-design-requirements.md
  task_adherence/m2-review-report.md

M.3+ (Implementation):
  progress.md (current status)
  PROJECT_SUMMARY.md (summary + adherence)
  m5-planning.md → design/m6-planning.md (PRIMARY_TASK_SPEC M.5)
  2026-03-25-test_matrix.md
```

### By Purpose

```
Requirements:
  PRIMARY_TASK_SPEC.md (A-G sections)
  TASK_ADHERENCE_PROMPT.md

Planning:
  2026-03-25-command-matrix.md
  2026-03-25-test_matrix.md
  m5-planning.md
  design/m6-planning.md
  execution_path_trace.md

Reference:
  sub-agents/*.md
  design/*.md

Tracking:
  progress.md
  PROJECT_SUMMARY.md
  task_adherence/*.md
```

---

## Maintenance Notes

### When Adding New Documents

1. Add entry to this TOC with appropriate status
2. Update PROJECT_SUMMARY.md if milestone changes
3. Update progress.md with agent assignments
4. Ensure consistent naming convention: `YYYY-MM-DD-descriptive-name.md`

### Status Updates

- Change status to 🟢 Final when task adherence agent approves
- Change status to 🔵 Archived when superseded by newer documents
- Keep status as 🟡 Draft while work is ongoing
- Mark as ⚪ Empty for placeholder documents

### Document Naming Convention

Per [AGENTS.md](../../../AGENTS.md):
- Use `docs/active/agents/` for shared agent documents
- Use date prefix for time-sensitive documents
- Use descriptive kebab-case names
- Group related documents in subdirectories

---

## Statistics

| Metric | Value |
|--------|-------|
| Total Documents | 20+ |
| Total Lines | ~10,000+ |
| Survey Documents | 9 |
| Design Documents | 6 |
| Task Adherence Reports | 2 |
| Coordination Documents | 6+ |
| Crates Surveyed | 6 |
| Commands Identified | 40+ |
| Test Requirements | 30+ |

---

*Last updated: 2026-03-25 — doc/code alignment with [PROJECT_SUMMARY.md](./PROJECT_SUMMARY.md)*  
*Next review: after M.4 binary wiring or major `xtask` behavior landing*
