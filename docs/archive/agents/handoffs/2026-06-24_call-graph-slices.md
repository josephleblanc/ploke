# Handoff Changelog

Archived from `docs/active/agents/HANDOFF_CHANGELOG.md` during the 2026-06-24 call-graph handoff rollover.

Active running handoff notes for Codex/user continuity. Keep this file under 120 lines or 10 entries; archive to `docs/archive/agents/handoffs/` when it grows.

Archived prior call-graph progress: `docs/archive/agents/handoffs/2026-06-24_call-graph-progress.md`.

## 2026-06-24 01:34 UTC - stop during tuple-field receiver typing

- Branch/HEAD: `prototype1-parent-mwv-live-r16-dangling-symlink-surface-fix-20260615t003337z-gen0` at `3be9c78a Make typed type graph baseline`.
- Active goal remains open: autonomously finish the call-graph plan by following nearby code patterns, preserving strict resolver/schema/import/fixture invariants, and keeping `call_graph` gated until backup fixtures are reviewed/regenerated.
- Completed before this stop: self-field external method classification for `self.secret.len()` and exact local path-call-result receiver resolution for `make_local_assoc().instance_value()`. Both were verified through parser, transform, DB, RAG, TUI check, fmt, diff-check, and GitNexus detect-changes.
- Current in-progress slice: tuple-field local receiver typing for `value.0.instance_value()`. This slice is not verified yet and may not compile until remaining exhaustive matches are fixed.
- Partial edits already in tree:
  - `crates/ingest/syn_parser/src/parser/nodes/call.rs`: added `FieldTypedLocalBinding` and `FieldInitializedLocalBinding` receiver variants.
  - `crates/ingest/syn_parser/src/parser/visitor/call_extraction.rs`: started carrying typed/initialized local binding proof for field receivers.
  - `crates/ingest/syn_parser/tests/common/call_site_paranoid.rs` and `crates/ingest/syn_parser/tests/uuid_phase3_resolution/call_sites.rs`: flipped the tuple-field method row toward `ResolvedMethodLocalExact`.
  - `crates/ingest/syn_parser/src/resolve/call_resolution.rs`: started `resolve_field_local_method_call`, `resolve_field_type_method`, and `field_type`.
  - `crates/ingest/ploke-transform/src/schema/edges.rs`, `crates/ploke-db/src/call_graph.rs`, `crates/ploke-core/src/rag_types.rs`, and `crates/ploke-rag/src/core/mod.rs`: started projecting/decoding/mapping the new receiver payloads.
- Important implementation note: the DB receiver encoding currently uses `[name, root_path..., "", field_path...]` for the new field-root payloads. This is intended as a strict separator because valid path/member segments are non-empty; if schema/tests reject empty strings, replace it with an explicit sentinel rather than loosening validation.
- Next command: run `cargo test -p syn_parser --features call_graph call_sites -- --nocapture` via a test sub-agent, fix compile errors/exhaustive matches, then rerun it until the focused parser suite passes.
- Next downstream verification after parser passes: `cargo test -p ploke-transform --features call_graph transform::tests -- --nocapture`, `cargo test -p ploke-db --features call_graph call_graph_queries -- --nocapture`, `cargo test -p ploke-rag --features call_graph call_context_collection_attaches_outgoing_call_payloads -- --nocapture`, and `cargo check -p ploke-tui --features call_graph`.
- Final hygiene before handoff/commit: `cargo fmt --all`, `git diff --check`, `npx gitnexus detect-changes`, and `git status --short --branch`.
- Dirty state remains intentional across broad call-graph parser/transform/DB/RAG/TUI/docs/fixture files; `AGENTS.md` is dirty and user-owned/pre-existing. Do not reset or clean.
- Gate/blocker: `CALL_GRAPH_GATE:db-projection` and backup fixture regeneration remain approval-gated; `docs/testing/BACKUP_DB_FIXTURES.md` review is overdue from 2026-06-12.

## 2026-06-24 01:51 UTC - tuple-field receiver typing completed

