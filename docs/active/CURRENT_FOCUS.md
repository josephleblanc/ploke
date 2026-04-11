# Current Focus

**Last Updated:** 2026-04-10 (Solution selected for A2 parser issue, awaiting implementation)
**Active Planning Doc:** [Bug Report: syn 2.x Rust 2015 trait objects](../bugs/2026-04-10-syn-2-fails-on-rust-2015-bare-trait-objects.md)

---

## What We're Doing Now

We have **validated A4 (comprehensive result schema) and A5 (replay/introspection)** — the RunRecord system is working and the hard gate for H0 interpretation is unblocked. We discovered and **diagnosed an A2 (data fidelity) issue**: syn 2.x fails on Rust 2015 bare trait objects (e.g., `Arc<Fn(...)>`). **Solution selected:** Dual syn versions — syn 1.x for Rust 2015 crates, syn 2.x for modern Rust. Implementation is documented and ready to begin post-compaction.

---

## Immediate Next Step

**Implement dual syn version support:**
- Solution documented in [bug report](../bugs/2026-04-10-syn-2-fails-on-rust-2015-bare-trait-objects.md)
- Add syn 1.x as dependency alongside syn 2.x
- Create `code_visitor_syn1.rs` adapted for syn 1.x AST
- Add dispatch logic to route by edition
- Estimated effort: 3-4 weeks

**Recently completed:**
- Phase 1 COMPLETE — RunRecord implementation with emission, compression, and introspection API
- A4/A5 VALIDATED — Real `record.json.gz` verified, all 16 tests pass
- A2 issue DIAGNOSED and SOLUTION SELECTED — Dual syn approach chosen after evaluating 6 alternatives
- Qwen deserialization bug fixed — Feature flag `qwen_reasoning_fix` implemented

---

## Quick Links

| Ask me about... | Check this... |
|-----------------|---------------|
| "What were we up to?" | This doc ↑ |
| "Remind me of next steps" | [Phase 1 RunRecord Tracking](plans/evals/phase-1-runrecord-tracking.md) |
| "Let's pick up where we left off" | Fix globset parsing in syn_parser |
| Overall eval workflow | [workflow/README.md](workflow/README.md) |
| Recent activity log | [workflow/handoffs/recent-activity.md](workflow/handoffs/recent-activity.md) |
| Hypothesis status | [workflow/hypothesis-registry.md](workflow/hypothesis-registry.md) |

---

## Update Instructions (For Agents)

**When this doc changes:**
1. Update the "Last Updated" date
2. Update "Active Planning Doc" link
3. Update the "What We're Doing Now" paragraph (keep it to 3-5 sentences)
4. Update "Immediate Next Step" with the current actionable task

**When the active planning doc changes:**
- This doc should be updated immediately to point to the new planning doc
- The old planning doc should be marked complete/superseded with a link to the new one

**When the user asks recovery questions:**
- "What were we up to?" → Read this doc, summarize Current Focus paragraph
- "Remind me of next steps" → Read linked planning doc, summarize next uncompleted task
- "Let's pick up where we left off" → Read linked planning doc, identify where work paused, suggest resume point
