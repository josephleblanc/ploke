# minimax/minimax-m2.5:free

- Date: 2026-05-15
- Worker: mode-survey-c retry
- Operator worktree: `/home/brasides/.ploke-eval/worktrees/p1-harness-operator-cmd-smoke-20260515-1`
- Request: `/home/brasides/.ploke-eval/campaigns/p1-sparse-context-broad-tui-20260514-4/prototype1/messages/edit-harness-request/node-e8202a069a00d7f5-r3.json`
- Submitted result slot: `/home/brasides/.ploke-eval/campaigns/p1-sparse-context-broad-tui-20260514-4/prototype1/messages/edit-harness-result/node-e8202a069a00d7f5-r3.json`

## Command

```sh
./target/debug/ploke-eval loop prototype1-harness attempt --request /home/brasides/.ploke-eval/campaigns/p1-sparse-context-broad-tui-20260514-4/prototype1/messages/edit-harness-request/node-e8202a069a00d7f5-r3.json --model-id minimax/minimax-m2.5:free --max-attempts 1 --timeout-secs 180 --format json
```

## Outcome

- Preflight: submitted result file did not exist before the attempt.
- Elapsed time: about 149 seconds observed wall time.
- Terminal status: exited 1.
- Admissible patch/result: no.
- Submitted result file after attempt: absent.
- Changed paths: none reported; `git status --short` in the candidate workspace was empty after the attempt.

## Failure Mode

The hidden attempt reached the headless `ploke-tui` LLM request path but OpenRouter returned HTTP 429 for the chat completion request. The harness then terminated with:

```text
batch selection is invalid: headless ploke-tui exhausted 1 attempt(s) without an admissible edit
```

The failure occurred before any edit was staged.

## Judgment

Maybe useful only if rate-limit behavior improves. The model path is wired through the harness, but this run consumed most of the attempt budget and ended with HTTP 429 before producing a patch.
