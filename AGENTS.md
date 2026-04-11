# Repository Guidelines

## Current Focus (Start Here)

**→ [docs/active/CURRENT_FOCUS.md](docs/active/CURRENT_FOCUS.md)** — What we're working on now, immediate next step, and quick recovery info.

**When the user says:** "What were we up to?", "Remind me of next steps", "Let's pick up where we left off", or similar — **check CURRENT_FOCUS.md first**, then follow the link to the active planning doc for details.

**Update frequency:** Whenever the active planning doc changes, update CURRENT_FOCUS.md immediately to point to it.

---

## Eval Workflow and Research Operations

When working on evaluation-driven development, benchmarking, or research-related tasks:

1. **Current focus**: [docs/active/CURRENT_FOCUS.md](docs/active/CURRENT_FOCUS.md) — what we're doing now
2. **Workflow overview**: [docs/active/workflow/README.md](docs/active/workflow/README.md) — live workflow overview
3. **Readiness status**: [docs/active/workflow/readiness-status.md](docs/active/workflow/readiness-status.md) — current workflow readiness
4. **Recent activity**: [docs/active/workflow/handoffs/recent-activity.md](docs/active/workflow/handoffs/recent-activity.md) — state board
5. **Phase status**: [docs/active/plans/evals/phased-exec-plan.md](docs/active/plans/evals/phased-exec-plan.md) — exit criteria
6. **Design reference**: [docs/active/plans/evals/eval-design.md](docs/active/plans/evals/eval-design.md) — central design and rationale

**⚠️ CRITICAL: Test Harness Reference**
- **Test harness location**: `crates/ploke-tui/src/app/commands/unit_tests/harness.rs`
- **ALWAYS read this file in full** before doing anything with evals — it contains the type-state builder pattern, relay system, event channel architecture, and how the test runtime provides access to AppState
- This is the canonical reference for how the eval harness works and should be consulted first for any eval-related changes

Key living artifacts (updated in real-time):
- [Hypothesis Registry](docs/active/workflow/hypothesis-registry.md) — claims and test status
- [Evidence Ledger](docs/active/workflow/evidence-ledger.md) — beliefs backed by artifacts
- [Priority Queue](docs/active/workflow/priority-queue.md) — what to work on next
- [Failure Taxonomy](docs/active/workflow/failure-taxonomy.md) — classification of failure modes

Key durable templates:
- EDR template: [docs/workflow/edr/EDR_TEMPLATE.md](docs/workflow/edr/EDR_TEMPLATE.md)
- Run manifest draft: [docs/workflow/run-manifest.v0.draft.json](docs/workflow/run-manifest.v0.draft.json)
- Skills: [docs/workflow/skills/](docs/workflow/skills/)

Working norms for eval work:
- Every substantial task gets a handoff doc or is appended to an existing one
- Every task states which workstream and gate it belongs to (A1-A5, H0)
- Every test or replay should say what proposition it proves
- Setup-only, replay, and live-model paths stay conceptually separate
- Do not silently weaken correctness to make evals look better

See also: [2026-04-09 Eval Workflow And Research Operations Plan](docs/active/agents/2026-04-09_eval-research-program/2026-04-09_eval-workflow-and-research-operations-plan.md)

## Rust version
We are using rust version 2024 in all crates.

## Shared Agent Documents
- When the user asks you to create a new document, you should use the `docs/active/agents` directory, unless directed otherwise.
- Shared agent documents are in `docs/active/agents`
- See `docs/active/agents/readme.md` for naming conventions of files and directories, and further details.

## Reading Logs
When the user asks you to "check the logs", "read the logs", "look into the logs", or similar:
- use `jq` for `.json` logs
  - make one initial query to see the shape, if the log structure is unfamiliar
  - in follow-up queries, prefer to limit output lines, and focus on the highest-signal log elements
- do not directly check/read/look into `.sqlite` logs or backup databases
- use `rg` for logs of other file types


## Correctness Guardrails
- Do not relax internal correctness, consistency, validation, schema, or import semantics without explicit user approval first.
- If a possible fix would make the system more permissive, tolerate previously invalid states, silently skip expected data, or weaken invariants, stop and ask before implementing it.
- When presenting such a proposal, describe the tradeoff plainly: what invariant would be weakened, what failures would stop surfacing, and what safer alternatives exist.

## Production Code Changes (Eval Work)
- **Do not modify production code outside `crates/ploke-eval/` without explicit user permission.**
- Before changing `syn_parser`, `ploke-tui`, `ploke-db`, `ploke-llm`, or other core crates:
  1. STOP and ask the user
  2. Wait for explicit permission before proceeding
- Rationale: Prevent unintended side effects on core infrastructure during eval work

## Backup Fixtures
- Treat backup fixture databases under `tests/backup_dbs/` as schema-coupled fixtures, not as long-term compatibility targets by default.
- When schema changes add, remove, or rename stored relations, prefer regenerating backup fixtures or adding an explicit migration path rather than loosening import behavior.
- Do not make backup import paths silently tolerate missing relations, extra relations, or schema drift unless the user explicitly approves that change.
- If tests fail because a backup fixture predates the current schema, first propose regenerating the fixture backups and only propose permissive loading or migration tooling as explicit alternatives.
- Before changing backup fixtures or tests that depend on them, check [docs/testing/BACKUP_DB_FIXTURES.md](docs/testing/BACKUP_DB_FIXTURES.md) for the current registry, fixture consumers, and regeneration instructions.
- If the fixture review date in [docs/testing/BACKUP_DB_FIXTURES.md](docs/testing/BACKUP_DB_FIXTURES.md) is more than 7 days old, remind the user and ask whether they want to start a fixture review now before making more backup-fixture changes.

## Test Execution
- When running tests, use a sub-agent to execute the test command and report the output back to the main agent.
- Use follow-up sub-agent test runs for retries or narrowed repros when needed, so the main thread keeps only the summarized result and next action.

## Experiment Decision Records
- For planned eval A/B tests, ablations, or other materially diagnostic workflow changes, create or update an EDR in [docs/active/workflow/edr](docs/active/workflow/edr) using the template in [docs/workflow/edr/EDR_TEMPLATE.md](docs/workflow/edr/EDR_TEMPLATE.md) before implementation when feasible.
- After the run, update the same EDR with linked manifests, outcome, and decision; do not leave experimental changes undocumented.

### Fail-until-impl (strict tests)
- Do not use tautological assertions (`is_ok() || is_err()`, match arms that accept both outcomes with no further checks).
- For behavior tests that require real output, do not add `Err` branches that pass on placeholder or “not yet implemented” messages; assert success with `expect`/`unwrap` on `Ok` and real invariants, or use intentional negative tests with `assert!(result.is_err())` plus concrete error expectations.
- Prefer exercising production entrypoints (`Command::execute`, executor paths) rather than failing only inside the test with `todo!()`.
- Until implementation exists, failure may be a panic from `todo!()` in the code under test or an `expect` on `Ok` that is not yet satisfied; do not paper over that with stub-tolerant matches.
