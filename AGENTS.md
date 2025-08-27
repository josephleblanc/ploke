# Agents Operating Guide

Purpose
- Define the interaction workflow between human and agents, and codify non-negotiable engineering principles for this codebase.

Workflow: Plan → Review → Implement
- When asked to produce a plan, create a standalone plan document under `docs/plans/agentic-system-plan/impl-plan/plan_YYYYMMDD-HHMMSSZ.md` containing:
  - Objectives and scope
  - Proposed steps with rationale and acceptance criteria
  - Deliverables and validation approach
- The human reviewer can either approve or propose changes.
  - If changes are proposed, you MUST update the plan document to incorporate them.
  - You MAY create a secondary document under `docs/plans/agentic-system-plan/impl-plan/` with:
    - Open questions / blockers requiring clarification
    - A brief critique of the requested changes if they degrade quality or introduce risk
- After the plan is approved or updated per instructions, proceed to implement according to the plan.
- Always link implementation logs (`impl_*.md`) to the corresponding plan file.

Engineering Principles
- Strong typing everywhere (no stringly typed plumbing):
  - All OpenRouter-touching code and tool schemas must use strongly typed structs/enums with `Serialize`/`Deserialize` derives (numeric fields as numeric types, e.g., `u32` for tokens, `f64` for costs).
  - Prefer enums and tagged unions to “detect” shapes; make invalid states unrepresentable.
  - Use `#[serde(untagged)]` only as a migration bridge with explicit deprecation notes.
  - Treat ad-hoc JSON maps and loosely typed values as errors at the boundaries; validate early and surface actionable messages.
- Safety-first editing:
  - Stage edits with verified file hashes; apply atomically via the IoManager; never write on hash mismatch.
  - Keep destructive operations behind explicit approvals.
- Evidence-based changes:
  - Run targeted and full test suites; record outputs under `docs/plans/agent-system-plan/testing/`.
  - Update design/reflection docs when making notable trade-offs.

Operational Notes
- Plans, logs, and reports live in `docs/plans/agentic-system-plan/` and `docs/reports/`.
- Reference key docs from plan files so future agents easily discover prior work.

