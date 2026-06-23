# Prototype 1 Persistence Inventory and Cozo Map

This page inventories where the current Prototype 1 loop writes data and starts
a Cozo schema map for moving durable state away from ad hoc files.

Scope: current live `loop walk`, `prototype1-state`, broad headless-TUI, child
runner, and successor handoff paths. Classify paths by ownership and purpose,
not only by physical prefix: an evaluated checkout may itself live below
`$PLOKE_EVAL_HOME/worktrees`, but checkout writes are still Artifact/workspace
writes, not campaign store writes.

This is a planning document. It does not change authority rules or import
semantics.

## Authority classes

| Class | Meaning | Examples |
| --- | --- | --- |
| Sealed authority | Append-only History facts that may advance lineage. | `prototype1/history/blocks/*` via `BlockStore::append` |
| Admitted configuration | Operator policy admitted with a digest. | `run-profile.toml`, `run-profile.commitment.json` |
| Channel evidence | Attempt-scoped parent/child runtime messages. | `nodes/<node>/channels/<runtime>/*.jsonl` |
| Transition evidence | Append-only transition replay/debug events. | `transition-journal.jsonl` |
| Projections | Mutable or rebuildable views over authority/evidence. | `scheduler.json`, node JSON, `heads.json` |
| Workspace artifacts | Git checkouts, worktrees, branches, commits, build outputs. | active checkout, broad harness worktrees, child `bin/` |
| Diagnostics | Logs and operator/debug traces. | `~/.ploke-eval/logs`, stream logs, tool-loop debug |

## Location inventory

### Active parent checkout

The active parent checkout is the `--repo-root` for live parent control. It is
also a Git Artifact surface.

| Path or action | Written by loop? | Git-tracked? | Notes |
| --- | --- | --- | --- |
| `.ploke/prototype1/parent_identity.json` | Yes | Yes | Checkout-local parent identity. Written during setup and successor handoff, then committed. |
| Git branch state | Yes | Git metadata | Setup creates a parent branch such as `prototype1-parent-<campaign>-gen0`; handoff switches to the selected Artifact branch. |
| `<repo-root>/target/` | Yes | No | Successor handoff builds `ploke-eval` in the active checkout; `target/debug/ploke-eval` is runtime build output. |
| Source files under the repo | Usually no direct parent write | Sometimes | Selected child/broad harness Artifact content becomes visible after `git switch` during handoff. |
| Git common dir | Indirectly | Git metadata | Refs, objects, worktree metadata, and stash data may live outside the visible checkout. |

If `PLOKE_EVAL_HEADLESS_TUI_STASH_TRANSFER` is enabled, headless TUI edits may
briefly exist in the source checkout and then be transferred to a candidate
worktree with git stash operations.

### Candidate and child worktrees

| Path | Written by loop? | Git-tracked? | Notes |
| --- | --- | --- | --- |
| `prototype1/nodes/<node>/worktree/<target_relpath>` | Yes | Committed if promoted | Direct child realization writes proposed content in a node-owned worktree. |
| `prototype1/workspaces/edit-harness/<parent-node-id>[-rN]/` | Yes | Admitted changed paths are committed | Broad harness candidate workspaces. The TUI edits files here. |
| `prototype1/nodes/<node>/target/` | Yes | No | Child build scratch target dir. Cleanup-managed. |
| `prototype1/nodes/<node>/bin/ploke-eval` | Yes | No | Copied child runtime binary after successful build. Cleanup-managed. |

### `.ploke` checkout metadata

The loop-owned `.ploke` write is currently:

```text
.ploke/prototype1/parent_identity.json
```

It records the campaign, parent/node id, generation, branch id, Artifact branch,
and predecessor links. It is committed into parent-capable Artifacts so a
hydrated runtime can recover its coordinate from the checkout.

### Campaign state under `$PLOKE_EVAL_HOME`

