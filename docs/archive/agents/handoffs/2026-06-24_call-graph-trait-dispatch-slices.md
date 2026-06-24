# Handoff Changelog

Active running handoff notes for Codex/user continuity. Keep this file under 120 lines or 10 entries; archive to `docs/archive/agents/handoffs/` when it grows.

Previous archives:
- `docs/archive/agents/handoffs/2026-06-24_call-graph-progress.md`
- `docs/archive/agents/handoffs/2026-06-24_call-graph-slices.md`
- `docs/archive/agents/handoffs/2026-06-24_call-graph-dynamic-branch-slices.md`
- `docs/archive/agents/handoffs/2026-06-24_call-graph-late-parser-slices.md`
- `docs/archive/agents/handoffs/2026-06-24_call-graph-parser-fnflow-slices.md`
- `docs/archive/agents/handoffs/2026-06-24_call-graph-reference-receiver-slices.md`

## 2026-06-24 19:10 UTC - borrowed-parameter implicit autoderef slice

- Branch/HEAD: `prototype1-parent-mwv-live-r16-dangling-symlink-surface-fix-20260615t003337z-gen0` at `3be9c78a Make typed type graph baseline`; active autonomous call-graph goal remains open.
- Changed: added `call_borrowed_param_instance_method(value: &LocalAssoc)` and made `value.instance_value()` resolve through one explicit reference layer on named owner parameters when the referenced local method target is proven exactly.
- Resolver: `resolve_param_method_call` now tries exact declared-type method resolution, then exact referenced-type method resolution for `Reference`/`Paren` type nodes, then the existing trait-bound fallback. This preserves existing `&dyn Trait` behavior while adding local-type implicit autoderef.
- Evidence: focused test first failed as `Unresolved`, then passed after resolver change. GitNexus impact for private helper was unavailable, so `resolve_method_call` was used as the indexed parent; risk was LOW with one direct upstream caller and no affected processes.
- Verified: focused `cargo test -p syn_parser --features call_graph borrowed_param_instance -- --nocapture` passed; standard bundle passed with parser `190 passed`, transform `5 passed`, DB call-graph queries `9 passed`, RAG call-context `1 passed`, and `cargo check -p ploke-tui --features call_graph`.
- Docs: call-graph README and coverage matrix now record 190 paranoid call-site cases and one explicit reference layer for borrowed owner-parameter method calls.
- Next implementation: continue with a behavior-expanding slice, likely concrete trait-object dispatch policy, broader `Fn`/`FnOnce` target modeling, or closure/async body ownership. Keep `call_graph` gated and avoid backup fixtures without explicit approval.

## 2026-06-24 19:16 UTC - referenced-local implicit autoderef slice

- Branch/HEAD: same branch at `3be9c78a`; active autonomous call-graph goal remains open.
- Changed: added `call_referenced_local_instance_method()` with `let value = &LocalAssoc; value.instance_value()` and classified direct referenced-local bindings as initialized-local method receiver proof.
- Implementation: `LocalBindingProof::Referenced { type_path }` now emits `MethodCallReceiver::InitializedLocalBinding { init_path: type_path }` for method calls only. Path-call and dynamic-call handling for referenced bindings remain conservative.
- Evidence: focused test first failed structurally with zero call-site matches, then passed after the classifier change. GitNexus impact for `classify_method_receiver` was LOW with no affected processes.
- Verified: focused `cargo test -p syn_parser --features call_graph referenced_local_instance -- --nocapture` passed; standard bundle passed with parser `191 passed`, transform `5 passed`, DB call-graph queries `9 passed`, RAG call-context `1 passed`, and `cargo check -p ploke-tui --features call_graph`.
- Docs: call-graph README and coverage matrix now record 191 paranoid call-site cases and direct referenced-local binding implicit autoderef.
- Next implementation: continue with a behavior-expanding slice, likely concrete trait-object dispatch policy, broader `Fn`/`FnOnce` target modeling, or closure/async body ownership. Keep `call_graph` gated and avoid backup fixtures without explicit approval.

## 2026-06-24 19:20 UTC - typed reference local receiver RED slice

- Branch/HEAD: same branch at `3be9c78a`; active autonomous call-graph goal remains open. The working tree already contains the broader dirty call-graph stack plus this in-progress fixture/test slice.
- Changed: added the next strict fixture case, `call_typed_reference_local_instance_method()`, with `let value: &LocalAssoc = &LocalAssoc; value.instance_value()`, plus a paranoid call-site expectation for `ExpectedMethodReceiver::TypedLocalBinding { name: "value", type_path: &["LocalAssoc"] }`.
- Evidence: focused sub-agent test `cargo test -p syn_parser --features call_graph typed_reference_local_instance -- --nocapture` failed RED as expected. The matcher found zero matching `instance_value()` call sites for regenerated ID `Method(MethodCallSiteId(Synthetic(e029b936-ffa7-54c9-81cd-7d3666416ad1)))`.
- Next implementation: run GitNexus impact before editing `local_binding_proof` or its indexed parent, then teach typed local binding extraction to recognize `&LocalAssoc`/parenthesized reference annotations by unwrapping one reference layer in the `Pat::Type` branch. Keep this scoped to typed local binding proof, not global `type_path_segments`, so other qself/type contexts stay conservative.
- Next verification: rerun the focused typed-reference-local test through a test sub-agent, then the standard call-graph bundle. If green, bump parser coverage docs from 191 to 192 and append the coverage-matrix row for `src/lib.rs:1060`.

