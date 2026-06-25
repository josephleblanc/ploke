# Handoff Changelog

Active running handoff notes for Codex/user continuity. Keep this file under 120 lines or 10 entries; archive to `docs/archive/agents/handoffs/` when it grows.

Previous archives:
- `docs/archive/agents/handoffs/2026-06-24_call-graph-progress.md`
- `docs/archive/agents/handoffs/2026-06-24_call-graph-slices.md`
- `docs/archive/agents/handoffs/2026-06-24_call-graph-dynamic-branch-slices.md`
- `docs/archive/agents/handoffs/2026-06-24_call-graph-late-parser-slices.md`
- `docs/archive/agents/handoffs/2026-06-24_call-graph-parser-fnflow-slices.md`
- `docs/archive/agents/handoffs/2026-06-24_call-graph-reference-receiver-slices.md`
- `docs/archive/agents/handoffs/2026-06-24_call-graph-trait-dispatch-slices.md`

## 2026-06-24 20:09 UTC - stop checkpoint after trait-dispatch slices

- Branch/HEAD: `prototype1-parent-mwv-live-r16-dangling-symlink-surface-fix-20260615t003337z-gen0` at `3be9c78a Make typed type graph baseline`; active autonomous call-graph goal remains open.
- Archive: moved the prior 10-entry active changelog to `docs/archive/agents/handoffs/2026-06-24_call-graph-trait-dispatch-slices.md` before appending this stop checkpoint.
- Current state: no new implementation after the `20:06 UTC` transitive blanket-bound proof slice. The dirty working tree still contains the broader call-graph stack, DB/RAG/TUI integration files, fixture/doc updates, and the new active/archive handoff files.
- Last known green verification from the previous slice: parser call-site focused suite `197 passed`; transform call-graph projection `5 passed`; DB call-graph queries `9 passed`; RAG call-context test `1 passed`; `cargo check -p ploke-tui --features call_graph` passed.
- Closeout verification: checked branch/status, recent commits, active goal, handoff size, and running Rust processes. No `cargo`, `rustc`, or `rustdoc` processes were running.
- Active gate: `CALL_GRAPH_GATE:db-projection` remains in force. Keep call-graph DB projection behind `--features call_graph` until backup fixtures are reviewed/regenerated; do not loosen backup import semantics without explicit approval.
- Next implementation: start with a small RED fixture/test, then implement using nearby parser/resolver/transform patterns. Recommended first slice is bounded `Fn`/`FnOnce`/function-pointer exact-proof work if a real target can be proven without inventing dynamic edges.
- Alternative next slices: multi-step concrete trait-object proof, broader imported trait scope forms, closure/async nested-owner modeling, or richer blanket-bound shapes beyond exact recursive one-parameter local impls.
- Required discipline next session: run GitNexus impact before editing indexed symbols; use sub-agents for cargo tests/checks; preserve strict fail-closed resolver behavior; keep `call_graph` gated; finish each slice with docs plus `git diff --check`, `npx gitnexus detect-changes`, `git status --short --branch`, and a Rust-process check.

## 2026-06-24 20:16 UTC - imported function-item binding proof

- Branch/HEAD: same branch at `3be9c78a`; active autonomous call-graph goal remains open.
- Changed: added `call_imported_function_item_binding()` with `let f = imported_alias; f()` and a strict paranoid expectation that records `InitializedValueBinding { path: ["f"], init_path: ["imported_alias"] }`.
- Result: no production resolver change was needed. Existing initialized value-binding resolution plus import backlinks resolve the call to `import_targets::imported_target` as `CallRelation::Function` with `Resolved(LocalExact)`.
- Guardrails: GitNexus could not find private/test fixture symbols; indexed boundary checks for `resolve_path_call` and `extract_body_call_sites` were LOW risk with no affected processes.
- Verified: focused `cargo test -p syn_parser --features call_graph imported_function_item_binding -- --nocapture` passed; standard bundle passed with parser `198 passed`, transform `5 passed`, DB call-graph queries `9 passed`, RAG call-context `1 passed`, and `cargo check -p ploke-tui --features call_graph`.
- Docs: call-graph README and coverage matrix now record 198 paranoid call-site cases and the `src/lib.rs:1119` imported function-item binding row.
- Next implementation: continue with a true behavior-expanding slice, likely multi-step concrete trait-object proof, broader imported trait scope forms, closure/async nested-owner modeling, or a bounded `Fn`/`FnOnce` exact-proof case if one can be proven without inventing dynamic edges.

## 2026-06-24 20:22 UTC - reference-alias trait-object proof

- Branch/HEAD: same branch at `3be9c78a`; active autonomous call-graph goal remains open.
- Changed: added `call_reference_alias_trait_object_binding_method()` with `let source = TraitDispatchTarget; let alias = &source; let value: &dyn LocalDispatchTrait = alias; value.trait_value()`.
- Evidence: focused test first failed because `value` was classified as `TypedLocalBinding { type_path: ["LocalDispatchTrait"] }`, then passed after parser local-binding proof started admitting trait-object initializers from visible `Referenced` local bindings.
- Implementation: new `trait_object_init_path` helper keeps direct `&path` proof and adds only the path-to-`LocalBindingProof::Referenced` case. Params, typed locals without concrete reference proof, arrays, constructed values, and opaque bindings still fail closed.
- Guardrails: GitNexus impact for `extract_body_call_sites`, `classify_method_receiver`, and `resolve_method_call` was LOW with no affected processes.
- Verified: focused `cargo test -p syn_parser --features call_graph reference_alias_trait_object -- --nocapture` passed; standard bundle passed with parser `199 passed`, transform `5 passed`, DB call-graph queries `9 passed`, RAG call-context `1 passed`, and `cargo check -p ploke-tui --features call_graph`.
- Docs: call-graph README and coverage matrix now record 199 paranoid call-site cases and the `src/lib.rs:1126` reference-alias trait-object row.
- Next implementation: continue with broader imported trait scope forms, closure/async nested-owner modeling, richer blanket-bound shapes, or another bounded exact-proof `Fn`/`FnOnce` case. Keep `call_graph` gated and avoid backup fixtures without explicit approval.

## 2026-06-24 20:28 UTC - reference-chain trait-object proof

- Branch/HEAD: same branch at `3be9c78a`; active autonomous call-graph goal remains open.
- Changed: added `call_reference_chain_trait_object_binding_method()` with `let first = &source; let second = first; let value: &dyn LocalDispatchTrait = second; value.trait_value()`.
- Evidence: focused test first failed because `value` remained `TypedLocalBinding { type_path: ["LocalDispatchTrait"] }`, then passed after untyped locals initialized from a visible `Referenced` binding preserve that concrete reference proof.
- Implementation: new `referenced_alias_path` helper only accepts a single-segment local path to `LocalBindingProof::Referenced`; params, non-single paths, typed/untyped opaque locals, arrays, constructed values, and non-reference proofs still fail closed.
- Guardrails: GitNexus impact for `extract_body_call_sites`, `classify_method_receiver`, and `resolve_method_call` was LOW with no affected processes.
- Verified: focused `cargo test -p syn_parser --features call_graph reference_chain_trait_object -- --nocapture` passed; standard bundle passed with parser `200 passed`, transform `5 passed`, DB call-graph queries `9 passed`, RAG call-context `1 passed`, and `cargo check -p ploke-tui --features call_graph`.
- Docs: call-graph README and coverage matrix now record 200 paranoid call-site cases and the `src/lib.rs:1134` reference-chain trait-object row.
- Next implementation: continue with broader imported trait scope forms, closure/async nested-owner modeling, richer blanket-bound shapes, or another bounded exact-proof `Fn`/`FnOnce` case. Keep `call_graph` gated and avoid backup fixtures without explicit approval.

## 2026-06-24 20:30 UTC - stop checkpoint after reference-chain slice

- Branch/HEAD: same branch at `3be9c78a`; active autonomous call-graph goal remains open and should resume from the existing dirty working tree.
- Current state: no new implementation after the `20:28 UTC` reference-chain trait-object proof. The broad call-graph stack remains unstaged and dirty; `docs/active/agents/HANDOFF_CHANGELOG.md` is intentionally updated for this checkpoint.
- Closeout verification: `git status --short --branch`, `git log --oneline -5`, handoff line count, active goal state, and Rust-process check were reviewed; no `cargo`, `rustc`, or `rustdoc` processes were running.
- Last known green verification: parser call-site focused suite `200 passed`; transform call-graph projection `5 passed`; DB call-graph queries `9 passed`; RAG call-context `1 passed`; `cargo check -p ploke-tui --features call_graph` passed.
- Next implementation: prefer a small RED fixture/test around broader imported trait scope forms or a bounded exact `Fn`/`FnOnce` proof if a real target can be proven. Defer closure/async nested-owner modeling until ready for a larger design slice.

## 2026-06-24 20:45 UTC - import proofs and generic self-type guard

- Branch/HEAD: same branch at `3be9c78a`; active autonomous call-graph goal remains open.
- Changed: added grouped-import trait method proof and local re-exported trait associated-function proof in `fixture_call_graph`; both passed without production resolver changes.
- Changed: added `call_constrained_generic_self_trait_method` regression for `impl<T: GenericWrapperBound> ConstrainedGenericSelfTrait for GenericWrapper<T>`, first RED because erased nominal `GenericWrapper<_>` matched as `Resolved(LocalExact)`.
- Implementation: `resolve_trait_impl_instance_method` now uses `exact_trait_impl_self_type_applies`, allowing direct self-type equality only for non-generic/non-where trait impls. Constrained generic self-type impls must go through existing exact blanket-bound proof or fail closed.
- Verified: focused grouped-import and re-exported trait associated-function tests passed; constrained generic self-type regression failed before the resolver guard and passed after it. Standard bundle passed with parser `203 passed`, transform `5 passed`, DB call-graph queries `9 passed`, RAG call-context `1 passed`, and `cargo check -p ploke-tui --features call_graph`.
- Hygiene: `cargo fmt --all` passed; `git diff --check` passed; `npx gitnexus detect-changes` reported low risk, `0` affected processes; no `cargo`, `rustc`, or `rustdoc` processes were running.
- Docs: call-graph README and coverage matrix now record 203 paranoid call-site cases, grouped trait import, re-exported trait associated function, and the constrained generic self-type fail-closed rule.
- Next implementation: prefer a behavior-expanding slice with exact proof, likely richer generic trait impl matching only if receiver generic arguments can be modeled safely, or move to closure/async nested-owner design when ready.

## 2026-06-24 20:51 UTC - stop checkpoint during generic-argument proof

- Branch/HEAD: same branch at `3be9c78a`; active autonomous call-graph goal remains open and should resume from the existing dirty working tree.
- Completed slice: exact generic-argument proof for `GenericWrapper<GenericBoundValue>` calling `value.constrained_generic_self_value()`. The paranoid test expects `ResolvedMethodLocalExact` against the impl span `(25164, 25318)`.
- Last evidence before production edits: focused `cargo test -p syn_parser --features call_graph constrained_generic_self_trait_method -- --nocapture` failed RED because the resolver returned `Unsupported`.
- Implementation: `resolve_type_use_method` falls back to `resolve_type_use_trait_impl_method` for exact one-target type-use receivers; generic self-type matching now requires concrete receiver arguments and exact proof that every generic bound is satisfied.
- Verified: focused generic proof passed. Standard bundle passed with parser `203 passed`, transform `5 passed`, DB call-graph queries `9 passed`, RAG call-context `1 passed`, and `cargo check -p ploke-tui --features call_graph`.
- Hygiene: `cargo fmt --all` passed; `git diff --check` passed; `npx gitnexus detect-changes` reported low risk, `0` affected processes.
- Guardrails: generic matching remains strict; erased `GenericWrapper<_>` matches must not resolve without concrete receiver arguments and proven bounds. Keep `CALL_GRAPH_GATE:db-projection` active and do not touch backup fixtures without approval.
- Closeout check: `git status --short --branch`, recent commits, active goal, handoff size, and Rust-process check were reviewed; no `cargo`, `rustc`, or `rustdoc` processes were running.

## 2026-06-24 21:18 UTC - fixture-backed DB call graph contracts

- Branch/HEAD: same branch at `71e6a7a7 Implement gated call graph integration`; active autonomous call-graph goal remains open.
- Changed: added `crates/ploke-db/tests/unit/call_graph_fixture_queries.rs` and wired it under `#[cfg(feature = "call_graph")]`.
- Implementation: new DB tests use fresh `fixture_call_graph` and `fixture_nodes` parser output, create an in-memory Cozo schema, run `transform_parsed_graph`, then assert persisted call-context rows for resolved path/method/associated-function/constructor/dynamic calls, imported/re-exported associated functions, trait-dispatch calls, borrowed/dereferenced receiver rows, path/method/await/try result receivers, tuple-field method/dynamic rows, macro statuses, enum variant constructors, and external/ambiguous/unsupported statuses without local targets. They also project resolved, external, and mixed multi-row fixture contexts into proof facts with real source provenance.
- Downstream RAG: added fresh `fixture_call_graph` coverage for `RagService::collect_call_context`, asserting the real `call_try_result_instance_method` rows for unsupported `Ok(...)`, resolved `try_local_assoc()`, and the resolved try-result method receiver are preserved in `CallContextInfo`.
- Verified: focused `cargo test -p ploke-db --features call_graph call_graph_fixture_queries -- --nocapture` passed with `20 passed`; broader `cargo test -p ploke-db --features call_graph call_graph_ -- --nocapture` passed with `29 passed`; `cargo test -p ploke-rag --features call_graph call_context_collection -- --nocapture` passed with `2 passed`.
- Next implementation: continue downstream coverage before adding more parser-only cases; useful next row is fixture-backed TUI rendering.

## 2026-06-24 21:30 UTC - TUI call-context rendering coverage

- Branch/HEAD: same branch at `71e6a7a7`; active autonomous call-graph goal remains open.
- Changed: added `format_call_context_block_renders_fixture_derived_rows` in `crates/ploke-tui/src/rag/context.rs`, `expanded_rag_part_displays_call_context_details` in `crates/ploke-tui/src/app/view/components/context_plan_overlay.rs`, and call-context carrier assertions in `crates/ploke-tui/tests/integration/tool_io_roundtrip.rs`, covering the downstream payload shape from `fixture_call_graph::call_try_result_instance_method`: unsupported `Ok(...)`, resolved `try_local_assoc()`, and resolved `try_local_assoc()?.instance_value()`.
- Verified: `cargo test -p ploke-tui --features call_graph call_context -- --nocapture` passed with `3 passed`; `cargo test -p ploke-tui --features call_graph tool_io_roundtrip -- --nocapture` passed with `3 passed`; dependent `cargo test -p ploke-rag --features call_graph call_context_collection -- --nocapture` passed with `2 passed`; broad DB call graph filter `cargo test -p ploke-db --features call_graph call_graph_ -- --nocapture` passed with `29 passed`.
- Hygiene: `cargo fmt --all -- --check` passed. GitNexus did not index the private formatter/test symbols; `build_rows` and `build_display_items` both reported LOW upstream impact with no affected processes.
- Next implementation: continue downstream, likely broader TUI prompt/context-plan integration with real fixture-derived call-context rows or proof-fact consumers before returning to parser-only resolver rows.

## 2026-06-24 22:13 UTC - function-pointer parameter cast shape

- Branch/HEAD: same branch at `71e6a7a7`; active autonomous call-graph goal remains open.
- Changed: `(f as fn() -> i32)()` where `f: fn() -> i32` is an opaque parameter now records `DynamicCallCallee::FnPointerCastLocalBinding { path: ["f"] }`, persists the dynamic `path`, and still fails closed as `Unsupported` with no semantic target.
- Verified: focused parser `cargo test -p syn_parser --features call_graph function_pointer_param_cast -- --nocapture` passed; focused DB `cargo test -p ploke-db --features call_graph function_pointer_param_cast -- --nocapture` passed; transform projection `cargo test -p ploke-transform --features call_graph transform::tests -- --nocapture` passed with `5 passed`.
- Broad verification: `cargo test -p syn_parser --features call_graph call_sites -- --nocapture` passed with `203 passed`; `cargo test -p ploke-db --features call_graph call_graph_ -- --nocapture` passed with `30 passed`.
- Next implementation: continue exact function-pointer/Fn-shape work only where source-to-target proof exists; opaque function-pointer parameters should remain fail-closed.

## 2026-06-24 22:25 UTC - target-centered DB caller helper

- Branch/HEAD: same branch at `71e6a7a7`; active autonomous call-graph goal remains open.
- Changed: added feature-gated `CallCallerRow` and `Database::callers_for_target(...)` in `ploke-db`, returning incoming call sites with their status row and matching semantic target edge.
- Coverage: added a synthetic DB helper test that filters out unrelated targets and preserves path/method receiver payloads, plus a fresh `fixture_call_graph` test proving `local_target` can be found from both a path caller and a dynamic-function caller.
- Verified: focused `cargo test -p ploke-db --features call_graph callers_for_target -- --nocapture` passed with `2 passed`; fixture module `cargo test -p ploke-db --features call_graph call_graph_fixture_queries -- --nocapture` passed with `22 passed`; broad DB filter `cargo test -p ploke-db --features call_graph call_graph_ -- --nocapture` passed with `32 passed`.
- Next implementation: keep focus on DB/RAG/proof consumers; useful next DB slice is caller-oriented RAG payload integration or proof/RAG queries that use `callers_for_target` for target-centered context expansion.

## 2026-06-24 22:35 UTC - RAG incoming caller expansion

- Branch/HEAD: same branch at `71e6a7a7`; active autonomous call-graph goal remains open.
- Changed: `RagService` now expands call-graph-enabled retrieval hits from a callee target to materializable caller owners through `Database::callers_for_target(...)` before optional reranking and context assembly. Expanded caller parts then use existing outgoing `ContextPart.call_context` payloads.
- Coverage: added fresh `fixture_call_graph` RAG coverage seeding `try_local_assoc` and proving `call_try_result_instance_method` is materialized with outgoing call context pointing back to the seed target.
- Verified: focused `cargo test -p ploke-rag --features call_graph call_context -- --nocapture` passed with `3 passed`; dependent broad filters `cargo test -p ploke-db --features call_graph call_graph_ -- --nocapture` passed with `32 passed`, and `cargo test -p ploke-tui --features call_graph call_context -- --nocapture` passed with TUI `3 passed`.
- Broad residual: full `cargo test -p ploke-rag --features call_graph -- --nocapture` remains red with `38 passed / 14 failed`. Failures are stale backup fixtures missing `call_relation` plus pre-existing search/snippet fixture failures such as missing `use_all_const_static`; do not weaken call-graph relation checks to hide this.
- Next implementation: consider caller-oriented proof/RAG surfaces that consume incoming expansion, or fixture review/regeneration when explicitly approved.

