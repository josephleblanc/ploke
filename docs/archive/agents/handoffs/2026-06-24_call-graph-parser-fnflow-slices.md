# Handoff Changelog

Archived from `docs/active/agents/HANDOFF_CHANGELOG.md` on 2026-06-24 17:59 UTC before appending the constructed-holder alias handoff.

Active running handoff notes for Codex/user continuity. Keep this file under 120 lines or 10 entries; archive to `docs/archive/agents/handoffs/` when it grows.

Previous archives:
- `docs/archive/agents/handoffs/2026-06-24_call-graph-progress.md`
- `docs/archive/agents/handoffs/2026-06-24_call-graph-slices.md`
- `docs/archive/agents/handoffs/2026-06-24_call-graph-dynamic-branch-slices.md`
- `docs/archive/agents/handoffs/2026-06-24_call-graph-late-parser-slices.md`

## 2026-06-24 16:22 UTC - graceful stop after parser coverage slices

- Branch/HEAD: `prototype1-parent-mwv-live-r16-dangling-symlink-surface-fix-20260615t003337z-gen0` at `3be9c78a Make typed type graph baseline`; active call-graph implementation goal remains open and should not be marked complete.
- State: stopped before starting another implementation slice. Working tree remains intentionally dirty with the call-graph implementation/test/docs stack, untracked call-graph DB/parser files, this handoff refresh, and pre-existing `AGENTS.md` changes.
- Changed this stop pass: archived the previous 116-line handoff to `docs/archive/agents/handoffs/2026-06-24_call-graph-late-parser-slices.md` and reset the active changelog to this compact continuation note.
- Last verified implementation state: parser call-site suite passed at `161 passed`; transform call-graph tests passed at `5 passed`; DB call-graph query tests passed at `9 passed`; RAG call-context test passed at `1 passed`; `cargo check -p ploke-tui --features call_graph`, `cargo fmt --all`, `git diff --check`, and `npx gitnexus detect-changes` passed.
- Most recent completed behavior: X10 statement-position item macros are unsupported macro calls with no expansion edge; parenthesized generic `FnOnce` and boxed `dyn Fn` value calls remain unsupported dynamic local bindings; `Box::new(local_target)` remains classified `External`.
- Next implementation options: P09 extern/FFI classification, X05 `assert_eq!`/cfg-test body routing, D14 async/coroutine closure calls, broader non-self/member/Fn-flow resolution, import/re-export/glob-aware path resolution, or downstream proof-fact/call-context polish.
- Recommended next move: inspect P09 first, but do not implement until the resolver boundary is clear. Check whether foreign functions are modeled from `ItemForeignMod`/`ForeignItemFn`, then decide whether local extern declarations should classify as `External`/FFI without weakening unresolved-call invariants.
- Commands to start next session: inspect `crates/ingest/syn_parser/src/resolve/call_resolution.rs`, parser visitor foreign-item handling, and `docs/active/agents/call-graph/2026-06-22_call-site-coverage-matrix.md`; run GitNexus impact before editing any existing symbol; run tests through sub-agents.
- Constraints: keep `call_graph` gated; do not touch backup fixtures without explicit approval. `docs/testing/BACKUP_DB_FIXTURES.md` review is overdue, so ask before fixture regeneration/review.

## 2026-06-24 16:31 UTC - P09 extern FFI and X05 test macro coverage completed

- Branch/HEAD: same branch at `3be9c78a`; active call-graph implementation goal remains open.
- Changed: added `ImportKind::ExternFunction`, foreign-function import recording for both syn visitors, resolver classification for scoped foreign-function imports, transform import-kind projection, and paranoid helper support.
- Fixtures/tests: added `fixture_call_graph::call_extern_c_function` for `unsafe extern "C" { fn abs(...) }` plus `#[cfg(test)] mod call_graph_tests::assert_eq_macro_call`.
- Behavior locked: `abs(value)` is a `PathCall` classified `External` with no local edge; `assert_eq!(1 + 1, 2)` in a cfg-test body is a `MacroCall` with cfg `test`, `Unsupported`, no edge.
- Verified: targeted P09 and X05 tests passed; focused bundle passed with parser call-sites `163 passed`, transform `5 passed`, DB `9 passed`, RAG `1 passed`; `cargo check -p ploke-tui --features call_graph`, `cargo fmt --all`, `git diff --check`, and `npx gitnexus detect-changes` passed.
- Next: D14 async/coroutine closure remains a named red row; broader non-self/member/Fn-flow, import/re-export/glob path resolution, downstream proof-fact/call-context polish, and backup fixture review/regeneration remain open.

