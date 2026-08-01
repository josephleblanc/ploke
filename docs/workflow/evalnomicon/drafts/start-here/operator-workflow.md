# Operator Workflow Map

The workflow skills describe how an operator should move through the loop.
They are not runtime authority. They are procedure guides for setup,
diagnostic stepping, blocker repair, and run review.

## Skill To Stage Map

| Skill | Use it when | Loop stages it covers |
| --- | --- | --- |
| `prototype1-run-setup` | creating a fresh worktree/campaign or taking the first bounded step | setup, admission, doctor, first `baseline_eval` or next diagnosed phase |
| `diagnostic-loop-run` | advancing or diagnosing one bounded loop step | any doctor phase, especially post-step evidence classification |
| `loop-blocker-repair` | a step, doctor check, replay, protocol run, or review finds a blocker | `blocked` branch and invalid-evidence stop branch |
| `run-review` | a run root exists and the question is what actually happened | eval/protocol/tool/patch/oracle trace reconstruction after child or baseline runs |

## Fresh Run Workflow

Use this when there is no trustworthy campaign yet, or when a prior campaign
has been abandoned because required transition evidence is invalid.

1. Confirm the checkout and worktrees.

   ```bash
   git status --short --branch
   git worktree list
   ```

2. Check current command surfaces instead of trusting stale notes.

   ```bash
   ./target/debug/ploke-eval loop prototype1-setup --help
   ./target/debug/ploke-eval loop prototype1-doctor --help
   ./target/debug/ploke-eval loop prototype1-step --help
   ./target/debug/ploke-eval model --help
   ```

3. Resolve the model route and provider from the current model surfaces and the
   profile. Do not infer a route from a path name or campaign id.

4. Create a neutral seed worktree. Let `prototype1-setup` create the parent
   identity branch.

5. Run setup from the new worktree or use a known binary with an explicit
   `--repo-root`.

6. Run doctor and confirm:

   - parent identity exists and matches the campaign
   - admitted run profile and commitment exist
   - model/provider route matches the intended run
   - phase is expected, usually `baseline_eval`
   - blockers are empty

7. Take at most one bounded step unless the user explicitly asks for continuous
   execution.

## Diagnostic Step Workflow

Use this for normal loop operation.

1. Anchor the run:

   ```bash
   ./target/debug/ploke-eval loop prototype1-doctor --repo-root . --format json
   ./target/debug/ploke-eval closure status --campaign <campaign> --format json
   git status --short --branch
   ```

2. Read admitted config from the campaign, not from memory:

   ```bash
   cat ~/.ploke-eval/campaigns/<campaign>/campaign.json
   cat ~/.ploke-eval/campaigns/<campaign>/prototype1/run-profile.toml
   cat ~/.ploke-eval/campaigns/<campaign>/closure-state.json
   ```

3. Advance one phase:

   ```bash
   ./target/debug/ploke-eval loop prototype1-step --repo-root . --format json
   ```

4. Always re-read state after the step, including after command failure.

5. If new run roots or child artifacts exist, collect enough evidence to decide
   whether the transition was trustworthy. Do not equate a terminal summary with
   semantic success when artifacts disagree.

## When To Use Continue

Use `prototype1-continue` only when the active parent checkout, binary
provenance, campaign, and profile are clear. It repeatedly advances diagnosed
phases and can run child phases with the admitted parallel cap.

Do not use `continue` when:

- the campaign is already `blocked`;
- provider route or request shape is uncertain;
- required eval, protocol, oracle, or History evidence is invalid;
- another loop command is already running against the same campaign;
- you are trying to inspect one transition boundary.

## Run Review Handoff

Switch from diagnostic stepping to run review when a run root exists and the
question becomes "what actually happened?"

Run review must keep these ledgers separate:

- mechanical completion: rows and artifacts exist
- benchmark outcome: patch, submission, oracle/MBE, checkout state
- provider trace: raw model responses, tool calls, finish reasons, usage
- recorded tool lifecycle: requested/completed/failed/staged/applied
- information success: whether returned tool payloads were useful
- learning signal: whether the model used evidence to improve behavior
- protocol judgment: what protocol adjudication saw or missed

For edit tools, reconstruct the lifecycle per call id. A staged proposal
completion is not the same as an applied patch.

## Blocker Branch

Stop advancing the campaign when continuing would create misleading evidence or
when required transition evidence is missing, malformed, contradictory, or not
admissible.

Use repair-and-resume only when the persisted campaign state remains
trustworthy and the broken contract is in code, configuration, environment, or
an idempotent preflight.

Use abandon-and-restart when the current run has already admitted or persisted
invalid required eval, protocol, oracle, selection, or History evidence. Do not
patch readers just to reinterpret that run as successful.

## Operator Rules

- Prefer `prototype1-step` for diagnostic work.
- Use the active checkout's binary for live parent execution unless deliberately
  launching a known binary with an explicit `--repo-root`.
- Do not print secrets, raw bearer tokens, credential file contents, or
  unnecessary provider metadata.
- Do not mutate active run artifacts while diagnosing.
- Treat environment-only failures as operator handoff, not code regressions.
- Treat repeated same-file edit failures as a missing invariant, not an
  isolated stale-index accident.

