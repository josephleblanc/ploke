# 2026-05-17 apply_code_edit rebase plan obscured by model mixing

## Trigger

The user asked for a concrete plan after proposing that pending patches might be
checked for compatibility and re-anchored instead of being blanket-marked stale.
The user then interrupted the planning flow and said they no longer understood
the explanation.

## User-visible failure

The agent buried the useful recommendation under mixed models:

- `ploke-tui` local patch/proposal mechanics;
- `ploke-eval` authority/admission vocabulary;
- plan-mode tradeoff prompts before a plain recommendation.

This made the discussion feel more complicated than the implementation question:
what should `apply_code_edit` do when one pending edit changes the file version
under another pending edit?

## Touched code surface

- `crates/ploke-tui/src/rag/tools.rs`
- `crates/ploke-tui/src/rag/editing.rs`
- `crates/ploke-tui/src/app_state/core.rs`

No repo code was changed during the failed explanation.

## What the agent did

The agent correctly noticed that `apply_code_edit` currently stores resolved
`WriteSnippetData` and loses the original canonical request, but explained the
problem through too many abstractions before giving the simpler product-level
answer.

## Skipped or overreached

- Overreached into `ploke-eval` authority language after the user was asking
  about `ploke-tui` patch mechanics.
- Asked a broad design question before first stating a clear recommended path.
- Did not reduce the idea to the concrete invariant: pending semantic edits
  need to be re-resolved against the latest parsed file state before approval.

## Why this was risky

This kind of explanation can make a good local implementation idea look
incoherent. It also risks pulling a `ploke-tui` repair into `ploke-eval`
workarounds, which is exactly the direction the user wanted to avoid.

## Prevention rule

When the user is asking whether a local mechanism should exist in `ploke-tui`,
answer first in `ploke-tui` terms:

```text
stored proposal input -> current file/index state -> compatibility check -> updated pending proposal or stale
```

Only bring in `ploke-eval` authority/admission language after the local TUI
mechanism has been explained and only if the user asks how eval should consume
it.

## Memory hypothesis

Memory strongly biases agents toward authority-boundary language around
Prototype 1. That is useful for `ploke-eval`, but it can obscure local
`ploke-tui` tool mechanics. Future agents should explicitly separate local TUI
mechanism from eval admission when both are nearby.
