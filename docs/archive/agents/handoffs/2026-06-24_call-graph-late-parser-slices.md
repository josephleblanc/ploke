# Handoff Changelog

Active running handoff notes for Codex/user continuity. Keep this file under 120 lines or 10 entries; archive to `docs/archive/agents/handoffs/` when it grows.

Previous archives:
- `docs/archive/agents/handoffs/2026-06-24_call-graph-progress.md`
- `docs/archive/agents/handoffs/2026-06-24_call-graph-slices.md`
- `docs/archive/agents/handoffs/2026-06-24_call-graph-dynamic-branch-slices.md`

## 2026-06-24 15:28 UTC - opaque function-pointer parameter fail-closed coverage completed

- Branch/HEAD: `prototype1-parent-mwv-live-r16-dangling-symlink-surface-fix-20260615t003337z-gen0` at `3be9c78a Make typed type graph baseline`; active goal remains open.
- Changed: added `fixture_call_graph::call_function_pointer_param`, `call_parenthesized_function_pointer_param`, and `call_function_pointer_param_cast` plus paranoid coverage proving opaque `fn() -> i32` parameters stay visible but fail closed.
- Behavior locked: direct `f()` records a `PathCall` value binding with `Unsupported`; `(f)()` records a `DynamicCallCallee::LocalBinding` with `Unsupported`; `(f as fn() -> i32)()` remains dynamic `Other` with `Unsupported`. No semantic edge is emitted for any opaque parameter case.
- Implementation note: no production classifier/resolver change was needed; existing param/value-binding guards already enforced the boundary.
- Verified through sub-agents: all three targeted parser tests passed; `cargo test -p syn_parser --features call_graph call_sites -- --nocapture` passed with `139 passed`; `cargo test -p ploke-transform --features call_graph transform::tests -- --nocapture` passed with `5 passed`; `cargo test -p ploke-db --features call_graph call_graph_queries -- --nocapture` passed with `9 passed`; `cargo test -p ploke-rag --features call_graph call_context_collection_attaches_outgoing_call_payloads -- --nocapture` passed with `1 passed`; `cargo check -p ploke-tui --features call_graph` passed.
- Local hygiene: `cargo fmt --all`, `git diff --check`, and `npx gitnexus detect-changes` passed; GitNexus reports low risk and 0 affected processes.
- Next good slices: closure-binding dynamic/cast/deref fail-closed fixtures, broader Fn/FnOnce value flow, closure/async owner modeling, or remaining proof-fact/call-context polish. Backup fixture review/regeneration remains approval-gated.

## 2026-06-24 15:30 UTC - stop point before next dynamic-call slice

- Branch/HEAD: `prototype1-parent-mwv-live-r16-dangling-symlink-surface-fix-20260615t003337z-gen0` at `3be9c78a Make typed type graph baseline`; active goal remains open and should not be marked complete yet.
- State: no new implementation started after the opaque function-pointer parameter slice. The working tree is still intentionally dirty with the call-graph implementation/test/docs set plus pre-existing `AGENTS.md`.
- Immediate next slice: add closure-binding dynamic-call guardrail coverage before changing production resolver logic. Suggested fixtures in `tests/fixture_crates/fixture_call_graph/src/lib.rs`: a closure binding cast callee like `(closure as fn() -> i32)()` and a dereferenced closure binding callee like `(*closure)()`.
- Expected behavior: both should parse as dynamic calls that fail closed with `CallResolutionStatus::Unsupported`, empty resolution paths, and no semantic call edge. If existing classifier logic already produces this, keep it as coverage-only.
- First tests to add: strict paranoid call-site tests in `crates/ingest/syn_parser/tests/uuid_phase3_resolution/call_sites.rs`, near the existing dynamic closure/function-pointer cases; update span constants and the call-site coverage matrix.
- Verification to rerun through sub-agents: targeted new parser tests, then `cargo test -p syn_parser --features call_graph call_sites -- --nocapture`, transform call-graph tests, DB call-graph query tests, RAG call-context test, and `cargo check -p ploke-tui --features call_graph`.
- Hygiene before handoff/commit: `cargo fmt --all`, `git diff --check`, and `npx gitnexus detect-changes`. Run GitNexus impact before editing any existing parser/resolver/transform symbols.

## 2026-06-24 15:34 UTC - closure-binding cast/deref guardrails completed

