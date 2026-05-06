# Status

`ploke-tui` already has a working semantic edit path, but the reusable core is not yet shaped as a clean crate/API surface for Prototype 1. The actual semantic edit mechanism is:

1. deserialize an `apply_code_edit` tool request as canonical Rust item edits;
2. resolve each `(file, canon, node_type)` through the code graph to `WriteSnippetData`;
3. stage an `EditProposal` with preview and `Pending` status;
4. apply only after approval, using `ploke-io` hash-checked byte splices;
5. rescan the workspace after successful semantic apply.

The strongest reusable pieces are below the TUI layer: `ploke-db` graph resolution, `ploke-rag` BM25/dense/hybrid retrieval, `ploke-core` tool/result DTOs, and `ploke-io` atomic write APIs. The weakest boundary is the proposal/approval layer: it lives inside `ploke-tui::AppState`, emits chat/UI events, persists to user config paths, and uses TUI commands. Prototype 1 should not treat that as an authority boundary.

Prototype 1 already has a small intervention surface, but it is currently a text-file intervention shim for tool-description mutations. The module explicitly says the local edit path is replaceable once execution delegates to `ploke-tui` (`crates/ploke-eval/src/intervention/mod.rs:1-8`). Its bounded algebra is the right conceptual slot: `Surface` is a bounded read/edit mediation layer (`crates/ploke-eval/src/intervention/algebra/mod.rs:68-85`), and `Intervention` is a typed transition that commits before/after records (`crates/ploke-eval/src/intervention/algebra/mod.rs:103-136`).

# Existing Pieces

## Semantic edit proposal and approval

- Tool schema and canonical parameters exist in `GatCodeEdit`: `file`, `canon`, `node_type`, and replacement `code` (`crates/ploke-tui/src/tools/code_edit.rs:20-51`, `crates/ploke-tui/src/tools/code_edit.rs:225-251`). The tool description tells models to use canonical targets and fall back to lookup/edges when needed (`crates/ploke-core/tool_text/apply_code_edit.md:1`).
- `GatCodeEdit::execute` does not own a pure edit API. It converts params to `rag::utils::ApplyCodeEditRequest` and calls the legacy staging path `apply_code_edit_tool`, then reads the proposal registry for the result (`crates/ploke-tui/src/tools/code_edit.rs:91-126`).
- `ApplyCodeEditRequest` and `Edit::{Canonical, Splice, Patch}` are currently in `ploke-tui::rag::utils`, not in a shared semantic-edit crate (`crates/ploke-tui/src/rag/utils.rs:13-51`).
- Canonical edit resolution is in `apply_code_edit_tool`: it checks parse freshness, validates nonempty edits, resolves scoped paths, restricts node type to primary/assoc nodes, splits the canonical path, queries `ploke-db`, handles relaxed fallback, and produces `WriteSnippetData` spans (`crates/ploke-tui/src/rag/tools.rs:622-910`).
- Canon parsing is private TUI code. `SemanticCanonTarget` distinguishes primary items from methods, and `split_canon_for_semantic_target` normalizes `crate::...` paths (`crates/ploke-tui/src/rag/tools.rs:430-523`).
- Staging creates an `EditProposal` with preview, status `Pending`, `is_semantic: true`, and a deterministic proposal id from request/call ids (`crates/ploke-tui/src/rag/tools.rs:70-247`; proposal id helper at `crates/ploke-tui/src/app_state/core.rs:380-389`).
- Approval dispatch is explicit: command parser recognizes `edit approve <uuid>` / `edit deny <uuid>` (`crates/ploke-tui/src/app/commands/parser.rs:405-418`), dispatcher routes to edit handlers (`crates/ploke-tui/src/app_state/dispatcher.rs:388-399`), and `approve_edits` selects semantic vs non-semantic apply based on `proposal.is_semantic` (`crates/ploke-tui/src/rag/editing.rs:19-79`).
- Semantic apply uses `IoManagerHandle::write_snippets_batch`, updates proposal status, emits tool/chat events, saves proposals, and schedules a rescan on success (`crates/ploke-tui/src/rag/editing.rs:338-457`). Denial marks proposals denied and emits a failed tool event (`crates/ploke-tui/src/rag/editing.rs:550-620`).
- Bulk approval/denial exists with overlap handling: newest pending proposals win, older overlaps become `Stale` (`crates/ploke-tui/src/rag/editing.rs:633-760`).

