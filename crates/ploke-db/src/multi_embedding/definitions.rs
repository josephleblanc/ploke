macro_rules! define_relation_schema {
    ($const_name:ident {
        $relation:literal,
        $($field_name:ident: $dv:literal),+ $(,)?
    }) => {
        pub const $const_name: ExperimentalRelationSchema = ExperimentalRelationSchema {
            relation: $relation,
            fields: &[
                $(CozoField::new(stringify!($field_name), $dv)),+
            ],
        };
    };
}

define_relation_schema!(FUNCTION_MULTI_EMBEDDING_SCHEMA {
    "function_multi_embedding",
    id: "Uuid",
    name: "String",
    docstring: "String?",
    vis_kind: "String",
    vis_path: "[String]?",
    span: "[Int; 2]",
    tracking_hash: "Uuid",
    cfgs: "[String]",
    return_type_id: "Uuid?",
    body: "String?",
    module_id: "Uuid",
    embeddings: "[(String, Int)]"
});

define_relation_schema!(CONST_MULTI_EMBEDDING_SCHEMA {
    "const_multi_embedding",
    id: "Uuid",
    name: "String",
    span: "[Int; 2]",
    vis_kind: "String",
    vis_path: "[String]?",
    ty_id: "Uuid",
    value: "String?",
    docstring: "String?",
    tracking_hash: "Uuid",
    cfgs: "[String]",
    embeddings: "[(String, Int)]"
});

define_relation_schema!(ENUM_MULTI_EMBEDDING_SCHEMA {
    "enum_multi_embedding",
    id: "Uuid",
    name: "String",
    span: "[Int; 2]",
    vis_kind: "String",
    vis_path: "[String]?",
    docstring: "String?",
    tracking_hash: "Uuid",
    cfgs: "[String]?",
    variants: "[Uuid]",
    embeddings: "[(String, Int)]"
});

define_relation_schema!(IMPL_MULTI_EMBEDDING_SCHEMA {
    "impl_multi_embedding",
    id: "Uuid",
    self_type: "Uuid",
    span: "[Int; 2]",
    trait_type: "Uuid?",
    methods: "[Uuid]?",
    cfgs: "[String]",
    embeddings: "[(String, Int)]"
});

define_relation_schema!(IMPORT_MULTI_EMBEDDING_SCHEMA {
    "import_multi_embedding",
    id: "Uuid",
    name: "String",
    span: "[Int; 2]",
    vis_kind: "String?",
    vis_path: "[String]?",
    cfgs: "[String]",
    source_path: "[String]",
    visible_name: "String",
    original_name: "String?",
    is_glob: "Bool",
    is_self_import: "Bool",
    import_kind: "String",
    embeddings: "[(String, Int)]"
});

define_relation_schema!(MACRO_MULTI_EMBEDDING_SCHEMA {
    "macro_multi_embedding",
    id: "Uuid",
    name: "String",
    span: "[Int; 2]",
    vis_kind: "String",
    vis_path: "[String]?",
    docstring: "String?",
    body: "String?",
    tracking_hash: "Uuid",
    cfgs: "[String]",
    kind: "String",
    proc_kind: "String?",
    embeddings: "[(String, Int)]"
});

define_relation_schema!(MODULE_MULTI_EMBEDDING_SCHEMA {
    "module_multi_embedding",
    id: "Uuid",
    name: "String",
    path: "[String]",
    vis_kind: "String",
    vis_path: "[String]?",
    docstring: "String?",
    span: "[Int; 2]",
    tracking_hash: "Uuid",
    module_kind: "String",
    cfgs: "[String]",
    embeddings: "[(String, Int)]"
});

define_relation_schema!(STATIC_MULTI_EMBEDDING_SCHEMA {
    "static_multi_embedding",
    id: "Uuid",
    name: "String",
    span: "[Int; 2]",
    vis_kind: "String",
    vis_path: "[String]?",
    ty_id: "Uuid",
    is_mutable: "Bool",
    value: "String?",
    docstring: "String?",
    tracking_hash: "Uuid",
    cfgs: "[String]",
    embeddings: "[(String, Int)]"
});

define_relation_schema!(STRUCT_MULTI_EMBEDDING_SCHEMA {
    "struct_multi_embedding",
    id: "Uuid",
    name: "String",
    span: "[Int; 2]",
    vis_kind: "String",
    vis_path: "[String]?",
    docstring: "String?",
    tracking_hash: "Uuid",
    cfgs: "[String]?",
    embeddings: "[(String, Int)]"
});

define_relation_schema!(TRAIT_MULTI_EMBEDDING_SCHEMA {
    "trait_multi_embedding",
    id: "Uuid",
    name: "String",
    span: "[Int; 2]",
    vis_kind: "String",
    vis_path: "[String]?",
    docstring: "String?",
    tracking_hash: "Uuid",
    cfgs: "[String]?",
    methods: "[Uuid]?",
    embeddings: "[(String, Int)]"
});

define_relation_schema!(TYPE_ALIAS_MULTI_EMBEDDING_SCHEMA {
    "type_alias_multi_embedding",
    id: "Uuid",
    name: "String",
    span: "[Int; 2]",
    vis_kind: "String",
    vis_path: "[String]?",
    docstring: "String?",
    tracking_hash: "Uuid",
    cfgs: "[String]?",
    ty_id: "Uuid",
    embeddings: "[(String, Int)]"
});

define_relation_schema!(UNION_MULTI_EMBEDDING_SCHEMA {
    "union_multi_embedding",
    id: "Uuid",
    name: "String",
    span: "[Int; 2]",
    vis_kind: "String",
    vis_path: "[String]?",
    docstring: "String?",
    tracking_hash: "Uuid",
    cfgs: "[String]?",
    embeddings: "[(String, Int)]"
});
