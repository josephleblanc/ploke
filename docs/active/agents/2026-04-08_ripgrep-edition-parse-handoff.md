# 2026-04-08 Ripgrep Edition Parse Handoff

- Date: 2026-04-08
- Task title: Historical ripgrep setup failure root cause isolation
- Focus: confirm the underlying parser failure behind the ripgrep setup repro, create a targeted `syn_parser` repro, and determine whether edition-aware parsing is sufficient or whether a larger parser front-end change is needed.

## Current Status

The historical ripgrep setup failure is now reproducible, fail-fast, and diagnostics-rich.

We previously fixed setup containment and replay visibility:

- setup parse failures now surface as `PrepareError::IndexingFailed`
- `ploke-tui` stays alive on setup parse failure and emits a warning containment path
- the eval harness now persists `parse-failure.json` with flattened nested parser diagnostics

The historical repro command remains:

- `cargo test -p ploke-eval test_historical_ripgrep_setup_failure_reports_indexing_failed_and_status_artifact -- --ignored --nocapture`

That replay now reports the concrete failing source path:

- `/home/brasides/.ploke-eval/repos/BurntSushi/ripgrep/crates/cli/src/process.rs`

## Most Important Finding

This does **not** currently look like a `KL-002` proc-macro pre-expansion parse failure.

Instead, the strongest current hypothesis is:

- `grep-cli` is effectively a Rust 2015 crate
- `process.rs` uses `async` as an identifier
- our parser pipeline is not carrying crate edition into parsing
- `syn` is being asked to parse the file without edition-aware handling
- that likely causes a modern-keyword parse failure on valid 2015 source

## Concrete Evidence

### 1. Exact failing source location

The failing file identified by the replay artifact is:

- `/home/brasides/.ploke-eval/repos/BurntSushi/ripgrep/crates/cli/src/process.rs`

The likely offending line is:

