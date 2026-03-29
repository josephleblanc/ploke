# Date
2026-03-28

# Task Title
Corpus Triage Workflow Setup

# Task Description
Define a repeatable workflow for surveying `parse debug corpus` failures with sub-agents, using persisted run artifacts, clustered failure signatures, and machine-queryable JSON reports.

# Related Planning Files
- [2026-03-28_error-diagnostic-rollout-plan.md](/home/brasides/code/ploke/docs/active/agents/2026-03-28_error-diagnostic-rollout-plan.md)

## Workflow

1. Run a broad corpus sweep, for example:

```text
cargo xtask parse debug corpus --limit 100 --workspace-mode probe
```

2. Build the triage index from the saved run:

```text
cargo xtask parse debug corpus-triage <run-id>
```

3. Use the emitted files under `<run-dir>/triage/`:
- `index.json`: full machine-readable triage payload
- `failures.jsonl`: one line per failed stage occurrence
- `clusters.json`: grouped failure signatures
- `reports/_report_template.json`: schema template for investigation output
- `reports/pending/*.json`: one stub per failure cluster

4. Assign one sub-agent per cluster stub, not one sub-agent per raw failure.

5. Have each sub-agent update its assigned pending report with:
- suspected root cause
- confidence
- fix-vs-document assessment
- whether the issue touches sensitive pipeline areas
- recommended next step
- relevant artifact and code paths

## Suggested Query Patterns

```text
jq -c '.clusters[] | {count, stage, failure_kind, error_signature, pending_report_path}' <run-dir>/triage/clusters.json
jq -c '.failures[] | {id, normalized_repo, member_label, stage, error_signature}' <run-dir>/triage/index.json
rg -n '"status": "pending"' <run-dir>/triage/reports/pending
```

## Report Expectations

Pending cluster reports are intended to stay concise and structured. They should capture enough evidence to decide whether the issue is:
- a likely parser fix
- a likely known limitation to document
- a duplicate symptom of an already-understood root cause

If an investigation points toward `code_visitor.rs`, merge functions, pruning functions, or another complex pipeline step, pause before implementation and review with the user first.
