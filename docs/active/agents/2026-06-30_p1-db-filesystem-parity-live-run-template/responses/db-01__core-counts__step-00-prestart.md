# db-01 — core normalized relation counts before live walk start

## Question

Can the owner DB answer basic run progress from normalized relations before the live walk starts?

## Query

- Query file: `../queries/db-01__core-counts.cozo`
- Campaign: `p1-gated-parent-3g1x3-p3-20260630-174316`
- Step: `00-prestart`

## Command

```bash
P1_BIN=/home/brasides/.ploke-eval/worktrees/p1-gated-parent-3g1x3-p3-20260630-174316/target/debug/ploke-eval
P1_ROOT=/home/brasides/.ploke-eval/worktrees/p1-gated-parent-3g1x3-p3-20260630-174316
"$P1_BIN" loop walk db_query --repo-root "$P1_ROOT" --format json --script "$(cat ../queries/db-01__core-counts.cozo)"
```

## Raw output summary

Rows of interest before live walk start:

```text
eval_campaign = 1
eval_profile_commitment = 1
eval_run_profile_policy = 1
eval_scheduler_node = 1
eval_scheduler_node_status_event = 1
eval_runner_request = 1
eval_record_ref = 2
all harness/agent-turn/child-plan/artifact/build/invocation/result/evaluation/selection/continuation/walk-event counts = 0
```

## Answer

Partial. The DB proves setup/admission/root-node facts are present, but no live parent-start or walk-transition facts exist yet.

## Helpfulness

Useful as a progress baseline and delta source. It immediately exposed that relation key names are not obvious from relation names: an initial query incorrectly assumed `eval_binary_ref.binary_id` and failed because the key is `binary_ref_id`.

## Schema/query design notes

Helpful: normalized setup, scheduler, and runner-request rows are present before walk start.

Awkward: writing count queries requires knowing per-relation key names. A documented `eval_relation_key` metadata relation, generated query view, or a CLI `walk db_query counts` helper would make this easier. This is a query ergonomics issue, not necessarily a storage schema flaw.