## Code graph, search, lookup, and IO

- `ploke-db::helpers::graph_resolve_exact` is already a crate-level helper that resolves a relation, absolute file path, canonical module path, and item name to `EmbeddingData` suitable for snippets/edits (`crates/ploke-db/src/helpers.rs:22-70`).
- `ploke-db::helpers::resolve_nodes_by_canon` is a relaxed canonical fallback without file equality (`crates/ploke-db/src/helpers.rs:304-343`).
- `code_item_lookup` and `code_item_edges` are TUI tools but reuse DB helpers and `IoManagerHandle::get_snippets_batch`; their execution is still tied to `Ctx/AppState/EventBus` (`crates/ploke-tui/src/tools/code_item_lookup.rs:108-274`, `crates/ploke-tui/src/tools/get_code_edges.rs:112-295`).
- `ploke-rag::RagService` is crate reusable. It owns BM25 sparse search, dense search, hybrid fusion, and context assembly (`crates/ploke-rag/src/core/mod.rs:121-145`, `crates/ploke-rag/src/core/mod.rs:554-688`). The TUI `request_code_context` tool is a wrapper around `RagService::get_context` using `RetrievalScope::LoadedWorkspace` (`crates/ploke-tui/src/tools/request_code_context.rs:107-212`).
- Shared DTOs exist in `ploke-core`: `RequestCodeContextResult`, `ConciseContext`, and `ApplyCodeEditResult` (`crates/ploke-core/src/rag_types.rs:83-198`), plus shared `ToolName` variants for `apply_code_edit`, `code_item_lookup`, and `code_item_edges` (`crates/ploke-core/src/tool_types.rs:8-60`).
- `ploke-io` is already the right write substrate. `IoManagerHandle` documents hash-verified snippet reads and atomic edit writes (`crates/ploke-io/src/handle.rs:7-22`); `WriteSnippetData` is in `ploke-core` (`crates/ploke-core/src/io_types.rs:176-198`); `write_snippets_batch` accepts these shared records (`crates/ploke-io/src/handle.rs:325-332`).
- The actual write path verifies the expected tracking hash, byte range, and UTF-8 boundaries before writing through a temp file, fsync, and rename (`crates/ploke-io/src/write.rs:396-550`).

## Prototype 1 bounded editing

- Current `ploke-eval` intervention specs are text-oriented: `ArtifactEdit::{ReplaceWholeText, AppendText, ReplaceSection}` and a `ValidationPolicy` with allowed relpaths, target existence, UTF-8, content-change, marker, and cargo-check flags (`crates/ploke-eval/src/intervention/spec.rs:18-43`).
- The only implemented policy helper is for tool-description targets; it allowlists the tool description artifact path and does not require cargo check (`crates/ploke-eval/src/intervention/spec.rs:45-57`).
- The current applier reads the expected target content, applies a full text edit through `execute_tool_text_intervention`, then returns before/after hashes and artifact/patch ids (`crates/ploke-eval/src/intervention/apply.rs:28-75`).
- The validator checks the target relpath against `allowed_relpaths` after writing, verifies target existence, UTF-8/nonempty/change, and required markers (`crates/ploke-eval/src/intervention/execute.rs:158-233`). This is useful but too weak for semantic Rust edits because allowlisting happens around a target relpath, not around a resolved code item/span before apply.
- Prototype 1 History docs are explicit that ordinary self-improvement must not unlock policy-bearing `crates/ploke-eval`; current immutable surface is `crates/ploke-eval`, mutated surface is tool-description text files, and `ploke-eval` mutation requires a later protocol-upgrade/fork transition (`docs/workflow/evalnomicon/chat-history/history-blocks-v2.md:158-237`).

# Gaps

