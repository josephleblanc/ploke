# Review of context-management plan

re: `docs/active/plans/context-management/context-management.md`

## Findings

  - High: The plan promises reproducible, explainable runs but doesn’t specify the determinism inputs you
    must snapshot (repo/graph version, tokenizer, model config, tool outputs, retrieval seeds), so
    RetrievalSetId/PlanId won’t actually be replayable without that contract. docs/active/plans/context-
    management/context-management.md:31, docs/active/plans/context-management/context-management.md:61,
    docs/active/plans/context-management/context-management.md:70, docs/active/plans/context-management/
    context-management.md:98, docs/active/plans/context-management/context-management.md:201
  - High: Branch isolation rules are left as “your choice,” but promotion/TTL updates can race across
    concurrent BranchRuns, which will make ContextPlans nondeterministic unless you define per-branch
    pin/TTL inheritance, conflict resolution, and write ordering. docs/active/plans/context-management/
    context-management.md:87, docs/active/plans/context-management/context-management.md:94, docs/active/
    plans/context-management/context-management.md:96, docs/active/plans/context-management/context-
    management.md:153
  - Medium: Lease Scope relies on TaskId, yet “Task” is explicitly fuzzy, so lease semantics can’t be
    implemented or tested until Task is defined. docs/active/plans/context-management/context-
    management.md:136, docs/active/plans/context-management/context-management.md:237
  - Medium: Activation scoring/promotion/touchpoints lack decay, caps, and negative signals (user
    rejection, tool errors), which risks context bloat and sticky stale items dominating the budget.
    docs/active/plans/context-management/context-management.md:151, docs/active/plans/context-management/
    context-management.md:165, docs/active/plans/context-management/context-management.md:190
  - Medium: There are no explicit success metrics or evaluation loops tied to the stated goals (quality,
    telemetry, concurrency), so it’s unclear how you’ll validate whether the system improves outcomes or
    just adds complexity. docs/active/plans/context-management/context-management.md:18, docs/active/
    plans/context-management/context-management.md:197

  - High: The plan doesn’t define a concrete “request snapshot” schema for reproducibility
    (model+tokenizer config, repo/graph revision, retrieval seeds, tool outputs). Without this,
    RetrievalSetId/PlanId aren’t replayable and the north‑star goal is undermined. docs/active/plans/
    context-management/context-management.md:12, docs/active/plans/context-management/context-
    management.md:61
  - High: Branch concurrency is underspecified (“your choice”) while TTL/pin promotion and activation
    scoring imply mutation; without explicit isolation, ordering, and conflict policy, concurrent
    BranchRuns will be nondeterministic. docs/active/plans/context-management/context-management.md:87,
    docs/active/plans/context-management/context-management.md:94, docs/active/plans/context-management/
    context-management.md:153
  - Medium: Lease Scope relies on “TaskId” but Task is not defined, so lease semantics can’t be
    implemented or tested. docs/active/plans/context-management/context-management.md:136
  - Medium: Activation/promotion lacks decay/caps/negative signals, so stale items can dominate the
    budget and create runaway context bloat. docs/active/plans/context-management/context-
    management.md:151, docs/active/plans/context-management/context-management.md:165
  - Medium: There are no explicit success metrics tied to the stated goals (quality, telemetry,
    concurrency), so validation is ambiguous. docs/active/plans/context-management/context-
    management.md:18, docs/active/plans/context-management/context-management.md:197
  - Low: “Orientation scaffold” and “candidate touchpoints” overlap with “symbol cards,” but no dedupe/
    precedence is defined; risks redundant context units in a tight budget. docs/active/plans/context-
    management/context-management.md:112, docs/active/plans/context-management/context-management.md:172,
    docs/active/plans/context-management/context-management.md:165

  Questions / Assumptions

  - Should ContextPlan include the exact model config + tokenizer + repo/graph snapshot hash to make
    replay deterministic, or is “replayable enough” defined differently?
  - Is pin/TTL state strictly per-branch, or can it be inherited/shared with a write policy (e.g., copy-
    on-write at fork)?
  - How will you treat negative feedback (tool errors, user rejections) in activation scoring and
    promotion?
  - CM-05: Should the leased cap be token-based (`max_leased_tokens`) or item-count based, and where
    should it live in config?
  - CM-05: Ordering for leased activation—`last_included_turn` desc then `include_count` desc, then a
    stable id for determinism?
  - CM-05: What is the canonical “turn” counter for `last_included_turn` (TurnId vs request counter)?
  - CM-05: Apply the cap to grouped tool episodes (post CM-03) or individual messages?
  - CM-05: Should excluded leased items be recorded in ContextPlan with a Budget exclusion reason?

  Overall, the framing is strong and coherent; the main gaps are determinism contracts, branch
  concurrency rules, and measurable success criteria.

