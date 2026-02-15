# Context Management Planning

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

This separation becomes the foundation for definitions: each step produces an artifact to log, display, and later replay.

## 3) Key Concepts

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

This is the artifact that makes runs reproducible.

### Atom

Grouped items that serve as an irreducible context unit

  - Tool call + response (enforced by API)
  - Conversation Turn: Needs to be a unit to have enough continuity to be
  useful, minimum is user message + llm response chain.


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

### Candidate Item

An input the planner *could* include:

* message group (turn slice / episode / pinned sysinfo)
* rag snippet
* focus hint / system prompt
  Each candidate has:
* stable id, section, token estimate, priority, TTL state, dependencies.

### Orientation Scaffold
  - module tree
  - tiny “how to navigate” note for the LLM, not the user (e.g., “use
  request_code_context to expand bodies”)
  - (possibly) 1-3 highest centrality nodes code module path + doc comments (if present)

### Pin

pin message to persist in context

  - sticky: user-added or architecture contract/key types
  - leased: auto-added and dropped after a given scope
  - ephemeral: thin pack to provide short cards (function
  signature, module path + short descr) as hook to provide more
  info

### Turn
A single "Turn" begins at the user's message, includes the model's tool
calls, and ends when the model returns control to the user (via stop token or
when it stops calling tools).

### Lease Scope

The scope in which a context item is kept in the context window.
  - Can be `TurnId` (see Turn) or `TaskId` (see experimental Task)

### Symbol Card

core info on node

  - module path
  - file path
  - short snippet (up to 5 lines)
  - doc comment (if present)
  - cfg attributes (if present)

### Promotion

If a pinned item is referenced/used frequently in conversation,
can promote from epheremeral to leased, only user pins as sticky

### Activation Score

Give items a score that updates as the
conversation continues by scoring relevence to task

  - included in the last ContextPlan
  - directly referenced by the model (e.g., it quoted a symbol name or file path)
  - directly referenced by the user

### Candidate Touchpoints
  - 6–12 symbol cards with:
  - path + signature + doc comment (if present)
  - short excerpt window (up to 30 lines)
  - provenance + why-retrieved

### Context Mode

Profile that determines limits for the rest of the context assembly process

Three modes:
- Off: never auto-retrieve
- Light (default): module map + thin touchpoint cards (cheap, low bloat)
- Heavy: include exemplars / larger bodies / more candidates

UI-wise:
- a small indicator near the input: CTX: Light
- hotkey cycles: Ctrl+f (or whatever) → Off/Light/Heavy
- config default + per-session override

This solves:
- “frictionless by default” without surprise bloat
- power users can turn it off
- when the user asks a big “implement X” question, they can bump intensity

### Budget Meter

Small helper tip with info on amount of context auto-included
as attachment line after user message.
  - provides insight into "Context Mode" token impact
  - includes tip for hotkey to learn more (provides breakdown of automatic context + reasons)
  
### Feedback + Reflection

persist info on which items where promoted and why, or why not included.

  - which plan items were actually included (PlanId)
  - which retrieval candidates existed but were excluded (RetrievalSetId + reasons)
  - which tools were called and what they returned (ToolTrace)
  - estimate vs actual token usage (UsageReport)

## Advanced/Experimental Concepts

- Query clusters (“phases”) without relying on the LLM’s self-awareness
  - Every retrieval / traversal call already has an implicit query. Assign it a QueryClusterId.
  - Cluster formation can be simple and deterministic:
    - if cosine similarity between the new query embedding and current cluster
    centroid < threshold → new cluster
    - else → same cluster

- LLM-suggested sticky pins:
  - Allow the model to emit suggestions (“These 3 items seem foundational; consider pinning.”)
  - UI shows a non-blocking hint: Suggested pins (3) — see context mgmt overlay
    - in context mgmt overlay suggested pins appear dimmed at the bottom of the
    list after a clear delimiter like "suggested pins"
  - Nothing stops the run; it’s an invitation.

- Capsules: LLM written summaries of modules/processes with staleness tracking
  - stored with source = LLM, confidence, generated_at, and code-hash/commit-hash
  - marked stale when the underlying file changes
  - never treated as ground truth—just as a retrieval aid / orientation layer

- KG-aware Orientation Scaffold:
  - include 5–8 “anchors” (entrypoints) if known
  - populated from LLM-generated Capsules

- Advanced Activation Score:
  - used as a hop target in traversal (callers/callees)
    - requires tracking intermediate hops and/or shortest path between items
  - returned by retrieval for the current query cluster (more below)
    - requires clustering

- Task: An automatically or LLM-annotated "Task" the user is setting for the LLM.
  - Used to scope leased context items.
  - Currently a fuzzy concept, needs to be clearly defined.
