# Run State / Lineage Review

Campaign: `p1-smoke-broad-harness-1x3-20260519-1`

Reviewed: 2026-05-19

Scope: run-state, lineage, metadata consistency, and artifact presence. This pass used bounded summaries only; it did not run `prototype1-state`, `prototype1-step`, or `prototype1-continue`, and it did not dump full traces.

## Verdict

The run appears finished in the operational/policy sense, but not because `scheduler.json` says so. The decisive surfaces are fresher node records, runner results, `transition-journal.jsonl`, sealed History blocks, and the active worktree branch.

The run reached max generation 2, evaluated six child treatment nodes, sealed two History successor decisions, advanced the active checkout to the selected generation-2 successor, and has no currently running `ploke-eval loop prototype1-state` process visible in the bounded host-process probe. `scheduler.json` is stale: it still shows only the root node as planned/frontier at `2026-05-19T09:01:09Z`.

Closure/protocol readiness is a separate earlier surface: `closure-state.json` says eval and protocol were complete at `2026-05-19T09:10:59Z`. That proves the baseline closure gate, not the final run lineage.

## Node Map

| Generation | Node | Parent / Source | Branch | Candidate | Node status | Runner result | Evaluation | History role |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 0 | `node-a87394840086768d` | `prototype1-root:p1-smoke-broad-harness-1x3-20260519-1` | `prototype1-parent-p1-smoke-broad-harness-1x3-20260519-1-gen0` | `root-parent` | `running` | missing | baseline closure only | opened/sealed block 0 as parent authority |
| 1 | `node-49f627d37985c53e` | `node-a87394840086768d` / gen0 branch | `branch-d43eb69306af2401` | `broad-harness-g1-01` | `succeeded` | `succeeded` | `reject` | considered in block 0 |
| 1 | `node-26e5f6b29d99238e` | `node-a87394840086768d` / gen0 branch | `branch-e707a012bdd47f71` | `broad-harness-g1-02` | `succeeded` | `succeeded` | `reject` | considered in block 0 |
| 1 | `node-81bd26e4b6222d08` | `node-a87394840086768d` / gen0 branch | `branch-1e4da15f47e52f34` | `broad-harness-g1-03` | `running` | `succeeded` | `reject` | selected in block 0; then parent authority for block 1 |
| 2 | `node-8fde8d78b873fafa` | `node-81bd26e4b6222d08` / `branch-1e4da15f47e52f34` | `branch-b1c602731878d8ff` | `broad-harness-g2-01` | `succeeded` | `succeeded` | `keep` | selected in block 1; active checkout head |
| 2 | `node-db394066dad6eaf9` | `node-81bd26e4b6222d08` / `branch-1e4da15f47e52f34` | `branch-dafe57f7e75214ad` | `broad-harness-g2-02` | `succeeded` | `succeeded` | `reject` | considered in block 1 |
| 2 | `node-21eaf04f2c01c6f2` | `node-81bd26e4b6222d08` / `branch-1e4da15f47e52f34` | `branch-d68716f09d5982ae` | `broad-harness-g2-03` | `succeeded` | `succeeded` | `keep` | considered in block 1 |

## Surface Comparison

### `scheduler.json`

`scheduler.json` is stale and should not be used as the run-health authority here.

- `updated_at`: `2026-05-19T09:01:09.049182723+00:00`
- `nodes`: one root node only
- `frontier_node_ids`: `node-a87394840086768d`
- root status: `planned`
- no completed or failed nodes

This conflicts with later node records, runner results, transition journal entries, evaluations, and sealed History blocks.

### Node Records And Runner Results

The node directory contains seven node records. Six child treatment nodes have `runner-result.json` with `status=succeeded` and `exit_code=0`. The root parent node has no `runner-result.json`, which is expected for the initial parent role but leaves its `node.json` status as `running`.

The selected generation-1 node, `node-81bd26e4b6222d08`, is internally inconsistent as a flat node record: `node.json` still says `running`, while its `runner-result.json` says `succeeded`, it has a child plan, it emitted parent/child channel records, and History block 1 was opened by `parent:node-81bd26e4b6222d08`. Treat this as stale node-status projection, not as live child execution.

### Transition Journal

`transition-journal.jsonl` has 102 entries. Kind counts:

- `parent_started`: 3
- `resource`: 5
- `materialize_branch`: 12
- `spawn_child`: 18
- `child`: 18
- `build_child`: 12
- `observe_child`: 12
- `child_artifact_committed`: 6
- `successor`: 12
- `active_checkout_advanced`: 2
- `successor_handoff`: 2

The tail records show generation-2 child observation followed by successor selection and handoff for `node-8fde8d78b873fafa`, then a new parent start and successor completion/resource entries. This matches History block 1 and the active worktree branch.

### History Blocks

History authority exists and is compact: two block records plus matching hash and lineage-height indexes.

