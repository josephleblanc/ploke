# 2026-04-17 Blind Trace Review Packet

- task_id: `AUDIT-P1`
- title: blind CLI-only trace review sample
- date: 2026-04-17
- owner_role: worker
- layer_workstream: `A4`
- related_hypothesis: blind CLI-first review of tool-call traces can be compared
  against persisted protocol artifacts to estimate protocol signal quality and
  reviewer variance
- design_intent: collect independent trace judgments without exposing reviewers
  to existing protocol outputs
- scope:
  - review the sampled runs below using `ploke-eval` CLI inspection commands
    only
  - treat each run as a standalone trace-review task
  - produce concise per-run findings and one compact overall summary
- non_goals:
  - do not inspect protocol artifacts or run `ploke-eval protocol ...`
  - do not read or edit source code
  - do not infer hidden labels or compare against any existing protocol output
- owned_files:
  - read-only CLI inspection over sampled run artifacts
- dependencies:
  - `./target/debug/ploke-eval inspect tool-calls`
  - `./target/debug/ploke-eval inspect turn`
  - `./target/debug/ploke-eval inspect conversations`
  - `docs/workflow/skills/cli-trace-review/SKILL.md`
- acceptance_criteria:
  1. every sampled run receives a concise CLI-only review
  2. each review cites commands actually used
  3. findings stay blind to protocol artifacts and avoid unsupported model blame
- required_evidence:
  - sampled instance ids reviewed
  - commands executed per run
  - concise findings in the `cli-trace-review` output shape
- report_back_location:
  - return a concise structured message to the orchestrator; do not edit repo
    docs unless explicitly reassigned
- status: `in_progress`

## Reviewer Instructions

- Use the `cli-trace-review` skill.
- Start from `./target/debug/ploke-eval inspect tool-calls --instance <id>`.
- Use only the smallest additional inspect surface needed for support.
- Keep the output concise and comparable across runs.

## Sampled Runs

- `clap-rs__clap-3700`
- `clap-rs__clap-5228`
- `clap-rs__clap-5873`
- `BurntSushi__ripgrep-1980`
- `sharkdp__bat-1518`
- `tokio-rs__tokio-4789`
- `tokio-rs__tokio-6252`
- `sharkdp__bat-1402`
