# 2026-05-12 Agent-Turn Record Projection Handoff

Current restart spine for the `agent-turn` persisted-schema ownership cleanup across `ploke-records`, `ploke-eval`, and `ploke-tree`.

Related planning/context:

- `docs/active/agents/2026-05-09_records-emission-clean-sweep-handoff.md`
- `docs/active/plans/self-improvement-loop/typed-persistence-spine/operating-console.md`
- `docs/active/plans/self-improvement-loop/typed-persistence-spine/implementation-slices.md`

## Why this handoff exists

The prior plan for tool and agent-turn persistence assumed the live writer
boundary was in `ploke-tui`. In fresh context, the actual `agent-turn-trace.json`
and `agent-turn-summary.json` writer is `ploke-eval::runner`. This handoff
captures the implemented ownership boundary so future work does not keep trying
to solve this by expanding the feature-gated `ploke-records <-tool_contracts-> ploke-tui`
bridge.

## Implemented boundary

- `ploke-records::record::ToRecord` now exists as a small structural projection
  trait. It does not perform I/O or choose write locations.
- `ploke-records::agent_turn` now owns the persisted `agent-turn` schema for:
  - tool UI payloads
  - tool error wire payloads
  - retry-context payloads
  - the existing trace/summary artifact envelope
- `ploke-eval::runner` remains the live authority that assembles turn artifacts,
  but it no longer writes the live `AgentTurnArtifact` directly.
- `ploke-eval::runner` now projects the live artifact into:
  - `ploke_records::agent_turn::AgentTurnTraceRecord`
  - `ploke_records::agent_turn::AgentTurnSummaryRecord`
- `ploke-tree` continues to deserialize the records-owned shape without behavior
  changes.

## Key files

- `crates/ploke-records/src/record.rs`
- `crates/ploke-records/src/agent_turn.rs`
- `crates/ploke-eval/src/runner.rs`

## Current model

- Live tool/session/runtime mutation remains in `ploke-tui` and `ploke-eval`.
- Persisted passive `agent-turn` schema ownership is in `ploke-records`.
- The conversion boundary for `agent-turn-*` files is currently the eval runner,
  not the TUI.
- The existing `tool-contracts` feature remains interim debt for persisted tool
  argument decoding and other replay surfaces. This slice did not remove that
  bridge.

## Constraints learned during implementation

- Do not try to add `impl ToRecord for ploke_tui::...` inside `ploke-tui` unless
  the crate graph changes first. `ploke-records` currently depends on
  `ploke-tui` behind the `tool-contracts` feature, so reversing that direction
  would create a cycle.
- For `agent-turn` persistence specifically, the practical projection boundary is
  the live eval artifact in `crates/ploke-eval/src/runner.rs`.
- Handwritten top-level projection code is intentional here. It keeps field drift
  explicit at the persisted boundary and avoids hiding schema motion behind a
  field-copy macro.

## Verification run in this slice

- `cargo fmt --all`
- `cargo test -p ploke-records --features tool-contracts tool_ui_payload_record_roundtrips_current_agent_turn_shape`
- `cargo test -p ploke-eval agent_turn_projection_converts_live_tool_payload_to_record_schema`
- `cargo test -p ploke-eval drain_post_terminal_events_captures_late_llm_response`
- `cargo test -p ploke-tree load_record_set_loads_agent_turn_trace_and_summary_evidence`

## Next likely work

1. Audit other persisted surfaces that still embed `ploke_tui::tools::ToolUiPayload`
   or similar live DTOs and decide whether they are true persisted-schema
   ownership problems or merely test/local replay conveniences.
2. Narrow the remaining `tool-contracts` bridge so `ploke-records` only owns
   passive persisted or replay-facing shapes, not high-churn live tool surfaces.
3. If future work wants `ploke-tui`-side `ToRecord` impls, first restructure the
   crate graph instead of adding another feature-gated cycle workaround.

## Operational rule

When touching `agent-turn-trace.json`, `agent-turn-summary.json`, tool UI/error
payload persistence, or related replay readers, treat this handoff as the
current restart packet before extending the boundary.