- Branch/HEAD: `prototype1-parent-mwv-live-r16-dangling-symlink-surface-fix-20260615t003337z-gen0` at `3be9c78a Make typed type graph baseline`; active goal remains open.
- Changed: added `fixture_call_graph::call_closure_binding_cast` and `call_dereferenced_closure_binding`, with paranoid tests proving `(closure as fn() -> i32)()` and `(*closure)()` remain dynamic, unsupported, and edge-free when `closure` is an opaque closure binding.
- Behavior locked: closure-binding casts are not admitted into function-pointer path proof, and dereferenced closure bindings are not treated as initialized function-pointer proofs. No production classifier/resolver change was needed.
- Docs: updated `docs/active/agents/call-graph/2026-06-22_call-site-coverage-matrix.md` to mark the D10/D11 closure-binding guardrails green and remove that specific gap from the remaining work row.
- Verified through sub-agents: targeted `closure_binding` parser filter passed with `2 passed`; `cargo test -p syn_parser --features call_graph call_sites -- --nocapture` passed with `141 passed`; transform call-graph tests passed with `5 passed`; DB call-graph query tests passed with `9 passed`; RAG call-context test passed; `cargo check -p ploke-tui --features call_graph` passed.
- Local hygiene: `cargo fmt --all`, `git diff --check`, and `npx gitnexus detect-changes` passed; GitNexus reports low risk and 0 affected processes.
- Next good slice: add remaining field/index/member dynamic-callee fail-closed fixtures, for example D08 `(s.callback)()` and D09 `funcs[0]()` before considering any semantic Fn-field/index resolution.

## 2026-06-24 15:38 UTC - field/index dynamic-callee guardrails completed

- Branch/HEAD: `prototype1-parent-mwv-live-r16-dangling-symlink-surface-fix-20260615t003337z-gen0` at `3be9c78a Make typed type graph baseline`; active goal remains open.
- Changed: added `CallbackHolder`, `fixture_call_graph::call_field_function_param`, and `call_indexed_function_pointer`, with paranoid tests for `(holder.callback)()` and `funcs[0]()`.
- Behavior locked: field callees rooted at opaque parameters record `DynamicCallCallee::FieldLocalBinding` and fail closed with `Unsupported`; indexed function-pointer callees remain plain dynamic `Other`, `Unsupported`, and edge-free. No production classifier/resolver change was needed.
- Docs: updated the call-site coverage matrix to mark D08/D09 green and move the remaining Fn/dynamic gap toward nested/member and broader Fn-flow cases.
- Verified through sub-agents: targeted `field_function_param` and `indexed_function_pointer` parser filters each passed; `cargo test -p syn_parser --features call_graph call_sites -- --nocapture` passed with `143 passed`; transform call-graph tests passed with `5 passed`; DB call-graph query tests passed with `9 passed`; RAG call-context test passed; `cargo check -p ploke-tui --features call_graph` passed.
- Local hygiene: `cargo fmt --all`, `git diff --check`, and `npx gitnexus detect-changes` passed; GitNexus reports low risk and 0 affected processes.
- Next good slices: move-closure literal/nested closure-body policy coverage, remaining nested/member dynamic callees, or broader Fn/FnOnce value-flow cases. Backup fixture review/regeneration remains approval-gated.

## 2026-06-24 15:43 UTC - move-closure literal boundary coverage completed

- Branch/HEAD: `prototype1-parent-mwv-live-r16-dangling-symlink-surface-fix-20260615t003337z-gen0` at `3be9c78a Make typed type graph baseline`; active goal remains open.
- Changed: added `fixture_call_graph::call_move_closure_literal_with_body_call` plus parser coverage for `(move || local_target())()`.
- Behavior locked: the outer move-closure literal call records as a `DynamicCall`, fails closed with `Unsupported`, and emits no edge; the nested `local_target()` inside the closure body is not attributed to the enclosing function owner until closure owners exist.
- Docs: updated the call-site coverage matrix to mark D03 green and remove the move-closure row from known fixture gaps.
- Verified through sub-agents: targeted `move_closure_literal` and `move_closure_body_call` parser filters passed; `cargo test -p syn_parser --features call_graph call_sites -- --nocapture` passed with `145 passed`; transform call-graph tests passed with `5 passed`; DB call-graph query tests passed with `9 passed`; RAG call-context test passed; `cargo check -p ploke-tui --features call_graph` passed.
- Local hygiene: `cargo fmt --all`, `git diff --check`, and `npx gitnexus detect-changes` passed; GitNexus reports low risk and 0 affected processes.
- Next good slices: remaining broader `if`/`match` dynamic callee guardrails, nested/member dynamic callees, or broader Fn/FnOnce value-flow cases. Backup fixture review/regeneration remains approval-gated.

## 2026-06-24 15:46 UTC - opaque if/match branch guardrails completed

