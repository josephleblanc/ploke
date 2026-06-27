# 2026-06-27 Prototype 1 normalized DB persistence pass

**Status:** active plan + audit log; child-plan normalized slice implemented in code, fresh-run DB proof still required
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
| Scheduler node projection `nodes/<node>/node.json` | `eval_record_ref.family=scheduler_node` | Payload-ref only | Needs normalized `eval_scheduler_node` / status-event relation; replace payload-only coverage. |
| Runner request `runner-request.json` | `eval_record_ref.family=runner_request` | Payload-ref only | Needs normalized runner-request relation with args/list fields. |
| Child plan MessageBox | `eval_child_plan`, `eval_child_plan_child`, `eval_child_plan_rejected_attempt` | Normalized for R8 child-plan authority | Fresh DB-backed proof: `p1-normchild-db-20260627-100712` at R8 has 1 plan row, 3 child rows, 0 rejected rows, and 0 `eval_record_ref.family=child_plan_file` rows. |
| Invocation JSON | `eval_invocation`, `eval_attempt` | Normalized metadata; body not normalized | Need decide which embedded invocation payload facts must be normalized for cross-machine Parent/Child. |
| Channel JSONL | `eval_channel_message`, receipt/import rows | Transport metadata | Channel as transport is allowed, but terminal result/treatment payload facts should be normalized. |
| Runner result JSON | `eval_record_ref.family=runner_result` | Payload-ref only | Needs normalized runner-result relation keyed by node/runtime/path type. |
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

2. **Create normalized scheduler-node schema**
   - Replace `scheduler_node` payload-ref-only coverage.
   - Candidate relations: `eval_scheduler_node`, `eval_scheduler_node_status_event`.
   - Must preserve status history rather than only latest file payload.

3. **Create normalized runner request/result schemas**
   - Runner request: identity, target paths, operation/patch/artifact ids, runner args list.
   - Runner result: node/runtime/path role, status/disposition/treatment/evaluation refs, exit/excerpts.

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
