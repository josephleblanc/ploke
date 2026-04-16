# Protocol Segment Review Rejects a Valid Intent Segment Index

- date: 2026-04-15
- task title: protocol segment review rejects a valid intent segment index
- task description: track the persistent `tool-call-segment-review` failure on `tokio-rs__tokio-6409` where the latest intent-segmentation artifact contains segment `2`, but the review command reports that index as not found
- related planning files: `docs/active/agents/2026-04-15_orchestration-hygiene-and-artifact-monitor.md`

## Summary

`./target/debug/ploke-eval protocol tool-call-segment-review --instance tokio-rs__tokio-6409 --format json 2`
fails with:

`database setup failed during 'protocol_tool_call_segment_review': segment index 2 not found`

This is surprising because the latest intent-segmentation artifact for the run
does include `segment_index = 2`.

## Evidence

- [tokio-rs__tokio-6409 intent segmentation artifact](/home/brasides/.ploke-eval/runs/tokio-rs__tokio-6409/protocol-artifacts/1776271730466_tool_call_intent_segmentation_tokio-rs__tokio-6409.json)
  The latest artifact currently lists segments `0`, `1`, and `2`.
- CLI repro:
  `./target/debug/ploke-eval protocol tool-call-segment-review --instance tokio-rs__tokio-6409 --format json 2`
  returns `segment index 2 not found`.

## Risk

- Owned-run protocol-artifact coverage remains incomplete for this run.
- The CLI may be reading a different segment source than the latest persisted intent-segmentation artifact, or it may have an off-by-one / stale-state bug.

## Suggested Follow-Up

- Recheck how `tool-call-segment-review` resolves segment indices for a run.
- Compare the command’s lookup source against the persisted intent-segmentation artifact.
- Decide whether the bug is in artifact selection, segment indexing, or database setup state.

## Additional Repro

- date: 2026-04-15
- task title: protocol segment review rejects tokio-rs__tokio-5179 segment 5
- task description: track a second persistent `tool-call-segment-review` failure where `tokio-rs__tokio-5179` segment `4` was recoverable after retry, but segment `5` still fails with `segment index 5 not found`
- related planning files: `docs/active/agents/2026-04-15_orchestration-hygiene-and-artifact-monitor.md`

### Evidence

- CLI repro:
  `./target/debug/ploke-eval protocol tool-call-segment-review --instance tokio-rs__tokio-5179 --format json 5`
  returns `segment index 5 not found`.
- The run’s latest intent-segmentation artifact for `tokio-rs__tokio-5179` still lists segment indices through `5`, so the lookup is failing against a segment that should be addressable from the persisted segmentation output.

### Risk

- Owned-run protocol-artifact coverage remains incomplete for `tokio-rs__tokio-5179`.
- The failure appears persistent for this segment index even after retrying the command shape used in the coverage pass.
