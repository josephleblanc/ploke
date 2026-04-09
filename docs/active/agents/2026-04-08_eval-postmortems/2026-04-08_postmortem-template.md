# Eval Postmortem Template

- date: 2026-04-08
- task title: Eval Postmortem Template
- task description: Reusable template for documenting eval-run failures with an emphasis on separating model mistakes from tool-design and runner-design issues.
- related planning files:
  - [2026-04-08_postmortem-plan.md](/home/brasides/code/ploke/docs/active/agents/2026-04-08_eval-postmortems/2026-04-08_postmortem-plan.md)

## Header

- batch id:
- batch manifest:
- run id:
- instance:
- model:
- provider:
- repository:
- base sha:
- stable evidence source:
- artifact paths:
  - run manifest:
  - execution log:
  - turn summary:
  - turn trace:
  - submission jsonl:
  - official benchmark logs/report:

## Outcome Snapshot

- final runner state:
- final chat outcome:
- primary user-visible failure:
- did the model produce a patch:
- did the target file change:
- official benchmark status:
- official benchmark evidence:

## Failure Classification

- primary category:
- secondary category:
- confidence:

Use one or more of:

- `model-drift`
- `tool-affordance-gap`
- `tool-retry-friction`
- `semantic-edit-limitation`
- `artifact-ambiguity`
- `provider-behavior`

## Timeline

1. Initial diagnosis:
2. First meaningful tool failure:
3. First edit proposal:
4. First compile or test failure:
5. End-of-run state:

## Evidence

### Correct Local Reasoning

- What did the model understand correctly?
- What evidence shows that?

### Tool Friction

- Which tools failed?
- Were the recovery hints useful or harmful?
- Did tool payloads misrepresent staged vs applied state?

### Model Mistake

- What decision or assumption caused the run to fail?
- Was the model solving the wrong problem or over-generalizing a correct diagnosis?

### Artifact Ambiguity

- Did the final artifacts tell the truth?
- If not, which fields or summaries were misleading?

### Benchmark Follow-Through

- If the official Multi-SWE-bench evaluator was run, what did it conclude?
- If not, what follow-up would be needed to get a benchmark verdict?

## Minimal Correct Fix

Describe the smallest correct code change the model should have made.

## Open Questions

- Tool-design questions:
- semantic editing capability questions:
- runner or artifact questions:

## Follow-Up Actions

- instrumentation:
- tool UX:
- runner artifact changes:
- regression tests:
