# Plan: Context Window Management for ploke-tui

Context: The current prompt assembly path concatenates RAG context and chat history without
explicit budgeting or consistent selection rules. Tokens are tracked for observability but not
used to enforce a model context limit. This plan defines a deterministic, model-aware context
window manager that selects messages and RAG snippets within budget, supports TTL policies, and
exposes inclusion/exclusion details in the UI.

This document includes: design goals, proposed data model and algorithm, a sequence diagram,
acceptance criteria, and test strategy. No code changes have been made yet.

---

## Goals

- Ensure the prompt fits within a model-aware context window with a safety buffer.
- Make context selection deterministic and explainable (include/exclude reasons).
- Respect TTL and user pinning policies across turns.
- Surface context usage and composition in the UI.
- Maintain backward compatibility when model metadata is missing.

Non-goals (for this phase):
- Automatic summarization of older messages.
- Tool-specific custom prompts beyond existing tool messages.
- Multi-model concurrent context windows.

---

## Current State (Summary)

- `ChatHistory::current_path_as_llm_request_messages` returns a path of messages with minimal
  filtering (SysInfo only if pinned).
- `rag/context.rs` prepends RAG context parts as system messages and then appends messages.
- `llm/manager/mod.rs` estimates token count but does not trim or budget.
- `ChatHistory::decrement_ttl` runs after each successful completion, but TTL does not influence
  inclusion.
- UI shows a placeholder context preview and a basic "ctx tokens" count.

---

## Proposed Design

### 1) New Context Window Manager

Add a module responsible for constructing a `ContextPlan`:

```
ContextPlan {
  model_context_len,
  max_completion_tokens,
  prompt_budget,
  safety_buffer,
  sections: {
    system_pinned,
    conversation,
    rag,
  },
  included: Vec<PlannedItem>,
  excluded: Vec<ExcludedItem>,
  estimated_tokens_total,
}
```

Each `PlannedItem` records:
- `source`: system / pinned / conversation / rag / tool
- `priority`: High / Normal / Low
- `ttl`: Limited(n) / Unlimited / NoneRemaining
- `reason`: Required / Pinned / Recent / Budget / Policy
- `token_estimate`

### 2) Budget Model

Determine model context length and max completion tokens from registry/endpoint metadata when
available; otherwise use defaults from config. Compute:

```
prompt_budget = model_context_len - max_completion_tokens - safety_buffer
```

Split the prompt budget into section budgets (defaults in config):
- `system_pinned`: 15%
- `conversation`: 55%
- `rag`: 30%

If a section has no content, its budget is reallocated to the others.

### 3) Selection Algorithm

Selection order:
1) Required items:
   - Base system prompt.
   - Focus hint (if a crate is loaded).
   - Tool call chain requirements.
2) Pinned items (priority order, then newest).
3) Conversation history (most recent user+assistant pairs first).
4) RAG context parts (highest relevance first).

Rules:
- Always keep the most recent user message, even if over budget (trims down to minimal prompt).
- If over budget after selection, drop lowest-priority pinned items (except `Unlimited`).
- TTL policy applies before budgeting:
  - `Automatic`: drop TTL expired (NoneRemaining).
  - `Ask`: keep for one turn and emit a SysInfo prompt to repin or drop.
  - `Unlimited`: keep.
- RAG selection should drop whole items (not partial snippets) to fit budget.

### 4) UI and Telemetry

- Replace the placeholder context preview with a list of included/excluded items, token estimates,
  and reasons (budget/TTL/policy).
- Status line: show `ctx tokens: X / prompt_budget` and model context length.
- Emit a `tokens` log with plan summary (section usage, counts, budget).

---

## Sequence Diagram (ASCII)

```
User -> App: Submit message
App -> ChatHistory: add user message
App -> RagService: get_context(search, budget_hint)
RagService --> App: AssembledContext (parts)
App -> ContextWindowManager: build plan(
  messages, rag parts, model caps, ctx policy
)
ContextWindowManager --> App: ContextPlan (included/excluded)
App -> LLM Manager: send RequestMessage list (plan.included)
LLM Manager -> ChatHistory: set current_context_tokens(plan.estimated)
LLM Manager -> Chat Session: run request
Chat Session --> App: completion + usage
App -> ChatHistory: decrement_ttl
App -> UI: render context preview + status line
```

---

## Acceptance Criteria

1) **Model-aware budgeting**
   - Given model context length and max completion tokens, the prompt budget is computed and
     enforced with a configurable safety buffer.

2) **Deterministic selection**
   - For identical inputs (messages, RAG parts, config), the same set of included items is produced.
   - The latest user message is always included.

3) **TTL policy enforcement**
   - `Automatic`: TTL-expired pinned messages are excluded from the prompt.
   - `Ask`: TTL-expired pinned messages remain for one turn and a SysInfo prompt appears.
   - `Unlimited`: TTL-expired messages remain indefinitely.

4) **RAG item trimming**
   - RAG items are either fully included or fully excluded; no mid-snippet truncation.

5) **UI visibility**
   - Context preview shows included/excluded items with reasons.
   - Status line shows `ctx tokens: <estimate> / <budget>`.

6) **Telemetry**
   - A single log record per request captures budget, section usage, counts, and totals.

---

## Test Strategy

### Unit Tests

- Budget computation:
  - `prompt_budget` matches formula and respects safety buffer.
- Selection:
  - Latest user message always included.
  - Deterministic ordering by priority and recency.
  - Dropping of lowest-priority pinned items when over budget.
- TTL policy:
  - Automatic/Ask/Unlimited behaviors.
- RAG trimming:
  - Drop whole RAG items only.

### Integration Tests

- Prompt assembly:
  - `ContextPlan` used instead of raw concatenation.
  - Current context tokens updated from plan estimate.
- UI:
  - Context preview reflects plan inclusion/exclusion and reasons.

---

## Implementation Notes

- Primary integration points:
  - `rag/context.rs` (prompt construction).
  - `llm/manager/mod.rs` (request preparation and token tracking).
  - `chat_history.rs` (TTL and message metadata).
  - `app/mod.rs` (context preview and status line).
  - `user_config.rs` (budget settings).

- Backward compatibility:
  - If model metadata is missing, fall back to config defaults.

