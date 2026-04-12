# S3D - Restart Rubric Observational Sample

- Date: 2026-04-12
- Owner role: worker
- Layer/workstream: A4
- Related hypothesis: The workflow/control-plane protocol is effective if a small sample of recent handoffs and control-plane states lets a replacement orchestrator recover context, locality, and next steps with low ambiguity
- Design intent: Turn the accepted `S3C` inventory into a narrow observational packet instead of a speculative process rewrite
- Scope: Sample a small recent cohort of handoffs/control-plane updates and score them against a simple restart rubric for context recovery, decision locality, and next-step discoverability
- Non-goals: Do not build workflow telemetry, do not prescribe a broad governance process, do not depend on chat-history retention
- Owned files: `docs/active/workflow/**`, `docs/workflow/**`, related sprint docs as needed
- Dependencies: accepted `S3B`, accepted `S3C`
- Acceptance criteria:
  1. The packet applies a simple restart rubric to a small real sample of recent artifacts.
  2. The output distinguishes where the current workflow is working from where ambiguity still leaks through.
  3. The packet recommends the next smallest workflow/process change only if the sample supports it.
- Required evidence:
  - sampled artifact list
  - explicit rubric dimensions and findings
  - concise recommendation tied to the sample rather than general preference
- Report-back location: `docs/active/agents/2026-04-12_eval-infra-sprint/`
- Status: accepted

## Permission Gate

No additional user permission required for doc-only observational work.
