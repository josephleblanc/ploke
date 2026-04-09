# Hypothesis Registry

- last_updated: 2026-04-09
- source: [eval-design.md](/home/brasides/code/ploke/docs/active/plans/evals/eval-design.md)
- owning_branch: `refactor/tool-calls`
- review_cadence: daily at 3:00 p.m. America/Los_Angeles local time
- update_trigger: update after formal runs, hypothesis status changes, or new blocking assumptions
- id_conventions: [docs/workflow/id-conventions.md](/home/brasides/code/ploke/docs/workflow/id-conventions.md)

| id | type | statement | metrics | status | next action |
|---|---|---|---|---|---|
| `H0` | primary | Structured-code tools improve solve rate with neutral or better efficiency. | `solve_rate`, `token_cost`, `wall_time` | active | Do not interpret formally until `A1`-`A4` are within guardrails. |
| `A1` | supporting | Tool interfaces are understandable and recoverable enough for fair evaluation. | `tool_misuse_rate`, `recovery_rate`, `abandon_rate` | active | Use EDR-driven A/B tests on tool recovery and description changes. |
| `A2` | supporting | The structured representation is accurate and fresh enough to support correct lookup and navigation. | `parse_coverage`, `node_accuracy`, `query_recall`, `staleness_rate` | active | Expand probe suite and use replayable lookup checks. |
| `A3` | supporting | Provider and runtime behavior do not dominate outcomes. | `request_failure_rate`, `retry_success_rate`, `provider_variance` | active | Separate provider failures from model failures in manifests and ledger updates. |
| `A4` | measurement | The harness records and grades runs correctly. | `false_negative_rate`, `false_positive_rate`, `data_completeness`, `triage_accuracy` | active | Converge split artifacts into a stronger run manifest and typed outcomes. |
| `A5` | measurement | Replay and introspection answer run questions without re-running the full eval. | `replay_success_rate`, `lookup_answerability`, `postmortem_latency` | proposed | Implement the narrowest useful replay and lookup APIs first. |
