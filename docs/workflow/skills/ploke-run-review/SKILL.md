---
name: run-review
description: Use this skill when reviewing a `ploke-eval` or Prototype 1 run, especially when the user asks what actually happened in an eval loop, whether closure/protocol success reflects real model progress, whether tool calls were semantically useful, or when writing/updating durable run-review docs under `docs/active/agents/run-reviews`.
---

# Ploke Run Review

Use this skill to turn a `ploke-eval` run into an evidence-grounded narrative.
Do not stop at closure state, token totals, or protocol aggregate counts. Reconstruct
the agent trace enough to say whether completed tools actually found useful
information and whether the run advanced toward a patch or oracle result.

## Default Workflow

1. Resolve the exact campaign, instance, run root, and `record.json.gz`.
2. Resolve the execution path that produced the artifact before borrowing any
   prior RCA:
   - exact CLI command or Prototype 1 state-machine phase
   - code entrypoint that ran the turn
   - event producer, event recorder, and read-side/protocol projection
3. Read durable state first:
   - `closure status`
   - run profile and campaign manifest
   - per-run submission and patch projection
   - protocol overview/artifacts
4. Check the runtime-playback inventory before claiming that records are
   missing. Use the inventory to distinguish absent records from records that
   exist but are not yet first-class playback steps.
5. Run the bundled trace audit if a run root is available:

   ```bash
   python3 docs/workflow/skills/ploke-run-review/scripts/run_trace_audit.py <run-root> --markdown
   ```

6. Compare three ledgers:
   - provider-emitted tool calls from `llm-full-responses.jsonl`
   - recorded tool lifecycle from `record.json.gz` and sidecars
   - semantic usefulness of returned payloads
7. Drill into suspicious calls before drawing conclusions.
8. Extract positive examples and candidate LLM-adjudication signals.
9. Classify action items as non-blockers or blockers. File or update alive bugs
   for non-blockers while the loop continues; blockers hand off to
   `ploke-blocker-repair-loop` before the campaign advances again.
10. Write or update the run review in `docs/active/agents/run-reviews/`.
11. Update `docs/active/agents/run-reviews/README.md` when adding a durable report.

## Record Inventory Before Missing-Record Claims

Before writing that evidence is missing, check the runtime-playback inventory:

- `docs/workflow/evalnomicon/drafts/observability/runtime-playback/inventory/README.md`
- `docs/workflow/evalnomicon/drafts/observability/runtime-playback/inventory/record-surface-map.md`
- `docs/workflow/evalnomicon/drafts/observability/runtime-playback/inventory/latest-run-emission-worksheet.md`

Use four separate labels in the review:

- `record absent`: the expected file or typed record was not written.
- `record present, playback gap`: the file exists, but current graph/playback
  does not expose it as an ordered typed iterator or drilldown family.
- `record present, manual join needed`: the file exists, but the reviewer had
  to join it by path, id, git state, or timestamp because the joined view is not
  available yet.
- `operator/convenience record`: the file helps discovery or reporting but is
  not an authority source.

For Prototype 1 and broad-harness reviews, explicitly check the relevant
families before concluding:

- campaign state: `campaign.json`, `closure-state.json`, `scheduler.json`,
  `prototype1/transition-journal.jsonl`
- node/runtime state: `nodes/<node>/node.json`, `runner-request.json`,
  `runner-result.json`, `invocations/*.json`, channel JSONL files
- broad-harness state: `messages/edit-harness-request/*.json`,
  `messages/edit-harness-result/*.json`, the attempt workspace, terminal
  record, submitted result, candidate commit, and checkout diff
- run-root evidence: `record.json.gz`, `agent-turn-trace.json`,
  `agent-turn-summary.json`, `llm-full-responses.jsonl`,
  `validation-audit.json`, `multi-swe-bench-submission.jsonl`,
  `benchmark-patch-projection.json`
- protocol evidence: `*_tool_call_intent_segmentation_*.json`,
  `*_tool_call_review_*.json`, and `*_tool_call_segment_review_*.json`

The default conclusion should not be "no records exist" unless those checks
prove absence. Prefer a precise claim such as "the record exists, but playback
currently summarizes it and the review had to manually join it to the attempt."

## Execution Path First

Before diagnosing a lifecycle or tool failure, prove which command path produced
the artifact. Do not transfer a prior fix or RCA from `tui_adapter`, `runner.rs`,
`record.rs`, or `session.rs` until the active path is confirmed.

Common split:

- `prototype1-harness attempt` / published broad headless-TUI requests use
  `tui_adapter::run_headless_with_model`.
