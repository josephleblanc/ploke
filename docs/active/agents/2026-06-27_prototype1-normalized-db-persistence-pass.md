# 2026-06-27 Prototype 1 normalized DB persistence pass

**Status:** active plan + audit log; child-plan normalized slice has fresh DB proof; scheduler-node setup rows have fresh DB proof; runner-request setup rows have fresh DB proof; runner-result proof still needs a successful child run
**Purpose:** restart spine for a fresh pass over `ploke-eval loop walk` persistence with normalized Cozo relations as the required target.
**Related:**
- `docs/active/agents/2026-06-22_prototype1-eval-store-data-model/README.md`
- `docs/active/agents/2026-06-17_typestate-loop-driver-plan.md`
- `docs/active/agents/2026-06-17_walk-command-guide.md`
- Current code: `crates/ploke-eval/src/cli/prototype1_state/eval_store/`

## User directive for this pass

The target is **DB-first / filesystem-independent persistence** for Prototype 1 walk state.

Rules for this pass:

1. Normalized `define_eval_schema!` relations are **required** for persisted facts that matter to DB-first inspection or cross-runtime coordination.
2. Existing `eval_record_ref`-only implementations are **not sufficient** coverage and should be replaced by normalized relations plus optional citation/provenance refs.
3. `eval_record_ref` should be treated primarily as a History/Block/provenance/citation layer, not as a way to leave structured state out of the database.
4. The long-term target is no dependency on filesystem state for shared Parent/Child facts.
5. `Channel` may remain a transport system, but any data assumed shared between Parent and Child via Channel must be re-evaluated and likely normalized if it is needed after transport.
6. Every claim that a fact exists in the DB must be confirmed with `ploke-eval loop walk db_query --script <cozo>` against the owner eval DB.
7. Keep this document updated with:
   - persistence matrix findings,
   - query snippets that worked well,
   - query gaps/friction that should influence future CLI/schema improvements.

## Current live run context at pass start

Evidence from the previous pass:

- Campaign: `p1-walk-g35-20260627-061559`
- Active worktree: `/home/brasides/.ploke-eval/worktrees/p1-walk-g35-20260627-061559`
- After successor handoff/restart, active parent is `node-88bcb4fbf7fb309a`.
- Reconstructed phase after restart: `r10 - selection strategy ready` for the successor parent.
- Latest commit before this pass: `7de5adac2 Mirror child plan projections into eval DB`.

Important caveat: the current live run is useful evidence, but it contains backfills and mixed binaries from the previous pass. Use a fresh campaign for final proof after schema/callsite changes.

## Initial active DB relation counts

Collected via `target/debug/ploke-eval loop walk db_query` on the current active campaign after successor replay/backfill.

```text
eval_agent_turn                    0
eval_agent_turn_event              0
eval_apply_event                   3
eval_artifact                      5
eval_artifact_ref                  6
eval_artifact_surface              6
eval_attempt                       6
eval_baseline                      2
eval_baseline_instance             2
eval_baseline_instance_metrics     2
eval_binary_ref                    10
eval_build_event                   10
eval_campaign                      1
eval_campaign_eval_budget          1
eval_campaign_eval_policy          1
eval_campaign_protocol_policy      1
eval_channel_message               17
eval_channel_receipt               14
eval_closure_artifact_ref          11
eval_closure_instance              2
eval_closure_protocol_counts       0
eval_closure_protocol_procedure    6
eval_closure_ref                   2
eval_continuation_decision         1
eval_evaluation                    4
eval_evaluation_instance           4
eval_import_event                  14
eval_invocation                    6
eval_log_ref                       10
eval_message_event                 0
eval_model_exchange                0
eval_operation                     5
eval_patch                         5
eval_profile_commitment            1
eval_record_ref                    58
eval_selection_candidate           1
eval_selection_decision            1
eval_selection_finding             2
eval_selection_score               1
eval_tool_event                    0
eval_trace_event                   0
eval_transition_event              2
```

## Query notebook

### Queries that are useful now

#### Relation inventory

```cozo
::relations
```

Good for a quick inventory and detecting whether a relation exists. Not enough to prove coverage.

