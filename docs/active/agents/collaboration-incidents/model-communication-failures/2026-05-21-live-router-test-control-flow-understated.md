# Live Router Test Control Flow Understated

## Trigger

The user asked "Yeah did you read the test?" after an answer about failing
`ploke-eval` live/replay tests.

## User-visible failure

The answer reported the observed live-test abort and the active model/env
mismatch, but it did not first restate the test's own control flow and
assertions. That made the diagnosis look like it came from external config
inspection rather than from the test body.

## Touched code surface

- `crates/ploke-eval/src/cli/prototype1_state/edit_surface/tests.rs`
- `crates/ploke-tui/src/llm/manager/mod.rs`
- `crates/ploke-eval/src/cli/prototype1_state/edit_surface/request_policy.rs`

## What the agent did

The agent had read the relevant test body, including:

- the `live_openrouter_env_or_skip` guard,
- active-model route selection into direct Google vs OpenRouter,
- the prompt requiring one `apply_code_edit` call,
- the loop that panics if terminal chat state arrives before proposal staging,
- the later OpenRouter request-policy receipt reconstruction.

The final answer compressed that into "Google env mismatch" plus a generic
`ToolChoice::Auto` warning, without leading from the test's branches and
assertions.

## Skipped docs / skills / instructions

No required source file was wholly skipped. The failure was communication
discipline: the answer did not prove that the test was read by naming the
specific branch and assertion chain before giving the diagnosis.

## Why this was risky

This test is a live canary with mixed routing and policy-receipt assumptions.
If the diagnosis is not tied to the exact test branches, a future fix may patch
the credential guard, the router setup, or the request-policy receipt in
isolation while leaving the actual canary contract incoherent.

## Prevention rule

When a user asks why a specific failing test fails, answer from the test body
first: name the setup guard, route/config branch, awaited event or assertion,
and failure point before giving external environment or historical-memory
context. If the test has mixed route or policy assumptions, call that out as
the test contract, not just as an environment symptom.

## Memory hypothesis

Prior memory about the same live canary helped identify the `ToolChoice::Auto`
risk, but it also biased the answer toward an older failure shape. Future
answers should separate current run facts from memory-derived context in the
first paragraph.
