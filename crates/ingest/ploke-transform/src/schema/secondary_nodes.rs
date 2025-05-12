use super::*;
use crate::define_schema;

// NOTE: Likely the following fields are temporary.
//  - function_id (id of the owning function)
//  - param_index (could be rolled into the name? might be better here.)
// TODO: Test and evaluate with queries.
define_schema!(ParamNodeSchema {
    "param",
    name: "String",
    function_id: "Uuid",
    param_index: "Int",
    type_id: "Uuid",
    is_mutable: "Bool",
    is_self: "Bool"
});

// TODO: Link to:
//  - fields (FieldNode)
//  - attributes (Attribute)
//  -> Change discriminant to Int
define_schema!(VariantNodeSchema {
    "variant",
    id: "Uuid",
    name: "String",
    discriminant: "String?",
    cfgs: "[String]?"
});

// TODO: Link to:
//  - attributes (Attribute)
//
// Added:
//  - owner_id
//  - index
define_schema!(FieldNodeSchema {
    "field",
    id: "Uuid",
    name: "String",
    owner_id: "Uuid",
    index: "Int",
    type_id: "Uuid",
    vis_kind: "String",
    vis_path: "[String]?",
    cfgs: "[String]?"
});

// NOTE: Leaving it in for now, but may remove
//  - owner_id (AnyNodeId)
//  - attr_index (possibly, could actually be useful)
define_schema!(AttributeNodeSchema {
    "attribute",
    name: "String",
    owner_id: "Uuid",
    index: "Int",
    args: "[String]",
    value: "String?"
});

// NOTE: Split GenericParamNode into:
//  - GenericTypeNode
//  - GenericLifetimeNode
//  - GenericConstNode
// NOTE: Likely temporary, consider removing later
//  - owner_id (will likely remove after debugging)
//  - param_index (might want to keep this one)
define_schema!(GenericTypeNodeSchema {
    "generic_type",
    id: "Uuid",
    name: "String",
    owner_id: "Uuid",
    param_index: "Int",
    kind: "String",
    bounds: "[Uuid]",
    default: "Uuid?",
});

define_schema!(GenericLifetimeNodeSchema {
    "generic_lifetime",
    id: "Uuid",
    name: "String",
    owner_id: "Uuid",
    param_index: "Int",
    kind: "String",
    bounds: "[Uuid]",
});

define_schema!(GenericConstNodeSchema {
    "generic_const",
    id: "Uuid",
    name: "String",
    owner_id: "Uuid",
    param_index: "Int",
    kind: "String",
    type_id: "Uuid",
});