## 2026-06-24 22:39 UTC - target-centered proof projection

- Branch/HEAD: same branch at `71e6a7a7`; active autonomous call-graph goal remains open.
- Changed: added feature-gated `Database::call_proof_facts_for_target(...)` and `Database::project_call_proof_facts_for_target(...)`, adapting `CallCallerRow` into the existing strict call proof fact builder path.
- Coverage: added fresh `fixture_call_graph` DB proof coverage seeding target `try_local_assoc`; projection stores the incoming caller edge/provenance and does not include the caller owner's unrelated unsupported `Ok(...)` blocker.
- Verified: focused `cargo test -p ploke-db --features call_graph target_centered_call_proof -- --nocapture` passed with `1 passed`; broad `cargo test -p ploke-db --features call_graph call_graph_ -- --nocapture` passed with `33 passed`; `cargo test -p ploke-db --features call_graph --test proof_graph_store -- --nocapture` passed with `10 passed`.
- Next implementation: continue with caller-oriented proof/RAG consumers or return to parser/resolver exact-proof gaps only when a strict local proof is available.

## 2026-06-24 22:46 UTC - target-centered proof strictness coverage

- Branch/HEAD: same branch at `71e6a7a7`; active autonomous call-graph goal remains open.
- Changed: added synthetic `ploke-db` coverage for `project_call_proof_facts_for_target(...)` when a target has mixed incoming rows, including a resolved caller and an inconsistent unresolved caller that still carries a local `call_relation`.
- Invariant: target-centered proof projection must reject non-resolved local target edges and must not store partial proof facts after rejection.
- Verified: focused `cargo test -p ploke-db --features call_graph target_centered_proof_projection_rejects_non_resolved_local_targets -- --nocapture` passed with `1 passed`; broader `cargo test -p ploke-db --features call_graph call_graph_ -- --nocapture` passed with `34 passed`.
- Next implementation: continue strengthening DB/RAG/proof consumers before returning to parser/resolver exact-proof gaps.

## 2026-06-24 22:51 UTC - public RAG call-context assembly coverage

- Branch/HEAD: same branch at `71e6a7a7`; active autonomous call-graph goal remains open.
- Changed: added `call_context_sparse_get_context_expands_target_hits_to_fixture_callers`, proving the public `RagService::get_context(...)` path expands a sparse `try_local_assoc` callee hit into the incoming `call_try_result_instance_method` caller owner and preserves the outgoing call-context edge through final context assembly.
- Guardrail: the test disables type-context expansion and first asserts sparse BM25 seeds only the callee target, so caller materialization is attributable to call-graph expansion.
- Verified: focused `cargo test -p ploke-rag --features call_graph sparse_get_context_expands_target_hits_to_fixture_callers -- --nocapture` passed with `1 passed`; broader `cargo test -p ploke-rag --features call_graph call_context -- --nocapture` passed with `4 passed`.
- Next implementation: continue downstream DB/RAG/proof verification where it proves public consumers, then move back to parser/resolver exact-proof gaps when a strict local proof is available.

## 2026-06-24 22:56 UTC - DB fixture raw identifier/drop coverage

- Branch/HEAD: same branch at `71e6a7a7`; active autonomous call-graph goal remains open.
- Changed: added fixture-backed `ploke-db` assertions for already-supported raw identifier path/method calls and explicit inherent `drop(self)` method calls. These prove fresh parser -> transform -> DB projection preserves raw spelling, receiver payloads, resolved statuses, and target edge kinds.
- Test helper note: raw identifier lookup uses new parameterized fixture-only helpers so `r#...` names are not interpolated into CozoScript.
- Verified: focused `cargo test -p ploke-db --features call_graph raw_identifier_calls -- --nocapture` passed with `1 passed`; focused `cargo test -p ploke-db --features call_graph explicit_drop_method_call -- --nocapture` passed with `1 passed`; fixture module `cargo test -p ploke-db --features call_graph call_graph_fixture_queries -- --nocapture` passed with `25 passed`; broad DB filter `cargo test -p ploke-db --features call_graph call_graph_ -- --nocapture` passed with `36 passed`.
- Next implementation: continue filling DB fixture-backed coverage for already-green parser/resolver rows where persisted shape has not been asserted, or move to parser/resolver exact-proof gaps only where strict local proof exists.

## 2026-06-24 23:00 UTC - DB fixture prelude drop shadowing coverage

- Branch/HEAD: same branch at `71e6a7a7`; active autonomous call-graph goal remains open.
- Changed: added fixture-backed `ploke-db` assertions for prelude `drop(value)` vs module-local `drop(1)` shadowing. The persisted DB contract now proves unshadowed prelude `drop` is `External` with no target edge, while local shadowing resolves to the local function before prelude classification.
- Verified: focused `cargo test -p ploke-db --features call_graph prelude_drop_shadowing -- --nocapture` passed with `1 passed`; fixture module `cargo test -p ploke-db --features call_graph call_graph_fixture_queries -- --nocapture` passed with `26 passed`; broad DB filter `cargo test -p ploke-db --features call_graph call_graph_ -- --nocapture` passed with `37 passed`.
- Next implementation: continue DB fixture-backed coverage for parser-green external/local-shadow method rows such as literal `.to_string()`, typed `Vec::len`, and shadowed local `Vec::len`, or move to parser/resolver exact-proof gaps when strict local proof exists.

## 2026-06-24 23:03 UTC - DB fixture external/local-shadow method coverage

- Branch/HEAD: same branch at `71e6a7a7`; active autonomous call-graph goal remains open.
- Changed: added fixture-backed `ploke-db` assertions for literal `"literal".to_string()`, `Vec::new()`, unshadowed typed `Vec::len`, and local-shadowed `Vec::len`. The persisted DB contract now proves the external/prelude method rows carry no semantic target edge while local shadowing resolves to the local inherent method.
- Verified: focused `cargo test -p ploke-db --features call_graph external_and_shadowed_method_calls -- --nocapture` passed with `1 passed`; fixture module `cargo test -p ploke-db --features call_graph call_graph_fixture_queries -- --nocapture` passed with `27 passed`; broad DB filter `cargo test -p ploke-db --features call_graph call_graph_ -- --nocapture` passed with `38 passed`.
- Next implementation: continue DB fixture-backed coverage for parser-green rows where persisted shape is still uncovered, or move to parser/resolver exact-proof gaps only when strict local proof exists.

## 2026-06-24 23:11 UTC - DB fixture dynamic callee shape coverage

- Branch/HEAD: same branch at `71e6a7a7`; active autonomous call-graph goal remains open.
- Changed: added fixture-backed `ploke-db` assertions for parser-green dynamic callee rows that previously lacked persisted API coverage. Resolved rows now cover function-pointer cast path/binding calls, dereferenced initialized function pointers, block callees, same-target if/match branch callees, named-field function callees, and indexed initialized array callees. Targetless rows now cover ambiguous if/match dynamic calls, opaque field/indexed calls, generic `FnOnce` parenthesized dynamic calls, and boxed `dyn Fn` dynamic calls while preserving the `Box::new` external row.
- Invariant: ambiguous, unsupported, boxed, and generic dynamic calls must not fabricate local `call_relation` edges; only exact local proofs get `DynamicFunction` edges.
- Verified: focused `cargo test -p ploke-db --features call_graph resolved_dynamic_function_shapes -- --nocapture` passed with `1 passed`; focused `cargo test -p ploke-db --features call_graph targetless_dynamic_failures -- --nocapture` passed with `1 passed`; fixture module `cargo test -p ploke-db --features call_graph call_graph_fixture_queries -- --nocapture` passed with `30 passed`; broad DB filter `cargo test -p ploke-db --features call_graph call_graph_ -- --nocapture` passed with `41 passed`.
- Next implementation: continue DB fixture-backed coverage for parser-green method/path rows still uncovered in persistence, then return to parser/resolver exact-proof gaps only where strict local proof exists.

## 2026-06-24 23:15 UTC - DB fixture alias and binding call coverage

- Branch/HEAD: same branch at `71e6a7a7`; active autonomous call-graph goal remains open.
- Changed: added fixture-backed `ploke-db` assertions for Rust type-alias associated-function calls, method-as-associated-function calls, local and imported function-item binding calls, typed function-pointer binding calls, and a shadowed local closure binding. These prove the persisted call-site path, local target relation kind, and targetless unsupported binding status survive parser -> transform -> DB projection.
- Invariant: value bindings that are closures or otherwise not exact local function items remain `Unsupported` with no fabricated local target edge; exact function-item and typed function-pointer bindings resolve only when their initializer proof reaches one local function.
- Verified: focused `cargo test -p ploke-db --features call_graph type_alias_associated_function_calls -- --nocapture` passed with `1 passed`; focused `cargo test -p ploke-db --features call_graph function_item_binding_calls -- --nocapture` passed with `1 passed`; fixture module `cargo test -p ploke-db --features call_graph call_graph_fixture_queries -- --nocapture` passed with `32 passed`; broad DB filter `cargo test -p ploke-db --features call_graph call_graph_ -- --nocapture` passed with `43 passed`.
- Next implementation: continue DB fixture-backed coverage for parser-green trait/generic method rows or proof/RAG consumers, then return to parser/resolver exact-proof gaps only where strict local proof exists.

## 2026-06-24 23:22 UTC - DB fixture trait and generic method coverage

- Branch/HEAD: same branch at `71e6a7a7`; active autonomous call-graph goal remains open.
- Changed: added fixture-backed `ploke-db` assertions for generic-bound methods, trait-object declaration-target methods, imported/re-exported/grouped trait method resolution, and unconstrained/constrained/transitive blanket trait impl methods. These prove the persisted method receiver payload, resolved status, local target ID, and `Method` relation survive parser -> transform -> DB projection for parser-green semantic method rows.
- Test helper note: added a fixture-local impl-trait method lookup helper for blanket impls where the impl self type is generic and cannot be selected by concrete self-type name.
- Verified: focused `cargo test -p ploke-db --features call_graph generic_bound_and_trait_object_methods -- --nocapture` passed with `1 passed`; focused `cargo test -p ploke-db --features call_graph imported_trait_method_calls -- --nocapture` passed with `1 passed`; focused `cargo test -p ploke-db --features call_graph blanket_trait_method_calls -- --nocapture` passed with `1 passed`; fixture module `cargo test -p ploke-db --features call_graph call_graph_fixture_queries -- --nocapture` passed with `35 passed`; broad DB filter `cargo test -p ploke-db --features call_graph call_graph_ -- --nocapture` passed with `46 passed`.
- Next implementation: continue DB fixture-backed coverage for remaining parser-green owner contexts such as trait/default method bodies and borrowed/deref parameter variants, or move to proof/RAG consumers; return to parser/resolver exact-proof gaps only where strict local proof exists.

## 2026-06-24 23:26 UTC - DB fixture method-owner and receiver variant coverage

- Branch/HEAD: same branch at `71e6a7a7`; active autonomous call-graph goal remains open.
- Changed: expanded fixture-backed `ploke-db` assertions for borrowed/reference/dereferenced receiver variants and added method-body owner coverage for trait default methods, trait impl methods, trait default self-method calls, and same-trait associated-function calls. These prove `call_context_for_owner` works for method owner IDs as well as function owner IDs.
- Invariant: method-body owner call rows must keep their actual method owner ID and exact local target edge; borrowed/deref/reference receiver variants must preserve their specific receiver classifier instead of collapsing to an ambiguous local binding.
- Verified: focused `cargo test -p ploke-db --features call_graph borrowed_and_dereferenced_method_receivers -- --nocapture` passed with `1 passed`; focused `cargo test -p ploke-db --features call_graph method_body_owner_calls -- --nocapture` passed with `1 passed`; fixture module `cargo test -p ploke-db --features call_graph call_graph_fixture_queries -- --nocapture` passed with `36 passed`; broad DB filter `cargo test -p ploke-db --features call_graph call_graph_ -- --nocapture` passed with `47 passed`.
- Next implementation: continue DB fixture-backed coverage for any remaining parser-green rows, then prioritize proof/RAG/TUI consumers and backup fixture gate review before considering the call-graph plan complete.

## 2026-06-24 23:31 UTC - DB fixture target-centered method caller coverage

- Branch/HEAD: same branch at `71e6a7a7`; active autonomous call-graph goal remains open.
- Changed: added fixture-backed `ploke-db` assertions for `Database::callers_for_target(...)` over method and associated-function targets. The test now proves incoming caller rows preserve method receiver payloads, path-call payloads, owner IDs for both function and method owners, resolved status, and target relation kinds for `Method` and `AssociatedFunction` edges.
- Invariant: target-centered caller queries must only return rows whose `call_relation.target_id` matches the requested target and must not collapse associated-function path calls into ordinary method call rows.
- Verified: focused `cargo test -p ploke-db --features call_graph method_and_associated_callers -- --nocapture` passed with `1 passed`; fixture module `cargo test -p ploke-db --features call_graph call_graph_fixture_queries -- --nocapture` passed with `37 passed`; broad DB filter `cargo test -p ploke-db --features call_graph call_graph_ -- --nocapture` passed with `48 passed`.
- Next implementation: continue DB fixture-backed coverage where it still informs downstream caller expansion, then prioritize proof/RAG/TUI consumers and backup fixture gate review before considering the call-graph plan complete.

## 2026-06-24 23:39 UTC - RAG method-target caller expansion coverage

- Branch/HEAD: same branch at `71e6a7a7`; active autonomous call-graph goal remains open.
- Changed: added `call_context_expansion_adds_incoming_fixture_method_callers` in `ploke-rag`, proving target-centered RAG expansion can seed from the real `LocalAssoc::instance_value` method target and materialize both a method-call owner and an associated-function path-call owner. The test then collects outgoing call context from those materialized owners and verifies the payload points back to the seeded method target with `Method` and `AssociatedFunction` relation kinds.
- Test config note: this fixture target has many incoming callers, so the test raises only its local `max_owner_hits` and `max_caller_hits` caps to avoid proving truncation behavior instead of method-target expansion.
- Verified: focused `cargo test -p ploke-rag --features call_graph incoming_fixture_method_callers -- --nocapture` passed with `1 passed`; broader `cargo test -p ploke-rag --features call_graph call_context -- --nocapture` passed with `5 passed`; dependent broad DB filter `cargo test -p ploke-db --features call_graph call_graph_ -- --nocapture` passed with `48 passed`.
- Next implementation: continue downstream public `get_context`/TUI proof where method-target caller expansion should surface, then return to parser/resolver gaps only when a strict proof case is available.

## 2026-06-24 23:48 UTC - Public RAG method-target caller expansion coverage

- Branch/HEAD: same branch at `71e6a7a7`; active autonomous call-graph goal remains open.
- Changed: added `call_context_sparse_get_context_expands_method_target_hits_to_fixture_callers` in `ploke-rag`, mirroring the existing public sparse `get_context` function-target coverage for the real `LocalAssoc::instance_value` method target. The test first proves strict BM25 seeds `get_context` with the method definition, then asserts final context assembly materializes both `call_typed_local_instance_method` and `call_method_as_associated_function` with outgoing call-context edges back to the seeded method target.
- Test config note: the fixture method target has many incoming callers, so this public-path test uses local `max_owner_hits` and `max_caller_hits` caps of 64 to keep the assertion focused on method-target caller expansion rather than truncation ordering.
- Verified: focused `cargo test -p ploke-rag --features call_graph method_target_hits_to_fixture_callers -- --nocapture` passed with `1 passed`; broader `cargo test -p ploke-rag --features call_graph call_context -- --nocapture` passed with `6 passed`; dependent broad DB filter `cargo test -p ploke-db --features call_graph call_graph_ -- --nocapture` passed with `48 passed`.
- Next implementation: continue downstream TUI/proof/RAG consumer coverage and only return to parser/resolver behavior when a strict DB or public consumer contract exposes a missing case.

## 2026-06-24 23:59 UTC - DB fixture ordinary path import coverage

- Branch/HEAD: same branch at `71e6a7a7`; active autonomous call-graph goal remains open.
- Changed: added `fixture_context_reads_projected_path_resolution_forms` in `ploke-db`, proving parser-green ordinary path-call resolution forms survive parser -> transform -> DB projection. The fixture-backed test covers unqualified local calls, `self::` and `super::` paths, crate/self module paths, direct import aliases, glob imports, re-exports, and module aliases.
- Invariant: each row must keep the original path spelling, zero argument/generic counts, resolved `LocalExact` status, and a matching `Function` target edge; import/glob/re-export/module-alias calls must not collapse to the target's canonical path in the persisted call-site payload.
- Verified: focused `cargo test -p ploke-db --features call_graph path_resolution_forms -- --nocapture` passed with `1 passed`; fixture module `cargo test -p ploke-db --features call_graph call_graph_fixture_queries -- --nocapture` passed with `38 passed`; broad DB filter `cargo test -p ploke-db --features call_graph call_graph_ -- --nocapture` passed with `49 passed`.
- Next implementation: continue fixture-backed DB coverage where parser-green rows still lack persisted contracts, especially direct owner-context assertions for remaining associated-function and receiver forms before broadening downstream consumers.

## 2026-06-25 00:08 UTC - DB fixture associated-function owner coverage

- Branch/HEAD: same branch at `71e6a7a7`; active autonomous call-graph goal remains open.
- Changed: added `fixture_context_reads_projected_self_and_qualified_associated_function_calls` in `ploke-db`, proving owner-scoped DB context rows for inherent `Self::make()` inside `LocalAssoc::call_self_make` and qualified `<LocalAssoc>::make()` in `call_qualified_local_assoc_make`.
- Invariant: associated-function owner context must preserve the syntactic path payload (`Self::make` vs normalized `LocalAssoc::make`), zero argument/generic counts, resolved `LocalExact` status, and an `AssociatedFunction` edge whose target kind is `Method`; this must not be flattened into an ordinary function edge.
- Verified: focused `cargo test -p ploke-db --features call_graph self_and_qualified_associated_function_calls -- --nocapture` passed with `1 passed`; fixture module `cargo test -p ploke-db --features call_graph call_graph_fixture_queries -- --nocapture` passed with `39 passed`; broad DB filter `cargo test -p ploke-db --features call_graph call_graph_ -- --nocapture` passed with `50 passed`.
- Next implementation: continue DB fixture-backed owner-context coverage for remaining parser-green receiver/associated forms, then use those persisted contracts to broaden proof/RAG/TUI consumer assertions.

