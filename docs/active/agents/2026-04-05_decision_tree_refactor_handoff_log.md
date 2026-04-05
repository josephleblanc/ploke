# Date: 2026-04-05
# Task Title: Decision Tree Contract Refactor
# Task Description: Centralize the command decision-tree contract in one canonical test table, split fast assertions by layer without fragmenting the spec, and wire `/index` so parser intent, forwarding, validation, and resolved effect remain coherent.
# Related Planning Files: docs/active/agents/2026-04-01_ux-polish/2026-04-01_indexing-policy.md

## Entry 1: Main Agent Setup
- Slice: coordination and worker setup
- Files changed: docs/active/agents/2026-04-05_decision_tree_refactor_handoff_log.md
- Contract decisions:
  - `decision_tree.rs` remains the canonical source of truth for input-to-effect expectations.
  - Fast tests should be split by assertion phase, not by independent case tables.
  - Initial implementation focus is `/index`, because the existing `scope` field is currently a placeholder and should either drive behavior or be removed from the contract.
- Known transitional gaps before worker execution:
  - `WorkspaceIntent::Index { scope, target }` drops `scope` in dispatcher resolution.
  - `Command::Load { force }` and `Command::Update { scope }` retain parsed structure that is not yet fully honored downstream.
  - The validation probe does not yet explicitly report resolved effects separate from forwarded intents.
- Tests run: none
- Open risks / follow-ups:
  - Worker changes must preserve a single canonical case table and avoid introducing parallel case definitions.
  - Each worker appends a new entry; existing entries must not be edited or deleted.

## Entry 2: Worker `Wegener` Parser and Forwarding Slice
- Slice: `/index` parser normalization and parser-unit assertions
- Files changed:
  - crates/ploke-tui/src/app/commands/parser.rs
  - crates/ploke-tui/src/app/commands/unit_tests/mod.rs
- Contract decisions:
  - Literal `.` is normalized away for `/index workspace .` and `/index crate .`, so the parser emits `target: None` instead of carrying a meaningless path segment downstream.
  - `/index` remains explicitly represented as `scope + target`.
- Placeholders left in place:
  - State-side `scope` semantics remain downstream.
  - `Load.force` and `Update.scope` were intentionally left untouched.
- Tests run:
  - `cargo fmt --all`
  - `cargo test -p ploke-tui --lib test_index_no_db_workspace_root_current_behavior -- --nocapture`
  - `cargo test -p ploke-tui --lib index_workspace_dot -- --nocapture`
- Open risks / follow-ups:
  - Parser normalization is only useful if canonical tests assert the resulting intent and resolution; otherwise the improvement can still drift.

## Entry 3: Worker `Erdos` Canonical Table Slice
- Slice: canonical decision-table schema and runner split
- Files changed:
  - crates/ploke-tui/src/app/commands/unit_tests/decision_tree.rs
- Contract decisions:
  - The canonical case table now carries phase-specific expectations for parsed command and forwarded intent.
  - Shared helpers assert parse, forwarding, and effect contracts from the same rows.
  - Smoke-path execution now captures parsed-command output and uses the same assertion helpers as fast-path tests.
- Placeholders left in place:
  - Non-`/index` rows still rely heavily on `expected_state_cmd` and legacy TDD placeholders.
  - `/save db` and `/update` remain transitional in the canonical table.
- Tests run:
  - `cargo test -p ploke-tui --lib test_no_db_loaded_all_cases -- --nocapture`
  - `cargo test -p ploke-tui --lib test_no_db_workspace_root_index_indexes_workspace_full_app -- --nocapture`
- Open risks / follow-ups:
  - The new state-side resolved-target signal must be consumed by the canonical table to avoid another orphaned contract field.

## Entry 4: Worker `Ramanujan` State and Harness Slice
- Slice: `/index` intent resolution and fast harness visibility
- Files changed:
  - crates/ploke-tui/src/app_state/commands.rs
  - crates/ploke-tui/src/app_state/dispatcher.rs
  - crates/ploke-tui/src/app/commands/unit_tests/harness.rs
