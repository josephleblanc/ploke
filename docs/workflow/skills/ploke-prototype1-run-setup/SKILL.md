---
name: ploke-prototype1-run-setup
description: Use this skill when setting up or advancing a fresh Prototype 1 `ploke-eval loop` run in a new worktree, especially when choosing model/provider routing, admitting a run profile with `prototype1-setup`, running `prototype1-doctor`, taking a bounded `prototype1-step`, or preparing artifacts for later run review.
---

# Ploke Prototype 1 Run Setup

Use this skill for the operator side of a Prototype 1 run: fresh worktree,
model/provider selection, setup admission, doctor verification, and one bounded
advance. Pair it with `ploke-run-review` once a run root exists and the question
turns into "what actually happened?" If setup or the first bounded step exposes
a loop-blocking defect, hand off to `ploke-blocker-repair-loop` before advancing
the campaign again. If the previous campaign was abandoned because required
transition evidence became invalid, use this skill to start a new worktree and
campaign rather than trying to salvage the old one.

## Boundaries

- Do not duplicate deep run forensics here. After eval/protocol artifacts exist,
  use `ploke-run-review`.
- Do not print raw env, API keys, bearer tokens, ADC details, or full secret file
  paths. Report only presence/absence and provider route names.
- Do not pre-create the `prototype1-parent-<campaign>-gen0` branch.
  `prototype1-setup` creates that branch as the parent identity witness.
- Use `prototype1-step` for a bounded advance unless the user explicitly asks for
  continuous execution.
- Do not run overlapping `prototype1-step`, `prototype1-continue`, or eval
  commands against the same campaign/worktree.
- Do not reuse an abandoned campaign/worktree for loop progress. Leave it in
  place as evidence unless the user asks for cleanup, and create a new campaign
  id for the next run.

## Preflight

1. Confirm current checkout and branch:

   ```bash
   git status --short --branch
   git worktree list
   ```

2. Respect the Prototype 1 setup gate. If the user has not already provided a
   recent green baseline, run the agreed workspace gate before setup. If it is
   red, stop unless the user explicitly says the failure is environment-only and
   asks to proceed.

3. Check current CLI surfaces instead of trusting stale docs:

   ```bash
   ./target/debug/ploke-eval loop prototype1-setup --help
   ./target/debug/ploke-eval loop prototype1-doctor --help
   ./target/debug/ploke-eval loop prototype1-step --help
   ./target/debug/ploke-eval model --help
   ```

4. Decide the campaign id and profile shape. Prefer cloning the closest recent
   profile and changing only the run identity unless the user asks for policy
   changes.

## Model And Provider Selection

Resolve route and provider before setup:

```bash
./target/debug/ploke-eval model find <model-stem>
./target/debug/ploke-eval model refresh
./target/debug/ploke-eval model providers <model-id>
./target/debug/ploke-eval model current
./target/debug/ploke-eval model parent-patcher current
./target/debug/ploke-eval model provider current --model-id <model-id>
```

Rules:

- For direct Google rows, use `--route-source direct-google` and do not pin an
  OpenRouter provider. Passing `--provider google` is acceptable when a command
  accepts it as the explicit direct-Google sentinel; it should resolve to no
  stored provider slug.
- For OpenRouter catalog rows such as `google/gemini-3.5-flash`, pin a concrete
  OpenRouter provider such as `google-ai-studio` if the run should be stable.
- If the intended route is direct Google and the registry row is missing or
  still shows OpenRouter provenance, refresh the model registry and re-check the
  row before setup. Do not substitute `--route-source openrouter` plus an
  OpenRouter provider pin unless the user explicitly chooses an OpenRouter run.
- If `prototype1-continue` has no per-command model override, set both active
  model and parent-patcher model before running:

  ```bash
  ./target/debug/ploke-eval model set <model-id>
  ./target/debug/ploke-eval model parent-patcher set <model-id>
  ./target/debug/ploke-eval model provider set --model-id <model-id> <provider-slug>
  ```

Record the model and provider choices in the handoff. If you changed persisted
defaults, say so.

## Fresh Worktree Setup

Use a neutral seed branch for the initial worktree. Let setup create the parent
identity branch.

```bash
git worktree add -b seed-<campaign> ~/.ploke-eval/worktrees/<campaign> HEAD
```

Run setup from the new worktree. It is fine to use the already-built main
checkout binary for setup if the new worktree does not have `target/debug` yet:

```bash
/home/brasides/code/ploke/target/debug/ploke-eval loop prototype1-setup \
  --campaign <campaign> \
  --profile <profile-path> \
  --model-id <model-id> \
  --route-source <direct-google|openrouter> \
  --provider <provider-slug> \
  --protocol-model-id <model-id> \
  --protocol-route-source <direct-google|openrouter> \
  --protocol-provider <provider-slug> \
  --format json
```

For direct Google, use the route explicitly and omit the OpenRouter provider
pin:

