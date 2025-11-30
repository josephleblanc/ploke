//! Query builder implementation
#![allow(dead_code)]

use itertools::Itertools;
use ploke_error::Error;
use ploke_transform::schema::edges::SyntacticRelationSchema;

// WARN: These schema are different for the ploke-transform feature flag "multi_embedding_schema",
// which has been set as the default feature flag for that crate.
// See the ploke-db Cargo.toml for the related configuration of ploke-db, which wants to only have
// the "multi_embedding_schema" version on for ploke-transform when the ploke-db flag for
// "multi_embedding_db" is on (as is default right now for linting purposes).
// TODO:multi_embedding
// Delete above warning after finishing new feature implementation for multi_embedding
use ploke_transform::schema::primary_nodes::{
    ConstNodeSchema, EnumNodeSchema, FunctionNodeSchema, ImplNodeSchema, ImportNodeSchema,
    MacroNodeSchema, ModuleNodeSchema, StaticNodeSchema, StructNodeSchema, TraitNodeSchema,
    TypeAliasNodeSchema, UnionNodeSchema,
};
use ploke_transform::schema::secondary_nodes::{
    AttributeNodeSchema, FieldNodeSchema, GenericConstNodeSchema, GenericLifetimeNodeSchema,
    GenericTypeNodeSchema, ParamNodeSchema, VariantNodeSchema,
};
use ploke_transform::schema::types::{
    ArrayTypeSchema, FunctionTypeSchema, ImplTraitTypeSchema, InferredTypeSchema, MacroTypeSchema,
    NamedTypeSchema, NeverTypeSchema, ParenTypeSchema, RawPointerTypeSchema, ReferenceTypeSchema,
    SliceTypeSchema, TraitObjectTypeSchema, TupleTypeSchema, UnknownTypeSchema,
};
use serde::{Deserialize, Serialize};
use tracing;

use crate::{DbError, QueryResult};
use std::collections::{BTreeMap, HashMap, HashSet};

pub const LOG_TARGET_QUERY_BUILDER: &str = "query_builder";