`$PLOKE_EVAL_HOME` defaults to `~/.ploke-eval`.

```text
$PLOKE_EVAL_HOME/campaigns/<campaign>/
  campaign.json
  closure-state.json
  prototype1-loop-trace.json
  prototype1/
    run-profile.toml
    run-profile.commitment.json
    scheduler.json
    branches.json
    transition-journal.jsonl
    evaluations/<branch-id>.json
    history/
      blocks/segment-000000.jsonl
      index/by-hash.jsonl
      index/by-lineage-height.jsonl
      index/heads.json
    messages/
      child-plan/<parent-node-id>.json
      edit-harness-request/<node[-rN]>.json
      edit-harness-request/<node[-rN]>.md
      edit-harness-result/<node[-rN]>.json
      edit-harness-result/<node[-rN]>.headless-tui.json
      edit-harness-result/<node[-rN]>.turn-live/
      pre-child-planning/*.json
      pre-child-planning/*.prompt.md
    nodes/<node>/
      node.json
      runner-request.json
      runner-result.json
      results/<runtime-id>.json
      invocations/<runtime-id>.json
      channels/<runtime-id>/child-to-parent.jsonl
      channels/<runtime-id>/parent-to-child.jsonl
      streams/<runtime-id>/stdout.log
      streams/<runtime-id>/stderr.log
      bin/ploke-eval
      target/
      worktree/
    workspaces/edit-harness/<...>/
    debug/tool-loop/
```

| Path | Role | Cozo priority |
| --- | --- | --- |
| `campaign.json` | Campaign configuration anchor. | Medium |
| `closure-state.json` | Baseline/target closure state. | Medium |
| `prototype1/run-profile.toml` | Admitted operator policy. | Medium |
| `prototype1/run-profile.commitment.json` | Digest commitment for the admitted profile. | Medium |
| `prototype1/history/blocks/*` | Sealed History authority. | Highest |
| `prototype1/history/index/*` | Rebuildable History indexes/head projection. | High, as projections |
| `prototype1/transition-journal.jsonl` | Append-only transition replay evidence. | High |
| `prototype1/messages/child-plan/*` | Parent-owned child-plan message box. | High |
| `prototype1/nodes/*/channels/*/*.jsonl` | Attempt-scoped channel evidence. | High |
| `prototype1/nodes/*/invocations/*` | Attempt bootstrap contracts. | High |
| `prototype1/nodes/*/results/*` | Attempt runner-result mirrors. | High |
| `prototype1/evaluations/*` | Branch evaluation reports. | Medium |
| `prototype1/branches.json` | Branch registry stream/projection. | Medium |
| `prototype1/scheduler.json` and `nodes/*/node.json` | Scheduler/node projections. | Medium |
| `prototype1/messages/edit-harness-*` | Edit request/result evidence and diagnostics. | Medium |
| `prototype1/workspaces/*`, `nodes/*/worktree` | Git workspace artifacts. | Keep as Git/path refs initially |
| `prototype1/debug/tool-loop/*` | Tool-loop debug checkpoints. | Low/medium |
| `nodes/*/streams/*` | Process stdout/stderr diagnostics. | Low |
| `nodes/*/target`, `nodes/*/bin` | Build products. | Keep as file artifacts initially |

### Other `$PLOKE_EVAL_HOME` writes

| Path | Role | Notes |
| --- | --- | --- |
| `logs/ploke_eval_<run>.log` | Eval tracing log. | Diagnostic. |
| `logs/llm_full_response_<run>.log` | Full response JSONL target. | Diagnostic/evidence depending on citation. |
| `logs/prototype1_observation_<run>.jsonl` | Prototype 1 observation JSONL when enabled. | Diagnostic/evidence. |
| `records/mirror.cozo.sqlite` | Existing passive record mirror. | Separate from the proposed Prototype 1 store migration. |
| `instances/...` | Per-instance baseline/treatment run roots. | Still referenced by branch evaluation evidence. |
| `registries/runs/<run-id>.json` | Run registration records. | Per-run lifecycle registry. |
| `protocol/...` | Protocol artifacts for runs. | Primary protocol artifact location for instance run dirs. |
| `cache/starting-dbs/<key>.sqlite` and `<key>.json` | Starting DB cache. | Cache; not authority. |
| `last-run.json` | Operator convenience pointer. | Projection only. |

