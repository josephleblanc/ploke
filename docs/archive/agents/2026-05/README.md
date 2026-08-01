# 2026-05 Archive

May 2026 archive bucket for topic trees moved from [`docs/active/agents/`](../../../active/agents/readme.md) with `git mv`, preserving internal relative links inside each moved folder.

## Keep vs archive (pre-spine refresh)

Use these defaults when triaging [`docs/active/agents/`](../../../active/agents/readme.md); adjust only when a path is explicitly the current thread’s **next implementation step**.

| Disposition | Rule |
| --- | --- |
| **Keep in `docs/active/agents/`** | Linked from the **spine** (see below) as the restart surface **or** called out there as the live “read this next” step for the current work. Living indexes (`open-questions.md`, `notable-inconsistencies.md`) stay active or are folded into `docs/active/workflow/` as you prefer—they are not archived just for being old. |
| **Archive under `docs/archive/agents/2026-05/<topic>/`** | Everything else that is dated or superseded topical material: prefer **moving whole directories** rather than splitting files so sibling relative links survive. Topic names stay the same basename as under `agents/`; **do not** move [`AGENTS.md`](../../../../AGENTS.md) or `.cursor/plans/` (out of scope for this archive layout). |

Heuristic from lineage: folders described in the spine as “legacy sprint / archive index” are **archive** once the spine no longer depends on paths under active.

## Spine files — single PR, serialized with moves

These files define **restart trajectory** for humans and agents. **Only one PR at a time** should edit them **or** they are updated in one **final batch after** move PRs merge (same plan: moves first, then one spine pass). Parallel branches that each edit these will force painful rebases.

Spine set for the `docs/active/agents` archive wave:

- [`docs/active/agents/readme.md`](../../../active/agents/readme.md)
- [`docs/active/CURRENT_FOCUS.md`](../../../active/CURRENT_FOCUS.md)
- [`docs/active/workflow/handoffs/recent-activity.md`](../../../active/workflow/handoffs/recent-activity.md)

Optional related handoffs (repair links when moves land; same serialization rule if they reference moved paths):

- [`docs/active/workflow/handoffs/2026-04-17_protocol-design-reset.md`](../../../active/workflow/handoffs/2026-04-17_protocol-design-reset.md)

## Topics in this bucket

Moved from [`docs/active/agents/`](../../../active/agents/readme.md) on **2026-05-08** (whole files/trees preserved):

- **`2026-05-07-*` edit-surface thread:** phase reviews (`edit-surface-phase[1-4]-*`), integration slice, backend bridge, resolver extraction, scoring evidence CLI review, loop rerun readiness review.
- **`2026-05-07-prototype1-edit-surface-*.md`:** handoff and implementation plan for bounded TUI/edit-surface wiring into Prototype 1; the model draft now lives in `docs/workflow/evalnomicon/drafts/edit-surface/model.md`.
- **`2026-05-07-tui-edit-surface-producer-{note,review}.md`** — deterministic parent-side producer notes.
- **History readiness reviews dated 2026-05-01:** `history-startup-state-review-2026-05-01/`, `single-ruler-readiness-2026-05-01/`, `startup-history-invariant-review-2026-05-01-0955/`.
