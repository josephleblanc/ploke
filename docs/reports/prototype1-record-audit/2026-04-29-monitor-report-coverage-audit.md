# Prototype 1 Monitor Report Coverage Audit

Scope: `./target/debug/ploke-eval loop prototype1-monitor --campaign p1-crown-baseline-3gen-15nodes-2 --repo-root /home/brasides/.ploke-eval/worktrees/prototype1-crown-history-experiment report --format json`

## Command Coverage

The command is a campaign-level projection, not a History import surface. The dispatch is explicit in `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:1458-1473`, and the report module says it is producing a provisional aggregate, not sealed History (`crates/ploke-eval/src/cli/prototype1_state/report.rs:1-7`).

Direct reads in `report.rs` are limited to:

- `prototype1/scheduler.json` (`report.rs:76-106`, `:163-182`)
- `prototype1/branches.json` (`report.rs:76-106`, `:259-299`)
- `prototype1/transition-journal.jsonl` (`report.rs:76-106`, `:313-353`)
- `prototype1/evaluations/*.json` (`report.rs:76-106`, `:371-469`, `:624-653`)

It then derives identifier sets from those same sources only: `campaign_id`, `node_id`, `generation`, `runtime_id`, `branch_id`, `candidate_id`, `source_state_id`, and `target_relpath` (`report.rs:510-587`).

## Missing Or Weak Coverage

Current campaign artifacts present under `.../prototype1/` include 10 `node.json` files, 10 `runner-request.json` files, 3 `runner-result.json` files, 3 attempt `results/*.json` files, 5 `invocations/*.json` files, 2 `successor-ready/*.json` files, 2 `successor-completion/*.json` files, and 4 `messages/child-plan/*.json` files. The report does not load or surface any of those payloads.

Under the instance/run tree there are also 4 `record.json.gz` archives, 4 `agent-turn-trace.json` files, 4 `agent-turn-summary.json` files, 4 `llm-full-responses.jsonl` files, 4 `multi-swe-bench-submission.jsonl` files, and 99 `protocol-artifacts/*` files. None are read by the command.

What is only weakly surfaced:

- `node.json` content is collapsed into scheduler-derived counts and the selected trajectory.
- `runner-request.json` is not shown at all, only the scheduler/registry ids that happen to match it.
- `runner-result.json`, `results/*.json`, `invocations/*.json`, `successor-ready/*.json`, and `successor-completion/*.json` are at most indirectly represented by `runtime_id` and journal counts.
- `parent_identity.json` is not read; it only shows up as a path string in derived `target_relpaths` when embedded elsewhere.

The command also omits the run-artifact families that the History admission map treats as admissible evidence or important projections: `nodes/*/results/<runtime-id>.json`, `nodes/*/runner-result.json`, `nodes/*/invocations/*.json`, `successor-ready/*.json`, `successor-completion/*.json`, `runner-request.json`, `.ploke/prototype1/parent_identity.json`, `record.json.gz`, `agent-turn-trace.json`, `agent-turn-summary.json`, tool-call records in `RunRecord`, protocol artifacts, and process logs (`history-admission-map.md:21-40, 48-69, 71-89, 93-165`).

This is consistent with the admission map’s rule that CLI reports and monitor output are projection only (`history-admission-map.md:39-40`), but it is narrower than the first read-only History preview shape, which expects source refs, payload hashes, provisional blocks, admitted-preview entries from journal/evaluation/run artifacts, and projection rows for scheduler/branch/frontier summaries (`history-admission-map.md:93-103, 134-165`).

## Recommended CLI / Report Improvements

1. Add an explicit artifact section for node/request/result triples, invocations, successor ready/completion, parent identity, and run archives, with path + digest rows instead of just ids.
2. Separate projection summaries from admissible evidence so the report can show what is canonical, what is degraded, and what is not read.
3. Carry source-path/digest fields for evaluation reports and any surfaced run artifact, matching the admission map’s ownership rules for `run record path`, `LLM prompt/response text`, and `tool call ids` (`history-admission-map.md:60-67`).
4. Either rename this command as a pure monitor projection or add a sibling `history preview`/`--include-artifacts` path that matches the admission map’s preview contract more closely.
