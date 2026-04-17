# Repository Guidelines

## Start Here

**Primary entry point:** [docs/active/CURRENT_FOCUS.md](docs/active/CURRENT_FOCUS.md)

When the user asks:
- "What were we up to?"
- "Remind me of next steps"
- "Let's pick up where we left off"
- or anything similar

Read `CURRENT_FOCUS.md` first.

If `CURRENT_FOCUS.md` points to an active control plane in `docs/active/agents/...`,
that control plane is the working task-entry document for the current eval sprint.

When the active planning doc changes, update `CURRENT_FOCUS.md` immediately in the
same change set and mark the old planning doc superseded with a forward link.

## Naming and Role Encoding
When multiple concrete types play the same functional role across modules, encode
that shared role structurally with traits and associated types rather than
relying on repeated generic names like `Context`, `State`, or `Output`.

Preferred pattern:
- traits define shared roles (`type Subject`, `type State`, `type Output`, etc.)
- concrete module-local types fill those roles
- semantics should live in the module tree and trait structure, not in long
  compound type names
- prefer module-qualified use sites like `review::State` over importing generic
  names bare

Avoid:
- inventing long compound names to carry semantic identity
- relying on repeated generic type names alone to imply structural similarity

## Eval Work Cold-Start Sequence

For evaluation-driven development, benchmarking, replay, or research work, start
in this order:

1. [docs/active/CURRENT_FOCUS.md](docs/active/CURRENT_FOCUS.md)
2. [docs/active/workflow/README.md](docs/active/workflow/README.md)
3. [docs/active/workflow/readiness-status.md](docs/active/workflow/readiness-status.md)
4. [docs/active/workflow/handoffs/recent-activity.md](docs/active/workflow/handoffs/recent-activity.md)
5. [docs/active/plans/evals/phased-exec-plan.md](docs/active/plans/evals/phased-exec-plan.md)
6. the active control plane or active planning doc linked from `CURRENT_FOCUS.md`
7. the specific packet, handoff, or living artifact the task depends on

Precedence rule:
- `docs/active/workflow/*` is the operational source of truth
- `CURRENT_FOCUS.md` is the recovery pointer
- the active control plane is the execution surface for the current sprint
- older handoffs or tracking docs lose if they conflict with the three items above unless explicitly noted

## Eval Workflow And Research Operations

### Required harness reference

Before doing anything substantial with evals, read this file in full:
- `crates/ploke-tui/src/app/commands/unit_tests/harness.rs`

This is the canonical reference for the type-state builder pattern, relay
system, event channel architecture, and test-runtime access to `AppState`.

### Key live artifacts

- [Hypothesis Registry](docs/active/workflow/hypothesis-registry.md)
- [Evidence Ledger](docs/active/workflow/evidence-ledger.md)
- [Priority Queue](docs/active/workflow/priority-queue.md)
- [Failure Taxonomy](docs/active/workflow/failure-taxonomy.md)
- [Recent Activity](docs/active/workflow/handoffs/recent-activity.md)
- [Programme Charter](docs/active/workflow/programme_charter.md)

### Key durable templates and references

- [EDR template](docs/workflow/edr/EDR_TEMPLATE.md)
- [Run manifest draft](docs/workflow/run-manifest.v0.draft.json)
- [Workflow skills](docs/workflow/skills/)
- [Eval design](docs/active/plans/evals/eval-design.md)
- [2026-04-09 Eval Workflow And Research Operations Plan](docs/active/agents/2026-04-09_eval-research-program/2026-04-09_eval-workflow-and-research-operations-plan.md)

### Operating rules for eval work

- Every substantial eval task gets a handoff doc, packet, or append to an existing active artifact.
- Every task must state which workstream and gate it belongs to (`A1`-`A5`, `H0`).
- Every test, replay, or inspection pass should state what proposition it proves.
- Setup-only, replay, and live-model execution paths must stay conceptually separate.
- Do not silently weaken correctness to make evals look better.
- Lower-layer measurement and replay gaps take priority over higher-layer tool optimization when interpretation is blocked.
- When running `ploke-eval` repeatedly during active eval/protocol work, prefer the built binary at `./target/debug/ploke-eval` over `cargo run -p ploke-eval` once the crate is already built.
- Use `cargo run -p ploke-eval` only when you actually need a rebuild or when the binary is not yet present.

## Orchestration And Reporting Rules

When working under an active eval control plane:

- use [2026-04-12_eval-orchestration-protocol.md](docs/active/agents/2026-04-12_eval-orchestration-protocol/2026-04-12_eval-orchestration-protocol.md) as the execution contract unless a newer control-plane protocol explicitly supersedes it
- Workers do not self-certify work as "verified", "done", or "accepted".
- Workers report `claims`, `evidence`, `unsupported_claims`, `not_checked`, and `risks`.
- Each claim should map to a numbered acceptance criterion in the task packet.
- Verifier passes should stay bounded to the cited evidence and packet scope.
- Only the orchestrator accepts, rejects, re-scopes, or supersedes a packet.

