# ploke-graph: The Graph Transformation Layer

## Purpose and Role in the ploke Project

The `ploke-graph` crate serves as the critical bridge between the parsed Rust code representation (`syn_parser`) and the hybrid vector-graph database (`ploke-db`). Its primary responsibilities are:

1. **Schema Definition**: Establishing the database schema that represents Rust code semantics
2. **Data Transformation**: Converting parsed AST nodes into database relations
3. **Type System Mapping**: Preserving Rust's rich type system in the database
4. **Visibility Handling**: Maintaining accurate visibility information for code elements
5. **Span Tracking**: Recording source code locations for precise code generation

As shown in PROPOSED_ARCH_V3, `ploke-graph` operates in the "Graph Transformer" stage (󰆧 Rayon domain), receiving parsed code from `syn_parser` and producing database-ready representations for `ploke-db`.

## Relationship to syn_parser Types

The crate transforms all major types from `syn_parser`:

### Core Graph Structure (graph.rs)
- **CodeGraph**: The root container that holds all parsed code elements. `ploke-graph` transforms this into multiple database relations while preserving all relationships.

### Node Types (nodes.rs)
- **NodeId**: Used as primary keys across all database relations
- **FunctionNode**: Mapped to `functions` relation with parameters in `function_params`
- **ParameterNode**: Stored in `function_params` with type and mutability info
- **TypeDefNode**: Dispatched to appropriate type-specific relations (structs/enums/etc)
- **StructNode**: Mapped to `structs` with fields in `struct_fields`
- **EnumNode**: Mapped to `enums` with variants in `enum_variants` 
- **FieldNode**: Stored in `struct_fields` or `union_fields` with type info
- **VariantNode**: Captured in `enum_variants` with discriminant values
- **TypeAliasNode**: Stored in `type_aliases` with target type reference
- **UnionNode**: Mapped to `unions` with fields in `struct_fields`
- **ImplNode**: Stored in `impls` with self/trait type references
- **TraitNode**: Mapped to `traits` with methods via relations
- **ModuleNode**: Stored in `modules` with hierarchical relationships
- **ValueNode**: Mapped to `values` with const/static differentiation
- **MacroNode**: Stored in `macros` with kind-specific metadata
- **MacroRuleNode**: Currently not fully mapped (future improvement)
- **Attribute**: Stored in `attributes` relation linked to owners

### Relations (relations.rs)  
- **RelationKind**: Mapped to string enums in `relations` table
- **Relation**: Stored as edges between nodes with kind labels

### Type System (types.rs)
- **TypeNode**: Mapped to `types` with detailed kind-specific data
- **TypeKind**: Translated to discriminators with kind-specific columns
- **GenericParamNode**: Stored in `generic_params` with bounds
- **VisibilityKind**: Systematically captured in all node relations

## The IntoCozo Trait

The `IntoCozo` trait serves as the foundation for type-safe database operations:

```rust
pub trait IntoCozo {
    fn cozo_relation() -> &'static str;
    fn into_cozo_map(self) -> BTreeMap<String, DataValue>;
    fn cozo_insert_script(&self) -> String { ... }
}
```

Its key roles are:
1. **Type-Safe Serialization**: Ensures Rust types are correctly mapped to CozoDB types
2. **Batch Operation Support**: Enables efficient bulk inserts via `BatchIntoCozo`
3. **Schema Enforcement**: Maintains consistency between Rust and database representations
4. **Query Generation**: Automates script creation for insert operations

The trait will be crucial for `ploke-db` to:
- Generate optimized queries
- Handle type conversions automatically
- Support both single and batch operations
- Maintain schema consistency

## Visibility and Span Handling

Our visibility system uses a two-part representation in the database:
1. A `kind` string (public/crate/restricted/inherited)
2. An optional `path` list for restricted visibility

This design allows for:
1. **Efficient Queries**: Simple equality checks for public/crate/inherited
2. **Path Analysis**: List operations for restricted visibility paths
3. **Flexible Storage**: Null path for non-restricted visibilities

Key benefits:
1. **Precise Privacy Enforcement**: Matches Rust's visibility rules exactly
2. **Hierarchical Visibility**: Supports nested module paths via list operations
3. **Optimized Storage**: Minimizes storage for common visibility cases
4. **Query Flexibility**: Enables both exact and pattern-matched visibility checks

Span tracking provides:
1. **Precise Code Locations**: For error reporting and code modification
2. **Change Detection**: Enables incremental updates to the code graph
3. **Context Preservation**: Maintains original source context for LLM prompts

## CozoScript Optimization Opportunities

From the Cozo documentation (`datatypes.rst`, `queries.rst`), we can improve:

1. **Type Handling** (`datatypes.rst`):
   - Better utilize Cozo's type system (especially `Json` and `List` types)
   - Implement proper null handling with `Any?` type
   - Use vector types more effectively for embeddings

2. **Query Optimization** (`queries.rst`):
   - Leverage fixed rules (`<~`) for performance-critical operations
   - Use query options like `:limit` and `:sort` more effectively
   - Implement proper transaction management

3. **DataValue Usage** (`value.rs`):
   - More efficient handling of numeric types
   - Better string serialization/deserialization
   - Improved list and tuple handling

Specific improvements to `IntoCozo`:

1. **Visibility Handling**:
```rust
// Current implementation (correct)
fn visibility_to_cozo(v: VisibilityKind) -> (String, Option<DataValue>) {
    match v {
        VisibilityKind::Public => ("public".into(), None),
        VisibilityKind::Crate => ("crate".into(), None),
        VisibilityKind::Restricted(path) => {
            let list = DataValue::List(path.into_iter().map(DataValue::from).collect());
            ("restricted".into(), Some(list))
        }
        VisibilityKind::Inherited => ("inherited".into(), None),
    }
}

// Database schema for visibility:
:create visibility {
    node_id: Int =>
    kind: String,
    path: [String]?
}

// Example query using visibility:
?[id, name] := 
    *functions[id, name, _, _, _, _],
    *visibility[id, "restricted", path],
    is_in("my_module", path)

// Key CozoScript list operations:
// - is_in(element, list) - membership test
// - first(list) - get first element
// - last(list) - get last element  
// - length(list) - get length
// - slice(list, start, end) - get sublist
// - chunks(list, n) - split into chunks
// - windows(list, n) - sliding windows
```

2. **Batch Operation Optimization**:
```rust
// Current
fn cozo_batch_insert_script(items: &[Self]) -> String

// Improved with CozoScript's bulk operations
fn cozo_bulk_insert(items: &[Self]) -> String {
    let sample = items[0].clone().into_cozo_map();
    let columns: Vec<_> = sample.keys().collect();
    
    format!(
        "?[{}] <~ Constant(data: $data) :put {}",
        columns.join(", "),
        Self::cozo_relation()
    )
}
```

## Future Improvements

1. **Advanced Type Handling**:
   - Leverage Cozo's type system more fully
   - Implement proper generic type resolution
   - Add better support for trait objects and impl traits

2. **Query Generation**:
   - Generate optimized queries based on relation patterns
   - Add support for Cozo's advanced features like HNSW indexes
   - Implement proper transaction handling

3. **Performance Optimization**:
   - Use Cozo's parallel processing capabilities
   - Implement more efficient batch operations
   - Add proper indexing strategies

The `ploke-graph` crate is central to ploke's mission of providing context-aware code generation by maintaining a rich, queryable representation of Rust code semantics that combines both structural (graph) and semantic (vector) information.