#### Column inspection before writing result queries

```cozo
::columns eval_selection_decision
::columns eval_evaluation_instance
```

Useful because column names drifted during schema work. This prevented false assumptions like querying `record_ref` on `eval_selection_decision` when the current relation uses `decision_ref`/`decision_hash`.

#### Per-relation row counts

Pattern:

```cozo
?[count(x)] := *eval_evaluation { evaluation_id: x }
```

Useful for quick smoke checks after a transition. Needs follow-up semantic queries.

#### Selection inspection

```cozo
?[decision_id, parent_id, set_id, procedure_id, selected_node_id, selected_artifact_id, outcome, disposition, decision_ref, decision_hash, recorded_at] :=
  *eval_selection_decision {
    decision_id,
    parent_id,
    set_id,
    procedure_id,
    selected_node_id,
    selected_artifact_id,
    outcome,
    disposition,
    decision_ref,
    decision_hash,
    recorded_at
  }
```

Good for answering “what did the parent select?” without reading files.

#### Evaluation + instance inspection

```cozo
?[evaluation_id, branch_id, treatment_id, disposition, record_ref, recorded_at] :=
  *eval_evaluation { evaluation_id, branch_id, treatment_id, disposition, record_ref, recorded_at }
```

```cozo
?[evaluation_id, instance_id, baseline_run_id, treatment_run_id, baseline_ref, treatment_ref, status, outcome, oracle_ref] :=
  *eval_evaluation_instance {
    evaluation_id,
    instance_id,
    baseline_run_id,
    treatment_run_id,
    baseline_ref,
    treatment_ref,
    status,
    outcome,
    oracle_ref
  }
```

Good for child outcome comparison reports.

#### Runtime/channel progress inspection

```cozo
?[node_id, runtime_id, role, invocation_path] :=
  *eval_invocation { node_id, runtime_id, role, invocation_path }
```

```cozo
?[node_id, runtime_id, message_kind, count(channel_message_id)] :=
  *eval_channel_message { node_id, runtime_id, message_kind, channel_message_id }
```

Good for determining which children started and which emitted ready/evaluating/result transport messages.

### Queries that are useful only as gap detectors

#### `eval_record_ref` family counts

```cozo
?[family, count(record_ref_id)] := *eval_record_ref { family, record_ref_id }
```

This is useful for finding raw-file mirrors, but **not** acceptable as proof of normalized DB coverage. A high count here often means “we still only have payload refs.”

### Query/CLI friction to improve

1. `db_query` is raw Cozo only. It is powerful, but repeated count/columns patterns are verbose. A read-only helper like `db_query --relations-with-counts` could reduce operator friction.
2. Failed field names are useful (`stored relation ... does not have field ...`), but an optional `--explain-columns-on-error` mode would speed iteration.
3. There is no first-class “file coverage” query because many filesystem artifacts are only in `eval_record_ref` or not present at all. This reinforces the need for normalized relations.
4. Relation counts do not show whether rows correspond to the current active parent/generation. Most inspection queries should include `campaign_id`, `parent_id`, `node_id`, `generation`, or `runtime_id` once normalized schemas expose them.
5. For composite/named Cozo relation inspection, prefer explicit `field: variable` bindings or full-column positional queries while iterating. Shorthand is easy to misread, and `::columns <relation>` should be checked before writing proof queries.
6. Important schema-evolution finding: adding a new relation to a restored non-empty Cozo backup can corrupt relation contents in this workflow. Treat existing owner DBs that predate current schema as invalid; regenerate instead of extending them in place.

## 2026-06-27 child-plan normalized slice

Implemented code slice:

- Added normalized relations:
  - `eval_child_plan`
  - `eval_child_plan_child`
  - `eval_child_plan_rejected_attempt`
- `write_child_plan_file` now writes normalized child-plan rows instead of `eval_record_ref.family = "child_plan_file"`.
- Child-plan replay writes the normalized rows too.
- Tests now assert child plans and rejected surface attempts through normalized relations.
- Added a guard so full eval-store schema install fails loudly when an existing owner DB is missing the new child-plan relations, matching the no-in-place-migration policy.

