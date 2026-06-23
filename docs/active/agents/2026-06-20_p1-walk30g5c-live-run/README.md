# 2026-06-20 Prototype 1 Walk 30g5c Live Run

Live Prototype 1 `loop walk` run packet for the 30-generation / 5-child run family that started with `p1-walk30g5c-det-g25p-p25f-20260620-172554` and moved to `p1-walk30g5c-pplxembed-det-g25p-p25f-20260620-173545` after the first campaign persisted a failed baseline embedding preflight.

## Run Configuration

- Source profile: `/home/brasides/.ploke-eval/profiles/prototype1/p1-walk5x3local2295-det-g25p-p25f-20260618-122326.toml`
- First profile: `/home/brasides/.ploke-eval/profiles/prototype1/p1-walk30g5c-det-g25p-p25f-20260620-172554.toml`
- First campaign/worktree: `p1-walk30g5c-det-g25p-p25f-20260620-172554` (abandoned after failed baseline embedding preflight)
- Replacement profile: `/home/brasides/.ploke-eval/profiles/prototype1/p1-walk30g5c-pplxembed-det-g25p-p25f-20260620-173545.toml`
- Replacement campaign/worktree: `p1-walk30g5c-pplxembed-det-g25p-p25f-20260620-173545`
- Eval embedding setup flags: `perplexity/pplx-embed-v1-4b` with provider `perplexity`
- Target instance: `BurntSushi__ripgrep-2295`
- Parent model: `google/gemini-2.5-pro` via `direct-google`
- Protocol model: `google/gemini-2.5-flash` via `direct-google`
- Search policy: `max_generations = 30`, `max_total_nodes = 192`, child `min/max/parallel_targets = 5`, `explore_from_rejected = true`
- Execution policy: deterministic TUI tools, `workspace-except-ploke-eval`, protocol enabled, MBE disabled

## Review Index

- `tool-loop-reviews/`
  Per-LLM-tool-loop run reviews written after each completed tool-call loop.
- `parent-cycle-reviews/`
  Per-parent-cycle type-state and run-review synthesis after each parent/child handoff.
- `ten-cycle-synthesis/`
  Reports after cycles 1-10, 11-20, and 21-30 on score trends, strengths, failures, and harness improvement candidates.
- `operator-log.md`
  Local orchestration notes, commands, and blocking events for this run.

## Review Rules

- Tool-loop reviews must name exact evidence roots and include at least one concrete trace chain.
- Parent-cycle reviews must cover the R0-R14 transition spine for the completed parent cycle and summarize linked tool-loop reviews.
- Ten-cycle synthesis reports must compare child-node scores over time and separate observed run behavior from proposed harness changes.
- Hard blockers pause the live walk before the next effectful transition; non-blocking findings stay in the relevant review file.
