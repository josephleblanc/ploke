
Yep—before we tighten “TTL” or “turn group” definitions, it helps to write down the *user/agent loop* you’re actually trying to support, and what artifacts you need to produce along the way.

Below is a framing you can drop straight into the doc as a “Foundations / Definitions” chapter. It’s organized as:

1. the desired user/agent flow (what the system is for)
2. the core artifacts (what must be recorded)
3. how code-suggestion quality + telemetry + future multi-branch all fall out of the same contracts
4. a first pass at definitions that compose cleanly

---

## 1) North-star goals and the intended loop

### The product loop

**Intent → Context → Action → Feedback → Reflection (→ Memory)**

* **Intent**: user asks a question / gives a task; may include focus scope (crate, module, files).
* **Context**: system assembles *explainable* prompt inputs (history + pinned + RAG + tool episode glue) within a strict budget.
* **Action**: model proposes edits / runs tools / answers questions.
* **Feedback**: user accepts/rejects, tools run (cargo), diffs produced, errors surfaced.
* **Reflection**: system updates TTLs, records what context was used, and stores telemetry that connects “what we showed the model” to “what happened.”
* **Memory (later)**: summarization, durable notes, user pinning workflows, branch-local “facts,” etc.

### Why this framing matters

You want three properties at once:

1. **High-quality code suggestions**
   Comes from *relevant* context + stable prompting + reliable tool feedback loops.

2. **Good telemetry for the code graph**
   Comes from treating each LLM request as a **traceable experiment**: inputs → retrieval → tools → outputs → user decisions.

3. **Future multi-branch concurrency**
   Comes from making each request/plan a **branch-scoped artifact** that can be reproduced and compared.

So the real goal isn’t just “fit budget.” It’s:

> Every model call should be reproducible, explainable, and attributable to a branch + context snapshot.

---

## 2) System boundaries and responsibilities (who does what)

### Components (conceptual)

* **ChatHistory / Conversation Store**
  Canonical source of messages, pins, TTL counters, branch structure, metadata.

* **RAG / Retrieval**
  Produces *candidate evidence items* (snippets) with scores + provenance (file, symbol, graph node ids).

* **Context Window Manager (CWM)**
  Produces a **ContextPlan**: a deterministic selection of items to include/exclude under budget, with reasons.

* **LLM Manager / Session**
  Executes the plan, records actual usage, handles retries/timeouts, surfaces tool calls.

* **Tool System**
  Executes commands (cargo, file reads, edits) and returns structured outputs.

* **UI**
  Shows: context composition, budget, item reasons, tool traces, and outcomes.

This separation becomes the foundation for definitions: each step produces an artifact you can log, display, and later replay.

---

## 3) First-class artifacts (the things you should be able to point at)

If you want good telemetry + multi-branch later, you need these artifacts per request:

1. **Turn** (user submission event)
2. **RetrievalSet** (what candidates RAG produced, with provenance)
3. **ContextPlan** (what was actually sent to the model, and why)
4. **ToolTrace** (what tools ran, inputs/outputs, durations, errors)
5. **UsageReport** (estimated vs actual tokens, latency, provider metadata)
6. **Outcome** (assistant response + user accept/reject actions + applied diffs)

The CWM is “just” the plan builder—but the system’s *power* comes from making the plan a durable, inspectable artifact.

---

## 4) Contracts that make everything compose

These are the “rules of the world” that your definitions should enforce:

### Determinism contract

Given the same:

* message store state (including TTL/pins),
* retrieval candidates + scores,
* config + model caps,

…the ContextPlan must be identical (same included ids, order, reasons).

This is what makes debugging, regression tests, and branch comparisons possible.

### Explainability contract

Every included/excluded decision must have a reason that maps to one of:

* required,
* pinned,
* TTL policy,
* budget,
* dependency (e.g., tool episode atom),
* duplication / shadowed / superseded.

This is what makes UI useful instead of “mystery trimming.”

### Atomicity / dependency contract