Validation commands run:

```text
cargo fmt --all
cargo test -p ploke-eval existing_owner_db --lib
cargo test -p ploke-eval eval_store_non_agent_schema_scripts_are_stable --lib
cargo test -p ploke-eval prototype1_eval_store_parent_start_db_schema_installs_idempotently --lib
cargo test -p ploke-eval tui_edit_surface_parent_selection_publishes_child_plan --lib
cargo test -p ploke-eval below_min_rejected_attempts_are_persisted_and_recoverable_from_existing_child_plan --lib
cargo build -p ploke-eval
```

DB-query proof attempt on active campaign:

- Rebuilt binary and ran `target/debug/ploke-eval loop walk start --until r10 --format json` to reconstruct without live fanout.
- Confirmed via `db_query` that the new relations existed:

```text
target/debug/ploke-eval loop walk db_query --format json --script '::relations' | jq -r '.rows[].name' | rg '^eval_child_plan'
```

Output:

```text
eval_child_plan
eval_child_plan_child
eval_child_plan_rejected_attempt
```

However, this active DB is **not valid proof** for row correctness. The active owner DB existed before this slice and was extended in place during the proof attempt; queries showed `eval_transition_event` and `eval_child_plan` cross-contaminated with each other's rows.

Compact invalid-row proof:

```text
target/debug/ploke-eval loop walk db_query --format json --script $'?[count(plan_id)] :=\n  *eval_child_plan { plan_id: plan_id, schema_version: schema_version },\n  schema_version != "prototype1-child-plan-file.v1"' | jq '.rows'
```

Output:

```json
[{"count(plan_id)":2}]
```

Example bad active rows:

```json
[
  {
    "plan_id": "20bfcc22318d88158cebc52da536a4cf108d54fb293f0a61986768c59d59d881",
    "schema_version": "node-88bcb4fbf7fb309a",
    "parent_node_id": "d5b8413e-9a72-41ff-9289-7512f46ed7b4",
    "child_generation": "node-88bcb4fbf7fb309a"
  },
  {
    "plan_id": "3d6cd7e4f6a48c92e2eb68cfd47a920bd77607872cd6ed665b0ccc980a98ce89",
    "schema_version": "node-8cef99c13707e561",
    "parent_node_id": "",
    "child_generation": "node-8cef99c13707e561"
  }
]
```

A focused regression reproduced the unsafe relation-addition pattern, so the code now fails loudly for existing DBs missing current child-plan relations. A second regression covers existing `eval_child_plan` rows whose `schema_version` is not `prototype1-child-plan-file.v1`.

Guard proof against the contaminated active DB:

```text
target/debug/ploke-eval loop walk start --until r10 --format json
```

Output excerpt:

```text
database setup failed during 'eval_child_plan_put': eval-store db setup schema.eval_child_plan.version failed: existing eval DB contains eval_child_plan rows with an unsupported schema_version; expected prototype1-child-plan-file.v1; regenerate the owner eval DB instead of reusing this backup
```

Note: `walk db_query` is intentionally raw/read-only and does not run schema installation/hygiene guards; use it to inspect suspect data, not to prove writer acceptance.

Operational consequence:

- Do not continue using `/home/brasides/.ploke-eval/campaigns/p1-walk-g35-20260627-061559/prototype1/eval-store.cozo.sqlite` as authoritative DB proof for this slice.
- Regenerate the active owner DB or start a fresh campaign before claiming DB-row parity for the new child-plan relations.
- The active walk server was stopped after the proof attempt and again after the guard-failure check.

Additional live-run note from the reconstruction status after the proof attempt:

- Reconstructed phase: `r10` for parent `node-88bcb4fbf7fb309a`.
- Current blocker reported by reconstruction: terminal child `node-956e2df24fb0374e` is missing a branch evaluation report; observe recovery is needed before durable R11 reconstruction.

### Fresh DB-backed proof for child-plan rows

Committed code slice: `c8f736546 Mirror child plans into normalized eval DB`.

False start cleaned up:

- Campaign `p1-normchild-g35-20260627-100457` used `storage.eval.backend = "fs"`, so it did not create an owner DB. Its worktree/campaign artifacts were removed.

