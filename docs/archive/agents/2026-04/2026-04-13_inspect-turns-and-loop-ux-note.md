# 2026-04-13 Inspect Turns And Loop UX Note

- date: 2026-04-13
- task title: `ploke-eval inspect` turn-selection and loop-view UX
- task description: preserve the accepted CLI inspection workflow decisions from the collaborative design pass, including the implemented turn-selection improvements and the next bounded slice for a mid-level loop view
- related planning files: `docs/active/CURRENT_FOCUS.md`, `docs/active/workflow/handoffs/recent-activity.md`, `docs/active/agents/2026-04-12_eval-infra-sprint/2026-04-12_eval-infra-sprint-control-plane.md`

## Scope

This note captures the accepted design direction from the CLI-first inspection UX pass so the next restart can continue without depending on chat-window memory.

## Accepted Direction

- `inspect conversations` should function as the compact turn-selection surface.
- `inspect turns` is the clearer mental model and should remain available as an alias-compatible entry point.
- `inspect turn` is the focused drilldown surface for one selected turn.
- The CLI should follow a progressive ladder:
  1. coarse list
  2. select one item
  3. inspect a mid-level view
  4. inspect full detail only when needed
- Every inspect surface should advertise the next narrower command at the bottom.

## Implemented In This Pass

- `inspect conversations` now accepts `turns` as an alias.
- `inspect turn` now accepts positional turn syntax:
  - `ploke-eval inspect turn 1`
- The hidden legacy form still parses for compatibility:
  - `ploke-eval inspect turn --turn 1`
- `inspect turns` now renders a narrow table intended to stay within terminal-friendly width:
  - `Turn`
  - `Tools`
  - `Failed`
  - `Outcome`
- `inspect turn 1` now renders a compact dotted summary:

```text
Turn 1
  tools .............. 31
  failed tools ....... 5
  messages ........... 12
  patch proposed ..... yes
  patch applied ...... partial
```

- `inspect turn --show messages` now supports role filters:
  - `--roles assistant,tool`
  - `--exclude-roles system,user`

## Important Caveat Discovered

- `turn.messages()` is not the full agent-tool loop transcript.
- It currently reconstructs prompt/response context from:
  - `agent_turn_artifact.llm_prompt`
  - optional `agent_turn_artifact.llm_response`
- It does **not** currently synthesize message entries from tool execution records.
- Therefore:
  - `inspect turn 1 --show messages --exclude-roles system,user`
  - may legitimately return `[]` even when the turn had many tool calls
- This reflects the underlying record shape, not a bug in the new filter logic.

## Next Bounded Slice

Add a new turn drilldown surface:

- `ploke-eval inspect turn 1 --show loop`

Purpose:

- provide the missing middle layer between:
  - `inspect turn 1 --show tool-calls`
  - `inspect turn 1 --show tool-call --index N`

Intended shape:

- chronological per-call blocks
- more informative than the current compressed table
- much lighter than full payload detail

Example direction:

```text
[7] read_file
  input ....... file: .../grep/src/main.rs
  status ...... failed
  code ........ internal
  summary ..... no such file or directory

[8] request_code_context
  input ....... search_term: Arg::with_name("iglob")
  status ...... completed
  returned .... 10 snippets
  top score .... 0.031
  summary ..... Context assembled
```

Field policy:

- always show:
  - input
  - status
  - summary
- on failure, also show:
  - error code
  - one or two key diagnostics such as `field`, `expected`, or equivalent
- on success, show:
  - one or two tool-specific informative fields such as `returned`, `entries`, `files`, or score-like data when available
- avoid raw payload dumps in the default loop view

## Rationale

- `messages` should keep its narrower prompt/response meaning unless deliberately redefined.
- The desired “show me the agent-tool interaction” path is better served by a dedicated reconstructed surface than by overloading `messages`.
- This keeps the CLI strongly progressive:
  - `inspect turns`
  - `inspect turn 1`
  - `inspect turn 1 --show loop`
  - `inspect turn 1 --show tool-calls`
  - `inspect turn 1 --show tool-call --index N`

## Suggested Resume Point

Resume in `crates/ploke-eval/src/cli.rs` by:

1. extending `TurnShowOption` with `Loop`
2. implementing a mid-level renderer from `turn.tool_calls()`
3. printing next-step hints from the loop view toward:
   - `--show tool-call --index N`
   - `--show tool-result --index N`
4. validating the output on the latest completed run via `ploke-eval inspect`