- Branch/HEAD: `prototype1-parent-mwv-live-r16-dangling-symlink-surface-fix-20260615t003337z-gen0` at `3be9c78a Make typed type graph baseline`; active goal remains open.
- Changed: added `fixture_call_graph::call_if_function_pointer_param_branch` and `call_match_function_pointer_param_arm` plus parser coverage for `(if flag { f } else { f })()` and `(match flag { true => f, false => f })()` where `f: fn() -> i32` is a parameter.
- Behavior locked: opaque parameter branch/arm paths stay plain dynamic calls with `Unsupported` and no edge; they are not admitted into exact `IfBranchPaths` / `MatchArmPaths` resolution.
- Docs: updated the call-site coverage matrix D05/D06 rows and fixture index; remaining if/match work is now nested branch/arm expression coverage rather than basic resolved/ambiguous/guarded/non-path/opaque cases.
- Verified through sub-agents: targeted `if_function_pointer_param_branch` and `match_function_pointer_param_arm` parser filters passed; `cargo test -p syn_parser --features call_graph call_sites -- --nocapture` passed with `147 passed`; transform call-graph tests passed with `5 passed`; DB call-graph query tests passed with `9 passed`; RAG call-context test passed; `cargo check -p ploke-tui --features call_graph` passed.
- Local hygiene: `cargo fmt --all`, `git diff --check`, and `npx gitnexus detect-changes` passed; GitNexus reports low risk and 0 affected processes.
- Next good slices: nested if/match branch expression guardrails, remaining nested/member dynamic callees, or broader Fn/FnOnce value-flow cases. Backup fixture review/regeneration remains approval-gated.

## 2026-06-24 15:50 UTC - nested if/match dynamic-callee guardrails completed

- Branch/HEAD: `prototype1-parent-mwv-live-r16-dangling-symlink-surface-fix-20260615t003337z-gen0` at `3be9c78a Make typed type graph baseline`; active goal remains open.
- Changed: added `fixture_call_graph::call_if_nested_branch_expression` and `call_match_nested_arm_expression` plus parser coverage for nested branch/arm dynamic callees.
- Behavior locked: nested branch and arm expressions remain plain dynamic calls with `Unsupported` and no edge; only direct unshadowed item-path branch/arm bodies are admitted into exact `IfBranchPaths` / `MatchArmPaths` resolution.
- Docs: updated the call-site coverage matrix D05/D06 rows and fixture index; basic resolved, ambiguous, guarded, non-path, opaque parameter, and nested expression if/match dynamic-callee cases are now green.
- Verified through sub-agents: targeted `if_nested_branch_expression` and `match_nested_arm_expression` parser filters passed; `cargo test -p syn_parser --features call_graph call_sites -- --nocapture` passed with `149 passed`; transform call-graph tests passed with `5 passed`; DB call-graph query tests passed with `9 passed`; RAG call-context test passed; `cargo check -p ploke-tui --features call_graph` passed.
- Local hygiene: `cargo fmt --all`, `git diff --check`, and `npx gitnexus detect-changes` passed; GitNexus reports low risk and 0 affected processes.
- Next good slices: remaining nested/member dynamic callees, broader Fn/FnOnce value-flow cases, or fixture-review/regeneration once approved. Backup fixture review/regeneration remains approval-gated.

## 2026-06-24 15:53 UTC - graceful stop before next call-graph slice

- Branch/HEAD: `prototype1-parent-mwv-live-r16-dangling-symlink-surface-fix-20260615t003337z-gen0` at `3be9c78a Make typed type graph baseline`; active goal remains open and is not complete.
- State: no new implementation started after the nested if/match guardrail slice. The working tree remains intentionally dirty with the call-graph implementation/test/docs stack and pre-existing `AGENTS.md` changes.
- Last verified state: focused parser call-site suite passed with `149 passed`; transform call-graph tests passed with `5 passed`; DB call-graph query tests passed with `9 passed`; RAG call-context test passed; `cargo check -p ploke-tui --features call_graph`, `cargo fmt --all`, `git diff --check`, and `npx gitnexus detect-changes` all passed.
- Recommended next slice: cover P30 chained returned-function calls. Add fixture functions such as `unary_target`, `make_unary_fn`, and `call_chained_returned_function` using `make_unary_fn()(5)`.
- Expected P30 behavior: inner `make_unary_fn()` is a normal resolved local `PathCall`; outer `make_unary_fn()(5)` is a `DynamicCall` with `arg_count = 1`, `DynamicCallCallee::Other`, `Unsupported`, empty resolution paths, and no semantic edge.
- Files to inspect first: existing `call_returned_function` fixture/test patterns in `tests/fixture_crates/fixture_call_graph/src/lib.rs` and `crates/ingest/syn_parser/tests/uuid_phase3_resolution/call_sites.rs`; update `docs/active/agents/call-graph/2026-06-22_call-site-coverage-matrix.md` after tests are green.
- Required process: run GitNexus impact before editing any existing parser/resolver/test helper symbols; run targeted parser tests through a sub-agent first, then the focused call-graph bundle and hygiene checks. Backup fixture review/regeneration remains approval-gated because `docs/testing/BACKUP_DB_FIXTURES.md` is overdue.