- Contract decisions:
  - `WorkspaceIntent::Index` now resolves to a structured `WorkspaceIntentIndexResolution`.
  - The validation probe records resolved `/index` targets so fast tests can assert state-side resolution without using integration tests.
- Placeholders left in place:
  - `LoadCrate` still routes through the temporary `LoadDb` compatibility path.
  - Error-producing `/index` branches are not yet fully modeled through `UiError`.
- Tests run:
  - `cargo test -p ploke-tui --lib`
- Open risks / follow-ups:
  - The initial resolver implementation was path-oriented and needed canonical-table assertions to prove which `/index` contexts are truly implemented versus still transitional.

## Entry 5: Main Agent Integration
- Slice: integrate worker outputs into one canonical `/index` contract
- Files changed:
  - crates/ploke-tui/src/app/commands/unit_tests/decision_tree.rs
- Contract decisions:
  - The canonical table now asserts resolved `/index` targets for the implemented success cases instead of treating them as generic forwarded intents.
  - Rows that still need richer semantics, especially error and recovery branches, remain explicitly transitional instead of being silently accepted.
- Tests run: pending delegated verification
- Open risks / follow-ups:
  - `/index crate` with no target, `/index workspace` error paths, and the broader `/load` and `/update` families still need the same explicit effect-level treatment.

## Entry 6: Main Agent Harness and Auto-Scope Fix
- Slice: seed fast-path test state correctly and align `/index` auto resolution with loaded membership
- Files changed:
  - crates/ploke-tui/src/app/commands/unit_tests/harness.rs
  - crates/ploke-tui/src/app/commands/unit_tests/decision_tree.rs
  - crates/ploke-tui/src/app_state/commands.rs
- Contract decisions:
  - Fast decision-tree tests now seed `SystemState.pwd` before building the app, so state-side resolution and canonical assertions use the same working directory context.
  - `WorkspaceIntent::Index` auto scope now resolves by loaded membership:
    - if `pwd` is a loaded member, re-index that crate
    - else if multiple members are loaded, re-index the workspace
    - else if one member or standalone is loaded, re-index the focused crate
    - else fall back to `pwd`
- Tests run:
  - `cargo test -p ploke-tui --lib test_no_db_loaded_all_cases -- --nocapture`
  - `cargo test -p ploke-tui --lib test_single_member_all_cases -- --nocapture`
  - `cargo test -p ploke-tui --lib test_full_workspace_all_cases -- --nocapture`
  - `cargo test -p ploke-tui --lib`
- Result:
  - targeted suites passed
  - full `--lib` pass succeeded: `189 passed; 0 failed; 5 ignored`
- Open risks / follow-ups:
  - `/index crate` with no explicit target still needs dedicated decision-tree semantics instead of falling through generic crate scope.
  - `/index workspace` error branches and the `/load`/`/update` families still need full effect-level modeling and recovery assertions.

## Entry 7: Decision Table Flattening
- Slice: canonical decision-table `/index` forwarding contract
- Files changed:
  - crates/ploke-tui/src/app/commands/unit_tests/decision_tree.rs
- Contract decisions:
  - `/index` rows now assert the flattened forward shape directly instead of the temporary `WorkspaceIntent(...)` wrapper vocabulary.
  - Existing resolved-target assertions remain intact for the implemented success rows.
  - Pending `/index` rows continue to be represented as pending effect cases, but now share the same forwarded-contract vocabulary.
- Tests run: not run in this slice
- Open risks / follow-ups:
  - The non-`/index` rows still use the older transitional command taxonomy.
  - This file now assumes the runtime will expose `StateCommand::Index(IndexCmd)` or an equivalent flattened debug shape when the implementation lands.

