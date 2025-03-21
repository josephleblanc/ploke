## Target Schema Design Framework
**Key Entities (Per [datatypes.rst])**
| Rust Item          | Cozo Node Type | Key Relations                  | Metadata Example                |
|--------------------|----------------|--------------------------------|----------------------------------|
| Function           | `function`     | `has_param`, `returns`         | `{async: bool, unsafe: bool}`   |
| Struct             | `struct`       | `has_field`, `impl_for`        | `{repr: Option<String>}`        |  
| Enum               | `enum`         | `has_variant`                  | `{repr: Option<String>}`        |
| Trait              | `trait`        | `requires_trait`, `provides`   | `{auto: bool, unsafe: bool}`    |
| Impl Block         | `impl`         | `for_type`, `implements`       | `{trait: "..."}`                |
| Module             | `module`       | `contains`                     | `{path: "..."}`                 |
| Type Alias         | `type_alias`   | `aliases`                      | `{generics: [...]}`             |
| Macro              | `macro`        | `expands_to`                   | `{hygiene: "..."}`              |

## CozoScript Schema Definition
```cozoscript
/* Nodes = All Definable Entities */
::create nodes {
    id: Uuid,          // Deterministic UUIDv5 (path + item name)
    kind: String,      // Type from table above
    name: String,      // Qualified name via RFC #926
    meta: Json?,       // Item-specific metadata
    valid_from: Validity  // Timestamp, per temporal docs
}

/* Relations = Code Structure Graph */
::create relations {
    source: Uuid,      // From node.id 
    target: Uuid,      // To node.id
    rel_type: String,  // From table column 3
    meta: Json?        // Position data, modifiers, etc.
}
```

**Critical Cozo Features Used:**
1. **UUID as Primary Key** ([datatypes.rst#Uuid])
   - Stable IDs via hash of item path + name space
   - Enables fast joins across tables

2. **Validity Type** ([datatypes.rst#Validity])
   - Track code evolution: older versions remain queryable
   - Example: Find all functions as of commit X

3. **JSON Column** ([datatypes.rst#Json])
   - Flexible storage for variant metadata shapes
   - Cozo supports partial indexing on JSON paths

## Immediate Implementation Plan

**1. Schema Migration**
```rust
// Replaces current CodeGraph struct with Cozo-native relations
fn batch_push(&mut self, table: &str, row: Vec<DataValue>) {
    // Convert existing NodeId-based relations to UUIDs
}
```

**2. Transaction Wrapper** ([stored.rst#Chaining])
```rust
impl CodeVisitorV2 {
    pub fn run_transaction<F>(db: &Db, f: F) 
    where F: FnOnce(&mut Self) {
        db.run_script("{?[] <- [] :start_txn}", ...).unwrap();
        let mut visitor = Self::new(db);
        f(&mut visitor);
        visitor.flush_all();
        db.run_script("{?[] <- [] :commit}", ...).unwrap();
    }
}
```

**3. Function Processing Example**
```rust
fn visit_item_fn(&mut self, item: &ItemFn) {
    let fn_id = item.to_uuid(); // Implement RFC 926 style path hashing
    
    // Store function node
    self.batch_push("nodes", vec![
        UuidWrapper(fn_id),
        "function".into(),
        item.sig.ident.to_string().into(),
        json!({"async": item.sig.asyncness.is_some()}),
        DataValue::Validity(/* timestamp */),
    ]);
    
    // Process parameters (demoing relationships)
    for param in &item.sig.inputs {
        let param_id = param.to_uuid(fn_id);
        self.batch_push("nodes", param.node_data());
        self.batch_push("relations", vec![
            UuidWrapper(fn_id),
            UuidWrapper(param_id),
            "has_param".into(),
            json!({"position": index}),
        ]);
    }
}
```

**Key Optimization (Per [stored.rst#Indices])**
```cozoscript
::index create nodes.by_kind { kind, valid_from => id }
::index create relations.by_type { rel_type, valid_from }
```

## Query Patterns for RAG Context
```cozoscript
// Get all functions modifying a target struct
?[fn_body] := 
    *nodes[struct_id, "struct", "TargetStruct"],
    *relations[impl_id, struct_id, "for_type"],
    *relations[impl_id, fn_id, "contains"],
    *code_snippets[fn_id, fn_body]
    valid_from @ '2024-02-01' // Temporal query
    
// Find traits a type indirectly implements
?[trait_name, depth] := 
    *nodes[type_id, "struct", "MyType"],
    relations*[type_id, trait_id, "requires_trait", depth: 1..5],
    *nodes[trait_id, "trait", trait_name]
    :order +depth  // From low to high specificity
```

**Implementation Checklist**
1. UUID generation service for all item types
2. Metadata schema per entity type (valid JSON shapes)
3. Batch size autotuning based on MemStorage limits
4. Temporal version hooks (git commit timestamps?)

Would any component benefit from deeper implementation guidance?
