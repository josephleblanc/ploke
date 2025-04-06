// Define a stable PROJECT_NAMESPACE UUID.
// Moved from syn_parser::discovery
// Generated via `uuidgen`: f7f4a9a0-1b1a-4b0e-9c1a-1a1a1a1a1a1a
#[cfg(feature = "uuid_ids")]
pub const PROJECT_NAMESPACE_UUID: uuid::Uuid = uuid::Uuid::from_bytes([
    0xf7, 0xf4, 0xa9, 0xa0, 0x1b, 0x1a, 0x4b, 0x0e, 0x9c, 0x1a, 0x1a, 0x1a, 0x1a, 0x1a, 0x1a, 0x1a,
]);

#[cfg(feature = "uuid_ids")]
mod ids {
    use std::str::Bytes;

    use serde::{Deserialize, Serialize};
    use uuid::Uuid;

    use crate::PROJECT_NAMESPACE_UUID;

    /// Unique identifier for code elements (functions, structs, modules, etc.).
    /// - `Path`: Stable ID based on the item's absolute path within the project/crate namespace.
    /// - `Synthetic`: Temporary ID generated during parallel parsing, resolved later to `Path` if possible.
    ///     - formed from project_namespace as namespace
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, PartialOrd, Ord)]
    pub enum NodeId {
        Path(Uuid),
        // Synthetic formed from project_namespace as namespace and:
        //  - file_path bytes for e.g. crate/module_dir1/module_dir2/file_name.resolved
        //  - relative module path for e.g. ["crate", "mod1", "mod2"]
        //  - item name, e.g. "SomeStruct" or "function_name"
        Synthetic(Uuid),
    }
    impl NodeId {
        // Good for items that won't have the same name in the same module
        //  - e.g. not good for function parameters, enum variant, struct field
        //  - good for funciton, struct, enum, etc
        pub fn generate_synthetic(
            crate_namespace: uuid::Uuid,
            file_path: &std::path::Path,
            relative_path: &[String],
            item_name: &str,
        ) -> Self {
            let fp_bytes: &[u8] = file_path.as_os_str().as_encoded_bytes();
            let synthetic_data: Vec<u8> = crate_namespace
                .as_bytes()
                .iter()
                .chain(fp_bytes)
                .chain(relative_path.join("::").as_bytes())
                .chain(item_name.as_bytes())
                .copied()
                .collect();

            Self::Synthetic(uuid::Uuid::new_v5(&PROJECT_NAMESPACE_UUID, &synthetic_data))
        }
        // Possibly useful but more likely to be too fine-grained to allow for incremental updates
        // Only here for now as a possible alternative. Probably delete/move into TrackingHash
        // instead.
        pub fn generate_synthetic_with_span(
            crate_namespace: uuid::Uuid,
            file_path: &std::path::Path,
            relative_path: &[String],
            item_name: &str,
            span: (usize, usize),
        ) -> Self {
            let fp_bytes: &[u8] = file_path.as_os_str().as_encoded_bytes();
            let span_start_bytes = span.0.to_le_bytes(); // use consistent byte order
            let span_end_bytes = span.1.to_le_bytes();
            let synthetic_data: Vec<u8> = crate_namespace
                .as_bytes()
                .iter()
                .chain(fp_bytes)
                .chain(relative_path.join("::").as_bytes())
                .chain(item_name.as_bytes())
                .chain(&span_start_bytes)
                .chain(&span_end_bytes)
                .copied()
                .collect();

            Self::Synthetic(uuid::Uuid::new_v5(&PROJECT_NAMESPACE_UUID, &synthetic_data))
        }
    }

    /// Unique identifier for a specific type structure *within a specific crate version*.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
    pub struct TypeId {
        /// Namespace UUID of the crate defining the type.
        pub crate_id: Uuid,
        /// UUID representing the canonical type structure within that crate.
        pub type_id: Uuid,
    }

    /// Stable identifier for a type's logical identity across crate versions.
    /// Primarily used for linking embeddings or tracking concepts over time.
    /// Can be generated based on project namespace, crate name, and type path.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
    pub struct LogicalTypeId(pub Uuid);

    /// Hash representing the meaningful content of a code node (e.g., function body).
    /// Used to detect changes for incremental processing, ignoring whitespace/comments.
    /// Represented as a Uuid for convenience in database storage and comparison.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
    pub struct TrackingHash(pub Uuid);

    // Consider adding helper methods like `is_synthetic()` to NodeId if needed.
}

#[cfg(feature = "uuid_ids")]
pub use ids::*;

// --- Fallback definitions when uuid_ids feature is NOT enabled ---

#[cfg(not(feature = "uuid_ids"))]
mod ids_compat {
    // Define NodeId and TypeId as usize for compatibility with the old system.
    // Add other necessary derives if the old system used them (e.g., Copy, Default).
    pub type NodeId = usize;
    pub type TypeId = usize;

    // LogicalTypeId and TrackingHash don't exist in the old system.
    // We could define dummy types or just not define them.
    // Let's not define them for now to make compile errors clearer
    // if code accidentally tries to use them without the flag.
    // pub struct LogicalTypeId; // Placeholder if needed
    // pub struct TrackingHash; // Placeholder if needed
}

#[cfg(not(feature = "uuid_ids"))]
pub use ids_compat::*;