## 2026-06-25 00:14 UTC - DB fixture local trait associated-function owner coverage

- Branch/HEAD: same branch at `71e6a7a7`; active autonomous call-graph goal remains open.
- Changed: strengthened `fixture_context_reads_projected_imported_trait_associated_function_calls` in `ploke-db` with the direct fully qualified local `<TraitAssocFunctionTarget as LocalAssocFunctionTrait>::trait_make()` owner context, alongside the already-covered imported shorthand trait associated-function rows.
- Invariant: fully qualified local trait associated-function calls must persist as path-call rows with the normalized trait method path, resolved `LocalExact` status, and an `AssociatedFunction` edge to the trait method target. They must not be treated as ordinary functions or require an instance method receiver.
- Verified: focused `cargo test -p ploke-db --features call_graph imported_trait_associated_function_calls -- --nocapture` passed with `1 passed`; fixture module `cargo test -p ploke-db --features call_graph call_graph_fixture_queries -- --nocapture` passed with `39 passed`; broad DB filter `cargo test -p ploke-db --features call_graph call_graph_ -- --nocapture` passed with `50 passed`.
- Next implementation: continue from DB fixture-backed contracts toward downstream proof/RAG/TUI consumer assertions, using parser/resolver changes only when a strict persisted or public-consumer gap exposes them.

## 2026-06-25 00:24 UTC - Target-centered method proof projection coverage

- Branch/HEAD: same branch at `71e6a7a7`; active autonomous call-graph goal remains open.
- Changed: added `fixture_projection_stores_real_target_centered_method_call_proof_facts` in `ploke-db`, proving `project_call_proof_facts_for_target(...)` works for the real `LocalAssoc::instance_value` method target with multiple incoming callers. The test derives the expected proof fact count from `callers_for_target(...)`, then verifies resolved proof edges for both `call_typed_local_instance_method` and `call_method_as_associated_function`.
- Invariant: target-centered method projection must store only resolved proof edges to the seeded method target, preserve caller owner IDs and source provenance, include associated-function path-call callers as `AssociatedFunction` edges, and not pull unrelated unsupported owner calls into blocker proof facts.
- Verified: focused `cargo test -p ploke-db --features call_graph target_centered_method_call_proof -- --nocapture` passed with `1 passed`; fixture module `cargo test -p ploke-db --features call_graph call_graph_fixture_queries -- --nocapture` passed with `40 passed`; broad DB filter `cargo test -p ploke-db --features call_graph call_graph_ -- --nocapture` passed with `51 passed`.
- Next implementation: continue downstream proof/RAG/TUI public-consumer assertions where target-centered method expansion should surface, while preserving the `call_graph` gate until fixture regeneration/default verification is approved and complete.

## 2026-06-25 00:33 UTC - RAG associated-function target expansion coverage

- Branch/HEAD: same branch at `71e6a7a7`; active autonomous call-graph goal remains open.
- Changed: added `call_context_expansion_adds_incoming_fixture_associated_function_callers` in `ploke-rag`, proving target-centered RAG expansion can seed from the real `LocalAssoc::make` associated-function target and materialize both a method owner (`Self::make`) and a function owner (`<LocalAssoc>::make()` normalized to `LocalAssoc::make`) with outgoing `AssociatedFunction` call context.
- Test config note: this fixture target has multiple incoming callers, so the test raises only its local `max_owner_hits` and `max_caller_hits` caps to avoid truncation ordering while still preserving the target-specific assertions.
- Verified: focused `cargo test -p ploke-rag --features call_graph incoming_fixture_associated_function_callers -- --nocapture` passed with `1 passed`; broader `cargo test -p ploke-rag --features call_graph call_context -- --nocapture` passed with `7 passed`.
- Next implementation: continue public-consumer coverage where deterministic retrieval exists; avoid public sparse `get_context` for generic `make` unless the seed can be proven deterministically first.

## 2026-06-25 00:42 UTC - TUI associated-function call-context rendering coverage

- Branch/HEAD: same branch at `71e6a7a7`; active autonomous call-graph goal remains open.
- Changed: strengthened TUI formatter and context-plan overlay tests with a fixture-shaped `Self::make` path call carrying an `AssociatedFunction` target. This proves model-facing context text and expanded overlay details render associated-function targets distinctly as `AssociatedFunction:<id>`.
- Invariant: TUI call-context rendering must preserve target relation labels from RAG payloads; associated-function calls must not be displayed as ordinary `Function` or `Method` targets.
- Verified: focused `cargo test -p ploke-tui --features call_graph format_call_context_block_renders_fixture_derived_rows -- --nocapture` passed with `1 passed`; focused `cargo test -p ploke-tui --features call_graph expanded_rag_part_displays_call_context_details -- --nocapture` passed with `1 passed`; broader `cargo test -p ploke-tui --features call_graph call_context -- --nocapture` passed with `3 passed`.
- Next implementation: continue downstream consumer coverage where call-context payload shape is externally visible, then return to parser/resolver implementation only when a strict DB or public-consumer contract exposes a missing behavior.

## 2026-06-25 00:15 UTC - DB fixture const/static owner coverage

- Branch/HEAD: same branch at `71e6a7a7`; active autonomous call-graph goal remains open.
- Changed: added fixture-backed `ploke-db` assertions for `call_context_for_owner(...)` over top-level const/static initializer owners in `fixture_nodes` and inherent/trait associated const initializer owners in `fixture_call_graph`.
- Invariant: const, static, and associated-const initializer calls must persist under the actual value owner ID, preserve the syntactic path payload (`five` or `assoc_const_value`), and resolve to a local `Function` edge without being attributed to an enclosing module, impl, or trait owner.
- Verified: focused `cargo test -p ploke-db --features call_graph initializer_calls -- --nocapture` passed with `2 passed`; fixture module `cargo test -p ploke-db --features call_graph call_graph_fixture_queries -- --nocapture` passed with `42 passed`; broad DB filter `cargo test -p ploke-db --features call_graph call_graph_ -- --nocapture` passed with `53 passed`.
- Next implementation: keep using `ploke-db` fixture-backed contracts for parser-green rows that still need persisted acceptance, then continue downstream proof/RAG/TUI consumer checks and leave the `call_graph` fixture gate in place until approved fixture review/regeneration is complete.

## 2026-06-25 00:20 UTC - DB fixture trait-object alias and constrained generic coverage

- Branch/HEAD: same branch at `71e6a7a7`; active autonomous call-graph goal remains open.
- Changed: expanded fixture-backed `ploke-db` trait-dispatch assertions for aliased concrete trait-object receiver proof and one-step reference-alias trait-object receiver proof, and added owner-context coverage for the constrained generic self-type impl case `GenericWrapper<GenericBoundValue>::constrained_generic_self_value`.
- Invariant: concrete trait-object alias forms must preserve `InitializedLocalBinding` receiver proof and resolve to the exact local impl method; constrained generic self-type dispatch must only resolve after the receiver generic argument proves the required local bound, without erasing `GenericWrapper<T>` into a broader target.
- Verified: focused `cargo test -p ploke-db --features call_graph trait_dispatch_method_calls -- --nocapture` passed with `1 passed`; focused `cargo test -p ploke-db --features call_graph constrained_generic_self_trait_method -- --nocapture` passed with `1 passed`; fixture module `cargo test -p ploke-db --features call_graph call_graph_fixture_queries -- --nocapture` passed with `43 passed`; broad DB filter `cargo test -p ploke-db --features call_graph call_graph_ -- --nocapture` passed with `54 passed`.
- Next implementation: continue DB fixture-backed coverage for parser-green dynamic/member callee rows that still lack persisted assertions, then keep expanding proof/RAG/TUI consumers from the proven DB surface.

## 2026-06-25 00:24 UTC - DB fixture dynamic member/index matrix coverage

- Branch/HEAD: same branch at `71e6a7a7`; active autonomous call-graph goal remains open.
- Changed: expanded fixture-backed `ploke-db` dynamic-call assertions for exact named-field aliases, indexed named-field aliases, indexed tuple-field aliases, typed indexed arrays, and one-step indexed array aliases. Also expanded fail-closed assertions for guarded/nested branch callees, closure branch/arm callees, opaque function-pointer branch/arm callees, closure cast/deref callees, opaque indexed field callees, and closure-literal body calls.
- Invariant: exact member/index initializer proof must persist as `DynamicFunction` edges to `local_target`, while opaque, guarded, nested, closure, or parameter-rooted dynamic callees must remain `Unsupported` with no fabricated target edge. Tuple-struct setup rows may coexist with the dynamic row, but must not hide or replace it.
- Verified: focused `cargo test -p ploke-db --features call_graph resolved_dynamic_function_shapes -- --nocapture` passed with `1 passed`; focused `cargo test -p ploke-db --features call_graph targetless_dynamic_failures -- --nocapture` passed with `1 passed`; fixture module `cargo test -p ploke-db --features call_graph call_graph_fixture_queries -- --nocapture` passed with `43 passed`; broad DB filter `cargo test -p ploke-db --features call_graph call_graph_ -- --nocapture` passed with `54 passed`.
- Next implementation: continue from DB fixture-backed contracts toward proof/RAG/TUI consumer assertions where these dynamic rows should be exposed or deliberately omitted.

## 2026-06-25 00:29 UTC - RAG dynamic call-context coverage

- Branch/HEAD: same branch at `71e6a7a7`; active autonomous call-graph goal remains open.
- Changed: added fixture-backed `ploke-rag` assertions that `RagService::collect_call_context(...)` preserves real `DynamicFunction` rows from `call_aliased_indexed_named_field_function_binding` and targetless unsupported dynamic rows from `call_dereferenced_closure_binding`. Added target-centered expansion coverage proving a `local_target` seed can materialize a dynamic-function caller owner and keep the outgoing `DynamicFunction` edge.
- Invariant: RAG call-context carriers must preserve `CallSiteKind::Dynamic`, `CallCalleeInfo::Dynamic`, `Resolved(LocalExact)` dynamic-function targets, and unsupported targetless dynamic calls without rewriting them into path/function calls or fabricating targets.
- Verified: focused `cargo test -p ploke-rag --features call_graph real_fixture_dynamic_rows -- --nocapture` passed with `1 passed`; focused `cargo test -p ploke-rag --features call_graph incoming_fixture_dynamic_callers -- --nocapture` passed with `1 passed`; broader `cargo test -p ploke-rag --features call_graph call_context -- --nocapture` passed with `9 passed`; dependent DB broad filter `cargo test -p ploke-db --features call_graph call_graph_ -- --nocapture` passed with `54 passed`.
- Next implementation: continue downstream TUI/tool carrier coverage for real dynamic call-context rows, then return to proof projection where dynamic target facts should be visible.

## 2026-06-25 00:33 UTC - TUI/tool dynamic call-context carrier coverage

- Branch/HEAD: same branch at `71e6a7a7`; active autonomous call-graph goal remains open.
- Changed: strengthened TUI formatter and context-plan overlay tests with a fixture-shaped dynamic call row carrying a `DynamicFunction` target, and strengthened `request_code_context` tool I/O serde coverage with the same dynamic carrier shape.
- Invariant: UI and tool-facing carriers must preserve dynamic call-context rows as `CallSiteKind::Dynamic` / `CallCalleeInfo::Dynamic` and render or roundtrip the `DynamicFunction:<id>` target label without collapsing it into an ordinary function edge.
- Verified: focused `cargo test -p ploke-tui --features call_graph format_call_context_block_renders_fixture_derived_rows -- --nocapture`, `expanded_rag_part_displays_call_context_details`, and `serde_roundtrip_request_code_context` each passed with `1 passed`; broader `cargo test -p ploke-tui --features call_graph call_context -- --nocapture` passed with `3 passed`; `cargo test -p ploke-tui --features call_graph tool_io_roundtrip -- --nocapture` passed with `3 passed`; dependent `cargo test -p ploke-rag --features call_graph call_context -- --nocapture` passed with `9 passed`.
- Next implementation: return to proof projection where dynamic target facts should be visible, while keeping the `call_graph` gate in place until fixture review/regeneration is approved and complete.

## 2026-06-25 00:41 UTC - DB proof dynamic call projection coverage

- Branch/HEAD: same branch at `71e6a7a7`; active autonomous call-graph goal remains open.
- Changed: added fixture-backed `ploke-db` proof projection assertions for real dynamic call rows: resolved `call_aliased_indexed_named_field_function_binding` projects a `DynamicFunction` proof edge to `local_target`, unsupported `call_dereferenced_closure_binding` projects a `dynamic_dispatch_unbounded` blocker with no edge, and target-centered `local_target` proof projection includes the dynamic incoming caller without importing unrelated unsupported dynamic blockers.
- Invariant: proof projection must preserve dynamic call-site identity, owner ID, target ID, resolved state, and source provenance for exact dynamic-function calls; targetless unsupported dynamic calls must remain blocker-only facts and must not fabricate proof checker edges.
- Verified: focused `cargo test -p ploke-db --features call_graph dynamic_call_proof -- --nocapture` passed with `2 passed`; focused `cargo test -p ploke-db --features call_graph unsupported_dynamic_call_without_edges -- --nocapture` passed with `1 passed`; fixture module `cargo test -p ploke-db --features call_graph call_graph_fixture_queries -- --nocapture` passed with `46 passed`; broad DB filter `cargo test -p ploke-db --features call_graph call_graph_ -- --nocapture` passed with `57 passed`.
- Next implementation: keep focusing on `ploke-db` proof/query contracts for parser-green call graph rows and only return to parser/resolver implementation when a DB or public-consumer contract exposes missing behavior.

## 2026-06-25 00:45 UTC - DB proof initializer owner coverage

- Branch/HEAD: same branch at `71e6a7a7`; active autonomous call-graph goal remains open.
- Changed: added fixture-backed `ploke-db` proof projection assertions for top-level const/static initializer owners in `fixture_nodes` and associated-const initializer owners in `fixture_call_graph`.
- Invariant: proof projection must preserve the const/static/associated-const value owner as the proof caller, store resolved proof edges to the local initializer function, retain call-site source provenance, and avoid blocker rows for these exact initializer calls.
- Verified: focused `cargo test -p ploke-db --features call_graph initializer_call_proof -- --nocapture` passed with `2 passed`; focused `cargo test -p ploke-db --features call_graph associated_const_initializer_call_proof -- --nocapture` passed with `1 passed`; fixture module `cargo test -p ploke-db --features call_graph call_graph_fixture_queries -- --nocapture` passed with `48 passed`; broad DB filter `cargo test -p ploke-db --features call_graph call_graph_ -- --nocapture` passed with `59 passed`.
- Next implementation: continue DB proof/query coverage for parser-green rows that do not yet have persisted proof assertions, then move back outward through RAG/TUI only when the DB contract is pinned.

## 2026-06-25 00:50 UTC - DB proof non-resolved strictness and blockers

- Branch/HEAD: same branch at `71e6a7a7`; active autonomous call-graph goal remains open.
- Changed: tightened `ploke-db` proof validation so ambiguous call statuses with local target rows reject like every other non-resolved status. Added owner-scoped and target-centered synthetic regressions for ambiguous local targets, plus fixture-backed proof projection for real macro blockers and real targetless ambiguous method blockers.
- Invariant: only `Resolved(LocalExact)` rows may carry proof checker edges. Ambiguous rows may project blocker/candidate resolution facts only when targetless in the current call graph; targetless macro calls must remain `macro_expansion_not_available` blockers without fabricated edges.
- Verified: focused `cargo test -p ploke-db --features call_graph ambiguous_local_targets -- --nocapture` passed with `2 passed`; focused `macro_call_without_edges` and `ambiguous_call_without_edges` passed with `1 passed` each; `cargo test -p ploke-db --features call_graph call_graph_queries -- --nocapture` passed with `13 passed`; fixture module passed with `50 passed`; broad DB filter passed with `63 passed`.
- Next implementation: continue from DB proof/query contracts; useful next slices are public consumer coverage for blocker rendering or more fixture-backed proof assertions for already-green external/unsupported shapes.

## 2026-06-25 00:55 UTC - DB proof constructor target coverage

- Branch/HEAD: same branch at `71e6a7a7`; active autonomous call-graph goal remains open.
- Changed: added fixture-backed `ploke-db` proof projection assertions for resolved tuple struct constructor and enum variant constructor call targets. The enum variant helper qualifies the variant by owning enum name rather than assuming a globally unique variant name.
- Invariant: proof projection must store constructor calls as resolved proof edges to their actual `StructNodeId` / `VariantNodeId` targets, preserve caller owner IDs and source provenance, and avoid collapsing constructor targets into ordinary function or method IDs.
- Verified: focused `cargo test -p ploke-db --features call_graph tuple_struct_constructor_call_proof -- --nocapture` passed with `1 passed`; focused `enum_variant_constructor_call_proof` passed with `1 passed`; fixture module passed with `52 passed`; broad DB filter passed with `65 passed`.
- Next implementation: continue DB proof/query coverage or move outward to RAG/TUI rendering of blocker and constructor target-family payloads once the DB surface is sufficiently pinned.

## 2026-06-25 01:01 UTC - RAG/TUI constructor call-context carriers

- Branch/HEAD: same branch at `71e6a7a7`; active autonomous call-graph goal remains open.
- Changed: added fixture-backed `ploke-rag` coverage proving real tuple struct and enum variant constructor rows are collected into `CallContextInfo` with `TupleStructConstructor` and `EnumVariantConstructor` target relations. Strengthened TUI formatter, context-plan overlay, and `request_code_context` tool roundtrip coverage so constructor target-family labels render and survive serde alongside function/method/associated/dynamic targets.
- Invariant: downstream call-context consumers must preserve constructor target families as distinct relation labels and must not collapse them into ordinary function or method targets after DB projection.
- Verified: focused `cargo test -p ploke-rag --features call_graph real_fixture_constructor_rows -- --nocapture` passed with `1 passed`; broader RAG call-context filter passed with `10 passed`; TUI focused formatter, overlay, and tool roundtrip tests each passed with `1 passed`; broader TUI call-context and tool-IO filters passed with `3 passed` each.
- Next implementation: continue outward consumer coverage for blocker/status rendering, or return to DB proof/query contracts if another persisted target/status family remains uncovered.

## 2026-06-25 01:08 UTC - RAG/TUI blocker call-context carriers

