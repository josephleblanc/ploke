# P0E - Replay Query Surface

- Date: 2026-04-12
- Owner role: worker
- Layer/workstream: A5
- Related hypothesis: A5 replay/introspection readiness requires arbitrary query replay against a turn snapshot for postmortem and tool-design work
- Design intent: Provide `replay_query(turn, query)` as the minimal arbitrary-query replay surface after timestamped query support exists
- Scope: Implement `replay_query(turn, query)` and any minimal supporting record/introspection glue inside `crates/ploke-eval/`
- Non-goals: Do not broaden into full counterfactual replay, do not redesign manifest/config capture, do not change parser/tool behavior
- Owned files: `crates/ploke-eval/src/record.rs`, `crates/ploke-eval/tests/` as needed
- Dependencies: `P0C`
- Acceptance criteria:
  1. `RunRecord` or equivalent replay surface provides `replay_query(turn, query)`.
  2. The method executes against the historical turn snapshot rather than `NOW`.
  3. Evidence includes at least one targeted replay query over recorded run data or a focused equivalent fixture proving timestamp use.
- Required evidence:
  - targeted diff summary for replay/introspection code
  - named test command(s)
  - explicit note on query result shape and error behavior
  - explicit note on whether the method is raw-query oriented or wraps a narrower API
- Report-back location: `docs/active/agents/2026-04-12_eval-infra-sprint/`
- Status: proposed

## Permission Gate

No additional permission if implementation stays inside `crates/ploke-eval/`, but this packet depends on historical query support from `P0C`.
