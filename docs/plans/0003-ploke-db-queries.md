# ploke-db Query Plan

## Existing Queries from ploke-graph

1. **Find Implementations**  
   Finds all implementations of a trait  
   ```cozo
   ?[struct_name, trait_name] := 
       *traits[trait_id, trait_name, _, _],
       *impls[_, struct_id, trait_id],
       *structs[struct_id, struct_name, _, _]
   ```

2. **Find Type Usages**  
   Finds all functions that use a specific type  
   ```cozo
   ?[fn_name, type_str] := 
       *functions[fn_id, fn_name, _, _, _, _],
       *function_params[fn_id, _, _, type_id, _, _],
       *types[type_id, _, type_str]
   ```

3. **Module Hierarchy**  
   Finds parent-child module relationships  
   ```cozo
   ?[parent_name, child_name] := 
       *modules[parent_id, parent_name, _, _],
       *module_relationships[parent_id, child_id, "Contains"],
       *modules[child_id, child_name, _, _]
   ```

4. **Recursive Module Traversal**  
   Finds all descendants of a module recursively  
   ```cozo
   descendants[ancestor, descendant] := 
       *modules[ancestor_id, ancestor, _, _],
       *module_relationships[ancestor_id, descendant_id, "Contains"],
       *modules[descendant_id, descendant, _, _]
   
   descendants[ancestor, descendant] := 
       descendants[ancestor, intermediate],
       *modules[intermediate_id, intermediate, _, _],
       *module_relationships[intermediate_id, descendant_id, "Contains"],
       *modules[descendant_id, descendant, _, _]
   ```

## Proposed Additional Queries for RAG

1. **Semantic Code Search**  
   Find code snippets similar to natural language query  
   ```cozo
   ?[id, node_type, text_snippet, score] := 
       *code_embeddings[id, node_id, node_type, embedding, text_snippet],
       ~text_search:search{query: $query, k: 5, ef: 100, bind_distance: score}
   ```

2. **Contextual Code Retrieval**  
   Get relevant code context for a given function  
   ```cozo
   ?[fn_name, called_fn, called_struct, used_trait] := 
       *functions[fn_id, fn_name, _, _, _, _],
       *relations[fn_id, called_fn_id, "Calls"],
       *functions[called_fn_id, called_fn, _, _, _, _],
       *relations[fn_id, struct_id, "Uses"],
       *structs[struct_id, called_struct, _, _],
       *impls[_, struct_id, trait_id],
       *traits[trait_id, used_trait, _, _]
   ```

3. **Type Dependency Graph**  
   Find all types used by a function and their relationships  
   ```cozo
   ?[type_name, related_type, relation_kind] := 
       *functions[fn_id, _, _, _, _, _],
       *function_params[fn_id, _, _, type_id, _, _],
       *types[type_id, _, type_name],
       *type_relations[type_id, _, related_type_id],
       *types[related_type_id, _, related_type],
       *relations[type_id, related_type_id, relation_kind]
   ```

4. **Documentation Retrieval**  
   Find relevant documentation for code elements  
   ```cozo
   ?[name, docstring, score] := 
       *functions[id, name, _, _, docstring, _],
       ~text_search:search{query: $query, k: 3, bind_distance: score}
       docstring != null
   ```

5. **Pattern Matching**  
   Find code patterns matching structural criteria  
   ```cozo
   ?[fn_name, struct_name] := 
       *functions[fn_id, fn_name, _, _, _, _],
       *function_params[fn_id, _, _, type_id, _, _],
       *types[type_id, "Struct", struct_name],
       *structs[struct_id, struct_name, _, _],
       *struct_fields[struct_id, _, "name", _, _]
   ```

6. **Change Impact Analysis**  
   Find code that would be affected by changing a type  
   ```cozo
   ?[fn_name, struct_name, relation_kind] := 
       *types[type_id, _, $target_type],
       *relations[type_id, related_id, relation_kind],
       *functions[fn_id, fn_name, _, _, _, _],
       *function_params[fn_id, _, _, related_id, _, _],
       *structs[struct_id, struct_name, _, _],
       *struct_fields[struct_id, _, _, related_id, _]
   ```

## Dependency and Breaking Change Queries (Future Work)

These queries would support dependency management and upgrade assistance:

