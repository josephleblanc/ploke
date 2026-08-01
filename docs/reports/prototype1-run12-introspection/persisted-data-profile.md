# Prototype 1 Run12 Persisted Data Profile

Campaign: `p1-3gen-15nodes-run12`

Root: `/home/brasides/.ploke-eval/campaigns/p1-3gen-15nodes-run12/prototype1`

This profile was produced from persisted files only. The `ploke-eval` CLI was not used.

## Read First

An introspection command should read these files first, in this order:

1. `scheduler.json`
   Canonical run-level state: policy, node set, frontier/completed/failed sets, latest continuation decision, and stop reason.
2. `branches.json`
   Canonical candidate registry: source states, all candidate branches, proposed content hashes, selected branch IDs, and latest evaluation summaries.
3. `transition-journal.jsonl`
   Canonical ordered protocol trace: materialization, build, spawn, child observation, successor selection, checkout advancement, handoff, and completion.
4. `evaluations/*.json`
   Authoritative evaluation payloads for completed selected branches, including compared baseline/treatment records and metric deltas.
5. `nodes/*/{node.json,runner-request.json,runner-result.json,results/*.json,invocations/*.json,successor-*/*.json,streams/*/*.log}`
   Node-local details and stream evidence. Treat `runner-result.json` as a convenience copy of the corresponding `results/<runtime>.json`.

Avoid reading `nodes/*/target/`, `nodes/*/worktree/`, and `nodes/*/bin/` for run introspection unless the command explicitly needs build products or artifact contents.

## Available Data

### Lineage

Authoritative sources:

- `scheduler.json`: 10 nodes total, 3 completed, 7 frontier, 0 failed.
- `transition-journal.jsonl`: ordered state transitions and parent identity handoffs.
- `nodes/*/node.json`: node-local projection of node identity, parent, generation, branch, and status.

Observed selected lineage:

| Generation | Node | Parent | Branch | Status |
| --- | --- | --- | --- | --- |
| 0 | `node-0c854700ebf414fe` | none | `prototype1-parent-p1-3gen-15nodes-run12-gen0` | `planned` |
| 1 | `node-bdfeda081f4451d4` | `node-0c854700ebf414fe` | `branch-50a418fb656c9e04` | `succeeded` |
| 2 | `node-d84bcd2e4ccb3976` | `node-bdfeda081f4451d4` | `branch-5ee5d0b29179ce2f` | `succeeded` |
| 3 | `node-f755d6f7cb56d976` | `node-d84bcd2e4ccb3976` | `branch-617f711f5d4a4a1e` | `succeeded` |

Unrun frontier candidates remain in `scheduler.json` and `nodes/*/node.json`: two generation-1 alternatives, two generation-2 alternatives, and two generation-3 alternatives, plus the root node remains listed in the frontier.

### Candidate Sets

Authoritative source: `branches.json`.

There are 3 source states. Each source state has 3 candidates for `crates/ploke-core/tool_text/read_file.md`:

| Source state | Selected branch | Candidate count | Latest selected evaluation |
| --- | --- | ---: | --- |
| `prototype1-parent-p1-3gen-15nodes-run12-gen0` | `branch-50a418fb656c9e04` | 3 | `keep` |
| `branch-50a418fb656c9e04` | `branch-5ee5d0b29179ce2f` | 3 | `reject` |
| `branch-5ee5d0b29179ce2f` | `branch-617f711f5d4a4a1e` | 3 | `keep` |

`messages/child-plan/*.json` is a summary/manifest for generated child node files. It points to `scheduler.json`, `branches.json`, and the child `node.json`/`runner-request.json` files, but does not own the candidate contents.

### Evaluations

Authoritative source: `evaluations/*.json`.

Completed selected branches:

