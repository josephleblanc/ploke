# HyperAgents Review Underread Existing Docs

Date: 2026-05-17

## Trigger

The user said the HyperAgents comparison could have been better and called out
missed points around held-out tests, improvement-memory projections, and the
existing History/Crown protocol-upgrade and ledger/consensus design direction.

## User-Visible Failure

The response gave a broadly useful gap list but flattened several existing
Prototype 1 design commitments into "missing" work. It also did not clearly
explain that HyperAgents itself uses training/validation feedback while
withholding a final test set for reporting generalization.

## Touched Surface

- `.agents/hyper-agents.txt`
- `.agents/prototype1-hyperagents-handoff-2026-05-06.md`
- `crates/ploke-eval/src/cli/prototype1_state/mod.rs`
- `crates/ploke-eval/src/cli/prototype1_state/history.rs`
- `docs/active/agents/2026-05-15_hyperagents-context-building-handoff.md`
- `docs/active/agents/2026-05-12_tui-adapter-boundary-review/prompt-evidence-oracles.md`
- `docs/workflow/evalnomicon/chat-history/on-hyper-agents.md`

## What The Agent Did

The agent read the paper and several local files, but stopped before integrating
the existing HyperAgents handoff, History/Crown module docs, and prompt/evidence
handoff docs into the comparison. The resulting answer treated some design work
as absent instead of distinguishing implemented, designed-but-unwired, and
paper-derived missing surfaces.

## Skipped Docs / Skills / Instructions

- Did not fully apply the semantic-architecture requirement to recover the
  richer existing object before naming gaps.
- Did not use sub-agents on the initial research review, despite the breadth of
  the comparison.
- Did not re-read `.agents/prototype1-hyperagents-handoff-2026-05-06.md` before
  contrasting Prototype 1 with HyperAgents.
- Did not distinguish local History/Crown tamper evidence from future
  distributed consensus/finality clearly enough.

## Why This Was Risky

Prototype 1 has accumulated design commitments around authority, mutable
surfaces, archive traversal, typed evidence, and protocol upgrades. Underreading
those commitments can push follow-up work toward duplicate architecture,
incorrect "missing feature" claims, or unsafe shortcuts around policy-surface
self-modification.

## Prevention Rule

Before giving a strategic review comparing Prototype 1 to a research system,
first classify each claimed gap as one of: absent, implemented, implemented but
narrow, designed but unwired, or intentionally protected behind a future
protocol-upgrade transition. If the comparison spans paper, code, and planning
docs, use sub-agents before the final review.

## Memory Hypothesis

Existing memory correctly warned not to overfit stale runtime docs, but the
review needed the complementary rule: when the user references HyperAgents, read
the current HyperAgents handoff and History/Crown docs before naming architecture
gaps.