Fresh proof run:

- Campaign: `p1-normchild-db-20260627-100712`
- Worktree: `/home/brasides/.ploke-eval/worktrees/p1-normchild-db-20260627-100712`
- Profile: `p1-walk-det2g1x3-rg2209-20260626-200752`
- Storage: `dual-strict`
- Parent: `node-7b51248d5a8dc9fe`
- Child-plan phase reached: `r8 - child-plan authority received`
- Server stopped at R8 after proof.

Commands:

```text
git worktree add --detach ~/.ploke-eval/worktrees/p1-normchild-db-20260627-100712 HEAD
cd ~/.ploke-eval/worktrees/p1-normchild-db-20260627-100712
CARGO_TARGET_DIR=/home/brasides/code/ploke/target cargo build -p ploke-eval
/home/brasides/code/ploke/target/debug/ploke-eval loop prototype1-setup \
  --campaign p1-normchild-db-20260627-100712 \
  --profile p1-walk-det2g1x3-rg2209-20260626-200752 \
  --format json
/home/brasides/code/ploke/target/debug/ploke-eval loop walk use ~/.ploke-eval/worktrees/p1-normchild-db-20260627-100712 --format json
/home/brasides/code/ploke/target/debug/ploke-eval loop walk start --until r7 --format json
/home/brasides/code/ploke/target/debug/ploke-eval loop walk step --until r8 --watch --format json
```

Initial fresh DB state before R8:

```text
eval_child_plan                  0
eval_child_plan_child            0
eval_child_plan_rejected_attempt 0
eval_record_ref child_plan_file  0
```

DB proof after R8 via `walk db_query`:

```text
eval_child_plan                  1
eval_child_plan_child            3
eval_child_plan_rejected_attempt 0
eval_record_ref child_plan_file  0
invalid child-plan schema rows   0
```

Normalized parent row:

```json
{
  "campaign_id": "p1-normchild-db-20260627-100712",
  "child_count": 3,
  "child_generation": 1,
  "message_sha256": "f8832b5f7c87bb8598016ccca51784e43e94c62f5ac8eb7c3ce06045939a3056",
  "parent_node_id": "node-7b51248d5a8dc9fe",
  "plan_id": "cfadfd0bd05e15bc1c7433afaf3f971262d565c80cb03d7197d33539a037025f",
  "rejected_count": 0,
  "schema_version": "prototype1-child-plan-file.v1"
}
```

Normalized child rows:

```text
node-96205055dcec2237 child_index=0 status=planned branch=branch-3f5d80faedfd9e69
node-e85be9ac8e1ede12 child_index=1 status=planned branch=branch-fc02fcd57a07391a
node-d5b1a097a253c916 child_index=2 status=planned branch=branch-b10541b38742bbf5
```

Filesystem parity check:

```text
/home/brasides/.ploke-eval/campaigns/p1-normchild-db-20260627-100712/prototype1/messages/child-plan/node-7b51248d5a8dc9fe.json
parent_node_id=node-7b51248d5a8dc9fe
child_generation=1
child_count=3
rejected_count=0
sha256=f8832b5f7c87bb8598016ccca51784e43e94c62f5ac8eb7c3ce06045939a3056
```

Conclusion for this slice: fresh DB-backed proof confirms child-plan authority no longer depends on `eval_record_ref.family = "child_plan_file"`. Scheduler-node and runner-request rows are still payload-ref-only and remain separate gaps.

## 2026-06-27 scheduler-node normalized slice

Implemented code slice:

- Added normalized relations:
  - `eval_scheduler_node`
  - `eval_scheduler_node_status_event`
  - `eval_scheduler_node_target_part`
- `save_node_record` now mirrors every `node.json` projection into the owner DB when an owner DB exists.
- `eval_scheduler_node` stores the current projection row keyed by `(campaign_id, node_id)`.
- `eval_scheduler_node_status_event` stores append-style status observations keyed by a deterministic status event hash.
- `eval_scheduler_node_target_part` stores operation-target parts keyed by `(campaign_id, node_id, content_sha256, target_part, target_index)` so stale target rows cannot masquerade as the current node projection after a future target-shape change.
- Existing owner DBs that predate these relations now fail loudly under the same no-in-place-migration rule as child-plan rows.