## Possible ways to address the findings (grounded in current code)

  - Determinism/replay: capture a concrete "request snapshot" at the point RAG context is assembled
    in `crates/ploke-tui/src/rag/context.rs` and the prompt/messages are finalized in
    `crates/ploke-tui/src/llm/manager/mod.rs`; emit the snapshot through the existing tracing
    pipeline in `crates/ploke-tui/src/tracing_setup.rs` so it lands alongside token/tool logs with a
    shared run id.
  - Branch isolation: `ChatHistory` already models branching, but TTL mutation is global via
    `ChatHistory::decrement_ttl` in `crates/ploke-tui/src/chat_history.rs` and
    `StateCommand::DecrementChatTtl` in `crates/ploke-tui/src/app_state/dispatcher.rs`; a minimal
    fix is to decrement only along the current path, while a stronger fix is to store per-branch
    TTL/pin overlays keyed by the current branch root id inside `crates/ploke-tui/src/chat_history.rs`.
  - Task/lease scope: define an initial TaskId as the parent user message id already flowing through
    `ChatEvt::PromptConstructed` in `crates/ploke-tui/src/rag/context.rs` and
    `crates/ploke-tui/src/llm/manager/mod.rs`, then attach that id to messages in
    `crates/ploke-tui/src/chat_history.rs` and drive lease behavior with the existing context policy
    config in `crates/ploke-tui/src/user_config.rs`.
  - Activation scoring/negative signals: use the structured error/finish handling in
    `crates/ploke-tui/src/llm/manager/session.rs` plus tool error payloads in
    `crates/ploke-tui/src/tools/cargo.rs` to penalize or decay activation; implement decay and caps
    alongside TTL updates in `crates/ploke-tui/src/chat_history.rs` or in the TTL command path in
    `crates/ploke-tui/src/app_state/dispatcher.rs`.
  - Metrics/evaluation: stitch together `ChatHistory::record_usage_delta` and `ContextTokens` in
    `crates/ploke-tui/src/chat_history.rs`, token estimates in `crates/ploke-tui/src/llm/manager/mod.rs`,
    and tool-call traces in `crates/ploke-tui/src/tracing_setup.rs` to define success metrics; use
    `ChatSessionReport` outcomes in `crates/ploke-tui/src/llm/manager/session.rs` as the primary
    per-run label for quality/regression tracking.
  - Deterministic snapshot schema: extend `ChatEvt::PromptConstructed` in
    `crates/ploke-tui/src/llm/manager/events.rs` to carry a structured snapshot (model/provider id,
    tokenizer name, repo/graph revision, retrieval req_id/seed, and tool policy); populate it in
    `crates/ploke-tui/src/rag/context.rs` (retrieval metadata) and
    `crates/ploke-tui/src/llm/manager/mod.rs` (model/config), then emit via
    `crates/ploke-tui/src/tracing_setup.rs` alongside the run id so it is replayable.
  - Retrieval provenance: thread the RAG request id already used in
    `crates/ploke-tui/src/rag/search.rs` into the assembled context in
    `crates/ploke-tui/src/rag/context.rs`, then store it on the prompt event in
    `crates/ploke-tui/src/llm/manager/events.rs` so RetrievalSetId can be traced back to the exact
    retrieval call.
  - Branch-safe TTL mutation: move TTL decrement into a branch-scoped operation by teaching
    `ChatHistory::decrement_ttl` in `crates/ploke-tui/src/chat_history.rs` to take the current branch
    head (or fork point) and only walk that path; keep `StateCommand::DecrementChatTtl` in
    `crates/ploke-tui/src/app_state/dispatcher.rs` as the gate for mutation so concurrency policy is
    enforced in one place.
  - Task/lease grounding: add a TaskId field to message metadata in
    `crates/ploke-tui/src/chat_history.rs` and initialize it from the `parent_id` in
    `ChatEvt::PromptConstructed` in `crates/ploke-tui/src/llm/manager/mod.rs`; then use
    `crates/ploke-tui/src/user_config.rs` context-management settings to decide whether to keep items
    through that TaskId or just TurnId.
  - Negative-signal decay: convert structured errors from
    `crates/ploke-tui/src/llm/manager/session.rs` and tool failures from
    `crates/ploke-tui/src/tools/cargo.rs` into a decay signal that updates activation scores in
    `crates/ploke-tui/src/chat_history.rs` (cap + decay + penalty on error/rejection).
  - Context dedupe: coalesce overlapping RAG parts (symbol cards vs touchpoints) in
    `crates/ploke-tui/src/rag/context.rs` by merging `ContextPart` with the same path/symbol, and add
    a final de-dup pass in `crates/ploke-tui/src/llm/manager/mod.rs` before the message list is sent
    to the model to avoid redundant system context.
