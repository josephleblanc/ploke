# S1E - Setup Phase Test Cleanup

- Date: 2026-04-12
- Owner role: worker
- Layer/workstream: A5
- Related hypothesis: `ploke-eval` tests are easier to trust and maintain when setup/integration assertions avoid large local helper paths that mirror production-adjacent behavior
- Design intent: Follow the accepted `S1B` cleanup findings with one more narrow test-shape cleanup pass inside `ploke-eval`
- Scope: Tighten `crates/ploke-eval/tests/setup_phase_integration.rs` if it contains duplicated setup-construction logic or other local helper structure that weakens signal or maintainability
- Non-goals: Do not redesign the eval runner, do not modify production code outside `crates/ploke-eval/`, do not reopen accepted replay/query/runtime behavior without new evidence
- Owned files: `crates/ploke-eval/tests/**`, related sprint docs as needed
- Dependencies: accepted `S1B`, accepted `P0A`-`P0F`
- Acceptance criteria:
  1. The packet identifies a bounded test-shape cleanup slice in `setup_phase_integration.rs` or explicitly concludes that the suspected duplication should remain.
  2. The output distinguishes cleanup of test construction/helpers from behavior changes.
  3. The output leaves behind either a cleaned-up test file with targeted evidence or a narrow rationale for not changing it.
- Required evidence:
  - targeted diff summary or no-change rationale
  - direct file references to the cleaned-up or retained helper path
  - targeted test command/result if the file changes
- Report-back location: `docs/active/agents/2026-04-12_eval-infra-sprint/`
- Status: accepted

## Permission Gate

No additional permission required if work stays inside `crates/ploke-eval/` and docs.
