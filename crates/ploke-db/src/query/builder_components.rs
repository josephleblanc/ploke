use ploke_transform::schema::primary_nodes::FunctionNodeSchema;

use super::*;

pub struct BuilderRhs<'a> {
    relation: &'static str,
    keys: &'a [ &'static str ],
}

// impl<'a> BuilderRhs<'a> {
//     pub fn all_functions() {
//         let schema = FunctionNodeSchema::SCHEMA;
//         let keys = schema.Err
//     }
// }

