# poolside/laguna-m.1:free live harness attempt

- Date: 2026-05-15
- Model ID: `poolside/laguna-m.1:free`
- Request path: `/home/brasides/.ploke-eval/campaigns/p1-broad-turn-complete-2x3-20260515-1/prototype1/messages/edit-harness-request/node-f2ad1a4af720620b-r3.json`
- Worktree: `/home/brasides/.ploke-eval/worktrees/p1-harness-operator-cmd-smoke-20260515-1`

## Command

```bash
./target/debug/ploke-eval loop prototype1-harness attempt --request /home/brasides/.ploke-eval/campaigns/p1-broad-turn-complete-2x3-20260515-1/prototype1/messages/edit-harness-request/node-f2ad1a4af720620b-r3.json --model-id poolside/laguna-m.1:free --max-attempts 1 --timeout-secs 180 --format json
```

## Outcome

- Elapsed time: about 164.6 seconds observed wall clock; terminal status reported the headless TUI timed out after 180 seconds.
- Exit/terminal status: exit code 1; `batch selection is invalid`.
- Admissible patch/result: no admissible patch was produced.
- Submitted result path: `/home/brasides/.ploke-eval/campaigns/p1-broad-turn-complete-2x3-20260515-1/prototype1/messages/edit-harness-result/node-f2ad1a4af720620b-r3.headless-tui.json` exists by metadata check, 54K, mtime `2026-05-15 06:39`.
- Changed paths: none reported by terminal; candidate workspace `git status --short` was clean.
- Failure mode: model behavior error `INVALID_MODEL_RESPONSE`, then harness channel closed, then headless TUI timed out.

## Judgment

Not useful for next broad-harness runs. This model avoided the immediate `HTTP_429` seen on the other two slots, but still failed to produce a staged edit within the configured harness timeout.
