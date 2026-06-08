# 2026-05-25 Prototype 1 `observe_child` Stale Hang

Status: fixed in source with local regression coverage after 2026-06-08 live
re-observation. Existing source detected the acknowledged-but-dead child as a
blocker, but the fresh campaign still produced the invalid child state.

## Symptom

Campaign `p1-gemini35-flash-direct-fresh-20260525-035030` reached
`observe_child` for node `node-4a657b2403d14770`, runtime
`c85fcad2-7f28-4cee-b470-a4781951e817`, then stopped making observable
progress. The transition journal had `observe_child:before` at
2026-05-24 23:17:22 PDT, no matching `observe_child:after`, and the
attempt-scoped result path was absent:

```text
prototype1/nodes/node-4a657b2403d14770/results/c85fcad2-7f28-4cee-b470-a4781951e817.json
```

Doctor still reported `phase=observe`, `status=running`, and no blockers.

## Broken Contract

`C4 -> C5` treated the child channel result as the only completion signal and
polled forever when the child stopped producing channel output and did not write
the attempt result. Replay classified the pending observe as generic
`ResultPending`, and doctor did not turn stale pending observe evidence into a
blocker.

## Fix

- `observe_child` now has a stale deadline from
  `[execution].observe_child_stale_after_secs`, with the same default used by
  journal replay callers that do not have an admitted profile.
- `observe_child` checks an attempt-scoped runner result path while polling, so
  a written result without a channel message is no longer invisible.
- Journal replay can classify a missing-result pending observe as
  `StaleOrHung`.
- Doctor adds a blocker for stale/hung pending observe states, including the
  runtime id, missing result path, pid liveness when known, and stream freshness
  when stream paths are recorded. The doctor threshold comes from the admitted
  `run-profile.toml`.
- Follow-up: the default observe-child stale threshold is now 1200 seconds so
  slow but live child eval/protocol runs are less likely to be misclassified as
  stale solely because protocol adjudication exceeded the previous 10-minute
  default.
- Follow-up: doctor now also blocks the pre-observe case where a node is
  `Running`, the spawn journal recorded an acknowledged child PID, no
  `observe_child:before` exists yet, the attempt-scoped runner result is absent,
  and the PID is no longer visible. Without this, the next `prototype1-step`
  could enter observe and wait the full stale threshold even though the child
  process was already gone.

## 2026-05-25 Follow-up Evidence

Campaign `p1-gemini35-flash-direct-15g2x3-fixed-20260525-123824` reached
`phase=observe` for child `node-b85ef46248b84eb4`, runtime
`d7f47451-ac79-4ab0-a980-e36bb10fc71f`. The journal recorded spawn,
ready, evaluating, and spawn-observed/acknowledged records for PID `2479313`,
but there was no `observe_child:before`, no
`nodes/node-b85ef46248b84eb4/results/d7f47451-ac79-4ab0-a980-e36bb10fc71f.json`,
and no live `ploke-eval` process. The treatment campaign had only setup
artifacts and closure state still reported eval evidence as missing.

Before the follow-up fix, doctor still reported no blockers because the stale
logic only handled a pending `observe_child:before`. After rebuilding source,
doctor reports:

```text
phase=blocked
blocker: node 'node-b85ef46248b84eb4' is running for runtime
'd7f47451-ac79-4ab0-a980-e36bb10fc71f', but child pid 2479313 is not visible
and no runner result exists at
.../nodes/node-b85ef46248b84eb4/results/d7f47451-ac79-4ab0-a980-e36bb10fc71f.json
```

Disposition for this campaign: stop advancing it for loop evidence. The child
did not produce treatment evidence, protocol evidence, or a runner result, so
there is no trustworthy child result to compare or select.

## 2026-06-08 Live Re-observation

Campaign
`p1-guided-surface-g35flash-p25flash-5g1x2-a2-pr1-20260608-064108`
reproduced the same blocked state after a successful guided-surface baseline,
full protocol closure, child planning, materialization, build, and spawn.

Evidence:

- Step command:
  `/home/brasides/code/ploke/target/debug/ploke-eval loop prototype1-step --repo-root /home/brasides/.ploke-eval/worktrees/p1-guided-surface-g35flash-p25flash-5g1x2-a2-pr1-20260608-064108 --format json`
