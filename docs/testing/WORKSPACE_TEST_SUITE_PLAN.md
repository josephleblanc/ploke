# Workspace Test Suite Plan

Last updated: 2026-06-02

## Current Policy

`cargo test --workspace` is allowed to exercise live API-sensitive paths when
they are part of the default feature graph. This is intentional.

OpenRouter is currently the fast embedding service used by large parts of the
application. Default workspace coverage that requires `OPENROUTER_API_KEY`
protects the embedding/indexing/retrieval path from accidental provider,
payload, schema, and capability regressions. Do not move these tests out of the
default lane, add `#[ignore]`, or weaken assertions just to make the suite
offline-only.

Google live tests are a different lane. They require ADC, route env, and model
configuration, and they should be run through the documented preflight:

```sh
cargo xtask auth google --strict-live
```

The desired end state is not "no live calls in the default suite." The desired
end state is a default workspace suite that is meaningful, explainable, fast
enough to run routinely, and quiet enough that real failures stand out.

## Current Evidence

On this branch, `cargo test --workspace` passed on 2026-06-02 in the configured
workspace with live credentials available.

Observed cleanup targets from that run:

- Warning noise remains, especially in `ploke-eval`, `ploke-egui`,
  `syn_parser` test imports, and `ploke-db` observability placeholders.
- Some passing tests print noisy stdout/stderr, including CLI help text,
  `git init` / commit output from temp repos, and intentional tracing logs.
- Several default tests are slow enough to make routine runs painful:
  `ploke-rag` around 181s, `ploke-db` lib around 124s, `ploke-tui` lib around
  107s, and `ploke-transform` around 97s in the observed run.
- There are 143 source `#[ignore]` markers. They are not one category.
- Existing `#[allow(...)]` and other allow-list style suppressions need a
  deliberate audit, not indefinite accumulation.

## Guardrails

Performance and cleanup work must not loosen tests.

- Preserve existing assertions unless replacing them with stricter or more
  direct assertions over the same behavior.
- When optimizing a slow test, first identify what behavior it proves. If a
  fixture, snapshot, cache, or smaller focused input can prove the same behavior,
  use that. Do not silently narrow the behavioral claim.
- Compare sensitive work against `origin/main` before editing shared parser,
  transform, DB, embedding, or eval surfaces:

```sh
git merge-base HEAD origin/main
git diff --stat origin/main...HEAD
git diff origin/main...HEAD -- <path>
```

- For performance claims, record before/after timings or allocation evidence.
  A passing test suite alone is not evidence of a performance improvement.
- Prefer focused measurements before broad refactors. Useful measurements
  include fine-grained timing spans, allocation counters, criterion benches,
  sync-vs-parallel baselines, and artifact output under `target/test-output/`.
- Keep live OpenRouter coverage meaningful. A "skip" is not a green live-path
  validation when the test is supposed to exercise provider behavior.

## Work Plan

### 1. Documented Live Coverage Policy

Status: started.

- Keep `OPENROUTER_API_KEY` as an expected prerequisite for default
  API-sensitive embedding coverage.
- Keep Google live tests explicit, preflighted, and distinguishable from the
  OpenRouter embedding path.
- Update docs whenever a live test changes lanes so future agents do not
  interpret default live coverage as accidental.

### 2. Warning Cleanup

Status: pending.

Goal: reduce warning noise without papering over useful diagnostics.

Tasks:

- Inventory warnings from `cargo test --workspace` and group them by crate.
- Fix clear unused imports, unused variables, dead code, and unreachable-code
  warnings where the code is genuinely stale.
- For intentional scaffolding, either remove it, make it test-only, or attach a
  narrow `#[allow(...)]` with a reason.
- Treat `todo!()` placeholders in ignored tests as backlog items, not warnings
  to hide casually.

Acceptance:

- A workspace run emits substantially fewer warnings.
- Any remaining warnings have an owner-facing reason.
- No assertions are loosened.

### 3. Slow Default Tests

Status: pending.

Goal: reduce default wall-clock time while preserving behavioral claims.

Candidate areas from the latest run:

- `ploke-rag` type/context expansion and search tests.
- `ploke-db` helper smoke tests, especially edge resolution over large
  fixtures.
- `ploke-tui` unit tests that build heavy runtime state.
- `ploke-transform` self-transform tests.

Allowed optimization patterns:

- Replace repeated fresh setup with validated fixture snapshots where the
  snapshot is already part of the behavior contract.
- Add small focused fixtures for one behavior instead of parsing/indexing a
  larger crate when the larger crate is not essential.
- Share expensive setup inside a test module only when isolation is preserved.
- Add timing instrumentation to identify the actual setup/query/assertion cost.
- Parallelize independent work only after measuring a sync baseline and proving
  output order/identity is stable.
- Reduce allocation churn with references, interning, or cached normalized
  strings where the ownership model remains clear.

Acceptance:

- Before/after timing evidence is recorded.
- The same behavioral properties remain asserted.
- `cargo test --workspace` still passes.

### 4. Stdout and Log Hygiene

Status: pending.

Goal: make green test output quiet enough that real failures stand out.

Tasks:

- Capture or suppress temp-repo `git` command output unless a failure needs it.
- Avoid printing full CLI help text from passing xtask tests unless the test is
  explicitly a rendering/display test.
- Keep intentional tracing/log tests, but assert captured output where possible
  instead of writing noisy lines to the workspace run.

Acceptance:

- Passing workspace output is materially quieter.
- Failure messages remain actionable.

### 5. Ignored Test Inventory

Status: pending.

Goal: split ignored tests into useful categories before deciding what to fix.

Initial categories:

- Local fixture/setup needed, including ripgrep-style or previous-run fixtures.
- Historical layout/data-structure expectations that should be handled by main
  branch fixture work, not by adding compatibility support here.
- Known parser limitations such as cfg/path/SPP/macro-related coverage.
- Live/provider/operator tests that should remain explicit.
- Long-running tests that may become optimized default coverage or remain
  scheduled/operator coverage.
- Stale or not-useful tests that should be removed or rewritten.

Acceptance:

- Each ignored test has a category and a proposed action.
- Tests relying on obsolete historical layouts are listed separately from tests
  that only need a local fixture.
- No compatibility support is added here solely to satisfy stale historical
  expectations.

### 6. Allow/Suppression Audit

Status: pending.

Goal: keep `allow` usage intentional and narrow.

Tasks:

- Inventory `#[allow(...)]`, crate-level allows, lint suppressions, and any
  test-specific allow-list behavior added during this branch.
- Remove suppressions that are no longer needed after warning cleanup.
- Narrow broad suppressions to the smallest item scope.
- Add concise reasons where a suppression is kept.

Acceptance:

- Every retained suppression has a reason or obvious local scope.
- Warning cleanup does not turn into unreviewed suppression growth.

## Suggested First Slice

Start with a measurement-only pass:

```sh
cargo test --workspace -- --nocapture
```

Record slow suites and warning groups, then pick one low-risk target with clear
assertions, such as noisy temp-repo output or a single slow fixture setup path.
Make one scoped change, verify against `origin/main` where relevant, and record
before/after evidence in the final note or `target/test-output/`.
