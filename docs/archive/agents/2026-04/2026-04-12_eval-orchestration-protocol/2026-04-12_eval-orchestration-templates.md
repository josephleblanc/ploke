# Eval Orchestration Templates

**Date:** 2026-04-12
**Task Title:** Eval orchestration templates
**Task Description:** Provide compact task packet and report templates for the eval orchestration protocol.
**Related Planning Files:** `docs/active/agents/2026-04-12_eval-orchestration-protocol/2026-04-12_eval-orchestration-protocol.md`, `docs/active/plans/evals/eval-design.md`, `docs/active/CURRENT_FOCUS.md`

## Task Packet Template

```md
# <task_id> - <title>

- Date:
- Owner role:
- Layer/workstream:
- Related hypothesis:
- Design intent:
- Scope:
- Non-goals:
- Owned files:
- Dependencies:
- Acceptance criteria:
- Required evidence:
- Report-back location:
- Status:
```

Use a compact current-state table in any active control-plane doc that tracks this packet. Keep the table limited to the fields the orchestrator needs to resume work:

| task_id | status | owner | layer/workstream | packet_link | latest_report_link | next_action |
| --- | --- | --- | --- | --- | --- | --- |

## Worker Report Template

```md
# <task_id> - Worker Report

- Date:
- Worker:
- Status:
- Implemented:
- Claims:
- Evidence:
- unsupported_claims:
- not_checked:
- Risks:
- Next step:
```

Report rules:

- map each claim to a numbered acceptance criterion when possible
- keep evidence bounded to the packet scope and cite the concrete file, command, or artifact inspected
- use `unsupported_claims` for any assertion the evidence does not actually support
- keep `not_checked` explicit so the verifier does not have to infer gaps
- keep `risks` short and realistic rather than speculative

## Verifier Note Template

```md
# <task_id> - Verifier Note

- Date:
- Verifier:
- Packet reviewed:
- Claims checked:
- Evidence inspected:
- Supported claims:
- Unsupported or weak claims:
- Gaps against acceptance criteria:
- Budget used:
- Risks:
- Recommended disposition:
```

Verifier notes should stay within the cited packet and evidence set unless the budget is exceeded or a new issue forces escalation.

## Orchestrator Acceptance Note Template

```md
# <task_id> - Orchestrator Disposition

- Date:
- Decision: accepted | revise | split | blocked | superseded
- Basis for decision:
- Follow-up packet(s):
- Workflow/docs updated:
```