| Branch | Node | Treatment campaign | Compared instances | Rejected instances | Disposition |
| --- | --- | --- | ---: | ---: | --- |
| `branch-50a418fb656c9e04` | `node-bdfeda081f4451d4` | `p1-3gen-15nodes-run12-treatment-branch-50a418fb656c9e04-1777300823090` | 1 | 0 | `keep` |
| `branch-5ee5d0b29179ce2f` | `node-d84bcd2e4ccb3976` | `p1-3gen-15nodes-run12-treatment-branch-5ee5d0b29179ce2f-1777301014292` | 1 | 1 | `reject` |
| `branch-617f711f5d4a4a1e` | `node-f755d6f7cb56d976` | `p1-3gen-15nodes-run12-treatment-branch-617f711f5d4a4a1e-1777301384939` | 1 | 0 | `keep` |

The generation-2 rejection reason was `same_file_patch_max_streak regressed: 0 -> 1`. The evaluation files also point to baseline/treatment `record.json.gz` paths under `/home/brasides/.ploke-eval/instances/...`; those are outside the campaign root and were not expanded here.

Duplicate/summary sources:

- `branches.json` embeds a compact `latest_evaluation` per selected branch.
- `nodes/*/runner-result.json` and `nodes/*/results/*.json` point back to the evaluation artifact path but do not contain metric comparisons.
- `transition-journal.jsonl` records observe-child success with only `evaluation_artifact_path` and `overall_disposition`.

### Selected Successors

Authoritative source: `transition-journal.jsonl`; run-level latest decision is also summarized in `scheduler.json`.

Observed successor decisions:

| Completed node | Decision | Selected branch | Next generation | Total nodes after continue |
| --- | --- | --- | ---: | ---: |
| `node-bdfeda081f4451d4` | `continue_ready` | `branch-50a418fb656c9e04` | 2 | 4 |
| `node-d84bcd2e4ccb3976` | `continue_ready` | `branch-5ee5d0b29179ce2f` | 3 | 7 |
| `node-f755d6f7cb56d976` | `stop_max_generations` | `branch-617f711f5d4a4a1e` | 4 | 10 |

Note: `branches.json.active_targets[0]` still names `branch-5ee5d0b29179ce2f` as active. For final run stop state, prefer `scheduler.json.last_continuation_decision` and the terminal `successor.selected` journal entry.

### Handoffs

Authoritative source: `transition-journal.jsonl`.

Node-local handoff files:

- `nodes/node-bdfeda081f4451d4/successor-ready/c123a07e-ced5-4b1e-8be1-882e11afc264.json`
- `nodes/node-bdfeda081f4451d4/successor-completion/c123a07e-ced5-4b1e-8be1-882e11afc264.json`
- `nodes/node-d84bcd2e4ccb3976/successor-ready/6e33cce9-20b1-4c75-a3be-ee7b1c553892.json`
- `nodes/node-d84bcd2e4ccb3976/successor-completion/6e33cce9-20b1-4c75-a3be-ee7b1c553892.json`

These node-local files are authoritative for ready/completion acknowledgements at their exact path, while the journal is authoritative for order and context. Handoff streams are under `nodes/<node>/streams/<successor-runtime>/{stdout,stderr}.log`.

### Stdout/Stderr Streams

Authoritative source for process output: `nodes/*/streams/*/{stdout,stderr}.log`.

There are 5 runtime stream pairs:

