use super::*;
use crate::define_schema;

// TODO: Link to:
//  - parameters (ParamData)
//  - generic_params (GenericParamNode)
//  - attributes (Attribute)
// NOTE: Temporary field (probably remove later)
//  - owner_id (impl id)
define_schema!(MethodNodeSchema {
    "method",
    id: "Uuid",
    name: "String",
    span: "[Int; 2]",
    vis_kind: "String",
    vis_path: "[String]?",
    docstring: "String?",
    body: "String?",
    tracking_hash: "Uuid",
    cfgs: "[String]",
    owner_id: "Uuid",
    embedding: "<F32; 384>?"
});
