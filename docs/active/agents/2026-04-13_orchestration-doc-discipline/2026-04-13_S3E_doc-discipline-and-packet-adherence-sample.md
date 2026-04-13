# S3E - Doc Discipline And Packet Adherence Sample

- Date: 2026-04-13
- Owner role: worker
- Layer/workstream: A4
- Related hypothesis: The eval workflow degrades when stale docs accumulate and when orchestration drifts away from packet-first, document-backed execution; a small sampled audit should be enough to show whether a targeted protocol tightening is justified
- Design intent: turn the current concern about stale-doc/archive handling and packet-first drift into a bounded `S3` observational packet rather than a broad process rewrite
- Scope:
  - sample a small set of recent orchestration/eval artifacts
  - assess whether packet-first discipline is being followed
  - assess whether stale/superseded docs are being clearly marked, localized, or retired
  - determine whether the current doc lifecycle rules are sufficient or need one narrow explicit addition
- Non-goals:
  - do not rewrite the whole orchestration protocol
  - do not build archive tooling or telemetry
  - do not restructure the entire `docs/active/agents/` tree
- Owned files:
  - `docs/active/agents/2026-04-12_eval-orchestration-protocol/**`
  - `docs/active/workflow/**`
  - selected recent packet/report docs under `docs/active/agents/**`
  - `AGENTS.md`
- Dependencies:
  - accepted `S3C`
  - accepted `S3D`
  - 2026-04-13 doc-discipline recon note
- Acceptance criteria:
  1. Apply a small rubric to a real sample of recent orchestration packets and related docs.
  2. Distinguish packet-first drift from stale-doc lifecycle drift instead of collapsing them together.
  3. Identify whether an existing rule already covers the observed problem or whether one narrow new rule is justified.
  4. Recommend the smallest supported follow-up, such as a protocol clarification, a directory-local archive convention, or no change.
- Required evidence:
  - sampled artifact list
  - explicit rubric dimensions
  - concrete findings with file references
  - concise recommendation tied to the sample
- Report-back location: `docs/active/agents/2026-04-13_orchestration-doc-discipline/`
- Status: ready

## Suggested Rubric

- `packet_first`
  - was the packet written before worker dispatch?
- `prompt_dryness`
  - did worker prompts reference the packet instead of duplicating it?
- `resume_locality`
  - could a replacement orchestrator recover the task from docs alone?
- `doc_lifecycle_clarity`
  - are stale/superseded docs clearly marked or localized?
- `selector_provenance`
  - when a task depends on a specific run or artifact, is that selection made explicit?

## Suggested Sample

- the recent inspect CLI UX lane
- one earlier accepted `S3` packet/report pair
- one current control-plane or workflow pointer path

## Decision Rule

- if the sample shows isolated drift only, prefer a small protocol clarification and local cleanup guidance
- if the sample shows repeated drift across packet-first and stale-doc handling, open a narrow follow-up to tighten the orchestration protocol and doc lifecycle conventions together
