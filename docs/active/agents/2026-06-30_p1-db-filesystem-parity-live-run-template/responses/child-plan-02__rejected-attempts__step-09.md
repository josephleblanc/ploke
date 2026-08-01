# child-plan-02 — rejected broad-harness attempt after R7→R8

## Question

Are rejected broad-harness attempts preserved as evidence rather than lost when no child is admitted from that attempt?

## Query

- Query file: `../queries/child-plan-02__rejected-attempts.cozo`
- Raw output: `child-plan-02__rejected-attempts__step-09.json`
- Step: `09 / after R7->R8`

## Output summary

```text
row_count = 1
attempt_index = 0
proposal_id = broad-harness-request:node-ec383aa38762a3d4:r3
outcome = rejected
policy = workspace_except_ploke_eval
producer_id = prototype1:broad-headless-tui-adapter-v1
reason = broad headless-tui slot ... tool failed: ... timed out waiting for BM25 readiness after applying proposal batch after 600s
```

## Answer

Yes. The rejected `-r3` attempt is normalized in `eval_child_plan_rejected_attempt` with producer, proposal id, policy, run id, target, outcome, and detailed reason.

## Helpfulness

Very useful. This DB query captures the known post-apply BM25 readiness failure as structured child-plan evidence, not just as a log line.

## Schema/query design notes

Helpful: the rejected-attempt relation makes failed broad attempts visible to selection and later analysis.

Gap/question: the reason string contains rich diagnostic text but not separately normalized fields for failure class (`bm25_readiness_timeout`), failing phase (`post_apply`), timeout seconds (`600`), diagnostics path, or changed files. Adding structured failure taxonomy fields would make longitudinal reliability queries much easier.