Risk/impact notes:

- GitNexus marked `write_node_projection`, `write_parent_node_projection`, and `save_node_record` as CRITICAL-risk surfaces; the change is additive and only runs the normalized DB mirror after the existing JSON record write succeeds.
- GitNexus marked `save_runner_request` and `save_runner_result` HIGH-risk; runner request/result normalization was intentionally deferred to a follow-up slice.

Validation commands run:

```text
cargo fmt --all
cargo test -p ploke-eval prototype1_eval_store_scheduler_node_projection_writes_normalized_rows --lib
cargo test -p ploke-eval prototype1_eval_store_parent_start_db_schema_installs_idempotently --lib
cargo test -p ploke-eval eval_store_non_agent_schema_scripts_are_stable --lib
cargo test -p ploke-eval existing_owner_db --lib
cargo build -p ploke-eval
git diff --check
```

Fresh DB proof after commit `1be870f7e Mirror scheduler nodes into normalized eval DB`:

- Campaign: `p1-sched-db-20260627-114548`
- Worktree: `/home/brasides/.ploke-eval/worktrees/p1-sched-db-20260627-114548`
- Profile: `p1-broad-db-1x1-20260627-111548`
- Storage: `dual-strict`
- Parent: `node-32f9189844650290`
- Setup branch: `prototype1-parent-p1-sched-db-20260627-114548-gen0`

DB proof immediately after setup via `walk db_query`:

```text
eval_scheduler_node              present
eval_scheduler_node_status_event present
eval_scheduler_node_target_part  present
eval_child_plan*                 present
```

Normalized scheduler node row:

```json
{
  "campaign_id": "p1-sched-db-20260627-114548",
  "node_id": "node-32f9189844650290",
  "projection_schema_version": "prototype1-scheduler-node.v1",
  "node_schema_version": "prototype1-treatment-node.v1",
  "generation": 0,
  "status": "planned",
  "branch_id": "prototype1-parent-p1-sched-db-20260627-114548-gen0",
  "candidate_id": "root-parent",
  "target_relpath": ".ploke/prototype1/parent_identity.json"
}
```

Normalized status row:

```json
{
  "node_id": "node-32f9189844650290",
  "status": "planned",
  "recorded_at": "2026-06-27T11:46:35.188786400+00:00",
  "content_sha256": "7c7b01a8cbc941e165094468c30565c1c846b84c05644f224520e29f32f73412"
}
```

Additional setup checks:

```text
eval_scheduler_node_target_part count = 0
invalid scheduler-node projection schema rows = 0
eval_record_ref.family=scheduler_node = 1
eval_record_ref.family=runner_request = 1
```

Interpretation: setup-time parent scheduler-node projection is now queryable from normalized DB rows. The legacy `scheduler_node` record-ref remains only as a citation/payload mirror.

Broad-harness R8 attempt on the same fresh campaign:

- `walk start --until r7` timed out after 900s but reconstruction reached `r7 - policy and child budget ready`.
- `walk step --until r8 --watch` failed at R7 with:

```text
child plan has 0 runnable child candidate(s), fewer than required minimum 1
```

The failed broad-harness attempt still proved live trace persistence and rejected-attempt child-plan persistence:

```text
eval_child_plan                  1
eval_child_plan_child            0
eval_child_plan_rejected_attempt 1
eval_agent_turn                  1
eval_tool_event                  37
eval_model_exchange              0
eval_message_event               0
eval_trace_event                 0
```

Rejected attempt row summary:

```text
producer_id=prototype1:broad-headless-tui-adapter-v1
policy=workspace_except_ploke_eval
outcome=rejected
target_relpath=.
reason=broad headless-tui slot ... has no admitted changes
```

Tool-event status counts included requested/completed `list_dir`, `read_file`, `request_code_context`, `cargo`, and requested/completed/failed `non_semantic_patch` events.

