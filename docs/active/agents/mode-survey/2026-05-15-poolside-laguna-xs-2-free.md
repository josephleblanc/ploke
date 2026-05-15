# poolside/laguna-xs.2:free live harness attempt

- Date: 2026-05-15
- Model ID: `poolside/laguna-xs.2:free`
- Request path: `/home/brasides/.ploke-eval/campaigns/p1-broad-turn-complete-2x3-20260515-1/prototype1/messages/edit-harness-request/node-f2ad1a4af720620b-r2.json`
- Worktree: `/home/brasides/.ploke-eval/worktrees/p1-harness-operator-cmd-smoke-20260515-1`

## Command

```bash
./target/debug/ploke-eval loop prototype1-harness attempt --request /home/brasides/.ploke-eval/campaigns/p1-broad-turn-complete-2x3-20260515-1/prototype1/messages/edit-harness-request/node-f2ad1a4af720620b-r2.json --model-id poolside/laguna-xs.2:free --max-attempts 1 --timeout-secs 180 --format json
```

## Outcome

- Elapsed time: about 71.9 seconds observed wall clock.
- Exit/terminal status: exit code 1; `batch selection is invalid`.
- Admissible patch/result: no admissible patch was produced.
- Submitted result path: `/home/brasides/.ploke-eval/campaigns/p1-broad-turn-complete-2x3-20260515-1/prototype1/messages/edit-harness-result/node-f2ad1a4af720620b-r2.headless-tui.json` exists by metadata check, 66K, mtime `2026-05-15 06:36`.
- Changed paths: none reported by terminal; candidate workspace `git status --short` was clean.
- Failure mode: OpenRouter chat completion returned `HTTP_429`; the headless TUI exhausted 1 attempt before staging an edit. Startup also reported an unknown OpenRouter provider value `Poolside`.

## Judgment

Not useful for next broad-harness runs. It reached the Poolside provider path but failed with provider/rate-limit access before any edit-producing response.