- Branch/HEAD: same branch at `71e6a7a7`; active autonomous call-graph goal remains open.
- Changed: added fixture-backed `ploke-rag` coverage proving real targetless macro and ambiguous method rows are collected into `CallContextInfo` with `Unsupported` / `Ambiguous` statuses and no targets. Strengthened TUI formatter, context-plan overlay, and `request_code_context` carrier coverage so macro callees and ambiguous local-binding method rows survive downstream.
- Invariant: blocker call-context rows must remain visible as status-bearing structural facts without fabricated targets. The TUI formatter and overlay keep the existing eight-row display cap and expose overflow through the `... 1 more call site(s)` marker.
- Verified: focused RAG blocker test passed with `1 passed`; broader RAG call-context filter passed with `11 passed`; focused TUI formatter and overlay tests each passed with `1 passed`; broader TUI call-context and tool-IO filters passed with `3 passed` each.
- Next implementation: finish focused TUI verification, then continue DB-first proof/query coverage or broader public consumer checks before adding more parser-only rows.

## 2026-06-25 01:16 UTC - RAG/TUI external call-context carriers

- Branch/HEAD: same branch at `71e6a7a7`; active autonomous call-graph goal remains open.
- Changed: added fixture-backed `ploke-rag` coverage proving real external targetless rows for `String::new()`, literal `.to_string()`, `Vec::new()`, and typed-local `Vec::len()` are collected into `CallContextInfo` with no targets. Added compact TUI formatter/overlay coverage and extended `request_code_context` carrier coverage for external targetless rows.
- Invariant: external call-context rows must preserve callee and receiver shape while remaining targetless; downstream carriers must not invent local edges for prelude/std calls.
- Verified: focused RAG external-row test passed with `1 passed`; broader RAG call-context filter passed; focused TUI formatter and overlay external-row tests each passed; focused `serde_roundtrip_request_code_context` passed; broader TUI call-context and tool-IO filters passed.
- Next implementation: continue DB-first proof/query coverage or broader public consumer checks before adding more parser-only rows.

## 2026-06-25 01:27 UTC - grouped function import path-call coverage

- Branch/HEAD: same branch at `71e6a7a7`; active autonomous call-graph goal remains open.
- Changed: appended `grouped_function_import_scope` to `fixture_call_graph` and added strict parser plus DB fixture assertions for grouped function imports resolving through direct grouped aliases to `import_targets::{imported_target, globbed_target}`.
- Result: no production resolver change was needed. Existing import backlink resolution already handles grouped function imports once the paranoid tests use exact post-format byte spans.
- Verified: focused grouped alias and grouped globbed-alias parser tests passed; focused DB `path_resolution_forms` passed; fixture-backed DB module passed with `52 passed`; broad parser `call_sites` passed with `205 passed`; broad DB `call_graph_` passed with `65 passed`.
- Next implementation: update this entry after broad verification, then continue with remaining strict resolver gaps or DB/proof/public-consumer contracts.

## 2026-06-25 01:32 UTC - grouped trait associated-function DB coverage

- Branch/HEAD: same branch at `71e6a7a7`; active autonomous call-graph goal remains open.
- Changed: appended `grouped_trait_assoc_function_scope` to `fixture_call_graph` and extended the existing `ploke-db` imported trait associated-function fixture contract with a grouped aliased `GroupedAssocFunctionTrait::imported_trait_make()` shorthand call.
- Result: no production resolver change was needed. Existing grouped import backlink resolution also carries shorthand local trait associated-function path calls through parser, transform, and DB call-context projection.
- Invariant: grouped imported trait associated-function calls must remain `Resolved(LocalExact)` `Path` call sites with a single `AssociatedFunction` edge to the imported trait method target; DB context must preserve the grouped alias path and method target family.
- Verified: focused parser `grouped_imported_trait_associated_function` passed with `1 passed`; focused DB `imported_trait_associated_function_calls` passed with `1 passed`; fixture-backed DB module passed with `52 passed`; broad DB `call_graph_` passed with `65 passed`; broad parser `call_sites` passed with `206 passed`.
- Next implementation: continue DB-first call-graph contracts, especially public query/proof/RAG/TUI consumer coverage, before adding parser-only rows.

## 2026-06-25 01:36 UTC - low-level DB call-graph helper coverage

- Branch/HEAD: same branch at `71e6a7a7`; active autonomous call-graph goal remains open.
- Changed: added `fixture_low_level_helpers_read_projected_sites_targets_and_statuses` in `ploke-db` to exercise `call_sites_for_owner`, `call_targets_for_site`, and `call_resolution_for_site` directly over the real `call_try_result_instance_method` fixture owner.
- Invariant: lower-level DB helpers must preserve source-span site ordering, distinguish unsupported targetless blocker rows from resolved rows, and expose the same site IDs, status kinds, source kinds, target IDs, relation kinds, and target families consumed by `call_context_for_owner` and proof projection.
- Verified: focused DB `low_level_helpers` passed with `1 passed`; fixture-backed DB module passed with `53 passed`; broad DB `call_graph_` passed with `66 passed`.
- Next implementation: continue DB-first helper/proof contracts, then move outward only when the persisted API surface is pinned enough for RAG/TUI consumers.

## 2026-06-25 01:39 UTC - call-graph relation gate helper coverage

- Branch/HEAD: same branch at `71e6a7a7`; active autonomous call-graph goal remains open.
- Changed: added `has_call_graph_relations_reports_schema_presence` in the synthetic `ploke-db` call-graph query suite.
- Invariant: `Database::has_call_graph_relations` must return `false` for an initialized DB without the call-graph relations and `true` once the full schema creates `call_site`, `call_site_edge`, `call_relation`, and `call_resolution_status`.
- Verified: focused DB `has_call_graph_relations` passed; synthetic DB `call_graph_queries` passed with `14 passed`; broad DB `call_graph_` passed with `67 passed`.
- Next implementation: keep strengthening DB query/proof contracts around persisted call-graph relations before removing or relaxing the rollout gate.

## 2026-06-25 01:45 UTC - generated call proof fact shape coverage

- Branch/HEAD: same branch at `71e6a7a7`; active autonomous call-graph goal remains open.
- Changed: added `call_proof_facts_for_owner_preserves_mixed_resolution_shape` in the synthetic `ploke-db` call-graph query suite.
- Invariant: generated owner-scoped call proof facts must preserve the pre-storage shape: one `call_site` fact per persisted site, exactly one `call_edge` fact for the resolved local row, one `call_resolution` fact per site, resolved rows carrying `resolved_def_id` with no blocker, external rows carrying `externally_summarized` plus `external_dependency_summary_missing`, and unsupported dynamic rows carrying `blocked` plus `dynamic_dispatch_unbounded`.
- Verified: focused DB `mixed_resolution_shape` passed with `1 passed`; synthetic DB `call_graph_queries` passed with `15 passed`; broad DB `call_graph_` passed with `68 passed`.
- Next implementation: continue DB/proof contracts for generated and stored call graph proof facts, then move outward to RAG/TUI only after persisted behavior is pinned.

## 2026-06-25 01:48 UTC - generated proof facts feed invariant checker

- Branch/HEAD: same branch at `71e6a7a7`; active autonomous call-graph goal remains open.
- Changed: added `generated_call_resolution_blockers_feed_proof_invariants` in the synthetic `ploke-db` call-graph query suite.
- Invariant: call-graph-generated `call_resolution` blocker facts must participate in `proof_invariant_findings` after storage. A generated external call resolution linked to a process effect must block the detached-process handoff invariant with `external_dependency_summary_missing`.
- Verified: focused DB `generated_call_resolution_blockers` passed with `1 passed`; synthetic DB `call_graph_queries` passed with `16 passed`; broad DB `call_graph_` passed with `69 passed`.
- Next implementation: continue with persisted proof/query contracts or move outward to RAG/TUI proof consumers once DB behavior is sufficiently pinned.

## 2026-06-25 01:54 UTC - RAG caller expansion limit coverage

- Branch/HEAD: same branch at `71e6a7a7`; active autonomous call-graph goal remains open.
- Changed: added `call_context_expansion_respects_max_caller_hits_by_score` in the `ploke-rag` call-context unit suite.
- Invariant: target-centered RAG call-context expansion must preserve seed hits, derive caller scores with `caller_factor`, prefer callers from higher-scored target hits, and enforce `max_caller_hits` without admitting lower-scored callers after the cap.
- Verified: focused `cargo test -p ploke-rag --features call_graph max_caller_hits_by_score -- --nocapture` passed with `1 passed`; broader `cargo test -p ploke-rag --features call_graph call_context -- --nocapture` passed with `13 passed`; dependent broad DB filter `cargo test -p ploke-db --features call_graph call_graph_ -- --nocapture` passed with `69 passed`.
- Next implementation: return to DB-first persisted query/proof coverage unless a downstream consumer test exposes a missing DB contract.

## 2026-06-25 01:59 UTC - stable call proof fact identity coverage

- Branch/HEAD: same branch at `71e6a7a7`; active autonomous call-graph goal remains open.
- Changed: added `owner_and_target_projection_share_stable_fact_identity` in the synthetic `ploke-db` call-graph query suite.
- Invariant: owner-scoped and target-centered projection of the same persisted resolved call site must share stable `call_site`, `call_edge`, and `call_resolution` fact identities, so incremental proof projection upserts existing facts instead of duplicating rows.
- Verified: focused DB `owner_and_target_projection_share_stable_fact_identity` passed with `1 passed`; synthetic DB `call_graph_queries` passed with `17 passed`; broad DB `call_graph_` passed with `70 passed`.
- Next implementation: continue DB-first persisted query/proof contracts, especially where RAG/TUI/proof consumers depend on incremental projection behavior.

## 2026-06-25 02:01 UTC - generated proof query linkage coverage

- Branch/HEAD: same branch at `71e6a7a7`; active autonomous call-graph goal remains open.
- Changed: added `proof_graphrag_context_links_generated_call_facts_by_call_site` in the synthetic `ploke-db` call-graph query suite.
- Invariant: after call-graph proof projection, a GraphRAG/proof query that matches the generated callee edge must include the linked `call_site` and `call_resolution` facts through shared `call_site_id`, even when those linked rows do not independently contain the callee id.
- Verified: focused DB `proof_graphrag_context_links_generated_call_facts_by_call_site` passed with `1 passed`; synthetic DB `call_graph_queries` passed with `18 passed`; broad DB `call_graph_` passed with `71 passed`.
- Next implementation: continue DB-first query/proof contracts or move outward only when the persisted proof context shape is pinned enough for RAG/TUI consumers.

## 2026-06-25 02:06 UTC - fixture proof query linkage coverage

- Branch/HEAD: same branch at `71e6a7a7`; active autonomous call-graph goal remains open.
- Changed: added `fixture_proof_query_links_real_resolved_call_facts` in the fresh fixture-backed `ploke-db` call-graph suite.
- Invariant: the same `call_site_id` linkage used by synthetic generated proof facts must hold after real parser -> transform -> DB projection. A query matching the real `local_target` callee edge must return the generated edge plus linked real `call_site` and `call_resolution` rows.
- Verified: focused DB `fixture_proof_query_links_real_resolved_call_facts` passed with `1 passed`; fixture-backed DB module passed with `54 passed`; broad DB `call_graph_` passed with `72 passed`.
- Next implementation: continue DB-first contracts, then widen to RAG/TUI only where persisted proof-query behavior is already pinned.

## 2026-06-25 02:11 UTC - RAG call-context missing-relation gate coverage

- Branch/HEAD: same branch at `71e6a7a7`; active autonomous call-graph goal remains open.
- Changed: added `call_context_disabled_safely_when_relations_absent` in the `ploke-rag` call-context unit suite.
- Invariant: when a database lacks call-graph relations, `RagService` must record degraded call-context state, disable call-context collection/expansion, and avoid querying `call_site`, `call_site_edge`, `call_relation`, or `call_resolution_status`.
- Verified: focused `cargo test -p ploke-rag --features call_graph call_context_disabled_safely_when_relations_absent -- --nocapture` passed with `1 passed`; broader `cargo test -p ploke-rag --features call_graph call_context -- --nocapture` passed with `14 passed`; dependent broad DB filter `cargo test -p ploke-db --features call_graph call_graph_ -- --nocapture` passed with `72 passed`.
- Next implementation: continue widening downstream contracts only where DB behavior is already pinned, or return to DB proof/query strictness if a consumer gap appears.

## 2026-06-25 02:14 UTC - TUI call-context degradation note coverage

- Branch/HEAD: same branch at `71e6a7a7`; active autonomous call-graph goal remains open.
- Changed: `request_code_context` now appends a model-visible note and recovery next steps when RAG reports degraded call-context under `call_graph`, matching the existing type-context degraded-note pattern.
- Invariant: when call-context collection/expansion is disabled because call-graph relations are unavailable, TUI tool output must not silently omit call graph payloads; the returned `RequestCodeContextResult` must explain that call-context expansion is unavailable.
- Verified: focused `cargo test -p ploke-tui --features call_graph call_context_degradation_is_model_visible -- --nocapture` passed with `1 passed`; broader TUI `call_context` filter passed with `6 passed`; dependent RAG `call_context` filter passed with `14 passed`.
- Next implementation: continue downstream model-facing contracts or return to DB/proof strictness as new gaps appear.

## 2026-06-25 02:23 UTC - DB call-context expansion helper coverage

- Branch/HEAD: same branch at `71e6a7a7`; active autonomous call-graph goal remains open.
- Changed: added `CallContextSeed`, `CallContextRelation`, `CallContextCandidate`, `CallContextOptions`, and `Database::expand_call_context`, then rewired RAG target-centered caller expansion to consume the DB helper.
- Invariant: application-facing call-context expansion must follow the type-graph helper pattern: owner seeds produce resolved outgoing target candidates, target seeds produce resolved incoming caller candidates, caps are enforced in the DB helper, and non-resolved target rows remain visible through low-level helpers without being promoted as graphRAG expansion candidates.
- Verified: focused `cargo test -p ploke-db --features call_graph expand_call_context -- --nocapture` passed with `3 passed`; synthetic DB `call_graph_queries` passed with `20 passed`; RAG `call_context` passed with `14 passed`; broad DB `call_graph_` passed with `75 passed`.
- Next implementation: keep DB-first helper/proof contracts as the primary focus; use RAG/TUI tests to verify consumers after each persisted behavior slice.

## 2026-06-25 02:28 UTC - RAG owner-seeded outgoing call expansion

- Branch/HEAD: same branch at `71e6a7a7`; active autonomous call-graph goal remains open.
- Changed: `RagService::expand_hits_with_call_context` now asks `Database::expand_call_context` for both owner-seeded outgoing callee candidates and target-seeded incoming caller candidates, then uses the existing score merge, materialization filter, and cap logic.
- Invariant: a retrieval hit on a caller owner must be able to materialize its resolved callee definitions through the same DB expansion helper used for incoming caller expansion, while preserving seed hits and retaining outgoing call-context payloads on the caller part.
- Verified: focused `cargo test -p ploke-rag --features call_graph call_context_expansion_adds_outgoing_fixture_targets -- --nocapture` passed with `1 passed`; public sparse `cargo test -p ploke-rag --features call_graph call_context_sparse_get_context_expands_owner_hits_to_fixture_callees -- --nocapture` passed with `1 passed`; broader RAG `call_context` filter passed with `16 passed`.
- Next implementation: continue validating downstream consumers of the DB helper, then return to DB/proof strictness or parser/resolver exact-proof gaps only where a strict local proof exists.

## 2026-06-25 02:37 UTC - DB fixture expansion provenance coverage

- Branch/HEAD: same branch at `71e6a7a7`; active autonomous call-graph goal remains open.
- Changed: strengthened `ploke-db` fixture-backed `expand_call_context` coverage with owner-seeded expansion from `call_try_result_instance_method` and target-seeded expansion from `LocalAssoc::instance_value`.
- Invariant: owner expansion over a mixed-status owner promotes only resolved outgoing callees and preserves the exact call-site IDs for `try_local_assoc()` and the try-result method call; target expansion for a method target preserves both ordinary method-call callers and associated-function path-call callers as incoming candidates.
- Verified: focused `cargo test -p ploke-db --features call_graph fixture_expand_call_context -- --nocapture` passed with `3 passed`; fixture module `cargo test -p ploke-db --features call_graph call_graph_fixture_queries -- --nocapture` passed with `57 passed`; broad DB `cargo test -p ploke-db --features call_graph call_graph_ -- --nocapture` passed with `77 passed`.
- Next implementation: keep focus on DB-first contracts for call-context expansion/proof facts, then propagate only newly proven DB behavior through RAG/TUI/tool metadata.

## 2026-06-25 02:47 UTC - RAG/TUI call-expansion provenance carrier

- Branch/HEAD: same branch at `71e6a7a7`; active autonomous call-graph goal remains open.
- Changed: added `CallExpansionKind` / `CallExpansionInfo` to the shared RAG payloads, threaded optional `call_expansion` through `ContextPart`, `ConciseContext`, RAG context assembly, context-plan events, model-facing prompt formatting, context-plan overlay details, and `request_code_context` tool carriers.
- Invariant: `call_expansion` explains why RAG added a context part via DB call-context expansion; seed hits remain unmarked, while newly materialized outgoing callees and incoming callers carry relation, seed ID, call-site ID, target ID, and distance.
- Verified: focused public RAG expansion tests for owner, function target, and method target seeds each passed with `1 passed`; focused TUI formatter, overlay, and tool serde tests each passed with `1 passed`; broader `cargo test -p ploke-rag --features call_graph call_context -- --nocapture` passed with `16 passed`; broader `cargo test -p ploke-tui --features call_graph call_context -- --nocapture` passed with `6 passed`; broader `cargo test -p ploke-tui --features call_graph tool_io_roundtrip -- --nocapture` passed with `3 passed`.
- Next implementation: continue DB/proof-first coverage for remaining persisted call families, then expose only proven DB behavior through RAG/TUI surfaces.

## 2026-06-25 02:59 UTC - DB fixture receiver and precedence coverage

