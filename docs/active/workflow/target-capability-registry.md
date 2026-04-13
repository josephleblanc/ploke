# Target Capability Registry

- last_updated: 2026-04-12
- source: [target-capability-registry.md](../../workflow/target-capability-registry.md)
- owning_branch: `refactor/tool-calls`
- review_cadence: update after parser/modeling/scaling discoveries or when a target's run policy changes
- update_trigger: update after a reviewed run, packet, bug, or postmortem changes target interpretability or run policy

## Purpose

This is the living registry for target- and task-level limitations that affect
whether graph-based evals are fair, interpretable, or worth running by default.

Use this registry to prevent known parser/modeling/scaling limitations from
being rediscovered expensively in live eval runs.

## Status Meanings

- `active`: currently relevant limitation
- `watch`: known limitation, but not currently blocking normal use
- `resolved_pending_reentry`: implementation landed; target should be re-tried
- `resolved`: no longer limits normal interpretation for the affected target

## Entries

| target_id | task_scope | status | limitation_class | interpretability_flag | run_policy | affected_surface | summary | evidence | workaround | reentry_condition | owner_workstream | last_reviewed |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| `BurntSushi__ripgrep` | target-wide mixed-edition parse path | `resolved` | `parser_blocker` | `graph_valid` | `default_run` | Rust 2015 parse path under mixed-edition code | Mixed Rust 2015/2018 code exposed `syn` edition-keyword failures such as `fn async()` and bare trait objects, which previously aborted parsing. `P2B` re-ran the documented `BurntSushi__ripgrep-1294` sentinel through the current CLI path and did not reproduce the old `globset` parse blocker, so this limitation is no longer an active run-policy gate. Keep ripgrep as a useful regression/sentinel target, but not as a default-excluded target. | [A2 entry](./hypothesis-registry.md), [P2B report](../agents/2026-04-12_eval-infra-sprint/2026-04-12_P2B_report.md), [bug: syn 2 fails on Rust 2015 bare trait objects](../../active/bugs/2026-04-10-syn-2-fails-on-rust-2015-bare-trait-objects.md), [recent activity](./handoffs/recent-activity.md) | use the dual-`syn` path; keep occasional ripgrep sentinel reruns as regression spot-checks rather than as a standing blocker check | revisit only if mixed-edition parse regressions reappear or a parser refactor warrants a sentinel recheck | `A2` | 2026-04-12 |
| `macro-heavy-targets` | tasks requiring symbols defined only through unmodeled macro expansion | `active` | `modeling_coverage_gap` | `graph_degraded` | `run_only_for_feature` | macro_rules and proc-macro-dependent symbol coverage | Some targets or tasks are effectively out of scope for pure graph-based retrieval because important semantic regions are omitted from the graph. This should be treated as a coverage limitation, not a generic agent/tool failure. | [eval design](../plans/evals/eval-design.md), current parser/modeling notes, future packet/postmortem links as they land | skip by default for fair graph-only evals; optionally use as a deliberate re-entry probe when testing macro modeling work | macro_rules / proc-macro modeling brings the missing region into the graph with bounded validation | `A2` | 2026-04-12 |
| `very-large-projects` | target-wide unless subset defined | `active` | `scaling_constraint` | `performance_restricted` | `subset_only` | indexing, embedding, and run latency on very large codebases | Some multi-million-LoC projects are currently too slow or costly to index/run under normal eval cadence. This is primarily an execution-policy constraint rather than a fairness claim about graph semantics. | parser/indexing operational experience; add concrete packets/postmortems as they land | use curated subsets, caps, or special runs instead of default formal-run treatment | indexing/runtime improvements or a defined large-target policy make full-target runs operationally acceptable | `A2` | 2026-04-12 |

## Notes

- Add task-level entries when only some benchmark tasks on a target are affected.
- Prefer linking concrete bug reports, packet reports, or postmortems as they
  become available.
- When a target changes from `active` to `resolved_pending_reentry`, the next
  action should be a deliberate re-entry run rather than silently removing the
  limitation from memory.
