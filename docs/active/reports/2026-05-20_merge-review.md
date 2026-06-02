Yes, that framing makes more sense. I checked it as a provenance/merge-shape question, not as “is this final ready to commit?”

The short version: the merge worktree mostly preserved the `tt-expr-core` payload, but the risky areas are exactly the hand-spliced files where primary had advanced a lot: fixtures, `xtask`, RAG/TUI integration, and embedding/indexing paths.

I classified the files changed by `tt-expr-core` relative to the primary merge base:

- `104` files match the `tt-expr-core` source exactly.
- `44` files are custom merge results.
- `236` files match the current primary side.
- `118` source files are missing in the merge result, but these are almost entirely older `docs/active/agents/...` historical docs, not core code.
- `2` files are primary-only/additive in the compared set.

What looks preserved from this repo:

- The core typed type graph payload appears intact: `ploke-transform` type graph transform files, `syn_parser` type nodes/type slots/type resolution files, DB `type_graph` files, and most new typed graph tests are either exact-source or present as expected.
- The `ploke-expr` removal is still honored. I found no `ploke-expr`/`ploke_expr` references in the integration tree.
- The new type-resolution docs under `docs/testing` and `docs/active/agents/2026-05-10_tt-expr-core_type-resolution-handoff` are present.

What might have been incorrectly merged or flattened:

- `crates/test-utils/src/fixture_dbs.rs` is a custom merge and carries the highest risk. It combines this repo’s shared fixture fallback work with newer primary fixture/provider changes. The direction is plausible, but the resulting registry points at `2026-05-19` fixture names while the worktree does not contain those committed seed files. That may be intended if shared-cache-only is the new policy, but it is not the same shape as the source branch.
- `crates/ploke-rag/src/core/unit_tests.rs` and `crates/ploke-rag/docs/FIXTURE_CONFIGURATION.md` disagree: code still uses the local fixture/local embedder, while docs say OpenRouter. That looks like a bad semantic splice, not just normal merge noise.
- `xtask/src/main.rs` is custom, while several `xtask/src/commands/*` files match primary. That is a classic risk zone: the source branch’s fixture regeneration/verification behavior may have been pasted into the older/main entrypoint path while primary had already moved behavior into command modules.
- `crates/ploke-tui/src/tools/request_code_context.rs` is custom. That is expected because primary had the budget-contract work and source had type-context expansion, but it needs targeted review because both branches changed the same tool boundary.
- Method/embedding paths were not lost, but they were changed beyond the source branch: `count_unembedded_searchable_nonfiles`, `primary_and_assoc_nodes`, and OpenRouter fixture changes are layered on top. That may be correct follow-up work, but it means this is no longer a clean “source payload only” merge.

My current judgment: the important `tt-expr-core` type graph work was not broadly lost. The likely incorrect merge areas are not missing parser/DB type graph files; they are integration-policy surfaces where fixture provider semantics, xtask commands, and RAG/TUI test setup got mixed between branches.
