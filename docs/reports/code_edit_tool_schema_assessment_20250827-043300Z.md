Code Edit Tool Schema Assessment — 2025-08-27 04:33:00Z

Purpose
- Evaluate the current and proposed fields for the `apply_code_edit` tool, ensure agent-friendly and type-safe design, and identify helpful ploke-db helpers and patterns.

Summary of Findings
- Tests failing cause: the current tool handler only deserializes a canonical schema and rejects direct splice inputs used by tests. Staging never occurs → proposal not found; approve does not modify the file.
- Strong typing is mandatory: use tagged enums for alternate edit modes; avoid shape detection via optional string fields.
- ploke-db helper available: `ploke_db::helpers::resolve_nodes_by_canon_in_file` returns `EmbeddingData` with `file_path`, `file_hash`, `span`, `namespace` helpful for canonical edits.

Existing Inputs
- Canonical (current): `{ file, canon, node_type, code }` → DB resolves span and file hash.
- Direct splice (tests): `{ file_path, expected_file_hash, start_byte, end_byte, replacement, namespace? }`.

Minimum Required Fields (per mode)
- Splice mode (minimal but safe):
  - `file_path: PathBuf` — absolute or project-root relative.
  - `expected_file_hash: FileHash` — file-level byte identity (TrackingHash today; SeaHash future).
  - `start_byte: u32`, `end_byte: u32` — UTF-8 boundary validated; inclusive-exclusive byte offsets.
  - `replacement: String` — new text.
  - `namespace: Uuid` — project namespace (default to `PROJECT_NAMESPACE_UUID` if omitted).
  Rationale: Enough to validate the file hasn’t changed and apply a precise splice.

- Canonical mode (agent-friendly for semantic rewrites):
  - `file: PathBuf` — target file.
  - `canon: CanonPath` — `crate::module::Item` canonical path.
  - `node_type: NodeKind` — enum of supported relations (`function`, `struct`, …), consistent with DB schema.
  - `code: String` — full rewritten item content.
  Rationale: Lets the system locate and rewrite a code item robustly using DB spans.

Recommended Typed Request
```rust
#[derive(Serialize, Deserialize)]
pub struct ApplyCodeEditRequest {
    pub edits: Vec<Edit>,
    pub confidence: Option<f32>,
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "mode", rename_all = "snake_case")]
pub enum Edit {
    Canonical { file: PathBuf, canon: String, node_type: NodeKind, code: String },
    Splice { file_path: PathBuf, expected_file_hash: FileHash, start_byte: u32, end_byte: u32, replacement: String, namespace: Uuid },
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NodeKind { Function, Const, Enum, Impl, Import, Macro, Module, Static, Struct, Trait, TypeAlias, Union }
```

Migration Strategy
- Temporarily support legacy direct splice shape via `#[serde(untagged)]` companion enum to map into `Edit::Splice`, gated by a feature flag or deprecation log.
- Keep tests for both modes; add serde round-trip tests for the request/response.

ploke-db Surfaces and Patterns
- Helper: `resolve_nodes_by_canon_in_file(db, relation, file_path, module_path, item_name)` → returns `EmbeddingData` with `file_path`, `file_hash`, `span`, `namespace` suited for building `WriteSnippetData`.
- Observability APIs (existing, extend later): `record_tool_call_requested/done`; edit proposal lifecycle tables exist per docs and need wiring from TUI once we persist proposals.
- Pattern reuse: request_code_context tool uses typed args/results and persists tool lifecycle; adopt same for apply_code_edit and add retrieval_event persistence where relevant.

Recommendation
- Implement `ApplyCodeEditRequest` with a tagged `Edit` enum; update the tool handler to match on variants and build `WriteSnippetData` accordingly.
- Enforce strict validation and early erroring for missing/invalid fields; avoid stringly detection.
- Add serde tests for request/response and integration tests to cover both modes.

References
- AGENTIC_SYSTEM_PLAN.md (Type Safety Policy)
- crates/ploke-db/src/helpers.rs::resolve_nodes_by_canon_in_file
- crates/ploke-tui/src/rag/tools.rs (apply_code_edit_tool current implementation)
- docs/testing/TEST_GUIDELINES.md

