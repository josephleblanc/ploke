# inclusionai/ring-2.6-1t:free live harness attempt

- Date: 2026-05-15
- Model ID: `inclusionai/ring-2.6-1t:free`
- Request path: `/home/brasides/.ploke-eval/campaigns/p1-broad-turn-complete-2x3-20260515-1/prototype1/messages/edit-harness-request/node-f2ad1a4af720620b.json`
- Worktree: `/home/brasides/.ploke-eval/worktrees/p1-harness-operator-cmd-smoke-20260515-1`

## Command

```bash
./target/debug/ploke-eval loop prototype1-harness attempt --request /home/brasides/.ploke-eval/campaigns/p1-broad-turn-complete-2x3-20260515-1/prototype1/messages/edit-harness-request/node-f2ad1a4af720620b.json --model-id inclusionai/ring-2.6-1t:free --max-attempts 1 --timeout-secs 180 --format json
```

## Outcome

- Elapsed time: about 70.8 seconds observed wall clock.
- Exit/terminal status: exit code 1; `batch selection is invalid`.
- Admissible patch/result: no admissible patch was produced.
- Submitted result path: `/home/brasides/.ploke-eval/campaigns/p1-broad-turn-complete-2x3-20260515-1/prototype1/messages/edit-harness-result/node-f2ad1a4af720620b.headless-tui.json` exists by metadata check, 63K, mtime `2026-05-15 06:35`.
- Changed paths: none reported by terminal; candidate workspace `git status --short` was clean.
- Failure mode: OpenRouter chat completion returned `HTTP_429`; the headless TUI exhausted 1 attempt before staging an edit.

## Judgment

Not useful for next broad-harness runs. This attempt did not reach an edit-producing model turn after funding; it still failed at provider/rate-limit access.
