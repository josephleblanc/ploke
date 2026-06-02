# date: 2026-03-30
# task title: syn_parser repro root-cause analysis
# task description: run the failing syn_parser repro tests, delegate root-cause analysis for each expected failure, and collect report-only follow-ups without code edits
# related planning files: /home/brasides/code/ploke/docs/active/agents/2026-03-29_corpus-triage/2026-03-30_corpus-triage-run-1774867607815.md, /home/brasides/code/ploke/docs/active/agents/2026-03-29_corpus-triage/2026-03-30_corpus-live-triage-runbook-v2.md

## Scope

- Run `cargo test -p syn_parser --test mod repro::fail` and capture the expected failures.
- Dispatch sub-agents for root-cause analysis only.
- Have each sub-agent write a report file in this directory.
- Produce one final summary document in this directory after all reports are complete.

## Working Rules

- No code edits from the RCA agents.
- Keep the main thread focused on orchestration and report aggregation.
- Prefer targeted `xtask` inspection commands when they help a sub-agent narrow the cause.
