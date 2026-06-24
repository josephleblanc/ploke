# Handoff Changelog

Active running handoff notes for Codex/user continuity. Keep this file under 120 lines or 10 entries; archive to `docs/archive/agents/handoffs/` when it grows.

Previous archive: `docs/archive/agents/handoffs/2026-06-24_call-graph-slices.md`.

## 2026-06-24 02:45 UTC - stop during tuple-field dynamic call slice

- Branch/HEAD: `prototype1-parent-mwv-live-r16-dangling-symlink-surface-fix-20260615t003337z-gen0` at `3be9c78a Make typed type graph baseline`.
- Active goal remains open: finish the remaining call-graph plan autonomously, following nearby parser/resolver/transform/DB/RAG/TUI patterns and preserving strict invariants plus the `call_graph` rollout gate.
- Changed since prior archive: implemented tuple-field dynamic function call proof for `let value = TupleFieldFunction(local_target); value.0()`. Parser now records `DynamicCallCallee::FieldInitializedLocalBinding { path, init_path }`, resolver resolves it through the constructor argument value-flow proof, transform preserves the callee path, and paranoid fixture coverage now expects `ResolvedDynamicFunctionLocalExact`.
- Verified through sub-agents: `cargo test -p syn_parser --features call_graph call_sites -- --nocapture` passed with `125 passed`; `cargo test -p ploke-transform --features call_graph transform::tests -- --nocapture` passed with `4 passed`; `cargo test -p ploke-db --features call_graph call_graph_queries -- --nocapture` passed with `9 passed`; `cargo test -p ploke-rag --features call_graph call_context_collection_attaches_outgoing_call_payloads -- --nocapture` passed with `1 passed`; `cargo check -p ploke-tui --features call_graph` passed.
- Known follow-up from this slice: downstream DB/RAG target enums still need explicit `DynamicFunction` representation. Add `CallRelationKind::DynamicFunction`, `CallTargetKind::DynamicFunction`, map it in `ploke-rag`, and add DB/RAG tests with a resolved dynamic function relation. The focused DB/RAG checks passed because they did not yet exercise that resolved relation kind.
- Next implementation candidates: after the `DynamicFunction` downstream coverage fix, continue with proof-backed gaps such as closure/async/const/static body ownership, import/re-export/glob-aware path resolution, associated-function path calls, tuple struct and enum variant constructor resolution, and TUI call-context surfaces.
- Still approval-gated: backup fixture review/regeneration and any relaxation of schema/import/resolver correctness. `docs/testing/BACKUP_DB_FIXTURES.md` review remains overdue from 2026-06-12; ask before touching backup fixtures.
- Dirty state is intentional broad call-graph work plus pre-existing/user-owned `AGENTS.md`. Do not reset, clean, or discard unrelated changes.

## 2026-06-24 03:02 UTC - DynamicFunction downstream projection completed

- Changed: preserved `CallRelation::DynamicFunction` as persisted `relation_kind = "DynamicFunction"` instead of flattening it to `Function`; added `CallRelationKind::DynamicFunction`, `CallTargetKind::DynamicFunction`, and the RAG mapping.
- Tests/docs: added transform projection coverage for `(local_target)()`, DB decode coverage for a resolved dynamic function row, and RAG call-context coverage for `CallTargetKind::DynamicFunction`. Call-graph README/matrix now record the downstream projection behavior.
- Verified through sub-agents: `cargo test -p syn_parser --features call_graph call_sites -- --nocapture` passed with `125 passed`; `cargo test -p ploke-transform --features call_graph transform::tests -- --nocapture` passed with `5 passed`; `cargo test -p ploke-db --features call_graph call_graph_queries -- --nocapture` passed with `9 passed`; `cargo test -p ploke-rag --features call_graph call_context_collection_attaches_outgoing_call_payloads -- --nocapture` passed with `1 passed`; `cargo check -p ploke-tui --features call_graph` passed.
- Hygiene: `cargo fmt --all`, `git diff --check`, and `npx gitnexus detect-changes` passed; GitNexus reported low risk, 0 affected processes across the dirty worktree.
- Next: continue proof-backed resolver/model gaps. Good candidates are closure/async/const/static body ownership, remaining import/re-export/glob path resolution, tuple struct/enum variant constructor resolution, or richer TUI call-context rendering. Backup fixture review/regeneration remains approval-gated.

