//! Abstract Syntax Tree (AST) visitor for building code knowledge graphs in CozoDB
//! -- AI-generated docs, placeholder --
//!
//! This module provides a visitor implementation that:
//! - Parses Rust code using `syn`
//! - Extracts semantic relationships between code entities
//! - Batches inserts for efficient database operations
//! - Maintains hierarchical scope context
//!
//! # Schema Overview
//! | Table      | Columns                          | Description                     |
//! |------------|----------------------------------|---------------------------------|
//! | `nodes`    | (id, kind, name, ...)           | Code entities (functions, types)|
//! | `relations`| (from, to, type, properties)     | Relationships between entities  |
//! | `types`    | (id, name, is_primitive, ...)   | Type information and metadata   |
// TODO: Think about visibility for structs/functions
// What should the crate expose? Does it even matter?
// For now we are more likely to target binaries, but should make th api as public as possible for
// people who want to hack around. Later we'll consider letting some stuff be gated for
// reliability, but we haven't done the tests required to identify points that will need it yet.
// #someday
use cozo::DataValue;
use cozo::Db;
use cozo::JsonData;
use cozo::MemStorage;
use cozo::ScriptMutability;
use cozo::UuidWrapper;
use quote::ToTokens;
use serde_json;

use serde_json::json;
use std::collections::BTreeMap;
use std::default;
use syn::{visit::Visit, ItemFn, ReturnType};
use uuid::Uuid;

use super::relations::RelationKind;

#[cfg(feature = "cozo_visitor")]
/// Configuration for batch insertion performance tuning
/// -- AI-generated docs, placeholder --
const DEFAULT_BATCH_SIZE: usize = 100;

/// AST Visitor that maintains parsing state and batches database operations
/// -- AI-generated docs, placeholder --
///
/// # Invariants
/// - `current_scope` forms a valid hierarchy through push/pop operations
/// - Batch vectors contain homogeneous entries per table
/// - UUIDs are generated deterministically using SHA-1 hashing (UUIDv5)
///
#[cfg(feature = "cozo_visitor")]
pub struct CodeVisitorV2<'a> {
    /// Database handle for batch insertion
    /// -- AI-generated docs, placeholder --
    ///
    pub db: &'a Db<MemStorage>,

    /// Hierarchical context stack using UUID identifiers
    ///
    /// # Scope Lifetime Management
    /// - Maintained as a stack structure throughout AST traversal
    /// - Pushed when entering a new scope (module, function, block)
    /// - Popped when exiting a scope
    /// - Root scope is initialized with deterministic UUIDv5("ROOT_SCOPE")
    ///
    /// # Invariants
    /// 1. Always contains at least the root scope UUID
    /// 2. Parent-child relationships are stored in `relations` table with:
    ///    - Edge type "contains"
    /// 3. Scopes form a directed acyclic graph (DAG) through:
    ///    - Imports (cross-scope references)
    ///    - Type implementations
    ///
    /// # Example Traversal
    /// ```rust
    /// // Entering crate root
    /// current_scope: [root]
    ///
    /// mod outer {
    ///     // Pushed to scope stack
    ///     current_scope: [root, outer_mod_id]
    ///     
    ///     fn inner() {
    ///         // Pushed to scope stack  
    ///         current_scope: [root, outer_mod_id, inner_fn_id]
    ///     } // Popped
    /// } // Popped
    /// ```
    ///
    /// # Deterministic IDs
    /// Child entries inherit parent's scope UUIDv5 namespace for consistent:
    /// - Cross-session identifiers
    /// - Diff detection
    /// - Cache utilization
    pub current_scope: Vec<Uuid>,

    /// Batched database operations ready for insertion
    /// -- AI-generated docs, placeholder --
    ///
    /// Keys correspond to CozoDB table names. Each entry contains:
    /// - `nodes`: Code entity definitions
    /// - `relations`: Entity relationships
    /// - `types`: Type system information
    pub batches: BTreeMap<Set<'static>, Vec<DataValue>>,

    /// Maximum number of entries per batch before flushing
    /// -- AI-generated docs, placeholder --
    pub batch_size: usize,
}

