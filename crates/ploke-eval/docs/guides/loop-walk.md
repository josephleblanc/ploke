# Prototype 1 `loop walk` guide

`ploke-eval loop walk` is the operator/debugger surface for Prototype 1 runs. It has two modes of use:

- **drive the live outer loop** (`walk start`, `walk step`, `walk stop`);
- **review recorded evidence** (`walk summary`, `walk replay`, `walk llm ...`).

## Which binary to use

Use the binary that matches the authority you need:

- For local prompt/format/debugger iteration, use the local dev binary from the checkout you are editing, for example:

  ```bash
  /home/brasides/code/ploke/target/debug/ploke-eval loop walk ...
  ```

- For authority-bearing live parent execution, use the binary from the active parent checkout/worktree for that run. Historical campaign/worktree paths are inputs; they do not automatically select the binary under test.

After changing walk protocol structs or output formatting, restart the walk server before relying on new commands. A running server keeps the old binary in memory.

## Starting and stopping the walk server

Typical setup:

```bash
ploke-eval loop walk start --repo-root /path/to/parent/worktree
ploke-eval loop walk show
ploke-eval loop walk stop
```

Most commands accept `--repo-root` to identify the parent checkout and `--socket`/format flags through the shared walk control options.

## Driving the live outer loop

`walk step` advances the live Prototype 1 typestate edge. It is not a historical replay command.

Examples:

```bash
ploke-eval loop walk step
ploke-eval loop walk step --watch
ploke-eval loop walk step --until R8 --watch
```

Workspace mutations are intentionally gated. Use mutation flags only when you intend to let the live parent proceed through effectful edges.

## Read-only run review

These commands do not call providers, execute tools, mutate the checkout, or advance typestate:

```bash
ploke-eval loop walk summary
ploke-eval loop walk replay
ploke-eval loop walk back
ploke-eval loop walk forward
```

Use `summary` for the current campaign overview and `replay/back/forward` as a cursor over the durable transition journal.

## LLM/tool-loop lane review

Tool-loop checkpoints are stored under:

```text
<campaign>/prototype1/debug/tool-loop/<session-id>/
```

Start with lanes and timeline:

```bash
ploke-eval loop walk llm lanes
ploke-eval loop walk llm focus <lane-id>
ploke-eval loop walk llm timeline
```

Then drill down:

```bash
ploke-eval loop walk llm show --step N
ploke-eval loop walk llm prompt --step N
ploke-eval loop walk llm tool --step N --call 1
ploke-eval loop walk llm protocol
```

Read-only lane commands include:

- `lanes` / `focus` — choose a default lane/session for subsequent commands;
- `timeline` — compact scan of recorded responses and tool batches;
- `show` — checkpoint transcript, tool results, and protocol hints when present;
- `prompt` — persisted request messages sent for a step;
- `tool` — selected persisted tool arguments plus current-renderer tool schema;
- `protocol` — persisted protocol artifact summary and per-call review feedback when present;
- `back` / `forward` / `head` — move the read-only lane cursor.

Important provenance distinction: historical request messages and tool arguments come from persisted checkpoints. Current tool definitions in `walk llm tool` come from the current renderer checkout and are labeled as such.

## Protocol review output

`walk llm protocol` is a read-only review command:

```bash
ploke-eval loop walk llm protocol
ploke-eval loop walk llm protocol --json
```

If no artifacts are available for the selected session, it says:

```text
protocol: not present for this tool-loop session
```

When artifacts exist, it summarizes artifact counts, call-review coverage, segment-review counts, missing reviews, and per-call feedback. Protocol review output is debugging evidence only; it is not child-plan admission authority and does not override protected-write enforcement.

## Effectful nested LLM debugger

The nested debugger can branch from a checkpoint and execute more tool-loop work. These commands are effectful:

```bash
ploke-eval loop walk llm step --step N --source historical --allow workspace-mutation
ploke-eval loop walk llm step --step N --source live --watch --allow workspace-mutation
ploke-eval loop walk llm finish --source live --watch --allow workspace-mutation
```

Rules of thumb:

- `--allow workspace-mutation` is required because tools may change the candidate workspace.
- Live mode requires `--watch` because it may call the provider and execute tool batches.
- `walk llm step`/`finish` create or focus a new debug session; they do not rewrite the historical session.
- Protected-write denials remain authoritative. Do not bypass them.

## Lightweight prompt-lab commands

For prompt iteration outside the full parent loop, prefer the hidden harness helpers:

```bash
ploke-eval loop prototype1-prompt --repo-root /path/to/worktree
ploke-eval loop prototype1-harness attempt --request /path/to/edit-harness-request.json --model-id google/gemini-2.5-pro --provider google --max-attempts 1 --timeout-secs 300 --format json
ploke-eval loop prototype1-harness sweep --help
```

Use these when you want to rerun a published broad-harness request without advancing the outer typestate walk.

## Pre-live-run review checklist

1. Restart the walk server after local protocol/output changes.
2. `walk summary` to confirm campaign phase and admission counts.
3. `walk llm lanes` and `walk llm timeline` to locate the session under review.
4. `walk llm show --step N` for the transcript and tool outcomes.
5. `walk llm prompt --step N` to inspect persisted request messages.
6. `walk llm tool --step N --call K` for selected tool arguments/schema provenance.
7. `walk llm protocol` for available protocol review feedback.
8. Only then use effectful `walk step` or `walk llm step/finish` with explicit mutation gates.
