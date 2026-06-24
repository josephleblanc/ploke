# Handoff Changelog

Active running handoff notes for Codex/user continuity. Keep this file under 120 lines or 10 entries; archive to `docs/archive/agents/handoffs/` when it grows.

## 2026-06-23 23:16 UTC - call-graph checkpoint

- Branch: `prototype1-parent-mwv-live-r16-dangling-symlink-surface-fix-20260615t003337z-gen0`
- HEAD: `3be9c78a Make typed type graph baseline`
- Active goal: continue the Ploke call-graph plan autonomously while preserving strict resolver/schema/import/fixture invariants and leaving `call_graph` gated until fixture review/regeneration.
- Changed this session: expanded `fixture_call_graph` and paranoid call-site coverage from 65 to 80 strict cases; added coverage for imported trait associated-function shorthand, module-qualified crate/self/super paths, method-as-associated syntax, typed function-pointer alias flow, fail-closed generic/boxed Fn value bindings, generic turbofish function calls, and raw identifier function/method calls. Updated `docs/active/agents/call-graph/README.md` and the coverage matrix accordingly.
- Verified: `cargo test -p syn_parser --features call_graph call_sites -- --nocapture` passed with `80 passed`; latest `cargo fmt --all` and `git diff --check` passed before the final parser run.
- Earlier in this checkpoint: downstream feature checks passed after the 77-case state: `cargo test -p ploke-transform --features call_graph transform::tests -- --nocapture`, `cargo test -p ploke-db --features call_graph call_graph_queries -- --nocapture`, and `cargo test -p ploke-rag --features call_graph call_context_collection_attaches_outgoing_call_payloads -- --nocapture`. Rerun these next because super/raw identifier rows were added after that downstream pass.
- GitNexus: latest detect-changes before final super/raw slices reported low risk, 0 affected processes. Rerun `npx gitnexus detect-changes` before any final handoff/commit.
- Dirty state: broad in-progress call-graph branch remains dirty across parser, transform, DB, RAG, TUI, docs, and fixtures; `AGENTS.md` is also dirty and was pre-existing/user-owned. Do not revert unrelated files.
- Gate/blocker: `CALL_GRAPH_GATE:db-projection` and `CALL_GRAPH_GATE:fixture-regeneration` remain active. `docs/testing/BACKUP_DB_FIXTURES.md` was last reviewed 2026-06-12, so fixture review is overdue; ask before regenerating/changing backup fixtures.
- Next non-gated slices: add strict coverage for `drop(x)` / prelude builtins as unsupported or external only if proven; add method turbofish `value.method::<T>()` preserving `generic_arg_count`; add explicit fail-closed rows for `(&value).method()` / `(*value).method()` before any autoderef work; add `crate::documented_macro!()` macro path coverage; scan `fixture_edge_cases` before adding more raw syntax fixtures.
- Larger next slices: closure/async body ownership requires a nested owner design; prelude/alloc shorthand external classification (`Box::new`) needs a separate resolver design; fixture regeneration/gate removal needs explicit approval.

## 2026-06-23 23:18 UTC - stop point

- Branch/HEAD unchanged: `prototype1-parent-mwv-live-r16-dangling-symlink-surface-fix-20260615t003337z-gen0` at `3be9c78a Make typed type graph baseline`.
- No additional implementation was started after the checkpoint above; the active call-graph goal remains open.
- Current state remains dirty with the broad call-graph work plus pre-existing/user-owned `AGENTS.md`. Leave it intact on resume.
- Immediate verification on resume: rerun downstream checks after the final 80-case parser additions: `cargo test -p ploke-transform --features call_graph transform::tests -- --nocapture`, `cargo test -p ploke-db --features call_graph call_graph_queries -- --nocapture`, and `cargo test -p ploke-rag --features call_graph call_context_collection_attaches_outgoing_call_payloads -- --nocapture`.
- Before finalizing or committing: rerun `git diff --check`, `cargo fmt --all` if any code changed, and `npx gitnexus detect-changes`.
- Recommended implementation order: first add narrow strict coverage for method turbofish and prelude/drop fail-closed behavior; then macro path coverage; then decide whether to design closure/async body ownership or prelude/alloc shorthand classification. Keep backup fixture regeneration and call-graph gate removal approval-gated.

