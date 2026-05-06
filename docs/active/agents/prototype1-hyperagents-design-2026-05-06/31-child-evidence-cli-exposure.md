# Agent 31: Child Evidence CLI Exposure

Date: 2026-05-06

## Scope

Implemented the read-only operator child evidence CLI surface described by
report 27.

Files changed:

- `crates/ploke-eval/src/cli.rs`
- `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs`
- `crates/ploke-eval/src/cli/prototype1_state/history_preview.rs`
- `docs/active/agents/prototype1-hyperagents-design-2026-05-06/31-child-evidence-cli-exposure.md`

Files intentionally not edited:

- `crates/ploke-eval/src/cli/prototype1_state/evidence.rs`
- `crates/ploke-eval/src/cli/prototype1_state/metrics.rs`
- `crates/ploke-eval/src/cli/prototype1_state/mod.rs`
- `crates/ploke-eval/src/cli/prototype1_state/history.rs`
- `crates/ploke-eval/src/cli/prototype1_state/README.md`

## Command Syntax Implemented

Canonical History projection:

```text
ploke-eval history --campaign <id> child-evidence --format table
ploke-eval history --campaign <id> child-evidence --format json
```

Monitor-local compatibility alias:

```text
ploke-eval loop prototype1-monitor --campaign <id> child-evidence --format table
ploke-eval loop prototype1-monitor --campaign <id> child-evidence --format json
```

The command is read-only. It does not call metrics, selection, scoring, History
admission, Crown, or block write APIs.

## Implementation Notes

- `Prototype1ChildEvidenceCommand` carries only `--format table|json`.
- `HistorySubcommand::ChildEvidence` and
  `Prototype1MonitorSubcommand::ChildEvidence` route through the same helper.
- `history_preview::build_child_evidence` constructs `FsEvidenceStore` and calls
  `FsEvidenceStore::child_evidence()` directly.
- JSON output serializes a minimal wrapper with campaign metadata and the
  existing `ChildEvidenceSet`.
- Table output is a bounded diagnostic projection over the already grouped
  `ChildEvidenceSet`. It prints schema/campaign counts, child rows, evaluation
  rows with compared run paths, unplaced evidence, and diagnostics.
- Table output explicitly labels source treatment strings as not sealed
  authority.

## Source Lines To Inspect

- `crates/ploke-eval/src/cli.rs:477`
  adds the monitor alias enum variant.
- `crates/ploke-eval/src/cli.rs:508`
  adds the canonical History enum variant.
- `crates/ploke-eval/src/cli.rs:553`
  defines `Prototype1ChildEvidenceCommand`.
- `crates/ploke-eval/src/cli.rs:12514`
  adds the History parse test.
- `crates/ploke-eval/src/cli.rs:12575`
  adds the monitor parse test.
- `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:1565`
  routes the monitor alias.
- `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:1597`
  routes the canonical History command.
- `crates/ploke-eval/src/cli/prototype1_state/history_preview.rs:274`
  builds the child evidence projection using `FsEvidenceStore::child_evidence()`.
- `crates/ploke-eval/src/cli/prototype1_state/history_preview.rs:290`
  renders table or JSON output.
- `crates/ploke-eval/src/cli/prototype1_state/history_preview.rs:383`
  starts the bounded table renderer.

## Verification

Commands run:

```text
cargo fmt --all
git diff --check -- crates/ploke-eval/src/cli.rs crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs crates/ploke-eval/src/cli/prototype1_state/history_preview.rs
cargo test -p ploke-eval cli::tests::history_child_evidence_should_parse
cargo test -p ploke-eval cli::tests::prototype1_monitor_child_evidence_should_parse
cargo check -p ploke-eval
```

Results:

- `cargo fmt --all` passed.
- `git diff --check` passed.
- Both focused parse tests were blocked during compilation by an existing
  non-owned error in `crates/ploke-eval/src/cli/prototype1_state/evidence.rs`.
- `cargo check -p ploke-eval` was blocked by the same non-owned error.

Blocking compiler error:

```text
error[E0425]: cannot find function `prepare_documents` in this scope
   --> crates/ploke-eval/src/cli/prototype1_state/evidence.rs:147:25
    |
147 |         let documents = prepare_documents(documents, &mut assembly.diagnostics);
    |                         ^^^^^^^^^^^^^^^^^ not found in this scope
```

I did not patch `evidence.rs` because it was explicitly outside this worker's
ownership.