## Entry 8: Decision Table Drift Cleanup
- Slice: align canonical table with the flattened `/index` runtime and the already-implemented `/load crate` and no-DB `/update` behavior
- Files changed:
  - crates/ploke-tui/src/app/commands/unit_tests/decision_tree.rs
- Contract decisions:
  - `/load crate` rows now assert the observed forwarded `Workspace(LoadDb ...)` shape instead of remaining as pending `TestTodo` placeholders.
  - No-DB `/update` rows are now explicit validation failures with the existing `No crate or workspace is loaded` reason.
  - `/index` rows remain flattened against `StateCommand::Index(IndexCmd)` and continue to carry the resolved-target assertions where already implemented.
- Remaining placeholders:
  - `/index workspace` error branches, `/index path/to/crate` error branches, and other unresolved `/index` effect rows are still intentionally pending.
  - `/load workspace` no-arg behavior is still intentionally pending.
  - `SaveDb` recovery/effect assertions remain transitional.
- Tests run:
  - `cargo test -p ploke-tui --lib decision_tree -- --nocapture`
- Result:
  - Passed: `7 passed; 0 failed; 3 ignored`
- Open risks / follow-ups:
  - The canonical table still mixes implemented effect assertions with explicit pending rows; future slices should keep the same pattern so unresolved branches stay visible instead of being hidden behind generic placeholders.

## Entry 8: Transitional Workspace Forwarding Contract
- Slice: keep `/load` and `/update` transitional while the `/index` flattening lands
- Files changed:
  - crates/ploke-tui/src/app/commands/unit_tests/decision_tree.rs
- Contract decisions:
  - `Workspace(...)` forwards for `/load` and `/update` remain accepted as pending test cases when the canonical table still marks them as `TestTodo`.
  - This preserves fast contract visibility without forcing the transitional rows to be rewritten before the `/index` path is finalized.
- Remaining placeholders:
  - `/load` and `/update` rows still defer full effect-level assertions.
  - `SaveDb` rows remain staged for later resolution visibility.
- Tests run:
  - not rerun in this slice
- Open risks / follow-ups:
  - These transitional acceptances should be removed once the load/update families get dedicated resolved-effect assertions.

## Entry 9: `/index` Validation Contract Cleanup
- Slice: stop treating `/index` as part of the generic validation contract in the canonical runner
- Files changed:
  - crates/ploke-tui/src/app/commands/unit_tests/decision_tree.rs
- Contract decisions:
  - The canonical decision table now treats `/index` success rows as resolution-driven, not validation-driven.
  - Generic validation assertions remain for other commands, but `/index` rows rely on resolved-target assertions and forwarded-command shape instead of an implied `validate()` contract.
- Remaining gaps:
  - `IndexCmd::validate()` is still a trivial no-op in the runtime layer; the test table no longer leans on it, but the implementation should eventually either become meaningful or be removed.
  - `/index` error branches are still only partially represented in the canonical table.
- Tests run:
  - not yet rerun after this cleanup
- Open risks / follow-ups:
  - If the runtime continues to expose validation events for `/index`, they should not be interpreted as semantic validation in the test contract.

## Entry 10: `/index` Resolve Errors and No-Fallback Cleanup
- Slice: remove deceptive `/index` no-op validation and replace silent fallback-to-pwd behavior with explicit resolution failures
- Files changed:
  - crates/ploke-tui/src/app_state/commands.rs
  - crates/ploke-tui/src/app_state/dispatcher.rs
  - crates/ploke-tui/src/app/commands/unit_tests/harness.rs
  - docs/active/agents/2026-04-05_decision_tree_refactor_handoff_log.md
- Contract decisions:
  - `IndexCmd` is no longer part of the generic validation path; `/index` is handled as a resolve-first command.
  - `IndexCmd::resolve(...)` now returns `Result<IndexResolution, IndexResolveError>` instead of silently defaulting every missing context to `pwd`.
  - Explicit `/index workspace` and `/index crate` requests fail when loaded workspace/crate context is required but absent.
  - The bootstrap cases remain stable: no-DB workspace/crate roots still resolve from `pwd` when there is no loaded context yet.
  - Dispatcher emits `AppEvent::Error` for resolve failures, and the fast probe captures the error string via `resolve_error`.