## 2026-06-23 23:34 UTC - prelude call coverage slice

- Changed: expanded focused `fixture_call_graph` coverage from 80 to 84 strict call expressions: method turbofish on a typed local receiver, crate-qualified macro path invocation, prelude `drop(value)`, local `drop(1)` shadowing, and exact unshadowed `Box::new(...)` classification.
- Implemented: `resolve_path_call` now classifies exact prelude-shaped `drop` and unshadowed `Box::new` as `External` only after local lookup declines a local target; local `drop` still resolves as `CallRelation::Function`.
- Verified: `cargo test -p syn_parser --features call_graph call_sites -- --nocapture` passed with `84 passed`; downstream `cargo test -p ploke-transform --features call_graph transform::tests -- --nocapture`, `cargo test -p ploke-db --features call_graph call_graph_queries -- --nocapture`, and `cargo test -p ploke-rag --features call_graph call_context_collection_attaches_outgoing_call_payloads -- --nocapture` all passed.
- Hygiene: `cargo fmt --all` passed; `git diff --check` passed; `npx gitnexus detect-changes` reported low risk and 0 affected processes.
- Next: remaining non-gated parser/resolver slices include explicit ref/deref receiver boundaries, broader prelude/std shorthand policy beyond `drop` and `Box::new`, and closure/async nested owner design. Fixture regeneration and call-graph gate removal still require approval.

## 2026-06-23 23:47 UTC - receiver boundary and prelude constructor slice

- Changed: expanded focused `fixture_call_graph` coverage from 84 to 88 strict call expressions.
- Implemented: method receiver payloads now include borrowed and dereferenced local bindings; `(&value).instance_value()` and `(*value).instance_value()` are recorded structurally and fail closed as `Unsupported` before autoderef/autoref resolution. The new receiver payloads are projected through transform, decoded by `ploke-db`, and mapped into RAG payloads.
- Implemented: exact unshadowed `String::new()` and `Vec::new()` classify as `External` through the same local-first prelude path policy as `Box::new`.
- Verified: `cargo test -p syn_parser --features call_graph call_sites -- --nocapture` passed with `88 passed`; downstream `cargo test -p ploke-transform --features call_graph transform::tests -- --nocapture`, `cargo test -p ploke-db --features call_graph call_graph_queries -- --nocapture`, and `cargo test -p ploke-rag --features call_graph call_context_collection_attaches_outgoing_call_payloads -- --nocapture` all passed.
- Hygiene: `cargo fmt --all` passed; `git diff --check` passed; `npx gitnexus detect-changes` reported low risk and 0 affected processes.
- Next: remaining non-gated parser/resolver slices include chained/call receivers such as `make().method()` or `x.borrow().method()`, broader external std/prelude method classification, and closure/async nested owner design. Fixture regeneration and call-graph gate removal remain approval-gated.

## 2026-06-23 23:53 UTC - call-result receiver boundary slice

- Changed: expanded focused `fixture_call_graph` coverage from 88 to 90 strict call expressions.
- Implemented: method receiver payloads now include `PathCallResult` and `MethodCallResult`; `make_local_assoc().instance_value()` and `value.clone_assoc().instance_value()` record the outer method call structurally and fail closed as `Unsupported` before return-type method resolution. The payloads project through transform, decode in `ploke-db`, and map into RAG payloads.
- Verified: `cargo test -p syn_parser --features call_graph call_sites -- --nocapture` passed with `90 passed`; downstream `cargo test -p ploke-transform --features call_graph transform::tests -- --nocapture`, `cargo test -p ploke-db --features call_graph call_graph_queries -- --nocapture`, and `cargo test -p ploke-rag --features call_graph call_context_collection_attaches_outgoing_call_payloads -- --nocapture` all passed.
- Hygiene: `cargo fmt --all` passed; `git diff --check` passed; `npx gitnexus detect-changes` reported low risk and 0 affected processes.
- Next: remaining non-gated parser/resolver slices include tuple-field receivers, field-call ambiguity like `x.0()`, broader external std/prelude method classification, and closure/async nested owner design. Fixture regeneration and call-graph gate removal remain approval-gated.

## 2026-06-24 00:02 UTC - tuple-field receiver slice

