use super::*;
use crate::define_schema;

// ------------------------------------------------------------
// ----------------- To Implement -----------------------------
// ------------------------------------------------------------
// Nodes:
//  - [ ] Const
//  - [ ] Struct
//      - [ ] Field
//  - [ ] Enum
//      - [ ] Variant
//      - [ ] Field (if different from struct field)
//  - [✔] Function
//      - [ ] ParamData
//  - [ ] Impl
//      - [ ] Method
//  - [ ] Macro
//  - [ ] Module
//      - [ ] ModuleKind (?)
//  - [ ] Trait
//      - [ ] Method (if different from impl method)
//  - [ ] TypeAlias
//  - [ ] Import
//
//  NOTE: Not exactly sure how to handle attributes and generic params here.
//  Cozo doesn't seem to be very friendly to nested data types, but having an edge to for evey
//  single one also feels kind of... I don't know, wasteful? Semantically frivolous? Reconsider
//  this design at some point.

// TODO: Link to:
//  - generic_param
//  - param_data
//  - return_type_id
define_schema!(FunctionNodeSchema {
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
    module_id: "Uuid"
});

// TODO: Link to:
// - attributes
define_schema!(ConstNodeSchema {
    id: "Uuid",
    name: "String",
    span: "[Int; 2]",
    vis_kind: "String",
    vis_path: "[String]?",
    type_id: "Uuid",
    value: "String?",
    docstring: "String?",
    tracking_hash: "Uuid?",
    cfgs: "[String]"
}); // need attributes

// TODO: Link to:
//  - variants (VariantNode)
//  - generic_params
//  - attributes
define_schema!(EnumNodeSchema {
    id: "Uuid",
    name: "String",
    span: "[Int; 2]",
    vis_kind: "String",
    vis_path: "[String]?",
    docstring: "String?",
    tracking_hash: "Uuid?",
    cfgs: "[String]",
    variants: "[Uuid]?",
});

// TODO: Link to:
//  - methods
//  - self_type
//  - self type (if applicable)
//  - generic_params
define_schema!(ImplNodeSchema {
    id: "Uuid",
    self_type: "Uuid",
    span: "[Int; 2]",
    trait_type: "Uuid?",
    methods: "Uuid",
    cfgs: "[String]"
}); // needs methods, trait_type, generic_params linked by uuids

// TODO: Link to:
//  - Re-export type
// NOTE: Flattened `ImportKind` into `import_kind` and `vis_kind`, `vis_path`
define_schema!(ImportNodeSchema {
    id: "Uuid",
    span: "[Int; 2]",
    source_path: "[String]",
    kind: "Uuid",
    visible_name: "String",
    original_name: "String?",
    is_glob: "Bool",
    import_kind: "String",
    vis_kind: "String?",
    vis_path: "[String]?"
});

// TODO: Link to:
//  - attributes
//  - kind (MacroKind)
define_schema!(MacroNodeSchema {
    id: "Uuid",
    name: "String",
    span: "[Int; 2]",
    vis_kind: "String",
    vis_path: "[String]?",
    docstring: "String?",
    body: "String?",
    tracking_hash: "Uuid?",
    cfgs: "[String]"
});

// TODO: Link to:
//  - Contained items
//  - imports
//  - exports
//  - module_def: This might need some thought
//  - attributes
// NOTE: Not exactly sure how to handle module_def here.
// Might even just want to make different schema for the different module types here: decl, defn,
// and inline. Linking to another node that has the details on the module might be kind of awkward.
// This is one case that will be significantly clarified with a "logical" layer to the graph.
define_schema!(ModuleNodeSchema {
    id: "Uuid",
    name: "String",
    path: "[String]",
    vis_kind: "String",
    vis_path: "[String]?",
    docstring: "String?",
    span: "[Int; 2]",
    tracking_hash: "Uuid?",
    module_def: "String"
});

// TODO: Link to:
//  - attributes
// NOTE: Consider linking to type?
define_schema!(StaticNodeSchema {
    id: "Uuid",
    name: "String",
    span: "[Int; 2]",
    vis_kind: "String",
    vis_path: "[String]?",
    type_id: "Uuid",
    is_mutable: "Bool",
    value: "String?",
    docstring: "String?",
    tracking_hash: "Uuid?",
    cfgs: "[String]"
});

// TODO: Link to:
//  - fields (FieldNode)
//  - generic_params (GenericParamNode)
//  - attributes (Attribute)
define_schema!(StructNodeSchema {
    id: "Uuid",
    name: "String",
    span: "[Int; 2]",
    vis_kind: "String",
    vis_path: "[String]?",
    docstring: "String?",
    tracking_hash: "Uuid?",
    cfgs: "[String]?"
});

// TODO: Link to:
//  - generic_params (GenericParamNode)
//  - super_traits (TraitNode)
//  - attributes (Attribute)
//  - methods (MethodNode)
define_schema!(TraitNodeSchema {
    id: "Uuid",
    name: "String",
    span: "[Int; 2]",
    vis_kind: "String",
    vis_path: "[String]?",
    docstring: "String?",
    tracking_hash: "Uuid?",
    cfgs: "[String]?"
});

// TODO: Link to:
//  - generic_params (GenericParamNode)
//  - attributes (Attribute)
define_schema!(TypeAliasNodeSchema {
    id: "Uuid",
    name: "String",
    span: "[Int; 2]",
    vis_kind: "String",
    vis_path: "[String]?",
    docstring: "String?",
    tracking_hash: "Uuid?",
    cfgs: "[String]?",
    type_id: "Uuid",
});

// TODO: Link to:
//  - fields (FieldNode)
//  - generic_params (GenericParamNode)
//  - attributes (Attribute)
define_schema!(UnionNodeSchema {
    id: "Uuid",
    name: "String",
    span: "[Int; 2]",
    vis_kind: "String",
    vis_path: "[String]?",
    docstring: "String?",
    tracking_hash: "Uuid?",
    cfgs: "[String]?",
});
