# Orchestration Hygiene And Artifact Monitor

- date: 2026-04-15
- task title: orchestration hygiene and artifact monitor
- task description: durable operating note for the current long-running pass over doc hygiene, restart cleanup, testing/documentation audit, and protocol-artifact coverage generation without new `ploke-protocol` development
- related planning files: `docs/active/CURRENT_FOCUS.md`, `docs/active/workflow/handoffs/recent-activity.md`, `docs/active/agents/2026-04-15_ploke-protocol-control-note.md`

## Operating Constraints

- Belay new `ploke-protocol` development for this pass.
- Prefer no coding in general.
- If an obvious code issue appears, have a sub-agent investigate and write a bug report instead of editing production code.
- `docs/workflow/evalnomicon/` is off-limits for edits during this pass, except that its current state may later be committed unchanged if needed for clean git state.
- `syn_parser` is explorer-only for this pass: inspect but do not edit.
- Testing review is tracking/reporting only: no test edits.

## Main Workstreams

1. General docs overview and hygiene with a light touch.
2. Restart-critical doc cleanup and broken-link/index cleanup.
3. Create a tracking surface for stale or unattended docs that should not be deleted yet.
4. Analyze doc comment and README coverage, with emphasis on:
   - incorrect doc comments
   - crate-central READMEs
   - consistency reporting rather than edits
5. Establish a stable table-of-contents policy for documentation folders.
6. Audit testing surfaces for later review, especially:
   - trivial tests
   - ignored and stale tests
   - obviously incorrect tests
   - tests that may conflict with backup fixture DB policy
7. Treat protocol-artifact coverage generation as the highest-priority operational lane:
   - use existing `ploke-eval` CLI methods where possible
   - prefer loops or orchestration outside code changes
   - if unattended looping fails, use attended sub-agent execution
   - target full coverage across finished eval runs
8. After at least half coverage, run sanity-check review passes comparing:
   - `ploke-eval inspect tool-calls` and related inspect surfaces
   - persisted `ploke-protocol` artifacts
   - sub-agent qualitative assessment of sampled records

## Expected Outputs

- restart-safe doc updates and indexes
- a stale/unattended-doc tracking note for later user review
- doc comment / README coverage reports
- testing audit report for later user review
- protocol-artifact coverage status and generated artifacts
- sampled sanity-check reports on protocol-output alignment

## Orchestration Notes

- Keep a light touch and favor reports over edits.
- Use polling/sleeping to keep long-running monitoring active.
- Maintain continuity in docs so the user can re-enter without chat history.