## 2026-06-24 03:12 UTC - TUI call-context details completed

- Changed: model-facing RAG context and the context-plan overlay now render outgoing call summaries with callee shape, span, status/resolution, and target relation IDs instead of only compact call counts.
- Tests/docs: added fail-first `reformat_context_to_system_includes_call_context_details`; call-graph README/matrix now mark richer TUI/system call-context rendering as implemented.
- Verified through sub-agents: `cargo test -p ploke-tui reformat_context_to_system_includes_call_context_details -- --nocapture` failed before implementation and passed after; `cargo test -p ploke-tui rag::context -- --nocapture` passed with `4 passed`; `cargo check -p ploke-tui --features call_graph` passed.
- Next: remaining non-fixture-gated gaps are mostly resolver/model work: closure/async body ownership, broader Fn/FnOnce/function-pointer resolution, broader trait dispatch, and remaining import/associated-function edge cases beyond the exact proofs already documented.

## 2026-06-24 stop note - pause before function-pointer cast dynamic call slice

- Stop state: no new implementation edits were started after the TUI call-context slice. The active goal is still open; do not mark it complete or blocked just because this session paused.
- Good next slice: add proof-backed dynamic function-pointer cast calls, starting with `(local_target as fn() -> i32)()`. Keep it narrow: only classify/resolve casts where the callee expression is a path cast to `syn::Type::BareFn`; everything else should remain unsupported rather than becoming permissive.
- Expected implementation shape: add a `DynamicCallCallee` variant such as `FnPointerCastPath { path }`, classify it in `call_extraction.rs` and the syn1 extraction path, resolve it in `resolve_dynamic_call` through the same exact local/external path resolver used by dynamic path calls, and include the variant in transform projection path handling.
- Expected tests first: add a fixture function in `tests/fixture_crates/fixture_call_graph/src/lib.rs`, add a strict paranoid call-site expectation in `crates/ingest/syn_parser/tests/uuid_phase3_resolution/call_sites.rs`, and run the focused parser test before implementation to confirm it fails for the intended reason.
- Files likely touched next: `parser/nodes/call.rs`, `parser/visitor/call_extraction.rs`, `parser/visitor/call_extraction_syn1.rs`, `resolve/call_resolution.rs`, `ploke-transform/src/schema/edges.rs`, `tests/common/call_site_paranoid.rs`, the call-graph fixture, and the call-site coverage docs.
- Required process: rerun smaller targeted reads before editing; run GitNexus impact for `classify_dynamic_callee`, `resolve_dynamic_call`, and `call_site_to_params`; run tests through sub-agents; finish with `cargo fmt --all`, `git diff --check`, and `npx gitnexus detect-changes`.
- Still do not touch backup fixtures without explicit approval. The backup fixture review date in `docs/testing/BACKUP_DB_FIXTURES.md` remains overdue from 2026-06-12.

## 2026-06-24 resume note - function-pointer cast dynamic call slice completed

- Changed: added `DynamicCallCallee::FnPointerCastPath { path }` and fixture coverage for `fixture_call_graph::call_function_pointer_cast_path`, whose `(local_target as fn() -> i32)()` call resolves to a local exact `CallRelation::DynamicFunction`.
- Guardrail: the main `syn` classifier only emits this variant for a path cast to `syn::Type::BareFn`; one-segment names shadowed by visible locals/parameters still fail closed as unsupported rather than faking an edge to a module function.
- Projection: `call_site_to_params` now persists the cast-path dynamic callee path, and `test_call_graph_projection_for_dynamic_function_call` asserts the cast row's `call_site`, `call_relation`, and `call_resolution_status` projection.
- Verified through sub-agents: fail-first targeted parser test first failed with `Other` vs `FnPointerCastPath`; after implementation, the targeted parser test passed, the local-binding cast guard test passed as `Unsupported`, `cargo test -p syn_parser --features call_graph call_sites -- --nocapture` passed with `127 passed`, `cargo test -p ploke-transform --features call_graph transform::tests -- --nocapture` passed with `5 passed`, `cargo test -p ploke-db --features call_graph call_graph_queries -- --nocapture` passed with `9 passed`, and `cargo test -p ploke-rag --features call_graph call_context_collection_attaches_outgoing_call_payloads -- --nocapture` passed.
- Local hygiene: `cargo fmt --all`, `cargo check -p ploke-tui --features call_graph`, `git diff --check`, and `npx gitnexus detect-changes` passed; GitNexus reports low risk and 0 affected processes across the dirty worktree.
- Next good slices: broader Fn/FnOnce/function-pointer value flow, closure/async body ownership model, or remaining trait-dispatch/import edge cases. Backup fixture review/regeneration remains approval-gated.

