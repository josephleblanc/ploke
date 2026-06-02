# Multi-Edit Apply And Result Accounting Lose Same-File Semantics

## Summary

The current edit-application path does not preserve correct semantics when a
single proposal contains multiple edits against the same file. Preview and
apply use different models of the edit set, later writes can be staged against
stale hashes and byte ranges, and result accounting drops per-edit outcomes
once multiple edits collapse to fewer file paths.

This is not just a UI issue. It can make a multi-edit proposal look cleaner in
preview than it is at apply time and can overstate success in downstream eval
artifacts.

## Concrete failure modes

### 1. Same-file semantic edits are previewed together but applied sequentially

`apply_code_edit_tool` groups semantic edits by file and builds an in-memory
preview by applying all same-file edits in descending byte order:

- [crates/ploke-tui/src/rag/tools.rs](/home/brasides/code/ploke/crates/ploke-tui/src/rag/tools.rs:676)
- [crates/ploke-tui/src/rag/tools.rs](/home/brasides/code/ploke/crates/ploke-tui/src/rag/tools.rs:682)

But approval later hands the original `proposal.edits` directly to
`write_snippets_batch()`:

- [crates/ploke-tui/src/rag/editing.rs](/home/brasides/code/ploke/crates/ploke-tui/src/rag/editing.rs:284)

Each `WriteSnippetData` still carries the original `expected_file_hash`,
`start_byte`, and `end_byte` captured before any write landed:

- [crates/ploke-tui/src/rag/tools.rs](/home/brasides/code/ploke/crates/ploke-tui/src/rag/tools.rs:648)

That means later same-file semantic edits can be applied against already-mutated
contents without rebasing ranges or updating the expected file hash.

### 2. Approval result accounting drops per-edit outcomes

Both semantic and non-semantic approval paths currently zip write results
against deduplicated file paths:

- semantic path:
  [crates/ploke-tui/src/rag/editing.rs](/home/brasides/code/ploke/crates/ploke-tui/src/rag/editing.rs:291)
- non-semantic path:
  [crates/ploke-tui/src/rag/editing.rs](/home/brasides/code/ploke/crates/ploke-tui/src/rag/editing.rs:127)

But `file_paths` is derived from `proposal.files`, which is per-file rather than
per-edit:

- proposal file list populated from a set in
  [crates/ploke-tui/src/rag/tools.rs](/home/brasides/code/ploke/crates/ploke-tui/src/rag/tools.rs:737)

If multiple edits target the same file, the result vector and file-path vector
are no longer describing the same cardinality. Extra per-edit outcomes are
silently lost from the JSON payload.

### 3. Non-semantic lower layer still assumes one patch survives

`apply_ns_code_edit_tool` already detects multiple patch edits in one request
and logs that shape:

- [crates/ploke-tui/src/rag/tools.rs](/home/brasides/code/ploke/crates/ploke-tui/src/rag/tools.rs:905)

But the helper still only processes the first patch via `patches.next()`:

- [crates/ploke-tui/src/rag/tools.rs](/home/brasides/code/ploke/crates/ploke-tui/src/rag/tools.rs:908)

So if a broader validation boundary is bypassed or weakened later, the lower
implementation still silently reduces a multi-patch request to one patch.

## Expected vs actual semantics

Expected:

- a proposal with multiple same-file edits should either:
  - be rebased into one coherent write plan before approval, or
  - apply with updated hashes/ranges per landed edit
- approval results should preserve one outcome per requested edit
- non-semantic multi-patch requests should fail clearly or apply all patches

Actual:

- preview can show a coherent combined diff
- approval can execute against stale state
- per-edit failures can disappear once results are collapsed onto file paths
- multi-patch non-semantic requests still have a first-patch-only lower path

## Why this matters for eval patch outcomes

Eval relies on these edit outcomes to explain why a patch did or did not land.
If same-file multi-edit application is stale and result accounting drops extra
outcomes, then:

- proposal-level success can overstate what actually landed
- empty-final-patch runs can be under-explained
- tool-level failure analysis loses the shape of which edit in a batch failed

This is one of the concrete seams that can turn real patching attempts into
misleading partial-success stories.
