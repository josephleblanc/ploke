# 2026-05-15 HyperAgents Context-Building Handoff

Short description: restart note for the Prototype 1 broad-harness prompt/RAG issue discovered after the first clean live headless-TUI splice.

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

## Next Useful Work

The next implementation slice should be in the headless-TUI broad-harness adapter, not branch scoring:

1. Add a `BroadHarnessContextPolicy` or equivalent existing-structure carrier that can disable generic source RAG for broad edit requests while leaving normal user-chat RAG untouched.
2. Persist enough prompt-context provenance to explain later why a snippet was shown.
3. Make the submitted return evidence distinguish:
   - agent-requested evidence;
   - harness-injected contract context;
   - harness-injected source snippets, if any remain.
4. Re-run one live request and verify the model must inspect evidence/source through explicit tools before editing.

Do not "fix" this by hard-coding benchmark target files into the prompt. That would violate the HyperAgents prompt posture and regress broad edit support into a brittle route table.
