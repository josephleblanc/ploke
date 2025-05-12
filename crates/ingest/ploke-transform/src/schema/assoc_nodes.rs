use super::*;
use crate::define_schema;

// TODO: Link to:
//  - parameters (ParamData)
//  - generic_params (GenericParamNode)
//  - attributes (Attribute)
define_schema!(MethodNodeInfo {
    "method",
    id: "Uuid",
    name: "String",
    span: "[Int; 2]",
    vis_kind: "String",
    vis_path: "[String]?",
    docstring: "String?",
    body: "String?",
    tracking_hash: "Uuid",
    cfgs: "[String]"
});