- Changed: expanded focused `fixture_call_graph` coverage from 90 to 92 strict call expressions.
- Implemented: method receiver payloads now include field projections rooted at local bindings; `value.0.instance_value()` records as `FieldLocalBinding` and fails closed as `Unsupported` before tuple-field receiver typing. Dynamic field-call callees such as `value.0()` now record the field path and fail closed as `Unsupported` before function-field resolution.
- Verified: `cargo test -p syn_parser --features call_graph call_sites -- --nocapture` passed with `92 passed`; downstream `cargo test -p ploke-transform --features call_graph transform::tests -- --nocapture`, `cargo test -p ploke-db --features call_graph call_graph_queries -- --nocapture`, and `cargo test -p ploke-rag --features call_graph call_context_collection_attaches_outgoing_call_payloads -- --nocapture` all passed.
- Hygiene: `cargo fmt --all` passed; `git diff --check` passed; `npx gitnexus detect-changes` reported low risk and 0 affected processes.
- Next: remaining non-gated slices include await/try receiver boundaries, broader external std/prelude method classification, and closure/async nested owner design. Fixture regeneration and call-graph gate removal remain approval-gated.

## 2026-06-24 00:31 UTC - stop point at 107 call-site cases

- Branch/HEAD: `prototype1-parent-mwv-live-r16-dangling-symlink-surface-fix-20260615t003337z-gen0` at `3be9c78a Make typed type graph baseline`.
- Changed since the 92-case checkpoint: expanded strict parser coverage to 107 concrete call expressions; added await/try/literal receiver boundary payloads, local alias checker coverage, `self.value.len()` / `self.value.into()` self-field rows, `info!` / `debug!` macro rows, `Regex::new(...).unwrap()` rows, and registered `fixture_impls` coverage for struct-literal initialized-local method calls plus exact local associated-function syntax.
- Verified latest focused suite through sub-agent: `cargo test -p syn_parser --features call_graph call_sites -- --nocapture` passed with `107` tests; `cargo test -p ploke-transform --features call_graph transform::tests -- --nocapture` passed; `cargo test -p ploke-db --features call_graph call_graph_queries -- --nocapture` passed; `cargo test -p ploke-rag --features call_graph call_context_collection_attaches_outgoing_call_payloads -- --nocapture` passed.
- Hygiene at stop: `git diff --check` passed; `npx gitnexus detect-changes` reported low risk and 0 affected processes, including after this handoff append. No new implementation slice was started after the 107-case verification.
- Dirty state: broad call-graph implementation remains intentionally dirty across parser, transform, DB, RAG, TUI, docs, and fixtures; `AGENTS.md` is dirty and user-owned/pre-existing. Do not reset or clean.
- Gate/blocker: `CALL_GRAPH_GATE:db-projection` and fixture regeneration remain approval-gated; `docs/testing/BACKUP_DB_FIXTURES.md` review date is still overdue from 2026-06-12.
- Next small implementation slice: add the ready `fixture_impls` `println!("field_one: {}, field_two: {}", x.field_one, x.field_two)` macro row as strict parser coverage, update the call-graph README count to 108, update the coverage matrix, then run parser focused tests via sub-agent.
- Next after that: continue fixture-driven coverage before new designs: remaining ready macro/path rows, non-self receiver boundaries, associated constructor variants, tuple/enum constructors, and import/re-export/glob-aware path cases. Defer closure/async body ownership until there is a typed nested-owner design. Keep backup fixture regeneration and call-graph gate removal blocked on explicit user approval.

## 2026-06-24 00:50 UTC - stop point at 116 call-site cases

