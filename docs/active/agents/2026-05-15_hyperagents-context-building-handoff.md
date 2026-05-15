# 2026-05-15 HyperAgents Context-Building Handoff

Short description: restart note for the Prototype 1 broad-harness prompt/RAG issue discovered after the first clean live headless-TUI splice, now updated for the context-mode Off adapter patch and the next multi-generation smoke campaign.

Related planning files:
- [`docs/design/drafts/edit-surface/hyperagents-prompt-posture.md`](../../design/drafts/edit-surface/hyperagents-prompt-posture.md)
- [`docs/workflow/evalnomicon/chat-history/on-hyper-agents.md`](../../workflow/evalnomicon/chat-history/on-hyper-agents.md)
- [`.agents/hyper-agents.txt`](../../../.agents/hyper-agents.txt)
- [`2026-05-12_hyperagents-broad-harness-orchestration-handoff.md`](2026-05-12_hyperagents-broad-harness-orchestration-handoff.md)

## Current Evidence

The clean live splice succeeded from a separate clean source checkout:

- clean source worktree: `/home/brasides/.ploke-eval/worktrees/p1-live-splice-clean-20260515-1`
- base commit: `8de50435dab2a230dfa417c4f18e9b255f819a02`
- request: `/home/brasides/.ploke-eval/campaigns/p1-sparse-context-broad-tui-20260514-4/prototype1/messages/edit-harness-request/node-e8202a069a00d7f5-r7.json`
- result: `/home/brasides/.ploke-eval/campaigns/p1-sparse-context-broad-tui-20260514-4/prototype1/messages/edit-harness-result/node-e8202a069a00d7f5-r7.json`
- diagnostics: `/home/brasides/.ploke-eval/campaigns/p1-sparse-context-broad-tui-20260514-4/prototype1/messages/edit-harness-result/node-e8202a069a00d7f5-r7.headless-tui.json`
- admitted changed path: `crates/ploke-tui/src/test_harness.rs`

The candidate changed `TEST_APP_DB` from `fresh_backup_fixture_db(&FIXTURE_NODES_CANONICAL)` to `ploke_test_utils::fixture_dbs::get_fixture_db_with_cache(... FixtureAccess::FreshMutable)` and rewrote the nearby comment as a cached fixture performance claim.

This is useful as a transport/admission proof. It is not useful evidence that the candidate improves MBE, oracle outcomes, or the Prototype 1 self-improvement process.

## What We Misread

The broad prompt posture is intentional. The design note explicitly says not to collapse broad edit surfaces into deterministic target-file routing. The HyperAgents lesson is not "tell the agent exactly which file to edit." It is:

- keep the outer archive/traversal/evaluation loop stable and inspectable;
- expose evidence and budget;
- let the harness choose an intervention inside the allowed surface;
- let evaluation and selection decide usefulness;
- allow successful descendants to develop better evaluation analysis, prompt infrastructure, memory, and strategy.

The observed failure is therefore not simply "the prompt was too broad." The failure is that automatic context construction injected source snippets selected by generic request-level RAG, and the harness treated those snippets as an implicit local neighborhood to improve.

## Why `fixture_dbs.rs` Was In Context

The prompt diagnostic for r7 recorded:

- `context_mode = Light`
- `bm25.status = ready`
- `bm25.docs = 5792`
- `included_rag_parts = 15`
- one selected RAG part from `crates/test-utils/src/fixture_dbs.rs`, score `136.83635`

The code path uses the submitted user message as the RAG query:

```rust
rag.get_context(
    &user_msg,
    profile.top_k,
    &budget,
    &retrieval_strategy,
    RetrievalScope::LoadedWorkspace,
)
```

The persisted diagnostic does not record per-term scoring or a textual "why this snippet" explanation. It records the selected snippets and scores. The best evidence-based interpretation is that the broad request text contained performance/test/headless/evaluation language, and generic RAG surfaced fixture DB code as plausibly related. The model then chased that local affordance.

## Tool Trace Summary

The r7 diagnostic recorded:

- `tool_request`: 20
- `tool_completed`: 17
- `tool_failed`: 4
- `proposal`: 1

Tool usage:

- `list_dir`: 8 requests
- `read_file`: 7 requests
- `request_code_context`: 1 request
- `code_item_lookup`: 2 requests, both failed for `TEST_APP_DB` / `TEST_APP`
- `apply_code_edit`: 1 request, failed on nonexistent `crate::test_harness::TEST_APP_DB_CACHE`
- `non_semantic_patch`: 1 request, succeeded and staged the final patch

The semantic Rust edit tools are currently brittle around macro-defined items such as `lazy_static!` contents. The fallback patch tool can still stage a candidate.

## Design Implication