- Changed: completed the tuple-field local receiver slice for `value.0.instance_value()`. The parser now carries constructed-local field receiver proof as `FieldInitializedLocalBinding`, and the resolver follows the local tuple struct field type only when the root type, field, type relation, and inherent method target are proven exactly.
- Guardrail: path-call initializers such as `Box::new(...)` do not become initialized callable bindings; callable locals like `boxed_fn()` still record `ValueBinding` and fail closed as `Unsupported`.
- Verified through sub-agents: `cargo test -p syn_parser --features call_graph call_sites -- --nocapture` passed with `123 passed`; `cargo test -p ploke-transform --features call_graph transform::tests -- --nocapture` passed with `4 passed`; `cargo test -p ploke-db --features call_graph call_graph_queries -- --nocapture` passed with `8 passed`; `cargo test -p ploke-rag --features call_graph call_context_collection_attaches_outgoing_call_payloads -- --nocapture` passed with `1 passed`; `cargo check -p ploke-tui --features call_graph` passed.
- Docs updated: call-graph README and coverage matrix now mark M09 as exact tuple-field receiver method resolution, while `value.0()` remains unsupported dynamic field-call coverage.
- Next: continue the remaining non-gated receiver/result slices, with good candidates `value.clone_assoc().instance_value()` return-type recovery, await/try result receiver typing, or broader external/prelude method classification. Keep closure/async nested ownership and backup fixture regeneration approval-gated.

## 2026-06-24 02:05 UTC - borrowed typed receiver slice completed

- Changed: completed exact borrowed explicitly typed local receiver handling for `(&value).instance_value()` where `value: LocalAssoc`. Parser records `BorrowedTypedLocalBinding`, transform/DB/RAG preserve the payload, and the resolver reuses exact local type method resolution.
- Guardrail: untyped borrowed locals remain `BorrowedLocalBinding` and unsupported; this does not claim general autoderef/autoref or dereferenced receiver resolution.
- Verified through sub-agents: `cargo test -p syn_parser --features call_graph call_sites -- --nocapture` passed with `123 passed`; `cargo test -p ploke-transform --features call_graph transform::tests -- --nocapture` passed with `4 passed`; `cargo test -p ploke-db --features call_graph call_graph_queries -- --nocapture` passed with `8 passed`; `cargo test -p ploke-rag --features call_graph call_context_collection_attaches_outgoing_call_payloads -- --nocapture` passed with `1 passed`; `cargo check -p ploke-tui --features call_graph` passed.
- Docs updated: call-graph README and coverage matrix now mark M11 as exact borrowed typed local receiver method resolution. M12 `(*value).instance_value()` remains unsupported.
- Next: continue remaining non-gated receiver/result slices, likely dereferenced reference-local receiver typing, method-call-result return-type recovery, or await/try result receiver typing. Keep closure/async nested ownership and backup fixture regeneration approval-gated.

## 2026-06-24 02:17 UTC - dereferenced reference-local receiver slice completed

- Changed: completed exact dereferenced local receiver handling for `(*value).instance_value()` where `let value = &LocalAssoc`. Parser records `DereferencedInitializedLocalBinding`, transform/DB/RAG preserve the payload, and the resolver follows the exact referenced local type path to the inherent method.
- Guardrail: this only covers direct `&TypePath` initializers that are not visible local/parameter bindings. It does not claim general autoderef chains, borrowed aliases, or deref trait resolution.
- Verified through sub-agents: `cargo test -p syn_parser --features call_graph call_sites -- --nocapture` passed with `123 passed` and no warnings; `cargo test -p ploke-transform --features call_graph transform::tests -- --nocapture` passed with `4 passed`; `cargo test -p ploke-db --features call_graph call_graph_queries -- --nocapture` passed with `8 passed`; `cargo test -p ploke-rag --features call_graph call_context_collection_attaches_outgoing_call_payloads -- --nocapture` passed with `1 passed`; `cargo check -p ploke-tui --features call_graph` passed.
- Docs updated: call-graph README and coverage matrix now mark M12 as exact dereferenced initialized local receiver method resolution.
- Next: continue remaining non-gated receiver/result slices only where proof is explicit. Method-call-result and await/try receiver typing likely require richer receiver payload or return-type proof; avoid guessing from method names alone. Keep closure/async nested ownership and backup fixture regeneration approval-gated.

## 2026-06-24 02:27 UTC - awaited path-call result receiver slice completed

