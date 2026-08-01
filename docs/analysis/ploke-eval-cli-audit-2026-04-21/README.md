# ploke-eval CLI Audit

Date: 2026-04-21

This directory is a CLI-first audit of `ploke-eval`.

The point is simple:
- stop treating the source as the primary operator manual
- identify the small command surface we actually need
- separate commands whose help text is mostly trustworthy from commands whose contract is misleading or fragile
- explain the higher-level `campaign` / `registry` / `closure` layer in operator terms

## Main outputs

- [cli-map-and-workflows.md](./cli-map-and-workflows.md)
  What the CLI commands are, what they claim to do, and the practical workflows implied by help text.
- [command-trust-audit.md](./command-trust-audit.md)
  Which commands appear reliable, which ones are materially misleading, what model/provider setup really does, and where the help surface fails both humans and agents.
- [skill-trials/README.md](./skill-trials/README.md)
  Blind validations of the new `ploke-eval-operator` skill against the CLI help surface before and after the help patch.

## Executive Summary

We have not completely fucked ourselves, but we have absolutely overgrown the operator surface.

The good news:
- There is a small usable core for eval work.
- Single-instance workflows are mostly understandable from the CLI.
- Model/provider commands are more real than fake.
- `campaign export-submissions` is actually a better artifact export path than the fragile batch aggregate JSONL.

The bad news:
- The batch commands overclaim simplicity and underdescribe their failure modes.
- The CLI does not clearly communicate the difference between per-run artifacts and batch aggregate artifacts.
- The CLI does not clearly explain model selection precedence, provider persistence, or embedding overrides.
- `campaign`, `registry`, and `closure` are probably the right operator abstraction for measured work, but their relationship to ordinary prepare/run commands is not explained well enough from help text.
- Inspection is powerful but fragmented: `transcript`, top-level `conversations`, and `inspect ...` overlap without a strong “start here” story.

The shortest path to unfuck ourselves:
1. Treat `run-msb-agent-single` as the default trustworthy execution primitive.
2. Treat `campaign export-submissions` and per-run `multi-swe-bench-submission.jsonl` as the trustworthy export surface.
3. Treat batch aggregate `multi-swe-bench-submission.jsonl` as non-authoritative until batch semantics are hardened.
4. Publish one short operator guide that starts with `list-msb-datasets`, `fetch-msb-repo`, `prepare-msb-single`, `run-msb-agent-single`, `inspect`, `campaign`, and `closure`.
5. Either harden `run-msb-agent-batch` semantics or demote it in help text so it stops reading like a clean, boring batch executor when it is not.

## Independent CLI readers

Sub-agent reports written from CLI help with no reliability priming:
- [gpt54-medium.md](./subagents/gpt54-medium.md)
- [gpt54-high.md](./subagents/gpt54-high.md)
- [gpt54mini-medium.md](./subagents/gpt54mini-medium.md)
- [gpt54mini-high.md](./subagents/gpt54mini-high.md)
