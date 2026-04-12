# S1B - Ploke-Eval Coherence Cleanup

- Date: 2026-04-12
- Owner role: worker
- Layer/workstream: A5
- Related hypothesis: `ploke-eval` is easier to extend and safer to rely on if the eval/inspection surface is coherent, documented, and free of obviously repeated or weakly-structured code paths
- Design intent: Turn the accepted coherence audit into a bounded cleanup pass over `ploke-eval` now that the primary P0 replay/inspection lane is accepted
- Scope: Identify and tighten the highest-value coherence issues in `ploke-eval`, with emphasis on API shape, repetition, documentation, and trivially passing or weakly-signaled tests
- Non-goals: Do not redesign the whole eval system, do not reopen accepted P0 packet scope without explicit evidence, do not broaden into parser or core runtime cleanup outside `ploke-eval`
- Owned files: `crates/ploke-eval/**`, related sprint docs as needed
- Dependencies: `S1A` report, accepted `P0A`-`P0F`
- Acceptance criteria:
  1. The packet identifies a bounded cleanup slice in `ploke-eval` with clear rationale tied back to the accepted coherence audit.
  2. The output distinguishes code-shape cleanup from behavior changes and notes any acceptance-boundary risks.
  3. The output recommends the next smallest useful cleanup implementation packet or produces it directly if the slice is already implementation-ready.
- Required evidence:
  - sampled file list or targeted diff summary
  - concise findings or changes tied to concrete file references
  - explicit note on test-strength implications
  - recommended follow-up packet(s) if more than one cleanup slice remains
- Report-back location: `docs/active/agents/2026-04-12_eval-infra-sprint/`
- Status: ready

## Permission Gate

No additional permission required if work stays inside `crates/ploke-eval/`.