### Runtime and temp writes

| Location | Role |
| --- | --- |
| `$PLOKE_EVAL_WALK_SOCKET_DIR` | Walk socket, context, and IPC files when set. |
| `$XDG_RUNTIME_DIR/ploke-eval/walk` | Default walk runtime dir when XDG runtime is set. |
| `/tmp/ploke-eval-$USER/walk` | Fallback walk runtime dir. |
| `PLOKE_PROTOTYPE1_TRACE_JSONL` target | Optional observation JSONL override; may be outside `$PLOKE_EVAL_HOME`. |
| Shell redirects/PID files | Operator-created, not loop-owned durable state. |

## Git-tracked summary

Committed by the loop:

- `.ploke/prototype1/parent_identity.json` on setup and handoff.
- Admitted broad harness changed paths on a `prototype1-broad-*` branch.
- Direct child target files when the child Artifact is persisted/promoted.

Not committed by the loop:

- campaign files under `$PLOKE_EVAL_HOME/campaigns/<campaign>/...`
- History segment/index files
- scheduler, branch registry, node, request, result, invocation, and channel files
- logs, streams, tool-loop debug files
- `target/` build output and copied `bin/ploke-eval` binaries

## Cozo schema map

### Migration principles

1. Start with sealed History. `BlockStore` is the authority seam; scheduler,
   branch registry, node files, and `heads.json` are projections or evidence.
2. Preserve History domain strings and raw hash preimages. A Cozo backend must
   store the raw sealed block JSON and raw entry JSON used for `verify_hash()`;
   do not depend only on deserialize/serialize round trips.
3. Keep append-only evidence append-only. Mutable views may be materialized, but
   their authority must be derivable from accepted events or blocks.
4. Keep transport separate from storage. `Channel<R, T: Transport>` and
   `FileTransport` may later gain a database transport, but that is not the
   same migration as History/storage persistence.
5. Dual-write before cutover. Add Cozo writes beside existing file writes,
   replay from files into Cozo, compare hashes, then switch readers.
6. Do not silently tolerate schema drift. If old fixtures miss relations, prefer
   regeneration or an explicit migration path.

### Common columns

Use semantic IDs at relation boundaries rather than flattened path-only keys.
Most relations should carry:

| Column | Use |
| --- | --- |
| `campaign_id` | Campaign namespace. |
| `node_id` | Prototype 1 node when scoped to a node. |
| `runtime_id` | Attempt/runtime namespace when scoped to an invocation. |
| `branch_id` | Treatment branch namespace when scoped to branch evidence. |
| `lineage_id` | History lineage key. |
| `schema` | Serialized schema version. |
| `source_path` | Original file path for dual-write/replay. |
| `raw_json` or `raw_text` | Canonical stored payload/preimage when needed. |
| `raw_sha256` | Digest of the stored raw bytes/text. |
| `recorded_at` | Producer timestamp when present. |
| `ingested_at` | Cozo import/write timestamp. |

### Phase 1: History authority

Initial Cozo-backed `BlockStore` relations:

```text
:create p1_history_block {
  campaign_id: String,
  lineage_id: String,
  block_hash: String
  =>
  block_height: Int,
  prev_hash: String?,
  state_root: String,
  entries_root: String,
  entry_count: Int,
  raw_json: String,
  raw_sha256: String,
  source_path: String?,
  source_line: Int?,
  appended_at: String,
}

:create p1_history_entry {
  block_hash: String,
  entry_index: Int
  =>
  entry_hash: String,
  selection_hash: String?,
  entry_kind: String,
  node_id: String?,
  branch_id: String?,
  runtime_id: String?,
  raw_json: String,
  raw_sha256: String,
}

:create p1_history_head {
  campaign_id: String,
  lineage_id: String
  =>
  block_hash: String,
  block_height: Int,
  state_root: String,
  updated_at: String,
}
```