- Branch/HEAD: same branch at `71e6a7a7`; active autonomous call-graph goal remains open.
- Changed: strengthened `ploke-db` fixture-backed owner-context coverage for local, initialized, parenthesized typed-local, type-alias-chain, and imported type-alias instance-method receivers; parenthesized function-item and typed function-pointer alias dynamic calls; and inherent-over-trait method precedence.
- Invariant: parser-green call families must preserve their persisted receiver/callee payloads, resolved `LocalExact` status, and target edge family through parser -> transform -> DB projection; inherent precedence rows must project a target owned by the inherent impl, not a same-name trait impl.
- Verified: focused `cargo test -p ploke-db --features call_graph fixture_context_reads_projected_local_and_alias_instance_method_receivers -- --nocapture` passed with `1 passed`; focused `cargo test -p ploke-db --features call_graph fixture_context_reads_projected_parenthesized_binding_dynamic_calls -- --nocapture` passed with `1 passed`; focused `cargo test -p ploke-db --features call_graph fixture_context_reads_projected_inherent_method_precedence -- --nocapture` passed with `1 passed`; fixture module `cargo test -p ploke-db --features call_graph call_graph_fixture_queries -- --nocapture` passed with `60 passed`; broad DB `cargo test -p ploke-db --features call_graph call_graph_ -- --nocapture` passed with `80 passed`.
- Next implementation: continue DB/proof-first coverage before adding new parser-only resolver rows.

## 2026-06-25 03:05 UTC - DB fixture callable path and returned-function coverage

- Branch/HEAD: same branch at `71e6a7a7`; active autonomous call-graph goal remains open.
- Changed: added `ploke-db` fixture assertions for `make_fn()()` nested returned-function rows, bare callable-value path failures for function-pointer parameters and generic `FnOnce` values, boxed `dyn Fn` path-call setup/failure rows, and prelude `Vec::new()` external classification.
- Invariant: nested returned-function calls must persist both the resolved inner path edge and unsupported outer dynamic row; callable-value path calls and prelude constructor rows must preserve their syntactic path payloads while avoiding fabricated local targets.
- Verified: focused `cargo test -p ploke-db --features call_graph fixture_context_reads_projected_returned_function_nested_calls -- --nocapture` passed with `1 passed`; focused `cargo test -p ploke-db --features call_graph fixture_context_reads_projected_callable_value_path_failures_and_vec_external -- --nocapture` passed with `1 passed`; fixture module `cargo test -p ploke-db --features call_graph call_graph_fixture_queries -- --nocapture` passed with `62 passed`; broad DB `cargo test -p ploke-db --features call_graph call_graph_ -- --nocapture` passed with `82 passed`.
- Coverage note: a direct comparison of explicit `fixture_call_graph` `call_*` owners against string-named owners in `call_graph_fixture_queries.rs` is now empty.
- Next implementation: continue proof/RAG/TUI surfaces from the now-complete explicit fixture-owner DB baseline.

## 2026-06-25 03:09 UTC - DB proof returned-function and callable-path coverage

- Branch/HEAD: same branch at `71e6a7a7`; active autonomous call-graph goal remains open.
- Changed: added fixture-backed proof projection coverage for `call_returned_function`, targetless callable-value path failures, boxed `dyn Fn` setup/failure rows, and prelude `Vec::new()`.
- Invariant: proof projection for `make_fn()()` must store the resolved inner function edge and the unsupported outer dynamic blocker with source provenance; callable-value path rows must become `type_resolution_missing` blockers without proof edges; external constructor rows must become `external_dependency_summary_missing` rows without proof edges.
- Verified: focused `cargo test -p ploke-db --features call_graph fixture_projection_stores_real_returned_function_call_proof_facts -- --nocapture` passed with `1 passed`; focused `cargo test -p ploke-db --features call_graph fixture_projection_marks_real_callable_path_and_vec_external_rows_without_edges -- --nocapture` passed with `1 passed`; fixture module `cargo test -p ploke-db --features call_graph call_graph_fixture_queries -- --nocapture` passed with `64 passed`; broad DB `cargo test -p ploke-db --features call_graph call_graph_ -- --nocapture` passed with `84 passed`.
- Next implementation: continue from DB/proof contracts into RAG/TUI proof consumers or any remaining public call-context surfaces that need these blocker shapes.

## 2026-06-25 03:14 UTC - RAG callable-path call-context payloads

- Branch/HEAD: same branch at `71e6a7a7`; active autonomous call-graph goal remains open.
- Changed: added fixture-backed `ploke-rag` coverage proving `RagService::collect_call_context` preserves returned-function mixed rows, opaque function-pointer parameter calls, generic `FnOnce` path calls, boxed `dyn Fn` setup/failure rows, and prelude `Vec::new()` external rows.
- Invariant: downstream RAG payloads must retain the persisted call-site kind, callee path/dynamic shape, status, resolution, and target list exactly enough to avoid fabricating local targets for targetless blockers while still exposing the resolved `make_fn()` edge.
- Verified: focused `cargo test -p ploke-rag --features call_graph call_context_collection_reads_real_fixture_callable_path_rows -- --nocapture` passed with `1 passed`; broader `cargo test -p ploke-rag --features call_graph call_context -- --nocapture` passed with `17 passed`.
- Next implementation: continue TUI/tool payload rendering for these callable-path blocker shapes if model-facing coverage is missing, otherwise continue proof/RAG surfaces from DB contracts.

## 2026-06-25 03:17 UTC - TUI callable-path call-context rendering

- Branch/HEAD: same branch at `71e6a7a7`; active autonomous call-graph goal remains open.
- Changed: added model-facing TUI formatter coverage for returned-function mixed rows, opaque callable-value path blockers, boxed `dyn Fn` setup/failure rows, and prelude `Vec::new()` external rows.
- Invariant: call-context text rendered into model-facing context must preserve path/dynamic callee shape, status, resolution, and target display for these callable-path rows, including explicit `targets []` for blocker/external rows.
- Verified: focused `cargo test -p ploke-tui --features call_graph format_call_context_block_renders_callable_path_rows -- --nocapture` passed with `1 passed`; broader `cargo test -p ploke-tui --features call_graph call_context -- --nocapture` passed with `7 passed`.
- Next implementation: continue public tool/TUI carrier coverage if needed, then return to DB/proof/RAG contracts for any remaining call-context/proof gaps.

## 2026-06-25 03:26 UTC - DB proof parenthesized callable blockers

- Branch/HEAD: same branch at `71e6a7a7`; active autonomous call-graph goal remains open.
- Changed: added DB proof projection coverage for parenthesized generic `FnOnce` and boxed `dyn Fn` dynamic call rows, plus expanded context-plan overlay coverage for callable-path blocker rows.
- Invariant: parenthesized callable-value dynamic rows project to `dynamic_dispatch_unbounded` blocker proof facts, external setup rows project to `external_dependency_summary_missing`, and neither path fabricates proof checker edges or semantic targets.
- Verified: focused `cargo test -p ploke-db --features call_graph fixture_projection_marks_real_parenthesized_callable_dynamic_rows_without_edges -- --nocapture` passed with `1 passed`; fixture module `cargo test -p ploke-db --features call_graph call_graph_fixture_queries -- --nocapture` passed with `65 passed`; broad DB `cargo test -p ploke-db --features call_graph call_graph_ -- --nocapture` passed with `85 passed`; broader TUI `cargo test -p ploke-tui --features call_graph call_context -- --nocapture` passed with `8 passed`.
- Next implementation: continue DB-first coverage for the remaining semantic gaps from the call-site matrix, especially broader trait dispatch and closure/async body ownership before ungating fixtures.

## 2026-06-25 03:33 UTC - DB proof trait-dispatch callers

- Branch/HEAD: same branch at `71e6a7a7`; active autonomous call-graph goal remains open.
- Changed: added fixture-backed DB proof projection coverage for real `LocalDispatchTrait for TraitDispatchTarget` method-call rows, including concrete trait-object alias and chained-reference callers.
- Invariant: resolved trait-dispatch method rows project as proof checker edges to the concrete impl method in both owner-scoped and target-centered projection, keep source provenance, and do not produce type-resolution blockers.
- Verified: focused `cargo test -p ploke-db --features call_graph trait_dispatch_call_proof -- --nocapture` passed with `2 passed`; fixture module `cargo test -p ploke-db --features call_graph call_graph_fixture_queries -- --nocapture` passed with `67 passed`; broad DB `cargo test -p ploke-db --features call_graph call_graph_ -- --nocapture` passed with `87 passed`.
- Next implementation: keep using DB/proof tests to pin any remaining resolver semantics before broader fixture ungating; closure/async body ownership is still a design/implementation gap rather than a DB-only gap.

## 2026-06-25 03:38 UTC - RAG trait-dispatch caller expansion

- Branch/HEAD: same branch at `71e6a7a7`; active autonomous call-graph goal remains open.
- Changed: added fixture-backed RAG coverage for target-centered expansion from the `LocalDispatchTrait for TraitDispatchTarget::trait_value` impl method to real initialized-local and chained trait-object caller owners.
- Invariant: RAG call-context expansion must preserve the seed trait-dispatch target, materialize concrete trait-object callers, and attach outgoing `Method` call-context rows that still point back to the seed target.
- Verified: focused `cargo test -p ploke-rag --features call_graph call_context_expansion_adds_incoming_fixture_trait_dispatch_callers -- --nocapture` passed with `1 passed`; broader `cargo test -p ploke-rag --features call_graph call_context -- --nocapture` passed with `18 passed`; DB sanity `cargo test -p ploke-db --features call_graph call_graph_ -- --nocapture` passed with `87 passed`.
- Next implementation: continue downstream coverage for any remaining proven DB rows, or move deliberately into closure/async body ownership design rather than treating it as a minor fixture addition.

## 2026-06-25 03:45 UTC - Public RAG trait-dispatch target expansion

- Branch/HEAD: same branch at `71e6a7a7`; active autonomous call-graph goal remains open.
- Changed: added public sparse `get_context` coverage for concrete trait-dispatch target hits by seeding the `LocalDispatchTrait for TraitDispatchTarget::trait_value` impl method with the stable `144 trait_value` query.
- Invariant: the public RAG path must carry target-centered call expansion from sparse retrieval through final context assembly, preserving incoming-caller provenance and outgoing `Method` rows for both initialized-local and chained trait-object callers.
- Verified: focused `cargo test -p ploke-rag --features call_graph call_context_sparse_get_context_expands_trait_dispatch_target_hits_to_fixture_callers -- --nocapture` passed with `1 passed`; broader `cargo test -p ploke-rag --features call_graph call_context -- --nocapture` passed with `19 passed`; DB sanity `cargo test -p ploke-db --features call_graph call_graph_ -- --nocapture` passed with `87 passed`.
- Note: an earlier `trait_value TraitDispatchTarget` query was too rank-sensitive in the broad filter; the final test uses the impl-body literal to make the concrete target seed deterministic.

## 2026-06-25 03:52 UTC - TUI trait-dispatch call-context carrier coverage

- Branch/HEAD: same branch at `71e6a7a7`; active autonomous call-graph goal remains open.
- Changed: added TUI formatter, context-plan overlay, and `request_code_context` tool-carrier coverage for fixture-shaped concrete trait-dispatch rows: `trait_value` on `value = TraitDispatchTarget` with a resolved `Method` target and incoming-caller expansion provenance.
- Invariant: downstream model text, overlay details, and public tool JSON must preserve initialized-local receiver proof, `Resolved(LocalExact)` status, `Method:<id>` target labels, and call-expansion metadata for concrete trait-dispatch callers.
- Verified: focused formatter, overlay, and tool roundtrip tests each passed with `1 passed`; broader `cargo test -p ploke-tui --features call_graph call_context -- --nocapture` passed with `10 passed`; broader `cargo test -p ploke-tui --features call_graph tool_io_roundtrip -- --nocapture` passed with `3 passed`.
- Next implementation: keep pressure on downstream DB/RAG/TUI/proof contracts; closure/async body ownership and fixture ungating remain unresolved before the full call-graph goal can close.

## 2026-06-25 03:56 UTC - DB closure/async owner-boundary projection

- Branch/HEAD: same branch at `71e6a7a7`; active autonomous call-graph goal remains open.
- Changed: added a fixture-backed `ploke-db` contract proving closure, move-closure, async-block, and async-closure body calls to `local_target()` do not project as call-context path rows or target edges on the enclosing function owner.
- Invariant: until nested closure/async body owners exist, parser-native extraction and DB projection must fail closed by preserving the explicit owner boundary instead of attributing inner body calls to the outer owner.
- Verified: focused `cargo test -p ploke-db --features call_graph fixture_context_does_not_project_closure_or_async_body_calls_to_outer_owner -- --nocapture` passed with `1 passed`; fixture module `cargo test -p ploke-db --features call_graph call_graph_fixture_queries -- --nocapture` passed with `68 passed`; broad DB `cargo test -p ploke-db --features call_graph call_graph_ -- --nocapture` passed with `88 passed`.
- Next implementation: continue DB/proof/RAG/TUI contracts for remaining downstream gaps; implementing real nested closure/async owners remains a larger model slice, not a fixture-only change.

## 2026-06-25 04:00 UTC - DB closure/async target-centered exclusion

- Branch/HEAD: same branch at `71e6a7a7`; active autonomous call-graph goal remains open.
- Changed: added a fixture-backed `ploke-db` target-centered contract proving `callers_for_target(local_target)` and target-seeded `expand_call_context` do not surface closure, move-closure, async-block, or async-closure outer owners for body-local `local_target()` calls.
- Invariant: the explicit closure/async owner boundary must hold in both owner-scoped and target-centered DB helper APIs; downstream expansion must not infer callers from body calls that have no persisted enclosing-owner call edge.
- Verified: focused `cargo test -p ploke-db --features call_graph fixture_callers_for_target_excludes_closure_or_async_body_outer_owners -- --nocapture` passed with `1 passed`; fixture module `cargo test -p ploke-db --features call_graph call_graph_fixture_queries -- --nocapture` passed with `69 passed`; broad DB `cargo test -p ploke-db --features call_graph call_graph_ -- --nocapture` passed with `89 passed`.
- Next implementation: continue downstream call-context/proof contracts from proven DB rows; real nested closure/async owners still require a larger parser/modeling design.

## 2026-06-25 04:05 UTC - Proof closure/async target-centered exclusion

- Branch/HEAD: same branch at `71e6a7a7`; active autonomous call-graph goal remains open.
- Changed: added fixture-backed proof projection coverage proving target-centered `local_target` proof facts do not include closure, move-closure, async-block, or async-closure outer owners for body-local calls.
- Invariant: proof projection must inherit the DB call graph owner boundary; generated proof edges and proof facts must not fabricate detached-process evidence from closure/async body calls that are not projected onto the enclosing owner.
- Verified: focused `cargo test -p ploke-db --features call_graph fixture_projection_excludes_closure_async_outer_owners_from_target_proof -- --nocapture` passed with `1 passed`; fixture module `cargo test -p ploke-db --features call_graph call_graph_fixture_queries -- --nocapture` passed with `70 passed`; broad DB `cargo test -p ploke-db --features call_graph call_graph_ -- --nocapture` passed with `90 passed`.
- Next implementation: continue downstream contracts or move into the larger nested closure/async owner model deliberately; do not treat nested body ownership as already implemented.

## 2026-06-25 04:08 UTC - RAG closure/async target expansion exclusion

- Branch/HEAD: same branch at `71e6a7a7`; active autonomous call-graph goal remains open.
- Changed: added fixture-backed RAG call-context expansion coverage proving `local_target` target expansion still materializes real dynamic callers while excluding closure, move-closure, async-block, and async-closure outer owners.
- Invariant: RAG expansion must preserve the DB owner boundary and must not attach call context to enclosing owners for closure/async body calls that were deliberately not projected as outer-owner calls.
- Verified: focused `cargo test -p ploke-rag --features call_graph call_context_expansion_excludes_closure_async_outer_owners_for_local_target -- --nocapture` passed with `1 passed`; broader `cargo test -p ploke-rag --features call_graph call_context -- --nocapture` passed with `20 passed`; DB sanity `cargo test -p ploke-db --features call_graph call_graph_ -- --nocapture` passed with `90 passed`.
- Next implementation: continue downstream public surface or proof contracts from proven DB rows; nested closure/async ownership remains a larger parser/modeling slice.

## 2026-06-25 04:13 UTC - Public RAG closure/async target expansion exclusion

- Branch/HEAD: same branch at `71e6a7a7`; active autonomous call-graph goal remains open.
- Changed: added public sparse `get_context` coverage for `local_target` target expansion using the stable `pub fn local_target` query. The test proves final assembled context materializes a real dynamic caller with incoming-caller provenance while excluding closure, move-closure, async-block, and async-closure outer owners.
- Invariant: public RAG assembly must preserve the same owner-boundary guarantees as DB and private RAG expansion; closure/async body calls must not surface as expanded caller parts.
- Verified: focused `cargo test -p ploke-rag --features call_graph call_context_sparse_get_context_excludes_closure_async_outer_owners_for_local_target -- --nocapture` passed with `1 passed`; broader `cargo test -p ploke-rag --features call_graph call_context -- --nocapture` passed with `21 passed`; DB sanity `cargo test -p ploke-db --features call_graph call_graph_ -- --nocapture` passed with `90 passed`.
- Next implementation: continue public surface/proof contracts or move deliberately into larger parser/modeling gaps; fixture ungating remains blocked on backup review/regeneration.

## 2026-06-25 04:22 UTC - DB unsupported dynamic proof sibling coverage

- Branch/HEAD: same branch at `71e6a7a7`; active autonomous call-graph goal remains open.
- Changed: broadened the fixture-backed unsupported dynamic proof projection contract so both opaque closure-binding cast and dereferenced closure-binding calls project as `dynamic_dispatch_unbounded` blockers.
- Invariant: closure-binding cast/deref calls must remain targetless at proof projection time, preserve source provenance, and fabricate no proof checker edges until closure/Fn semantic target modeling exists.
- Verified: focused `cargo test -p ploke-db --features call_graph unsupported_dynamic_call_without_edges -- --nocapture` passed with `1 passed`; fixture module `cargo test -p ploke-db --features call_graph call_graph_fixture_queries -- --nocapture` passed with `70 passed`; broad DB `cargo test -p ploke-db --features call_graph call_graph_ -- --nocapture` passed with `90 passed`.
- Next implementation: keep shifting parser-green cases into DB/proof/RAG contracts; the next likely DB slice is a strict proof/query contract for remaining branch/match dynamic failures or another downstream gap already green in `syn_parser`.

## 2026-06-25 04:31 UTC - DB branch/match dynamic proof failure coverage

