// Testing in `ploke-tui`
// Use more crate-local tests instead of having a folder with tests outside of src, since it
// forces us to make our data structures public.
// Instead, we can run our integration tests here. Since we are a user-facing application, we are
// more concerned with running tests that ensure our application works and is correct than
// providing public-visibility functions to other applications.
// In short, we are a binary, not a lib.

// Note: UI tests that require full AppState are better as integration tests
// using the AppHarness. See tests/ui_approvals_integration.rs for the main
// deadlock fix validation tests.

pub mod ui_approvals_simple;