```bash
/home/brasides/code/ploke/target/debug/ploke-eval loop prototype1-setup \
  --campaign <campaign> \
  --profile <profile-path> \
  --model-id <model-id> \
  --route-source direct-google \
  --provider google \
  --protocol-model-id <model-id> \
  --protocol-route-source direct-google \
  --protocol-provider google \
  --format json
```

After setup and before any step, inspect the admitted campaign or closure state.
If the actual `route_source` does not match the intended route, abandon that
campaign/worktree as setup-invalid and start another fresh one.

Expected setup effects:

- campaign manifest under `~/.ploke-eval/campaigns/<campaign>/campaign.json`
- closure state under `~/.ploke-eval/campaigns/<campaign>/closure-state.json`
- admitted profile under `~/.ploke-eval/campaigns/<campaign>/prototype1/run-profile.toml`
- scheduler under `~/.ploke-eval/campaigns/<campaign>/prototype1/scheduler.json`
- parent identity at `<worktree>/.ploke/prototype1/parent_identity.json`
- fresh branch `prototype1-parent-<campaign>-gen0`
- one parent identity commit whose only payload is the parent identity file

If setup fails because the gen0 branch already exists and you created that
branch in the current attempt, recover by moving the worktree to a neutral seed
branch, deleting only the mistakenly created empty parent branch, and removing
only the partial campaign directory from that failed attempt. Do not delete or
rewrite older campaign artifacts without explicit user approval.

## Fresh Run After Abandonment

When a prior run is abandoned because required protocol/eval/oracle/History
evidence is invalid or not admissible:

- record the old campaign id, worktree, and stop reason in the operator report
  or active notes;
- do not run another `prototype1-step` against that worktree;
- do not patch readers or aggregate code just to make the old run pass;
- choose a new campaign id and neutral seed branch;
- reuse the intended profile/model/provider policy only after re-reading the
  current files;
- run setup and doctor from the clean worktree, then take at most one bounded
  step.

The check we want from the next run is that the transition succeeds the first
time under the current source and admitted profile. A stale invalid artifact
from the abandoned run is not evidence of success.

## Verify Setup

Build the binary in the new worktree so doctor suggestions are runnable from
that checkout:

```bash
cargo build -p ploke-eval
```

Then run doctor from inside the worktree:

```bash
./target/debug/ploke-eval loop prototype1-doctor --repo-root . --format json
```

Verify:

- phase is the expected next phase, usually `baseline_eval`
- blockers are empty
- parent identity campaign, node, branch, and generation match setup output
- admitted profile commitment exists
- model/provider in `campaign.json` match the intended run
- worktree `git status --short --branch` is clean except the parent identity
  commit already recorded in Git history

## Take One Bounded Step

For a single advance:

```bash
./target/debug/ploke-eval loop prototype1-step --repo-root . --format json
```

While it runs:

- keep the exec session open; do not start another loop command for the same
  campaign
- short status updates should distinguish "still running" from actual evidence
- if artifact directories appear, read them without mutating the run

After it exits, run doctor again:

```bash
./target/debug/ploke-eval loop prototype1-doctor --repo-root . --format json
```

Interpret the phase transition literally. For example, `baseline_eval` to
`baseline_protocol` means eval completed and protocol has not run yet.

If the step reveals a non-blocking defect, file or update an alive bug and keep
the next command explicit. If it reveals a blocker, stop the run and switch to
`ploke-blocker-repair-loop` to decide whether the current run can resume or
must be abandoned and replaced with a fresh worktree/campaign.

## Immediate Post-Step Checks

Before giving a verdict, check:

```bash
cat ~/.ploke-eval/campaigns/<campaign>/closure-state.json
find ~/.ploke-eval/instances/prototype1/<campaign>/<instance>/runs -maxdepth 2 -type f
cat <run-root>/execution-log.json
cat <run-root>/benchmark-patch-projection.json
cat <run-root>/multi-swe-bench-submission.jsonl
git -C <target-checkout> status --short --branch
git -C <target-checkout> diff --check
```

If there is a patch, run the narrow verification the model claimed when it is
cheap and safe, and add a formatting check when patch hygiene matters:

```bash
cargo test -p <target-package>
cargo fmt -- --check
```

These checks are not a substitute for protocol or oracle evidence. Report MBE
only from the admitted profile and actual artifacts such as `final_report.json`,
not from assumptions.

## Handoff To Run Review

Once a run root exists, use `ploke-run-review` for the detailed answer. Pass it:

- campaign id
- instance id
- run root
- target checkout
- whether protocol has run
- any direct verification commands already run

If that review identifies a blocker rather than an ordinary follow-up, use
`ploke-blocker-repair-loop` for the repair pass.

Keep the final setup answer compact:

- campaign/worktree/model/provider
- command that advanced
- current doctor phase
- key artifacts
- whether patch/tests/protocol/MBE exist
- next exact command