- Branch/HEAD: `prototype1-parent-mwv-live-r16-dangling-symlink-surface-fix-20260615t003337z-gen0` at `3be9c78a Make typed type graph baseline`.
- Active goal remains open: continue the full call-graph plan autonomously, preserve strict resolver/schema/import/fixture invariants, and keep `call_graph` gated until backup fixtures are reviewed/regenerated and default workspace verification is green.
- Changed since the 107-case checkpoint: expanded strict parser coverage to 116 concrete call expressions; added `fixture_impls` `println!(...)`; added `fixture_path_resolution` workspace/dependency external-root rows for `TypeId::Synthetic`, `NodeId::generate_synthetic`, `uuid::Uuid::nil`, and `.uuid()` receiver recording; fixed dependency-name external matching for hyphenated Cargo packages imported as underscore Rust crate paths; registered `fixture_edge_cases`; added `h.help()`, `uh.help()`, `"hello".to_string()`, and trait-impl `format!(...)` rows.
- Verified latest focused parser suite through sub-agent: `cargo test -p syn_parser --features call_graph call_sites -- --nocapture` passed with `116 passed; 0 failed`. Afterward, `cargo fmt --all`, `git diff --check`, and `npx gitnexus detect-changes` passed; GitNexus reported low risk and 0 affected processes.
- Not rerun after the 116-case parser slice: downstream `ploke-transform`, `ploke-db`, `ploke-rag`, and `ploke-tui` call-graph checks. Rerun them before treating the downstream projection as refreshed at 116 cases.
- Dirty state: broad call-graph implementation remains intentionally dirty across parser, transform, DB, RAG, TUI, docs, and fixtures; `AGENTS.md` is dirty and user-owned/pre-existing. Do not reset or clean.
- Gate/blocker: `CALL_GRAPH_GATE:db-projection` and fixture regeneration remain approval-gated; `docs/testing/BACKUP_DB_FIXTURES.md` review is overdue from 2026-06-12.
- Next small implementation slice: continue `fixture_edge_cases` with cfg-gated `internal::test_visibility` path calls, but first verify the exact parser-normalized cfg string for `#[cfg(not(feature = "type_bearing_ids"))]`. Candidate spans noted from the previous inspection: `utils::internal_helper()` `(2634, 2658)`, `utils::super_helper()` `(2674, 2695)`, and `restricted::restricted_func()` `(2711, 2740)`.
- Next after that: add generic-associated unsupported coverage such as `T::default()` if current behavior is confirmed; then scan/register `fixture_generics` and `fixture_type_resolution_v2` for existing generic/trait call rows before adding new fixture code. Keep closure/async body ownership deferred until there is a typed nested-owner design.

## 2026-06-24 01:15 UTC - stop point at 123 call-site cases

- Branch/HEAD: `prototype1-parent-mwv-live-r16-dangling-symlink-surface-fix-20260615t003337z-gen0` at `3be9c78a Make typed type graph baseline`.
- Active goal remains open: continue the full call-graph plan autonomously, preserve strict resolver/schema/import/fixture invariants, and keep `call_graph` gated until backup fixtures are reviewed/regenerated and default workspace verification is green.
- Changed since the 116-case checkpoint: strict parser coverage reached 123 concrete call expressions by adding cfg-gated `fixture_edge_cases::internal::test_visibility` rows, `GenericItem::new` `T::default()` unsupported coverage, `fixture_generics` generic/trait macro rows, `fixture_type_resolution_v2` associated-const `panic!` coverage, literal `.to_string()` external classification, and external path-call-result method classification for `Regex::new(...).unwrap()` / `NodeId::generate_synthetic(...).uuid()`.
- Reverted before stopping: an attempted semantic upgrade for `self.secret.len()` to `External` was incomplete and still produced `Unsupported`. The test/docs/resolver are back to the green fail-closed behavior: `SelfField { field_path: ["secret"] }` with `Unsupported` and no local edge.
- Verified latest focused parser suite through sub-agent: `cargo test -p syn_parser --features call_graph call_sites -- --nocapture` passed with `123 passed; 0 failed`; no warnings. `cargo fmt --all` also passed after reverting the incomplete slice.
- Downstream checks known green after the previous two production slices: `cargo test -p ploke-transform --features call_graph transform::tests -- --nocapture`, `cargo test -p ploke-db --features call_graph call_graph_queries -- --nocapture`, `cargo test -p ploke-rag --features call_graph call_context_collection_attaches_outgoing_call_payloads -- --nocapture`, and `cargo check -p ploke-tui --features call_graph`. Rerun them after the next production resolver change.
- Dirty state: broad call-graph implementation remains intentionally dirty across parser, transform, DB, RAG, TUI, docs, and fixtures; `AGENTS.md` is dirty and user-owned/pre-existing. Do not reset or clean.
- Gate/blocker: `CALL_GRAPH_GATE:db-projection` and fixture regeneration remain approval-gated; `docs/testing/BACKUP_DB_FIXTURES.md` review is overdue from 2026-06-12.
- Next slice: debug self-field receiver typing before changing behavior. Start with a temporary targeted inspection test for `fixture_nodes::PrivateStruct::get_secret_len` to print the owner impl `self_type`, the `PrivateStruct.secret` field `type_id`, and corresponding `TypeNode`; remove the debug test before committing. Only classify `self.secret.len()` as `External` if the concrete field type proof is explicit and local shadowing remains fail-closed.

