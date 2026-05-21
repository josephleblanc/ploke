# Historical Replay Probe Workflow

Date: 2026-05-20

Status: intended workflow with the first CLI-backed replay slices implemented.
Verify against current `ploke-tree`, `ploke-eval::replay`, and
`ploke-tui::llm` code before treating any later command shape here as
implemented.

Implemented slice as of 2026-05-20: `ploke-eval run replay inspect` provides a
read-only projection over typed `agent-turn` artifacts and
`llm-full-responses.jsonl`. It lists replay cursors, compact tape continuity,
tool-event rows, and prompt/workspace path signals without starting the TUI loop
or calling a live provider. Replay anchor resolution also loads agent-turn
records directly, so it no longer requires `scheduler.json` beside the turn
artifacts.

Implemented slice added later on 2026-05-20: `ploke-eval run replay turn-live`
can now install a selected provider-response prefix rather than always using
the full assistant-message tape. `--through-response-index N` installs responses
`0..=N`; `--through-event` maps the selected `--event-index` through the typed
agent-turn artifact back to the provider response that produced the relevant
tool call; `--tail stop` runs the prefix through current tools without a live
provider call, while `--tail live` keeps the existing prefix-then-live behavior.

Implemented slice added on 2026-05-21: recorded-only stop mode now has clean
terminal semantics. `RecordedResponseTape` exhaustion is represented as
`LlmError::ReplayExhausted`, and the `ploke-tui` session treats it as an
intentional replay boundary rather than an invalid model response. The CLI
probe can therefore replay a historical prefix through current tools, stop
before the next provider call, and show the compact tool/request outcome
without misleading `INVALID_MODEL_RESPONSE` noise.

Implemented slice added later on 2026-05-21: table output now keeps every
returned tool result item visible by default while bounding each item. In
particular, `request_code_context` completions show every returned path,
canonical path, and a short snippet; `read_file` completions show file metadata
and a bounded excerpt. The renderer decodes tool payloads through
`ploke-records::tool_contracts` and uses full in-memory headless events for the
interactive table, while leaving the persisted JSON evidence summary bounded.

## Goal

Historical loop replay is not meant to prove that an old model run now succeeds.
It is meant to use an old nondeterministic run as deterministic scaffolding for
reconstructing suspicious loop states.

At a chosen breakpoint, current code should be in charge of the real tool loop:
search, code context, file reads, edit proposal state, patch application, path
policy, validation, and the next provider request. The question is whether the
agent has a plausible path forward from that reconstructed state, or whether our
tool surface, prompt, path handling, or feedback is making failure likely.

## Replay Boundary

Replay provider/model output into the normal `ploke-tui` session path. Do not
inject historical tool events by hand.

The intended replay surface is:

1. typed historical records, primarily `agent-turn-trace.json`,
   `agent-turn-summary.json`, and `llm-full-responses.jsonl`;
2. `RecordedResponseTape` installed into the TUI LLM manager;
3. real `run_chat_session` / tool-dispatch behavior;
4. real current workspace tools and proposal state;
5. captured request state before the next recorded or live assistant turn.

This keeps historical model output deterministic while letting the current
tooling prove whether it behaves well.

## Operator Workflow

1. **Inspect candidate runs.**
   List replayable turn artifacts, assistant message ids, response indexes,
   tool calls, tape continuity, embedded workspace paths, and target workspace
   candidates. This should identify broken tapes or stale prompt paths before a
   probe starts.

2. **Step through recorded output.**
   Replay historical provider responses through current tools one response or
   tool step at a time. The operator should be able to run ranges such as
   "steps 1..15" and inspect compact per-step outcomes.

3. **Find the suspicious region.**
   Stop near a failure mode: search churn, stale workspace paths, malformed
   patch attempts, repeated denied edits, staged edits represented as applied,
   validation becoming invisible, or a tool reply that gives no recovery path.

4. **Branch from a breakpoint.**
   Rewind to a chosen event or response prefix, replay the recorded prefix to
   reconstruct state, then continue with one or a few live assistant turns.
   The live model may still fail, but the probe should show whether our current
   tools gave it a reasonable chance to recover.

5. **Use focused drills only after localization.**
   Once the stepper identifies one bad tool call, a faster tool-call drill can
   run that specific call through the current executor. That is useful for
   debugging, but it is not a substitute for provider-output replay through the
   real session loop.

## Design Shape

The replay coordinate should express a breakpoint, not just an assistant tape.
A useful coordinate probably needs:

- artifact family and artifact path;
- event index inside the `agent-turn` artifact;
- assistant message id;
- provider response index or response-prefix length;
- target workspace root;
- optional historical-to-target workspace rebase information.

The command surface should stay thin. The library layer should own:

- locating and validating candidate replay artifacts;
- mapping `agent-turn` event steps to provider response prefixes;
- installing recorded prefixes into the TUI session;
- running recorded-only and prefix-then-live modes;
- capturing compact per-step evidence.

The CLI should parse flags and render either a table, JSON summary, or probe
bundle. It should not become the place where replay semantics are inferred.

## Useful Modes

- `inspect`: show candidate turns, tape continuity, response indexes, tool
  calls, and path mismatches without executing the loop.
- `recorded-only`: replay a prefix or range through current tools and stop
  before any live provider call. Exhausting the selected recorded prefix is the
  intended stop condition, not model-output failure.
- `live-tail`: replay a prefix to a breakpoint, then allow a bounded number of
  live assistant turns.
- `tool-call-drill`: rerun one localized historical tool call through current
  tool code after the recorded stepper has identified it.
- `bundle`: write a compact probe artifact with step summaries, next request
  snippets, tool outcomes, workspace diff, and detected quality signals.

## Quality Signals

The first useful reports can be mostly mechanical:

- recorded prompt mentions a stale or unreadable workspace path;
- target workspace and prompt workspace disagree;
- response tape has missing or duplicated response indexes;
- tool description and validator behavior disagree;
- tool reply lacks an actionable recovery hint;
- model repeats the same denied action;
- search calls return no new information across several steps;
- patch result is staged but represented to the model as applied;
- validation failure is hidden from the next request;
- proposal state, changed files, and final patch output disagree.

These signals do not judge model quality directly. They judge whether the loop
environment is giving the model useful, truthful feedback.

## Next Steps

1. Extend the implemented inspect projection with better selector ergonomics
   once real use-testing shows which filters matter most. It currently lists
   replayable cursors, tape continuity, response indexes, tool names, call ids,
   and embedded workspace paths.
2. Done: make recorded-only stop-mode output cleaner. `ReplayExhausted` now
   distinguishes intentional tape exhaustion from malformed provider output,
   and the session reports the probe boundary as a completed replay stop.
3. Done in part: compact per-step tool output now shows selected response
   prefix metadata, tool requests, and current tool outcomes with bounded
   per-result detail. The remaining piece is an explicit next-provider-request
   snapshot.
4. Use the breakpoint selectors in live-tail smoke probes with explicit
   live-step and timeout budgets.
5. Add probe bundles for request-before-breakpoint, per-step tool outcomes,
   detected quality signals, and workspace diff.
6. Add deterministic regression tests for the stepper and keep live-provider
   probes as explicit smoke tests, not replay contract tests.
