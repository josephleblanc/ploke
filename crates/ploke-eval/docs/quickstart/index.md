# Quick Start

Use this section when you want commands, not architecture.

Choose a path:

| Goal | Start here | Writes state? |
| --- | --- | --- |
| Prepare local environment | [Setup](setup.md) | no, except optional model cache |
| Run one benchmark instance | [Single Run](single-run.md) | yes: repo/instance artifacts |
| Run a small batch | [Batch Run](batch-run.md) | yes: batch/instance artifacts |
| Inspect or pin model/provider routing | [Providers](providers.md) | maybe: model registry/preferences |
| Start a Prototype 1 loop | [Prototype 1](prototype1.md) | yes: campaign and checkout state |

Default eval home:

```text
~/.ploke-eval/
```

Override it for scratch work:

```bash
PLOKE_EVAL_HOME=/tmp/ploke-eval-scratch cargo run -p ploke-eval -- doctor
```

Safety labels used in this book are defined in
[Command Safety](../operations/command-safety.md).
