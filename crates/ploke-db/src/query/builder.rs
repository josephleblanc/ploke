//! Query builder implementation
#![allow(dead_code)]

use ploke_transform::schema::primary_nodes::{
    ConstNodeSchema, EnumNodeSchema, FunctionNodeSchema, ImplNodeSchema, ImportNodeSchema, MacroNodeSchema, ModuleNodeSchema, StaticNodeSchema, StructNodeSchema, TraitNodeSchema, TypeAliasNodeSchema, UnionNodeSchema
};
use ploke_transform::schema::secondary_nodes::{AttributeNodeSchema, FieldNodeSchema, GenericConstNodeSchema, GenericLifetimeNodeSchema, GenericTypeNodeSchema, ParamNodeSchema, VariantNodeSchema};

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
    (GENERIC_LIFETIME_FIELDS, GenericLifetimeNodeSchema, GenericLifetime),
    (GENERIC_CONST_FIELDS, GenericConstNodeSchema, GenericConst),
    (NAMED_TYPE_FIELDS, NamedTypeSchema, NamedType),
    (REFERENCE_TYPE_FIELDS, ReferenceTypeSchema, ReferenceType),
    (SLICE_TYPE_FIELDS, SliceTypeSchema, SliceType),
    (ARRAY_TYPE_FIELDS, ArrayTypeSchema, ArrayType),
    (TUPLE_TYPE_FIELDS, TupleTypeSchema, TupleType),
    (FUNCTION_TYPE_FIELDS, FunctionTypeSchema, FunctionType),
    (NEVER_TYPE_FIELDS, NeverTypeSchema, NeverType),
    (INFERRED_TYPE_FIELDS, InferredTypeSchema, InferredType),
    (RAW_POINTER_TYPE_FIELDS, RawPointerTypeSchema, RawPointerType),
    (TRAIT_OBJECT_TYPE_FIELDS, TraitObjectTypeSchema, TraitObjectType),
    (IMPL_TRAIT_TYPE_FIELDS, ImplTraitTypeSchema, ImplTraitType),
    (PAREN_TYPE_FIELDS, ParenTypeSchema, ParenType),
    (MACRO_TYPE_FIELDS, MacroTypeSchema, MacroType),
    (UNKNOWN_TYPE_FIELDS, UnknownTypeSchema, UnknownType),
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


