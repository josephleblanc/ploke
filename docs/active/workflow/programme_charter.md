# Programme Charter

- last_updated: 2026-04-09
- source: [eval-design.md](../plans/evals/eval-design.md)
- owning_branch: `refactor/tool-calls`
- review_cadence: as-needed when the programme definition changes materially
- update_trigger: update when endpoints, decision rules, or workstream boundaries change

## Goal

Determine whether structured code representations improve coding-task performance relative to shell and text centric interaction.

## Primary Hypothesis

`H0`: agents with structured-code tools achieve higher benchmark success rates with lower or comparable token usage and wall-clock time.

## Endpoints

- primary:
  `solve_rate`
- secondary:
  `token_cost`
  `wall_time`

## Enabling And Measurement Hypotheses

- `A1` tool system effectiveness
- `A2` parsing and index fidelity
- `A3` provider and runtime robustness
- `A4` eval harness fidelity
- `A5` replay and introspection usefulness

## Decision Rule

- strong support:
  success improves and efficiency is neutral or better
- partial support:
  success improves and efficiency regresses
- weak or negative support:
  success does not improve
- evidence against:
  success regresses after validity gates are satisfied

## Operating Constraint

Do not spend significant effort on prompt, tool, or retrieval ablations while setup reliability, provider reliability, tool-contract correctness, or replay/observability still make results hard to interpret.
