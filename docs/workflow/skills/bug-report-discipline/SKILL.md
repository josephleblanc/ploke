---
name: bug-report-discipline
description: Use when creating or enriching a durable bug report, alive bug, blocker note, regression report, or root-cause note so it identifies the broken contract, evidence chain, source boundary, docs expectation, and missing repro coverage instead of stopping at a thin symptom summary.
---

# Bug Report Discipline

Use this skill before writing or updating bug reports under `docs/active/bugs/`
or similar durable issue records. The report should explain what is actually
broken, what evidence proves it, and what still needs to be reproduced.

## Quality Gate

A bug report is not acceptable if it only says that something timed out, failed,
panicked, returned an error, or probably came from a provider/config issue.
Before saving it, make sure it has:

- the broken contract in one sentence;
- exact artifact, command, source, or docs paths that prove the observation;
- a trace chain from event/tool/source to state transition or decision to
  persisted result;
- a distinction between symptom, downstream guard, and upstream cause;
- current repro coverage and the missing minimal repro, if any;
- a fix direction that preserves invariants instead of making invalid state
  easier to admit.

If the evidence is not available yet, mark the report incomplete and say which
artifact join, source trace, docs check, or replay is missing.

## Workflow

1. Anchor the scope: exact run, campaign, worktree, command, artifact root,
   source path, and any existing related bug report.
2. Name the violated contract: what should happen, what happened instead, and
   why that violates an invariant or durable-state expectation.
3. Build an evidence receipt: artifact paths, relevant JSON fields, source
   functions, docs expectations, tests, git history, and live/replay commands
   when they matter.
4. Separate layers that are easy to conflate:
   - symptom versus root cause;
   - mechanical completion versus semantic success;
   - persisted projection versus lifecycle authority;
   - environment failure versus repo bug;
   - downstream admission guard versus upstream classifier/adapter bug;
   - stale prior theory versus currently verified cause.
5. Trace the path explicitly:
   `event/tool/source -> state transition or decision -> artifact/result`.
6. Point at the source boundary that should own the fix. Do not turn a reader
   into a permissive salvage path when the writer, classifier, validator, or
   transition authority is wrong.
7. State repro status:
   - existing regression and what it proves;
   - what it does not prove;
   - smallest missing upstream repro;
   - live-provider, replay, doctor, or step-level surface needed to validate.
8. Update an existing report instead of creating a duplicate. If adding a new
   file in an indexed docs directory, update the local README/index.

## Durable Report Shape

Use this shape unless the local directory has a stricter template:

- `Status`: open, fixed, blocked, incomplete, abandoned, or superseded.
- `Broken Contract`: one sentence naming the expected invariant.
- `Evidence`: concise table or bullets with exact paths and observed fields.
- `Source Trace`: functions/modules and the decision they make.
- `Docs/Policy Expectation`: what local docs, profile, protocol, or operator
  policy said should happen.
- `Current Repro Coverage`: tests/replays already present and what they prove.
- `Missing Repro / Validation`: smallest remaining proof gap.
- `Fix Direction`: authority-bearing layer to change and non-fixes to avoid.
- `Related Bugs`: links to related reports or stale theories now corrected.

## Thin-Report Smells

Stop and enrich the report when you see any of these:

- no exact artifact paths;
- no source boundary;
- no docs or policy expectation;
- a root cause that is only an error message;
- a suspicious success that was not validated against the declared contract;
- a test that proves only a downstream guard while the upstream cause remains
  untested;
- a report that keeps an older theory after new trace evidence disproves it.

## Prototype 1 Example Pattern

For Prototype 1 broad-headless-TUI issues, preserve enough detail to distinguish
adapter-clean from contract-clean:

- applied proposal ids and changed paths;
- turn outcomes and active-turn terminal state;
- declared validation contract and the validations actually run;
- `HeadlessTerminal` classification and whether it proves the declared
  validation contract;
- whether a regression constructs a post-facto terminal state or exercises the
  upstream adapter path that produced it.
