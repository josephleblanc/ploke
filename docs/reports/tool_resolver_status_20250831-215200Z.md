Tool Resolver Status Report (2025-08-31)

Summary
- Implemented strict helper fix using `file_owner_for_module` and added relaxed resolver.
- Integrated strict→fallback in `apply_code_edit` tool with path normalization when user provides relative `file`.

Test Evidence
- ploke-db: helpers::tests::test_resolve_nodes_by_canon_in_file_via_paths_from_id → PASS (1/1)
- ploke-db: helpers::tests::test_relaxed_fallback_when_file_mismatch → PASS (1/1)
- ploke-tui: e2e_apply_code_edit_canonical_on_fixture → PASS (1/1)
- ploke-tui (live): live_tool_call_request_code_context → PASS (status OK), Not Validated (no tool_calls observed)

Artifacts
- DB helper artifact: crates/ploke-db/tests/ai_temp_data/test-output.txt
- Live: crates/ploke-tui/ai_temp_data/live/* (request.json, response.json, optional no_tool_calls.txt)

Notes
- Canon parsing bug fixed (avoid duplicate leading `crate`).
- Live API path exercised, but tool_calls were not observed under current model/provider. Next iteration should:
  - Select a tool-capable model/provider pair explicitly.
  - Set `tool_choice` to function-required.
  - Verify body contains `tool_calls` or function object; otherwise mark as not validated.