- Branch/HEAD: same branch at `71e6a7a7`; active autonomous call-graph goal remains open.
- Changed: added fixture-backed proof projection coverage for ambiguous if/match dynamic callees and guarded, opaque, and nested branch/match dynamic callees that already have parser and owner-context DB coverage.
- Invariant: ambiguous branch/match dynamic callees must project as `type_resolution_missing` blockers, unsupported guarded/opaque/nested branch/match callees must project as `dynamic_dispatch_unbounded` blockers, and none may fabricate proof checker edges.
- Verified: focused `cargo test -p ploke-db --features call_graph branch_and_match_dynamic_failures -- --nocapture` passed with `1 passed`; fixture module `cargo test -p ploke-db --features call_graph call_graph_fixture_queries -- --nocapture` passed with `71 passed`; broad DB `cargo test -p ploke-db --features call_graph call_graph_ -- --nocapture` passed with `91 passed`.
- Next implementation: continue turning parser-green DB owner rows into proof/RAG/TUI contracts, or move deliberately into the larger closure/async owner model once downstream contracts are saturated.

## 2026-06-25 04:38 UTC - DB resolved branch/match dynamic proof coverage

- Branch/HEAD: same branch at `71e6a7a7`; active autonomous call-graph goal remains open.
- Changed: added fixture-backed proof projection coverage for same-target if/match dynamic callees that already resolve to `local_target` in parser and owner-context DB tests.
- Invariant: same-target branch/match dynamic callees must project as resolved proof edges to `local_target`, preserve source provenance, and produce no `dynamic_dispatch_unbounded` blockers.
- Verified: focused `cargo test -p ploke-db --features call_graph branch_and_match_dynamic_call_proof -- --nocapture` passed with `1 passed`; fixture module `cargo test -p ploke-db --features call_graph call_graph_fixture_queries -- --nocapture` passed with `72 passed`; broad DB `cargo test -p ploke-db --features call_graph call_graph_ -- --nocapture` passed with `92 passed`.
- Next implementation: continue checking parser-green DB owner rows for missing proof/RAG/TUI contracts; backup fixture ungating still requires explicit review/regeneration approval.

## 2026-06-25 04:46 UTC - DB callable-expression dynamic proof coverage

- Branch/HEAD: same branch at `71e6a7a7`; active autonomous call-graph goal remains open.
- Changed: added fixture-backed proof projection coverage for resolved parenthesized path/binding dynamic callees, function-pointer cast/deref callees, block callees, and indexed-array dynamic callees.
- Invariant: exact dynamic callable-expression shapes that resolve to `local_target` must project as resolved `DynamicFunction` proof edges with source provenance and must not emit `dynamic_dispatch_unbounded` blockers.
- Verified: focused `cargo test -p ploke-db --features call_graph callable_expression_dynamic_call_proof -- --nocapture` passed with `1 passed`; fixture module `cargo test -p ploke-db --features call_graph call_graph_fixture_queries -- --nocapture` passed with `73 passed`; broad DB `cargo test -p ploke-db --features call_graph call_graph_ -- --nocapture` passed with `93 passed`.
- Next implementation: scan remaining DB owner-context rows for proof/RAG/TUI gaps, especially field/tuple-field dynamic families and low-level proof fact shape checks that are still only represented by one example.

## 2026-06-25 04:54 UTC - DB field dynamic proof coverage

- Branch/HEAD: same branch at `71e6a7a7`; active autonomous call-graph goal remains open.
- Changed: added fixture-backed proof projection coverage for resolved named-field, indexed named-field, and indexed tuple-field dynamic callees.
- Invariant: exact field and tuple-field dynamic callable shapes that resolve to `local_target` must project as resolved `DynamicFunction` proof edges with source provenance; tuple-constructor setup calls may add their own proof facts, but the dynamic field edge must still be present and no dynamic-dispatch blocker may be emitted.
- Verified: focused `cargo test -p ploke-db --features call_graph field_dynamic_call_proof -- --nocapture` passed with `1 passed`; fixture module `cargo test -p ploke-db --features call_graph call_graph_fixture_queries -- --nocapture` passed with `74 passed`; broad DB `cargo test -p ploke-db --features call_graph call_graph_ -- --nocapture` passed with `94 passed`.
- Next implementation: the DB proof layer now covers the main resolved and fail-closed dynamic families; continue with remaining downstream proof/RAG/TUI gaps or deliberately move into closure/async owner modeling.

## 2026-06-25 05:02 UTC - DB path-resolution proof coverage

- Branch/HEAD: same branch at `71e6a7a7`; active autonomous call-graph goal remains open.
- Changed: added fixture-backed proof projection coverage for resolved function path calls across local, self/super/crate/module-qualified, import-alias, glob, re-export, imported-module, and grouped-import forms.
- Invariant: every exact function path resolution form that persists as a resolved DB call edge must project as a resolved proof edge with source provenance and no `type_resolution_missing` blocker.
- Verified: focused `cargo test -p ploke-db --features call_graph path_resolution_call_proof -- --nocapture` passed with `1 passed`; fixture module `cargo test -p ploke-db --features call_graph call_graph_fixture_queries -- --nocapture` passed with `75 passed`; broad DB `cargo test -p ploke-db --features call_graph call_graph_ -- --nocapture` passed with `95 passed`.
- Next implementation: continue filling owner-scoped proof coverage for associated-function/import/type-alias path families, then move outward to any remaining RAG/TUI surfaces.

## 2026-06-25 05:10 UTC - DB associated-function proof coverage

- Branch/HEAD: same branch at `71e6a7a7`; active autonomous call-graph goal remains open.
- Changed: added fixture-backed proof projection coverage for resolved associated-function path calls across inherent `Self::make`/`Type::make`, qualified paths, imported types, local/imported type aliases, method-as-associated syntax, and local/imported/re-exported/grouped trait associated functions.
- Invariant: every exact associated-function path form that persists as a resolved DB call edge must project as a resolved proof edge to the method target with source provenance and no `type_resolution_missing` blocker.
- Verified: exact focused `cargo test -p ploke-db --features call_graph fixture_projection_stores_real_associated_function_call_proof_facts -- --nocapture` passed with `1 passed`; fixture module `cargo test -p ploke-db --features call_graph call_graph_fixture_queries -- --nocapture` passed with `76 passed`; broad DB `cargo test -p ploke-db --features call_graph call_graph_ -- --nocapture` passed with `96 passed`.
- Note: the first attempted focused filter, `associated_function_call_proof`, matched zero tests while still exiting successfully; the exact function-name filter above is the authoritative focused result.
- Next implementation: owner-scoped proof coverage now spans the main function/associated-function/dynamic families; continue with remaining method receiver proof families or downstream RAG/TUI contracts.

## 2026-06-25 05:18 UTC - DB local receiver method proof coverage

- Branch/HEAD: same branch at `71e6a7a7`; active autonomous call-graph goal remains open.
- Changed: added fixture-backed proof projection coverage for resolved inherent method calls on local, initialized-local, typed-local, type-alias, borrowed, and dereferenced receivers.
- Invariant: every exact local receiver method form that persists as a resolved DB method edge to `LocalAssoc::instance_value` must project as a resolved proof edge with source provenance and no `type_resolution_missing` blocker.
- Verified: focused `cargo test -p ploke-db --features call_graph local_receiver_method_call_proof -- --nocapture` passed with `1 passed`; fixture module `cargo test -p ploke-db --features call_graph call_graph_fixture_queries -- --nocapture` passed with `77 passed`; broad DB `cargo test -p ploke-db --features call_graph call_graph_ -- --nocapture` passed with `97 passed`.
- Next implementation: continue owner-scoped proof coverage for trait/generic/blanket/imported method receiver families, then move outward to any remaining RAG/TUI contracts.

## 2026-06-25 05:26 UTC - DB trait-family method proof coverage

- Branch/HEAD: same branch at `71e6a7a7`; active autonomous call-graph goal remains open.
- Changed: added fixture-backed proof projection coverage for resolved generic-bound, impl-trait, trait-object, imported-trait, constrained-generic-self, and exact blanket trait method calls.
- Invariant: every exact trait-family method form that persists as a resolved DB method edge must project as a resolved proof edge with source provenance and no `type_resolution_missing` blocker.
- Verified: focused `cargo test -p ploke-db --features call_graph trait_family_method_call_proof -- --nocapture` passed with `1 passed`; fixture module `cargo test -p ploke-db --features call_graph call_graph_fixture_queries -- --nocapture` passed with `78 passed`; broad DB `cargo test -p ploke-db --features call_graph call_graph_ -- --nocapture` passed with `98 passed`.
- Next implementation: owner-scoped proof coverage now spans the main method receiver families; continue with result/field receiver method chains and any remaining downstream RAG/TUI surfaces.

## 2026-06-25 05:34 UTC - DB result and field receiver method proof coverage

- Branch/HEAD: same branch at `71e6a7a7`; active autonomous call-graph goal remains open.
- Changed: added fixture-backed proof projection coverage for path-call result, method-call result, await-result, and tuple-field receiver method chains, including the inner resolved call edges that share each owner.
- Invariant: multi-row owners with exact result/field receiver method calls must project every resolved inner and outer proof edge with source provenance and no `type_resolution_missing` blocker.
- Verified: focused `cargo test -p ploke-db --features call_graph result_and_field_receiver_method_call_proof -- --nocapture` passed with `1 passed`; fixture module `cargo test -p ploke-db --features call_graph call_graph_fixture_queries -- --nocapture` passed with `79 passed`; broad DB `cargo test -p ploke-db --features call_graph call_graph_ -- --nocapture` passed with `99 passed`.
- Next implementation: owner-scoped proof coverage is now broad for path, associated-function, dynamic, and method families; continue with downstream RAG/TUI coverage or deliberate closure/async body-owner modeling.

## 2026-06-25 05:13 UTC - Public RAG associated-function target expansion

- Branch/HEAD: same branch at `71e6a7a7`; active autonomous call-graph goal remains open.
- Changed: added public sparse `get_context` coverage for associated-function target hits by seeding the `LocalAssocFunctionTrait::trait_make` trait method with the stable `233 trait_make` query.
- Invariant: the public RAG path must carry target-centered associated-function call expansion from sparse retrieval through final context assembly, preserving incoming-caller provenance and the outgoing `AssociatedFunction` edge for `call_trait_associated_function`.
- Verified: focused `cargo test -p ploke-rag --features call_graph call_context_sparse_get_context_expands_associated_function_target_hits_to_fixture_callers -- --nocapture` passed with `1 passed`; broader `cargo test -p ploke-rag --features call_graph call_context -- --nocapture` passed with `22 passed`; DB sanity `cargo test -p ploke-db --features call_graph call_graph_ -- --nocapture` passed with `99 passed`.
- Note: an earlier attempted `LocalAssoc::make` sparse query was too ambiguous across the fixture's `make` family, so the final public test uses the unique trait-associated function body literal while the private expansion test continues to cover `LocalAssoc::make` caller fanout.
- Next implementation: continue downstream TUI/tool coverage for this associated-function target shape or move into the next unpinned RAG/TUI proof carrier before tackling closure/async body-owner modeling.

## 2026-06-25 05:18 UTC - TUI associated-function tool carrier coverage

- Branch/HEAD: same branch at `71e6a7a7`; active autonomous call-graph goal remains open.
- Changed: extended the `request_code_context` tool JSON roundtrip contract with a fixture-shaped `LocalAssocFunctionTrait::trait_make` path call carrying an `AssociatedFunction` target.
- Invariant: public tool payload serialization must preserve associated-function call-context rows through the `ContextPart -> ConciseContext -> RequestCodeContextResult` carrier path, alongside the existing method, dynamic-function, constructor, external, macro, and ambiguous rows.
- Verified: focused `cargo test -p ploke-tui --features call_graph serde_roundtrip_request_code_context -- --nocapture` passed with `1 passed`; broader `cargo test -p ploke-tui --features call_graph tool_io_roundtrip -- --nocapture` passed with `3 passed`.
- Next implementation: continue checking model-facing TUI formatting/overlay surfaces for any newly pinned RAG call-context shape, then return to the remaining closure/async body-owner and fixture-rollout gates.

## 2026-06-25 05:22 UTC - DB target-centered associated-function proof coverage

- Branch/HEAD: same branch at `71e6a7a7`; active autonomous call-graph goal remains open.
- Changed: added fixture-backed target-centered proof projection coverage for the real `LocalAssoc::make` associated-function target.
- Invariant: `project_call_proof_facts_for_target(...)` must store resolved proof edges and source provenance for multiple incoming associated-function path callers, while deriving the proof fact count from `callers_for_target(...)` and emitting no unrelated type-resolution blockers.
- Verified: focused `cargo test -p ploke-db --features call_graph fixture_projection_stores_real_target_centered_associated_function_call_proof_facts -- --nocapture` passed with `1 passed`; fixture module `cargo test -p ploke-db --features call_graph call_graph_fixture_queries -- --nocapture` passed with `80 passed`; broad DB `cargo test -p ploke-db --features call_graph call_graph_ -- --nocapture` passed with `100 passed`.
- Next implementation: continue closing remaining downstream proof/RAG/TUI contracts that are backed by real persisted rows; larger parser/modeling slices remain function-pointer/generic `FnOnce`, richer trait dispatch, and closure/async body ownership.

## 2026-06-25 05:27 UTC - DB imported trait associated-function target proof coverage

- Branch/HEAD: same branch at `71e6a7a7`; active autonomous call-graph goal remains open.
- Changed: added fixture-backed target-centered proof projection coverage for the real `ImportedAssocFunctionTrait::imported_trait_make` associated-function target.
- Invariant: imported trait associated-function target projection must preserve resolved proof edges and source provenance for direct, alias, glob, re-export, and grouped-import shorthand callers without fabricating blocker facts.
- Verified: focused `cargo test -p ploke-db --features call_graph fixture_projection_stores_real_target_centered_imported_trait_assoc_function_call_proof_facts -- --nocapture` passed with `1 passed`; fixture module `cargo test -p ploke-db --features call_graph call_graph_fixture_queries -- --nocapture` passed with `81 passed`; broad DB `cargo test -p ploke-db --features call_graph call_graph_ -- --nocapture` passed with `101 passed`.
- Next implementation: continue with downstream proof/RAG/TUI contracts backed by real rows, or start a deliberate resolver/modeling slice for function-pointer/generic `FnOnce`, richer trait dispatch, or closure/async body ownership.

## 2026-06-25 05:33 UTC - Public RAG imported trait associated-function expansion

- Branch/HEAD: same branch at `71e6a7a7`; active autonomous call-graph goal remains open.
- Changed: added public sparse `get_context` coverage for the real `ImportedAssocFunctionTrait::imported_trait_make` associated-function target.
- Invariant: the public RAG path must carry imported trait associated-function target expansion from sparse retrieval through final context assembly, preserving direct, alias, glob, re-export, and grouped-import caller owners with outgoing `AssociatedFunction` rows and incoming-caller provenance.
- Verified: focused `cargo test -p ploke-rag --features call_graph call_context_sparse_get_context_expands_imported_trait_associated_function_target_hits_to_fixture_callers -- --nocapture` passed with `1 passed`; broader `cargo test -p ploke-rag --features call_graph call_context -- --nocapture` passed with `23 passed`; DB sanity `cargo test -p ploke-db --features call_graph call_graph_ -- --nocapture` passed with `101 passed`.
- Next implementation: continue downstream TUI/tool carrier coverage for this imported trait associated-function expansion shape, or move to the next DB-backed proof/RAG contract before larger resolver/modeling work.

## 2026-06-25 05:42 UTC - DB call-relation endpoint-family validation

- Branch/HEAD: same branch at `71e6a7a7`; active autonomous call-graph goal remains open.
- Changed: tightened `ploke-db` call-graph read helpers so `call_targets_for_site` and `callers_for_target` only surface relation rows whose `source_kind` matches the actual call-site kind and whose `relation_kind/source_kind/target_kind` tuple matches the parser's typed `CallRelation` families. Added synthetic regression coverage that injects malformed persisted `call_relation` rows directly.
- Invariant: DB query helpers must not expose impossible call-relation endpoint families to call-context expansion, target-centered callers, proof projection, RAG, or TUI consumers, even if malformed rows exist in storage.
- Verified: focused `cargo test -p ploke-db --features call_graph call_graph_queries_exclude_invalid_endpoint_family_rows -- --nocapture` passed with `1 passed`; synthetic DB suite `cargo test -p ploke-db --features call_graph call_graph_queries -- --nocapture` passed with `21 passed`; broad DB `cargo test -p ploke-db --features call_graph call_graph_ -- --nocapture` passed with `102 passed`; downstream sanity `cargo test -p ploke-rag --features call_graph call_context -- --nocapture` passed with `23 passed`.
- Next implementation: continue DB-first contracts for any remaining persisted call families, then move outward only where the DB surface is already pinned.

## 2026-06-25 05:49 UTC - DB call-resolution status source-kind validation

- Branch/HEAD: same branch at `71e6a7a7`; active autonomous call-graph goal remains open.
- Changed: tightened `ploke-db::Database::call_resolution_for_site(...)` so a persisted `call_resolution_status.source_kind` must match the actual `call_site.call_kind` before the status row is returned. Added synthetic regression coverage that injects a mismatched resolved status row directly.
- Invariant: call-context expansion, target-centered callers, proof projection, RAG, and TUI consumers must not trust a resolved/unresolved status row whose endpoint family disagrees with the stored call-site family.
- Verified: focused `cargo test -p ploke-db --features call_graph call_graph_queries_reject_status_source_kind_mismatch -- --nocapture` passed with `1 passed`; synthetic DB suite `cargo test -p ploke-db --features call_graph call_graph_queries -- --nocapture` passed with `22 passed`; broad DB `cargo test -p ploke-db --features call_graph call_graph_ -- --nocapture` passed with `103 passed`; downstream sanity `cargo test -p ploke-rag --features call_graph call_context -- --nocapture` passed with `23 passed`.
- Next implementation: continue DB-first contracts for persisted call graph consistency, then move outward only where the DB surface is already pinned.

## 2026-06-25 05:53 UTC - DB call-site edge target-kind validation

- Branch/HEAD: same branch at `71e6a7a7`; active autonomous call-graph goal remains open.
- Changed: tightened `ploke-db` owner and target-centered call-site queries so a persisted `BodyContainsCall` edge must have a `target_kind` matching the actual `call_site.call_kind` before the call site is exposed. Added synthetic regression coverage that injects a mismatched `call_site_edge` directly.
- Invariant: owner context, target-centered callers, proof projection, RAG, and TUI consumers must not assemble a call site from a `BodyContainsCall` row whose endpoint family disagrees with the stored call-site family.
- Verified: focused `cargo test -p ploke-db --features call_graph call_graph_queries_exclude_body_contains_call_site_kind_mismatch -- --nocapture` passed with `1 passed`; synthetic DB suite `cargo test -p ploke-db --features call_graph call_graph_queries -- --nocapture` passed with `23 passed`; broad DB `cargo test -p ploke-db --features call_graph call_graph_ -- --nocapture` passed with `104 passed`; downstream sanity `cargo test -p ploke-rag --features call_graph call_context -- --nocapture` passed with `23 passed`.
- Next implementation: continue DB-first contracts for persisted call graph consistency, then move outward only where the DB surface is already pinned.

