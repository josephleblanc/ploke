Database Observability Audit — 2025-08-26 20:47:58Z

Scope
- Assess ploke-db’s observability API coverage and identify gaps for agentic workflows and analysis.

Current API (as implemented)
- Conversation
  - upsert_conversation_turn(turn): stores message id, parent id, kind, content, created_at.
  - list_conversation_since (doc mention): query interface to fetch recent turns.

- Tool Calls
  - record_tool_call_requested(req): idempotent insert; ignores duplicates/terminal states.
  - record_tool_call_done(done): correlates against request, stores ended_at, latency, outcome_json or error, status; idempotent for identical payloads.
  - get_tool_call(request_id, call_id): fetches requested/done pair; used to compute latency and to avoid double finalization.
  - list_tool_calls_by_parent (doc mention): inspect sessions by parent message.

Identified Gaps
- Retrieval context events are not captured (query, top_k, strategy, scores, chosen snippets). Add retrieval_event schema and helper methods.
- Edit proposal lifecycle is not first-class (proposal created, preview built, files, expected hash); not persisted beyond tool_done JSON. Add proposal table and link to files.
- Edit apply results are not persisted beyond tool_done outcome JSON; missing per‑file new hash and validation status. Add apply_result table keyed by request_id + path.
- Cost/usage is not stored on conversation turns or tool runs. Add usage/cost fields on turns, or a separate session_cost table keyed by conversation/thread.
- Model/provider/context settings used per request are not persisted. Add minimal request_context record (model id, provider slug, tools enabled, budgets) for analysis.

Recommended Additions
- Tables (names illustrative): retrieval_event, proposal, proposal_file, apply_result, turn_usage, request_context.
- Methods:
  - record_retrieval_event(parent_id, query, strategy, top_k, budget, items: Vec<{path, score, span}>)
  - create_proposal(request_id, parent_id, files, preview_meta)
  - record_apply_result(request_id, path, old_hash, new_hash, status, error?)
  - upsert_turn_usage(turn_id, prompt_tokens, completion_tokens, total, cost)
  - record_request_context(parent_id, model, provider_slug, tools_on, history_budget, tool_budget)

Notes
- Preserve idempotency and NOW snapshots consistent with existing functions; avoid duplicate lifecycle rows.
- Keep JSON fields for flexible payloads (e.g., retrieval item arrays) but add key columns (parent_id, path, request_id) for efficient queries.
Primary Nodes and Schema Context (from ploke-transform)
- Primary node schemas (crates/ingest/ploke-transform/src/schema/primary_nodes.rs):
  - FunctionNodeSchema, ConstNodeSchema, EnumNodeSchema, ImplNodeSchema, ImportNodeSchema,
    MacroNodeSchema, ModuleNodeSchema, StaticNodeSchema, StructNodeSchema, TraitNodeSchema,
    TypeAliasNodeSchema, UnionNodeSchema.
- Associated nodes (assoc_nodes.rs): MethodNodeSchema.
- Types (schema/types.rs): NamedType, ReferenceType, SliceType, ArrayType, Tuple types.
- Edges (schema/edges.rs): Syntactic relations and derived semantic relations (Contains, Defines, Uses).

Schema Proposal (Observability + Conversation/Agent Layers)
- New relations (illustrative Cozo-like names):
  - conversation_turn(id, parent_id, kind, content, created_at)
  - tool_call_requested(request_id, call_id, parent_id, vendor, tool_name, args_sha256, arguments_json, started_at, model, provider_slug)
  - tool_call_done(request_id, call_id, ended_at, latency_ms, outcome_json, error_kind, error_msg, status)
  - retrieval_event(parent_id, query, strategy, top_k, budget_json, items_json)  // items: [{path, score, span, node_id?}]
  - proposal(request_id, parent_id, created_at, preview_meta_json)
  - proposal_file(request_id, path, expected_hash, start_byte, end_byte)
  - apply_result(request_id, path, old_hash, new_hash, status, error)
  - turn_usage(turn_id, prompt_tokens, completion_tokens, total_tokens, cost_usd)
  - request_context(parent_id, model, provider_slug, tools_on, history_budget, tool_budget)

Bridging Edges
- Link observability/conversation to code graph (primary nodes):
  - affects(request_id, node_id) — a proposal/apply_result edge to primary node ids; optionally by file path + span.
  - references(parent_id, node_id) — retrieval_event items to primary nodes when resolution exists.
  - mentions(turn_id, node_id) — when conversation content references a canon path resolved to node id.

Rationale
- These edges connect user prompts and agent actions to specific code items (primary nodes), enabling queries to analyze how inputs and agent steps shaped the code base over time.
- request_context + usage/cost allow cost/performance analytics per session/turn.