#[cfg(feature = "cozo_visitor")]
impl<'a> CodeVisitorV2<'a> {
    /// Creates a new visitor instance with initialized state
    /// -- AI-generated docs, placeholder --
    ///
    /// # Arguments
    /// * `db` - Connected CozoDB instance with required schemas
    ///
    /// # Schemas Must Exist
    // TODO: Fill out the following and include in documentation once we have landed on a working
    // and validated schema.
    //
    // Ensure these relations are created first:
    //
    // CAUTION: UNTESTED SCHEMA
    // Get all functions modifying a target struct
    // ?[fn_body] :=
    //     *nodes[struct_id, "struct", "TargetStruct"],
    //     *relations[impl_id, struct_id, "for_type"],
    //     *relations[impl_id, fn_id, "contains"],
    //     *code_snippets[fn_id, fn_body]
    //     valid_from @ '2024-02-01' // Temporal query
    //
    // CAUTION: UNTESTED SCHEMA
    // Find traits a type indirectly implements
    // ?[trait_name, depth] :=
    //     *nodes[type_id, "struct", "MyType"],
    //     relations*[type_id, trait_id, "requires_trait", depth: 1..5],
    //     *nodes[trait_id, "trait", trait_name]
    //     :order +depth  // From low to high specificity
    pub fn new(db: &'a Db<MemStorage>) -> Self {
        Self {
            db,
            // AI handle current_scope below.
            // ??? What should this be?
            // Actually, I have no idea what this field even is supposed to represent. Can you tell
            // me about it?
            current_scope: vec![Uuid::new_v5(&Uuid::NAMESPACE_OID, "ROOT_SCOPE".as_bytes())], // Initialize with root scope
            batches: BTreeMap::from([
                (Vec::with_capacity(DEFAULT_BATCH_SIZE),),
                (Vec::with_capacity(DEFAULT_BATCH_SIZE),),
                ("types", Vec::with_capacity(DEFAULT_BATCH_SIZE)),
            ]),
            batch_size: DEFAULT_BATCH_SIZE,
        }
    }

    /// Processes a Rust type annotation to extract semantic information
    /// -- AI-generated docs, placeholder --
    ///
    /// # Deterministic ID Generation
    /// Uses UUIDv5 with OID namespace for consistent hashing:
    /// ```rust
    /// let type_id = Uuid::new_v5(&Uuid::NAMESPACE_OID, type_str.as_bytes());
    /// ```
    ///
    /// # Panics
    /// - If type parsing fails (indicates invalid AST state)
    fn process_type(&mut self, ty: &syn::Type) -> Uuid {
        let type_str = ty.to_token_stream().to_string();
        let type_id = Uuid::new_v5(&Uuid::NAMESPACE_OID, type_str.as_bytes());
        let type_is_primative = is_primitive_type(ty);

        // Batch the type for insertion
        self.batch_push(
            "types",
            vec![
                // TODO: Find backup docs that will show me clearly that there isn't a problem with
                // Uuid here (Uuid::new_v5 genering a new one here vs. cozo's handling of them)
                DataValue::Uuid(UuidWrapper(type_id)),
                // Stil worried about this.
                // Is there any reason not to handle the type_id like this? It doesn't seem like
                // cozo has an easy `from` or `into`, which makes me think there could be a reason
                // why they don't want you to put Uuids like this in here. Is there something about
                // the way cozo handles the DataValue::Uuid type that would make us want to avoid
                // this approach, or is there another way to use a publically facing cozo method
                // here? Like is there a way to use a cozo-core method to generate the id instead
                // of doing it with uuid::Uuid::new_v5 like we do above?
                // There could also be potential for a mismatch in versions. Danger? Collisions?
                DataValue::Str(type_str.into()),
                // There is no `is_primative_type` function that I can see, am I missing something here?
                DataValue::Bool(type_is_primative),
                DataValue::Null, // Placeholder for generic params
            ],
        );

        type_id // Concerned about this type_id being a hash directly while the one pushed into
                // cozo is wrapped. Is Cozo going to do something out of sight that might mess with the way
                // these hashes are timestamped? Makes me nervous.
    }