`p1_history_head` is a projection. The sealed block stream remains authority.
A Cozo implementation should verify append eligibility from the current stored
lineage state in the same transaction that inserts the new block and updates the
head projection.

### Phase 2: transition and runtime evidence

```text
:create p1_transition_event {
  campaign_id: String,
  event_index: Int
  =>
  event_kind: String,
  phase: String?,
  node_id: String?,
  runtime_id: String?,
  branch_id: String?,
  raw_json: String,
  raw_sha256: String,
  source_path: String?,
  recorded_at: String?,
}

:create p1_invocation {
  campaign_id: String,
  node_id: String,
  runtime_id: String
  =>
  role: String,
  argv_json: String,
  repo_root: String?,
  workspace_root: String?,
  raw_json: String,
  raw_sha256: String,
  source_path: String?,
}

:create p1_channel_msg {
  campaign_id: String,
  node_id: String,
  runtime_id: String,
  direction: String,
  msg_index: Int
  =>
  msg_id: String,
  body_hash: String,
  body_kind: String,
  raw_json: String,
  raw_sha256: String,
  source_path: String?,
  recorded_at: String?,
}

:create p1_runner_result {
  campaign_id: String,
  node_id: String,
  runtime_id: String?
  =>
  disposition: String,
  branch_id: String?,
  treatment_id: String?,
  raw_json: String,
  raw_sha256: String,
  source_path: String?,
  recorded_at: String?,
}
```

Keep `runtime_id = null`/absent support for legacy latest-result projections,
but prefer attempt-scoped rows for new writes.

### Phase 3: projections and planning records

```text
:create p1_node {
  campaign_id: String,
  node_id: String
  =>
  generation: Int,
  parent_node_id: String?,
  branch_id: String,
  candidate_id: String,
  status: String,
  target_path: String,
  workspace_root: String,
  raw_json: String,
  raw_sha256: String,
  source_path: String?,
  updated_at: String?,
}

:create p1_child_plan {
  campaign_id: String,
  parent_node_id: String
  =>
  child_count: Int,
  raw_json: String,
  raw_sha256: String,
  source_path: String?,
  recorded_at: String?,
}

:create p1_child_plan_member {
  campaign_id: String,
  parent_node_id: String,
  node_id: String
  =>
  branch_id: String,
  candidate_id: String,
  target_path: String,
}

:create p1_branch_record {
  campaign_id: String,
  event_index: Int
  =>
  body_kind: String,
  branch_id: String?,
  parent_branch_id: String?,
  raw_json: String,
  raw_sha256: String,
  source_path: String?,
  recorded_at: String?,
}

:create p1_branch_eval {
  campaign_id: String,
  branch_id: String
  =>
  disposition: String,
  compared_count: Int,
  rejected_count: Int,
  raw_json: String,
  raw_sha256: String,
  source_path: String?,
  recorded_at: String?,
}
```

These relations support diagnosis and successor selection inputs, but they do
not replace sealed History authority.

### Phase 4: edit harness, workspaces, logs, and caches

