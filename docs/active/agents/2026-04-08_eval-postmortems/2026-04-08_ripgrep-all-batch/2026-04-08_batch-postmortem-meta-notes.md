# Ripgrep Batch Postmortem Meta Notes

- date: 2026-04-08
- task title: Batch Postmortem Meta Notes
- task description: Record improvements to the eval post-mortem template and identify missing or low-salience run information encountered while reviewing `ripgrep-all` batch runs.
- related planning files:
  - [2026-04-08_postmortem-plan.md](/home/brasides/code/ploke/docs/active/agents/2026-04-08_eval-postmortems/2026-04-08_postmortem-plan.md)
  - [2026-04-08_batch-postmortem-index.md](/home/brasides/code/ploke/docs/active/agents/2026-04-08_eval-postmortems/2026-04-08_ripgrep-all-batch/2026-04-08_batch-postmortem-index.md)
  - [2026-04-08_postmortem-template-review.md](/home/brasides/code/ploke/docs/active/agents/2026-04-08_eval-postmortems/2026-04-08_ripgrep-all-batch/2026-04-08_postmortem-template-review.md)

## Template Improvements

- The template should explicitly instruct authors to pull `base sha`, `instance id`, and `expected_patch_files` from `run.json`, not from memory or the stable log envelope.
- The template should distinguish `local ploke-eval outcome` from `official Multi-SWE-bench verdict` more sharply. One report treated local `Request summary: [success]` as an official benchmark success signal.
- The template should ask whether `agent-turn-trace.json` is known-good for this run or appears stale/partial, and require evidence for that claim.
- The template should require one short `artifact trust` section:
  - which artifact is authoritative
  - which artifacts are partial, stale, or missing
  - why
- The template should ask authors to anchor claims with line-linked log evidence when naming a first tool failure or first validation failure.
- The template should require a `report confidence limits` line when the report relies on partial artifacts or broad log search instead of a tight event slice.
- The template should discourage using codebase source-file links as evidence for run behavior unless those links are directly tied to a logged event or artifact field.
- The current batch also exposed a pre-turn failure mode that the template does not name explicitly: indexing can time out before any assistant turn summary exists. Reports need a dedicated way to say "no turn happened" without implying a model mistake.
- The current `run.json` structure is ambiguous if authors assume top-level identity fields are populated. For this batch, the canonical identity lives under `source.*`; the top-level `instance_id`, `org`, `repo`, and `number` fields are null.

## Missing Or Low-Salience Information

- `agent-turn-summary.json` still does not preserve the final assistant message content reliably enough for post-mortem reading; analysts have to fall back to the trace or the raw eval log.
- The batch-level artifacts do not yet expose incremental progress while the batch is still running, so monitoring currently depends on scanning per-instance run directories and the live eval log.
- There is no single clearly surfaced field for `first tool error`, `first recovered error`, `first test failure`, or `first compile failure`, so each report must reconstruct the timeline manually.
- The per-run artifacts do not make the mapping from run to official benchmark follow-through salient. Reports need an explicit place to link any downstream Multi-SWE-bench output if it exists.
- The summary artifact can show a completed run with patch metadata but still leave ambiguity about whether the target file actually changed in the final accepted patch, especially when `expected_file_changes` is empty or incomplete.
- The latest live eval log is noisy to query because it repeats the full benchmark prompt many times; a lower-noise run-event stream would make active monitoring and post-mortems much easier.
- Runs can now end in a third useful state besides `patch applied` and `patch absent`: the model aborts with an empty `fix_patch` submission artifact, which should be surfaced separately from a missing artifact.
- Transport-timeout aborts after many attempts are a distinct failure mode worth tagging, because they mix model churn with provider or network instability.
- Embedding/context-provider failures should be surfaced as first-class run events; in the current logs they show up only indirectly through later tool failures.
- For the timeout runs specifically, the only concrete artifacts were `run.json`, `repo-state.json`, and `indexing-failure.db`; a report template should call that artifact set out explicitly so readers do not expect a turn trace or submission file.
- This qwen/alibaba batch also showed a second class of contract mismatch: the model can still attempt unsupported tool names such as `run_shell_command`, so the report template should have a dedicated place to note invalid-tool attempts separately from code-level mistakes.
- For batch runs, the `provider` can be recovered from the batch summary even when `run.json.selected_provider` is null; that distinction should be called out so report authors do not assume the run manifest is fully populated.