- Changed: completed exact awaited path-call result receiver handling for `make_ready_local_assoc().await.instance_value()`. Parser records `AwaitPathCallResult`, transform/DB/RAG preserve the payload, and the resolver follows the local async function return type to the exact inherent method.
- Guardrail: plain `AwaitResult` remains available for non-path-call await operands; this does not infer arbitrary future output types or async block/closure ownership.
- Verified through sub-agents: `cargo test -p syn_parser --features call_graph call_sites -- --nocapture` passed with `123 passed` and no warnings; `cargo test -p ploke-transform --features call_graph transform::tests -- --nocapture` passed with `4 passed`; `cargo test -p ploke-db --features call_graph call_graph_queries -- --nocapture` passed with `8 passed`; `cargo test -p ploke-rag --features call_graph call_context_collection_attaches_outgoing_call_payloads -- --nocapture` passed with `1 passed`; `cargo check -p ploke-tui --features call_graph` passed.
- Docs updated: call-graph README and coverage matrix now mark M15 as exact awaited local path-call result receiver method resolution.
- Next: continue only with non-gated slices that have explicit proof available. `try_local_assoc()?.instance_value()` likely needs `Result<T, E>` output extraction, and method-call-result return typing likely needs richer inner-call payload or target proof.

## 2026-06-24 02:42 UTC - try path-call result receiver slice completed

- Changed: completed exact try path-call result receiver handling for `try_local_assoc()?.instance_value()`. Parser records `TryPathCallResult`, transform/DB/RAG preserve the payload, and the resolver follows the local function's syntactic `Result<T, E>` success type to the exact inherent method.
- Guardrail: only local function path calls with declared `Result<T, E>`, `std::result::Result<T, E>`, or `core::result::Result<T, E>` return types are handled; unqualified `Result` fails closed if a local `Result` segment is visible. Plain `TryResult` remains for other try operands and broader `Try` trait semantics.
- Verified through sub-agents: `cargo test -p syn_parser --features call_graph call_sites -- --nocapture` passed with `123 passed` and no warnings; `cargo test -p ploke-transform --features call_graph transform::tests -- --nocapture` passed with `4 passed`; `cargo test -p ploke-db --features call_graph call_graph_queries -- --nocapture` passed with `8 passed`; `cargo test -p ploke-rag --features call_graph call_context_collection_attaches_outgoing_call_payloads -- --nocapture` passed with `1 passed`; `cargo check -p ploke-tui --features call_graph` passed.
- Docs updated: call-graph README and coverage matrix now mark M16 as exact try local path-call result receiver method resolution.
- Next: remaining adjacent non-gated slices include field-expression dynamic function calls (`value.0()`), method-call-result return typing if receiver payload is enriched safely, or external/prelude method summaries with strict local shadowing guards.

## 2026-06-24 02:53 UTC - downstream receiver payload coverage expanded

- Changed: added `ploke-db` call-context decode coverage for recent receiver payloads: `BorrowedTypedLocalBinding`, `DereferencedInitializedLocalBinding`, `FieldInitializedLocalBinding`, `AwaitPathCallResult`, and `TryPathCallResult`.
- Changed: extended the `ploke-rag` call-context collection test to assert RAG mapping for `TryPathCallResult`.
- Verified through sub-agents: `cargo test -p ploke-db --features call_graph call_graph_queries -- --nocapture` passed with `9 passed`; `cargo test -p ploke-rag --features call_graph call_context_collection_attaches_outgoing_call_payloads -- --nocapture` passed with `1 passed`.
- Next: implementation candidates still need proof-backed designs. Avoid resolving `value.0()` to `local_target` without value-flow proof through tuple-struct construction; avoid method-call-result return typing until the inner call target or receiver proof is carried structurally.

## 2026-06-24 03:00 UTC - graceful stop before next slice

