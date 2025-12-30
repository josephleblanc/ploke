Here are “do-it-this-week” steps that keep Ploke working, don’t require a full rewrite, and still move you toward the architecture we just designed. I’m ordering these by **leverage / risk**.

NOTE: This is a set of conceptual, high-level suggestions, which was reviewed and made actionable for this code base in `docs/active/todo/2025-12-30_context-management-grounded-plan.md`

---

## 1) Stop the worst bloat now: split your TTL into Sticky vs Leased (minimal schema change)

Right now you have “keep all tool calls for next 10 requests.” That’s a blunt instrument.

**Immediate change**

* Introduce two retention classes in metadata (even if they’re just booleans/enum with defaults):

  * `retention = Sticky | Leased | Ephemeral`
* Default everything that was previously “kept by TTL” to **Leased**.
* Keep your existing “10 requests” TTL as the leased default, *but* add an eviction rule:

  * **Only decrement TTL for items that were actually included in the last request.**
  * If an item wasn’t included, it doesn’t get its TTL refreshed.

This single tweak prevents “tool spam” from persisting indefinitely just because the model ran a lot of calls early.

---

## 2) Add a lightweight ContextPlan *without changing prompt assembly yet*

You don’t need the CWM to control inclusion on day 1. You need it to **describe** what happened.

**Immediate change**

* Create a `ContextPlan` struct and produce it right before building the request.
* Fill it with:

  * model caps (or config fallback)
  * estimated token totals (even if rough)
  * the list of included items (just ids + type + rough token estimate)
  * excluded items can be empty for now

**Why now?**

* This lets you build UI + telemetry + tests incrementally.
* Later you’ll switch from “plan describes” → “plan decides” with less churn.

---

## 3) Make “Light pack” the default: module map + symbol cards, not full bodies

Your current retrieval returns top 10 items. If those are full bodies, that’s often too heavy.

**Immediate change**

* Change the retrieved item rendering into **Symbol Cards**:

  * path, signature, doc comment (if any), and a short excerpt window
* Keep full bodies behind an explicit tool call:

  * `expand_symbol_body(symbol_id)` or `request_code_context(symbol_id, depth=...)`

This reduces noise *and* nudges the model toward intentional deepening.

---

## 4) Atomize tool episodes (tiny refactor, big coherence win)

Even before budgets, you want to stop the “tool result without context” failure modes.

**Immediate change**

* When building the message list for the LLM, treat tool events as an **Atom**:

  * include tool call + tool result together, or drop both together when TTL excludes them
* Same for “assistant message that triggered tool” if you can cheaply group it.

This aligns you with the future “atomicity contract” without implementing full planning.

---

## 5) Add a leased budget cap (simple numeric guardrail)

You don’t need fancy selection yet. You just need “leased doesn’t run away.”

**Immediate change**

* Add a config value like:

  * `max_leased_tokens` (or `max_leased_items`)
* When assembling the request, include leased items newest-first (or highest activation if you implement it) until you hit the cap.
* Everything beyond the cap is excluded (and recorded in ContextPlan as `Budget`).

Even a crude cap prevents “30k tokens of yesterday’s discoveries.”

---

## 6) Add a tiny Activation signal (no embeddings, no graph required)

You can get 80% of “activation” with two cheap signals.

**Immediate change**

* Track per leased item:

  * `last_included_turn` (or request counter)
  * `include_count`
* Selection order for leased items becomes:

  * newest included first, then include_count (tie-breaker stable id)

Later you can add “referenced by model/user” parsing, but you’ll already have a working decay mechanism.

---

## 7) Add Context Mode as a UI-only knob first (Off/Light/Heavy)

Don’t wire mode to sophisticated planning yet—wire it to just one thing: **how much auto-retrieval you do**.

**Immediate change**

* `Off`: no auto retrieval (only conversation + sticky + required)
* `Light` (default): module map + 6–10 symbol cards
* `Heavy`: module map + 12–20 symbol cards (or include 1–2 exemplars)

Show `CTX: Light` in the status line and add a hotkey to cycle it.

---

## 8) Add the “Budget meter line” after the user message (non-blocking)

This is a cheap UX win and builds trust.

**Immediate change**

* After sending, append a dim SysInfo line:

  * `CTX Light · retrieved 8 items · est +2.4k tokens · press P for context details`
* Make it config-toggleable if you want (default on is probably fine if it’s subtle).

---

## 9) One “integration test” that locks in the direction

Pick a forcing function that makes future refactors safer.

**Immediate change**

* Add a golden test for `ContextPlan` determinism:

  * given fixed messages + fixed retrieval results + config, plan output stable
* Even if the plan isn’t enforcing budgets yet, you’re locking in the *artifact*.

---

# Suggested implementation order (fastest path)

1. Retention enum + “decrement TTL only if included”
2. ContextPlan artifact + logging
3. Symbol Cards (thin pack) + atomize tool episodes
4. Leased cap + simple activation ordering
5. Context Mode knob + budget meter line
6. Golden plan test

This gets you:

* immediate bloat reduction
* better prompt coherence
* user-visible transparency
* the scaffolding to later “flip the switch” and let the plan drive inclusion.

If you paste the current prompt assembly flow (the functions that gather chat history + tool messages + retrieval items), I can map these steps onto your actual modules with the smallest possible diff.
