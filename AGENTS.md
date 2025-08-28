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
  - Run targeted and full test suites; prefer brief inline summaries (pass/fail/ignored counts and notable failures). Avoid writing run outputs to docs by default.
  - When deeper artifacts are required (e.g., live API diagnostics), keep them under `logs/` or `target/test-output/` and do not commit them unless explicitly requested.
  - Update design/reflection docs when making notable trade-offs.
  - Live gates discipline: When a live gate is ON (e.g., OpenRouter tests), do not report tests as green unless the live path was actually exercised and key properties were verified (tool_calls observed, proposal staged, approval → Applied, file delta). A “skip” must be treated as “not validated” and must not be counted as pass under live gates.
  - Evidence for readiness: For any claim of phase readiness, include verifiable proof in your summary (pass/fail counts, properties satisfied) and reference artifact paths generated under `target/test-output/...`. If evidence is missing, explicitly state that readiness is not established.

Operational Notes
- Plans, logs, and reports live in `docs/plans/agentic-system-plan/` and `docs/reports/`.
- Reference key docs from plan files so future agents easily discover prior work.

Rust Doc Comments (non-negotiable formatting)
- Use `//!` only for a single, file-top inner doc block documenting the crate or module.
- Use `///` for item-level docs (functions, structs, fields, tests) and inline explanations.
- Do not place `//!` after imports or mix multiple `//!` blocks in a file; prefer one contiguous block at the very top.
- Integration test files are crate roots; if documenting the whole test crate, put a single `//!` block at the very top. Otherwise, prefer `///` on specific items.