- Child node:
  `prototype1/nodes/node-5f465d71ca469dde/node.json`
- Runtime:
  `427f5715-3b7d-4fe9-a1a2-627d899288ac`
- Child PID:
  `586539`
- Treatment campaign:
  `p1-guided-surface-g35flash-p25flash-5g1x2-a2-pr1-20260608-064108-treatment-branch-f5c8c69ddaf21b32-1780930712647`
- Child channel:
  `prototype1/nodes/node-5f465d71ca469dde/channels/427f5715-3b7d-4fe9-a1a2-627d899288ac/child-to-parent.jsonl`
- Missing attempt result:
  `prototype1/nodes/node-5f465d71ca469dde/results/427f5715-3b7d-4fe9-a1a2-627d899288ac.json`

Observed state:

- `child-to-parent.jsonl` contains only `ready` and `evaluating`.
- No `runner-result.json` and no attempt-scoped result exist.
- `ps -fp 586539` shows no live child process.
- The treatment campaign `closure-state.json` reports `eval.status = missing`
  and `protocol.status = missing/ineligible`.
- The child stream logs stop during treatment eval setup after branch checkout
  and ripgrep workspace parsing; there is no terminal child `Result`.
- `prototype1-doctor` now reports `phase=blocked`, allowed action `doctor`
  only, with the blocker:

```text
node 'node-5f465d71ca469dde' is running for runtime
'427f5715-3b7d-4fe9-a1a2-627d899288ac', but child pid 586539 is not visible
and no runner result exists at
.../nodes/node-5f465d71ca469dde/results/427f5715-3b7d-4fe9-a1a2-627d899288ac.json
```

Additional related evidence: the build step printed `WorkspacePathMismatch`
before still advancing to `spawn`. The expected workspace was
`prototype1/nodes/node-5f465d71ca469dde/worktree`, while the observed workspace
was the broad edit-harness workspace:
`prototype1/workspaces/edit-harness/node-50fe903bf027a783`.
The transition journal also records `materialize_branch:after` and
`build_child:*` with `repo_root`/`workspace_root` set to that edit-harness
workspace.

Root-cause update: the leaf child launch path in
`crates/ploke-eval/src/cli/prototype1_state/c3.rs` spawned the child in the
same process group as the bounded `prototype1-step` parent command. The
successor launch path already isolated successor processes with
`process_group(0)`, but `SpawnChild` did not. In the live run, the child wrote
`ready` and `evaluating`, then its process disappeared without stderr, a runner
result, or a terminal channel message. That matches process-group cleanup after
the parent command returned, not a child-auth or protocol failure.

Source repair: `SpawnChild` now applies the same Unix process-group isolation
before spawning the leaf child. A focused Unix regression probes that the
spawned child gets its own process group.

Disposition for this campaign: stop advancing it for loop evidence. The current
persisted child state is not trustworthy: the parent has a `running` child, the
child process is dead, and the required treatment/runner/channel terminal
evidence is absent.

## Verification

Focused tests:

```text
cargo test -p ploke-eval replay_marks_missing_runner_result_as_stale_or_hung -- --nocapture
cargo test -p ploke-eval replay_observe_child_uses_default_stale_threshold -- --nocapture
cargo test -p ploke-eval stale_observe_before_adds_doctor_blocker -- --nocapture
cargo test -p ploke-eval dead_acknowledged_running_child_adds_doctor_blocker_before_observe -- --nocapture
cargo test -p ploke-eval isolate_process_group_gives_child_own_group -- --nocapture
cargo test -p ploke-eval --lib
```

The 2026-06-08 source repair passed the new process-group regression, the
existing dead-child doctor regression, and the full `ploke-eval` lib suite.
Fresh-campaign live verification is still required before treating this blocker
as cleared for new loop evidence.

Fresh live verification was attempted with campaign
`p1-guided-surface-g35flash-p25flash-5g1x2-a2-pr1-spawnfix-20260608-081204`
from source commit `fc8a463c`. Setup, doctor, headless TUI preflight, live
protocol preflight, and baseline eval passed, but the run stopped in
`baseline_protocol` before child planning or spawn. The blocking evidence is a
direct-Google protocol 429 after a partial segmentation-only protocol artifact;
see `2026-05-22-prototype1-continue-protocol-quota-no-progress-loop.md`.
