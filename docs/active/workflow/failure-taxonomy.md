# Failure Taxonomy

- last_updated: 2026-04-09
- source section: `§VI` of [eval-design.md](../plans/evals/eval-design.md)
- owning_branch: `refactor/tool-calls`
- review_cadence: every 2 days at 3:00 p.m. America/Los_Angeles local time
- update_trigger: update after repeated misclassification, a new canonical example, or a confirmed missing category

## Scope Rule

Keep the taxonomy tight. Add a new top-level category only when repeated failures do not fit an existing category without losing important meaning.

## Categories

| code | use when | canonical example |
|---|---|---|
| `SETUP_ENVIRONMENT` | The run fails before meaningful agent work because checkout, indexing, snapshot load, or benchmark environment setup is broken. | Cached snapshot missing or indexing times out before turn 1. |
| `MODEL_STRATEGY` | The model plans poorly, chases the wrong diagnosis, loops, or terminates early despite adequate tools and data. | The agent inspects the right files but edits an unrelated branch of the bug. |
| `TOOL_DISCOVERY` | The right tool exists but the model fails to find or choose it. | The agent keeps shell-grepping for a symbol that the structured lookup tool could resolve directly. |
| `TOOL_SEMANTICS` | The tool contract or response shape misleads the model. | A lookup result is technically valid but omits the distinction the agent needs to choose the right item. |
| `TOOL_RECOVERY` | The initial tool failure was recoverable but the recovery path is too weak or too confusing. | A lookup miss returns an opaque error instead of actionable next steps. |
| `INDEX_FIDELITY` | The structured representation is absent, stale, misparsed, or mislinked. | A function exists in source but not in the DB snapshot queried during the run. |
| `QUERY_QUALITY` | The data is present, but the request formulation is weak or too ambiguous. | The agent asks for an overly broad symbol name and ignores disambiguation clues already available. |
| `RUNTIME_INFRA` | Provider, network, timeout, or transport behavior invalidates or aborts the run after setup. | The model request aborts after repeated provider errors mid-run. |
| `EVAL_HARNESS` | Runner, grader, or artifact generation behavior is wrong or misleading. | The patch is correct but the harness marks it failed, or the final assistant message is missing from preserved artifacts. |
| `PATCH_SYNTHESIS` | The agent localizes the problem correctly but writes the wrong change. | The patch touches the correct file and function but introduces a logically incorrect condition. |
| `VALIDATION_GAP` | The visible local signal looks good, but broader validation still reveals an incomplete fix. | The patch passes visible checks but fails hidden tests because an edge case remains unfixed. |

## Classification Rules

- Pick one primary category and at most two secondary categories.
- Prefer the earliest blocking cause, not the loudest downstream symptom.
- Use postmortem notes for subtypes; do not create category sprawl for every recurring edge case.
- Keep raw case detail in postmortems; keep only stable canonical examples and classification rules here.