## 2026-06-24 16:33 UTC - stop checkpoint before next slice

- Stop reason: user asked to close out gracefully before starting another implementation slice.
- Goal state: active call-graph goal remains open; do not mark it complete.
- Recommended next slice: start with D14 async/coroutine closure only after checking current Rust/syn support and nearby closure/dynamic-call extraction patterns. Add a fail-first paranoid fixture if behavior is meant to change.
- If D14 is not viable yet: move to the next bounded parser/resolver slice from the matrix, preferably broader non-self/member/Fn-flow guardrails or import/re-export/glob path resolution. Keep each slice small enough to verify with focused parser, transform, DB, RAG, and TUI checks as applicable.
- Before editing: run GitNexus impact for existing symbols; warn on HIGH/CRITICAL risk. Keep `call_graph` gated and do not touch backup fixtures without explicit approval because fixture review is overdue.

## 2026-06-24 16:44 UTC - D14 and D08 member dynamic slices completed

- Branch/HEAD: same branch at `3be9c78a`; active call-graph implementation goal remains open.
- Changed: added D14 async-closure fixture coverage for `(async || local_target())()` as an unsupported outer `DynamicCall` and verified the inner closure-body call is not attributed to the enclosing owner.
- Changed: extended `call_extraction` constructed-local proof from tuple constructor fields to direct named struct-literal fields, preserving fail-closed initializer proof. `NamedCallbackHolder { callback: local_target }` now records `FieldInitializedLocalBinding` and resolves `(holder.callback)()` to `CallRelation::DynamicFunction`.
- Verified: focused D14 filter `2 passed`; named-field fail-first test failed as `FieldLocalBinding` vs `FieldInitializedLocalBinding`, then passed after implementation; parser call-sites `166 passed`; transform `5 passed`; DB call-graph queries `9 passed`; RAG call-context `1 passed`; `cargo check -p ploke-tui --features call_graph`, `cargo fmt --all`, and `git diff --check` passed.
- Next candidate: indexed initialized dynamic calls, e.g. `let funcs = [local_target]; funcs[0]()`. This likely needs a new persisted `DynamicCallCallee` classifier plus parser, resolver, transform, and DB parsing updates. GitNexus reports the public resolver entry `resolve_call_relations_after_tree` as CRITICAL blast radius, so keep the slice narrow and verify downstream surfaces.

## 2026-06-24 17:18 UTC - D09 indexed initialized dynamic slice completed

- Branch/HEAD: same branch at `3be9c78a`; active call-graph implementation goal remains open.
- Changed: added `DynamicCallCallee::IndexedInitializedLocalBinding` and parser proof for exact local array literals with path-valued elements plus literal indexes. `let funcs = [local_target]; funcs[0]()` now records path `["funcs", "0"]`, initializer `["local_target"]`, and resolves to `CallRelation::DynamicFunction`.
- Preserved invariant: opaque indexed parameters such as `call_indexed_function_pointer(funcs)` still record unsupported `DynamicCall`/`Other` and remain edge-free.
- Verified: fail-first indexed test failed as `Other` vs `IndexedInitializedLocalBinding`, then passed; parser call-sites `167 passed`; transform `5 passed`; DB call-graph queries `9 passed`; RAG call-context `1 passed`; `cargo check -p ploke-tui --features call_graph`, `cargo fmt --all`, and `git diff --check` passed.
- Next candidates: remaining broader Fn-flow forms, broader import/re-export/glob path resolution, or downstream proof-fact/call-context polish. Keep `call_graph` gated and do not touch backup fixtures without explicit approval.

## 2026-06-24 17:21 UTC - graceful stop before next implementation slice

