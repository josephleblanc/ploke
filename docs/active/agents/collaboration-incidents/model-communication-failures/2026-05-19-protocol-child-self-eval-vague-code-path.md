# 2026-05-19 Protocol Child Self-Eval Vague Code Path

## Trigger

While investigating missing treatment protocol artifacts in
`p1-broad-batch-admission-20260518-2`, the agent answered with vague language:
"the code path exists."

## User-Visible Failure

The answer did not say which runtime role owned protocol closure, where the call
was made, whether the child or parent executed it, or how that related to the
C1-C5 child chain. The user had to ask for the basic execution boundary again.

## Touched Code Surface

- `crates/ploke-eval/src/cli/prototype1_process.rs`
- `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs`
- `crates/ploke-eval/src/cli/prototype1_state/c3.rs`
- `crates/ploke-eval/src/cli/prototype1_state/c4.rs`
- `crates/ploke-eval/src/cli.rs`

## What The Agent Did

The agent had already traced the relevant calls, but compressed the result into
ambiguous prose instead of naming the execution chain:

- parent C1-C4 materializes, builds, and spawns the child runtime
- child runtime enters `execute_prototype1_runner_invocation`
- child self-eval calls `run_prototype1_resolved_branch_treatment`
- that function calls `advance_eval_closure` and then `advance_protocol_closure`
- parent C5 observes the child terminal payload and compares treatment evidence

## Skipped Docs / Skills / Instructions

The relevant skills were loaded, but the answer violated the practical output
requirement of stating exact verified surfaces and exact authority boundaries.
It also failed the user's preference for decisive evidence over vague forensic
phrasing.

## Why This Was Risky

This investigation depends on whether missing protocol artifacts are a child
self-evaluation failure, a parent comparison failure, or cleanup/data-loss. Vague
"path exists" language hides the runtime authority boundary and can lead to a
wrong fix in the parent selection layer instead of the child self-eval completion
contract.

## Prevention Rule

When explaining Prototype 1 runtime behavior, always name:

- caller role (`Parent`, `Child`, or `Successor`)
- transition phase (`C1` through `C5` where applicable)
- exact function and file
- persisted output path
- whether the code reads, writes, or only diagnoses the record

Do not use "code path exists" without immediately naming the caller, callee, and
runtime role.

## Memory Hypothesis

Memory correctly pointed at prior protocol persistence gaps, but the agent
over-compressed the answer after verification. The failure was communication
precision, not missing evidence.
