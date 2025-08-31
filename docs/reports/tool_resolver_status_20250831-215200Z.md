Tool Resolver Status Report (2025-08-31)

Summary
- Implemented strict helper fix using `file_owner_for_module` and added relaxed resolver.
- Integrated strict→fallback in `apply_code_edit` tool with path normalization when user provides relative `file`.

Test Evidence
- ploke-db: helpers::tests::test_resolve_nodes_by_canon_in_file_via_paths_from_id → PASS (1/1)
- ploke-db: helpers::tests::test_relaxed_fallback_when_file_mismatch → PASS (1/1)
- ploke-tui: e2e_apply_code_edit_canonical_on_fixture → PASS (1/1)

Artifacts
- DB helper artifact: crates/ploke-db/tests/ai_temp_data/test-output.txt

Notes
- Canon parsing bug fixed (avoid duplicate leading `crate`).
- Live API tests remain pending (feature-gated).
