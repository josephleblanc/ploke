# Active Agent Docs

Restart-oriented table of contents for `docs/active/agents/` and nearby restart-critical surfaces.

## Start Here

- [../CURRENT_FOCUS.md](../CURRENT_FOCUS.md)
  Primary restart pointer for the current eval/protocol thread.
- [../workflow/handoffs/recent-activity.md](../workflow/handoffs/recent-activity.md)
  Rolling workflow board with the freshest state changes and restart consequences.
- [../workflow/handoffs/2026-04-17_protocol-design-reset.md](../workflow/handoffs/2026-04-17_protocol-design-reset.md)
  Compact restart handoff for the protocol frontier, scheduler cost model, and design pivot.

## Current Planning Surfaces

- [2026-04-17_eval-failure-and-protocol-audit/README.md](./2026-04-17_eval-failure-and-protocol-audit/README.md)
  Control-plane docs for failed-run audit, known-limitations reconciliation, blind trace review, and protocol-output comparison.
- [2026-04-16_eval-closure-formal-sketch.md](./2026-04-16_eval-closure-formal-sketch.md)
  Active planning note for layered registry/eval/protocol closure.
- [2026-04-15_ploke-protocol-control-note.md](./2026-04-15_ploke-protocol-control-note.md)
  Authoritative checkpoint for the active `ploke-protocol` architecture thread.
- [2026-04-12_eval-orchestration-protocol/2026-04-12_eval-orchestration-protocol.md](./2026-04-12_eval-orchestration-protocol/2026-04-12_eval-orchestration-protocol.md)
  Execution contract for eval-control-plane work unless explicitly superseded.

## Supporting Context

- [2026-04-15_protocol-aggregate-cli.md](./2026-04-15_protocol-aggregate-cli.md)
  Aggregate inspection surface for protocol artifacts and coverage.
- [2026-04-15_clap-baseline-eval-orchestration.md](./2026-04-15_clap-baseline-eval-orchestration.md)
  Batch/operator context for the Clap-heavy Rust baseline work.
- [2026-04-12_eval-infra-sprint/README.md](./2026-04-12_eval-infra-sprint/README.md)
  Legacy sprint archive index; keep for lineage, not as the primary restart surface.

## Conventions

- When creating a new active agent document, use the current date (`yyyy-mm-dd`) at the beginning of the file or directory name.
- In the document header include:
  - date
  - task title
  - task description
  - related planning files
- Archived one-off agent docs live under `docs/archive/agents/YYYY-MM/`.
- `open-questions.md` is for agent-to-agent open questions, not direct user prompts.
- `notable-inconsistencies.md` is for durable inconsistencies worth preserving without forcing immediate action.
