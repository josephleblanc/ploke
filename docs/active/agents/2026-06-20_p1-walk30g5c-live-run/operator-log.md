# Operator Log

## 2026-06-20 17:25 PDT Setup

- Created run packet directory.
- Cloned profile settings from `/home/brasides/.ploke-eval/profiles/prototype1/p1-walk5x3local2295-det-g25p-p25f-20260618-122326.toml`.
- Changed search policy for the requested live run: `max_generations = 30`, `max_total_nodes = 192`, child `min/max/parallel_targets = 5`, and `control.parallel_cap = 5`.
- Created worktree `/home/brasides/.ploke-eval/worktrees/p1-walk30g5c-det-g25p-p25f-20260620-172554`.
- `prototype1-setup` admitted parent node `node-750e566c3f85053b` on branch `prototype1-parent-p1-walk30g5c-det-g25p-p25f-20260620-172554-gen0`.
- Built `ploke-eval` in the worktree.
- Doctor passed at `phase = baseline_eval`; effective control was `parallel_cap = 5`, `patch_generation_parallel_cap = 5`.
- Initial sandboxed live protocol preflight failed with generic `provider_request`; unsandboxed ADC token minting succeeded and re-running doctor under unrestricted environment passed live protocol preflight for `google/gemini-2.5-flash` via `direct_google`.

## 2026-06-20 17:34 PDT First Campaign Abandoned

- Started `loop walk` for `p1-walk30g5c-det-g25p-p25f-20260620-172554` at `R0`.
- `walk step --until r7 --watch` advanced through `R5`, then failed before `R6`.
- Failure: `Parent<node-750e566c3f85053b> cannot spawn children: baseline instance 'BurntSushi__ripgrep-2295' is Failed`.
- Closure evidence showed baseline eval failed during `embedding_model_preflight` on default `mistralai/codestral-embed-2505` because the required env var was absent.
- Disposition: stop-use for loop progress. The campaign has persisted failed baseline evidence and should not be resumed.
- Stopped the walk server and ran `cargo clean` in the failed worktree, removing local build artifacts while preserving run evidence.

## 2026-06-20 17:35 PDT Replacement Campaign Plan

- Replacement campaign/profile: `p1-walk30g5c-pplxembed-det-g25p-p25f-20260620-173545`.
- Same 30-generation / 5-child / direct-Google / deterministic-TUI profile shape.
- Setup must include explicit embedding flags: `--embedding-model-id perplexity/pplx-embed-v1-4b` and `--embedding-provider perplexity`.

## 2026-06-20 17:40 PDT Replacement Campaign Verified

- Created worktree `/home/brasides/.ploke-eval/worktrees/p1-walk30g5c-pplxembed-det-g25p-p25f-20260620-173545`.
- `prototype1-setup` admitted parent node `node-0f11d55798e2a3a7` on branch `prototype1-parent-p1-walk30g5c-pplxembed-det-g25p-p25f-20260620-173545-gen0`.
- Campaign manifest verified parent model `google/gemini-2.5-pro` via `direct_google`, protocol model `google/gemini-2.5-flash` via `direct_google`, and eval embeddings `perplexity/pplx-embed-v1-4b` with provider `perplexity`.
- Built `ploke-eval` successfully in the replacement worktree.
- Doctor passed at `phase = baseline_eval`; effective control was `parallel_cap = 5`, `patch_generation_parallel_cap = 5`.
- Live protocol preflight passed with canary budget `4096` tokens. Headless TUI setup preflight was skipped because the run profile does not use broad-harness headless TUI generation.

## 2026-06-20 17:43 PDT Replacement Campaign Abandoned