- Tests run:
  - `cargo test -p ploke-tui --lib test_no_db_loaded_all_cases -- --nocapture`
  - `cargo test -p ploke-tui --lib test_single_member_all_cases -- --nocapture`
  - `cargo test -p ploke-tui --lib test_full_workspace_all_cases -- --nocapture`
  - `cargo test -p ploke-tui --lib`
- Result:
  - Passed: `189 passed; 0 failed; 5 ignored`
- Remaining gaps:
  - `/index` error branches that depend on more detailed workspace-member mismatch guidance are still partially represented in the canonical table.
  - The broader command taxonomy (`/load`, `/update`) still has transitional forwarding semantics outside this slice.

## Entry 11: `/index` UiError Emission Boundary
- Slice: replace raw `/index` resolve failure emission with the public `UiError` constructor chain, and teach the fast probe to capture the user-facing summary
- Files changed:
  - crates/ploke-tui/src/app_state/commands.rs
  - crates/ploke-tui/src/app_state/dispatcher.rs
  - crates/ploke-tui/src/app/commands/unit_tests/harness.rs
  - docs/active/agents/2026-04-05_decision_tree_refactor_handoff_log.md
- Contract decisions:
  - `/index` resolve failures now translate into a public `UiError` chain at the dispatcher boundary instead of emitting a raw `AppEvent::Error` string.
  - The fast probe records the user-facing message and recovery suggestion directly from the `/index` resolve error mapping, so the canonical contract can assert the visible failure text without depending on the old raw error event path.
  - Success-path `/index` behavior remains unchanged: successful resolutions still forward to `IndexTargetDir` and spawn indexing normally.
- Remaining gaps:
  - `/index` error guidance is still centralized in the resolve-error mapping helper rather than being fully expressed in the canonical decision table.
  - The broader command families (`/load`, `/save`, `/update`) still use transitional handling and have not yet been moved onto the same `UiError` boundary pattern.
- Tests run:
  - `cargo test -p ploke-tui --lib`
  - `cargo test -p ploke-tui --lib`
- Result:
  - Passed: `189 passed; 0 failed; 5 ignored`

## Entry 11: `/index` UiError Contract Tightening
- Slice: promote the stable full-workspace `/index` error rows into explicit UiError assertions and teach the canonical runner to prefer `resolve_error`
- Files changed:
  - crates/ploke-tui/src/app/commands/unit_tests/decision_tree.rs
  - docs/active/agents/2026-04-05_decision_tree_refactor_handoff_log.md
- Contract decisions:
  - The canonical decision table now treats full-workspace `/index crate <not member>` and `/index path/to/crate` error rows as explicit `Index` outcomes with user-facing error assertions.
  - The effect assertion helper now prefers `ValidationProbeEvent::resolve_error()` when matching expected `/index` error text, which keeps the contract aligned with the runtime's resolution boundary.
  - Error message matching stays intentionally coarse (`Failed to normalize target path`) so the table verifies the stable user-visible boundary instead of brittle path details.
- Tests run:
  - `cargo test -p ploke-tui --lib test_full_workspace_all_cases -- --nocapture`
- Result:
  - Passed: `1 passed; 0 failed; 0 ignored; 0 measured; 193 filtered out`
- Remaining gaps:
  - The other `/index` error rows in the table remain transitional and should only be promoted once their runtime behavior is confirmed stable.
  - `/load` and `/update` still carry older transitional contract vocabulary outside this slice.

## Entry 12: `/index` Stable Row Promotion Pass
- Slice: promote additional `/index` rows that already have deterministic runtime behavior, and leave the unstable workspace-error row pending
- Files changed:
  - crates/ploke-tui/src/app/commands/unit_tests/decision_tree.rs
  - docs/active/agents/2026-04-05_decision_tree_refactor_handoff_log.md
