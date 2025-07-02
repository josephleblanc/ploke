# ADR-018: File-Level Hash Verification for I/O Integrity

## Status
PROPOSED

## Context
The `ploke-io` actor is responsible for reading specific byte ranges (snippets) from source files. The database provides these byte ranges based on a previous parsing phase. A critical problem arises if a file is modified on disk after being parsed but before the snippet is read: the stored byte ranges may become invalid, pointing to incorrect or garbled code. This is the "Stale Span Problem".

An initial thought was to verify each snippet individually by re-calculating its `TrackingHash`. However, this is inefficient and complex. A more subtle issue is that a formatting-only change (like adding a newline between functions) can invalidate byte spans for all subsequent items in a file, even though the individual `TrackingHash` of each item would remain unchanged, making item-level verification insufficient to detect this class of error.

## Decision
We will implement a file-level verification strategy to ensure I/O integrity. The core of this decision is to use the `TrackingHash` that `syn_parser` already generates for the entire file's `TokenStream` (stored on the file-level `ModuleNode`).

1.  **Data Flow:** When the database is queried for items to be embedded, the query will be modified to also retrieve the `TrackingHash` of the file-level `ModuleNode` that contains the item.
2.  **`ploke-io` Verification:** The `ploke-io` actor will receive this file-level `TrackingHash` with each batch of requests for a given file. Before reading any snippets, it will perform a single verification step:
    a. Read the entire file content from disk.
    b. Parse the content into a `proc_macro2::TokenStream`.
    c. Re-calculate the `TrackingHash` using the exact same logic as the parser (`ploke_core::TrackingHash::generate`).
    d. Compare the newly calculated hash with the expected hash from the request.
3.  **Outcome:** If the hashes match, all byte spans for that file are considered valid, and all snippets are read and returned. If the hashes do not match, the file is considered stale, and all requests for that file are failed with a `ContentMismatch` error.

## Consequences
- **Positive:**
  - **Correctness:** Reliably solves the "Stale Span Problem" by detecting both semantic and formatting changes that would invalidate byte offsets.
  - **Efficiency:** Verification is performed only once per file, not per snippet, which is significantly more performant than re-parsing every individual code snippet.
  - **Architectural Cohesion:** Leverages an existing mechanism (`TrackingHash` on `ModuleNode`) for a new purpose, strengthening the architectural design rather than adding a new, separate hashing mechanism.

- **Negative:**
  - **Increased I/O:** The verification step requires reading the entire file content in `ploke-io`, even if only a small snippet is ultimately needed. However, this is a necessary trade-off for correctness.
  - **Added Dependency:** `ploke-io` will now require dependencies on `syn` and `proc-macro2` to perform the parsing for verification.

- **Neutral:**
  - This decision clarifies and solidifies the role of `ploke-io` as a *verified* I/O provider, not just a raw byte reader.

## Compliance
[PROPOSED_ARCH_V3.md](/PROPOSED_ARCH_V3.md) Items: Supports the integrity of the data pipeline between the Database and the Embedding components.
[IDIOMATIC_RUST.md](ai_workflow/AI_Always_Instructions/IDIOMATIC_RUST.md) Sections: C-VALIDATE (ensures data is in the expected state before processing).
[CONVENTIONS.md](ai_workflow/AI_Always_Instructions/CONVENTIONS.md) Items: N/A
