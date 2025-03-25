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
