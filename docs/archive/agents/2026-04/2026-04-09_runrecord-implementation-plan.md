# RunRecord Implementation Plan (SUPERSEDED)

**⚠️ DEPRECATED:** This document has been superseded by [Phase 1 RunRecord Tracking](../plans/evals/phase-1-runrecord-tracking.md)

**Date:** 2026-04-09  
**Status:** Superseded - See updated plan for current status  
**Branch:** `refactor/tool-calls`  
**Related:** [2026-04-09_run-record-type-inventory.md](./2026-04-09_run-record-type-inventory.md), [2026-04-09_run-record-design-handoff.md](../workflow/handoffs/2026-04-09_run-record-design-handoff.md), **[phase-1-runrecord-tracking.md](../plans/evals/phase-1-runrecord-tracking.md)**

---

## Summary

This plan outlines the implementation of a comprehensive `RunRecord` for the ploke-eval harness. The RunRecord will aggregate all run data into a single queryable structure, enabling replay and introspection (A5 - hard gate for H0 interpretation).

---

## Current State

### Existing Artifacts (12+ files per run)
- `run.json` - PreparedSingleRun (manifest)
- `execution-log.json` - ExecutionLog (steps)
- `repo-state.json` - RepoStateArtifact
- `indexing-status.json` - IndexingStatusArtifact
- `agent-turn-trace.json` / `agent-turn-summary.json` - AgentTurnArtifact
- `final-snapshot.db` - CozoDB final state
- Plus: checkpoint DBs, parse failures, MSB submission

### Critical Gaps
| Data | Currently | Needed For |
|------|-----------|------------|
| LLM request/response | Debug strings only | Structured replay |
| Token usage | Logged but not persisted | Cost analysis |
| Conversation history | Initial prompt only | Full reconstruction |
| Cozo timestamps | Not captured | Time-travel queries |

---

## Updated Plan Location

**This plan has been superseded by:**

📄 **[Phase 1 RunRecord Tracking](../plans/evals/phase-1-runrecord-tracking.md)**

The updated plan includes:
- Workflow-validated structure (aligned with Phase 1 Foundations)
- Minimized production code changes (use existing `ChatEvt::Response`)
- Detailed task breakdown with acceptance criteria
- Progress tracking for interruption recovery
- Link to A5 in hypothesis registry

---

## Key Findings from Validation

### ✅ Workflow Alignment
- Plan moved to `docs/active/plans/evals/` (proper location for phase tracking)
- Linked to A5 in hypothesis registry
- Follows `phase-tracking-template.md` structure

### ✅ Production Code Changes Minimized
- Can capture ~80% of LLM data using **existing** `ChatEvt::Response` event
- Only need to modify `ploke-eval` (not `ploke-tui/ploke-llm`) for initial implementation
- Optional: Add `ChatEvt::StepCompleted` variant if raw response data needed (~10 lines)

---

## Next Steps

See **[Phase 1 RunRecord Tracking](../plans/evals/phase-1-runrecord-tracking.md)** for:
1. Phase 1A: Foundation Types (Est. 4 hrs)
2. Phase 1B: Cozo Time Travel (Est. 2 hrs)
3. Phase 1C: Conversation History (Est. 2 hrs)
4. Phase 1D: Structured LLM Capture (Est. 3 hrs)
5. Phase 1E: Emission and Compression (Est. 2 hrs)
6. Phase 1F: Introspection API (Est. 3 hrs)

**Current state:** Ready to begin Phase 1A
