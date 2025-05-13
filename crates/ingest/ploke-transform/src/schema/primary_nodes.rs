use super::*;
use crate::define_schema;

// ------------------------------------------------------------
// ----------------- To Implement -----------------------------
// ------------------------------------------------------------
//
// TODO: Change the define_schema attribute to change the name from Camel-case to snake-case
//
// Nodes:
//  - [ ] Const
//      - [✔] Define Schema (*NodeSchema)
//      - [ ] Define tranform
//          - [ ] Basic testing
//  - [ ] Static
//      - [ ] Define Schema (*NodeSchema)
//      - [ ] Define tranform
//          - [ ] Basic testing
//  - [✔] Struct
//      - [✔] Define Schema (*NodeSchema)
//      - [✔] Define tranform
//          - [✔] Basic testing
//      - [✔] Field
//          - [✔] Define Schema (*NodeSchema)
//          - [✔] Add edge (implicit in ParamData owner_id)
//          - [✔] Define tranform
//              - [✔] Basic testing
//          - [ ] Add explicit edges Struct->Field
//  - [✔] Enum
//      - [✔] Define Schema (*NodeSchema)
//      - [✔] Define tranform
//          - [ ] Basic testing
//      - [✔] Variant
//          - [✔] Define Schema (*NodeSchema)
//          - [✔] Add edge (implicit in Variant owner_id)
//          - [ ] Add explicit edge (SyntacticRelation)
//      - [✔] Field (if different from struct field)
//          - [✔] Add edge (implicit in Field owner_id)
//          - [ ] Add edge
//  - [✔] Function
//      - [✔] Define tranform
//          - [✔] Basic testing
//      - [✔] Define Schema (*NodeSchema)
//      - [✔] ParamData
//          - [✔] Define tranform
//              - [✔] Basic testing
//          - [✔] Define Schema (*NodeSchema)
//          - [✔] Add edge (implicit in ParamData owner_id)
//          - [ ] Add explecit edge Function->ParamData
//  - [✔] Impl
//      - [✔] Define Schema (*NodeSchema)
//      - [✔] Define tranform
//          - [✔] Basic testing
//      - [✔] Method
//          - [✔] Define Schema (*NodeSchema)
//          - [✔] Define tranform
//              - [ ] Basic testing
//          - [✔] Add edge (implicit in method field: owner_id)
//          - [✔] Add edge (implicit in impl field: methods)
//          - [ ] Add explicit edge
//  - [ ] Macro
//      - [✔] Define Schema (*NodeSchema)
//      - [ ] Define tranform
//          - [ ] Basic testing
//  - [ ] Module (split into FileModuleNode, InlineModuleNode, DeclModuleNode)
//      - [✔] Define Schema (FileModuleNodeSchema)
//      - [ ] Define tranform
//          - [ ] Basic testing
//      - [✔] Define Schema (InlineModuleNodeSchema)
//      - [ ] Define tranform
//          - [ ] Basic testing
//      - [✔] Define Schema (DeclModuleNodeSchema)
//      - [ ] Define tranform
//          - [ ] Basic testing
//  - [ ] Trait
//      - [✔] Define Schema (*NodeSchema)
//      - [ ] Define tranform
//          - [ ] Basic testing
//      - [ ] Method (if different from impl method)
//          - [ ] Define tranform
//              - [ ] Basic testing
//          - [✔] Define Schema (*NodeSchema)
//          - [ ] Add edge
//  - [ ] TypeAlias
//      - [✔] Define Schema (*NodeSchema)
//      - [ ] Define tranform
//          - [ ] Basic testing
//  - [ ] Import
//      - [✔] Define Schema (*NodeSchema)
//      - [ ] Define tranform
//          - [ ] Basic testing
//
// Add Schema definitions for Associated Nodes:
//  - [✔] MethodNodeSchema
//  - [ ] AssociatedConstNode (not tracked yet)
//  - [ ] AssociatedStaticNode (not tracked yet)
//  - [ ] AssociatedFunctionNode (not tracked yet)
//      - I think we don't track this yet? Check on this.
//
//  - [✔] ParamData
//      - [✔] Add function to turn into BTree
//  - [✔] VariantNode
//      - [✔] Add function to turn into BTree
//  - [✔] FieldNode
//      - [✔] Add function to turn into BTree
//  - [✔] GenericParamNode
//      - [✔] GenericTypeNodeSchema
//      - [✔] GenericLifetimeNodeSchema
//      - [✔] GenericConstNodeSchema
//      - [✔] Add function to turn into BTree
//  - [✔] Attribute
//      - [✔] Add function to turn into BTree
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
    module_id: "Uuid"
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
    cfgs: "[String]"
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
});

// TODO: Link to:
//  - methods
//  - self_type
//  - generic_params
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
// NOTE: Flattened `ImportKind` into `import_kind` and `vis_kind`, `vis_path`
define_schema!(ImportNodeSchema {
    "import",
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
    "macro",
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
    "module",
    id: "Uuid",
    name: "String",
    path: "[String]",
    vis_kind: "String",
    vis_path: "[String]?",
    docstring: "String?",
    span: "[Int; 2]",
    tracking_hash: "Uuid?",
    module_kind: "String"
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
    "struct",
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
    "trait",
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
    "type_alias",
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
    "union",
    id: "Uuid",
    name: "String",
    span: "[Int; 2]",
    vis_kind: "String",
    vis_path: "[String]?",
    docstring: "String?",
    tracking_hash: "Uuid?",
    cfgs: "[String]?",
});
