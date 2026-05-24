---
name: ploke-run-review
description: Use this skill when reviewing a `ploke-eval` or Prototype 1 run, especially when the user asks what actually happened in an eval loop, whether closure/protocol success reflects real model progress, whether tool calls were semantically useful, or when writing/updating durable run-review docs under `docs/active/agents/run-reviews`.
---

# Ploke Run Review

Use this skill to turn a `ploke-eval` run into an evidence-grounded narrative.
Do not stop at closure state, token totals, or protocol aggregate counts. Reconstruct
the agent trace enough to say whether completed tools actually found useful
information and whether the run advanced toward a patch or oracle result.

## Default Workflow

1. Resolve the exact campaign, instance, run root, and `record.json.gz`.
2. Read durable state first:
   - `closure status`
   - run profile and campaign manifest
   - per-run submission and patch projection
   - protocol overview/artifacts
3. Run the bundled trace audit if a run root is available:

   ```bash
   python3 docs/workflow/skills/ploke-run-review/scripts/run_trace_audit.py <run-root> --markdown
   ```

4. Compare three ledgers:
   - provider-emitted tool calls from `llm-full-responses.jsonl`
   - recorded tool lifecycle from `record.json.gz` and sidecars
   - semantic usefulness of returned payloads
5. Drill into suspicious calls before drawing conclusions.
6. Write or update the run review in `docs/active/agents/run-reviews/`.
7. Update `docs/active/agents/run-reviews/README.md` when adding a durable report.

## Required Distinctions

Keep these layers separate in the report:

- `mechanical completion`: closure/eval/protocol rows exist and finished
- `benchmark outcome`: patch, submission, oracle/MBE evidence, checkout state
- `provider trace`: raw model responses, finish reasons, usage, tool calls emitted
- `recorded tool lifecycle`: requested/completed/failed tools captured by the framework
- `information success`: whether the tool result actually contained useful evidence
- `protocol judgment`: what protocol adjudication saw and what it missed

Do not equate `ToolCompleted`, `ok:true`, or protocol `key_progress` with real
progress unless the returned content supports that claim.

## Trace Audit Checklist

For each reviewed run, check:

- provider call count versus recorded call count
- provider call IDs missing from the recorded lifecycle
- schema-invalid provider tool requests
- final response finish reason, content length, and completion tokens
- repeated full-file reads that are truncated
- completed reads with empty content
- fuzzy or invented search terms that returned broad context
- late useful context followed by no edit or no patch
- edit/tool success that means staged/proposed rather than applied
- protocol artifacts that over-credit empty or low-information results

When a tool result looks suspicious, verify against the target checkout with
direct `rg`, `sed`, `wc`, or git commands. For example, if a completed `read_file`
returns no content for a line range, check whether that range exists in the file.

## Report Shape

Use this order unless the user asks for a narrower answer:

1. short verdict
2. evidence roots
3. closure state
4. eval and patch output
5. oracle/MBE state
6. LLM and tool behavior
7. trace reconstruction
8. protocol review and protocol blind spots
9. what is working
10. what is not working yet
11. action items

The trace reconstruction should explain the chain of events, not just list
counts. Include turning points such as first useful localization, repeated
thrash, malformed provider calls, empty successful reads, and the last point
where the model had enough information to act.

## Output Rules

- Prefer concrete artifact-backed claims over model speculation.
- Use short path labels in prose and reserve full paths for evidence roots.
- Say when a result is mechanically complete but benchmark-useless.
- Say when protocol completed but was not a semantic success auditor.
- Keep action items tied to observed gaps in artifacts or tooling.