If the active control plane and a local assumption disagree, stop and reconcile
the docs first.

## Rust Version

We are using Rust 2024 in all crates.

## Shared Agent Documents

- When the user asks you to create a new document, use `docs/active/agents/` unless directed otherwise.
- Shared agent documents live in `docs/active/agents/`.
- See [docs/active/agents/readme.md](docs/active/agents/readme.md) for naming conventions and directory guidance.
- For the active `ploke-protocol` architecture thread, check [docs/active/agents/2026-04-15_ploke-protocol-control-note.md](docs/active/agents/2026-04-15_ploke-protocol-control-note.md) to resolve the current authoritative checkpoint, fork lineage, and next intended slice before relying on chat history alone.

## End-Of-Session Doc Cleanup

- When a session is ending or a fresh restart is being prepared, bring up the `doc-cleanup` sidecar check.
- Use it to sweep stale `docs/active/agents/` docs into archive and keep the active set restart-critical.
- Treat it as a closeout/restart hygiene pass, not an interruption during substantive work.

## Reading Logs

When the user asks you to check or read logs:

- use `jq` for `.json` logs
- make one initial query to inspect the shape if the structure is unfamiliar
- in follow-up queries, limit output and focus on the highest-signal fields
- do not directly inspect `.sqlite` logs or backup databases
- use `rg` for non-JSON logs

## Correctness Guardrails

- Do not relax internal correctness, consistency, validation, schema, or import semantics without explicit user approval first.
- If a possible fix would make the system more permissive, tolerate previously invalid states, silently skip expected data, or weaken invariants, stop and ask before implementing it.
- When presenting such a proposal, describe the tradeoff plainly: what invariant would be weakened, what failures would stop surfacing, and what safer alternatives exist.

## Production Code Changes For Eval Work

- **Do not modify production code outside `crates/ploke-eval/` without explicit user permission.**
- Before changing `syn_parser`, `ploke-tui`, `ploke-db`, `ploke-llm`, or other core crates:
  1. stop
  2. ask the user
  3. wait for explicit permission before proceeding

Rationale:
- prevents unintended side effects on core infrastructure during eval work

## Backup Fixtures

- Treat backup fixture databases under `tests/backup_dbs/` as schema-coupled fixtures, not long-term compatibility targets by default.
- When schema changes add, remove, or rename stored relations, prefer regenerating backup fixtures or adding an explicit migration path rather than loosening import behavior.
- Do not make backup import paths silently tolerate missing relations, extra relations, or schema drift unless the user explicitly approves that change.
- If tests fail because a backup fixture predates the current schema, first propose regenerating the fixture backups and only propose permissive loading or migration tooling as explicit alternatives.
- Before changing backup fixtures or tests that depend on them, check [docs/testing/BACKUP_DB_FIXTURES.md](docs/testing/BACKUP_DB_FIXTURES.md) for the registry, fixture consumers, and regeneration instructions.
- If the fixture review date in `docs/testing/BACKUP_DB_FIXTURES.md` is more than 7 days old, remind the user and ask whether they want to start a fixture review before making more backup-fixture changes.

## Test Execution

- When running tests, use a sub-agent to execute the test command and report the output back to the main agent.
- Use follow-up sub-agent test runs for retries or narrowed repros when needed so the main thread keeps only the summarized result and next action.

## Experiment Decision Records

- For planned eval A/B tests, ablations, or materially diagnostic workflow changes, create or update an EDR in [docs/active/workflow/edr](docs/active/workflow/edr) using [docs/workflow/edr/EDR_TEMPLATE.md](docs/workflow/edr/EDR_TEMPLATE.md) before implementation when feasible.
- After the run, update the same EDR with linked manifests, outcome, and decision.
- Do not leave experimental changes undocumented.

## Fail-Until-Impl Strict Test Rules

- Do not use tautological assertions such as `is_ok() || is_err()` or match arms that accept both outcomes without further checks.
- For behavior tests that require real output, do not add `Err` branches that pass on placeholder or "not yet implemented" messages.
- Assert success with `expect` or `unwrap` on `Ok` plus real invariants, or use intentional negative tests with `assert!(result.is_err())` plus concrete error expectations.
- Prefer exercising production entrypoints such as `Command::execute` and executor paths rather than failing only inside the test with `todo!()`.
- Until implementation exists, failure may be a panic from `todo!()` in the code under test or an `expect` on `Ok` that is not yet satisfied; do not paper over that with stub-tolerant matches.
After cold restarts, remind the user that some `ploke-protocol` and `ploke-eval` crate docs/manifests may still be hidden from normal `git status` by local git metadata (`skip-worktree` / local exclude rules), and offer to show or restore that state if they want.
