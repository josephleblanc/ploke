//! Query builder implementation
#![allow(dead_code)]

use ploke_transform::schema::primary_nodes::{
    EnumNodeSchema, FunctionNodeSchema, ModuleNodeSchema, StructNodeSchema, TraitNodeSchema,
};

use crate::error::Error;
use crate::QueryResult;
use std::collections::BTreeMap;

/// Main query builder struct
pub struct QueryBuilder<'a> {
    db: &'a cozo::Db<cozo::MemStorage>,
    selected_node: Option<NodeType>,
    filters: Vec<String>,
    limit: Option<usize>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum NodeType {
    Function,
    Struct,
    Enum,
    Trait,
    Module,
    // AI: Fill out the rest of these `NodeType`s with the remaining types from the
    // `primary_nodes.rs` file
}

impl<'a> QueryBuilder<'a> {
    /// Create a new query builder
    pub fn new(db: &'a cozo::Db<cozo::MemStorage>) -> Self {
        Self {
            db,
            selected_node: None,
            filters: Vec::new(),
            limit: None,
        }
    }

    /// Select functions to query
    pub fn functions(mut self) -> Self {
        self.selected_node = Some(NodeType::Function);
        self
    }

    /// Select structs to query
    pub fn structs(mut self) -> Self {
        self.selected_node = Some(NodeType::Struct);
        self
    }

    /// Filter by name (exact match)
    pub fn with_name(mut self, name: &str) -> Self {
        self.filters.push(format!("name = '{}'", name));
        self
    }

    /// Set maximum number of results
    pub fn limit(mut self, limit: usize) -> Self {
        self.limit = Some(limit);
        self
    }

    /// Execute the constructed query
    pub fn execute(self) -> Result<QueryResult, Error> {
        let relation = match self.selected_node {
            Some(NodeType::Function) => FunctionNodeSchema::SCHEMA.relation,
            Some(NodeType::Struct) => StructNodeSchema::SCHEMA.relation,
            _ => return Err(Error::QueryConstruction("No node type selected".into())),
        };

        let mut query = match relation {
            "functions" => format!(
                "?[id, name, visibility, return_type_id, docstring, body] := *{}[id, name, visibility, return_type_id, docstring, body]",
                relation
            ),
            "structs" => format!(
                "?[id, name, visibility, docstring] := *{}[id, name, visibility, docstring]",
                relation
            ),
            _ => return Err(Error::QueryConstruction(format!("Unsupported relation: {}", relation))),
        };

        if !self.filters.is_empty() {
            query.push_str(", ");
            query.push_str(&self.filters.join(", "));
        }

        if let Some(limit) = self.limit {
            query.push_str(&format!(" :limit {}", limit));
        }

        self.db
            .run_script(&query, BTreeMap::new(), cozo::ScriptMutability::Immutable)
            .map(QueryResult::from)
            .map_err(|e| Error::Cozo(e.to_string()))
    }
}

macro_rules! define_static_fields {
    (
        $(($name:ident, $schema:ty, $node_type:ident)),+
    ) => {
        lazy_static::lazy_static! {
            $(
                static ref $name: String = format!("*{} {{ {} }}", <$schema>::SCHEMA.relation, <$schema>::SCHEMA_FIELDS.join(", "));
            )+
        }
        impl NodeType {
            pub fn to_base_query(self) -> &'static str{
                match self {
                    $(
                        NodeType::$node_type => &$name
                    ),+
                }
            }
        }
    };
}

define_static_fields!(
    (FUNCTION_FIELDS, FunctionNodeSchema, Function),
    (STRUCT_FIELDS, StructNodeSchema, Struct),
    (ENUM_FIELDS, EnumNodeSchema, Enum),
    (TRAIT_FIELDS, TraitNodeSchema, Trait),
    (MODULE_FIELDS, ModuleNodeSchema, Module)
    // AI: Fill out the remaining macro fields here with the rest of the items from
    // `primary_nodes.rs` AI!
);

// impl NodeType {
//     pub fn to_base_query(self) -> String {
//         match self {
//             NodeType::Function => ,
//             NodeType::Struct => todo!(),
//             NodeType::Enum => todo!(),
//             NodeType::Trait => todo!(),
//             NodeType::Module => todo!(),
//         }
// }
// }


