# Eval Triage Rubric

This note sharpens what the smoke-eval phase is for, what the baseline phase is
for, and which failures should or should not be fixed before the first reported
full run.

## Larger Object

The thing being measured is not just "did the model solve the task." The
experiment is trying to measure:

- whether the evaluation substrate is valid enough to trust
- what failures belong to the current semantic frontier
- what failures belong to the current tool/action surface
- whether later automated tool refinement improves outcomes while holding the
  evaluation substrate and known frontier constraints fixed

The tempting reduction to avoid is:

- fix every observed failure before baseline

That would collapse architecture evidence into setup work and make later
improvements harder to interpret.

## Failure Classes

Every smoke-eval failure should be assigned one primary class.

### 1. Harness Invalidity

Definition:
- the benchmark did not fairly exercise the architecture because the evaluation
  substrate was broken or stale

Typical examples:
- stale paths or fixtures
- dead external API assumptions
- malformed tool schema or unusable tool registration
- patch production/apply bookkeeping bugs
- eval runner crashes
- result recording or submission formatting bugs

Rule:
- fix before baseline

Reason:
- these failures invalidate measurement

### 2. Known Frontier Limit

Definition:
- the run failed because the system lacks semantic substrate that is already
  known to be out of near-term scope

Typical examples:
- missing `macro_rules!` understanding
- missing proc-macro expansion
- parser/indexing gaps already accepted as current frontier constraints

Rule:
- do not fix before baseline unless the project scope explicitly changes

Reason:
- these failures are real, but they are not the primary target of the immediate
  self-improvement loop

### 3. Action-Surface Failure

Definition:
- the benchmark had enough substrate to be fair, but the available tools,
  contracts, evidence surfaces, or action protocol were too weak

Typical examples:
- no first-class action for creating a needed Rust item
- the model had the right context but could not express the needed move
- evidence lookup was adequate in principle but the tool contract made the
  action unreliable or awkward
- the model overused a fallback because the intended semantic tool was too weak

Rule:
- preserve in the baseline
- prioritize after baseline for automated refinement

Reason:
- this is the main object of study for the current loop

## Smoke-Eval Protocol

Smoke evals are evaluation-validity checks, not the reported baseline.

For each smoke failure:

1. Ask whether the benchmark fairly exercised the architecture.
2. If no, classify as `Harness Invalidity` and fix it.
3. If yes, ask whether the failure depends on a known out-of-scope semantic
   frontier.
4. If yes, classify as `Known Frontier Limit` and retain it.
5. Otherwise classify as `Action-Surface Failure` and retain it.

## Baseline Protocol

The first full reported run should happen only after:

- the harness is admissible
- the tool surface is intentionally fixed for that run
- known frontier limits are documented rather than quietly patched away

The baseline should then report at least:

- overall outcomes
- failures tagged by class
- the subset of failures eligible for immediate automated refinement

## Optimization Target

For the first self-improving loop, the primary target should be:

- `Action-Surface Failure`

The loop may observe `Known Frontier Limit`, but should not claim those as
near-term improvements unless the semantic frontier itself changes.

## Practical Operating Rule

Use this short rule during triage:

- if the run was not fair, fix it now
- if the run was fair but hit a known frontier, tag it and move on
- if the run was fair and the action surface lost, keep it in baseline and use
  it to drive refinement
