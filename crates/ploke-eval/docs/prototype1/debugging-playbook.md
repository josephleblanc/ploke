# Debugging Playbook

Use this page when a Prototype 1 run is stalled, surprising, or hard to read.

## 1. Confirm the two roots

Keep these separate:

```text
campaign root:       ~/.ploke-eval/campaigns/<campaign-id>/prototype1/
active parent root:  the --repo-root checkout with .ploke/prototype1/parent_identity.json
```

Use the `ploke-eval` binary built inside the active parent checkout.

## 2. Run read-only diagnosis first

```bash
./target/debug/ploke-eval loop prototype1-doctor --repo-root .
```

If provider request shape is the suspected blocker and a live call is acceptable:

```bash
./target/debug/ploke-eval loop prototype1-doctor --repo-root . --live-protocol-preflight
```

## 3. Prefer metadata before full JSONL reads

For routine checks, inspect:

- node counts;
- status counts;
- file mtimes;
- byte sizes;
- disk free space.

Read large JSONL files only for a named anomaly, with record and width caps.

## 4. Use `walk` for historical inspection

```bash
./target/debug/ploke-eval loop walk use .
./target/debug/ploke-eval loop walk summary -v
./target/debug/ploke-eval loop walk replay --index 0
./target/debug/ploke-eval loop walk forward --steps 10 --tail 20
```

`replay`, `back`, and `forward` are read-only. `step` is live.

## 5. Check authority boundaries

Do not promote evidence just because it appears in a projection.

| Evidence family | Use |
| --- | --- |
| `history/blocks/*` | sealed handoff authority |
| `transition-journal.jsonl` | append-only transition replay stream |
| child channels | runtime lifecycle and terminal child evidence |
| scheduler/branch/node JSON | operational projections |
| CLI tables | display projections |

## 6. If blocked, classify the blocker

| Blocker shape | First places to inspect |
| --- | --- |
| Missing parent identity | `<repo-root>/.ploke/prototype1/parent_identity.json`, setup logs |
| Missing/invalid profile | `run-profile.toml`, `run-profile.commitment.json` |
| Child plan absent | `messages/child-plan/`, edit-harness request/result slots |
| Child stuck before build | worktree path, node status, transition journal |
| Child stuck after spawn | invocation, channels, streams, result mirror |
| Selection blocked | evaluation reports, branch registry, selection policy |
| Handoff blocked | active checkout state, History append, ready/completion files |

## 7. Advance only after the evidence matches the phase

```bash
./target/debug/ploke-eval loop prototype1-step --repo-root .
```

Use `continue` only when repeated live transitions are intentional.
