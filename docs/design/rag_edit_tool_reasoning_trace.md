# High-Level Reasoning Trace (Summary)

Purpose:
- Reduce LLM cognitive load and error rate by removing hashes and byte-offset fields from the apply_code_edit payload.
- Preserve safety/invariants via internal resolution to spans/hashes using the database at @ 'NOW'.

Key decisions:
- Shift to canonical-path rewrite format:
  - Input: { action, file, canon, node_type, code }
  - Server resolves span/hash via DB and uses IoManager to enforce content-lock and UTF-8 boundary checks.
- Keep file creation out-of-scope for now; edits only.
- “Rewrite the whole item” rather than “body-only edits” to avoid AST slicing complexity.
- Retain get_file_metadata tool for other flows that depend on file details, but stop requiring the LLM to use it for edits.
- Include @ 'NOW' in Cozo field access per existing patterns.

Risk/mitigation:
- Query fragility: Inline Cozo with string literals can break. Mitigate by centralizing into a DB helper and, longer term, parameterize queries.
- Ambiguity: If multiple nodes match, fail early and ask for clarification; recommend adding typed NodeType to disambiguate.

Why not anchors or UUIDs:
- Anchors are convenient but ambiguous under formatting changes; lack AST precision.
- UUIDs are precise but not ergonomic for LLMs and may not be stable across re-indexing.

Next steps:
- Introduce a typed NodeType (serde) in the request; eliminate string validation and allow-list.
- Implement Database::get_pnode_from_canon to remove inline query construction from the handler.
- Add tests for happy path and ambiguity/failure states; consider golden tests for diffs.