## 2026-06-24 16:04 UTC - P30/P10/P24/M23/X04 parser coverage completed

- Branch/HEAD: `prototype1-parent-mwv-live-r16-dangling-symlink-surface-fix-20260615t003337z-gen0` at `3be9c78a Make typed type graph baseline`; active goal remains open.
- Changed: added fixture coverage for `make_unary_fn()(5)`, `vec![1, 2, 3]`, `unsafe { unsafe_target() }`, `NewType(value)`, and trait-default `self.required()`.
- Behavior locked: P30 records an inner resolved local `PathCall` plus an outer one-argument unsupported `DynamicCall`; X04 records only the macro invocation; P10 resolves the local unsafe function call while leaving unsafe metadata future; P24 resolves a tuple-struct constructor edge; M23 records `SelfValue` and fails closed with `Unsupported`.
- Docs: updated `docs/active/agents/call-graph/2026-06-22_call-site-coverage-matrix.md` to mark P30, X04, P10, P24, and M23 green.
- Verified through sub-agents: targeted `chained_returned_function`, `vec_macro`, `unsafe_function`, `new_type_constructor`, and `required_self_method` filters passed; final bundle passed with `cargo test -p syn_parser --features call_graph call_sites -- --nocapture` at `155 passed`, transform call-graph tests at `5 passed`, DB call-graph query tests at `9 passed`, RAG call-context test at `1 passed`, and `cargo check -p ploke-tui --features call_graph` passed.
- Local hygiene: `cargo fmt --all`, `git diff --check`, and `npx gitnexus detect-changes` passed; GitNexus reports low risk and 0 affected processes. No cargo/rustc/rustdoc processes were left running.
- Next good slices: remaining explicit gaps include P09 extern/FFI classification, M25 explicit `.drop()` behavior, imported/renamed macro X07, test-body macro X05 if test bodies are routed, and broader Fn/FnOnce/member dynamic-flow cases. Backup fixture review/regeneration remains approval-gated.

## 2026-06-24 16:10 UTC - X07/M25 parser coverage completed

- Branch/HEAD: `prototype1-parent-mwv-live-r16-dangling-symlink-surface-fix-20260615t003337z-gen0` at `3be9c78a Make typed type graph baseline`; active goal remains open.
- Changed: added `call_graph_exported_alias_macro` imported as `imported_macro_alias`, plus `ExplicitDropTarget::drop(self)` and `call_explicit_drop_method`.
- Behavior locked: imported/renamed macro invocations record the visible alias spelling as a `MacroCall`, `Unsupported`, no edge; valid inherent `value.drop()` resolves as an ordinary local method call with no destructor special-casing.
- Docs: updated the call-site coverage matrix to mark X07 and M25 green.
- Verified through sub-agents: targeted `imported_macro_alias` and `explicit_drop_method` filters passed; final bundle passed with parser call-sites at `157 passed`, transform call-graph tests at `5 passed`, DB call-graph query tests at `9 passed`, RAG call-context test at `1 passed`, and `cargo check -p ploke-tui --features call_graph` passed.
- Local hygiene: `cargo fmt --all`, `git diff --check`, and `npx gitnexus detect-changes` passed; GitNexus reports low risk and 0 affected processes. No cargo/rustc processes were left running.
- Next good slices: P09 extern/FFI classification needs resolver-boundary inspection before codifying `External`/FFI semantics; X05 depends on whether cfg-test modules are routed; X10 legal item-macro-in-body coverage and broader Fn/FnOnce/member dynamic-flow cases remain. Backup fixture review/regeneration remains approval-gated.

## 2026-06-24 16:15 UTC - X10 and parenthesized Fn-flow coverage completed

- Branch/HEAD: same; active goal remains open.
- Changed: added `call_item_macro_inside_body`, `call_parenthesized_generic_fn_once_value_binding`, and `call_parenthesized_boxed_dyn_fn_value_binding`.
- Behavior locked: statement-position item macro invocations are recorded only as unsupported `MacroCall`s; `(generic_f)()` and `(boxed_fn)()` record dynamic local bindings and fail closed, while boxed setup `Box::new(local_target)` remains `External`.
- Verified: targeted `item_macro_inside_body` and `parenthesized_` filters passed; final bundle passed with parser call-sites at `161 passed`, transform `5 passed`, DB `9 passed`, RAG `1 passed`, and TUI feature check passed. `cargo fmt --all`, `git diff --check`, and `npx gitnexus detect-changes` passed; GitNexus low risk, 0 affected processes.
- Next: remaining small rows are not purely mechanical: P09 needs FFI resolver-boundary design, X05 needs cfg-test routing decision, D14 needs async/coroutine closure support. Broader Fn/member flow and backup fixture review/regeneration remain open.
