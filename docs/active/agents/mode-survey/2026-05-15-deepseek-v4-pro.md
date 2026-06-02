# deepseek/deepseek-v4-pro

- Date: 2026-05-15
- Worker: mode-survey-b retry
- Request: `/home/brasides/.ploke-eval/campaigns/p1-broad-harness-2x3-20260515-1/prototype1/messages/edit-harness-request/node-81ce0b03398f1d89-r6.json`
- Submitted result path: `/home/brasides/.ploke-eval/campaigns/p1-broad-harness-2x3-20260515-1/prototype1/messages/edit-harness-result/node-81ce0b03398f1d89-r6.json`

## Preflight

- Submitted result file before attempt: missing.
- Built operator binary: present at `./target/debug/ploke-eval` in the smoke worktree.
- Note: the first sandboxed invocation of the same command failed before the model attempt because git could not create the worktree branch ref lock under sandbox restrictions. The approved rerun reached the live OpenRouter call.

## Command

```sh
./target/debug/ploke-eval loop prototype1-harness attempt --request /home/brasides/.ploke-eval/campaigns/p1-broad-harness-2x3-20260515-1/prototype1/messages/edit-harness-request/node-81ce0b03398f1d89-r6.json --model-id deepseek/deepseek-v4-pro --max-attempts 1 --timeout-secs 180 --format json
```

## Outcome

- Elapsed time: approximately 53 seconds for the approved run.
- Terminal status: failed, exit code 1.
- Admissible patch/result produced: no.
- Submitted result file after attempt: missing.
- Changed paths: none reported; scratch workspace `git status --short` was empty.
- Failure mode: OpenRouter chat completion returned HTTP 403. The harness reported `headless ploke-tui exhausted 1 attempt(s) without an admissible edit` after the request aborted before staging an edit.
- Provider flag: not passed.

## Judgment

Not useful for future live broad-harness runs until OpenRouter access for `deepseek/deepseek-v4-pro` is fixed or the model route is otherwise authorized. The model did not reach tool use or candidate generation.
