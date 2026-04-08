# Tool Node-Kind Vocabulary Design Memo

- date: 2026-04-07
- task title: Prevent tool node-kind vocabulary drift
- task description: Follow up on the `apply_code_edit` replay failure investigation with a durable design for keeping model-facing tool descriptions, runtime validation, and DB lookup semantics aligned.
- related planning files:
  - /home/brasides/code/ploke/docs/active/agents/2026-04-07_tool-node-kind-vocabulary-plan.md
  - /home/brasides/code/ploke/docs/active/agents/2026-04-07_tool-call-failure-replay-plan.md

## Problem

The replay investigation established that the recorded failing request looked up:

- file: `/home/brasides/.ploke-eval/repos/BurntSushi/ripgrep/crates/printer/src/util.rs`
- canon: `crate::util::Replacer::replace_all`
- requested kind: `function`

The DB probe showed:

- `replace_all` exists in the snapshot DB
- it is stored as `method`
- the strict lookup failed because the request asked for `function`

The immediate cause appears to be model-facing tool vocabulary drift:

- [`code_item_lookup.rs`](/home/brasides/code/ploke/crates/ploke-tui/src/tools/code_item_lookup.rs) lists allowed `node_kind` values in a hand-written description string and a separate hand-written runtime validator
- [`get_code_edges.rs`](/home/brasides/code/ploke/crates/ploke-tui/src/tools/get_code_edges.rs) duplicates the same list
- the recorded API request fixture shows the model was not offered `method` as a valid value

This is not only a missing-value bug. It is a design issue: the tool layer currently represents a closed vocabulary as unconstrained strings plus prose.

## Existing Type Surfaces

There are already multiple vocabularies in the codebase:

