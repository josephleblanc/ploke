# EDR-0001: Ripgrep 1294 Phase 2 Entry

- status: proposed
- date: 2026-04-12
- owning_branch: `refactor/tool-calls`
- review_cadence: review alongside active Phase 2 packet work
- update_trigger: update before execution and again after results are available
- owners: eval infra sprint
- hypothesis ids: H0, A3, A4
- related issues/prs: none yet
- linked manifest ids: none yet
- linked artifacts:
  - [P2B report](../../agents/2026-04-12_eval-infra-sprint/2026-04-12_P2B_report.md)
  - [P2C report](../../agents/2026-04-12_eval-infra-sprint/2026-04-12_P2C_report.md)
  - [P2D report](../../agents/2026-04-12_eval-infra-sprint/2026-04-12_P2D_report.md)
  - [P2E report](../../agents/2026-04-12_eval-infra-sprint/2026-04-12_P2E_report.md)
  - [exp-001 config](../../agents/2026-04-12_eval-infra-sprint/2026-04-12_exp-001-ripgrep-1294-phase2-entry.config.json)

## Decision

Plan one narrow formal Phase 2 comparison on `BurntSushi__ripgrep-1294` using a single config/EDR pair, adopted validity guards, and explicit waivers for fields the current harness does not freeze yet.

## Why Now

`P2B` revalidated the former ripgrep mixed-edition sentinel through the current CLI path, `P2C` resolved the validity-guard policy ambiguity, `P2D` fixed the bounded provenance surface for formal runs, and `P2E` narrowed the first formal packet to one ripgrep instance plus one config/EDR pair. The remaining gap is no longer target fairness or workflow ambiguity; it is whether the current runner can express the intended control and treatment arms honestly enough to execute the packet.

## Control And Treatment

- control:
  shell-only planned control on `BurntSushi__ripgrep-1294`
- treatment:
  structured-current-policy planned treatment on the same instance
- frozen variables:
  same benchmark instance, same base SHA, same `moonshotai/kimi-k2.5` + `friendli` choice, same run budget, same bounded provenance surface, and the same adopted validity guards

## Acceptance Criteria

- primary:
  author an interpretable first formal packet whose control/treatment definition, guards, and waivers are explicit enough that execution would be fair once the runner supports the arm surface
- secondary:
  keep the packet narrow to one live target and one paired artifact set without broadening into multi-target scheduling
- validity guards:
  `max_provider_failure_rate <= 0.05`, `max_setup_failure_rate <= 0.05`, `require_full_telemetry = true`, `require_frozen_subset = true`

## Waivers

- `system_prompt_id`, `system_prompt_sha256`, `system_prompt_version`
- `tool_schema_version`
- `tool_allowlist`, `tool_overrides`
- `retry_policy_id`, `timeout_policy_id`
- observed `wall_clock_secs` as a binding metric
- `tool_implementation_version`, `container_image`, `rust_toolchain`, `seed`, `host_os`
- `dataset_version` as a runtime-frozen claim beyond the current source descriptor

These fields are waived for the first packet because current harness outputs do not freeze them as first-class runtime facts yet. They should remain explicit waivers rather than implied guarantees.

## Plan

1. Use the committed config as the intended runtime contract for the first formal packet.
2. Confirm or implement the minimal runner surface needed to express the control and treatment arms honestly.
3. Execute only after the arm surface is explicit and update this record with manifest IDs, metrics, and decision.

## Result

- outcome:
  not executed yet
- key metrics:
  none yet
- failure breakdown:
  none yet
- surprises:
  the remaining blocker is more precise than expected: current runner policy surface, not target readiness or validity-guard ambiguity

## Decision And Follow-Up

- adopt / reject / inconclusive:
  inconclusive pending `P2G`
- next action:
  complete the minimal runner/control-surface prerequisite in `crates/ploke-eval/` before executing this formal packet