```text
:create p1_harness_request {
  campaign_id: String,
  request_id: String
  =>
  parent_node_id: String,
  workspace_root: String,
  prompt_text: String?,
  raw_json: String,
  raw_sha256: String,
  source_path: String?,
}

:create p1_harness_result {
  campaign_id: String,
  request_id: String,
  attempt: Int
  =>
  status: String,
  workspace_root: String,
  changed_paths: String,
  raw_json: String,
  raw_sha256: String,
  source_path: String?,
}

:create p1_artifact_ref {
  campaign_id: String,
  artifact_id: String
  =>
  kind: String,
  git_commit: String?,
  git_branch: String?,
  workspace_root: String?,
  target_path: String?,
  content_hash: String?,
}

:create p1_diag_ref {
  campaign_id: String,
  diag_id: String
  =>
  kind: String,
  node_id: String?,
  runtime_id: String?,
  source_path: String,
  raw_sha256: String?,
  byte_len: Int?,
  recorded_at: String?,
}
```

Large logs, copied binaries, Cargo build output, and Git worktrees should stay as
file/Git artifacts at first. Store references, hashes, and lifecycle metadata in
Cozo rather than moving every byte into the database immediately.

## Current file-to-relation map

| Current file/prefix | Initial relation(s) | Notes |
| --- | --- | --- |
| `history/blocks/segment-000000.jsonl` | `p1_history_block`, `p1_history_entry` | Store raw JSON preimages and verify hashes during import. |
| `history/index/heads.json` | `p1_history_head` | Projection only; rebuild from blocks. |
| `transition-journal.jsonl` | `p1_transition_event` | Preserve event index and raw line. |
| `nodes/*/channels/*/*.jsonl` | `p1_channel_msg` | Direction and runtime id are part of the key. |
| `nodes/*/invocations/*.json` | `p1_invocation` | Attempt bootstrap contract. |
| `nodes/*/results/*.json` | `p1_runner_result` | Attempt-scoped terminal evidence mirror. |
| `nodes/*/runner-result.json` | `p1_runner_result` | Latest projection; retain path/source role. |
| `nodes/*/node.json` | `p1_node` | Projection of scheduler/node state. |
| `messages/child-plan/*.json` | `p1_child_plan`, `p1_child_plan_member` | Parent-owned child-plan box. |
| `branches.json` | `p1_branch_record` | Append-only branch record stream. |
| `evaluations/*.json` | `p1_branch_eval` | Branch comparison reports. |
| `messages/edit-harness-request/*` | `p1_harness_request` | JSON request plus prompt text. |
| `messages/edit-harness-result/*` | `p1_harness_result`, `p1_diag_ref` | Submitted result, diagnostics, turn-live traces. |
| `run-profile.toml`, commitment | `p1_run_profile` | Add after History; retain TOML text and digest. |
| `.ploke/prototype1/parent_identity.json` | `p1_parent_identity` or History evidence | Checkout-carried Artifact identity; keep Git commit context. |
| `instances/.../runs/...` | `p1_eval_run_ref`, `p1_diag_ref` | Keep per-run artifact files initially; index references. |
| `logs/*` and streams | `p1_diag_ref` | Store refs and hashes first. |

## Open design questions

- Should large raw payloads live in Cozo as strings, in a blob store, or both?
  History block and entry preimages should be in Cozo at least until the hash
  semantics are proven across stores.
- What is the transaction boundary for importing one parent turn: one block, one
  generation, or one campaign replay batch?
- Should branch registry snapshots be normalized immediately or retained as raw
  records plus query-time projections?
- How should legacy latest-result files map when no runtime id is recoverable?
- When channel transport moves away from `FileTransport`, which relation becomes
  transport state and which remains admitted channel evidence?

## First implementation slice

1. Add a `CozoBlockStore` beside `FsBlockStore`.
2. Make `BlockStore` expose backend-neutral append receipts, verified head
   loading, and verified block streaming.
3. Write cross-store tests that append the same sealed blocks to FS and Cozo and
   compare `BlockHash`, `entries_root`, `HistoryStateRoot`, and
   `HistoryHash::of_domain_json` results.
4. Add an explicit import command or test helper that reads existing FS History
   blocks into Cozo and rejects hash/preimage mismatches.
5. Only after History parity is proven, migrate transition/channel/projection
   readers.
