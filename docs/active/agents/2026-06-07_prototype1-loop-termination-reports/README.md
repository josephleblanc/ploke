# Prototype 1 Loop Termination Reports

Purpose: collect evidence-grounded termination and timing reports for recent
Prototype 1 loop campaigns. Each report should identify whether the run is live,
terminated cleanly, failed, or stopped by a configured budget, and cite the
artifact that proves the final state.

Required evidence per campaign:

- host-process status from a self-filtered `ps` probe;
- `prototype1/transition-journal.jsonl` tail with durable epoch-ms timestamps;
- broad request/result mtimes when child generation occurred;
- node invocation/result/successor records that prove the last reached phase;
- stream stderr/stdout evidence for terminal errors;
- final verdict: `clean`, `failed`, `live`, or `incomplete evidence`.

Reports:

- [`setup-and-prechild-failures.md`](setup-and-prechild-failures.md) —
  `p1-rejected-handoff-5g1x2-a2-20260607-191057` has incomplete terminal
  evidence after parent start, and `p1-handofffix-5g1x2-a2-20260607-190702`
  failed during baseline `embedding_model_preflight`.
- [`handoff-failure-campaigns.md`](handoff-failure-campaigns.md) —
  `p1-nokeep-handoff-5g1x2-a2-20260607-192811` and
  `p1-historyfix-handoff-5g1x2-a2-20260607-155010` both terminated with failed
  successor-completion channel records.
- [`handofffix-embed-budget-stop.md`](handofffix-embed-budget-stop.md) —
  `p1-handofffix-embed-5g1x2-a2-20260607-192954` ended cleanly at the
  configured historical-traversal budget stop.
- [`../run-reviews/2026-06-08-p1-selectfix-replay-5g1x2-a2-20260607-224502/terminal-status.md`](../run-reviews/2026-06-08-p1-selectfix-replay-5g1x2-a2-20260607-224502/terminal-status.md)
  — `p1-selectfix-replay-5g1x2-a2-20260607-224502` ended cleanly at
  `stop_historical_traversal_budget` after generation-4 children completed
  under the `max_generations=5` profile.