- [`ploke_core::ItemKind`](/home/brasides/code/ploke/crates/ploke-core/src/lib.rs#L845)
  - includes `Function`, `Method`, `Struct`, `Enum`, `Impl`, `Import`, `TypeAlias`, etc.
- [`ploke_db::NodeType`](/home/brasides/code/ploke/crates/ploke-db/src/query/builder.rs#L78)
  - includes `Method` and maps to DB relation names via `relation_str()`
- tool-layer request schemas
  - currently use raw JSON `"type": "string"` with free-form descriptions

The current failure happened because the tool layer is not generated from either core type.

## Recommendation

Introduce one canonical tool-facing node-kind type and generate all model-facing and runtime behavior from it.

That type should:

- be backed by existing domain vocabulary, preferably [`NodeType`](/home/brasides/code/ploke/crates/ploke-db/src/query/builder.rs#L78) or a narrow wrapper around it
- explicitly include the queryable kinds the tools support:
  - `function`
  - `method`
  - `const`
  - `enum`
  - `impl`
  - `import`
  - `macro`
  - `module`
  - `static`
  - `struct`
  - `trait`
  - `type_alias`
  - `union`
- serialize to the exact DB relation strings used by lookup code

The tool layer should stop maintaining this vocabulary as duplicated string arrays and prose bullets.

## Why This Is The Right Fix

This addresses the actual failure mode:

1. the model chooses from whatever the schema and tool description present
2. the runtime validator separately decides what is legal
3. the DB resolver separately decides what exists

If those three surfaces drift, the model can make a request that is syntactically valid in the tool prompt but semantically impossible in the DB.

A single canonical tool-kind type removes that class of drift.

## Concrete Design

### 1. Add a shared tool-query kind type

Add a small enum in a shared location used by the TUI tools. Example shape:

```rust
pub enum ToolNodeKind {
    Function,
    Method,
    Const,
    Enum,
    Impl,
    Import,
    Macro,
    Module,
    Static,
    Struct,
    Trait,
    TypeAlias,
    Union,
}
```

This type should provide:

- `as_str() -> &'static str`
- `description_list() -> &'static str` or equivalent generated text
- `json_schema_enum() -> serde_json::Value`
- `TryFrom<&str>` or `Deserialize` for validation
- `to_relation() -> &'static str` or `to_node_type() -> NodeType`

If the codebase can cleanly use [`NodeType`](/home/brasides/code/ploke/crates/ploke-db/src/query/builder.rs#L78) directly, that is better than adding another enum. But the tool-facing subset must still be explicit.

### 2. Emit JSON Schema `enum`

Today the schema says:

```json
{ "type": "string", "description": "..." }
```

It should instead say:

```json
{
  "type": "string",
  "enum": ["function", "method", "const", "enum", "impl", "import", "macro", "module", "static", "struct", "trait", "type_alias", "union"],
  "description": "Kind of code item."
}
```

This matters because the model sees the schema, not just the prose.

### 3. Parse to a typed value early

`LookupParams` and `EdgesParams` currently hold `node_kind` as `Cow<'a, str>`. That keeps validation late and stringly typed.

Prefer:

- `node_kind: ToolNodeKind` in the owned/validated path
- or an immediate parse step after deserialization

This keeps resolver calls typed until the final relation-string conversion.

### 4. Generate validator and docs from the same source

The following should all derive from the same type:

- JSON schema `enum`
- human-readable description text
- `allowed_kinds` validation
- any examples shown in helper UIs or docs

That includes:

- [`code_item_lookup.rs`](/home/brasides/code/ploke/crates/ploke-tui/src/tools/code_item_lookup.rs)
- [`get_code_edges.rs`](/home/brasides/code/ploke/crates/ploke-tui/src/tools/get_code_edges.rs)
- [`code_edit.rs`](/home/brasides/code/ploke/crates/ploke-tui/src/tools/code_edit.rs#L230), which currently uses a vague `"function|struct|enum|..."` description
- any tool-list/help rendering that reproduces tool signatures

## Guardrails

### Hard invariant tests

Add tests that assert:

- the emitted schema enum exactly matches the validator's accepted values
- the validator's accepted values exactly match the canonical tool-kind source
- `method` is present

This should be a unit test, not a manual check.

### Request-level fixture test

Use the existing replay fixture:

- [BurntSushi__ripgrep-2209_code_item_lookup_context.json](/home/brasides/code/ploke/crates/ploke-eval/src/tests/fixtures/BurntSushi__ripgrep-2209_code_item_lookup_context.json)

Deserialize its `api_request` payload and assert that the `code_item_lookup` tool definition includes `method` in the schema enum once the fix is in place.

That catches drift in the actual request construction path, not just source-side helper constants.

### Strict lookup with helpful recovery

Do not silently coerce `function` to `method`.

Silent coercion would weaken semantics and make future classification mistakes harder to detect.

Instead, preserve exact lookup and improve the error path:

- if lookup for `function` fails
- and a same-name candidate exists as `method`
- return a recovery-oriented error message indicating the nearby kind mismatch

That keeps correctness strict while improving model recovery.

## Scope Notes

One subtle point: tool-queryable kinds are not identical to `NodeType::primary_nodes()`.

For example:

- the lookup tools currently advertise `impl` and `import`
- [`NodeType::primary_nodes()`](/home/brasides/code/ploke/crates/ploke-db/src/query/builder.rs#L158) does not include them
- [`NodeType::primary_and_assoc_nodes()`](/home/brasides/code/ploke/crates/ploke-db/src/query/builder.rs#L176) includes `Method` but still excludes `Impl` and `Import`

So the correct source of truth is not "all primary nodes". It is "all kinds the tool contract promises are queryable".

That subset should be defined once, explicitly.

## Minimal Rollout

1. Introduce the canonical tool-kind source.
2. Switch `code_item_lookup` and `code_item_edges` to generated schema enum plus typed validation.
3. Update `apply_code_edit` to use the same source for `node_type`.
4. Add invariant tests.
5. Add a request-level regression test against the replay fixture.

## Decision

The recommended direction is:

- strict semantics
- typed closed vocabulary
- generated schema and validation
- regression tests at both source and request-construction layers

That is the smallest design change that directly blocks recurrence of the failure seen in the replay.