- block height 0, hash `6150145232070a5ee812989231086a599432345fcb4fd3a14d9080d9995ab30f`
  - opened by `parent:node-a87394840086768d`
  - selected parent identity: `node-81bd26e4b6222d08`, generation 1, branch `branch-1e4da15f47e52f34`
  - selected artifact branch: `prototype1-broad-broad-harness-request-node-a87394840086768d-r7`
- block height 1, hash `ac95fcff9fcdf39831bb147fb81018b58cb13e5819a2b7249e3add04a92c0ed0`
  - parent block: height 0 hash above
  - opened by `parent:node-81bd26e4b6222d08`
  - selected parent identity: `node-8fde8d78b873fafa`, generation 2, branch `branch-b1c602731878d8ff`
  - selected artifact branch: `prototype1-broad-broad-harness-request-node-81bd26e4b6222d08`

`history/index/heads.json` points the campaign lineage to block 1 hash `ac95fcff...`, so sealed History says the latest selected successor is `node-8fde8d78b873fafa`.

### Evaluations

There are six branch evaluation records, one per treatment child:

| Branch | Node | Disposition | Compared instances | Baseline calls/failures | Treatment calls/failures | Patch state |
| --- | --- | --- | --- | --- | --- | --- |
| `branch-d43eb69306af2401` | `node-49f627d37985c53e` | `reject` | 1 | 17 / 3 | 20 / 0 | baseline applied/nonempty, treatment no/empty |
| `branch-e707a012bdd47f71` | `node-26e5f6b29d99238e` | `reject` | 1 | 17 / 3 | 28 / 0 | baseline applied/nonempty, treatment no/empty |
| `branch-1e4da15f47e52f34` | `node-81bd26e4b6222d08` | `reject` | 1 | 17 / 3 | 28 / 0 | baseline applied/nonempty, treatment no/empty |
| `branch-b1c602731878d8ff` | `node-8fde8d78b873fafa` | `keep` | 1 | 28 / 0 | 22 / 0 | both no/empty |
| `branch-dafe57f7e75214ad` | `node-db394066dad6eaf9` | `reject` | 1 | 28 / 0 | 29 / 4 | both no/empty |
| `branch-d68716f09d5982ae` | `node-21eaf04f2c01c6f2` | `keep` | 1 | 28 / 0 | 28 / 0 | both no/empty |

History selected the rejected generation-1 candidate despite `overall_disposition=reject`; that is consistent with the admitted search profile allowing continuation from rejected branches (`require_keep_for_continuation=false`, `explore_from_rejected=true`). At generation 2, History selected a `keep` candidate.

### Child Plans

There are two child-plan records:

- `node-a87394840086768d.json`: three planned generation-1 children, matching the three gen1 node records.
- `node-81bd26e4b6222d08.json`: three planned generation-2 children, matching the three gen2 node records.

No child plan was found for `node-8fde8d78b873fafa`; that is consistent with `max_generations=2` and the active checkout being advanced to the selected generation-2 successor rather than scheduling generation 3.

### Edit-Harness Requests And Results

The message directory contains 18 JSON edit-harness requests: nine request variants for root parent `node-a87394840086768d` and nine for selected gen1 parent `node-81bd26e4b6222d08`.

Result presence is partial at the request-variant level:

- `node-a87394840086768d`: 9 request JSON files, 3 submitted-result JSON files, 3 headless-TUI trace JSON files.
- `node-81bd26e4b6222d08`: 9 request JSON files, 4 submitted-result JSON files, 8 headless-TUI trace JSON files.
- request names with no matching result basename:
  - `node-81bd26e4b6222d08-r4`
  - `node-a87394840086768d-r3`
  - `node-a87394840086768d-r4`
  - `node-a87394840086768d-r5`
  - `node-a87394840086768d-r6`
  - `node-a87394840086768d-r8`
  - `node-a87394840086768d-r9`

Headless terminal summaries found:

- root parent variants: `node-a...`, `node-a...-r2`, and `node-a...-r7` were `applied`.
- gen1 parent variants: `node-81...`, `-r2`, `-r3`, and `-r9` were `applied`; `-r5`, `-r6`, `-r7`, and `-r8` timed out; `-r4` has no result.

The existence of missing or timed-out request variants does not by itself invalidate the six node-level results. It does show that per-request trace availability is incomplete and should not be conflated with the child-node success/evaluation surface.

### Trace Availability

Per-node channel traces exist under `prototype1/nodes/<node>/channels/<runtime>/child-to-parent.jsonl`:

- gen1 and gen2 treatment children each have a primary runtime channel with three lines.
- selected parent-role nodes also have successor channels:
  - `node-81bd26e4b6222d08` has a three-line child channel for its treatment runtime plus a two-line parent/successor channel.
  - `node-8fde8d78b873fafa` has a three-line treatment channel plus a two-line successor-completion channel.

Agent-turn trace artifacts exist for the baseline run and all six treatment branch runs under the instances tree: each has `record.json.gz`, `agent-turn-summary.json`, and `agent-turn-trace.json`.