Some items must be included/excluded as a unit:

* tool call ↔ tool result (and often the follow-up assistant message),
* turn pairs (user question ↔ assistant answer) depending on your chosen grouping.

This prevents nonsensical prompts.

### Budget safety contract

Unless explicitly in a “hard overflow / truncated latest user message” mode:

* `estimated_prompt_tokens <= prompt_budget`.

And if overflow happens, it must be explicit and visible.

### Attribution contract (telemetry)

Every request is attributable to:

* a **branch id**,
* a **context snapshot id** (plan id),
* a **retrieval set id**,
* a **tool trace id**.

That’s how you later ask: *“Why did branch B solve it but branch A didn’t?”*

---

## 5) Definitions that fit this world

Here’s a clean, composable set of definitions (you can refine names later):

### Message

A single authored utterance with role + content + metadata.

* `MessageId` stable
* belongs to exactly one `BranchId`
* may carry flags: `pinned`, `ttl`, `is_sysinfo`, etc.

### Turn

A user submission event plus any immediately associated assistant work.

* Identified by `TurnId`
* Contains one “root” user message + subsequent assistant/tool messages until completion (or failure).

(You can still store as individual messages; “Turn” is a logical grouping for planning + UI.)

### Episode (Tool Episode)

A group of messages that must remain coherent:

* assistant tool call message(s)
* tool result message(s)
* assistant follow-up message(s) (optional but often valuable)

This is your atomic unit for tool-chain inclusion rules.

### Candidate Item

An input the planner *could* include:

* message group (turn slice / episode / pinned sysinfo)
* rag snippet
* focus hint / system prompt
  Each candidate has:
* stable id, section, token estimate, priority, TTL state, dependencies.

### RetrievalSet

The ordered list of candidate evidence items returned by RAG for a turn.

* stable `RetrievalSetId`
* each item has provenance: file path, span, symbol id, graph node ids, score.

(Important: this is what ties LLM decisions back to the code graph.)

### ContextPlan (Context Snapshot)

The ordered list of groups/messages actually sent to the model for a specific request.

* stable `PlanId`
* includes: included + excluded with reasons
* includes computed budgets and estimates

This is *the* artifact that makes runs reproducible.

### Branch

A named timeline of turns/messages.

* `BranchId`
* parent pointer (optional) + fork point (message/turn id)
* has its own head pointer
* can have independent pinned/ttl state (or inherit—your choice, but define it)

### Branch Run (future)

An execution instance that advances a branch (possibly concurrently).

* has a `RunId`
* binds a model, config snapshot, and scheduling metadata

This is how you scale to multiple branches “running simultaneously” without mixing concerns.

---

## 6) How this supports multi-branch later (without redesign)

If you treat `RetrievalSet` + `ContextPlan` as branch-scoped artifacts, then multi-branch becomes:

* Each branch advances via independent runs:

  * `BranchId + TurnId → RetrievalSet → ContextPlan → LLM → Outcome`
* Shared resources (db, file system, tool execution) can be arbitrated separately without breaking the mental model.
* UI can compare plans between branches:

  * “branch A included snippet X, branch B didn’t”
  * “branch B overflowed budget and truncated latest user message”
  * “branch A used older pinned sysinfo that expired in B”

This is exactly where determinism + attribution pay off.

---

## 7) Suggested doc restructuring (so definitions land well)

Add a top section before the “Plan”:

1. **Product loop and user flow** (intent→context→action→feedback→reflection)
2. **Artifacts and contracts** (what must be recorded, why determinism matters)
3. **Definitions** (Turn/Episode/Branch/Plan/etc.)
4. Then return to your current plan details (budget model, selection algorithm, UI, tests)

---

If you want, next we can take *your existing plan text* and rewrite it with these definitions baked in—so “Selection order” talks about **candidate groups**, “TTL” talks about **pinned message groups**, and acceptance criteria references **PlanId/RetrievalSetId attribution** instead of loose phrasing.
