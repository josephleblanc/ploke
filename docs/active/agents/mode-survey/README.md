# Mode Survey

This directory stores per-model feasibility reports for the hidden
`prototype1-harness attempt` command.

Verified against the live OpenRouter `/api/v1/models` catalog on 2026-05-15:

| Model ID | Context | Max output | Tool parameter |
| --- | ---: | ---: | --- |
| `inclusionai/ring-2.6-1t:free` | 262144 | 65536 | yes |
| `poolside/laguna-xs.2:free` | 131072 | 8192 | yes |
| `poolside/laguna-m.1:free` | 131072 | 8192 | yes |
| `deepseek/deepseek-v4-pro` | 1048576 | 384000 | yes |
| `deepseek/deepseek-v4-flash` | 1048576 | 131072 | yes |
| `xiaomi/mimo-v2.5` | 1048576 | 131072 | yes |
| `minimax/minimax-m2.7` | 196608 | 131072 | yes |
| `minimax/minimax-m2.5:free` | 196608 | 8192 | yes |
| `z-ai/glm-5` | 202752 | provider default | yes |

Run surface:

- Operator worktree: `/home/brasides/.ploke-eval/worktrees/p1-harness-operator-cmd-smoke-20260515-1`
- Command: `./target/debug/ploke-eval loop prototype1-harness attempt`
- Intended budget: `--max-attempts 1 --timeout-secs 180 --format json`
- Do not run `prototype1-state` for this survey.

Each report should include:

- model ID and request path used
- exact command run
- elapsed time and terminal status
- whether an admissible patch/result was produced
- changed paths, if reported
- failure mode, if any
- a short judgment: useful for next live broad-harness runs, maybe useful, or not useful