    /// Batches a database row for later insertion
    /// -- AI-generated docs, placeholder --
    ///
    /// # Table Requirements
    /// - `table` must exist in `self.batches` initialization
    /// - Row format must match target table schema
    ///
    /// # Flush Triggers
    /// Automatically flushes when batch reaches `batch_size`
    pub fn batch_push(&mut self, table: &'static str, row: Vec<DataValue>) {
        let batch = self
            .batches
            .entry(table)
            .or_insert_with(|| Vec::with_capacity(self.batch_size));
        batch.push(DataValue::List(row));

        if batch.len() >= self.batch_size {
            self.flush_table(table);
        }
    }

    pub fn flush_table(&mut self, table: &'static str) {
        if let Some(rows) = self.batches.get_mut(table) {
            let query = format!(
                "?[id, name, is_primitive] <- $rows;
                :put {}",
                table
            );

            let params =
                BTreeMap::from([("rows".to_string(), DataValue::List(std::mem::take(rows)))]);
            self.db
                .run_script(&query, params, ScriptMutability::Mutable)
                .unwrap_or_else(|_| panic!("Failed to flush {}", table));
        }
    }

    /// Flushes all remaining batches to the database
    pub fn flush_all(&mut self) {
        // let keys = ["node", "relation", "type"];
        // Flush each table individually
        // let tables: Vec<&str> = self.batches.keys().copied().collect();

        for key in self.batches.iter_mut() {
            match key {}
            self.flush_table(table);
        }
    }
}

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum Set {
    Node {
        id: Uuid,
        name: String,
        kind: NodeKind,
    },
    Relation {
        source: Uuid,
        target: Uuid,
        rel_type: RelationKind,
    },
    Type {
        id: Uuid,
        name: String,
        is_primitive: bool,
    },
}

pub enum NodeKind {
    Fn,
    FnArg,

    Named,
    // Reference
    Slice,
    Array {
        // Element type is in related_types[0]
        size: Option<String>,
    },
    Tuple {
        // Element types are in related_types
    },
    // ANCHOR: ExternCrate
    Function {
        // Parameter types are in related_types (except last one)
        // Return type is in related_types[last]
        is_unsafe: bool,
        is_extern: bool,
        abi: Option<String>,
    },
    //ANCHOR_END: ExternCrate
    Never,
    Inferred,
    RawPointer {
        is_mutable: bool,
        // Pointee type is in related_types[0]
    },
    // ANCHOR: TraitObject
    TraitObject {
        // Trait bounds are in related_types
        dyn_token: bool,
    },
    //ANCHOR_END: TraitObject
    // ANCHOR: ImplTrait
    ImplTrait {
        // Trait bounds are in related_types
    },
    //ANCHOR_END: ImplTrait
    // Paren,
    // Inner type is in related_types[0]
    // ANCHOR: ItemMacro
    Macro {
        name: String,
        tokens: String,
    },
    //ANCHOR_END: ItemMacro
    Unknown {
        type_str: String,
    },
}

