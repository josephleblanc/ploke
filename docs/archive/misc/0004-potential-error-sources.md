# A list of possibly quiet sources of error

## Parsing

### Visibility
It is not clear to me that we are correctly handling visibility. Ideally, we
should be able to say with certainty that a given node (e.g. FunctionNode,
StructNode) is visible within a given span (defined as byte start to byte end).
I have downloaded the repository for `syn`, and the relevant file for
`Visibility` is:
 - ~/clones/syn/src/restriction.rs
 - Contains definition of Visibility
 - Good jumping off point to find more docs/source describing exactly how visibility is handled,
 Questions:
 - What exactly is the `Path` type used in `VisRestricted`?
 - Can we link the `Path` type to a file and/or span?

- See further:
  - VisitorState::convert_visibility

- Tracking:
  - 26-03-2025: Added TODO with description in crates/ingest/syn_parser/src/parser/types.rs
