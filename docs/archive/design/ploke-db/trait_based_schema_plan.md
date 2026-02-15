# Plan: Trait-Based CozoDB Schema Definition

**Goal:** Define the CozoDB schema by associating schema fragments and data transformation logic directly with the corresponding `syn_parser` node types using a shared trait.

**Approach:**

1.  **Define Core Schema Types:** Create Rust structs/enums (`RelationDef`, `ColumnDef`, `CozoType`, `IndexDef`) to represent the components of a CozoDB schema in a type-safe way.
2.  **Define `CozoRepresentation` Trait:** Create a trait with methods to provide the primary relation definition (`primary_relation_def`) and the data transformation logic (`into_cozo_map`).
3.  **Implement Trait for Node Types:** Implement `CozoRepresentation` for each relevant `syn_parser` node type (e.g., `FunctionNode`, `StructNode`, `ModuleNode`) in a dedicated file (e.g., `ploke_graph::schema_providers`). Use shared constants or helper methods within each `impl` block to ensure consistency between the schema definition and the data map keys.
4.  **Central Schema Builder:** Create a function that:
    *   Maintains an explicit list of function pointers/closures referencing the `primary_relation_def` method for all relevant node types.
    *   Iterates through this list, collects the `RelationDef`s, and potentially handles duplicates.
    *   Separately defines the `RelationDef`s for edge relations (e.g., `contains`, `implements`).
    *   Separately defines all `IndexDef`s.
    *   Combines all definitions into a final CozoScript string.
5.  **Initialization:** Use the generated script string with `Db::run_script` to initialize the CozoDB schema.

**Example Implementation Snippets:**

