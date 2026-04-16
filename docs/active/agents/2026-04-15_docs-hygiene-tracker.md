# Docs Hygiene Tracker

- date: 2026-04-15
- task title: docs hygiene tracker
- task description: track the current restart/doc-hygiene findings, unattended-doc candidates, and a stable folder table-of-contents policy for later review
- related planning files: `docs/active/CURRENT_FOCUS.md`, `docs/active/agents/2026-04-15_orchestration-hygiene-and-artifact-monitor.md`, `docs/active/workflow/handoffs/recent-activity.md`

## Current Findings

- `CURRENT_FOCUS.md` and `recent-activity.md` had drifted behind the current operational pass and needed restart-surface refresh.
- `docs/active/workflow/handoffs/README.md` under-indexed its own dated notes.
- `docs/workflow/README.md` under-indexed durable root assets such as the run-policy file and schema drafts.
- `docs/active/agents/readme.md` had a stale `open_questions.md` path.
- Root `README.md` had a malformed known-limitations link.
- `syn_parser` has at least one real doc mismatch:
  - README/module docs describe discovery as `src/`-only
  - implementation also conditionally scans `tests/`, `examples/`, and `benches`
- README coverage across crate roots is uneven and should be tracked rather than “fixed everywhere” blindly.

## Stable Folder TOC Policy

For documentation folders, prefer one canonical `README.md` per durable folder root with:

1. a one-line purpose statement
2. a flat list of child files/subdirectories worth retrieving
3. one short retrieval-oriented description per entry
4. an authority or start-here rule when the folder has both live and durable surfaces

Guidance:

- Apply this policy to durable doc trees and active planning trees, not to every source-code directory.
- Do not create README churn for tiny or generated directories.
- If a file or subdirectory is important enough to preserve across restart, it should be indexed from the nearest durable folder README.
- If an indexed file becomes stale but must remain, keep it listed and mark its role clearly instead of letting it disappear from discovery.

## Candidate Unattended Docs

- [2026-04-09-doc-review-followups.md](/home/brasides/code/ploke/docs/active/workflow/handoffs/2026-04-09-doc-review-followups.md)
  Short closure note from the doc-review passes. Mostly historical now, but it still explains why some workflow indices have their current shape.
- [2026-04-09_run-record-design-handoff.md](/home/brasides/code/ploke/docs/active/workflow/handoffs/2026-04-09_run-record-design-handoff.md)
  Early run-record and manifest split design note with time-travel assumptions. Useful lineage, but largely superseded by later persistence work.
- [2026-04-10_conversation-capture-design.md](/home/brasides/code/ploke/docs/active/workflow/handoffs/2026-04-10_conversation-capture-design.md)
  Design note for capturing LLM traffic via events instead of reconstructing from UI state. Still useful rationale even though the implementation moved on.
- [2026-04-10_phase-1d-structured-llm-capture.md](/home/brasides/code/ploke/docs/active/workflow/handoffs/2026-04-10_phase-1d-structured-llm-capture.md)
  Records the move from debug-string capture toward typed response capture. Historical, but still a good breadcrumb for later review.
- [2026-04-11_dual-syn-implementation-handoff.md](/home/brasides/code/ploke/docs/active/workflow/handoffs/2026-04-11_dual-syn-implementation-handoff.md)
  Captures the Rust 2015 `syn` compatibility fix and the first introspection slice. Relevant lineage, no longer live state.
- [2025-12-19_embeds-restore-regression.md](/home/brasides/code/ploke/docs/active/issues/2025-12-19_embeds-restore-regression.md)
  Specific restore-path regression note where the first load missed the populated embedding set. Worth keeping as a regression reference.
- [2025-12-30_tool-failure.md](/home/brasides/code/ploke/docs/active/issues/2025-12-30_tool-failure.md)
  Narrow compiler/indexing failure report with a log pointer. Low-churn symptom record that should stay discoverable for later review.
- [2026-03-19_backup-dbs.md](/home/brasides/code/ploke/docs/active/todo/2026-03-19_backup-dbs.md)
  Broad backup-fixture policy and helper-planning note. Parts of it are in motion now, but it still frames the fixture hygiene problem space.
- [2026-03-20_db-rag-workspace-survey.md](/home/brasides/code/ploke/docs/active/reports/2026-03-20_db-rag-workspace-survey.md)
  Survey of workspace-level ingestion and search gaps. Useful context for the upcoming shift toward targeted tool-system changes.

## Tracking Schema

Recommended per-finding record shape for future reporting:

```text
scope:
audited_at:
surface_type: [README | module-doc | crate-doc | folder-index]
status: [current | stale | broken-link | missing | needs-review]
claim:
evidence:
unsupported_claims:
not_checked:
risk:
recommended_followup:
owner_or_path:
```
