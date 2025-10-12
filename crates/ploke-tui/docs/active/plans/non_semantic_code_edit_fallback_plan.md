Title: Non-Semantic Code Edit Fallback — Plan and Progress

Purpose
- Provide a robust fallback path for malformed Rust files by adding:
  - A typed Read File tool (no parser dependency) to fetch full file contents safely.
  - A typed non-semantic Splice Edit tool to edit by byte ranges with verified hashes and IoManager staging.

Scope
- Crates: ploke-core, ploke-tui (tools, LLM wiring, tests). IoManager is reused (no changes planned initially).

Design Constraints
- Strong typing (Serialize/Deserialize, numeric fields numeric types).
- Static dispatch (Tool trait), zero-copy borrowed params where possible.
- Safety-first edits: stage proposals; all writes via IoManager; hash verification.
- Evidence-based: targeted tests per phase; summarize results.

Phases
1) Core types
   - Add `rag_types::ReadFileResult` in ploke-core with fields:
     - `ok: bool`, `file_path: String`, `exists: bool`, `byte_len: u64`, `is_binary: bool`,
       `encoding: "utf8"|"base64"`, `content: String`.
   - Tests: serde round-trip for utf8 and base64; numeric types preserved.

2) Tool: ReadFile (non-semantic, robust)
   - New module `ploke-tui/src/tools/read_file.rs` implementing Tool.
   - Params: `{ file_path: Cow<'a, str>, expected_file_hash: Option<TrackingHash> }`.
   - Schema: `file_path` required; optional `expected_file_hash`.
   - Execute: resolve path, verified read if hash present; utf8 vs base64 selection; emit `ToolCallCompleted` with `ReadFileResult` JSON.
   - Wire-up: register in `tools/mod.rs`, add dispatch arm, expose in `llm/manager` tool list.
   - Tests: schema shape; params parse; execution happy-paths (utf8/binary, hash ok/bad); path scoping; tool loop signal.

3) Tool: ApplySpliceEdit (non-semantic, robust)
   - New module `ploke-tui/src/tools/splice_edit.rs` implementing Tool.
   - Params: `{ file_path, expected_file_hash, start_byte: u32, end_byte: u32, replacement, namespace? }`.
   - Schema: required fields typed; numeric fields numeric.
   - Execute: resolve; build `ApplyCodeEditRequest` with `Edit::Splice`; call legacy staging path; return `ApplyCodeEditResult`.
   - Wire-up: register + dispatch + tool list exposure.
   - Tests: schema, params parse, staging success, range/hash/path errors; tool loop signal.

4) LLM exposure & prompting
   - Ensure both tools are included when `crate_focus` is set; keep `ToolChoice::Auto`.
   - Tests: builder tests include both tools; omit when no crate loaded.

5) Fallback workflow (offline E2E)
   - Document LLM sequence: get_file_metadata → read_file → compute splice ranges → apply_splice_edit → approve.
   - Tests: harness scenario with malformed file; verify proposal + optional auto-approval + DB rescan queued.

6) Safety & limits
   - Read limit: truncate large content and flag `truncated: bool` (if needed in result).
   - Splice guard: reject overlarge replacements to avoid runaway edits.
   - Tests: truncation and guard conditions.

7) Observability & docs
   - Add DEBUG_TOOLS tracing; brief docs under docs/active/reports.

Live tests (gated)
- Optional: add `#[cfg(all(feature = "test_harness", feature = "live_api_tests"))]` cases to validate tool_calls and applied deltas. Treat unsupported endpoints as not validated.

Progress Log
- 2025-10-12: Phase 1 — Core types: COMPLETED
  - Added `ReadFileResult` + `ReadEncoding` to `crates/ploke-core/src/rag_types.rs`.
  - Tests: 2 unit tests passed (serde round-trip utf8/base64). Summary: pass=2, fail=0, ignored=0.
  - Next: Phase 2 — implement ReadFile tool in ploke-tui and wire into LLM tools list; add unit + dispatch tests.