```rust
// --- 1. Core Schema Types (e.g., in ploke_graph::schema_types) ---
use std::collections::BTreeMap;
use cozo::DataValue; // Assuming cozo is a dependency
use uuid::Uuid;
// Add other necessary imports: FunctionNode, StructNode, etc.
// from ploke_core or syn_parser

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum CozoType {
    Uuid,
    String,
    Int,
    Bool,
    Float,
    Bytes,
    List, // Consider specifying inner type if possible, e.g., List(Box<CozoType>)
    Json,
    // Add others as needed
}

impl CozoType {
    // Helper to convert to CozoScript type string
    fn to_script_string(&self) -> &'static str {
        match self {
            CozoType::Uuid => "Uuid",
            CozoType::String => "String",
            CozoType::Int => "Int",
            CozoType::Bool => "Bool",
            CozoType::Float => "Float",
            CozoType::Bytes => "Bytes",
            CozoType::List => "List", // CozoScript might just use 'List' generically
            CozoType::Json => "Json",
        }
    }
}


#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ColumnDef {
    pub name: String,
    pub type_: CozoType,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RelationDef {
    pub name: String,
    pub keys: Vec<ColumnDef>, // Consider NonEmptyVec<ColumnDef>
    pub values: Vec<ColumnDef>,
}

impl RelationDef {
    // Method to generate the :create script part
    pub fn to_create_script(&self) -> String {
        let keys_str = self.keys.iter()
            .map(|c| format!("{}: {}", c.name, c.type_.to_script_string()))
            .collect::<Vec<_>>()
            .join(", ");
        let values_str = self.values.iter()
            .map(|c| format!("{}: {}", c.name, c.type_.to_script_string()))
            .collect::<Vec<_>>()
            .join(", ");

        // Basic format, might need refinement for empty values etc.
        format!(":create {} {{ {} => {} }}", self.name, keys_str, values_str)
    }
}

#[derive(Debug, Clone)]
pub struct IndexDef {
    pub relation_name: String,
    pub index_name: String,
    pub columns: Vec<String>,
}

impl IndexDef {
    // Method to generate the ::index script part
    pub fn to_index_script(&self) -> String {
        let cols_str = self.columns.join(", ");
        format!("::index create {}:{} {{ {} }}", self.relation_name, self.index_name, cols_str)
    }
}

// --- 2. Define the Trait (e.g., in ploke_graph::traits) ---

// Placeholder for actual node types
use crate::syn_parser::parser::nodes::{FunctionNode, StructNode}; // Adjust path

pub trait CozoRepresentation {
    /// Provides the definition for the primary Cozo relation this node maps to.
    fn primary_relation_def() -> Option<RelationDef>;

    /// Converts this node instance into a CozoDB data map.
    /// The keys in the map MUST correspond to column names defined
    /// in the `RelationDef` returned by `primary_relation_def`.
    fn into_cozo_map(self) -> BTreeMap<String, DataValue>;
}


// --- 3. Implement Trait (e.g., in ploke_graph::schema_providers) ---
// Assume schema_types are imported

// Use constants for column names to ensure consistency
mod function_columns {
    pub const ID: &str = "id";
    pub const NAME: &str = "name";
    pub const MODULE_ID: &str = "module_id";
    pub const SPAN_START: &str = "span_start";
    pub const SPAN_END: &str = "span_end";
    // ... other column names
}

impl CozoRepresentation for FunctionNode {
    fn primary_relation_def() -> Option<RelationDef> {
        use function_columns as C;
        Some(RelationDef {
            name: "functions".to_string(),
            keys: vec![
                ColumnDef { name: C::ID.to_string(), type_: CozoType::Uuid },
            ],
            values: vec![
                ColumnDef { name: C::NAME.to_string(), type_: CozoType::String },
                ColumnDef { name: C::MODULE_ID.to_string(), type_: CozoType::Uuid },
                ColumnDef { name: C::SPAN_START.to_string(), type_: CozoType::Int },
                ColumnDef { name: C::SPAN_END.to_string(), type_: CozoType::Int },
                // ... other columns using constants ...
            ],
        })
    }

    fn into_cozo_map(self) -> BTreeMap<String, DataValue> {
        use function_columns as C;
        let mut map = BTreeMap::new();
        map.insert(C::ID.to_string(), DataValue::Uuid(cozo::UuidWrapper(self.id.into()))); // Assuming NodeId converts to Uuid
        map.insert(C::NAME.to_string(), DataValue::from(self.name));
        map.insert(C::MODULE_ID.to_string(), DataValue::Uuid(cozo::UuidWrapper(self.module_id.into())));
        map.insert(C::SPAN_START.to_string(), DataValue::Int(self.span.0 as i64));
        map.insert(C::SPAN_END.to_string(), DataValue::Int(self.span.1 as i64));
        // ... map other fields using constants ...
        map
    }
}

// Implement for StructNode, ModuleNode, EnumNode, etc. similarly...
// Use dedicated column name constants (e.g., `mod struct_columns { ... }`) for each.


// --- 4/5. Schema Builder & Edge/Index Defs (e.g., in ploke_db::schema) ---
use std::collections::{HashMap, HashSet};
use crate::ploke_graph::schema_types::*; // Adjust path
use crate::ploke_graph::schema_providers::*; // Adjust path
use crate::syn_parser::parser::nodes::*; // Adjust path

fn build_schema_script() -> String {
    let mut relation_defs: HashMap<String, RelationDef> = HashMap::new();
    let mut index_defs: Vec<IndexDef> = Vec::new();

    // Explicit list of types providing node relation schema parts
    // Note: Using fn pointers requires the methods to be static, which they are.
    let node_providers: &[fn() -> Option<RelationDef>] = &[
        FunctionNode::primary_relation_def,
        StructNode::primary_relation_def,
        ModuleNode::primary_relation_def,
        EnumNode::primary_relation_def,
        TypeAliasNode::primary_relation_def,
        UnionNode::primary_relation_def,
        TraitNode::primary_relation_def,
        ImplNode::primary_relation_def, // Assuming ImplNode gets its own relation
        ValueNode::primary_relation_def, // Assuming ValueNode gets its own relation (e.g., for consts/statics)
        // Add other node types here...
    ];

    for provider_fn in node_providers {
        if let Some(rel_def) = provider_fn() {
            if let Some(existing_def) = relation_defs.insert(rel_def.name.clone(), rel_def.clone()) {
                 if existing_def != rel_def {
                     // Handle conflict: panic or log warning
                     // This indicates two different node types tried to define the same
                     // relation name with different structures.
                     panic!("Conflicting schema definition for relation '{}'", rel_def.name);
                 }
             }
        }
    }

    // Define edge relations explicitly
    let contains_rel = RelationDef {
        name: "contains".to_string(),
        keys: vec![
            ColumnDef { name: "parent_id".to_string(), type_: CozoType::Uuid },
            ColumnDef { name: "child_id".to_string(), type_: CozoType::Uuid },
        ],
        values: vec![],
    };
    relation_defs.entry("contains".to_string()).or_insert(contains_rel);

    let implements_rel = RelationDef {
        name: "implements".to_string(),
        keys: vec![
            ColumnDef { name: "type_id".to_string(), type_: CozoType::Uuid }, // Or NodeId if impls map to NodeId
            ColumnDef { name: "trait_id".to_string(), type_: CozoType::Uuid },
        ],
        values: vec![],
    };
    relation_defs.entry("implements".to_string()).or_insert(implements_rel);
    // ... other edge relations ...


    // Define indices explicitly
    index_defs.push(IndexDef {
        relation_name: "functions".to_string(),
        index_name: "by_name".to_string(),
        columns: vec![function_columns::NAME.to_string(), function_columns::ID.to_string()]
    });
    index_defs.push(IndexDef {
        relation_name: "structs".to_string(), // Assuming relation name is "structs"
        index_name: "by_name".to_string(),
        columns: vec!["name".to_string(), "id".to_string()] // Use struct_columns constants ideally
    });
    // ... other indices ...


    // Generate final script
    let relation_script = relation_defs.values()
        .map(|r| r.to_create_script())
        .collect::<Vec<_>>()
        .join("\n");
    let index_script = index_defs.iter()
        .map(|i| i.to_index_script())
        .collect::<Vec<_>>()
        .join("\n");

    format!("{}\n{}", relation_script, index_script)
}

// --- 6. Initialization Function (e.g., in ploke_db::database) ---
use crate::ploke_db::schema::build_schema_script; // Adjust path
use crate::PlokeDbError; // Define this error type

pub fn initialize_database(db: &cozo::Db<cozo::MemStorage>) -> Result<(), PlokeDbError> {
    let schema_script = build_schema_script();
    db.run_script(&schema_script, Default::default(), cozo::ScriptMutability::Mutable)
      .map_err(|e| PlokeDbError::SchemaInitialization(e.to_string()))?;
    println!("Database schema initialized successfully.");
    Ok(())
}

```

**Discussion:**

*   **Pros:**
    *   Co-locates schema fragment definition and data transformation logic for each node type.
    *   Using shared constants/helpers within `impl` blocks significantly reduces the risk of inconsistencies between schema columns and map keys for that type.
    *   Compiler enforces implementation of both methods if the trait is implemented.
*   **Cons:**
    *   Overall schema view is fragmented (node relations in `impl`s, edges/indices defined centrally).
    *   Requires manual maintenance of the `node_providers` list in the builder function. Forgetting to add a new type means its schema is silently omitted.
    *   Still requires runtime/integration testing to ensure `DataValue` variants produced by `into_cozo_map` match the `CozoType` declared in the schema fragment.
    *   Handling potential conflicts if multiple node types define the same relation name requires explicit logic in the builder.

This plan provides a concrete structure for implementing the trait-based approach, highlighting how local consistency can be improved while acknowledging the remaining challenges.
