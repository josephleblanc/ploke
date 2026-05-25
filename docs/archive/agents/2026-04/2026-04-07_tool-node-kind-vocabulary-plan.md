# Tool Node-Kind Vocabulary Plan

- date: 2026-04-07
- task title: Roll out shared tool node-kind vocabulary
- task description: Convert tool-facing node-kind strings into a shared typed vocabulary with generated schema and regression coverage.
- related planning files:
  - /home/brasides/code/ploke/docs/active/agents/2026-04-07_tool-call-failure-replay-plan.md

## Proposed Steps

1. Define a canonical tool-queryable node-kind source.
   - Prefer a shared type backed by existing `NodeType`/`ItemKind` semantics.
   - Include `method`.

2. Refactor tool schemas to use generated JSON Schema `enum`.
   - `code_item_lookup`
   - `code_item_edges`
   - `apply_code_edit`

3. Refactor runtime validation to parse into the shared type.
   - Remove duplicated string arrays where possible.

4. Add invariant tests.
   - schema enum matches accepted values
   - `method` is present
   - description/examples do not drift from the canonical set

5. Add a request-construction regression test.
   - Deserialize the replay fixture API request and assert the emitted tool schema includes `method`.

6. Consider a follow-up UX improvement for lookup mismatches.
   - Keep exact matching strict.
   - Improve error messages when a nearby candidate exists under another kind.

## Non-Goals

- Do not silently coerce `function` into `method`.
- Do not weaken exact-match semantics in the DB resolver.
- Do not broaden tool contracts without an explicit decision.