- `loop prototype1-step` eval turns use the Prototype 1 state machine and
  `runner.rs::run_benchmark_turn`; later protocol/read-side code may consume
  the resulting `record.json.gz`.

In the report, name the active path compactly, for example:

```text
prototype1-step -> run_planned_child -> runner.rs::run_benchmark_turn -> record.rs -> protocol
```

For Prototype 1 child-plan broad-harness fanout, review each completed child
attempt independently when its artifact appears. Use the specific request slot,
workspace, branch, commit, submitted result, terminal record, and TUI trace for
that attempt. Do not collapse attempt `r1`, `r2`, and later retries into one
summary unless the user explicitly asks for a batch-level report. If an attempt
shows `TOOL_EXECUTION_FAILED` but still produces an applied commit, reconstruct
whether the failure was terminal, recoverable, or hidden by later artifact
accounting before classifying the child.

When invoked by an orchestrator, the review worker should not advance the loop
or mutate the run. Own the evidence report for the assigned attempt, update the
run-review index, and call out which findings should become alive bugs,
blocker-repair work, or future adjudication signals.

## Required Distinctions

Keep these layers separate in the report:

- `mechanical completion`: closure/eval/protocol rows exist and finished
- `benchmark outcome`: patch, submission, oracle/MBE evidence, checkout state
- `provider trace`: raw model responses, finish reasons, usage, tool calls emitted
- `recorded tool lifecycle`: requested/completed/failed tools captured by the framework
- `information success`: whether the tool result actually contained useful evidence
- `learning signal`: concrete evidence that the model used tool output to improve or recover
- `protocol judgment`: what protocol adjudication saw and what it missed

Do not equate `ToolCompleted`, `ok:true`, or protocol `key_progress` with real
progress unless the returned content supports that claim.

## Edit Lifecycle Reconstruction

For edit tools, build a per-`call_id` timeline from raw events before trusting
summaries:

```text
requested -> staged/pending -> applied/failed/stale/denied
```

Do not summarize an edit call from the first `ToolCallCompleted`. For staged
proposal tools, `ToolCallCompleted` can mean only `staged > 0, applied = 0`.
Keep these views separate:

- actual edit/apply outcome in the raw event stream and proposal artifact
- model-visible tool result or message update
- persisted run-record projection
- protocol/adjudication projection

When a final patch is suspicious, compare `patch_artifact` proposal statuses,
raw edit events, and the target checkout diff. If an edit applied to a file and
a later same-file edit failed with `Content changed`, check whether the later
edit resolved against stale file-hash or node-span metadata after the first
apply.

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
- same-call edit lifecycles where a staged `ToolCallCompleted` is superseded by
  a later applied/failed/stale/denied event
- same-file follow-up edits after an earlier applied edit, especially stale
  hash failures or missing post-apply refresh boundaries
- protocol artifacts that over-credit empty or low-information results
- model-visible tool output that caused a useful follow-up read, edit, or validation retry
- final validation commands that resolved to an unrelated or weak target
- missing checks that the final answer implicitly claimed, such as formatting or oracle checks

When a tool result looks suspicious, verify against the target checkout with
direct `rg`, `sed`, `wc`, or git commands. For example, if a completed `read_file`
returns no content for a line range, check whether that range exists in the file.

## Positive Examples And Adjudication Candidates

Do not only collect failures. Preserve high-signal examples that future
LLM-adjudication steps could promote into explicit rubric fields.

Look for compact chains like:

- `tool output -> model interpretation -> follow-up action -> later result`
- `useful context -> targeted edit -> validation feedback -> repaired patch`
- `tool failure -> model changes strategy -> successful alternate tool path`

For each strong example, record:

- event or call ids, if available
- the model-visible payload or short excerpt
- the next model statement that quotes, summarizes, or ignores it
- the follow-up action and later validation result
- whether this should become an adjudication field or just remain review evidence

Candidate adjudication signals include:

- cargo/test output was visible and materially used
- failed validation led to a targeted repair instead of repetition
- final validation was strong, weak, or missing
- context retrieval found the right code before the first edit
- protocol or audit summaries hid model-visible evidence
- a successful tool call was semantically low-information

## Report Shape

Use this order unless the user asks for a narrower answer:

1. short verdict
2. evidence roots
3. closure state
4. eval and patch output
5. oracle/MBE state
6. LLM and tool behavior
7. positive examples and adjudication candidates
8. trace reconstruction
9. protocol review and protocol blind spots
10. what is working
11. what is not working yet
12. action items

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
- When the review finds a blocker, name the broken contract and point to
  `ploke-blocker-repair-loop` instead of only listing another follow-up.