## 2026-06-25 06:00 UTC - TUI imported trait associated-function tool carrier coverage

- Branch/HEAD: same branch at `71e6a7a7`; active autonomous call-graph goal remains open.
- Changed: extended the `request_code_context` tool JSON roundtrip contract with a fixture-shaped imported trait associated-function path call, `VisibleAssocFunctionTrait::imported_trait_make`, carrying an `AssociatedFunction` target.
- Invariant: public tool payload serialization must preserve imported trait associated-function call-context rows through the `ContextPart -> ConciseContext -> RequestCodeContextResult` carrier path, alongside local trait associated-function, method, dynamic-function, constructor, external, macro, and ambiguous rows.
- Verified: focused `cargo test -p ploke-tui --features call_graph serde_roundtrip_request_code_context -- --nocapture` passed with `1 passed`; broader `cargo test -p ploke-tui --features call_graph tool_io_roundtrip -- --nocapture` passed with `3 passed`.
- Next implementation: return focus to `ploke-db` query/proof contracts for persisted call graph consistency, using TUI/RAG coverage only after the DB surface is pinned.

## 2026-06-25 06:07 UTC - DB call-site edge source-owner-kind validation

- Branch/HEAD: same branch at `71e6a7a7`; active autonomous call-graph goal remains open.
- Changed: tightened `ploke-db` owner and target-centered call-site queries so a persisted `BodyContainsCall.source_kind` must match the actual stored owner node family (`function`, `method`, `const`, or `static`) before the call site is exposed. Added synthetic regression coverage that injects a `BodyContainsCall` with a mismatched source owner kind.
- Invariant: owner context, target-centered callers, proof projection, RAG, and TUI consumers must not assemble call context from a `BodyContainsCall` row whose source-owner family disagrees with the stored owner node.
- Verified: focused `cargo test -p ploke-db --features call_graph call_graph_queries_exclude_body_contains_owner_kind_mismatch -- --nocapture` failed before the query fix and passed after it; synthetic DB suite `cargo test -p ploke-db --features call_graph call_graph_queries -- --nocapture` passed with `24 passed`; broad DB `cargo test -p ploke-db --features call_graph call_graph_ -- --nocapture` passed with `105 passed`; downstream RAG `cargo test -p ploke-rag --features call_graph call_context -- --nocapture` passed with `23 passed` after updating its synthetic fixture owner setup.
- Next implementation: continue DB-first persisted consistency contracts, then move outward only after the relevant DB surface is pinned.

## 2026-06-25 06:17 UTC - DB malformed call-site shape validation

- Branch/HEAD: same branch at `71e6a7a7`; active autonomous call-graph goal remains open.
- Changed: tightened `ploke-db` call-site decoding so persisted `call_site` rows must match the syntactic shape implied by `call_kind`; for example, a `Path` row with a leaked `method_name` is rejected instead of being assembled into call context. Added synthetic regression coverage for that malformed Path row.
- Invariant: owner context, target-centered callers, proof projection, RAG, and TUI consumers must not receive contradictory callee payloads from malformed persisted `call_site` rows.
- Verified: focused `cargo test -p ploke-db --features call_graph call_graph_queries_reject_malformed_call_site_shape -- --nocapture` failed before the decoder fix and passed after it; synthetic DB suite `cargo test -p ploke-db --features call_graph call_graph_queries -- --nocapture` passed with `25 passed`; broad DB `cargo test -p ploke-db --features call_graph call_graph_ -- --nocapture` passed with `106 passed`; downstream RAG `cargo test -p ploke-rag --features call_graph call_context -- --nocapture` passed with `23 passed`.
- Next implementation: continue DB-first persisted consistency contracts, then use downstream RAG/TUI checks only after each DB surface is pinned.

## 2026-06-25 06:23 UTC - DB call-resolution status/resolution validation

- Branch/HEAD: same branch at `71e6a7a7`; active autonomous call-graph goal remains open.
- Changed: tightened `ploke-db` call-resolution decoding so `Resolved` rows must carry `LocalExact`, while `Unresolved`, `Ambiguous`, `External`, and `Unsupported` rows must not carry a `resolution_kind`. Added synthetic regression coverage for both malformed directions.
- Invariant: owner context, target-centered callers, proof projection, RAG, and TUI consumers must not receive contradictory resolution payloads from malformed persisted `call_resolution_status` rows.
- Verified: focused `cargo test -p ploke-db --features call_graph call_graph_queries_reject_status_resolution_kind_mismatch -- --nocapture` failed before the decoder fix and passed after it; synthetic DB suite `cargo test -p ploke-db --features call_graph call_graph_queries -- --nocapture` passed with `26 passed`; broad DB `cargo test -p ploke-db --features call_graph call_graph_ -- --nocapture` passed with `107 passed`; downstream RAG `cargo test -p ploke-rag --features call_graph call_context -- --nocapture` passed with `23 passed`.
- Next implementation: continue DB-first persisted consistency contracts, then use downstream RAG/TUI checks only after each DB surface is pinned.

## 2026-06-25 06:30 UTC - DB resolved target cardinality validation

- Branch/HEAD: same branch at `71e6a7a7`; active autonomous call-graph goal remains open.
- Changed: tightened `ploke-db::Database::call_context_for_owner(...)` so `Resolved(LocalExact)` rows must have exactly one valid target before owner context or owner-seeded expansion can use them. Added synthetic regression coverage for both zero-target and two-target resolved rows, and reshaped an expansion fixture so two outgoing callees are represented by two resolved call sites instead of one impossible multi-target site.
- Invariant: owner context, owner-seeded expansion, proof projection, RAG, and TUI consumers must not receive a resolved local call payload with zero or multiple targets.
- Verified: focused `cargo test -p ploke-db --features call_graph call_graph_queries_reject_resolved_target_cardinality_mismatch -- --nocapture` failed before the helper fix and passed after it; synthetic DB suite `cargo test -p ploke-db --features call_graph call_graph_queries -- --nocapture` passed with `27 passed`; broad DB `cargo test -p ploke-db --features call_graph call_graph_ -- --nocapture` passed with `108 passed`; downstream RAG `cargo test -p ploke-rag --features call_graph call_context -- --nocapture` passed with `23 passed`.
- Next implementation: continue DB-first persisted consistency contracts, then use downstream RAG/TUI checks only after each DB surface is pinned.

## 2026-06-25 06:41 UTC - DB fixture relation invariant coverage

- Branch/HEAD: same branch at `71e6a7a7`; active autonomous call-graph goal remains open.
- Changed: added fixture-backed `ploke-db` invariants over raw transformed call graph rows. The new coverage asserts every persisted `call_site` has exactly one matching `BodyContainsCall` edge with the same owner, owner kind, and call kind; every persisted call site has exactly one matching `call_resolution_status`; and every persisted `call_relation` source/target pair is anchored to an existing call site and endpoint node with a valid endpoint family.
- Invariant: fresh parser -> transform -> DB projection must not only be queryable through typed helpers; its stored call graph relation families must be internally self-consistent before downstream RAG, TUI, or proof consumers rely on them.
- Verified: focused `cargo test -p ploke-db --features call_graph fixture_projected_call_ -- --nocapture` failed first on incorrect test-helper query bindings and then passed with `2 passed`; fixture module `cargo test -p ploke-db --features call_graph call_graph_fixture_queries -- --nocapture` passed with `83 passed`; broad DB `cargo test -p ploke-db --features call_graph call_graph_ -- --nocapture` passed with `110 passed`.
- Next implementation: keep the DB-first focus by adding any missing fixture invariants for real persisted call graph/proof projection, then move outward to RAG/TUI only where DB contracts are already pinned.

## 2026-06-25 06:46 UTC - DB fixture inverse relation invariant coverage

- Branch/HEAD: same branch at `71e6a7a7`; active autonomous call-graph goal remains open.
- Changed: added fixture-backed inverse invariants over raw transformed `BodyContainsCall` and `call_resolution_status` rows. The new coverage iterates stored edge/status rows directly and requires each one to point back to an existing call-site owner, call site, owner family, call kind, and valid status/resolution shape.
- Invariant: transformed call graph storage must not contain orphaned call-site containment or resolution-status rows that typed helper queries would simply fail to discover by starting from `call_site`.
- Verified: focused `cargo test -p ploke-db --features call_graph fixture_projected_call_edge_and_status_rows_have_existing_anchors -- --nocapture` passed with `1 passed`; fixture module `cargo test -p ploke-db --features call_graph call_graph_fixture_queries -- --nocapture` passed with `84 passed`; broad DB `cargo test -p ploke-db --features call_graph call_graph_ -- --nocapture` passed with `111 passed`.
- Next implementation: continue DB-first with any remaining raw storage/proof invariants, then move outward to RAG/TUI only where the DB contracts are pinned.

## 2026-06-25 06:51 UTC - DB fixture status/relation cardinality coverage

- Branch/HEAD: same branch at `71e6a7a7`; active autonomous call-graph goal remains open.
- Changed: added a fixture-backed raw cardinality invariant tying `call_resolution_status` rows to persisted `call_relation` rows. The new coverage sweeps every transformed `call_site`, requires one status row, then requires `Resolved(LocalExact)` sites to have exactly one semantic target row and non-resolved sites to have none.
- Invariant: resolved local calls and unresolved/external/unsupported/ambiguous calls must be separated in storage, not only after typed helper filtering; transformed DB rows must not silently carry target rows for non-resolved statuses or omit target rows for resolved statuses.
- Verified: focused `cargo test -p ploke-db --features call_graph fixture_projected_status_rows_match_relation_cardinality -- --nocapture` passed with `1 passed`; fixture module `cargo test -p ploke-db --features call_graph call_graph_fixture_queries -- --nocapture` passed with `85 passed`; broad DB `cargo test -p ploke-db --features call_graph call_graph_ -- --nocapture` passed with `112 passed`.
- Next implementation: continue DB-first with proof-fact storage/query invariants or move outward to RAG/TUI only where DB contracts are pinned.

## 2026-06-25 06:58 UTC - DB fixture proof-row linkage coverage

- Branch/HEAD: same branch at `71e6a7a7`; active autonomous call-graph goal remains open.
- Changed: added fixture-backed proof-fact linkage coverage for the real mixed owner `call_try_result_instance_method`. The new test projects owner-scoped proof facts, compares fact count to DB call context, and verifies each call site has one `call_site` fact and one `call_resolution` fact; resolved rows have exactly one `call_edge` fact matching the DB target; non-resolved rows have no edge and carry a blocker reason.
- Invariant: proof projection must preserve DB call-context semantics without fabricating proof edges for non-resolved rows or dropping proof facts for resolved rows.
- Verified: focused `cargo test -p ploke-db --features call_graph fixture_projection_links_mixed_owner_proof_rows_to_call_context -- --nocapture` passed with `1 passed`; fixture module `cargo test -p ploke-db --features call_graph call_graph_fixture_queries -- --nocapture` passed with `86 passed`; broad DB `cargo test -p ploke-db --features call_graph call_graph_ -- --nocapture` passed with `113 passed`.
- Next implementation: continue DB/proof invariants for target-centered proof projection, then move outward to RAG/TUI only where DB contracts are pinned.

## 2026-06-25 07:07 UTC - DB remaining receiver decoder coverage

- Branch/HEAD: same branch at `71e6a7a7`; active autonomous call-graph goal remains open.
- Changed: added synthetic `ploke-db` helper coverage for the remaining method receiver payload encodings exposed by `call_context_for_owner`, including `SelfValue`, borrowed/dereferenced local receivers, field receivers, path/method result receivers, await/try result receivers, and literal receivers. Added a small raw receiver helper for test setup so receiver kinds with null receiver paths can be represented directly.
- Invariant: typed DB call-context decoding must preserve the distinct receiver classifier emitted by parser/transform rows instead of collapsing unsupported or pathless receiver payloads into generic local bindings.
- Verified: focused `cargo test -p ploke-db --features call_graph context_for_owner_decodes_remaining_method_receivers -- --nocapture` passed with `1 passed`; broad DB `cargo test -p ploke-db --features call_graph call_graph_ -- --nocapture` passed with `114 passed`.
- Next implementation: continue DB-first call graph contracts around target-centered proof/query behavior, then move outward only where the DB surface is pinned.

## 2026-06-25 07:10 UTC - DB target-centered proof-row linkage coverage

- Branch/HEAD: same branch at `71e6a7a7`; active autonomous call-graph goal remains open.
- Changed: added fixture-backed target-centered proof-fact linkage coverage for the real `local_target` incoming caller set. The new test projects target-scoped proof facts, compares fact count to `callers_for_target`, and verifies each caller call site has one `call_site` fact, one `call_resolution` fact, and one `call_edge` fact matching the caller owner and seed target.
- Invariant: target-centered proof projection must preserve DB caller semantics without dropping call facts, fabricating unrelated blockers, or mismatching caller/callee IDs across incoming rows.
- Verified: focused `cargo test -p ploke-db --features call_graph fixture_projection_links_target_centered_proof_rows_to_callers -- --nocapture` passed with `1 passed`; fixture module `cargo test -p ploke-db --features call_graph call_graph_fixture_queries -- --nocapture` passed with `87 passed`; broad DB `cargo test -p ploke-db --features call_graph call_graph_ -- --nocapture` passed with `115 passed`.
- Next implementation: continue DB-first call graph contracts around target-centered query/proof behavior or move to RAG/TUI only where DB contracts are already pinned.

## 2026-06-25 07:19 UTC - DB target-centered proof provenance linkage

- Branch/HEAD: same branch at `71e6a7a7`; active autonomous call-graph goal remains open.
- Changed: strengthened the target-centered `local_target` proof linkage test so projected proof rows must contain no unrelated rows and every incoming caller call site must have source provenance matching its persisted call-site span.
- Invariant: target-centered proof projection must preserve both row membership and source provenance for every caller row, not only caller/callee IDs.
- Verified: focused `cargo test -p ploke-db --features call_graph fixture_projection_links_target_centered_proof_rows_to_callers -- --nocapture` passed with `1 passed`; fixture module `cargo test -p ploke-db --features call_graph call_graph_fixture_queries -- --nocapture` passed with `87 passed`; broad DB `cargo test -p ploke-db --features call_graph call_graph_ -- --nocapture` passed with `115 passed`.
- Next implementation: continue DB-first query/proof contracts or move outward to RAG/TUI only where the DB surface is already pinned.

## 2026-06-25 07:26 UTC - DB call-site identity universe invariant

- Branch/HEAD: same branch at `71e6a7a7`; active autonomous call-graph goal remains open.
- Changed: added a fixture-backed raw DB invariant proving transformed `call_site.id` values do not overlap stored code-node IDs or type-use/type IDs. The test checks the fresh `fixture_call_graph` parser -> transform -> DB projection directly, alongside the existing raw relation anchor checks.
- Invariant: DB projection must preserve call occurrences as `CallId` identities and must not leak call-site IDs into `NodeId`, `TypeUseId`, or `TypeId` endpoint families.
- Verified: focused `cargo test -p ploke-db --features call_graph fixture_projected_call_site_ids_do_not_overlap_node_or_type_ids -- --nocapture` passed with `1 passed`; fixture module `cargo test -p ploke-db --features call_graph call_graph_fixture_queries -- --nocapture` passed with `88 passed`; broad DB `cargo test -p ploke-db --features call_graph call_graph_ -- --nocapture` passed with `116 passed`.
- Next implementation: continue DB-first contracts where identity, relation, and proof invariants are not pinned; then move outward to RAG/TUI only where the DB surface is already proven.

## 2026-06-25 07:33 UTC - DB fixture proof blockers feed invariants

- Branch/HEAD: same branch at `71e6a7a7`; active autonomous call-graph goal remains open.
- Changed: added fixture-backed proof-invariant coverage for a real projected external call blocker. The test projects the `call_prelude_string_new` external call into proof facts, adds proof-only process-effect evidence for that call site, and asserts `proof_invariant_findings` reports the detached-process handoff invariant as blocked by `external_dependency_summary_missing`.
- Invariant: fixture-derived call-graph `call_resolution` blocker facts must participate in the proof invariant checker after storage, not only in `proof_graphrag_context` lookup.
- Verified: focused `cargo test -p ploke-db --features call_graph fixture_projected_external_call_blocker_feeds_proof_invariants -- --nocapture` passed with `1 passed`; fixture module `cargo test -p ploke-db --features call_graph call_graph_fixture_queries -- --nocapture` passed with `89 passed`; broad DB `cargo test -p ploke-db --features call_graph call_graph_ -- --nocapture` passed with `117 passed`.
- Next implementation: continue DB-first proof/query contracts or move outward to RAG/TUI only where the DB surface is already proven.

## 2026-06-25 07:43 UTC - DB proof symbol lookup fixture linkage

- Branch/HEAD: same branch at `71e6a7a7`; active autonomous call-graph goal remains open.
- Changed: added fixture-backed `proof_symbol_lookup` coverage for a real resolved call edge. The test projects `call_crate_local_target` proof facts and queries by the real `local_target` callee id, then verifies the lookup returns the matching `call_edge` plus linked `call_site` and `call_resolution` rows by call-site identity.
- Invariant: symbol lookup over proof facts must preserve the same call-site linkage contract as GraphRAG proof lookup for real projected call graph facts.
- Verified: focused `cargo test -p ploke-db --features call_graph fixture_proof_symbol_lookup_links_real_resolved_call_facts -- --nocapture` passed with `1 passed`; fixture module `cargo test -p ploke-db --features call_graph call_graph_fixture_queries -- --nocapture` passed with `90 passed`; broad DB `cargo test -p ploke-db --features call_graph call_graph_ -- --nocapture` passed with `118 passed`.
- Next implementation: continue DB/proof query contracts or move outward to RAG/TUI only where the DB surface is already proven.

## 2026-06-25 07:48 UTC - DB expansion call-site provenance

