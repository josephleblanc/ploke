use crate::define_schema;

use super::*;

// NOTE: Keeping `items` field specifically for file-level modules for now, however I may want to just
// remove this in favor of explicit relations later.
//  - Try not to rely on this for queries, but it might be good for debugging for now.
define_schema!(FileModuleNodeSchema {
    "file_mod",
    owner_id: "Uuid",
    file_path: "String",
    file_docs: "String?",
    items: "[Uuid]",
    namespace: "Uuid",
});
