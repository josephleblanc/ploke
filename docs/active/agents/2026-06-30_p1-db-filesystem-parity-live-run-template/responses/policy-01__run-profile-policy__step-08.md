# policy-01 — admitted run-profile policy at R7

## Question

Which admitted policy controlled generation, child budget, broad-harness generation, oracle mode, and control parallelism?

## Query

- Query file: `../queries/policy-01__run-profile-policy.cozo`
- Raw output: `policy-01__run-profile-policy__step-08.json`
- Campaign: `p1-gated-parent-3g1x3-p3-20260630-174316`
- Step: `08 / after-r7`

## Output summary

```text
max_generations = 3
max_total_nodes = 10
child_min = 1
child_max = 3
parallel_targets = 3
parallel_cap = 3
generation_source = broad-harness-request
explore_rejected = true
oracle_mode = record-only
oracle_required = true
broad_max_attempts = 1
broad timeout_secs = 1800
control_mode = continuous
```

## Answer

Yes for persisted admitted policy facts. The DB relation `eval_run_profile_policy` has the search/generation/control/oracle values needed to verify the intended run shape.

## Helpfulness

Useful. This answers the main run-shape policy question from normalized DB rows rather than from TOML alone.

## Schema/query design notes

Helpful: policy fields are normalized into direct columns, making this a simple one-relation query.

Caveat: `oracle_required = true` is persisted, but its semantics depend on `oracle_mode`. For `record-only`, the run-time selection code treats `require_evidence` as inert. A derived field or documented view such as `oracle_blocks_selection` would make the operational semantics easier to query.
