# Bug: current `nushell` eval runs time out waiting for `indexing_completed`

- date: 2026-04-17
- status: active
- crate affected: eval indexing/runtime path
- severity: medium-high

## Summary

Six current `nushell__nushell` eval runs in the `rust-baseline-grok4-xai`
campaign fail with the same status:

`timed out waiting for 'indexing_completed' after 300 seconds`

Affected runs:

- `nushell__nushell-10381`
- `nushell__nushell-10405`
- `nushell__nushell-10613`
- `nushell__nushell-10629`
- `nushell__nushell-11169`
- `nushell__nushell-11292`

This note tracks the repeated operational failure as a live bug even though the
deeper cause is not yet isolated.

## Evidence

- [nushell__nushell-10381 indexing-status.json](/home/brasides/.ploke-eval/runs/nushell__nushell-10381/indexing-status.json)
- [nushell__nushell-10405 indexing-status.json](/home/brasides/.ploke-eval/runs/nushell__nushell-10405/indexing-status.json)
- [nushell__nushell-10613 indexing-status.json](/home/brasides/.ploke-eval/runs/nushell__nushell-10613/indexing-status.json)
- [nushell__nushell-10629 indexing-status.json](/home/brasides/.ploke-eval/runs/nushell__nushell-10629/indexing-status.json)
- [nushell__nushell-11169 indexing-status.json](/home/brasides/.ploke-eval/runs/nushell__nushell-11169/indexing-status.json)
- [nushell__nushell-11292 indexing-status.json](/home/brasides/.ploke-eval/runs/nushell__nushell-11292/indexing-status.json)
- Archived symptom match:
  [BurntSushi__ripgrep-1294 indexing timeout postmortem](/home/brasides/code/ploke/docs/archive/agents/2026-04/2026-04-08_eval-postmortems/2026-04-08_ripgrep-all-batch/BurntSushi__ripgrep-1294/2026-04-08_BurntSushi__ripgrep-1294_qwen3.6plus-alibaba_indexing-timeout_postmortem.md)

## Why This Matters

- The timeout blocks eval coverage even when a more specific parser failure is
  not surfaced in the run-local summary.
- The symptom is repeated enough that `nushell` should remain a low-yield family
  for default expansion until the timeout cause is isolated.
- Without an active bug note, this failure mode would remain only in ephemeral
  run artifacts and older postmortems.

## Suggested Follow-Up

- Compare timeout runs against any available checkpoint or incremental indexing
  artifacts to determine where progress stalls.
- Check whether these are pure scaling timeouts or hidden parser/transform
  failures that never surface cleanly before the 300s deadline.
- Revisit the timeout budget only after determining whether the current ceiling
  is masking a different bug.