## Finished In What Sense

- Closure/protocol readiness: complete before the loop work, per `closure-state.json` (`eval.status=complete`, `protocol.status=complete`, updated `2026-05-19T09:10:59Z`).
- Headless candidate attempts: partial by request variant; 18 requests, 11 headless traces, 7 request basenames without a result basename, and 3 timed-out gen1-parent headless traces.
- Node success: six treatment child nodes succeeded with runner results; root parent lacks runner result; selected gen1 parent has stale `node.json` status despite runner success and parent-role evidence.
- History admission: complete through two sealed blocks; latest head is height 1, selecting `node-8fde8d78b873fafa`.
- Successor selection: performed twice. Generation 1 selected `node-81bd26e4b6222d08`; generation 2 selected `node-8fde8d78b873fafa`.
- Runtime/process liveness: no current `ploke-eval` process running `loop prototype1-state` was found by the bounded process-name probe.
- Policy completion: the active worktree is on `prototype1-broad-broad-harness-request-node-81bd26e4b6222d08` at commit `663248e8`, matching the generation-2 selected successor branch/hash path. With `max_generations=2`, this is a finished run in the search-policy sense.

## Mismatches / Risks

1. `scheduler.json` is stale and contradicts every fresher runtime surface. It shows only root planned/frontier and should be treated as a projection bug for this run.
2. `node-a87394840086768d/node.json` remains `running` and has no runner result, even though it opened/sealed History block 0 and produced child plans/results.
3. `node-81bd26e4b6222d08/node.json` remains `running` even though its runner result succeeded, it was selected into History block 0, it generated/evaluated gen2 children, and it opened/sealed History block 1.
4. Edit-harness request/result presence is incomplete at the request-variant level. Seven request basenames have no matching result basename, and three gen1-parent headless traces timed out.
5. Projection files and closure metadata are not Crown/History authority. The actual lineage authority in this pass is the two-block History chain and its indexes.
6. The branch evaluation records and History selection are consistent, but the generation-1 selection is easy to misread: the selected branch was rejected operationally, and continuation was allowed by policy rather than by a `keep`.

## Smallest Verification Commands

```bash
jq '{updated_at, frontier_node_ids, completed_node_ids, failed_node_ids, node_count:(.nodes|length), nodes}' \
  /home/brasides/.ploke-eval/campaigns/p1-smoke-broad-harness-1x3-20260519-1/prototype1/scheduler.json
```

```bash
for f in /home/brasides/.ploke-eval/campaigns/p1-smoke-broad-harness-1x3-20260519-1/prototype1/nodes/*/node.json; do
  jq -r '[(input_filename|split("/")[-2]), .generation, (.source_state_id // "-"), .status, .branch_id, .candidate_id] | @tsv' "$f"
done | sort -k2,2n -k1,1
```

```bash
for f in /home/brasides/.ploke-eval/campaigns/p1-smoke-broad-harness-1x3-20260519-1/prototype1/nodes/*/runner-result.json; do
  jq -r '[(input_filename|split("/")[-2]), .status, .exit_code] | @tsv' "$f"
done | sort
```

```bash
wc -l \
  /home/brasides/.ploke-eval/campaigns/p1-smoke-broad-harness-1x3-20260519-1/prototype1/transition-journal.jsonl \
  /home/brasides/.ploke-eval/campaigns/p1-smoke-broad-harness-1x3-20260519-1/prototype1/history/blocks/segment-000000.jsonl
```

```bash
sed -n '1,2p' \
  /home/brasides/.ploke-eval/campaigns/p1-smoke-broad-harness-1x3-20260519-1/prototype1/history/blocks/segment-000000.jsonl |
  jq -r '[.state.header.common.block_height, .state.header.selected_parent_identity.node_id, .state.header.selected_parent_identity.generation, .state.header.selected_parent_identity.previous_parent_id, .state.header.selected_parent_identity.branch_id, .state.header.block_hash] | @tsv'
```

```bash
for f in /home/brasides/.ploke-eval/campaigns/p1-smoke-broad-harness-1x3-20260519-1/prototype1/evaluations/*.json; do
  jq -r '[.branch_id, .overall_disposition, (.compared_instances|length), .compared_instances[0].status] | @tsv' "$f"
done | sort
```

```bash
for f in /home/brasides/.ploke-eval/campaigns/p1-smoke-broad-harness-1x3-20260519-1/prototype1/messages/child-plan/*.json; do
  jq -r '[(input_filename|split("/")[-1]), .parent_node_id, .child_generation, (.children|length), (.children | map(.node.node_id+":"+.node.branch_id+":"+.node.candidate_id+":"+(.node.status)) | join(","))] | @tsv' "$f"
done | sort
```

```bash
ps -eo pid=,comm=,args= | awk '$2 == "ploke-eval" && /loop prototype1-state/ {print $1, $2, "prototype1-state"}'
```