Remaining scheduler proof gap: this failed R8 did not admit a runnable child, so it did not produce child scheduler-node rows. A fresh successful broad-harness child admission is still needed to prove normalized scheduler-node rows for generation-1 children.

## 2026-06-27 runner request/result normalized slice

Implemented code slice:

- Added normalized relations:
  - `eval_runner_request`
  - `eval_runner_request_arg`
  - `eval_runner_request_target_part`
  - `eval_runner_result`
- `save_runner_request` now mirrors `runner-request.json` into normalized owner DB rows after the existing JSON file and `eval_record_ref` writes succeed.
- `save_runner_result` now mirrors both attempt-scoped result JSON and latest node `runner-result.json` into normalized owner DB rows after the existing JSON file and child-runtime `eval_record_ref` writes succeed.
- Runner request rows store identity, target path, workspace/binary paths, stop flag, content hash, arg count, and optional operation/patch/artifact ids.
- Runner request args are normalized one row per argument and keyed by request content hash.
- Runner request operation-target vector parts are normalized one row per part and keyed by request content hash.
- Runner result rows store status, disposition, treatment/evaluation refs, exit code/excerpts, path kind (`attempt`, `node_latest`, or `custom`), optional runtime id from attempt result path, content hash, and timestamps.
- Existing owner DBs that predate these relations fail loudly under the no-in-place-migration rule.

Risk/impact notes:

- GitNexus marked `save_runner_request` HIGH risk because it affects setup and loop-controller flows.
- GitNexus marked `save_runner_result` HIGH risk because it affects child runner result recording.
- The change is additive and preserves existing JSON files plus compatibility `eval_record_ref` rows; normalized writes occur only after the existing write path succeeds.

Validation commands run:

```text
cargo fmt --all
cargo test -p ploke-eval prototype1_eval_store_record_ref_runner_request_projection_writes_owner_db_row --lib
cargo test -p ploke-eval prototype1_eval_store_record_ref_runner_result_writes_child_local_rows --lib
cargo test -p ploke-eval eval_store_non_agent_schema_scripts_are_stable --lib
cargo test -p ploke-eval prototype1_eval_store_parent_start_db_schema_installs_idempotently --lib
cargo test -p ploke-eval existing_owner_db --lib
cargo build -p ploke-eval
git diff --check
```

Fresh DB proof after commit `9aaf9848f Mirror runner IO into normalized eval DB`:

- Campaign: `p1-runnerio-db-20260627-125536`
- Worktree: `/home/brasides/.ploke-eval/worktrees/p1-runnerio-db-20260627-125536`
- Profile: `p1-broad-db-1x1-20260627-111548`
- Storage: `dual-strict`
- Parent: `node-b86fe92de458ef31`
- Setup branch: `prototype1-parent-p1-runnerio-db-20260627-125536-gen0`

DB proof immediately after setup via `walk db_query`:

```text
eval_runner_request             present
eval_runner_request_arg         present
eval_runner_request_target_part present
eval_runner_result              present
eval_scheduler_node*            present
eval_child_plan*                present
```

Normalized runner request row:

```json
{
  "campaign_id": "p1-runnerio-db-20260627-125536",
  "node_id": "node-b86fe92de458ef31",
  "projection_schema_version": "prototype1-runner-request.v1",
  "request_schema_version": "prototype1-treatment-node.v1",
  "generation": 0,
  "branch_id": "prototype1-parent-p1-runnerio-db-20260627-125536-gen0",
  "target_relpath": ".ploke/prototype1/parent_identity.json",
  "stop_on_error": false,
  "runner_arg_count": 4,
  "content_sha256": "0c74e78ec0b97ad448ed3e14ac31ddc82c2778b8705a8d1324a4def5e0ffd5e3"
}
```

Normalized runner args:

```text
0 loop
1 prototype1-state
2 --repo-root
3 /home/brasides/.ploke-eval/worktrees/p1-runnerio-db-20260627-125536
```

Additional setup checks:

```text
eval_runner_request_target_part count = 0
invalid runner-request projection schema rows = 0
eval_runner_result count = 0
eval_record_ref.family=runner_request = 1
eval_record_ref.family=scheduler_node = 1
```

