# Codex Branch And PR Workflow

Date: 2026-06-02
Repo: `josephleblanc/ploke`
Primary base branch: `feature/ploke-loop`

## Branching

- Start from the active base branch unless the user names a different base:
  `git switch feature/ploke-loop && git pull --ff-only`.
- Create task branches with a `codex/` prefix, for example
  `codex/dev-workflow-baseline` or `codex/fix-indexing-timeout`.
- Keep branches scoped to one reviewable unit. Prefer separate commits for:
  setup/docs, lint/test fixes, and product code changes.

## Local Gates

Use `scripts/check_dev_env.sh quick` to confirm the VM has the expected tools.
Before opening a PR, run the same gates as CI:

```bash
cargo fmt --all -- --check
cargo check --workspace
cargo clippy --all-targets -- -D warnings
cargo test --workspace
```

`scripts/check_dev_env.sh verify` runs the same sequence after the environment
checks.

## GitNexus

- Prefer `pnpm dlx gitnexus ...`; `npx gitnexus ...` currently fails in this VM.
- Before code edits, run impact checks for the changed symbol when GitNexus can
  identify it.
- Before committing, run GitNexus change detection when available and include the
  output in the handoff or PR notes.

## Push And PR

- Push task branches to origin with `git push -u origin <branch>`.
- Open PRs against `feature/ploke-loop` unless the user requests another base:
  `gh pr create --base feature/ploke-loop --head <branch>`.
- The fine-grained token should only need repository contents and pull-request
  write access for this flow. Repo-admin endpoints are not required.

## Guardrails

- Do not force-push, reset, or revert user changes unless explicitly requested.
- Do not commit local secrets or VM-specific token files.
- If the working tree contains unrelated user changes, leave them untouched and
  mention them in the handoff.
