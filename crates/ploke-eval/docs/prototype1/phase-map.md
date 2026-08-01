# Phase Map

`prototype1-doctor` diagnoses the active parent checkout and reports the next
phase. `prototype1-step` advances exactly one diagnosed phase. `prototype1-continue`
repeats phases until the current turn completes, blocks, or hands off.

| Phase | Meaning | Safety | Primary evidence/files |
| --- | --- | --- | --- |
| `baseline_eval` | Generation-0 parent still needs baseline eval closure. | `FS` + `NET` | `closure-state.json`, instance records |
| `baseline_protocol` | Generation-0 parent still needs baseline protocol closure. | `FS` + `NET` | `closure-state.json`, protocol artifacts |
| `child_plan` | Parent is ready but no child-plan message exists. | `FS` + maybe `NET` + `AUTH` | `messages/child-plan/<parent-node-id>.json`, request/result slots |
| `materialize` | Planned child still needs workspace/artifact surface. | `FS` + `GIT` | `nodes/<node-id>/worktree/`, journal |
| `build` | Child workspace exists but binary is not built. | `BUILD` + `FS` | `nodes/<node-id>/bin/`, `target/`, journal |
| `spawn` | Child binary exists but runtime is not spawned. | `FS` + process spawn | invocation file, channel dir, streams |
| `observe` | Child is running or terminal evidence needs comparison. | `FS` | child-to-parent channel, result mirror, evaluation report |
| `select` | Children are terminal and parent can record selection. | `FS` + `AUTH` | evaluation reports, selection/journal evidence |
| `handoff` | Selected successor can be installed and acknowledged. | `FS` + `GIT` + `AUTH` | active checkout, History, ready/completion files |
| `complete` | No next active phase for this parent turn. | `RO` | summary only |
| `blocked` | Required evidence or consistency check failed. | `RO` until repaired | blocker text names missing/mismatched evidence |

## Operator rule

Start with:

```bash
./target/debug/ploke-eval loop prototype1-doctor --repo-root .
```

Then choose:

- `prototype1-step` when you want one phase and a fresh status;
- `prototype1-continue` only when binary provenance and checkout state are clear;
- `walk replay/back/forward` when you want read-only historical inspection.