- Started `loop walk` for `p1-walk30g5c-pplxembed-det-g25p-p25f-20260620-173545` at `R0`.
- `walk step --until r7 --watch` advanced to `R5`, then failed before `R6`.
- Failure: `Parent<node-0f11d55798e2a3a7> cannot spawn children: baseline instance 'BurntSushi__ripgrep-2295' is Failed`.
- Closure evidence showed baseline eval failed during `embedding_model_preflight` for `perplexity/pplx-embed-v1-4b` with `Var error: Error from env variable`.
- A bounded setup-only probe against the same prepared run manifest and explicit `perplexity/pplx-embed-v1-4b` / `perplexity` flags succeeded, writing `/home/brasides/.ploke-eval/instances/prototype1/p1-walk30g5c-pplxembed-det-g25p-p25f-20260620-173545/BurntSushi__ripgrep-2295/runs/run-1782002582325-shell-only-4db3bca4/execution-log.json`.
- Disposition: stop-use for loop progress. The campaign has persisted failed baseline evidence and should not be resumed.
- Stopped the walk server at `R5`.

## 2026-06-20 17:46 PDT Worktree-Correct Campaign Verified

- Attempted campaign `p1-walk30g5c-pplxembed-r2-det-g25p-p25f-20260620-174400`; initial setup was blocked by dirty source docs but persisted a partial campaign manifest, so that campaign ID is tainted and should not be reused.
- Attempted campaign `p1-walk30g5c-pplxembed-r3-det-g25p-p25f-20260620-174508`; setup admitted successfully but used `/home/brasides/code/ploke` as `repo_root`, so it does not satisfy the requested separate-worktree run shape and is stop-use for this live walk.
- Created seed worktree `/home/brasides/.ploke-eval/worktrees/p1-walk30g5c-pplxembed-r4-det-g25p-p25f-20260620-174602` from `feature/ploke-loop` at `15322e88`.
- Ran `prototype1-setup` inside the seed worktree for campaign `p1-walk30g5c-pplxembed-r4-det-g25p-p25f-20260620-174602`.
- `prototype1-setup` admitted parent node `node-737ffdeb22262f67` on branch `prototype1-parent-p1-walk30g5c-pplxembed-r4-det-g25p-p25f-20260620-174602-gen0` with `repo_root = /home/brasides/.ploke-eval/worktrees/p1-walk30g5c-pplxembed-r4-det-g25p-p25f-20260620-174602`.
- Campaign manifest verified parent model `google/gemini-2.5-pro` via `direct_google`, protocol model `google/gemini-2.5-flash` via `direct_google`, and eval embeddings `perplexity/pplx-embed-v1-4b` with provider `perplexity`.
- Built `ploke-eval` successfully in the `r4` worktree.
- Doctor passed at `phase = baseline_eval`; effective control was `parallel_cap = 5`, `patch_generation_parallel_cap = 5`.
- Live protocol preflight passed with canary budget `4096` tokens. Headless TUI setup preflight was skipped because the run profile does not use broad-harness headless TUI generation.

## 2026-06-20 17:59 PDT R5 Campaign Reaches R7

- `r4` reproduced the baseline embedding preflight failure at `R5` because commands launched with the external worktree as `cwd` did not inherit `OPENROUTER_API_KEY`; the walk server PID also lacked that variable. Disposition: stop-use for loop progress.
- Created seed worktree `/home/brasides/.ploke-eval/worktrees/p1-walk30g5c-pplxembed-r5-det-g25p-p25f-20260620-175208` from `feature/ploke-loop` at `f51e981e`.
- Ran `prototype1-setup`, doctor, live protocol preflight, and `walk start` through a shell wrapper that exports `OPENROUTER_API_KEY` before entering the worktree.
- Confirmed the r5 walk server PID had `OPENROUTER_API_KEY` in `/proc/<pid>/environ`.
- `walk step --until r7 --watch` advanced from `R0` to `R7`; baseline evidence became `Ready<CompleteBaseline>` and policy evidence became `Ready<Prototype1SearchPolicy, Prototype1ChildBudget>`.
- Baseline eval run root: `/home/brasides/.ploke-eval/instances/prototype1/p1-walk30g5c-pplxembed-r5-det-g25p-p25f-20260620-175208/BurntSushi__ripgrep-2295/runs/run-1782003272473-structured-current-policy-e8e82501`.
- Baseline tool-loop review written by sub-agent: `tool-loop-reviews/cycle-00-baseline-run-1782003272473.md`.