1. **Dependency Usage Tracking**
   ```cozo
   ?[crate, version, usage_type, count] := 
       *dependency_usages[crate, version, usage_type, locations],
       count := len(locations)
   ```

2. **Breaking Change Impact Analysis**  
   Cross-references project code with known breaking changes:
   ```cozo
   ?[file, line, change_desc, mitigation] :=
       *code_usages[item, file, line],
       *breaking_changes[item, change_desc, mitigation]
   ```

3. **Version Migration Mapping**  
   Finds replacement patterns for deprecated items:
   ```cozo
   ?[old_item, new_item, example] :=
       *version_mappings[old_item, new_item, _],
       *migration_examples[old_item, example]
   ```

Key Requirements:
- Database of breaking changes (potentially community-maintained)
- Precise tracking of dependency item usage
- Version-aware type system mapping

## Proposed Additional MVP Queries (Untested)

**Warning**: These queries are proposed for MVP but haven't been tested yet. CozoScript syntax can be tricky and may need adjustment.

1. **Type Context Expansion**  
   Get all types used in a function's signature with docs:
   ```cozo
   ?[type_name, kind, docstring] :=
       *types[type_id, kind, type_name],
       *functions[fn_id, $target_fn, _, _, _, _],
       *function_params[fn_id, _, _, type_id, _, _],
       or(
           *structs[type_id, _, _, docstring],
           *enums[type_id, _, _, docstring]
       )
   ```
   **Purpose**: Provides minimal but complete type context without pulling entire files.

2. **Method Signature Retrieval**  
   Get signatures of methods called in a function:
   ```cozo
   ?[method_name, params, return_type] :=
       *functions[fn_id, method_name, _, return_type_id, _, _],
       *types[return_type_id, _, return_type],
       *relations[$target_fn, fn_id, "Calls"],
       *function_params[fn_id, _, param_name, param_type_id, _, _],
       *types[param_type_id, _, param_type],
       group_concat([param_name, param_type], ", ", params)
   ```
   **Purpose**: Shows how types are actually used in practice.

3. **Breaking Change Protection**  
   Validate methods against versioned registry:
   ```cozo
   :create version_checks {
       crate: String,
       version: String,
       item: String,
       replacement: String? =>
   }
   ```
   **Implementation Notes**:
   - High manual maintenance cost (requires tracking breaking changes)
   - Only feasible for select high-value crates
   - Would need UI for managing version constraints
   **Someday/Maybe**: Requires "supports breaking changes" feature flag per crate.

4. **Docstring Retrieval**  
   Find relevant documentation for structs/functions:
   ```cozo
   ?[name, docstring, score] := 
       *structs[id, name, _, docstring],
       ~text_search:search{query: $query, bind_distance: score},
       docstring != null
   ```

2. **Method Pattern Search**  
   Find construction patterns for types:
   ```cozo
   ?[struct, method, params] :=
       *structs[sid, struct, _, _],
       *relations[sid, mid, "Contains"],
       *functions[mid, method, _, _, _, _],
       *function_params[mid, _, param, _, _],
       group_concat(param, ", ", params)
   ```

3. **Attribute Search**  
   Find types with specific attributes:
   ```cozo
   ?[name, attr] :=
       *structs[id, name, _, _],
       *attributes[id, _, attr, _]
   ```

4. **Type Usage Examples**  
   Find where types are used in code:
   ```cozo
   ?[file, line, context] :=
       *code_usages[type_id, file, line, context],
       *types[type_id, _, $target_type]
   ```

## Open Questions

1. **Context Weighting**:
   How to balance project-specific vs dependency patterns?
   - Potential solutions to test:
     - Priority scoring based on usage frequency
     - Explicit user preferences
     - Machine learning on past accept/reject decisions

2. **Breaking Change Coverage**:
   Which crates justify the maintenance cost of version tracking?
   - Criteria might include:
     - Popularity in Rust ecosystem
     - Frequency of breaking changes
     - Impact radius of changes

## Implementation Plan

1. First implement the basic query interface in `ploke-db`:
   - Raw query execution
   - Query builder for simple queries
   - Result formatting

2. Then implement the specific query types:
   - Start with direct ports from ploke-graph
   - Add semantic search capabilities
   - Build up to complex contextual queries

3. Finally optimize:
   - Add query caching
   - Implement batched queries
   - Add performance monitoring
