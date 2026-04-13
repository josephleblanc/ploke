# P2B - Ripgrep A2 Validation

- Date: 2026-04-12
- Owner role: worker
- Layer/workstream: `A2`
- Related hypothesis: The dual-`syn` implementation is now sufficient to restore fair mixed-edition parse readiness for ripgrep, so ripgrep can move from sentinel-only concern to evidence-backed Phase 2 baseline candidacy
- Design intent: Resolve the currently explicit `A2` gating ambiguity on the best-documented target family before any formal baseline/control packet is allowed to proceed
- Scope: Validate the dual-`syn` parser readiness on the documented ripgrep mixed-edition sentinel path, using the already named `BurntSushi__ripgrep-1294` failure and the "all 9 ripgrep crates indexed" success condition from the hypothesis registry, then update the target capability registry and hypothesis status only if the evidence supports it
- Non-goals: Do not run a full formal H0 baseline batch, do not broaden into tool UX or provider-policy changes, do not silently reinterpret failed parses as acceptable degradation
- Owned files: `docs/active/workflow/**`, `docs/active/agents/2026-04-12_eval-infra-sprint/**`, read-only inspection of relevant parser/eval code and artifacts
- Dependencies: accepted `P2A`, current `A2` entry in `hypothesis-registry.md`, ripgrep entry in `target-capability-registry.md`, documented bug at `2026-04-10-syn-2-fails-on-rust-2015-bare-trait-objects.md`
- Acceptance criteria:
  1. The packet produces concrete evidence for whether the documented ripgrep mixed-edition parser blocker is actually resolved in the current code path.
  2. The packet states whether ripgrep should remain a sentinel-only target, become a baseline candidate, or move to an intermediate registry state such as `resolved_pending_reentry`.
  3. The packet updates or recommends updates to the `A2` hypothesis and target capability registry without overstating what was checked.
- Required evidence:
  - named command(s) or artifact inspection(s) tied to the ripgrep sentinel
  - explicit statement about whether `globset` or the previously failing mixed-edition path now parses cleanly
  - explicit statement about whether all 9 ripgrep crates indexed, if that was checked
- Report-back location: `docs/active/agents/2026-04-12_eval-infra-sprint/`
- Status: accepted

## Permission Gate

No additional permission required for read-only validation and workflow-doc updates. If validation requires test or eval commands that need broader execution rights, request permission or user direction before broadening scope.
