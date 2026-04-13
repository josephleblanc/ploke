# EDR-0002: CLI trace review skill experiment

- status: complete
- date: 2026-04-13
- owning_branch: `refactor/tool-calls`
- review_cadence: revisit after each comparison round
- update_trigger: update before running the prompt comparison and again after each round with outcome and promotion decision
- owners: design-mode main agent, future orchestrator
- hypothesis ids: `A1`, `A5`
- related issues/prs: none yet
- linked manifest ids: none
- linked artifacts: `docs/active/agents/2026-04-13_cli-trace-review-skill-meta-experiment.md`

## Decision

Run a bounded comparison of multiple instruction templates for CLI-only
`ploke-eval` trace review before promoting the workflow into a durable repo-local
skill.

## Why Now

The current eval workflow increasingly depends on post-run CLI inspection of
tool-call traces to distinguish tool robustness gaps, tool workflow gaps, and
true model mistakes. That analysis pattern appears reusable, but the instruction
set is not yet stable enough to freeze into a skill without testing.

The recent ripgrep follow-up exposed exactly the kind of ambiguity this workflow
must handle well:

- repeated semantically similar queries suggest a sequence-level story
- `read_file` failures expose possible tool robustness gaps and weak recovery UX
- premature model blame would likely hide actionable harness or tool debt

## Control And Treatment

- control:
  ad hoc trace review without a dedicated reusable instruction template
- treatment:
  three bounded instruction-template variants applied to the same CLI evidence
  surface and compared with a shared rubric
- frozen variables:
  same latest-run target unless explicitly changed, same CLI-only boundary, same
  shared output schema, same comparison rubric

## Acceptance Criteria

- primary:
  one instruction variant clearly outperforms ad hoc review on structure,
  actionability, and discipline against unsupported model blame
- secondary:
  the winning variant's output fits packet-report or postmortem use with minimal
  rewriting
- validity guards:
  reviewers stay inside the CLI-only evidence boundary, do not read
  `crates/ploke-eval/` source during the comparison, and keep the evidence
  surface comparable across variants

## Plan

1. Prepare the comparison note with instruction variants, shared output schema,
   and rubric.
2. Run the first-round comparison on the same latest-run CLI evidence surface.
3. Compare outputs, revise the strongest variant once, and run one more round.
4. Update this record with the outcome and decide whether to promote the winner
   into `docs/workflow/skills/`.

## Result

- outcome:
  adopted after round 1; no second bake-off needed
- key metrics:
  qualitative prompt comparison only
- failure breakdown:
  - Variant A performed best on sequence-aware narrative, turning points, and disciplined avoidance of unsupported model blame
  - Variant B performed best on failure bucketing and repeated-miss pattern identification
  - Variant C performed best on concrete intervention proposals and highest-leverage follow-up framing
- surprises:
  - all three variants independently converged on `read_file` missing-path recovery as the highest-signal robustness gap
  - the patch-batching failure remained useful evidence, but all variants treated it as secondary to the earlier path-recovery loop
  - prompt variance changed emphasis more than conclusion, which suggested the workflow itself was already stable enough to promote

## Decision And Follow-Up

- adopt / reject / inconclusive:
  adopt
- next action:
  use Variant A as the durable base and capture the stable learned behavior in `docs/workflow/skills/cli-trace-review/SKILL.md`