- Contract decisions:
  - Promoted the no-DB workspace-root `/index crate <name>` row to an explicit `Index` success assertion with a resolved target of `tests/fixture_workspace/ws_fixture_01/member_root`.
  - Promoted the no-DB crate-root `/index path/to/crate` row to an explicit `Index` failure assertion with the stable `Failed to normalize target path` user-facing message.
  - Promoted the single-member `/index crate <not member>` row to the same stable target-normalization failure assertion.
  - Promoted the standalone-crate `/index crate <different>` and `/index path/to/other` rows to the same stable target-normalization failure assertion.
  - Promoted the full-workspace `/index path/to/crate indexes if within workspace` row to an explicit `Index` success assertion with a resolved target of `tests/fixture_workspace/ws_fixture_01/member_root`.
  - Promoted the loaded-crate `/index crate <not loaded>` row to the same stable target-normalization failure assertion.
  - Left the standalone-crate `/index workspace` row pending because the current runtime still routes it through a non-error path, so the intended `UiError` contract is not yet stable there.
- Tests run:
  - `cargo test -p ploke-tui --lib`
- Result:
  - Passed: `189 passed; 0 failed; 5 ignored`
- Remaining gaps:
  - The remaining `/index` rows still marked `TestTodo` need to be promoted only after their runtime behavior is confirmed stable.
  - `/load` and `/update` still carry transitional contract vocabulary outside this slice.

## Entry 13: `/index crate ...` Workspace-Aware Resolution Check
- Slice: verify whether the remaining `/index crate ...` rows are stable enough to promote after the runtime slice
- Files changed:
  - docs/active/agents/2026-04-05_decision_tree_refactor_handoff_log.md
- Contract decisions:
  - No additional canonical rows were promoted in this pass.
  - Current `--lib` behavior still forwards the remaining `/index crate ...` rows in sections 3, 4, 5, and 6 instead of resolving them into the intended workspace-aware success or error outcomes.
  - The already-promoted rows remain valid as-is, but the unresolved crate-target rows should stay pending until `IndexCmd::resolve()` becomes workspace/member-aware for those cases.
- Tests run:
  - `cargo test -p ploke-tui --lib test_single_member_all_cases -- --nocapture`
  - `cargo test -p ploke-tui --lib test_standalone_crate_all_cases -- --nocapture`
  - `cargo test -p ploke-tui --lib test_full_workspace_all_cases -- --nocapture`
  - `cargo test -p ploke-tui --lib test_pwd_crate_loaded_all_cases -- --nocapture`
- Result:
  - Passed: all requested `--lib` checks for the inspected slices
- Remaining gaps:
  - `/index crate <focused>`, `/index crate <other member>`, `/index crate <member>`, and `/index crate <PWD match>` still need the runtime workspace-aware resolution change before they can be promoted safely.

## Entry 14: `/index crate ...` Canonical Table Promotion
- Slice: promote the rows that now have stable workspace-aware `/index crate ...` resolution and keep semantically mismatched rows pending
- Files changed:
  - crates/ploke-tui/src/app/commands/unit_tests/decision_tree.rs
  - docs/active/agents/2026-04-05_decision_tree_refactor_handoff_log.md
