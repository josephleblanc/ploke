# eval-02 — baseline readiness at R6

## Question

Was a generation-0 baseline established before child comparison, and can the DB identify the baseline instance, metrics, closure status, and record path?

## Query

- Query file: `../queries/eval-02__baseline-readiness.cozo`
- Raw output: `eval-02__baseline-readiness__step-07.json`
- Campaign: `p1-gated-parent-3g1x3-p3-20260630-174316`
- Step: `07 / after-r6`

## Command

```bash
P1_BIN=/home/brasides/.ploke-eval/worktrees/p1-gated-parent-3g1x3-p3-20260630-174316/target/debug/ploke-eval
P1_ROOT=/home/brasides/.ploke-eval/worktrees/p1-gated-parent-3g1x3-p3-20260630-174316
"$P1_BIN" loop walk db_query --repo-root "$P1_ROOT" --format json --script "$(cat ../queries/eval-02__baseline-readiness.cozo)"
```

## Output summary

```text
row_count = 1
baseline_id = e08c51307a5b56fb45e0ed356e452c31e35e594df8c6389ee737b8fb6496c4e1
status = complete
instance_id = BurntSushi__ripgrep-2209
eval_status = complete
protocol_status = complete
tool_calls_total = 51
tool_calls_failed = 0
valid_patch = true
convergence = true
record_path = /home/brasides/.ploke-eval/instances/prototype1/p1-gated-parent-3g1x3-p3-20260630-174316/BurntSushi__ripgrep-2209/runs/run-1782848953623-structured-current-policy-85169a85/record.json.gz
```

## Answer

Yes for normalized baseline readiness and basic metrics. The DB has one complete baseline row, one baseline instance row, one metrics row, and closure status joined to `eval_status=complete` and `protocol_status=complete`.

## Helpfulness

Useful. It answers the baseline-before-child-planning question from normalized rows without reading the baseline run directory.

## Schema/query design notes

Helpful: baseline, instance, metrics, and closure relations are normalized enough to answer the basic readiness question.

Awkward: the detailed run record remains a `record_path` to a compressed file. Deeper questions about benchmark transcript, full protocol artifacts, or exact evaluator output still require file inspection or additional normalized run/protocol relations.