| Node | Runtime | Role | Stdout | Stderr | Summary |
| --- | --- | --- | ---: | ---: | --- |
| `node-bdfeda081f4451d4` | `be8bbb4b-34e9-4ddc-93e9-79a20b436f5a` | child eval | 14 lines / 613 bytes | 22 lines / 13426 bytes | `branch-50...` evaluation, `keep`, total 101.064s |
| `node-bdfeda081f4451d4` | `c123a07e-ced5-4b1e-8be1-882e11afc264` | successor | 21 lines / 1404 bytes | 10 lines / 596 bytes | child-plan generation, successor handoff acknowledged |
| `node-d84bcd2e4ccb3976` | `d6da69cb-fee2-446f-a5b4-8ec3ffb3ef0f` | child eval | 14 lines / 613 bytes | 24 lines / 14083 bytes | `branch-5ee...` evaluation, `reject`, total 268.119s |
| `node-d84bcd2e4ccb3976` | `6e33cce9-20b1-4c75-a3be-ee7b1c553892` | successor | 19 lines / 1075 bytes | 10 lines / 596 bytes | child-plan generation, terminal stop handoff skipped |
| `node-f755d6f7cb56d976` | `539d96b4-982d-46f6-85ee-1ae70fec0917` | child eval | 14 lines / 613 bytes | 24 lines / 14324 bytes | `branch-617...` evaluation, `keep`, total 447.149s |

Stdout carries JSON result summaries or successor outcome summaries. Stderr carries timing spans plus warnings; it may include verbose prompt/context material and should be summarized by pattern rather than emitted wholesale.

### Stop Reason

Authoritative source: `scheduler.json.last_continuation_decision`, corroborated by the terminal `successor.selected` entry in `transition-journal.jsonl`.

Stop reason: `stop_max_generations`.

Policy: `max_generations = 3`, `max_total_nodes = 15`, `stop_on_first_keep = false`, `require_keep_for_continuation = false`.

Terminal decision selected `branch-617f711f5d4a4a1e` with `next_generation = 4` and `total_nodes_after_continue = 10`, then stopped because generation 4 would exceed the configured maximum generation.

### Timing

Authoritative sources:

- `scheduler.json.updated_at` for final scheduler write time.
- `transition-journal.jsonl.recorded_at` for ordered protocol timestamps.
- `streams/*/stderr.log` for phase durations emitted by each runtime.

Observed timeline:

| Event | Time UTC | Notes |
| --- | --- | --- |
| Root scheduler node created | 2026-04-27T14:26:38.615025929Z | From `scheduler.json` |
| First journal `parent_started` | 2026-04-27T14:35:26.175Z | From `transition-journal.jsonl` |
| Gen 1 child eval | 2026-04-27T14:40:23Z to 14:42:04Z | 101.064s |
| Gen 1 successor run | 2026-04-27T14:42:10Z to 14:42:26Z | 15.252s |
| Gen 2 child eval | 2026-04-27T14:43:34Z to 14:48:02Z | 268.119s |
| Gen 2 successor run | 2026-04-27T14:48:09Z to 14:48:41Z | 32.084s |
| Gen 3 child eval | 2026-04-27T14:49:44Z to 14:57:12Z | 447.149s |
| Final scheduler update | 2026-04-27T14:57:12.197640990Z | From `scheduler.json` |
| Final successor completion journal entry | 2026-04-27T14:57:12.198Z | Completion of generation-2 successor runtime |

The elapsed time from root scheduler node creation to final scheduler update is about 30m33s. The elapsed time from first journal parent start to final successor completion is about 21m46s.

## Authority Map

| Question | Read first | Treat as duplicate/summary |
| --- | --- | --- |
| What nodes exist and what stopped the run? | `scheduler.json` | `nodes/*/node.json`, successor stdout summaries |
| What candidates existed and which branch was selected? | `branches.json` | `messages/child-plan/*.json`, planned-node files |
| What happened in order? | `transition-journal.jsonl` | node-local ready/completion files |
| What was evaluated and why was it kept/rejected? | `evaluations/*.json` | `branches.json.latest_evaluation`, runner result files, observe-child journal summaries |
| What process wrote a result? | `transition-journal.jsonl`, `nodes/*/invocations/*.json` | stdout JSON summaries |
| What did a runtime print? | `nodes/*/streams/*/*.log` | `runner-result.json` |
| Did a successor handoff happen? | `transition-journal.jsonl` | `successor-ready/*.json`, `successor-completion/*.json`, successor stdout |