- Branch/HEAD: `prototype1-parent-mwv-live-r16-dangling-symlink-surface-fix-20260615t003337z-gen0` at `3be9c78a Make typed type graph baseline`.
- Active goal remains open: finish the remaining call-graph plan autonomously while following nearby patterns, keeping strict invariants, and preserving the `call_graph` rollout gate until backup fixtures are explicitly reviewed/regenerated.
- Current state: no new implementation slice was started after downstream receiver payload coverage. The broad dirty tree is intentional call-graph work plus pre-existing/user-owned `AGENTS.md`; do not reset, clean, or discard.
- Last known verification remains: parser call-site suite, transform call-graph projection tests, DB call-graph query tests, RAG call-context test, `cargo check -p ploke-tui --features call_graph`, `cargo fmt --all`, `git diff --check`, and `npx gitnexus detect-changes` all passed in the prior session.
- Process check: no long-running cargo/rustc/rustdoc/gitnexus work was intentionally left running.
- Recommended next safe slice: add strict external/prelude method-summary coverage for an explicitly typed local receiver such as `let value: Vec<i32> = Vec::new(); value.len()`, guarded against local shadowing. Before resolver edits, run GitNexus impact for `resolve_method_call`; after edits, rerun focused parser/transform/DB/RAG/TUI checks through test sub-agents.
- Avoid next: do not resolve `value.0()` dynamic field calls without value-flow proof through tuple-struct construction, and do not infer method-call-result return typing from method names alone. Those need richer structural proof first.
- Still approval-gated: backup fixture review/regeneration and any relaxation of schema/import/resolver correctness.

## 2026-06-24 03:25 UTC - typed prelude method external slice completed

- Changed: added strict typed-local receiver classification for external/prelude `.len()` when the receiver type path is external/imported external or exact unshadowed prelude `String`/`Vec`.
- Guardrail: added `local_prelude_shadow::Vec` fixture coverage proving local shadowing wins and resolves to the local inherent `Vec::len` instead of being classified as prelude external.
- Tests/docs: focused call-site suite now covers 125 concrete call expressions; call-graph README and matrix document the new M07 rows.
- Verified through sub-agents: `cargo test -p syn_parser --features call_graph call_sites -- --nocapture` passed with `125 passed`; `cargo test -p ploke-transform --features call_graph transform::tests -- --nocapture` passed with `4 passed`; `cargo test -p ploke-db --features call_graph call_graph_queries -- --nocapture` passed with `9 passed`; `cargo test -p ploke-rag --features call_graph call_context_collection_attaches_outgoing_call_payloads -- --nocapture` passed with `1 passed`; `cargo check -p ploke-tui --features call_graph` passed.
- Hygiene: `cargo fmt --all`, `git diff --check`, and `npx gitnexus detect-changes` passed; GitNexus reported low risk and 0 affected processes.
- Next: continue with proof-backed gaps only. Good candidates are enriching method-call-result receivers with inner target proof, or designing value-flow proof for tuple-field dynamic function calls; do not infer either from names or field types alone.

## 2026-06-24 03:45 UTC - method-call result return typing completed

- Changed: `value.clone_assoc().instance_value()` now resolves the outer `MethodCallResult` through the direct inner method call occurrence, the inner method's exact local target, and that target method's return type. `Self` returns are interpreted through the inner method's owning impl.
- Guardrail: no receiver payload/schema change was needed; the resolver finds exactly one direct nested inner method call by same owner, same start span, shorter maximal end span, and inner method name. Ambiguous or unsupported inner calls still fail closed.
- Tests/docs: the M13 paranoid row now expects `Resolved(LocalExact)` and the call-graph README/matrix describe the exact-proof requirement.
- Verified through sub-agents: `cargo test -p syn_parser --features call_graph call_sites -- --nocapture` passed with `125 passed`; `cargo test -p ploke-transform --features call_graph transform::tests -- --nocapture` passed with `4 passed`; `cargo test -p ploke-db --features call_graph call_graph_queries -- --nocapture` passed with `9 passed`; `cargo test -p ploke-rag --features call_graph call_context_collection_attaches_outgoing_call_payloads -- --nocapture` passed with `1 passed`; `cargo check -p ploke-tui --features call_graph` passed.
- Hygiene: `cargo fmt --all`, `git diff --check`, and `npx gitnexus detect-changes` passed; GitNexus reported low risk and 0 affected processes.
- Next: remaining hard gaps include tuple-field dynamic function calls, deeper closure/async ownership, and backup fixture review/regeneration. Keep avoiding dynamic field-call resolution without value-flow proof.
