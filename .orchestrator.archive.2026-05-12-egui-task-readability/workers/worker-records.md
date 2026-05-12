# Worker Packet: worker-records

- role: Worker

## Active Task

### records-rag-retry-context-cleanup: Replace RAG tool retry-context JSON with typed ToolRetryContext

- lane: records
- priority: 1
- state: AssignedActive { worker: "worker-records" }
- allowed_edit:
  - crates/ploke-tui/src/rag/tools.rs
  - crates/ploke-tui/src/tools/error.rs
  - crates/ploke-records/src/tool_contracts.rs
- forbidden_edit:
  - crates/ploke-eval
  - crates/ploke-tree
  - crates/ploke-egui
- docs:
  - docs/active/agents/ploke-tree-graph-ingestion/reviewer-graph-tool-ui-contract-2026-05-11.md
- acceptance:
  - RAG retry-context callsites use ToolRetryContext instead of serde_json::json!
  - Temporary From<serde_json::Value> retry-context bridge is removed if no longer needed
  - No ploke-eval edits

## Queued Tasks

No queued tasks.

## Report Contract

- Report changed files or `none`.
- Report verification commands and outcomes.
- Report blockers with exact file paths or evidence.
- Do not edit outside allowed surfaces.
- Do not touch forbidden files.