- [`process.rs:234`](/home/brasides/.ploke-eval/repos/BurntSushi/ripgrep/crates/cli/src/process.rs#L234)

It contains:

- `fn async(mut stderr: process::ChildStderr) -> StderrReader`

That is valid in Rust 2015 and invalid as a bare identifier in later editions.

### 2. No obvious proc-macro / pre-expansion syntax

Manual inspection of `process.rs` did **not** reveal:

- proc-macro placeholder syntax
- `#[duplicate_item(...)]` style raw pre-expansion syntax
- other obvious `KL-002` signatures

This makes the proc-macro hypothesis weak for this case.

### 3. Cargo resolves `grep-cli` as edition 2015

Command run in the historical ripgrep checkout:

- `cargo metadata --no-deps --format-version 1`

Result:

- package `grep-cli` reports `edition: "2015"`
- target `grep_cli` also reports `edition: "2015"`

The relevant manifest:

- [`crates/cli/Cargo.toml`](/home/brasides/.ploke-eval/repos/BurntSushi/ripgrep/crates/cli/Cargo.toml)

does not specify an edition explicitly, so Cargo defaults it to 2015.

### 4. Cargo itself accepts the crate

Command run in the historical ripgrep checkout:

- `cargo check -p grep-cli`

Result:

- succeeds
- warns that no edition is set and defaults to 2015

This is a key discriminator: the crate is build-valid under Cargo.

## Current Root-Cause Hypothesis

The parser pipeline appears to be dropping edition information before source parsing.

That means a file using edition-sensitive identifiers can fail under `syn` even though Cargo accepts it.

This is likely an **edition-awareness bug** in our parser pipeline, not a scheduler bug and not obviously a proc-macro bug.

## Relevant Code Paths

### Discovery currently does not preserve edition in `CrateContext`

- [`discovery/mod.rs`](/home/brasides/code/ploke/crates/ingest/syn_parser/src/discovery/mod.rs)
- [`single_crate.rs`](/home/brasides/code/ploke/crates/ingest/syn_parser/src/discovery/single_crate.rs)

Key detail:

- `CrateContext` currently stores name/version/root/files/targets/deps/etc.
- it does **not** currently store effective crate edition

### Parsing still uses plain `syn::parse_file`

- [`visitor/mod.rs`](/home/brasides/code/ploke/crates/ingest/syn_parser/src/parser/visitor/mod.rs#L127)

Current parse entry:

- reads file contents
- calls `syn::parse_file(&file_content)?`

No crate-edition information is supplied here.

## What We Know and What We Do Not Know

### Known

- the historical ripgrep setup failure resolves to `crates/cli/src/process.rs`
- `grep-cli` is effectively edition 2015
- the file uses `async` as an identifier
- Cargo accepts the crate
- our parser currently does not preserve edition in `CrateContext`

### Not yet confirmed

- the exact `syn` error string emitted for this file in-process
- whether `syn` has a clean edition-aware parse path we can use directly
- whether fixing this requires only parse entry changes or broader downstream handling

## Immediate Next Step

Create a small focused repro in `syn_parser` for a Rust 2015 crate that uses `async` as an identifier.

That repro should prove or falsify the edition-mismatch hypothesis without depending on the full ripgrep workspace.

## Recommended Next Implementation Slice

### 1. Add a failing repro test

Add a targeted `syn_parser` repro fixture/test that:

- creates or uses a tiny crate with effective edition 2015
- omits `edition` in `Cargo.toml` or explicitly uses `2015`
- contains a function or method named `async`
- asserts the current parser behavior fails in the same class of way

Preferred location:

- `crates/ingest/syn_parser/tests/repro/fail/`

Suggested file:

- `edition_2015_async_identifier.rs`

### 2. Thread effective crate edition through discovery

Extend `CrateContext` to record effective edition from Cargo manifest resolution.

Important rule:

- missing `edition` must resolve the same way Cargo does
- do **not** infer member crate edition from workspace root edition

### 3. Investigate `syn` capabilities

Determine whether `syn` exposes an edition-aware parse mode that can cleanly accept 2015-era identifiers like `async`.

If yes:

- use that path

If no:

- evaluate whether a preprocessing/token-rewrite layer is acceptable
- or whether a different parser front-end is needed before `syn`

## Cautions

- Do not classify this as `KL-002` unless new evidence appears.
- Do not weaken parser correctness by silently skipping edition-mismatched files.
- Do not assume workspace root edition applies to members.
- Keep the setup-containment warning path as temporary only; it is not the final fix.

## Useful Commands

- Historical replay:
  - `cargo test -p ploke-eval test_historical_ripgrep_setup_failure_reports_indexing_failed_and_status_artifact -- --ignored --nocapture`
- Historical checkout metadata:
  - `cargo metadata --no-deps --format-version 1`
- Cargo acceptance check:
  - `cargo check -p grep-cli`

## Relevant Files

### Historical target

- [`process.rs`](/home/brasides/.ploke-eval/repos/BurntSushi/ripgrep/crates/cli/src/process.rs)
- [`Cargo.toml`](/home/brasides/.ploke-eval/repos/BurntSushi/ripgrep/crates/cli/Cargo.toml)
- [`Cargo.toml`](/home/brasides/.ploke-eval/repos/BurntSushi/ripgrep/Cargo.toml)

### Parser / discovery

- [`discovery/mod.rs`](/home/brasides/code/ploke/crates/ingest/syn_parser/src/discovery/mod.rs)
- [`single_crate.rs`](/home/brasides/code/ploke/crates/ingest/syn_parser/src/discovery/single_crate.rs)
- [`visitor/mod.rs`](/home/brasides/code/ploke/crates/ingest/syn_parser/src/parser/visitor/mod.rs)

### Harness / diagnostics

- [`parse_errors.rs`](/home/brasides/code/ploke/crates/ploke-tui/src/utils/parse_errors.rs)
- [`core.rs`](/home/brasides/code/ploke/crates/ploke-tui/src/app_state/core.rs)
- [`indexing.rs`](/home/brasides/code/ploke/crates/ploke-tui/src/app_state/handlers/indexing.rs)
- [`runner.rs`](/home/brasides/code/ploke/crates/ploke-eval/src/runner.rs)
- [`replay.rs`](/home/brasides/code/ploke/crates/ploke-eval/src/tests/replay.rs)

## Resume Prompt

Continue from `docs/active/agents/2026-04-08_ripgrep-edition-parse-handoff.md`.
Create a focused `syn_parser` repro for a Rust 2015 crate that uses `async` as an identifier, confirm the current failure mode, then inspect `syn` and our parser entrypoints to determine whether edition-aware parsing can fix it cleanly or whether a broader parser adaptation is required.