- No pure semantic-edit service boundary exists. The useful operation should be something like `resolve_semantic_edits(db, io, path_scope, request) -> Vec<WriteSnippetData>`, but today it is embedded in `ploke-tui::rag::tools::apply_code_edit_tool` with `AppState`, `ToolCallParams`, chat events, and TUI error emission.
- Proposal records are TUI state, not transition authority. `AppState` stores proposals in `RwLock<HashMap<Uuid, EditProposal>>` (`crates/ploke-tui/src/app_state/core.rs:46-49`), and persistence writes JSON under the user config directory or `PLOKE_PROPOSALS_PATH` (`crates/ploke-tui/src/app_state/handlers/proposals.rs:10-45`). Prototype 1 needs artifact-local, generation-local records with digests, not user-session state.
- Approval is command/UI-bound. The approval transition is triggered by TUI commands and emits chat/tool UI events (`crates/ploke-tui/src/app_state/dispatcher.rs:388-399`, `crates/ploke-tui/src/rag/editing.rs:421-447`). Prototype 1 needs a parent-authorized transition record, not a human/TUI approval command.
- The semantic apply success rule is too lenient for a safe bounded surface: `apply_semantic_edit` treats `applied > 0` as success (`crates/ploke-tui/src/rag/editing.rs:381-393`). A Prototype 1 semantic intervention should require all resolved edits to apply, unless partial application is modeled as a typed rejected outcome.
- Path/surface policy is split. TUI path scoping resolves against the loaded workspace (`crates/ploke-tui/src/rag/tools.rs:652-692`, `crates/ploke-tui/src/tools/code_item_lookup.rs:140-167`), while Prototype 1 `ValidationPolicy` allowlists relpaths (`crates/ploke-eval/src/intervention/spec.rs:34-43`). These must become one pre-apply semantic surface check over `(relpath, node_type, canon, resolved span, expected hash)`.
- Index freshness and artifact identity are not factored. TUI can check parse failure/stale app state, but Prototype 1 must ensure the DB/index corresponds to the child Artifact being edited. Otherwise a code graph span from one checkout can be applied to another.
- Post-apply rescan is a TUI side effect (`crates/ploke-tui/src/rag/editing.rs:449-457`). Prototype 1 should model re-indexing as a separate artifact/index refresh step or evidence record, not as hidden behavior inside approval.
- `ploke-eval` already depends on `ploke-tui` with `test_harness` (`crates/ploke-eval/Cargo.toml:18-24`), so direct calls are technically possible, but that dependency is not a clean authority boundary. Calling `ploke-tui` APIs directly would import UI/chat/session semantics into the Prototype 1 transition path.

# Recommended Next Slice

The smallest demo surface should be a single semantic Rust item in a non-policy artifact, not any file under `crates/ploke-eval`. Concretely:

- Define a demo `SemanticRustItemSurface` for one indexed fixture or non-authority target crate in the child Artifact.
- Allow only one or a small set of relpaths, one or two node types (`function`/`method`), and one canonical path prefix.
- Require exact DB resolution to one node, expected file hash match, UTF-8 byte boundaries, and all edits applied.
- Stage a generation-local proposal record containing: request id, runtime/artifact id, target relpath, canon, node type, resolved span, expected hash, replacement digest, preview digest/text, and validation policy digest.
- Let the Parent approve/apply through Prototype 1 intervention records, not through TUI `edit approve`.
- After apply, compute changed file hash and update the artifact/index evidence in a separate record.

This gives a semantic edit demo without unlocking policy-bearing `ploke-eval`: the mutable surface is the selected fixture/target Rust item; `crates/ploke-eval` remains the immutable runtime authority surface described in History docs.

Factoring needed before safe invocation:

- Move or duplicate as crate-level API: canonical target parsing, semantic edit request DTOs, DB resolution to `WriteSnippetData`, and preview generation. Good destinations are `ploke-core` for DTOs plus either `ploke-rag` or a small new shared edit module for graph-backed edit resolution.
- Introduce a non-TUI proposal/apply trait: `ProposalStore`, `Approval`, and `ApplyOutcome` should not require `AppState`, `EventBus`, chat messages, or user config paths.
- Make surface policy pre-apply and semantic: validate relpath/node/canon/span/hash before `write_snippets_batch`, and reject any resolved edit outside the surface before bytes are written.
- Require all-or-none semantics for Prototype 1 semantic edits, or explicitly model partial application as `Outcome::Rejected` with rollback/repair requirements.
- Tie the code graph/index to the Artifact being edited. The resolver must prove its DB snapshot/index was produced from the same artifact tree or from an accepted pre-apply refresh.
- Keep `ploke-eval` out of the mutable surface until there is an explicit protocol-upgrade/fork transition with its own Crown/History admission rule.