- Stop reason: user asked to close out soon and leave next-step notes rather than start another code slice.
- Branch/HEAD: same branch at `3be9c78a`; active autonomous call-graph implementation goal remains open and incomplete.
- Current dirty state: broad call-graph implementation stack remains unstaged, including parser/resolver/transform/DB/RAG/TUI/docs/fixture edits plus untracked `call_extraction_syn1.rs`, `ploke-db/src/call_graph.rs`, DB query tests, active handoff notes, and handoff archives.
- Most recent verified state remains the 17:18 UTC D09 bundle: parser call-sites `167 passed`, transform `5 passed`, DB call-graph queries `9 passed`, RAG call-context `1 passed`, `cargo check -p ploke-tui --features call_graph`, `cargo fmt --all`, and `git diff --check` passed. No new tests were run after this stop note.
- Recommended next slice: add fail-first coverage for a typed exact indexed initializer, e.g. `let funcs: [fn() -> i32; 1] = [local_target]; funcs[0]()` in `fixture_call_graph`, then preserve fail-closed behavior for opaque indexed parameters. This should likely stay parser-local if `DynamicCallCallee::IndexedInitializedLocalBinding` can be reused.
- Likely implementation point: inspect `LocalBindingProof::Array` and the `syn::Pat::Type` branch in `crates/ingest/syn_parser/src/parser/visitor/call_extraction.rs`. A conservative shape is to carry optional `type_path` on array proof so typed arrays can still act as typed method receivers while indexed initializer proof uses the element initializer list.
- Guardrails for the next session: run GitNexus impact before editing existing symbols; warn again before touching `resolve_call_relations_after_tree` because prior impact was CRITICAL; keep `call_graph` gated; do not touch backup fixtures without explicit approval because fixture review remains overdue.

## 2026-06-24 17:26 UTC - typed indexed initializer slice completed

- Branch/HEAD: same branch at `3be9c78a`; active autonomous call-graph implementation goal remains open.
- Changed: added `fixture_call_graph::call_typed_indexed_initialized_function_array` for `let funcs: [fn() -> i32; 1] = [local_target]; funcs[0]()` and a paranoid test expecting `IndexedInitializedLocalBinding` resolved to `CallRelation::DynamicFunction`.
- Parser fix: the `syn::Pat::Type` branch in `call_extraction.rs` now checks `array_init(...)` before falling back to typed/path-initializer proof, reusing the existing D09 classifier/resolver/persistence path without schema changes.
- Fail-first evidence: focused test initially failed with callee `Other` vs expected `IndexedInitializedLocalBinding { path: ["funcs", "0"], init_path: ["local_target"] }`, then passed after the parser change.
- Verified: parser call-sites `168 passed`; transform `5 passed`; DB call-graph queries `9 passed`; RAG call-context `1 passed`; `cargo check -p ploke-tui --features call_graph`, `cargo fmt --all`, and `git diff --check` passed.
- Next candidates: broader Fn-flow forms beyond exact initializer proof, import/re-export/glob path resolution gaps still visible in the matrix, or downstream proof-fact/call-context polish. Keep `call_graph` gated and do not touch backup fixtures without explicit approval.

## 2026-06-24 17:30 UTC - aliased indexed initializer slice completed

- Branch/HEAD: same branch at `3be9c78a`; active autonomous call-graph implementation goal remains open.
- Changed: added `fixture_call_graph::call_aliased_indexed_initialized_function_array` for `let funcs = [local_target]; let alias = funcs; alias[0]()` and a paranoid test expecting `IndexedInitializedLocalBinding` resolved to `CallRelation::DynamicFunction`.
- Parser fix: added `array_binding_init(...)` proof cloning for visible locals already proven as `LocalBindingProof::Array`; opaque parameters and non-array bindings still fail closed.
- Fail-first evidence: focused test initially failed with callee `Other` vs expected `IndexedInitializedLocalBinding { path: ["alias", "0"], init_path: ["local_target"] }`, then passed after the parser change.
- Verified: parser call-sites `169 passed`; transform `5 passed`; DB call-graph queries `9 passed`; RAG call-context `1 passed`; `cargo check -p ploke-tui --features call_graph`, `cargo fmt --all`, and `git diff --check` passed.
- Next candidates: remaining broader Fn-flow forms, broader import/re-export/glob path resolution, or downstream proof-fact/call-context polish. Keep `call_graph` gated and do not touch backup fixtures without explicit approval.