## 2026-06-24 03:15 UTC - initialized function-pointer cast binding slice completed

- Changed: added `DynamicCallCallee::FnPointerCastInitializedLocalBinding { path, init_path }` and upgraded `fixture_call_graph::call_function_pointer_cast_binding` so `let f: fn() -> i32 = local_target; (f as fn() -> i32)()` resolves to a local exact `CallRelation::DynamicFunction`.
- Guardrail: only visible local bindings that already carry initializer-path proof enter this variant; opaque locals/parameters remain unsupported instead of fake-resolving to same-named module functions.
- Projection/docs: `call_site_to_params` preserves the binding path, transform projection tests assert the initialized cast row's `call_site`, `call_relation`, and `call_resolution_status`, and the call-graph README/matrix now list exact initialized-binding casts as implemented while broader Fn/FnOnce flow remains future work.
- Verified through sub-agents: targeted parser test failed first with `Other` vs `FnPointerCastInitializedLocalBinding`, then passed after implementation; `cargo test -p syn_parser --features call_graph call_sites -- --nocapture` passed with `127 passed`; `cargo test -p ploke-transform --features call_graph transform::tests -- --nocapture` passed with `5 passed`; `cargo test -p ploke-db --features call_graph call_graph_queries -- --nocapture` passed with `9 passed`; `cargo test -p ploke-rag --features call_graph call_context_collection_attaches_outgoing_call_payloads -- --nocapture` passed.
- Local hygiene: `cargo fmt --all`, `cargo check -p ploke-tui --features call_graph`, `git diff --check`, and `npx gitnexus detect-changes` passed; GitNexus reports low risk and 0 affected processes. Backup fixture review/regeneration remains approval-gated.

## 2026-06-24 03:21 UTC - dereferenced function-pointer binding slice completed

- Changed: added `DynamicCallCallee::DereferencedInitializedLocalBinding { path, init_path }` and fixture coverage for `fixture_call_graph::call_dereferenced_function_pointer_binding`, whose `let f: fn() -> i32 = local_target; (*f)()` resolves to a local exact `CallRelation::DynamicFunction`.
- Guardrail: only dereferenced local bindings with existing initializer-path proof enter this variant; opaque deref calls remain unsupported.
- Projection/docs: `call_site_to_params` persists the dereferenced binding path, transform projection tests assert the deref row's `call_site`, `call_relation`, and `call_resolution_status`, and the call-graph README/matrix now mark exact initialized deref calls as implemented while broader opaque Fn/FnOnce flow remains future work.
- Verified through sub-agents: targeted parser test failed first with `Other` vs `DereferencedInitializedLocalBinding`, then passed after implementation; `cargo test -p syn_parser --features call_graph call_sites -- --nocapture` passed with `128 passed`; `cargo test -p ploke-transform --features call_graph transform::tests -- --nocapture` passed with `5 passed`; `cargo test -p ploke-db --features call_graph call_graph_queries -- --nocapture` passed with `9 passed`; `cargo test -p ploke-rag --features call_graph call_context_collection_attaches_outgoing_call_payloads -- --nocapture` passed.
- Local hygiene: `cargo fmt --all`, `cargo check -p ploke-tui --features call_graph`, `git diff --check`, and `npx gitnexus detect-changes` passed; GitNexus reports low risk and 0 affected processes. Backup fixture review/regeneration remains approval-gated.

## 2026-06-24 14:57 UTC - single-path block dynamic callee slice completed