- Branch/HEAD: same branch at `71e6a7a7`; active autonomous call-graph goal remains open.
- Changed: strengthened the fixture-backed `expand_call_context` DB test for the real `local_target` incoming caller set. The test now cross-checks `call_context_for_owner`, `callers_for_target`, and `expand_call_context` so owner-seeded outgoing expansion and target-seeded incoming expansion preserve the exact persisted `call_site.id` for the ordinary `crate::local_target()` path caller and the resolved `(local_target)()` dynamic caller.
- Invariant: DB call-context expansion must carry exact call-site provenance from persisted rows, not only the reachable owner/target IDs that downstream RAG/TUI surfaces display.
- Verified: focused `cargo test -p ploke-db --features call_graph fixture_expand_call_context_reads_real_outgoing_and_incoming_candidates -- --nocapture` passed with `1 passed`; broad DB `cargo test -p ploke-db --features call_graph call_graph_ -- --nocapture` passed with `118 passed`.
- Next implementation: continue DB-first query/proof contracts until broad DB coverage is stable, then move outward only where the DB surface is already pinned.

## 2026-06-25 07:55 UTC - DB target-centered proof symbol lookup

- Branch/HEAD: same branch at `71e6a7a7`; active autonomous call-graph goal remains open.
- Changed: added fixture-backed `proof_symbol_lookup` coverage after target-centered projection of the real `local_target` incoming caller set. The test projects all caller facts for the target, queries by the callee id, and verifies linked `call_site`, `call_edge`, and `call_resolution` rows for both the ordinary `crate::local_target()` path caller and the resolved `(local_target)()` dynamic caller.
- Invariant: proof symbol lookup must preserve target-centered call-site linkage across multiple incoming callers, including dynamic call edges, not only single owner-scoped projections.
- Verified: focused `cargo test -p ploke-db --features call_graph fixture_proof_symbol_lookup_links_target_centered_local_target_facts -- --nocapture` passed with `1 passed`; fixture module `cargo test -p ploke-db --features call_graph call_graph_fixture_queries -- --nocapture` passed with `91 passed`; broad DB `cargo test -p ploke-db --features call_graph call_graph_ -- --nocapture` passed with `119 passed`.
- Next implementation: continue DB-first proof/query contracts or move outward only where the DB surface is already pinned.

## 2026-06-25 08:01 UTC - RAG public local_target caller provenance

- Branch/HEAD: same branch at `71e6a7a7`; active autonomous call-graph goal remains open.
- Changed: strengthened the public sparse `get_context` test for `local_target` target-centered expansion. The test now requires final assembled context to include both the ordinary `crate::local_target()` path caller and the resolved dynamic caller, with outgoing call-context rows and `IncomingCaller` expansion provenance tied to each persisted call-site ID, while still excluding closure/async outer owners.
- Invariant: DB call-site provenance must survive through public RAG context assembly, not only private expansion helpers.
- Verified: focused `cargo test -p ploke-rag --features call_graph call_context_sparse_get_context_excludes_closure_async_outer_owners_for_local_target -- --nocapture` passed with `1 passed`; broad RAG `cargo test -p ploke-rag --features call_graph call_context -- --nocapture` passed with `23 passed`.
- Next implementation: continue downstream RAG/TUI call-context assertions where DB contracts are already pinned, then return to parser/resolver only for strict behavior gaps.

## 2026-06-25 08:05 UTC - TUI tool payload path/dynamic caller carriers

- Branch/HEAD: same branch at `71e6a7a7`; active autonomous call-graph goal remains open.
- Changed: strengthened the `request_code_context` tool I/O roundtrip so `RequestCodeContextResult::from_assembled` and JSON serde preserve separate incoming caller context parts for an ordinary `crate::local_target()` path call and a resolved dynamic call. Each part carries outgoing call context plus `IncomingCaller` expansion provenance tied to the matching call-site ID.
- Invariant: once RAG assembles path and dynamic caller parts, the model-visible TUI tool payload must preserve both call-context and call-expansion carrier fields without collapsing them into one row or dropping call-site provenance.
- Verified: focused `cargo test -p ploke-tui --features call_graph tool_io_roundtrip -- --nocapture` passed with `3 passed`; broad TUI `cargo test -p ploke-tui --features call_graph call_context -- --nocapture` passed with `10 passed`.
- Next implementation: continue downstream TUI/model-visible call-context coverage where useful, or return to parser/resolver behavior gaps only where a strict local proof is available.

## 2026-06-25 08:11 UTC - DB proof projection build-domain invariant

- Branch/HEAD: same branch at `71e6a7a7`; active autonomous call-graph goal remains open.
- Changed: added synthetic `ploke-db` proof-projection coverage requiring owner-scoped and target-centered call-proof generation/projection to reject an empty `build_domain_id`.
- Invariant: call-graph proof facts must carry an explicit build-domain lineage before entering the proof store; failed projection attempts must not leave partial proof rows behind.
- Verified: focused `cargo test -p ploke-db --features call_graph proof_projection_requires_non_empty_build_domain_id -- --nocapture` passed with `1 passed`; broad DB `cargo test -p ploke-db --features call_graph call_graph_ -- --nocapture` passed with `120 passed` in `tests/mod.rs`.
- Next implementation: continue DB-first proof/query contracts where small invariants remain, otherwise move back to resolver behavior gaps only when a strict fixture proof is available.

## 2026-06-25 08:15 UTC - DB proof projection source-provenance invariant

- Branch/HEAD: same branch at `71e6a7a7`; active autonomous call-graph goal remains open.
- Changed: added synthetic `ploke-db` proof-projection coverage requiring owner-scoped and target-centered projection to reject call sites whose caller owner cannot be mapped to one source file.
- Invariant: proof facts must retain source provenance for navigation/checking; missing source ancestry must fail closed before any partial proof rows are stored.
- Verified: focused `cargo test -p ploke-db --features call_graph proof_projection_requires_source_provenance_before_storage -- --nocapture` passed with `1 passed`; broad DB `cargo test -p ploke-db --features call_graph call_graph_ -- --nocapture` passed with `121 passed` in `tests/mod.rs`.
- Next implementation: continue DB-first proof/query contracts where strict metadata invariants remain, otherwise return to resolver behavior gaps with fixture-backed RED tests.

## 2026-06-25 08:24 UTC - DB target-centered missing-status invariant

- Branch/HEAD: same branch at `71e6a7a7`; active autonomous call-graph goal remains open.
- Changed: added synthetic `ploke-db` coverage requiring `callers_for_target` and target-seeded `expand_call_context` to reject incoming call relations whose call site has no `call_resolution_status`.
- Invariant: target-centered DB helpers must fail closed on missing status rows instead of silently returning or promoting incomplete call graph relations.
- Verified: focused `cargo test -p ploke-db --features call_graph callers_for_target_rejects_missing_status -- --nocapture` passed with `1 passed`; broad DB `cargo test -p ploke-db --features call_graph call_graph_ -- --nocapture` passed with `122 passed` in `tests/mod.rs`.
- Next implementation: continue DB-first invariant coverage where strict query/proof metadata branches remain; otherwise move outward only where the DB surface is already pinned.

## 2026-06-25 08:32 UTC - DB target-centered resolved-cardinality invariant

- Branch/HEAD: same branch at `71e6a7a7`; active autonomous call-graph goal remains open.
- Changed: added synthetic `ploke-db` coverage and a production helper check requiring `callers_for_target` and target-seeded `expand_call_context` to reject `Resolved(LocalExact)` call sites with more than one valid semantic target.
- Invariant: target-centered DB helpers must enforce the same resolved-call cardinality rule as owner-scoped context before downstream RAG/proof expansion can promote caller rows.
- Verified: focused RED `cargo test -p ploke-db --features call_graph callers_for_target_rejects_resolved_target_cardinality_mismatch -- --nocapture` failed by returning one partial caller row; after the fix, the same focused command passed with `1 passed`; broad DB `cargo test -p ploke-db --features call_graph call_graph_ -- --nocapture` passed with `123 passed` in `tests/mod.rs`.
- Next implementation: continue DB-first invariant coverage where strict query/proof branches remain; otherwise shift outward to RAG/TUI only where the DB surface is already pinned.

## 2026-06-25 08:38 UTC - DB target-centered proof source prevalidation

- Branch/HEAD: same branch at `71e6a7a7`; active autonomous call-graph goal remains open.
- Changed: added synthetic `ploke-db` proof coverage for a target-centered caller set containing one caller with source provenance followed by another resolved caller without source provenance.
- Invariant: target-centered call-proof generation/projection must prevalidate every caller source before storage; a later missing-source caller must not leave partial proof facts for earlier valid callers.
- Verified: focused `cargo test -p ploke-db --features call_graph target_centered_proof_projection_prevalidates_caller_sources_before_storage -- --nocapture` passed with `1 passed`; broad DB `cargo test -p ploke-db --features call_graph call_graph_ -- --nocapture` passed with `124 passed` in `tests/mod.rs`.
- Next implementation: continue DB-first proof/query contracts if a strict invariant branch remains; otherwise move outward to RAG/TUI production-path coverage where DB behavior is already pinned.

## 2026-06-25 09:02 UTC - DB call target endpoint-existence invariant

- Branch/HEAD: same branch at `71e6a7a7`; active autonomous call-graph goal remains open.
- Changed: added synthetic `ploke-db` coverage requiring owner-scoped targets, target-centered callers, and `expand_call_context` to ignore `call_relation` rows whose declared target endpoint row is missing.
- Invariant: call graph DB helpers must validate both endpoint family labels and actual endpoint relation membership before surfacing or promoting semantic call targets.
- Verified: focused RED `cargo test -p ploke-db --features call_graph call_graph_queries_exclude_missing_endpoint_target_rows -- --nocapture` failed by returning two target rows; after adding `valid_target` endpoint joins in `call_targets_for_site` and `callers_for_target`, the same focused command passed with `1 passed`. Module checks passed: `cargo test -p ploke-db --features call_graph call_graph_queries -- --nocapture` with `34 passed`, and `cargo test -p ploke-db --features call_graph call_graph_fixture_queries -- --nocapture` with `91 passed`.
- Note: a follow-up slice updated the stale `fixture_nodes` embeddable count expectation exposed by the unfiltered `ploke-db` feature run.
- Next implementation: continue DB/RAG/TUI work only where contracts are pinned; the remaining unfiltered `ploke-db` feature failures are backup-fixture regeneration gate failures.

## 2026-06-25 09:09 UTC - DB fixture_nodes embeddable-count baseline

- Branch/HEAD: same branch at `71e6a7a7`; active autonomous call-graph goal remains open.
- Changed: updated the exact `fixture_nodes` common embeddable-node count in `ploke-db` from `152` to `153`; the non-file count remains derived from that value minus the existing 10 file nodes.
- Invariant: multi-embedding tests still assert exact fixture cardinality; the baseline now matches the current parser/modeling output instead of failing one node low.
- Verified: focused `cargo test -p ploke-db --features call_graph multi_embedding::db_ext::tests::test_get_common_nodes -- --nocapture` passed with `1 passed`; focused `cargo test -p ploke-db --features call_graph multi_embedding::db_ext::tests::multi_pending_embeddings_count_basic -- --nocapture` passed with `1 passed`.
- Broader status: unfiltered `cargo test -p ploke-db --features call_graph -- --nocapture` now passes `src/lib.rs` with `98 passed`, `callsite_logging_tests` with `1 passed`, and `debug_obsv` with `1 passed`; it remains red only in backup-backed type-graph tests under `tests/mod.rs` with `195 passed`, `34 failed`, all failing on missing `call_relation` in registered backups.
- Next implementation: do not loosen backup import behavior; fixture regeneration/review is still required before removing `CALL_GRAPH_GATE:db-projection`.

## 2026-06-25 09:31 UTC - TUI request_code_context expanded caller payload

- Branch/HEAD: same branch at `71e6a7a7`; active autonomous call-graph goal remains open.
- Changed: added direct `request_code_context` production-tool coverage over a fresh `fixture_call_graph` database. The test seeds sparse retrieval with the `LocalAssoc::instance_value` method target and requires the returned model-visible `RequestCodeContextResult` to materialize an `IncomingCaller` part with the outgoing method call-context row and matching call-site provenance.
- Fixed: `RagService::collect_call_context` now always collects call-context rows for call-expanded owners that survive into final assembled hits, even when ordinary retrieval/type-context hits occupy the default `max_owner_hits` collection window first.
- Fixed: the synthetic RAG call-context unit helper now seeds matching endpoint rows for declared semantic target families (`function`, `method`, `struct`, `variant`) so tests satisfy the stricter DB target-existence invariant instead of creating dangling resolved relations.
- Invariant: if RAG returns a call-expanded caller part, the tool payload must include the outgoing call row that explains the expansion; `call_expansion` without its corresponding call-site payload is incomplete model context.
- Verified: focused RED `cargo test -p ploke-tui --features "call_graph test_harness" request_code_context_returns_method_target_callers_with_call_context -- --nocapture` first failed with the expanded method caller present but `call_context: []`; after the RAG fix and tightening the test to the deterministic production-default caller, the same focused command passed with `1 passed`. Focused RAG check `cargo test -p ploke-rag --features call_graph call_context_sparse_get_context_expands_method_target_hits_to_fixture_callers -- --nocapture` passed with `1 passed`. Broader RAG filter initially exposed stale synthetic target setup in `call_context_collection_attaches_outgoing_call_payloads`; after endpoint seeding, focused `cargo test -p ploke-rag --features call_graph call_context_collection_attaches_outgoing_call_payloads -- --nocapture` passed with `1 passed`, and broad `cargo test -p ploke-rag --features call_graph call_context -- --nocapture` passed with `23 passed`. Broad TUI `cargo test -p ploke-tui --features "call_graph test_harness" call_context -- --nocapture` passed with `11 passed`.
- Next implementation: keep moving outward only where DB/RAG contracts are pinned; the backup-fixture regeneration gate remains the known broad feature-run blocker.

## 2026-06-25 09:25 UTC - DB constructor target expansion coverage

- Branch/HEAD: same branch at `71e6a7a7`; active autonomous call-graph goal remains open.
- Changed: added fixture-backed `ploke-db` coverage requiring target-seeded `callers_for_target` and `expand_call_context` to preserve real constructor caller rows for both `TupleStructConstructor` (`fixture_call_graph::NewType(value)`) and `EnumVariantConstructor` (`fixture_nodes::EnumWithData::Variant1(1)`).
- Invariant: constructor call relations use typed `Struct`/`Variant` endpoint families in target-centered queries; they must not be treated as ordinary function callees or dropped from incoming caller expansion.
- Verified: focused `cargo test -p ploke-db --features call_graph fixture_expand_call_context_target_seed_preserves_constructor_callers -- --nocapture` passed with `1 passed`. The post-slice broad fixture filter `cargo test -p ploke-db --features call_graph call_graph_fixture_queries -- --nocapture` passed with `92 passed`.
- Next implementation: continue DB-first only for uncovered helper/proof contracts; backup-import relation filtering remains fixture-review-gated because `docs/testing/BACKUP_DB_FIXTURES.md` was last reviewed on 2026-06-12.

## 2026-06-25 09:29 UTC - DB target-centered constructor proof coverage

- Branch/HEAD: same branch at `71e6a7a7`; active autonomous call-graph goal remains open.
- Changed: added fixture-backed proof projection coverage requiring `project_call_proof_facts_for_target(...)` to preserve real constructor incoming caller proof edges for `NewType(value)` and `EnumWithData::Variant1(1)`.
- Invariant: target-centered proof projection must carry constructor callees through their typed `Struct` and `Variant` endpoint families with source provenance; constructor proof edges must not be fabricated as ordinary function edges.
- Verified: focused `cargo test -p ploke-db --features call_graph fixture_projection_stores_real_target_centered_constructor_call_proof_facts -- --nocapture` passed with `1 passed`; broad `cargo test -p ploke-db --features call_graph call_graph_fixture_queries -- --nocapture` passed with `93 passed`.
- Next implementation: continue DB/proof coverage only for remaining helper families not yet covered; backup-fixture import behavior remains fixture-review-gated.

## 2026-06-25 09:43 UTC - Consolidated constructor DB/RAG fixture coverage

- Branch/HEAD: same branch at `71e6a7a7`; active autonomous call-graph goal remains open.
- Changed: consolidated constructor fixture/proof assertions around a shared `ConstructorCase` table covering the tuple-struct constructor in `fixture_call_graph` and the enum-variant constructor in `fixture_nodes`. The same case table now drives target-seeded expansion, owner-scoped proof projection, and target-centered proof projection assertions.
- Added: RAG helper coverage now seeds constructor targets directly and proves incoming caller expansion materializes the real constructor caller owners with outgoing `TupleStructConstructor` / `EnumVariantConstructor` call-context rows.
- Inventory: next batched DB/proof consolidation candidates are the repeated target-centered method-family cases: ordinary method target, associated-function target, imported trait associated-function target, and trait-dispatch target. Backup-backed type-graph failures that report missing `call_relation` stay under `CALL_GRAPH_GATE:fixture-regeneration`; do not loosen import/schema validation for them.
- Verified: exact DB batch passed one test at a time: `fixture_expand_call_context_target_seed_preserves_constructor_callers`, `fixture_projection_stores_real_constructor_call_proof_facts`, and `fixture_projection_stores_real_target_centered_constructor_call_proof_facts`. Focused RAG constructor expansion `cargo test -p ploke-rag --features call_graph call_context_expansion_adds_incoming_fixture_constructor_callers -- --nocapture` passed with `1 passed`.

## 2026-06-25 09:49 UTC - Consolidated target-centered method-family proof helpers

- Branch/HEAD: same branch at `71e6a7a7`; active autonomous call-graph goal remains open.
- Changed: extracted shared target-centered proof assertions for resolved caller sets, proof-fact counts, checker edges, blocker absence, and source provenance. The ordinary method, associated-function, imported trait associated-function, and trait-dispatch target-centered proof tests now use the same helper while keeping their family-specific caller relation checks.
- Invariant: target-centered proof projection must keep rejecting unrelated blockers and must keep every emitted checker edge resolved to the seed target; helper consolidation must not weaken endpoint-family assertions for method or associated-function caller rows.
- Verified: exact DB batch passed one test at a time: `fixture_projection_stores_real_target_centered_method_call_proof_facts`, `fixture_projection_stores_real_target_centered_associated_function_call_proof_facts`, `fixture_projection_stores_real_target_centered_imported_trait_assoc_function_call_proof_facts`, and `fixture_projection_stores_real_target_centered_trait_dispatch_call_proof_facts`.
- Next implementation: continue inventory-driven batching; likely next DB/proof cleanup is owner-scoped method-family proof helper reuse or a deliberate backup-fixture review/regeneration slice if explicitly approved.
