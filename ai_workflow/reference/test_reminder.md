Based on our project's [Testing Philosophy Document Name], propose a paranoid integration test for `analyze_file_phase2` using the `simple_crate` fixture.

Key validation points:
1.  Ensure the output `Vec<Result<CodeGraph, _>>` has the correct length.
2.  Verify nodes have `Synthetic` IDs and `TrackingHash` is present.
3.  Confirm a specific known relation (e.g., `Contains`) exists using `assert_relation_exists`.
4.  Use `find_*_node_paranoid` helpers for locating nodes, respecting their ID regeneration logic.
