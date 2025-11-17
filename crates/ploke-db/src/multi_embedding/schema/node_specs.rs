use super::*;

#[derive(Copy, Clone)]
pub struct ExperimentalNodeRelationSpec {
    pub name: &'static str,
    pub node_type: NodeType,
    pub metadata_schema: &'static ExperimentalRelationSchema,
    pub vector_relation_base: &'static str,
}

impl ExperimentalNodeRelationSpec {
    pub fn relation_name(&self) -> &'static str {
        self.metadata_schema.relation()
    }
}

pub fn experimental_spec_for_node(
    node_type: NodeType,
) -> Option<&'static ExperimentalNodeRelationSpec> {
    EXPERIMENTAL_NODE_RELATION_SPECS
        .iter()
        .find(|spec| spec.node_type == node_type)
}

fn metadata_projection_fields(spec: &ExperimentalNodeRelationSpec) -> Vec<&'static str> {
    spec.metadata_schema
        .field_names()
        .filter(|field| *field != "embeddings")
        .collect()
}

pub const EXPERIMENTAL_NODE_RELATION_SPECS: [ExperimentalNodeRelationSpec; 12] = [
    ExperimentalNodeRelationSpec {
        name: "function",
        node_type: NodeType::Function,
        metadata_schema: &FUNCTION_MULTI_EMBEDDING_SCHEMA,
        vector_relation_base: "function_embedding_vectors",
    },
    ExperimentalNodeRelationSpec {
        name: "const",
        node_type: NodeType::Const,
        metadata_schema: &CONST_MULTI_EMBEDDING_SCHEMA,
        vector_relation_base: "const_embedding_vectors",
    },
    ExperimentalNodeRelationSpec {
        name: "enum",
        node_type: NodeType::Enum,
        metadata_schema: &ENUM_MULTI_EMBEDDING_SCHEMA,
        vector_relation_base: "enum_embedding_vectors",
    },
    ExperimentalNodeRelationSpec {
        name: "impl",
        node_type: NodeType::Impl,
        metadata_schema: &IMPL_MULTI_EMBEDDING_SCHEMA,
        vector_relation_base: "impl_embedding_vectors",
    },
    ExperimentalNodeRelationSpec {
        name: "import",
        node_type: NodeType::Import,
        metadata_schema: &IMPORT_MULTI_EMBEDDING_SCHEMA,
        vector_relation_base: "import_embedding_vectors",
    },
    ExperimentalNodeRelationSpec {
        name: "macro",
        node_type: NodeType::Macro,
        metadata_schema: &MACRO_MULTI_EMBEDDING_SCHEMA,
        vector_relation_base: "macro_embedding_vectors",
    },
    ExperimentalNodeRelationSpec {
        name: "module",
        node_type: NodeType::Module,
        metadata_schema: &MODULE_MULTI_EMBEDDING_SCHEMA,
        vector_relation_base: "module_embedding_vectors",
    },
    ExperimentalNodeRelationSpec {
        name: "static",
        node_type: NodeType::Static,
        metadata_schema: &STATIC_MULTI_EMBEDDING_SCHEMA,
        vector_relation_base: "static_embedding_vectors",
    },
    ExperimentalNodeRelationSpec {
        name: "struct",
        node_type: NodeType::Struct,
        metadata_schema: &STRUCT_MULTI_EMBEDDING_SCHEMA,
        vector_relation_base: "struct_embedding_vectors",
    },
    ExperimentalNodeRelationSpec {
        name: "trait",
        node_type: NodeType::Trait,
        metadata_schema: &TRAIT_MULTI_EMBEDDING_SCHEMA,
        vector_relation_base: "trait_embedding_vectors",
    },
    ExperimentalNodeRelationSpec {
        name: "type_alias",
        node_type: NodeType::TypeAlias,
        metadata_schema: &TYPE_ALIAS_MULTI_EMBEDDING_SCHEMA,
        vector_relation_base: "type_alias_embedding_vectors",
    },
    ExperimentalNodeRelationSpec {
        name: "union",
        node_type: NodeType::Union,
        metadata_schema: &UNION_MULTI_EMBEDDING_SCHEMA,
        vector_relation_base: "union_embedding_vectors",
    },
];
