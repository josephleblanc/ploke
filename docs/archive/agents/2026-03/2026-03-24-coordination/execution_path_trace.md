# Execution Path Trace: `/index start` → `CodeVisitor`

**Research Goal:** Trace the complete execution path from the `/index start` command in `ploke-tui` to the `CodeVisitor` methods in `syn_parser`, including ALL intermediate functions.

**Status:** COMPLETE with ParsedGraph merge/resolution methods

---

## Table of Contents

1. [High-Level Execution Path (14 Steps)](#high-level-execution-path)
2. [Intermediate Functions: analyze_file_phase2](#intermediate-functions-step-11--step-12)
3. [Intermediate Functions: CodeVisitor::new and visit_file](#intermediate-functions-codevisitornew-and-visitfile-internal-flow)
4. [Intermediate Functions: visit_item_fn](#intermediate-functions-visit_item_fn-detailed-call-trace)
5. [Intermediate Functions: visit_item_struct](#intermediate-functions-visit_item_struct-deep-dive)
6. **[ParsedGraph Merge and Resolution Methods](#parsedgraph-merge-and-resolution-methods)** ← NEW

---

## High-Level Execution Path (14 Steps)

| # | Function/Method | Location (File) | Description | Traced By |
|---|-----------------|-----------------|-------------|-----------|
| 1 | `execute` | `crates/ploke-tui/src/app/commands/exec.rs:43` | Main command dispatcher | Agent (verification) |
| 2 | `execute_legacy` | `crates/ploke-tui/src/app/commands/exec.rs:886` | Legacy handler for `/index start` | Agent (verification) |
| 3 | `StateCommand::IndexTargetDir` dispatch | `ploke-tui/src/app_state/dispatcher.rs` | Dispatches to indexing handler | Agent (verification) |
| 4 | `index_workspace` | `ploke-tui/src/app_state/handlers/indexing.rs:37` | Main indexing handler | Agent (verification) |
| 5 | `run_parse_resolved` | `ploke-tui/src/parser.rs:174` | Parses resolved target | Agent (verification) |
| 6 | `parse_workspace` | `syn_parser/src/lib.rs:72` | Workspace parsing entry | Agent (verification) |
| 7 | `try_run_phases_and_merge` | `syn_parser/src/lib.rs:346` | Runs phases, merges graphs, builds tree | Agent (verification) |
| 8 | `try_run_phases_and_resolve` | `syn_parser/src/lib.rs:184` | Discovery + parallel parsing | Agent (verification) |
| 9 | `run_discovery_phase` | `syn_parser/src/discovery/mod.rs` | Phase 1: File discovery | Agent (verification) |
| 10 | `analyze_files_parallel` | `syn_parser/src/parser/visitor/mod.rs:475` | Parallel file analysis | Agent (verification) |
| 11 | `analyze_file_phase2` | `syn_parser/src/parser/visitor/mod.rs:99` | Per-file worker | Agent (verification) |
| 12 | `CodeVisitor::new` | `code_visitor.rs:63` | Creates CodeVisitor | Agent (verification) |
| 13 | `CodeVisitor::visit_file` | `code_visitor.rs` | syn Visitor entry | Agent (verification) |
| 14 | `visit_item_*` methods | `code_visitor.rs` | Individual item visitors | Agent (verification) |

---

## ParsedGraph Merge and Resolution Methods

**Research Date:** 2026-03-24  
**Purpose:** Complete trace of merging and resolution methods called after parsing

---

### Overview

After `analyze_files_parallel` returns `Vec<Result<ParsedCodeGraph, SynParserError>>`, the following merge and resolution steps occur:

```
try_run_phases_and_merge (entry point)
    ├── try_run_phases_and_resolve
    │   └── analyze_files_parallel → Vec<ParsedCodeGraph>
    ├── ParsedCodeGraph::merge_new
    │   └── append_all (for each graph)
    └── ParsedCodeGraph::build_tree_and_prune
        ├── build_module_tree
        │   ├── ModuleTree::new_from_root
        │   ├── add_module (each module)
        │   ├── extend_relations
        │   ├── link_mods_syntactic
        │   ├── resolve_pending_path_attrs
        │   ├── process_path_attributes
        │   ├── update_path_index_for_custom_paths
        │   ├── prune_unlinked_file_modules
        │   └── link_definition_imports
        │       └── try_link_single_import
        │           └── resolve_path_relative_to
        └── prune
```

---

### 1. try_run_phases_and_resolve (lib.rs:184-243)

**File:** `crates/ingest/syn_parser/src/lib.rs`

| Step | Line | Method Call | Description |
|------|------|-------------|-------------|
| 1 | 198-203 | `run_discovery_phase(None, &[path_buf])` | Phase 1: Discovers all source files |
| 2 | 211-212 | `analyze_files_parallel(&discovery_output, 0)` | Phase 2: Parallel parsing |
| 3 | 215-216 | `results.into_iter().partition(Result::is_ok)` | Separates successes/errors |
| 4 | 222-226 | `successes.iter().any(|pr| pr.crate_context.is_some())` | Validates crate context |

**Returns:** `Vec<ParsedCodeGraph>` - Individual parsed graphs for each file

---

### 2. try_run_phases_and_merge (lib.rs:346-368)

**File:** `crates/ingest/syn_parser/src/lib.rs`

| Step | Line | Method Call | Description |
|------|------|-------------|-------------|
| 1 | 347 | `try_run_phases_and_resolve(target_crate)?` | Gets individual file graphs |
| 2 | 350 | `ParsedCodeGraph::merge_new(parsed_graphs)?` | **CRITICAL:** Merges all graphs |
| 3 | 361 | `merged.build_tree_and_prune()` | Builds module tree, prunes orphans |
| 4 | 364-367 | Constructs `ParserOutput` | Returns final result |

---

### 3. ParsedCodeGraph::merge_new (parsed_graph.rs:93-127)

**File:** `crates/ingest/syn_parser/src/parser/graph/parsed_graph.rs`

| Step | Line | Method Call | Description |
|------|------|-------------|-------------|
| 1 | 95-97 | `graphs.pop().ok_or(...)?` | Pops first graph as base |
| 2 | 100 | `new_graph.crate_context.take()` | Preserves crate context |
| 3 | 101-111 | `for mut graph in graphs` | Iterates remaining graphs |
| 4 | 110 | `new_graph.append_all(graph)?` | **CALLS:** Appends all nodes |
| 5 | 115-124 | `debug_relationships()` / `validate_unique_rels()` | [cfg(validate)] Validation |

---

### 4. ParsedCodeGraph::append_all (parsed_graph.rs:129-159)

**File:** `crates/ingest/syn_parser/src/parser/graph/parsed_graph.rs`

Core merge method - uses `Vec::append()` for O(1) moves:

| Component | Line | Operation |
|-----------|------|-----------|
| functions | 130 | `self.graph.functions.append(&mut other.graph.functions)` |
| defined_types | 132-133 | `self.graph.defined_types.append(...)` |
| type_graph | 134 | `self.graph.type_graph.append(...)` |
| impls | 135 | `self.graph.impls.append(...)` |
| traits | 136 | `self.graph.traits.append(...)` |
| relations | 137 | `self.graph.relations.append(...)` |
| modules | 138 | `self.graph.modules.append(...)` |
| consts | 139 | `self.graph.consts.append(...)` |
| statics | 140 | `self.graph.statics.append(...)` |
| macros | 141 | `self.graph.macros.append(...)` |
| use_statements | 142 | `self.graph.use_statements.append(...)` |

---

### 5. ParsedCodeGraph::build_tree_and_prune (parsed_graph.rs:538-555)

**File:** `crates/ingest/syn_parser/src/parser/graph/parsed_graph.rs`

| Step | Line | Method Call | Description |
|------|------|-------------|-------------|
| 1 | 540-543 | `self.build_module_tree()?` | **CALLS:** Builds module tree |
| 2 | 545 | `self.prune()` | **CALLS:** Prunes orphaned items |

---

### 6. ParsedCodeGraph::build_module_tree (parsed_graph.rs:182-274)

**File:** `crates/ingest/syn_parser/src/parser/graph/parsed_graph.rs`

**7-Step Process:**

| Step | Lines | Method Call | Description |
|------|-------|-------------|-------------|
| 1 | 184-190 | `self.crate_context.as_ref().ok_or(...)?` | Gets crate context |
| 2 | 193-195 | `ModuleTree::new_from_root(root_module_id)` | Creates tree with root |
| 3 | 198-201 | `tree.add_module(module)` | Registers all modules |
| 4 | 204 | `tree.extend_relations(self.graph.relations.iter().cloned())` | Copies relations |
| 5 | 207 | `tree.link_mods_syntactic()` | Links decl→def |
| 6 | 210 | `tree.resolve_pending_path_attrs()` | Processes `#[path]` attrs |
| 7 | 213-248 | `tree.process_path_attributes()` | Creates CustomPath relations |
| 8 | 251-258 | `tree.update_path_index_for_custom_paths()` | Updates path index |
| 9 | 261 | `tree.prune_unlinked_file_modules()` | Removes unlinked modules |
| 10 | 264-271 | `tree.link_definition_imports(self)` | Links imports to definitions |

---

### 7. Path Resolution Methods

#### 7.1 resolve_path_relative_to (path_resolver.rs:663-834)

**File:** `crates/ingest/syn_parser/src/resolve/path_resolver.rs`

Core path resolution algorithm:
- Handles `self::`, `super::`, `crate::` prefixes
- Iterates path segments
- Resolves via `Contains` relations
- Calls `is_accessible()` for visibility checks

**Key steps:**
1. Parse path string into segments
2. Handle relative prefixes (`self::`, `super::`, `crate::`)
3. Walk module tree following segments
4. Check visibility at each step via `is_accessible()`
5. Return resolved `NodeId` or error

#### 7.2 is_accessible / get_effective_visibility (path_resolver.rs:424-653)

Visibility checking logic:
- `Public`: Always accessible
- `Crate`: Accessible from same crate
- `Restricted`: Check path restriction
- `Inherited`: Check parent visibility

---

### 8. ModuleTree Methods

| Method | Location | Purpose |
|--------|----------|---------|
| `new_from_root` | module_tree.rs | Creates tree with root module |
| `add_module` | module_tree.rs | Registers a module |
| `extend_relations` | module_tree.rs | Adds relations to index |
| `link_mods_syntactic` | module_tree.rs | Links declarations to definitions |
| `resolve_pending_path_attrs` | module_tree.rs | Resolves `#[path]` attributes |
| `process_path_attributes` | module_tree.rs | Creates CustomPath relations |
| `prune_unlinked_file_modules` | module_tree.rs | Removes orphaned modules |
| `link_definition_imports` | module_tree.rs | Links `use` statements to defs |

---

### 9. Pruning (parsed_graph.rs:475-534)

**File:** `crates/ingest/syn_parser/src/parser/graph/parsed_graph.rs`

The `prune()` method removes orphaned items:

| Step | Description |
|------|-------------|
| 1 | Collect all module IDs from ModuleTree |
| 2 | `retain()` on functions - keep if in valid module |
| 3 | `retain()` on defined_types |
| 4 | `retain()` on impls |
| 5 | `retain()` on traits |
| 6 | `retain()` on consts/statics/macros |
| 7 | `retain()` on relations - keep if both ends exist |

---

## Complete Execution Path with All Methods

```
/index start command
    └── execute_legacy (ploke-tui)
        └── StateCommand::IndexTargetDir dispatch
            └── index_workspace (ploke-tui)
                └── run_parse_resolved (ploke-tui)
                    └── parse_workspace OR direct call (syn_parser)
                        └── try_run_phases_and_merge
                            └── try_run_phases_and_resolve
                                ├── run_discovery_phase
                                └── analyze_files_parallel
                                    └── analyze_file_phase2 (per file)
                                        ├── syn::parse_file
                                        ├── VisitorState::new
                                        ├── extract_cfg_strings
                                        ├── calculate_cfg_hash_bytes
                                        ├── NodeId::generate_synthetic
                                        ├── ModuleNode::new
                                        └── CodeVisitor::new
                                            └── visit_file (syn trait)
                                                └── visit_item_* (CodeVisitor)
                                                    └── [MANY methods...]
                            └── ParsedCodeGraph::merge_new
                                └── append_all (per graph)
                            └── ParsedCodeGraph::build_tree_and_prune
                                ├── build_module_tree
                                │   ├── ModuleTree::new_from_root
                                │   ├── add_module
                                │   ├── extend_relations
                                │   ├── link_mods_syntactic
                                │   ├── resolve_pending_path_attrs
                                │   ├── process_path_attributes
                                │   ├── update_path_index_for_custom_paths
                                │   ├── prune_unlinked_file_modules
                                │   └── link_definition_imports
                                │       └── try_link_single_import
                                │           └── resolve_path_relative_to
                                │               └── is_accessible
                                └── prune
```

---

## Key Files Reference

| File | Purpose |
|------|---------|
| `ploke-tui/src/app/commands/exec.rs` | Command handling |
| `ploke-tui/src/app_state/handlers/indexing.rs` | Indexing orchestration |
| `ploke-tui/src/parser.rs` | Parser bridge |
| `syn_parser/src/lib.rs` | Main entry: `try_run_phases_and_merge`, `try_run_phases_and_resolve` |
| `syn_parser/src/discovery/mod.rs` | Discovery phase |
| `syn_parser/src/parser/visitor/mod.rs` | `analyze_files_parallel`, `analyze_file_phase2` |
| `syn_parser/src/parser/visitor/code_visitor.rs` | `CodeVisitor` |
| `syn_parser/src/parser/graph/parsed_graph.rs` | `ParsedCodeGraph`, merge, build_tree, prune |
| `syn_parser/src/resolve/path_resolver.rs` | Path resolution |
| `syn_parser/src/parser/nodes/ids/internal.rs` | ID generation |

---

## Intermediate Functions (Step 11 → Step 12)

**33 intermediate calls** inside `analyze_file_phase2` before `CodeVisitor::new`:

| # | Function/Method Call | Location | Description |
|---|---------------------|----------|-------------|
| 11.1 | `std::fs::read_to_string(&file_path)` | `mod.rs:115` | Reads file content |
| 11.2 | `syn::parse_file(&file_content)` | `mod.rs:124` | Parses to AST |
| 11.3 | `VisitorState::new(...)` | `mod.rs:129` | Creates state |
| 11.4 | `extract_cfg_strings(&file.attrs)` | `mod.rs:135` | Extracts CFGs |
| 11.5 | `calculate_cfg_hash_bytes(&file_cfgs)` | `mod.rs:139` | Hashes CFGs |
| 11.6 | `NodeId::generate_synthetic(...)` | `mod.rs:152` | Generates root module ID |
| 11.7 | `state.generate_tracking_hash(...)` | `mod.rs:190` | Content hash |
| 11.8 | `extract_file_level_attributes(...)` | `mod.rs:194` | File attrs |
| 11.9 | `extract_file_level_docstring(...)` | `mod.rs:195` | File docs |
| 11.10 | `ModuleNode::new(...)` | `mod.rs:201` | Root module node |
| 11.11 | `state.code_graph.modules.push(...)` | `mod.rs:201` | Add to graph |
| 11.12 | `state.current_primary_defn_scope.push(...)` | `mod.rs:209` | Push scope |
| **12** | **`CodeVisitor::new(&mut state)`** | **`mod.rs:212`** | **Create visitor** |

---

## Intermediate Functions: visit_item_fn Detailed Call Trace

**Two branches:** Proc Macro (19 calls) and Regular Function (23 calls)

### Regular Function Branch Key Calls:
1. `extract_cfg_strings(&func.attrs)`
2. `calculate_cfg_hash_bytes(&provisional_effective_cfgs)`
3. `register_new_node_id(&fn_name, ItemKind::Function, cfg_bytes)`
   - `generate_synthetic_node_id()` → `NodeId::generate_synthetic()`
4. `push_primary_scope(&fn_name, fn_typed_id, &provisional_effective_cfgs)`
5. `process_fn_arg(arg)` (per parameter)
   - `get_or_create_type()` → `process_type()` → `generate_type_id()`
6. `get_or_create_type()` (return type)
7. `process_generics(&func.sig.generics)`
   - `process_type_bound()` → `get_or_create_type()`
8. `pop_primary_scope(&fn_name)`
9. `extract_docstring(&func.attrs)`
10. `extract_attributes(&func.attrs)`
    - `parse_attribute()` (per attr)
11. `convert_visibility(&func.vis)`
12. `generate_tracking_hash(&func.to_token_stream())`
13. `FunctionNode { ... }` construction
14. `relations.push(SyntacticRelation::Contains { ... })`

---

## Intermediate Functions: visit_item_struct Deep Dive

**47+ calls in 11 phases:**

### Phase 1: CFG Handling
- `extract_cfg_strings(&item_struct.attrs)`
- `calculate_cfg_hash_bytes(&provisional_effective_cfgs)`

### Phase 2: Node ID Registration  
- `register_new_node_id(&struct_name, ItemKind::Struct, cfg_bytes)`
  - `generate_synthetic_node_id()` → `NodeId::generate_synthetic()`

### Phase 3: Scope Push
- `push_primary_scope(&struct_name, struct_typed_id, &provisional_effective_cfgs)`

### Phase 4: Field Processing (loop per field)
- `extract_cfg_strings(&field.attrs)`
- `calculate_cfg_hash_bytes(&field_provisional_effective_cfgs)`
- `generate_synthetic_node_id(&field_ref, ItemKind::Field, field_cfg_bytes)`
- `get_or_create_type(&field.ty)`
  - `process_type()` → `generate_type_id()`
- `convert_visibility(&field.vis)`
- `extract_attributes(&field.attrs)`
  - `parse_attribute()`

### Phase 5: Generic Processing
- `process_generics(&item_struct.generics)`
  - `process_type_bound()` → `get_or_create_type()`
  - `generate_synthetic_node_id()` (per generic param)

### Phase 6: Attribute Extraction
- `extract_docstring(&item_struct.attrs)`
- `extract_attributes(&item_struct.attrs)`

### Phase 7: Node Creation
- `convert_visibility(&item_struct.vis)`
- `generate_tracking_hash(&item_struct.to_token_stream())`
- `StructNode { ... }` construction

### Phase 8: Relations
- `SyntacticRelation::StructField { ... }` (per field)
- `SyntacticRelation::Contains { ... }`

### Phase 9: Child Visit
- `visit::visit_item_struct(self, item_struct)`

### Phase 10: Scope Pop
- `pop_primary_scope(&struct_name)`

---

*Document compiled from sub-agent research on 2026-03-24*
