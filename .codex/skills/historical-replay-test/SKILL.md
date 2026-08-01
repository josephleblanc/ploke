---
name: historical-replay-test
description: Use this skill when adding, reviewing, or repairing a historical replay test for ploke-eval, Prototype 1, headless TUI, tool-call, protocol, or edit-harness behavior. A historical replay test must replay real persisted artifacts through production replay/tool/session code, not replace the incident with a synthetic projection.
---

# Historical Replay Test

Use this skill when the requested regression is a historical replay, or when a
live run incident needs replay coverage before the loop resumes.

## Contract

A historical replay test reconstructs the same evidence class that failed in a
real run and drives it through the current production path.

Required properties:

- Start from real persisted artifacts: run manifest, transition journal,
  agent-turn trace, headless TUI result, sidecar, request, response tape, or
  other durable run evidence.
- Preserve provenance in the test: campaign/run path, target repo or fixture,
  recorded call ids, request ids where useful, and the artifact names consumed.
- Replay exact recorded tool requests, phase transitions, protocol requests, or
  provider responses through the same production entrypoints the run used.
- Assert structured outcomes: typed records, statuses, proposal state, tool
  events, database rows, snippets, transition state, or persisted artifacts.
- Fail if the historical symptom reappears, not merely if a synthetic helper
  example behaves differently.

Synthetic unit tests are allowed as supplements, but they are not the historical
replay. Do not rename a synthetic projection to make it sound historical.

## Anti-Patterns

- Do not use string-based tracing/log calls as test evidence.
- Do not replace recorded tool requests with manually invented arguments unless
  the test explicitly says it is a synthetic supplement.
- Do not assert on a stale or loosely related symptom when the recorded failure
  can be replayed directly.
- Do not patch old bug reports as if they were mutable source files. Append
  findings or add a new report instead.
- Do not make invalid persisted run evidence acceptable just so an old run can
  pass. A replay should either prove the production reader now handles valid
  evidence correctly or prove invalid evidence is rejected clearly.

## Workflow

1. Identify the failed contract and the exact historical artifacts that carry
   the evidence.
2. Load those artifacts in the test, using existing typed record structures when
   available.
3. Recreate only the environment needed to run the production path: checkout,
   runtime DB, loaded workspace, response tape, event bus, or protocol context.
4. Drive recorded requests or transitions through production code. Avoid direct
   result injection unless the historical surface itself is a result reader.
5. Wait on the same completion barrier the operator relies on: tool-completed
   events, transition journal updates, closure status, or persisted run state.
6. Assert typed invariants that represent the historical failure and the desired
   recovery.
7. Run the replay test once failing when practical, then run it passing after the
   fix. Record the exact command in the handoff.

## Acceptance Check

Before calling a test a historical replay, answer yes to all of these:

- Does the test name and body point at the real incident artifacts?
- Would changing the recorded artifact path, call id, or request payload change
  what the test covers?
- Does the test exercise the same production code path that failed in the run?
- Would the original incident have failed this test before the fix?
- Are assertions based on structured state rather than tracing text?