## 2026-06-24 17:38 UTC - indexed constructed-field array slice completed

- Branch/HEAD: same branch at `3be9c78a`; active autonomous call-graph implementation goal remains open.
- Changed: added `CallbackArrayHolder`, `call_indexed_field_function_param`, and `call_indexed_named_field_function_binding`. Opaque `holder.callbacks[0]()` stays unsupported; constructed `CallbackArrayHolder { callbacks: [local_target] }` resolves `holder.callbacks[0]()` to `CallRelation::DynamicFunction`.
- Parser fix: added `FieldInitProof::{Path, Array}` for named constructed fields and taught indexed dynamic callee extraction to consume array proof from constructed local fields while preserving direct path proof for existing field-call behavior.
- Fail-first evidence: focused constructed-field indexed test initially failed with callee `Other` vs expected `IndexedInitializedLocalBinding { path: ["holder", "callbacks", "0"], init_path: ["local_target"] }`, then passed; the opaque parameter guard also passed.
- Verified: parser call-sites `171 passed`; transform `5 passed`; DB call-graph queries `9 passed`; RAG call-context `1 passed`; `cargo check -p ploke-tui --features call_graph`, `cargo fmt --all`, and `git diff --check` passed.
- Next candidates: remaining broader Fn-flow forms, broader import/re-export/glob path resolution, or downstream proof-fact/call-context polish. Keep `call_graph` gated and do not touch backup fixtures without explicit approval.

## 2026-06-24 17:46 UTC - indexed tuple-field array slice completed

- Branch/HEAD: same branch at `3be9c78a`; active autonomous call-graph implementation goal remains open.
- Changed: added `TupleCallbackArrayHolder`, `call_indexed_tuple_field_function_param`, and `call_indexed_tuple_field_function_binding`. Opaque `holder.0[0]()` stays unsupported; constructed `TupleCallbackArrayHolder([local_target])` resolves `holder.0[0]()` to `CallRelation::DynamicFunction`.
- Parser fix: generalized tuple constructor field proof to reuse `FieldInitProof::{Path, Array}`; existing direct tuple-field function calls still consume only `Path` proof, while indexed tuple-field calls consume only `Array` proof.
- Evidence: a fail-first/stale-span run found no matching dynamic ID; a temporary debug inspection showed actual tuple-field call spans exclude the final call paren. After correcting spans, the resolving tuple-field test, opaque tuple-field guard, and existing direct tuple-field test all passed.
- Verified: parser call-sites `173 passed`; transform `5 passed`; DB call-graph queries `9 passed`; RAG call-context `1 passed`; `cargo check -p ploke-tui --features call_graph`, `cargo fmt --all`, and `git diff --check` passed.
- Next candidates: remaining broader Fn-flow forms, broader import/re-export/glob path resolution, or downstream proof-fact/call-context polish. Keep `call_graph` gated and do not touch backup fixtures without explicit approval.

## 2026-06-24 17:52 UTC - field array-alias slice completed

- Branch/HEAD: same branch at `3be9c78a`; active autonomous call-graph implementation goal remains open.
- Changed: added `call_indexed_named_field_array_alias_binding` and `call_indexed_tuple_field_array_alias_binding`, covering `funcs = [local_target]` passed through named and tuple constructed fields before indexing.
- Parser fix: `constructed_field_init(...)` now lifts a path-valued field initializer to `FieldInitProof::Array` only when the path names a visible local already proven as `LocalBindingProof::Array`; otherwise it falls back to existing path proof.
- Fail-first evidence: both new tests initially failed as callee `Other` vs expected `IndexedInitializedLocalBinding`, then passed after the proof change.
- Verified: parser call-sites `175 passed`; transform `5 passed`; DB call-graph queries `9 passed`; RAG call-context `1 passed`; `cargo check -p ploke-tui --features call_graph`, `cargo fmt --all`, and `git diff --check` passed.
- Next candidates: remaining broader Fn-flow forms, broader import/re-export/glob path resolution, or downstream proof-fact/call-context polish. Keep `call_graph` gated and do not touch backup fixtures without explicit approval.