- Changed: added fixture coverage for `fixture_call_graph::call_block_function_item`, whose `({ local_target })()` parenthesized single-path block callee normalizes to `DynamicCallCallee::Path { path: ["local_target"] }` and resolves to local exact `CallRelation::DynamicFunction`.
- Guardrail: only expression blocks with exactly one path expression and no statements use this path; nontrivial block/if/match dynamic callees remain unsupported.
- Projection/docs: transform tests assert the block row's persisted `call_site`, `call_relation`, and `call_resolution_status`; call-graph README/matrix now mark the D07 single-path block subset as implemented.
- Verified through sub-agents: targeted parser test failed first with `Other` vs `Path`, then passed after implementation; `cargo test -p syn_parser --features call_graph call_sites -- --nocapture` passed with `129 passed`; `cargo test -p ploke-transform --features call_graph transform::tests -- --nocapture` passed with `5 passed`; `cargo test -p ploke-db --features call_graph call_graph_queries -- --nocapture` passed with `9 passed`; `cargo test -p ploke-rag --features call_graph call_context_collection_attaches_outgoing_call_payloads -- --nocapture` passed.
- Local hygiene: `cargo fmt --all`, `cargo check -p ploke-tui --features call_graph`, `git diff --check`, and `npx gitnexus detect-changes` passed; GitNexus reports low risk and 0 affected processes. Backup fixture review/regeneration remains approval-gated.

## 2026-06-24 15:01 UTC - stop during if-expression dynamic callee setup

- Branch/HEAD: `prototype1-parent-mwv-live-r16-dangling-symlink-surface-fix-20260615t003337z-gen0` at `3be9c78a Make typed type graph baseline`; active goal remains open and should not be marked complete or blocked.
- Partial new work: fixture-only setup landed in `tests/fixture_crates/fixture_call_graph/src/lib.rs` for `other_target`, `call_if_same_function_item`, and `call_if_ambiguous_function_item`. No parser enum, resolver behavior, transform projection, docs, or fail-first tests for this D05 slice have been completed yet.
- Verification boundary: the latest green focused/broad checks are from the completed single-path block slice above. No tests or formatting were rerun after appending the if-expression fixtures, so resume with fail-first parser tests before implementation.
- Recommended next implementation: add a conservative dynamic callee representation for if-branch path candidates, resolve only when all branches prove the same local target, record `Ambiguous` with no edge when branch targets differ, and keep unsupported/opaque branch expressions failing closed.
- Next commands: compute fixture spans with `rg -n -b -o -F`, add strict paranoid expectations, run the targeted `syn_parser --features call_graph call_sites` test through a sub-agent to confirm intended failure, then implement classifier/resolver/transform coverage.
- Process notes: no requested dev server or test session is intentionally running. Continue to run GitNexus impact before symbol edits, tests through sub-agents, and do not touch backup fixtures without explicit approval; `docs/testing/BACKUP_DB_FIXTURES.md` review remains overdue from 2026-06-12.

## 2026-06-24 15:10 UTC - if-expression dynamic callee slice completed

- Changed: added `DynamicCallCallee::IfBranchPaths { paths }`, strict fixture coverage for `call_if_same_function_item` and `call_if_ambiguous_function_item`, and conservative parser classification for two path-valued if branches with local-shadowing guards.
- Resolver/projection: if all supported branch paths prove the same local function, resolver emits one `CallRelation::DynamicFunction` and `Resolved(LocalExact)`; branch paths proving different local functions emit `Ambiguous` with no edge. Transform persists the uniform path only when all branch paths match; divergent branch paths persist null `call_site.path`.
- Verified through sub-agents: fail-first targeted parser tests first failed with `Other` vs `IfBranchPaths`, then passed; `cargo test -p syn_parser --features call_graph call_sites -- --nocapture` passed with `131 passed`; `cargo test -p ploke-transform --features call_graph transform::tests -- --nocapture` passed with `5 passed`; `cargo test -p ploke-db --features call_graph call_graph_queries -- --nocapture` passed with `9 passed`; `cargo test -p ploke-rag --features call_graph call_context_collection_attaches_outgoing_call_payloads -- --nocapture` passed with `1 passed`; `cargo check -p ploke-tui --features call_graph` passed.
- Local hygiene: `cargo fmt --all`, `git diff --check`, and `npx gitnexus detect-changes` passed; GitNexus reports low risk and 0 affected processes. Active goal remains open.
- Next good slices: match-expression dynamic callees, opaque/unsupported if-branch fixtures, broader Fn/FnOnce value flow, or closure/async owner modeling. Backup fixture review/regeneration remains approval-gated.

## 2026-06-24 15:16 UTC - match-expression dynamic callee slice completed

