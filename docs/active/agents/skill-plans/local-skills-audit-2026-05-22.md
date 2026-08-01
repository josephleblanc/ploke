# Local Codex Skills Audit - 2026-05-22

Scope: repo-local skills under `.codex/skills`, checked against current
`skill-creator` guidance and current local command surfaces. `.codex/` is
gitignored local workflow state; this report is the durable tracked audit.

## Method

- Ran `quick_validate.py` across all local skill folders with `uv run --with pyyaml`.
- Split a read-only orchestrator wave across foundational, runtime, UI, and
  structure/naming skills.
- Verified command-sensitive findings against `target/debug/ploke-eval` and
  `target/debug/xtask`.

## Validation Baseline

Most folders with a `SKILL.md` pass validation. Exceptions:

- `.codex/skills/structural-naming/SKILL.md:3` fails because frontmatter
  contains `Role<State>`, and the validator rejects angle brackets.
- `.codex/skills/task-stack/` and `.codex/skills/typed-persistence-spine/` are
  empty skill directories without `SKILL.md`. They should be removed or restored;
  current memory/history suggests `typed-persistence-spine` was intentionally
  retired, so deletion is the likely cleanup.

## Highest Priority Findings

1. Stale Prototype 1 observe commands.
   `.codex/skills/prototype1-loop-runtime/SKILL.md:379` and
   `.codex/skills/prototype1-loop-runtime/references/observe-commands.md:33,44`
   document `loop prototype1-monitor` and `history preview`. Current
   `LoopSubcommand` has no `Prototype1Monitor` variant
   (`crates/ploke-eval/src/cli.rs:546-568`), and `HistorySubcommand` has no
   `Preview` variant (`crates/ploke-eval/src/cli.rs:794-807`). Local help
   confirmed both commands exit as unrecognized.

2. Skill folder/name mismatch.
   `.codex/skills/structural-carrier-gate/SKILL.md:2` declares
   `name: state-transition-check`. `skill-creator` says the folder should match
   the skill name. Rename the folder to `state-transition-check` or rename the
   skill to `structural-carrier-gate`; the first option matches the current
   session skill registry.

3. Orchestrator skill under-states write behavior.
   `.codex/skills/orchestrator-conveyor/SKILL.md:80-104` treats `status` and
   `check` as routine read-like checks, but `xtask/src/commands/orchestrate.rs:79-86`
   records usage after every orchestrate command and
   `xtask/src/commands/orchestrate/usage.rs:56-65` writes the ledger. Add a
   note that board commands may mutate `.orchestrator/usage.json`, and keep them
   main-thread-only.

## Consistency And Trigger Findings

- `.codex/skills/orchestrator-conveyor/SKILL.md:3` should say it is for the
  main/orchestrator thread only; lines 11-14 already say sub-agents must not run
  board commands, but frontmatter is the trigger surface.
- `.codex/skills/light-thread-orchestrator/SKILL.md:17` still says "current task
  stack"; update to board/packet/lane context so it does not revive the legacy
  `.codex/task-stack.jsonl` workflow.
- `.codex/skills/collaboration-incident-logging/SKILL.md:73-77` encourages
  same-turn edits to `AGENTS.md` or skills during trust-loss incidents. Require
  explicit user scope/approval for broad guardrail edits, or log the proposed
  guardrail as follow-up.
- `.codex/skills/ui-claim-archaeology/SKILL.md:15-26` contains trigger detail
  not fully represented in frontmatter. Fold the Artifact/Runtime/Parent,
  candidate/source-ref/evidence/role, and "presentational but semantic slot"
  triggers into the description.
- `.codex/skills/structural-naming/SKILL.md:3` and
  `.codex/skills/structural-carrier-gate/SKILL.md:3` overlap heavily. Keep
  `structural-naming` generic; make `state-transition-check` explicitly the
  `ploke-eval` pre-edit transition checklist.

## Skill Design Findings

- `.codex/skills/prototype1-loop-runtime/SKILL.md` is 391 lines and its
  `references/eval-home-size-log.md` is a 236-line mutable operational log.
  Move the size log to a tracked docs/runtime area or runtime home, keep only
  the command and recording policy in the skill, and add a clear pointer to
  `references/observe-commands.md` from the Observe Workflow section.
- `.codex/skills/prototype1-loop-runtime/SKILL.md:327-331` says scheduler data is
  not primary health evidence, while lines 370-374 include scheduler state in
  progress inference. Reword scheduler use as secondary projection only.
- `.codex/skills/ploke-egui-benchmarking/SKILL.md:83` has a stale
  `native-benchmark` feature summary. Current
  `crates/ploke-egui/Cargo.toml:48-54` also includes `tracking-allocator`,
  `backtrace`, and `tracing-subscriber`; point to `Cargo.toml` as authoritative.
- `.codex/skills/ploke-debugger-claim-workflow/SKILL.md:27-161` duplicates much
  of `ui-claim-archaeology`. Make archaeology report/index/proof-comment rules
  owned by `ui-claim-archaeology`, and keep the debugger skill focused on the
  typed record -> graph -> borrowed UI witness evidence chain.
- `.codex/skills/ploke-debugger-claim-workflow/SKILL.md:226-247` mixes 8-item,
  9-step, and "six-step" evidence-chain counts. Use one named checklist or
  remove the count from prose.
- `.codex/skills/ploke-debugger-claim-workflow/SKILL.md:89` names
  `docs/active/archaeology/ploke-tree-graph/patch-identity.md`, which does not
  exist. Remove it or mark it as a future illustrative example.
- `.codex/skills/regression-test-tracking/agents/openai.yaml:8` does not mention
  `$regression-test-tracking`; `skill-creator/references/openai_yaml.md:34`
  requires default prompts to explicitly name the skill.

## Suggested Fix Order

1. Fix validator failures: remove angle brackets from `structural-naming`
   frontmatter, then delete or restore the two empty skill directories.
2. Correct stale Prototype 1 command references and add the missing observe
   reference pointer.
3. Rename `structural-carrier-gate` to match `state-transition-check`, or align
   name/frontmatter the other way.
4. Tighten orchestrator and incident-logging frontmatter/body rules before the
   next multi-agent wave.
5. Refactor long or duplicated skill bodies, especially `prototype1-loop-runtime`
   and the debugger/archaeology pair.