## 2026-06-24 01:23 UTC - self-field external method slice

- Branch/HEAD: `prototype1-parent-mwv-live-r16-dangling-symlink-surface-fix-20260615t003337z-gen0` at `3be9c78a Make typed type graph baseline`.
- Changed: completed the previously reverted `self.secret.len()` semantic slice. `fixture_nodes_get_secret_len_records_self_field_len_external_method_call_site` now expects `External`, and the resolver proves the owner impl self type through existing `TypeRelation::Ordinary`, finds the local struct field type, and classifies exact external/prelude concrete `len` methods with no local edge.
- Guardrail: the self-field path still fails closed when owner is not an impl method, self type lacks a unique local struct target, field lookup is missing/ambiguous, field path is nested, method is not an allowed external method, or local `String`/`Vec` shadowing is visible. Generic `self.value.len()` and `self.value.into()` remain `Unsupported`.
- Verified through sub-agents: `cargo test -p syn_parser --features call_graph call_sites -- --nocapture` passed with `123 passed; 0 failed`; `cargo test -p ploke-transform --features call_graph transform::tests -- --nocapture` passed with `4 passed`; `cargo test -p ploke-db --features call_graph call_graph_queries -- --nocapture` passed with `8 passed`; `cargo test -p ploke-rag --features call_graph call_context_collection_attaches_outgoing_call_payloads -- --nocapture` passed with `1 passed`; `cargo check -p ploke-tui --features call_graph` passed.
- Hygiene: `cargo fmt --all`, `git diff --check`, and `npx gitnexus detect-changes` passed; GitNexus reported low risk and 0 affected processes.
- Next: continue non-gated semantic coverage from the matrix. Good candidates are exact tuple/local field receiver typing (`value.0.instance_value()`), exact await/try result receiver typing if return-type proof is already present, or broader external/prelude method classification. Keep closure/async body ownership and backup-fixture regeneration approval-gated.

## 2026-06-24 01:28 UTC - local path-result return-type method slice

- Branch/HEAD: `prototype1-parent-mwv-live-r16-dangling-symlink-surface-fix-20260615t003337z-gen0` at `3be9c78a Make typed type graph baseline`.
- Changed: completed exact local path-call-result receiver resolution for `make_local_assoc().instance_value()`. The strict row is now `fixture_call_graph_call_path_result_instance_method_resolves_returned_type_method_call_site` and expects `Resolved(LocalExact)` / `CallRelation::Method` for `LocalAssoc::instance_value`.
- Guardrail: the resolver only follows local function path results with a unique function target and a return type that has a unique `TypeRelation::Ordinary` target; external/imported path results, missing returns, unresolved/ambiguous function paths, unresolved/ambiguous return types, and non-matching methods still fail closed. `value.clone_assoc().instance_value()` remains `MethodCallResult`/`Unsupported`.
- Verified through sub-agents: `cargo test -p syn_parser --features call_graph call_sites -- --nocapture` passed with `123 passed`; `cargo test -p ploke-transform --features call_graph transform::tests -- --nocapture` passed with `4 passed`; `cargo test -p ploke-db --features call_graph call_graph_queries -- --nocapture` passed with `8 passed`; `cargo test -p ploke-rag --features call_graph call_context_collection_attaches_outgoing_call_payloads -- --nocapture` passed with `1 passed`; `cargo check -p ploke-tui --features call_graph` passed.
- Hygiene: `cargo fmt --all`, `git diff --check`, and `npx gitnexus detect-changes` passed; GitNexus reported low risk and 0 affected processes.
- Next: remaining non-gated candidates include method-call-result return-type resolution if receiver method return-type proof can be recovered, tuple-field receiver typing only after carrying root binding proof or otherwise proving the binding type safely, and broader external/prelude method summaries. Keep closure/async body ownership and backup-fixture regeneration approval-gated.
