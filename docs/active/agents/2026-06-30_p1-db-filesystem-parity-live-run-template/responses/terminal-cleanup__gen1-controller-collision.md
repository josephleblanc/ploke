# Terminal cleanup after gen1 controller collision

timestamp_before: 2026-06-30T14:55:17-07:00

## processes before
 186748     806     972 Sl   /home/brasides/.ploke-eval/worktrees/p1-gated-parent-3g1x3-p3-20260630-174316/target/debug/ploke-eval loop prototype1-state --campaign p1-gated-parent-3g1x3-p3-20260630-174316 --repo-root /home/brasides/.ploke-eval/worktrees/p1-gated-parent-3g1x3-p3-20260630-174316 --handoff-invocation /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/nodes/node-2fe75acd9e9cf6c3/invocations/a96b5766-b5e0-43a8-99d8-b81936cb5449.json --stop-after complete --format json
 257418  186748     358 Sl   /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/nodes/node-e9be81e08bda07a1/bin/ploke-eval loop prototype1-runner --invocation /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/nodes/node-e9be81e08bda07a1/invocations/22ad6f62-17af-4440-b5a7-b507dc262fe7.json --execute --format json
 258190     806     260 Sl   /home/brasides/.ploke-eval/worktrees/p1-gated-parent-3g1x3-p3-20260630-174316/target/debug/ploke-eval loop walk serve --repo-root /home/brasides/.ploke-eval/worktrees/p1-gated-parent-3g1x3-p3-20260630-174316 --socket /run/user/1000/ploke-eval/walk/p1walk-cface20f07a9eb55.sock --ttl-secs 1800

## action
Sending SIGTERM to successor prototype1-state controller PID 186748 and walk server PID 258190.

timestamp_after_term: 2026-06-30T14:55:22-07:00
## processes after SIGTERM
 257418     806     363 Sl   /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/nodes/node-e9be81e08bda07a1/bin/ploke-eval loop prototype1-runner --invocation /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/nodes/node-e9be81e08bda07a1/invocations/22ad6f62-17af-4440-b5a7-b507dc262fe7.json --execute --format json

## action
Some processes survived SIGTERM; sending SIGKILL to remaining known PIDs.

timestamp_after_kill: 2026-06-30T14:55:24-07:00
## processes after SIGKILL
