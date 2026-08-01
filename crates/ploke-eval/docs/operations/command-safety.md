# Command Safety

This book uses short labels to make mutability explicit.

| Label | Meaning |
| --- | --- |
| `RO` | Read-only inspection or diagnosis. |
| `BUILD` | Writes Cargo build artifacts under `target/`. |
| `FS` | Writes local eval artifacts under `~/.ploke-eval/` or an override root. |
| `NET` | May contact a remote git, model metadata, provider metadata, or dataset endpoint. |
| `LIVE-API` | May make a live provider/model request. Do not treat these examples as safe dry runs. |
| `GIT` | Mutates a git checkout, worktree, branch, or commit. |
| `PROJ` | Writes projection/debug state that is useful but not authority-bearing. |
| `AUTH` | Performs or records an authority-bearing transition, admitted profile, sealed History block, or handoff fact. |

The labels are descriptive, not a replacement for reading command help.

## Common commands

| Command | Safety | Notes |
| --- | --- | --- |
| `ploke-eval doctor` | `RO` | Setup diagnosis. |
| `run datasets list` | `RO` | Built-in registry view. |
| `run repo fetch` | `FS` + `NET` + `GIT` | Creates/fetches benchmark checkout. |
| `run prepare instance` | `FS` | Writes normalized run manifest. |
| `run prepare batch` | `FS` | Writes batch manifest and per-instance manifests. |
| `run single setup` | `FS` + `GIT` + maybe `NET` | Resets repo, indexes, snapshots. |
| `run single agent` | `FS` + `GIT` + `NET` + `LIVE-API` | Adds agent turn and patch/submission artifacts. |
| `run batch agent` | `FS` + `GIT` + `NET` + `LIVE-API` | Repeats single-agent work across a batch. |
| `transcript` | `RO` | Reads last completed run unless a run is specified. |
| `inspect ...` | `RO` | Should remain inspection-only. |
| `campaign list/show/validate` | `RO` | Validation may inspect provider/local state. |
| `campaign init` | `FS` | Writes campaign manifest. |
| `campaign export-submissions` | `FS` | Writes campaign-scoped JSONL export. |
| `model list/find/current` | `RO` | Reads cached model registry/selection. |
| `model providers` | `RO` + `NET` | May call the OpenRouter endpoints API unless the cached model route is direct Google. |
| `model refresh` | `FS` + `NET` | Updates cached OpenRouter model registry. |
| `model provider set/clear` | `FS` | Updates provider preferences. |

Source excerpts for live agent command shape:

```rust,ignore
{{#include ../../src/cli/args/run.rs:ploke_eval_run_msb_agent_single_command}}
{{#include ../../src/cli/args/run.rs:ploke_eval_run_msb_agent_batch_command}}
```

## Prototype 1 commands

| Command | Safety | Notes |
| --- | --- | --- |
| `loop prototype1-doctor` | `RO` | Diagnosis; `--live-protocol-preflight` adds `NET` + `LIVE-API`. |
| `loop prototype1-prompt` | `RO` | Prints current broad-harness prompt. |
| `loop prototype1-setup` | `FS` + `GIT` + `AUTH` | Admits profile, writes parent identity, commits it. |
| `loop prototype1-step` | `FS` + maybe `NET`/`GIT`/`AUTH` | Mutability depends on diagnosed phase. |
| `loop prototype1-continue` | `FS` + `NET` + `GIT` + `AUTH` + `LIVE-API` | Repeatedly advances phases until terminal/blocking condition. |
| `loop prototype1-state` | `FS` + `GIT` + `AUTH` | Typed parent runtime path. |
| `loop walk summary/replay/back/forward` | `RO` | Durable historical inspection. |
| `loop walk step` | `FS` + maybe `NET`/`GIT`/`AUTH`/`LIVE-API` | Drives live typestate edges; guarded by flags for long/mutating edges. |

Source excerpt for walk safety gates:

```rust,ignore
{{#include ../../src/cli/args/loop_args.rs:prototype1_walk_step_live_edge_admission}}
```

When in doubt, run `doctor`, `inspect`, `history`, or `walk replay` first.