## 2026-06-24 19:25 UTC - typed reference local receiver green slice

- Branch/HEAD: same branch at `3be9c78a`; active autonomous call-graph goal remains open.
- Changed: `local_binding_proof` now uses a typed-local-only helper that recognizes one explicit reference layer around a local type path, so `let value: &LocalAssoc = &LocalAssoc; value.instance_value()` records `TypedLocalBinding { type_path: ["LocalAssoc"] }` and resolves to the exact inherent method.
- Guardrails: GitNexus did not index private helpers, so impact was run on indexed parser boundaries. `extract_body_call_sites` and `classify_method_receiver` both reported LOW risk, one direct caller, and no affected processes.
- Verified: focused typed-reference-local sub-agent test passed; standard bundle passed with parser `192 passed`, transform `5 passed`, DB call-graph queries `9 passed`, RAG call-context `1 passed`, and `cargo check -p ploke-tui --features call_graph`.
- Docs: call-graph README and coverage matrix now record 192 paranoid call-site cases and the `src/lib.rs:1060` typed reference local receiver row.
- Next implementation: continue with another behavior-expanding RED slice, preferably one of the remaining receiver-resolution gaps not requiring backup fixture regeneration, such as parenthesized typed reference locals, mutable reference local receivers, or broader trait-object/concrete dispatch policy.

## 2026-06-24 19:33 UTC - local trait-object binding receiver slice

- Branch/HEAD: same branch at `3be9c78a`; active autonomous call-graph goal remains open.
- Changed: added `call_local_trait_object_binding_method(input: &dyn GenericBoundTrait)` with `let value: &dyn GenericBoundTrait = input; value.bound_value()`. Typed local extraction now recognizes exactly one trait-object bound path, including through one reference layer, and typed-local method resolution falls back to a local trait method declaration only when ordinary local type lookup is unresolved.
- Evidence: focused sub-agent test first failed structurally with zero `bound_value()` matches, then passed after parser/resolver changes. GitNexus impact used indexed boundaries because private helpers were not indexed: `extract_body_call_sites` and `resolve_method_call` both reported LOW risk with no affected processes.
- Verified: focused `cargo test -p syn_parser --features call_graph local_trait_object_binding -- --nocapture` passed; standard bundle passed with parser `193 passed`, transform `5 passed`, DB call-graph queries `9 passed`, RAG call-context `1 passed`, and `cargo check -p ploke-tui --features call_graph`.
- Docs: call-graph README and coverage matrix now record 193 paranoid call-site cases and the `src/lib.rs:1065` M17 local trait-object binding row.
- Next implementation: continue with recommended broader function-pointer/Fn/FnOnce or concrete trait-dispatch slices; avoid treating this declaration-edge trait-object support as concrete runtime dispatch.

## 2026-06-24 19:38 UTC - trait impl body same-impl self-call coverage

- Branch/HEAD: same branch at `3be9c78a`; active autonomous call-graph goal remains open.
- Changed: added `TraitImplBodyCallTrait` and an impl method body containing `self.required_impl_call()`, with a strict paranoid expectation owned by the trait impl method body and targeting the exact same-impl method definition.
- Evidence: focused sub-agent test `cargo test -p syn_parser --features call_graph trait_impl_method_body -- --nocapture` passed without production changes, proving the existing same-impl `self.method()` resolver path already covers this trait impl body shape. GitNexus impact for `resolve_method_call` and `extract_body_call_sites` remained LOW with no affected processes.
- Verified: standard bundle passed with parser `194 passed`, transform `5 passed`, DB call-graph queries `9 passed`, RAG call-context `1 passed`, and `cargo check -p ploke-tui --features call_graph`.
- Docs: call-graph README and coverage matrix now record 194 paranoid call-site cases and the `src/lib.rs:1080` trait impl body self-call row.
- Next implementation: continue with a true behavior-expanding slice, likely requiring new semantic modeling for Fn/FnOnce/FnPtr calls or a carefully bounded broader trait-dispatch case.

## 2026-06-24 19:46 UTC - direct concrete trait-object binding slice