For broad-harness self-improvement requests, automatic code-context injection is not neutral. It can steer the model toward whatever generic RAG says is nearby, without the agent first performing evaluation analysis.

This does not mean the prompt should become a target-file picker. It means the harness needs a different context posture:

1. Keep the broad HyperAgents-style edit permission.
2. Expose evidence roots and compact run/evaluation artifacts.
3. Do not automatically inject arbitrary source-code RAG snippets into the initial prompt for this mode.
4. If automatic context is retained, restrict it to stable contract/context material such as the request contract, protected-core definition, and evidence index. Source-code snippets should be requested intentionally by the agent through tools.
5. Persist the retrieval query, selected snippets, scores, and reason/category so later evaluation can distinguish agent-chosen evidence from harness-injected context.

The richer object is not a prompt that returns a file edit. It is a broad self-modification protocol with separate phases:

- request contract;
- evidence inventory;
- agent-chosen inquiry;
- proposed edit;
- admission;
- evaluation;
- archive traversal.

The current implementation collapses request contract and initial code context. That collapse is what made `fixture_dbs.rs` feel like instruction rather than incidental context.

## 2026-05-15 Adapter Patch State

The context-posture fix now uses existing `ploke-tui` configuration rather than a new broad-harness prompt mechanism:

1. `crates/ploke-eval/src/cli/prototype1_state/edit_surface/tui_adapter.rs` sets `runtime.state.config.write().await.context_management.mode = ploke_tui::user_config::CtxMode::Off` before submitting broad headless TUI prompts.
2. `PromptDiagnostic::context_unavailable_reason()` no longer treats the intentional Off-mode fallback notice as a fatal missing-context condition.
3. The RAG/BM25 service remains configured and ready, so explicit tools such as `request_code_context` still work.
4. Existing live canaries now assert the split facts: workspace/BM25 ready, automatic prompt RAG off, and zero initial RAG parts.

Verification already run in the active checkout:

- `cargo fmt --all`
- `cargo test -p ploke-eval context_unavailable 2>&1 | tail -n 80`
- `cargo test -p ploke-eval live_tui_initial_prompt_off_skips_rag_parts -- --ignored --nocapture 2>&1 | tail -n 120`
  - observed BM25 ready with `5792` docs;
  - observed `included_rag_parts=0`;
  - observed fallback `Context mode is Off; proceeding without code context.`
- `cargo test -p ploke-eval live_tui_request_code_context_ploke_workspace_returns_results -- --ignored --nocapture 2>&1 | tail -n 140`
  - explicit `request_code_context` returned snippets.
- `cargo test -p ploke-eval live_tui_adapter_canary_uses_indexed_context_before_applied_edit -- --ignored --nocapture 2>&1 | tail -n 180`
  - provider-backed canary requested `request_code_context` before `apply_code_edit`;
  - staged the expected `src/lib.rs` change.

At this handoff, the adapter patch is still a local source change unless it has been committed after this note. Commit it before setting up the next campaign so the Runtime/Artifact identity includes the prompt-posture fix.

## Next Useful Work

The next implementation/run slice is multi-generation continuity for broad surface edits, not more prompt tuning:

1. Commit the context-mode Off adapter patch.
2. Run one clean live broad-harness splice against an existing published request and confirm diagnostics show:
   - `context_mode = Off`;
   - `included_rag_parts = 0`;
   - ready BM25;
   - explicit source/evidence inspection through tools before editing.
3. Set up the next smoke campaign from a fresh, separate worktree under `~/.ploke-eval/worktrees`; do not run it from the active development checkout.
4. Use `~/.ploke-eval/profiles/prototype1/smoke-3x4-edit-surface.toml` first. The goal is three generations with four children each, proving:
   - broad harness requests publish child slots;
   - children run and produce applied/rejected/failed evidence;
   - successor selection picks one child;
   - successor hydration creates the next parent with correct Artifact/Runtime identity;
   - generation N+1 runs from that successor parent.
5. If `smoke-3x4` completes, move to `five-gen-3-child-edit-surface.toml`. Only after that should a 10-generation profile be used, preferably `10x3` before `10x6`.

Observation should stay metadata-first during smoke runs:

- admitted node count;
- generation range;
- status counts for attempted/applied/rejected/failed children;
- prompt diagnostics count by `context_mode` and initial RAG part count;
- tool counts for `request_code_context`, `read_file`, edit tools, and failed tools;
- terminal reason if the run stops.

Fix only generation-blocking failures first: checkout/source-dirty contamination, provider/tool timeout handling, context-unavailable misclassification, missing child evidence, successor-selection failure, or successor hydration/parent identity mismatch.

Do not "fix" this by hard-coding benchmark target files into the prompt. That would violate the HyperAgents prompt posture and regress broad edit support into a brittle route table.
