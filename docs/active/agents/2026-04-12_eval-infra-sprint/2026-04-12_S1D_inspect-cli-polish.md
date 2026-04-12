# S1D - Inspect CLI Polish

- Date: 2026-04-12
- Owner role: worker
- Layer/workstream: A5
- Related hypothesis: The inspect CLI becomes a dependable quick-touch eval surface when the common bootstrap paths are obvious and placeholder behavior is replaced with concrete or clearly-bounded output
- Design intent: Turn the accepted `S1C` audit into a narrow polish packet for the most obvious inspect CLI friction points
- Scope: Improve the inspect-oriented `ploke-eval` CLI in the smallest high-value ways identified by `S1C`, especially the empty `inspect turn --show messages` placeholder and bootstrap-path discoverability
- Non-goals: Do not redesign the CLI wholesale, do not broaden into new data-model work, do not reopen accepted replay/query behavior
- Owned files: `crates/ploke-eval/src/cli.rs`, `crates/ploke-eval/tests/**`, related sprint docs as needed
- Dependencies: accepted `S1C`, accepted `P0D/P0E/P0F`
- Acceptance criteria:
  1. The packet removes or replaces the most misleading placeholder behavior in the inspect CLI, or converts it into an explicit bounded limitation.
  2. The packet keeps bootstrap-path guidance discoverable for common inspection questions.
  3. The output includes direct evidence from the affected CLI path and notes any residual UX limitations.
- Required evidence:
  - targeted command or test evidence for the changed CLI path
  - concise diff summary tied to concrete file references
  - explicit note on residual discoverability or coverage gaps
- Report-back location: `docs/active/agents/2026-04-12_eval-infra-sprint/`
- Status: accepted

## Permission Gate

No additional permission required if work stays inside `crates/ploke-eval/` and docs.