- Branch/HEAD: same branch at `3be9c78a`; active autonomous call-graph goal remains open and should not be marked complete yet.
- Changed: added `call_concrete_trait_object_binding_method()` with `let value: &dyn LocalDispatchTrait = &TraitDispatchTarget; value.trait_value()`. Local trait-object binding proof now preserves a direct concrete reference initializer as `InitializedLocalBinding`, allowing the existing concrete local trait-impl resolver to target the exact `LocalDispatchTrait for TraitDispatchTarget` impl method.
- Guardrails: GitNexus impact was run on indexed boundaries before production edits: `classify_method_receiver`, `extract_body_call_sites`, and `resolve_method_call` all reported LOW risk with no affected processes.
- Verified: focused concrete-trait-object-binding sub-agent test passed; standard bundle passed with parser `195 passed`, transform `5 passed`, DB call-graph queries `9 passed`, RAG call-context `1 passed`, and `cargo check -p ploke-tui --features call_graph`.
- Docs: call-graph README and coverage matrix now record 195 paranoid call-site cases and the `src/lib.rs:1086` direct concrete trait-object binding row.
- Next implementation: prefer a bounded Fn/FnOnce/FnPtr semantic slice or closure/async nested-owner modeling. For trait dispatch, remaining useful slices are non-direct trait-object concrete proof, constrained blanket impl bound satisfaction, and broader imported trait scope forms. Keep `call_graph` gated and do not touch backup fixtures without explicit approval.

## 2026-06-24 19:53 UTC - constrained blanket impl trait dispatch slice

- Branch/HEAD: same branch at `3be9c78a`; active autonomous call-graph goal remains open.
- Changed: strengthened the existing inline-bound and where-bound blanket impl rows from fail-closed to exact resolution. `resolve_trait_impl_instance_method` now admits exact one-type-parameter constrained blanket impls only when every inline/where trait bound resolves to a local trait and the concrete receiver type has exact local impl evidence for each bound.
- Guardrails: private resolver helpers were not indexed. GitNexus impact on `resolve_method_call` was LOW with one direct caller and no affected processes; top-level `resolve_call_relations_after_tree` is CRITICAL because it feeds transform/ingest flows, so the implementation stayed inside the trait-impl candidate predicate.
- Verified: focused `cargo test -p syn_parser --features call_graph bound_blanket_trait_method -- --nocapture` passed; standard bundle passed with parser `195 passed`, transform `5 passed`, DB call-graph queries `9 passed`, RAG call-context `1 passed`, and `cargo check -p ploke-tui --features call_graph`.
- Docs: call-graph README and coverage matrix now record constrained one-parameter blanket impl resolution without changing the 195 parser test count.
- Next implementation: continue with a bounded Fn/FnOnce/FnPtr exact-proof slice, non-direct concrete trait-object proof, richer blanket-bound shapes, broader imported trait scope forms, or closure/async nested-owner modeling. Keep `call_graph` gated and avoid backup fixtures without explicit approval.

## 2026-06-24 19:58 UTC - aliased concrete trait-object proof slice

- Branch/HEAD: same branch at `3be9c78a`; active autonomous call-graph goal remains open.
- Changed: added `call_aliased_concrete_trait_object_binding_method()` with `let source = TraitDispatchTarget; let value: &dyn LocalDispatchTrait = &source; value.trait_value()`. `referenced_init_path` now reuses exact initializer proof from referenced local bindings, while opaque parameters and locals without initializer proof still produce no concrete trait-object proof.
- Guardrails: GitNexus impact for `classify_method_receiver` and `extract_body_call_sites` was LOW with no affected processes.
- Verified: focused aliased concrete trait-object sub-agent test passed; standard bundle passed with parser `196 passed`, transform `5 passed`, DB call-graph queries `9 passed`, RAG call-context `1 passed`, and `cargo check -p ploke-tui --features call_graph`.
- Docs: call-graph README and coverage matrix now record 196 paranoid call-site cases and the `src/lib.rs:1092` aliased concrete trait-object binding row.
- Next implementation: continue with bounded Fn/FnOnce/FnPtr exact-proof work, multi-step trait-object concrete proof, richer blanket-bound shapes, broader imported trait scope forms, or closure/async nested-owner modeling. Keep `call_graph` gated and avoid backup fixtures without explicit approval.

## 2026-06-24 20:06 UTC - transitive blanket-bound proof slice

- Branch/HEAD: same branch at `3be9c78a`; active autonomous call-graph goal remains open.
- Changed: added a transitive blanket-bound fixture chain where `BlanketDispatchTarget: TransitiveBaseBound`, `impl<T: TransitiveBaseBound> TransitiveDerivedBound for T`, and `impl<T: TransitiveDerivedBound> TransitiveBoundBlanketTrait for T`; `value.transitive_bound_value()` now resolves to the final concrete blanket impl method.
- Implementation: constrained blanket proof now recurses through exact one-parameter local blanket impls with `MAX_BLANKET_BOUND_DEPTH`, while unproven, cyclic, or unsupported bound chains still fail closed.
- Guardrails: private helpers were not indexed; `resolve_method_call` impact was LOW with one direct caller and no affected processes.
- Verified: focused transitive blanket-bound sub-agent test passed after a RED `Unsupported` failure; standard bundle passed with parser `197 passed`, transform `5 passed`, DB call-graph queries `9 passed`, RAG call-context `1 passed`, and `cargo check -p ploke-tui --features call_graph`.
- Docs: call-graph README and coverage matrix now record 197 paranoid call-site cases and the `src/lib.rs:1114` transitive blanket-bound row.
- Next implementation: continue with bounded Fn/FnOnce/FnPtr exact-proof work, multi-step trait-object concrete proof, broader imported trait scope forms, or closure/async nested-owner modeling. Keep `call_graph` gated and avoid backup fixtures without explicit approval.
