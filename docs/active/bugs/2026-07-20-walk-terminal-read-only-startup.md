# Walk Server Rejects Read-Only Inspection After Runtime Completion

Status: fixed and live verified on preserved V29 evidence; walk protocol v11.

## Broken Contract

A completed Prototype 1 runtime must remain inspectable through the shared walk
service while all mutating requests remain rejected. The strict rule that a
terminal successor lifecycle cannot reopen controller-transfer authority must
not prevent the read-only service from starting.

## Evidence

- Campaign:
  `p1-v29-strictpatchhandoff-mbe-g35f-direct-3g1x3-p3-obs2400-ptok8k-20260719-001018`.
- Parent checkout:
  `/home/brasides/.ploke-eval/setup-seeds/p1-v29-strictpatchhandoff-mbe-g35f-direct-3g1x3-p3-obs2400-ptok8k-20260719-001018`.
- Current parent/runtime:
  `node-25e372fee4efe3eb`,
  `8bb89724-76d8-4b54-b273-2930a967ec38`, generation 2.
- The final transition-journal lifecycle for that runtime is `Completed`.
- `walk summary --repo-root <V29-root>` succeeds without a server and reports
  the configured `max_total_nodes` terminal state.
- `walk status --repo-root <V29-root>` reports the endpoint offline without
  trying to start it.
- `walk show --repo-root <V29-root> --with-version` briefly connects to an
  auto-spawned socket, receives `Connection reset by peer`, and then reports a
  startup timeout.
- Foreground `walk serve` with a private temporary socket directory exposes the
  suppressed startup error:
  `batch selection is invalid: successor transfer cannot open after runtime lifecycle Completed`.
- `show`, `config`, trace inspection, session history, audit, database queries,
  read-only LLM inspection, and replay navigation all share the auto-spawn path.
  They therefore fail as a class after this completed run.
- The V29 transition journal had SHA-256
  `5c4fe3f273adb96f7c172689a8860404c905aeae23108bbc0dca59748a50d8d6`
  before repair validation. Its three controller journals had SHA-256
  `4964a614f6975aeb26eea39d1b958283b2a47693b0e58dd4b719e3334a8eb869`,
  `1f0169b10bddd2586f4122673a717b22967dfecc42a8fc3b96c3ad79ccef327b`,
  and
  `b5298db29a8a74537ed0912cc96570a1cd93baebbcb777915d1aba8576ce66fb`.

## Source Trace

`walk show`
-> client `ensure_server`
-> detached `walk serve`
-> `server::serve`
-> `control::walk_server_admission`
-> strict `successor_transfer_release`
-> `SuccessorLifecycle::Completed`
-> `InvalidBatchSelection`
-> detached server exits after binding but before publishing a healthy endpoint
-> client sees a reset and then a startup timeout.

Commit `aa0af840a` introduced both durable server admission and the correct
terminal-lifecycle rejection. The regression is that generic server admission
uses the controller-transfer result as a service-existence decision.

## Docs and Operator Expectation

The walk command help directs operators to summary and replay inspection after a
run completes. Replay, trace, audit, database, and LLM inspection are also
intended to be sibling CLI/UI views over the same persisted source of truth.
Completion should close mutation authority, not erase that inspection surface.

The run's digest, lifecycle, controller, and handoff checks remain mandatory.
The repair must not reinterpret `Completed` as transferable authority.

## Repro Coverage

Added with the repair:

- exact V29 transition-journal replay through the production successor
  lifecycle classifier;
- a server regression proving terminal read-only authority admits inspection;
- a regression proving the same endpoint rejects every transfer-required
  request with a terminal blocker.

The checked-in fixture preserves the exact V29 parent identity bytes and the
four exact lifecycle journal records needed to classify runtime
`8bb89724-76d8-4b54-b273-2930a967ec38` as `Completed`.

## Implemented Repair

- Add a typed terminal read-only `ServerAdmission`.
- Have walk-only admission classify a validated terminal lifecycle before
  calling the unchanged strict controller-transfer helper.
- Carry the closed gate's typed blocker into status and mutation errors.
- Keep inspection and local `Stop` enabled; keep all controller mutations
  disabled.
- Bump the walk protocol because the serialized authority/blocker vocabulary
  changes.

Do not:

- change `successor_transfer_release`;
- reopen a completed controller session;
- relax lifecycle, digest, source-epoch, or handoff validation;
- modify V29 run history during validation.

## Live Validation

The rebuilt binary auto-spawned a protocol-v11 read-only server for the
preserved V29 checkout. The following command families succeeded against that
same endpoint:

- `show`, `status`, `files`, `config`, and `session-history`;
- `trace list` and exact `trace show`;
- `audit --transition r13a-to-r14a`;
- immutable `db_query`;
- `llm sessions` and exact `llm observe`;
- `summary`, `replay`, `back`, and `forward`.

Status reported `authority=read_only`, blocker `run_completed`, and
`mutation_authority=read_only`. `reset` was rejected with the typed
`run_completed` error, while `stop` shut down the idle inspection server.
Regression coverage applies that same rejection to Start, Step, Reset, recovery
mutation, BranchLive, and live LLM requests.

`recover` remained inspection-only and correctly reported `EpochChanged`
between the persisted protocol-v10 session epoch and the rebuilt protocol-v11
binary. No epoch admission was attempted.

After validation, the transition journal and all three controller journals had
the same SHA-256 values recorded above. No walk server process remained.

Two non-correctness sharp edges remain visible: cold terminal reconstruction
took roughly 15–30 seconds before the endpoint answered, and `files` includes
up to a 128 KiB prefix of the transition journal in its response.
