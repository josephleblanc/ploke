use super::*;
use crate::define_schema;

// TODO: Add more fields from the CrateContext struct as individual nodes that are pointed at by
// this or vice versa.
// WARNING: id is same as namespace right now. We should have a better solution
define_schema!(CrateContextSchema {
    "crate_context",
    id: "Uuid",
    name: "String",
    version: "String",
    namespace: "Uuid",
    root_path: "String",
    files: "[String]",
});
