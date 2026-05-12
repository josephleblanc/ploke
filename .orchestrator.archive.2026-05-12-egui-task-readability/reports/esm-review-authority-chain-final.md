# Final Review: Edit-Surface Authority Chain

Decision: fix before accept.

Findings:

- High: TUI apply still permits authority-less `Reported`/`Applied` states even
  after `Grant` itself became authority-bearing. `Apply::from_results(...)`
  constructs no-authority apply state, `authority()` returns `Option`, and
  backend has runtime checks for a state that should be unrepresentable.
- Medium: History still stores runtime id in string-facing replay carriers,
  though the live checked path starts from typed `RuntimeId`.
- Medium: tests cover happy paths but do not yet pin authority-less apply as
  unconstructible or checked/admitted runtime mismatch as rejected.

Required correction:

- Make TUI apply authority mandatory in the checked apply path.
- Remove backend runtime checks for missing authority.
- Update tests to use authority-bearing apply construction.
- Add a negative History test for checked/admitted runtime mismatch if not
  already covered.