Interpretation: setup-time parent runner request is now queryable from normalized DB rows. The legacy `runner_request` record-ref remains only as a citation/payload mirror.

Broad-harness R8 attempt on the same fresh campaign:

- `walk start --until r7` reached `r7 - policy and child budget ready` cleanly.
- `walk step --until r8 --watch` failed at R7 with:

```text
child plan has 0 runnable child candidate(s), fewer than required minimum 1
```

Failed-attempt DB counts:

```text
eval_runner_request             1
eval_runner_request_arg         4
eval_runner_result              0
eval_child_plan                 1
eval_child_plan_child           0
eval_child_plan_rejected_attempt 1
eval_agent_turn                 1
eval_tool_event                 0
```

Rejected attempt row summary:

```text
producer_id=prototype1:broad-headless-tui-adapter-v1
policy=workspace_except_ploke_eval
outcome=rejected
target_relpath=.
reason=broad headless-tui slot ... timed out after 300 seconds
```

Remaining runner proof gap: this failed R8 did not admit or run a child, so `eval_runner_result` remains unproven by `walk db_query` on a live campaign. A successful admitted child run is still required for runner-result DB proof.

## Initial persistence matrix

Legend:

- **Normalized:** structured DB rows exist and are the intended inspection surface.
- **Payload-ref only:** currently stored mostly as `eval_record_ref.payload_json`; not sufficient for this pass.
- **Missing:** filesystem state has no adequate DB representation.
- **Transport exception:** channel can transport bytes, but durable shared facts extracted from it still need normalized rows.

| Area / artifact | Current DB coverage | Status for target | Notes / next action |
|---|---:|---|---|
| Campaign manifest setup | `eval_campaign`, campaign policy/budget/protocol/profile relations | Normalized | Keep; list fields are Cozo list columns. |
| Closure state | `eval_closure_ref`, `eval_closure_instance`, artifact/procedure/count relations | Mostly normalized | Counts relation exists but may be empty; verify semantics before marking complete. |
| Baseline | `eval_baseline`, instance, metrics relations | Normalized | Current run has two baselines after successor. |
| Parent-start journal evidence | `eval_transition_event` + some refs | Partially normalized | Later journal events are not comprehensively normalized. |
| Scheduler node projection `nodes/<node>/node.json` | `eval_scheduler_node`, `eval_scheduler_node_status_event`, `eval_scheduler_node_target_part`; legacy `eval_record_ref.family=scheduler_node` may remain as citation | Implemented; setup/root proof complete, generation-1 child proof pending | Fresh campaign `p1-sched-db-20260627-114548` proves the root parent row and status row. A successful admitted broad-harness child is still needed to prove child scheduler rows. |
| Runner request `runner-request.json` | `eval_runner_request`, `eval_runner_request_arg`, `eval_runner_request_target_part`; legacy `eval_record_ref.family=runner_request` may remain as citation | Implemented; setup/root proof complete, child request proof pending | Fresh campaign `p1-runnerio-db-20260627-125536` proves the root parent request row and arg rows. A successful child admission is still needed for child runner requests. |
| Child plan MessageBox | `eval_child_plan`, `eval_child_plan_child`, `eval_child_plan_rejected_attempt` | Normalized for R8 child-plan authority | Fresh DB-backed proof: `p1-normchild-db-20260627-100712` at R8 has 1 plan row, 3 child rows, 0 rejected rows, and 0 `eval_record_ref.family=child_plan_file` rows. |
| Invocation JSON | `eval_invocation`, `eval_attempt` | Normalized metadata; body not normalized | Need decide which embedded invocation payload facts must be normalized for cross-machine Parent/Child. |
| Channel JSONL | `eval_channel_message`, receipt/import rows | Transport metadata | Channel as transport is allowed, but terminal result/treatment payload facts should be normalized. |
| Runner result JSON | `eval_runner_result`; legacy `eval_record_ref.family=runner_result` may remain as citation | Implemented, fresh-run DB proof pending | Normalizes status/disposition, treatment/evaluation refs, path kind, runtime id when path-scoped, exit/excerpts, and content hash. Needs successful admitted child run for DB proof. |
| Branch evaluation report | `eval_evaluation`, `eval_evaluation_instance` | Normalized | Good DB-first inspection path exists. |
| Branch registry / comparison log | `eval_record_ref.family=branch_registry` | Payload-ref only | Needs normalized branch registry / comparison summary relation. |
| Build / artifact / patch / apply facts | `eval_artifact*`, `eval_binary_ref`, `eval_build_event`, `eval_operation`, `eval_patch`, `eval_apply_event` | Partly normalized | Need verify whether all file artifacts have normalized coverage or only event projections. |
| Selection | `eval_selection_decision`, candidate/finding/score | Normalized | Good initial shape; verify rows per generation/current parent. |
| Continuation decision | `eval_continuation_decision` | Normalized | Present after handoff. |
| History blocks/indexes | none dedicated; maybe refs only | Missing | Required next major schema area: blocks, block entries, heads/indexes. |
| Parent identity / handoff | no first-class relation | Missing | Needs normalized parent identity and successor handoff relations. |
| Agent turn / LLM/tool traces | `eval_agent_turn*` schema exists but current rows are zero | Missing or unwired | Need inspect writer paths and live tool/protocol adjudication outputs. |
| Trace events | `eval_trace_event` exists but current rows zero | Missing or unwired | Decide whether needed for run inspection vs logs. |