/// Main query builder struct
#[derive(Debug, Clone)]
pub struct QueryBuilder {
    pub selected_node: Option<NodeType>,
    pub lhs: HashSet<&'static str>,
    pub custom_lhs: Vec<String>,
    pub rhs_rels: HashMap<RhsRelation, Vec<FieldValue>>,
    filters: Vec<String>,
    limit: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FieldValue {
    pub field: &'static str,
    pub value: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RhsRelation {
    pub node_type: NodeType,
    pub node_type_index: i8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NodeType {
    Function,
    Struct,
    Enum,
    Trait,
    Module,
    Const,
    Impl,
    Import,
    Macro,
    Static,
    TypeAlias,
    Union,
    Param,
    Variant,
    Field,
    Attribute,
    GenericType,
    GenericLifetime,
    GenericConst,
    NamedType,
    ReferenceType,
    SliceType,
    ArrayType,
    TupleType,
    FunctionType,
    NeverType,
    InferredType,
    RawPointerType,
    TraitObjectType,
    ImplTraitType,
    ParenType,
    MacroType,
    UnknownType,
    SyntaxEdge,
}

impl NodeType {
    pub fn all_variants() -> [Self; 34] {
        use NodeType::*;
        [
            Function,
            Struct,
            Enum,
            Trait,
            Module,
            Const,
            Impl,
            Import,
            Macro,
            Static,
            TypeAlias,
            Union,
            Param,
            Variant,
            Field,
            Attribute,
            GenericType,
            GenericLifetime,
            GenericConst,
            NamedType,
            ReferenceType,
            SliceType,
            ArrayType,
            TupleType,
            FunctionType,
            NeverType,
            InferredType,
            RawPointerType,
            TraitObjectType,
            ImplTraitType,
            ParenType,
            MacroType,
            UnknownType,
            SyntaxEdge,
        ]
    }

    pub fn primary_nodes() -> [Self; 10] {
        use NodeType::*;
        [
            Function, Const, Enum,
            // NOTE: leaving import and impl out, we don't generate a tracking hash for these for some
            // reason?
            // Import,
            // Impl,
            Macro, Module, Static, Struct, Trait, TypeAlias, Union,
        ]
    }
    pub fn is_primary(type_name: &str) -> bool {
        Self::primary_nodes()
            .iter()
            .any(|pn| pn.relation_str() == type_name)
    }
    pub fn embeddable_nodes() -> String {
        let star: &'static str = " *";
        let left: &'static str = " {";
        let right: &'static str = " }";
        let rhs = NodeType::primary_nodes()
            .iter()
            .map(|n| {
                [star]
                    .iter()
                    .chain(&[n.relation_str()])
                    .chain(&[left])
                    .chain(n.fields())
                    .chain(&[right])
                    .join("")
            })
            .join(" or ");
        rhs
    }
    #[cfg(not(feature = "multi_embedding_db"))]
    pub fn embeddable_nodes_now() -> String {
        let star: &'static str = " *";
        let left: &'static str = " {";
        let right: &'static str = " @ 'NOW' }";
        let hash: &'static str = "hash";
        let rhs = NodeType::primary_nodes()
            .iter()
            .map(|n| {
                [star]
                    .iter()
                    .chain(&[n.relation_str()])
                    .chain(&[left])
                    .chain(
                        n.fields()
                            .iter()
                            .filter(|s| {
                                ["id", "name", "tracking_hash", "span", "embedding"].contains(s)
                            })
                            // replace tracking_hash for simplicity in `get_nodes_ordered`
                            .map(|th| if *th == "tracking_hash" { &hash } else { th })
                            .intersperse(&", "),
                    )
                    .chain(&[right])
                    .join("")
            })
            .join(" or ");
        rhs
    }

    #[cfg(feature = "multi_embedding_db")]
    pub const LEGACY_EMBEDDABLE_NODE_FIELDS: [&'static str; 5] =
        ["id", "name", "tracking_hash", "span", "embedding"];

    #[cfg(feature = "multi_embedding_db")]
    pub const EMBEDDABLE_NODE_FIELDS: [&'static str; 4] = ["id", "name", "tracking_hash", "span"];

    #[cfg(feature = "multi_embedding_db")]
    pub fn legacy_embeddable_nodes_now_rhs() -> String {
        let star: &'static str = " *";
        let left: &'static str = " {";
        let right: &'static str = " @ 'NOW' }";
        let hash: &'static str = "hash";
        let rhs = NodeType::primary_nodes()
            .iter()
            .map(|n| {
                [star]
                    .iter()
                    .chain(&[n.relation_str()])
                    .chain(&[left])
                    .chain(
                        n.fields()
                            .iter()
                            .filter(|s| Self::LEGACY_EMBEDDABLE_NODE_FIELDS.contains(s))
                            // // replace tracking_hash for simplicity in `get_nodes_ordered`
                            // .map(|th| if *th == "tracking_hash" { &hash } else { th })
                            .intersperse(&", "),
                    )
                    .chain(&[right])
                    .join("")
            })
            .join(" or ");
        rhs
    }

    #[cfg(feature = "multi_embedding_db")]
    pub fn embeddable_nodes_now() -> String {
        let star: &'static str = " *";
        let left: &'static str = " {";
        let right: &'static str = " @ 'NOW' }";
        let hash: &'static str = "hash";
        let rhs = NodeType::primary_nodes()
            .iter()
            .map(|n| {
                [star]
                    .iter()
                    .chain(&[n.relation_str()])
                    .chain(&[left])
                    .chain(
                        n.fields()
                            .iter()
                            .filter(|s| ["id", "name", "tracking_hash", "span"].contains(s))
                            // // replace tracking_hash for simplicity in `get_nodes_ordered`
                            // .map(|th| if *th == "tracking_hash" { &hash } else { th })
                            .intersperse(&", "),
                    )
                    .chain(&[right])
                    .join("")
            })
            .join(" or ");
        rhs
    }
}

lazy_static::lazy_static! {
    pub static ref EMBEDDABLE_NODES: String = {
        NodeType::embeddable_nodes()
    };
    pub static ref EMBEDDABLE_NODES_NOW: String = {
        NodeType::embeddable_nodes_now()
    };
    pub static ref EMBEDDABLE_NODES_NOW_LEGACY_RHS: String = {
        NodeType::legacy_embeddable_nodes_now_rhs()
    };
}

impl QueryBuilder {
    /// Create a new query builder
    pub fn new() -> Self {
        Self {
            selected_node: None,
            lhs: HashSet::new(),
            rhs_rels: HashMap::new(),
            filters: Vec::new(),
            limit: None,
            custom_lhs: Vec::new(),
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

    /// Add a field value to the lhs (left hand side) of a cozo query for a node, depending on the
    /// currently selected node type in `self.selected_node`.
    /// ```rust
    /// use ploke_db::QueryBuilder;
    /// use ploke_transform::schema::primary_nodes::FunctionNodeSchema;
    /// use cozo::{Db, MemStorage};
    ///
    /// let db = Db::new(MemStorage::default()).expect("Failed to create database");
    /// db.initialize().expect("Failed to initialize database");
    ///
    /// let schema = &FunctionNodeSchema::SCHEMA;
    /// let name_field = schema.name();
    ///
    /// let builder = QueryBuilder::new()
    ///     .structs()
    ///     .add_lhs(name_field);
    ///
    /// assert!(builder.has_field(name_field));
    /// assert!(!builder.has_field("random_name"));
    /// ```
    pub fn add_lhs(mut self, field: &'static str) -> Self {
        if let Some(node_ty) = self.selected_node {
            if node_ty.fields().contains(&field) {
                self.lhs.insert(field);
            } else {
                tracing::warn!(target: LOG_TARGET_QUERY_BUILDER,
                    "Cannot add field {} to {:?}", field, node_ty
                );
            }
        } else {
            tracing::warn!(target: LOG_TARGET_QUERY_BUILDER,
                "Calling add_lhs on None: {}", field
            );
        }
        self
    }

    pub fn insert_lhs_field(&mut self, field: &'static str) {
        if let Some(node_ty) = self.selected_node {
            if node_ty.fields().contains(&field) {
                self.lhs.insert(field);
            } else {
                tracing::warn!(target: LOG_TARGET_QUERY_BUILDER,
                    "Cannot add field {} to {:?}", field, node_ty
                );
            }
        } else {
            tracing::warn!(target: LOG_TARGET_QUERY_BUILDER,
                "Calling add_lhs on None: {}", field
            );
        }
    }

    /// Represents a custom binding in the datalog query, e.g.
    /// ```datalog
    /// ?[
    ///     id,
    ///     name,
    ///     custom_binding
    /// ] := *function {
    ///     id,
    ///     name,
    ///     body: custom_binding
    /// }
    /// ```
    pub fn insert_lhs_custom(&mut self, custom_field: String) {
        if self.selected_node.is_none() {
            self.custom_lhs.push(custom_field);
        } else {
            tracing::warn!(target: LOG_TARGET_QUERY_BUILDER,
                "Calling insert_lhs_custom on Some, when it should only ever be called when the builder's selected_node is None: {}", custom_field
            );
        }
    }

    pub fn lhs_to_query_string(&self) -> String {
        format!("?[\n\t{}\n]", self.lhs.iter().join(",\n\t"))
    }

    pub fn rhs_to_query_string(&self) -> String {
        let mut rhs_string = String::new();
        for (rhs_rel, field_vals) in self.rhs_rels.iter() {
            // push start of relation, e.g. `*function {`
            rhs_string.push('*');
            rhs_string.push_str(rhs_rel.node_type.relation_str());
            rhs_string.push_str(" {\n");

            // process fields, e.g. `name: "func_name", id: first_id, body`
            for field_val in field_vals {
                rhs_string.push('\t');
                rhs_string.push_str(field_val.field);
                if !field_val.value.is_empty() {
                    rhs_string.push(':');
                    rhs_string.push(' ');
                    rhs_string.push_str(&field_val.value);
                }
                rhs_string.push(',');
                rhs_string.push('\n');
            }

            // end the fields section of the rhs
            rhs_string.push('}');
            rhs_string.push(',');
            rhs_string.push('\n');
        }

        rhs_string
    }

    pub fn has_field(&self, field: &'static str) -> bool {
        self.lhs.contains(field)
    }

    /// Infallible insertion of new rhs to `rhs_rels`.
    ///
    /// Checks if the node type is already reprsented in the rhs, and if so will find the highest
    /// node_type_index and increment the count to give an accurate index of the newly added rhs.
    ///
    /// For example, if there is already a `*struct { .. }` in the query's rhs, that first
    /// occurrance of the `struct` relation will be stored in the `rhs_rel` field with the key,
    /// `(NodeType::Struct, 0)`, where `0` is the number of `NodeType::Struct` in the rhs_rels
    /// `HashMap`. When we want to add a second `*struct { .. }` to the rhs, it will be stored with
    /// the key `(NodeType::Struct, 1)`, and each entry will have their own distinct set of
    /// included fields for the rhs.
    ///
    /// Keeping the set of fields tracked for each rhs item separately allows us to have queries
    /// like:
    /// ```datalog
    /// ?[name] :=
    ///     *module { id: first_id, name },
    ///     *module { id: second_id },
    ///     *syntax_edge {
    ///         source_id: first_id,
    ///         target_id: second_id,
    ///         relation_kind: "Contains"
    ///     }
    /// ```
    pub fn insert_rhs_rel(&mut self, node_type: NodeType) {
        let node_type_index = self
            .rhs_rels
            .keys()
            .max_by_key(|rhs| rhs.node_type_index)
            .map(|rhs| rhs.node_type_index + 1)
            .unwrap_or(0);
        let new_rhs = RhsRelation {
            node_type,
            node_type_index,
        };

        self.rhs_rels.insert(new_rhs, Vec::new());
    }

    // /// Execute the constructed query
    // pub fn execute(self) -> Result<QueryResult, Error> {
    //     let relation = match self.selected_node {
    //         Some(NodeType::Function) => FunctionNodeSchema::SCHEMA.relation,
    //         Some(NodeType::Struct) => StructNodeSchema::SCHEMA.relation,
    //         None => {
    //             return Err(Error::from(DbError::QueryConstruction(
    //                 "No node type selected".into(),
    //             )))
    //         }
    //         _ => {
    //             return Err(Error::from(DbError::QueryConstruction(
    //                 "Warning! Node type not yet supported".into(),
    //             )))
    //         }
    //     };
    //
    //     let mut query = match relation {
    //         "functions" => format!(
    //             "?[id, name, visibility, return_type_id, docstring, body] := *{}[id, name, visibility, return_type_id, docstring, body]",
    //             relation
    //         ),
    //         "structs" => format!(
    //             "?[id, name, visibility, docstring] := *{}[id, name, visibility, docstring]",
    //             relation
    //         ),
    //         _ => return Err(Error::from(DbError::QueryConstruction(format!("Unsupported relation: {}", relation)))),
    //     };
    //
    //     if !self.filters.is_empty() {
    //         query.push_str(", ");
    //         query.push_str(&self.filters.join(", "));
    //     }
    //
    //     if let Some(limit) = self.limit {
    //         query.push_str(&format!(" :limit {}", limit));
    //     }
    //
    //     self.db
    //         .run_script(&query, BTreeMap::new(), cozo::ScriptMutability::Immutable)
    //         .map(QueryResult::from)
    //         .map_err(|e| DbError::Cozo(e.to_string()))
    //         .map_err(Error::from)
    // }
}

impl Default for QueryBuilder {
    fn default() -> Self {
        Self::new()
    }
}
use ploke_transform::schema::ID_KEYWORDS;

macro_rules! define_static_fields {
    (
        $(($name:ident, $schema:ty, $node_type:ident)),+
    ) => {
        lazy_static::lazy_static! {
            $(
                static ref $name: String = format!("*{} {{ {} }}", <$schema>::SCHEMA.relation, <$schema>::SCHEMA_FIELDS.join(",\n\t "));
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
            pub fn fields(self) -> &'static [&'static str] {
                match self {
                    $(
                        NodeType::$node_type => <$schema>::SCHEMA_FIELDS
                    ),+
                }
            }
            pub fn relation_str(self) -> &'static str {
                match self {
                    $(
                        NodeType::$node_type => <$schema>::SCHEMA.relation
                    ),+
                }
            }
            /// Re-implementation of original in macro found in ploke-transform/src/schema/mod.rs
            pub fn keys(self) -> impl Iterator<Item = &'static str > {
                self.fields().iter().filter(|f| ID_KEYWORDS.contains(f)).copied()
            }
            /// Re-implementation of original in macro found in ploke-transform/src/schema/mod.rs
            pub fn vals(self) -> impl Iterator<Item = &'static str > {
                self.fields().iter().filter(|f| !ID_KEYWORDS.contains(f)).copied()
            }
            // NOTE: Seems like there should be a way to make this a &'static str, look into it.
            pub fn identity(self) -> String {
                match self {
                    $(
                        NodeType::$node_type => <$schema>::SCHEMA.script_identity()
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
    (MODULE_FIELDS, ModuleNodeSchema, Module),
    (CONST_FIELDS, ConstNodeSchema, Const),
    (IMPL_FIELDS, ImplNodeSchema, Impl),
    (IMPORT_FIELDS, ImportNodeSchema, Import),
    (MACRO_FIELDS, MacroNodeSchema, Macro),
    (STATIC_FIELDS, StaticNodeSchema, Static),
    (TYPE_ALIAS_FIELDS, TypeAliasNodeSchema, TypeAlias),
    (UNION_FIELDS, UnionNodeSchema, Union),
    (PARAM_FIELDS, ParamNodeSchema, Param),
    (VARIANT_FIELDS, VariantNodeSchema, Variant),
    (FIELD_FIELDS, FieldNodeSchema, Field),
    (ATTRIBUTE_FIELDS, AttributeNodeSchema, Attribute),
    (GENERIC_TYPE_FIELDS, GenericTypeNodeSchema, GenericType),
    (
        GENERIC_LIFETIME_FIELDS,
        GenericLifetimeNodeSchema,
        GenericLifetime
    ),
    (GENERIC_CONST_FIELDS, GenericConstNodeSchema, GenericConst),
    (NAMED_TYPE_FIELDS, NamedTypeSchema, NamedType),
    (REFERENCE_TYPE_FIELDS, ReferenceTypeSchema, ReferenceType),
    (SLICE_TYPE_FIELDS, SliceTypeSchema, SliceType),
    (ARRAY_TYPE_FIELDS, ArrayTypeSchema, ArrayType),
    (TUPLE_TYPE_FIELDS, TupleTypeSchema, TupleType),
    (FUNCTION_TYPE_FIELDS, FunctionTypeSchema, FunctionType),
    (NEVER_TYPE_FIELDS, NeverTypeSchema, NeverType),
    (INFERRED_TYPE_FIELDS, InferredTypeSchema, InferredType),
    (
        RAW_POINTER_TYPE_FIELDS,
        RawPointerTypeSchema,
        RawPointerType
    ),
    (
        TRAIT_OBJECT_TYPE_FIELDS,
        TraitObjectTypeSchema,
        TraitObjectType
    ),
    (IMPL_TRAIT_TYPE_FIELDS, ImplTraitTypeSchema, ImplTraitType),
    (PAREN_TYPE_FIELDS, ParenTypeSchema, ParenType),
    (MACRO_TYPE_FIELDS, MacroTypeSchema, MacroType),
    (UNKNOWN_TYPE_FIELDS, UnknownTypeSchema, UnknownType),
    (SYNTAX_EDGE_FIELDS, SyntacticRelationSchema, SyntaxEdge)
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
#[cfg(test)]
mod test {
    use ploke_transform::schema::primary_nodes::StructNodeSchema;

    #[test]
    fn add_lhs() {
        use crate::QueryBuilder;
        use cozo::{Db, MemStorage};

        let db = Db::new(MemStorage::default()).expect("Failed to create database");
        db.initialize().expect("Failed to initialize database");

        let schema = &StructNodeSchema::SCHEMA;
        let name_field = schema.name();

        let builder = QueryBuilder::new()
            .with_name(name_field)
            .structs()
            .add_lhs(name_field);

        eprintln!("{:#?}", builder);
        assert!(builder.has_field(name_field));
        assert!(!builder.has_field("random_field_name"));
    }
}
