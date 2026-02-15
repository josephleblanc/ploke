# ploke-db Query Plan

## Existing Queries from ploke-graph

1. **Find Implementations**  
   ```cozo
   ?[struct_name, trait_name] := 
       *traits[trait_id, trait_name, _, _],
       *impls[_, struct_id, trait_id],
       *structs[struct_id, struct_name, _, _]
   ```
   **Purpose**: Maps trait implementations to their concrete types  
   **Rationale**:
   - Essential for understanding polymorphism in code
   - Helps find all types that satisfy a trait interface
   - Used when suggesting implementations or refactoring trait bounds

2. **Find Type Usages**  
   ```cozo
   ?[fn_name, type_str] := 
       *functions[fn_id, fn_name, _, _, _, _],
       *function_params[fn_id, _, _, type_id, _, _],
       *types[type_id, _, type_str]
   ```
   **Purpose**: Identifies where specific types are used in function signatures  
   **Rationale**:
   - Critical for impact analysis when changing types
   - Helps find all functions that need updating during refactoring
   - Shows how types flow through the codebase

3. **Module Hierarchy**  
   ```cozo
   ?[parent_name, child_name] := 
       *modules[parent_id, parent_name, _, _],
       *module_relationships[parent_id, child_id, "Contains"],
       *modules[child_id, child_name, _, _]
   ```
   **Purpose**: Shows direct module containment relationships  
   **Rationale**:
   - Helps understand code organization
   - Useful for navigation and refactoring module structure
   - Basis for more complex module analysis

4. **Recursive Module Traversal**  
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
   **Purpose**: Finds all modules nested within a parent module  
   **Rationale**:
   - Essential for comprehensive module restructuring
   - Helps analyze visibility and access patterns
   - Basis for package-level refactoring operations

## Proposed Additional Queries for RAG

1. **Semantic Code Search**  
   ```cozo
   ?[id, node_type, text_snippet, score] := 
       *code_embeddings[id, node_id, node_type, embedding, text_snippet],
       ~text_search:search{query: $query, k: 5, ef: 100, bind_distance: score}
   ```
   **Purpose**: Finds code semantically similar to natural language queries  
   **Rationale**:
   - Core RAG functionality for natural language search
   - Uses vector embeddings for fuzzy matching
   - Returns ranked results by similarity score

2. **Contextual Code Retrieval**  
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
   **Purpose**: Builds complete call graph context for a function  
   **Rationale**:
   - Shows how functions fit into larger patterns
   - Reveals indirect dependencies
   - Helps understand architectural relationships

3. **Type Dependency Graph**  
   ```cozo
   ?[type_name, related_type, relation_kind] := 
       *functions[fn_id, _, _, _, _, _],
       *function_params[fn_id, _, _, type_id, _, _],
       *types[type_id, _, type_name],
       *type_relations[type_id, _, related_type_id],
       *types[related_type_id, _, related_type],
       *relations[type_id, related_type_id, relation_kind]
   ```
   **Purpose**: Maps all type relationships used by a function  
   **Rationale**:
   - Essential for understanding complex type systems
   - Shows generics, references, and nested types
   - Helps prevent type-related refactoring errors

4. **Documentation Retrieval**  
   ```cozo
   ?[name, docstring, score] := 
       *functions[id, name, _, _, docstring, _],
       ~text_search:search{query: $query, k: 3, bind_distance: score}
       docstring != null
   ```
   **Purpose**: Finds relevant documentation for code elements  
   **Rationale**:
   - Provides LLM with API usage context
   - Helps explain patterns and conventions
   - Complements code examples with explanations

5. **Pattern Matching**  
   ```cozo
   ?[fn_name, struct_name] := 
       *functions[fn_id, fn_name, _, _, _, _],
       *function_params[fn_id, _, _, type_id, _, _],
       *types[type_id, "Struct", struct_name],
       *structs[struct_id, struct_name, _, _],
       *struct_fields[struct_id, _, "name", _, _]
   ```
   **Purpose**: Identifies structural code patterns  
   **Rationale**:
   - Finds common implementation patterns
   - Helps maintain consistent style
   - Useful for boilerplate generation

6. **Change Impact Analysis**  
   ```cozo
   ?[fn_name, struct_name, relation_kind] := 
       *types[type_id, _, $target_type],
       *relations[type_id, related_id, relation_kind],
       *functions[fn_id, fn_name, _, _, _, _],
       *function_params[fn_id, _, _, related_id, _, _],
       *structs[struct_id, struct_name, _, _],
       *struct_fields[struct_id, _, _, related_id, _]
   ```
   **Purpose**: Predicts effects of type changes  
   **Rationale**:
   - Critical for safe refactoring
   - Shows direct and indirect dependencies
   - Helps estimate refactoring scope

## Additional Queries (rough)
 // Find all items visible to current module's parent
 ?[id] := *visibility[id, 'restricted', path],
path matches ['super', ..]

 // Find crate-local items
 ?[id] := *visibility[id, $kind, _],
kind in ['crate', 'public']

 // Check specific visibility
 ?[is_visible] := *visibility[$node_id, 'restricted', ['my', 'module']],
is_visible = true

## Dependency and Breaking Change Queries (Future Work)

These queries would support dependency management and upgrade assistance:

1. **Dependency Usage Tracking**
   ```cozo
   ?[crate, version, usage_type, count] := 
       *dependency_usages[crate, version, usage_type, locations],
       count := len(locations)
   ```
   **Purpose**: Tracks where and how dependencies are used  
   **Rationale**:
   - Essential for dependency impact analysis
   - Helps identify unused dependencies
   - Basis for version migration planning

2. **Breaking Change Impact Analysis**  
   ```cozo
   ?[file, line, change_desc, mitigation] :=
       *code_usages[item, file, line],
       *breaking_changes[item, change_desc, mitigation]
   ```
   **Purpose**: Identifies code affected by dependency changes  
   **Rationale**:
   - Critical for safe dependency upgrades
   - Pinpoints exact locations needing changes
   - Provides mitigation strategies

3. **Version Migration Mapping**  
   ```cozo
   ?[old_item, new_item, example] :=
       *version_mappings[old_item, new_item, _],
       *migration_examples[old_item, example]
   ```
   **Purpose**: Provides upgrade paths for deprecated items  
   **Rationale**:
   - Automates common migration patterns
   - Reduces upgrade research time
   - Helps maintain compatibility

Key Requirements:
- **Breaking Change Database**: Community-maintained registry of breaking changes
- **Precise Usage Tracking**: Fine-grained dependency usage monitoring
- **Version-Aware Types**: Type system that understands version differences

## Proposed Additional MVP Queries (Untested)

**Warning**: These queries are proposed for MVP but haven't been tested yet. CozoScript syntax can be tricky and may need adjustment.

1. **Longest Docstrings**  
   Find items with the most detailed documentation:
   ```cozo
   ?[name, kind, doc_length] :=
       or(
           *functions[id, name, _, _, docstring, _],
           *structs[id, name, _, docstring],
           *enums[id, name, _, docstring],
           *traits[id, name, _, docstring]
       ),
       docstring != null,
       doc_length := len(docstring),
       kind := if(*functions[id, _, _, _, _, _], "function",
               if(*structs[id, _, _, _], "struct",
               if(*enums[id, _, _, _], "enum",
               if(*traits[id, _, _, _], "trait", "unknown")))),
       :order -doc_length
       :limit 10
   ```
   **Purpose**: Identifies core concepts and well-documented patterns in dependencies.

2. **Type Context Expansion**  
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