#[cfg(feature = "cozo_visitor")]
impl<'a> Visit<'a> for CodeVisitorV2<'a> {
    /// Processes function definitions and their relationships
    /// -- AI-generated, placeholder --
    ///
    /// # Key Operations
    /// 1. Generates function ID using its signature
    /// 2. Records parameter-return type relationships
    /// 3. Maintains scope hierarchy during nested processing
    ///
    /// # Example
    /// For `fn foo(x: i32) -> bool` creates:
    /// - Node entry for `foo`
    /// - Relation `foo -(RETURNS)-> bool`
    /// - Relation `foo -(HAS_PARAM)-> x`
    fn visit_item_fn(&mut self, item: &'a ItemFn) {
        let fn_id = generate_fn_uuid(item);

        // Process return type
        let return_type_id = match &item.sig.output {
            ReturnType::Type(_, ty) => self.process_type(ty),
            _ => Uuid::nil(),
        };

        self.batch_push(
            "nodes",
            vec![
                DataValue::Uuid(UuidWrapper(fn_id)),
                DataValue::Str("function".into()),
                DataValue::Str(item.sig.ident.to_string().into()),
                // ... other fields
            ],
        );
        let json_data = json!({ "optional": false });

        if !return_type_id.is_nil() {
            self.batch_push(
                "relations",
                vec![
                    DataValue::Uuid(UuidWrapper(fn_id)),
                    DataValue::Uuid(UuidWrapper(return_type_id)),
                    DataValue::Str("returns".into()),
                    DataValue::Json(JsonData(json_data)),
                ],
            );
        }

        // Process parameters
        for param in &item.sig.inputs {
            if let syn::FnArg::Typed(pat) = param {
                let param_type_id = self.process_type(&pat.ty);
                let param_id = Uuid::new_v5(
                    &Uuid::NAMESPACE_OID,
                    format!("{}${}", fn_id, pat.pat.to_token_stream()).as_bytes(),
                );

                let is_mutable = if let syn::Pat::Ident(pat_ident) = &*pat.pat {
                    pat_ident.mutability.is_some()
                } else {
                    false
                };

                self.batch_push(
                    "nodes",
                    vec![
                        DataValue::Uuid(UuidWrapper(param_id)),
                        DataValue::Str("parameter".into()),
                        // ... other fields
                    ],
                );

                let json_data = json!({"mutable": is_mutable });
                self.batch_push(
                    "relations",
                    vec![
                        DataValue::Uuid(UuidWrapper(fn_id)),
                        DataValue::Uuid(UuidWrapper(param_type_id)),
                        DataValue::Str("has_param".into()),
                        DataValue::Json(JsonData(json_data)),
                    ],
                );
            }
        }
    }
}

// AI: Is this safe? I don't have any idea how NAMESPACE_OID works. Tell me about it.
// The name NAMESPACE_OID suggests to me that it has to do with the namespace it is in I guess?
// Does that change when the Uuid::NAMESPACE_OID is called within a function?
pub fn generate_fn_uuid(item: &ItemFn) -> Uuid {
    let fn_id = Uuid::new_v5(&Uuid::NAMESPACE_OID, item.sig.ident.to_string().as_bytes());
    fn_id
}
// AI?

// TODO: Move this function where it belongs (tbd)
// utility function, should go in a more appropriate location once we've finished testing the
// current approach to visitor.
//
// Parses actual syntax rather than string matching
pub fn is_primitive_type(ty: &syn::Type) -> bool {
    if let syn::Type::Path(type_path) = ty {
        let ident = type_path.path.get_ident().map(|i| i.to_string());
        ident.as_deref().is_some_and(|i| {
            matches!(
                i,
                "bool"
                    | "char"
                    | "u8"
                    | "u16"
                    | "u32"
                    | "u64"
                    | "u128"
                    | "usize"
                    | "i8"
                    | "i16"
                    | "i32"
                    | "i64"
                    | "i128"
                    | "isize"
                    | "f32"
                    | "f64"
                    | "str"
                    | "String"
            )
        })
    } else {
        false
    }
}

// fn is_primitive_type(ty: &syn::Type) -> bool {
//     matches!(&ty {
//             Type::Path(p) => p.path.segments.last().map_or(false, |s| {
//                 matches!(
//                     s.ident.to_string().as_str(),
//                     "i8" | "i16" | "i32" | "i64" | "i128" | "u8" | "u16" |
//     "u32" | "u64" | "u128" | "f32" |
//                     "f64" | "bool" | "char" | "str" | "usize" | "isize"
//                 )
//             }),
//             _ => false,
//         })
// }
