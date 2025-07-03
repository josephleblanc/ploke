use super::*;
use crate::define_schema;

//  NOTE: Not exactly sure how to handle attributes and generic params here.
//  Cozo doesn't seem to be very friendly to nested data types, but having an edge to for evey
//  single one also feels kind of... I don't know, wasteful? Semantically frivolous? Reconsider
//  this design at some point.

// TODO: Link to:
//  - generic_param
//  - param_data
//  - return_type_id
define_schema!(FunctionNodeSchema {
    "function",
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
    embedding: "<F32; 384>?"
});

// TODO: Link to:
// - attributes
define_schema!(ConstNodeSchema {
    "const",
    id: "Uuid",
    name: "String",
    span: "[Int; 2]",
    vis_kind: "String",
    vis_path: "[String]?",
    ty_id: "Uuid",
    value: "String?",
    docstring: "String?",
    tracking_hash: "Uuid?",
    cfgs: "[String]",
    embedding: "<F32; 384>?"
}); // need attributes

// TODO: Link to:
//  - variants (VariantNode)
//  - generic_params
//  - attributes
// NOTE: Removed fields `varaints` for now. May return in favor of explicit edges.
define_schema!(EnumNodeSchema {
    "enum",
    id: "Uuid",
    name: "String",
    span: "[Int; 2]",
    vis_kind: "String",
    vis_path: "[String]?",
    docstring: "String?",
    tracking_hash: "Uuid?",
    cfgs: "[String]?",
    variants: "[Uuid]",
    embedding: "<F32; 384>?"
});

// TODO: Link to:
//  - methods
//  - self_type (trait or struct)
//  - generic_params
// TODO: add new field for type of impl (trait/struct)
// NOTE: Should this have attributes?
define_schema!(ImplNodeSchema {
    "impl",
    id: "Uuid",
    self_type: "Uuid",
    span: "[Int; 2]",
    trait_type: "Uuid?",
    methods: "[Uuid]?",
    cfgs: "[String]"
}); // needs methods, trait_type, generic_params linked by uuids

// TODO: Link to:
//  - Re-export type
// NOTE: Should this have attributes? The answer is "Yes, yes it should"
// NOTE: Flattened `ImportKind` into `import_kind` and `vis_kind`, `vis_path`
// NOTE: Including redundant name and visible_name for now. They will always be the same but it
// might help to not surprise me when I look for the name of the import.
define_schema!(ImportNodeSchema {
    "import",
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
});

// TODO: Link to:
//  - attributes
//  - kind (MacroKind)
define_schema!(MacroNodeSchema {
    "macro",
    id: "Uuid",
    name: "String",
    span: "[Int; 2]",
    vis_kind: "String",
    vis_path: "[String]?",
    docstring: "String?",
    body: "String?",
    tracking_hash: "Uuid?",
    cfgs: "[String]",
    kind: "String",
    proc_kind: "String?",
    embedding: "<F32; 384>?"
});

// TODO: Link to:
//  - Contained items
//  - imports
//  - exports
//  - module_def: This might need some thought
//  - attributes
//  - resolves to definition edge from module decl->file-level module
//  NOTE: `items` field likely temporary. Just use it for debugging and don't rely on it in
//  searches. Once I'm convinced we don't need it, we can get rid of it.
define_schema!(ModuleNodeSchema {
    "module",
    id: "Uuid",
    name: "String",
    path: "[String]",
    vis_kind: "String",
    vis_path: "[String]?",
    docstring: "String?",
    span: "[Int; 2]",
    tracking_hash: "Uuid?",
    module_kind: "String",
    cfgs: "[String]",
    embedding: "<F32; 384>?"
});

// TODO: Link to:
//  - attributes
// NOTE: Consider linking to type?
define_schema!(StaticNodeSchema {
    "static",
    id: "Uuid",
    name: "String",
    span: "[Int; 2]",
    vis_kind: "String",
    vis_path: "[String]?",
    ty_id: "Uuid",
    is_mutable: "Bool",
    value: "String?",
    docstring: "String?",
    tracking_hash: "Uuid?",
    cfgs: "[String]",
    embedding: "<F32; 384>?"
});

// TODO: Link to:
//  - fields (FieldNode)
//  - generic_params (GenericParamNode)
//  - attributes (Attribute)
define_schema!(StructNodeSchema {
    "struct",
    id: "Uuid",
    name: "String",
    span: "[Int; 2]",
    vis_kind: "String",
    vis_path: "[String]?",
    docstring: "String?",
    tracking_hash: "Uuid?",
    cfgs: "[String]?",
    embedding: "<F32; 384>?"
});

// TODO: Link to:
//  - generic_params (GenericParamNode)
//  - super_traits (TraitNode)
//  - attributes (Attribute)
//  - methods (MethodNode)
define_schema!(TraitNodeSchema {
    "trait",
    id: "Uuid",
    name: "String",
    span: "[Int; 2]",
    vis_kind: "String",
    vis_path: "[String]?",
    docstring: "String?",
    tracking_hash: "Uuid?",
    cfgs: "[String]?",
    methods: "[Uuid]?",
    embedding: "<F32; 384>?"
});

// TODO: Link to:
//  - generic_params (GenericParamNode)
//  - attributes (Attribute)
define_schema!(TypeAliasNodeSchema {
    "type_alias",
    id: "Uuid",
    name: "String",
    span: "[Int; 2]",
    vis_kind: "String",
    vis_path: "[String]?",
    docstring: "String?",
    tracking_hash: "Uuid?",
    cfgs: "[String]?",
    ty_id: "Uuid",
    embedding: "<F32; 384>?"
});

// TODO: Link to:
//  - fields (FieldNode)
//  - generic_params (GenericParamNode)
//  - attributes (Attribute)
define_schema!(UnionNodeSchema {
    "union",
    id: "Uuid",
    name: "String",
    span: "[Int; 2]",
    vis_kind: "String",
    vis_path: "[String]?",
    docstring: "String?",
    tracking_hash: "Uuid?",
    cfgs: "[String]?",
    embedding: "<F32; 384>?"
});