- Changed: added `DynamicCallCallee::MatchArmPaths { paths }`, fixture coverage for `call_match_same_function_item` and `call_match_ambiguous_function_item`, and conservative parser classification for path-valued match arms with the same local-shadowing guard used by if branches.
- Resolver/projection: match arms reuse the branch-path dynamic resolver. Same proven local target emits one `CallRelation::DynamicFunction` plus `Resolved(LocalExact)`; different proven local targets emit `Ambiguous` with no edge. Transform persists a uniform path only when all arm paths match.
- Verified through sub-agents: fail-first targeted parser tests first failed with `Other` vs `MatchArmPaths`, then passed; `cargo test -p syn_parser --features call_graph call_sites -- --nocapture` passed with `133 passed`; `cargo test -p ploke-transform --features call_graph transform::tests -- --nocapture` passed with `5 passed`; `cargo test -p ploke-db --features call_graph call_graph_queries -- --nocapture` passed with `9 passed`; `cargo test -p ploke-rag --features call_graph call_context_collection_attaches_outgoing_call_payloads -- --nocapture` passed with `1 passed`; `cargo check -p ploke-tui --features call_graph` passed.
- Local hygiene: `cargo fmt --all`, `git diff --check`, and `npx gitnexus detect-changes` passed; GitNexus reports low risk and 0 affected processes. Active goal remains open.
- Next good slices: explicit unsupported/opaque if/match branch fixtures, broader Fn/FnOnce value flow, closure/async owner modeling, or remaining call-context/proof-fact polish. Backup fixture review/regeneration remains approval-gated.

## 2026-06-24 15:21 UTC - guarded match dynamic callee fail-closed slice completed

- Changed: added `fixture_call_graph::call_match_guarded_function_item` and paranoid coverage proving guarded match-arm dynamic callees remain `DynamicCallCallee::Other` with `Unsupported` status and no semantic edge.
- Guardrail: `match_arm_paths` now rejects any guarded arm before admitting exact `MatchArmPaths`, preventing guarded match semantics from being treated as a plain arm-path proof.
- Verified through sub-agents: targeted parser test first failed with `MatchArmPaths` vs `Other`, then passed; `cargo test -p syn_parser --features call_graph call_sites -- --nocapture` passed with `134 passed`; `cargo test -p ploke-transform --features call_graph transform::tests -- --nocapture` passed with `5 passed`; `cargo test -p ploke-db --features call_graph call_graph_queries -- --nocapture` passed with `9 passed`; `cargo test -p ploke-rag --features call_graph call_context_collection_attaches_outgoing_call_payloads -- --nocapture` passed with `1 passed`; `cargo check -p ploke-tui --features call_graph` passed.
- Local hygiene: `cargo fmt --all`, `git diff --check`, and `npx gitnexus detect-changes` passed; GitNexus reports low risk and 0 affected processes. Active goal remains open.
- Next good slices: non-path if/match branch fail-closed fixtures, broader Fn/FnOnce value flow, closure/async owner modeling, or remaining proof-fact/call-context polish. Backup fixture review/regeneration remains approval-gated.

## 2026-06-24 15:24 UTC - non-path if/match branch fail-closed coverage completed

- Changed: added `fixture_call_graph::call_if_closure_branch` and `fixture_call_graph::call_match_closure_arm` plus paranoid coverage proving non-path branch/arm dynamic callees remain `DynamicCallCallee::Other`, `Unsupported`, and no-edge.
- Implementation note: no production classifier change was needed; existing `branch_path` rejection for closure literals already enforced the fail-closed boundary.
- Verified through sub-agents: both targeted parser tests passed; `cargo test -p syn_parser --features call_graph call_sites -- --nocapture` passed with `136 passed`; `cargo test -p ploke-transform --features call_graph transform::tests -- --nocapture` passed with `5 passed`; `cargo test -p ploke-db --features call_graph call_graph_queries -- --nocapture` passed with `9 passed`; `cargo test -p ploke-rag --features call_graph call_context_collection_attaches_outgoing_call_payloads -- --nocapture` passed with `1 passed`; `cargo check -p ploke-tui --features call_graph` passed.
- Local hygiene: `cargo fmt --all`, `git diff --check`, and `npx gitnexus detect-changes` passed; GitNexus reports low risk and 0 affected processes. Active goal remains open.
- Next good slices: nested if/match fail-closed fixtures, broader Fn/FnOnce value flow, closure/async owner modeling, or remaining proof-fact/call-context polish. Backup fixture review/regeneration remains approval-gated.