- Contract decisions:
  - Promoted `3.3 /index crate <focused> re-indexes focused` to an explicit `Index` success assertion with resolved target `tests/fixture_workspace/ws_fixture_01/member_root`.
  - Promoted `4.2 /index crate <loaded> re-indexes` to an explicit `Index` success assertion with resolved target `tests/fixture_crates/fixture_nodes`.
  - Promoted `5.2 /index crate <member> indexes that member` to an explicit `Index` success assertion with resolved target `tests/fixture_workspace/ws_fixture_01/member_root`.
  - Promoted `6.5 /index crate <PWD match> re-indexes` to an explicit `Index` success assertion with resolved target `tests/fixture_workspace/ws_fixture_01/member_root`.
  - Promoted `6.6 /index crate <different loaded> switches focus + indexes` to an explicit `Index` success assertion with resolved target `tests/fixture_workspace/ws_fixture_01/nested/member_nested`.
  - Tightened `3.5`, `4.3`, `5.3`, and `6.7` error assertions to the crate-aware target-resolution failure text instead of the old generic normalization-only substring.
  - Left `4.4 /index workspace error 'not a workspace'` pending because the current runtime still forwards it instead of emitting the expected `UiError`.
  - Left `3.4`, `6.2`, and `6.4` pending because their row names remain semantically mismatched to the current runtime behavior.
- Tests run:
  - `cargo test -p ploke-tui --lib test_single_member_all_cases -- --nocapture`
  - `cargo test -p ploke-tui --lib test_standalone_crate_all_cases -- --nocapture`
  - `cargo test -p ploke-tui --lib test_full_workspace_all_cases -- --nocapture`
  - `cargo test -p ploke-tui --lib test_pwd_crate_loaded_all_cases -- --nocapture`
  - `cargo test -p ploke-tui --lib`
- Result:
  - Passed: `189 passed; 0 failed; 5 ignored`
- Remaining gaps:
  - The remaining pending `/index crate ...` rows still need a runtime change before they can be promoted safely.
  - `/index workspace` for standalone crate remains pending until the dispatcher emits the `UiError` path there.

## Entry 14: `/index crate ...` Workspace-Aware Runtime Slice
- Slice: resolve `/index crate ...` by loaded workspace/crate context instead of path-joining under the focused crate root
- Files changed:
  - crates/ploke-tui/src/app_state/commands.rs
  - docs/active/agents/2026-04-05_decision_tree_refactor_handoff_log.md
- Contract decisions:
  - `mode=Crate` now first checks loaded state for an explicit crate match before falling back to path-joining semantics.
  - Loaded workspace requests resolve against the loaded workspace roots, so member names can map directly to their loaded crate roots.
  - Standalone crate requests can resolve the loaded crate name directly and return a user-facing `IndexResolveError` for other names.
  - No-DB behavior remains unchanged so `/index crate member_root` from a workspace root can still resolve through the existing relative-path flow.
- Tests run: not run in this slice
- Remaining gaps:
  - The remaining `/index crate ...` rows still need canonical-table promotion once the runtime behavior is confirmed stable under `--lib`.
  - The error text remains intentionally coarse for now so the existing decision-tree assertions continue to match the current user-facing boundary.

## Entry 15: `/index crate ...` Runtime Verification
- Slice: verify the workspace-aware `/index crate ...` resolution change against the affected `--lib` suites
- Files changed:
  - docs/active/agents/2026-04-05_decision_tree_refactor_handoff_log.md
- Contract decisions:
  - The new runtime branch is stable under the current canonical table.
  - `/index crate ...` now resolves through loaded workspace/crate context without breaking the already-promoted no-DB or explicit error assertions.
- Tests run:
  - `cargo test -p ploke-tui --lib test_single_member_all_cases -- --nocapture`
  - `cargo test -p ploke-tui --lib test_standalone_crate_all_cases -- --nocapture`
  - `cargo test -p ploke-tui --lib test_full_workspace_all_cases -- --nocapture`
  - `cargo test -p ploke-tui --lib test_pwd_crate_loaded_all_cases -- --nocapture`
  - `cargo test -p ploke-tui --lib`
- Result:
  - Passed: `189 passed; 0 failed; 5 ignored`
- Remaining gaps:
  - The remaining `/index crate ...` rows in sections 3, 4, 5, and 6 are still pending canonical promotion.
  - The user-facing error wording remains coarse and should be tightened once the table is updated to assert the intended policy-specific messages.