## Required next implementation slices

Do not continue live fanout as proof work until these are handled in a structured way.

1. **Create normalized child-plan schema** — done for R8 child-plan authority.
   - Relations: `eval_child_plan`, `eval_child_plan_child`, `eval_child_plan_rejected_attempt`.
   - Writer no longer emits `child_plan_file` as the child-plan persistence surface.
   - Fresh proof campaign `p1-normchild-db-20260627-100712` confirms normalized rows via `walk db_query`.

2. **Create normalized scheduler-node schema** — implemented; setup/root DB proof complete, child-row proof pending.
   - Relations: `eval_scheduler_node`, `eval_scheduler_node_status_event`, `eval_scheduler_node_target_part`.
   - Current node projection and status history are normalized; target parts are keyed by node projection content hash.
   - Fresh `walk db_query` proof: `p1-sched-db-20260627-114548` setup has one root parent scheduler row and one planned status row.
   - Remaining proof gap: generation-1 child scheduler rows need a successful admitted broad-harness child; first fresh broad attempt rejected because no admitted changes were produced.

3. **Create normalized runner request/result schemas** — implemented; root request proof complete, result proof pending.
   - Runner request: `eval_runner_request`, `eval_runner_request_arg`, `eval_runner_request_target_part`.
   - Runner result: `eval_runner_result`.
   - Fresh `walk db_query` proof: `p1-runnerio-db-20260627-125536` setup has one root parent request row and four arg rows.
   - Remaining proof gap: `eval_runner_result` needs a successful admitted child run; first fresh broad attempt timed out before admission.

4. **Normalize parent identity and successor handoff**
   - Parent identity currently changes on disk at handoff; DB should expose current and historical parent lineage.
   - Handoff should be queryable without reading `.ploke/prototype1/parent_identity.json` or history files.

5. **Normalize History store enough for DB-first restart/inspection**
   - Do not rely on `eval_record_ref` as the only layer.
   - Need explicit block/head/index relations that can later evolve into the blockchain-like History model.

6. **Audit Channel-derived shared payloads**
   - Channel can remain transport, but terminal result/treatment evidence and any shared Parent/Child capabilities should be normalized after receipt/send.

7. **Agent-turn / LLM protocol trace wiring**
   - Existing schema is empty in current run. Need identify whether protocol adjudication uses a path that bypasses agent-turn persistence.

## Audit loop for the next pass

For each transition:

1. Run `walk show --with-version` and record phase/parent.
2. Run the transition.
3. List files modified under the campaign root and parent worktree.
4. Query normalized DB relations expected for that transition.
5. If the only DB evidence is `eval_record_ref`, classify as a gap.
6. Implement required normalized schema + callsite.
7. Rebuild current binary.
8. Re-run transition on a fresh campaign or deterministic replay path.
9. Update this document with query evidence.
