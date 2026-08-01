# 2026-06-23 Prototype 1 DB-dual Broad Harness 3g1x3 Live Run

Live Prototype 1 `loop walk` setup packet for the DB-dual broad-harness canary run.

## Run configuration

- Campaign/profile: `p1-dbdual-broad3g1x3-pplxembed-g25p-p25f-20260623-174553`
- Profile path: `/home/brasides/.ploke-eval/profiles/prototype1/p1-dbdual-broad3g1x3-pplxembed-g25p-p25f-20260623-174553.toml`
- Worktree path: `/home/brasides/.ploke-eval/worktrees/p1-dbdual-broad3g1x3-pplxembed-g25p-p25f-20260623-174553`
- Source profile: `/home/brasides/.ploke-eval/profiles/prototype1/p1-walk5g3x5-pplxembed-g25p-p25f-evidencefix-20260621-170937.toml`
- Eval storage backend: `dual-strict`
- Target instance: `BurntSushi__ripgrep-2295`
- Parent model: `google/gemini-2.5-pro` via `direct-google`
- Protocol model: `google/gemini-2.5-flash` via `direct-google`
- Eval embedding setup flags: `perplexity/pplx-embed-v1-4b` with provider `perplexity`
- Generation source: `broad-harness-request`
- Search policy: `max_generations = 3`, `max_total_nodes = 24`, child `min = 1`, `max = 3`, `parallel_targets = 3`, `explore_from_rejected = true`
- Broad TUI policy: `max_attempts = 2`, `fresh_slots_per_child = 2`, `timeout_secs = 900`
- Execution policy: protocol enabled, MBE disabled

## Purpose

This is a live filesystem-authority plus DB-dual canary. Success should mean the filesystem-backed run can advance while `prototype1/eval-store.cozo.sqlite` captures the implemented eval-store evidence rows. It is not a DB-only readiness test.

## Review rules

- Treat files, History, Channel, MessageBox, bootstrap, and artifact/worktree state as authority unless a later slice explicitly migrates that read path.
- Treat DB rows as audit/query evidence for this run.
- Record setup and operator commands in `operator-log.md`.

## Final status snapshot

As of 2026-06-24, the canary is **not cleanly terminal**. It advanced through gen3 child execution but stopped at an R12/R13 policy-state inconsistency: durable continuation evidence says `stop_historical_traversal_budget` while also selecting historical successor `node-2ee45b1f1ce50161`, and `walk step` refuses the stopped R13a path because selected-successor evidence exists. I did not force the suggested R13b handoff because that would continue through a stop-budget disposition and risk corrupting the canary semantics.

Final audited paths:

- Worktree: `/home/brasides/.ploke-eval/worktrees/p1-dbdual-broad3g1x3-pplxembed-g25p-p25f-20260623-174553`
- Campaign root: `/home/brasides/.ploke-eval/campaigns/p1-dbdual-broad3g1x3-pplxembed-g25p-p25f-20260623-174553`
- Owner eval DB: `/home/brasides/.ploke-eval/campaigns/p1-dbdual-broad3g1x3-pplxembed-g25p-p25f-20260623-174553/prototype1/eval-store.cozo.sqlite`
- Final command logs: `command-logs/035-walk-terminal-stop-after-gen3.log` through `command-logs/039-final-history-projection-summaries.log`
