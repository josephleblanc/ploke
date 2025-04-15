// Define a stable PROJECT_NAMESPACE UUID.
// Moved from syn_parser::discovery
// Generated via `uuidgen`: f7f4a9a0-1b1a-4b0e-9c1a-1a1a1a1a1a1a
pub const PROJECT_NAMESPACE_UUID: uuid::Uuid = uuid::Uuid::from_bytes([
    0xf7, 0xf4, 0xa9, 0xa0, 0x1b, 0x1a, 0x4b, 0x0e, 0x9c, 0x1a, 0x1a, 0x1a, 0x1a, 0x1a, 0x1a, 0x1a,
]);

mod ids {
    use std::path::Path;

    use serde::{Deserialize, Serialize};
    use uuid::Uuid;

    use crate::PROJECT_NAMESPACE_UUID;

    /// Unique identifier for code elements (functions, structs, modules, etc.).
    /// - `Resolved`: Stable ID based on the item's absolute path within the project/crate namespace.
    /// - `Synthetic`: Temporary ID generated during parallel parsing, resolved later to `Path` if possible.
    ///     - formed from project_namespace as namespace
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, PartialOrd, Ord)]
    pub enum NodeId {
        // Resolved formed from project_namespace as namespace and:
        //  - file_path bytes for e.g. crate/module_dir1/module_dir2/file_name.resolved
        //  - cannonical module path for e.g. ["crate", "mod1", "mod2"]
        //     - Fully resolved in Phase 3 of parsing after module tree created
        //     - Guarenteed to resolve for all nodes in Phase 3
        //  - item name, e.g. "SomeStruct" or "function_name"
        Resolved(Uuid),
        // Synthetic formed from project_namespace as namespace and:
        //  - file_path bytes for e.g. crate/module_dir1/module_dir2/file_name.resolved
        //  - relative module path for e.g. ["mod1", "mod2"]
        //      - Due to possibility of re-exports and aliases from other files,
        //        e.g. a mod.rs in same dir with "pub us mod_z::mod_y as mod1",
        //        cannot guarentee module path to correctly resolve at parse-time.
        //  - item name, e.g. "SomeStruct" or "function_name"
        //  - span start/end
        Synthetic(Uuid),
    }
    impl NodeId {
        // Good for items that won't have the same name in the same module
        //  - e.g. not good for function parameters, enum variant, struct field (not nodes)
        //  - good for funciton, struct, enum, etc (nodes)
        pub fn generate_resolved(
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
        /// Generates a temporary `Synthetic` `NodeId` based on stable context.
        ///
        /// This ID is used during the parallel parsing phase (Phase 2) before full
        /// name resolution is available. It aims to be deterministic and stable
        /// against formatting changes by excluding `span` information.
        ///
        /// # Arguments
        /// * `crate_namespace` - The UUID namespace of the crate being parsed.
        /// * `file_path` - The absolute path to the file containing the item.
        /// * `relative_path` - The logical module path within the file (e.g., `["inner_mod"]`).
        /// * `item_name` - The name of the item (e.g., function name, struct name).
        /// * `item_kind` - The kind of item (e.g., `ItemKind::Function`, `ItemKind::Struct`).
        ///   Used for disambiguation (e.g., function `foo` vs struct `foo`).
        /// * `parent_scope_id` - The `NodeId` of the immediate parent scope (e.g., the module
        ///   containing a function, or the struct containing a field). `None` for top-level
        ///   items within a file (like the root module itself).
        ///
        /// # Returns
        /// A `NodeId::Synthetic` variant containing a UUIDv5 hash derived from the inputs.
        pub fn generate_synthetic(
            crate_namespace: uuid::Uuid,
            file_path: &std::path::Path,
            relative_path: &[String],
            item_name: &str,
            item_kind: crate::ItemKind, // Use ItemKind from this crate
            parent_scope_id: Option<NodeId>,
        ) -> Self {
            let fp_bytes: &[u8] = file_path.as_os_str().as_encoded_bytes();
            // Use discriminant of ItemKind for hashing (stable and simple)
            let item_kind_bytes = (item_kind as u8).to_le_bytes();

            // Get bytes for parent_scope_id, using a placeholder for None
            let parent_id_bytes = match parent_scope_id {
                Some(NodeId::Resolved(uuid)) => *uuid.as_bytes(),
                Some(NodeId::Synthetic(uuid)) => *uuid.as_bytes(),
                None => [0u8; 16], // Placeholder for None
            };

            let synthetic_data: Vec<u8> = crate_namespace
                .as_bytes()
                .iter()
                .chain(b"::FILE::")
                .chain(fp_bytes)
                .chain(b"::REL_PATH::")
                .chain(relative_path.join("::").as_bytes())
                .chain(b"::PARENT_ID::")
                .chain(&parent_id_bytes) // Add parent ID bytes
                .chain(b"::KIND::")
                .chain(&item_kind_bytes) // Add item kind bytes
                .chain(b"::NAME::")
                .chain(item_name.as_bytes())
                .copied()
                .collect();

            Self::Synthetic(uuid::Uuid::new_v5(&PROJECT_NAMESPACE_UUID, &synthetic_data))
        }
    }
    impl std::fmt::Display for NodeId {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            match self {
                NodeId::Resolved(uuid) => write!(f, "R:{}", short_uuid(*uuid)),
                NodeId::Synthetic(uuid) => write!(f, "S:{}", short_uuid(*uuid)),
            }
        }
    }

    fn short_uuid(uuid: Uuid) -> String {
        let fields = uuid.as_fields();
        // First 4 bytes (as u32) and last 4 bytes (from the 8-byte array)
        format!(
            "{:08x}..{:02x}{:02x}{:02x}{:02x}",
            fields.0, fields.3[4], fields.3[5], fields.3[6], fields.3[7]
        )
    }

    // impl std::fmt::Display for NodeId {
    //     fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    //         match self {
    //             NodeId::Resolved(uuid) => write!(f, "R:{}", short_uuid(*uuid)),
    //             NodeId::Synthetic(uuid) => write!(f, "S:{}", short_uuid(*uuid)),
    //         }
    //     }
    // }
    //
    // fn short_uuid(uuid: Uuid) -> String {
    //     let bytes = uuid.as_fields().0;
    //     format!("{:x}..{:x}", bytes[0], bytes[3])
    // }

    /// Unique identifier for a specific type structure *within a specific crate version*.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, PartialOrd, Ord)]
    pub enum TypeId {
        Resolved(Uuid),
        Synthetic(Uuid),
    }
    impl TypeId {
        /// Generates a temporary Synthetic TypeId based on the context where a type
        /// is used and a string representation of that type.
        ///
        /// # Arguments
        /// * `crate_namespace` - The Uuid namespace of the crate where the usage occurs.
        /// * `file_path` - The path to the file where the usage occurs.
        /// * `type_string_repr` - A consistent string representation of the syn::Type
        ///   (typically generated using `ty.to_token_stream().to_string()`).
        pub fn generate_synthetic(
            crate_namespace: Uuid,
            file_path: &Path,
            type_string_repr: &str,
        ) -> Self {
            // Use as_encoded_bytes() for potentially non-UTF8 paths
            let fp_bytes = file_path.as_os_str().as_encoded_bytes();

            // Combine namespace, file path bytes, and type string bytes.
            // Using a separator helps ensure distinctness if components could overlap,
            // though UUIDv5 hashing is generally robust.
            let synthetic_data: Vec<u8> = crate_namespace
                .as_bytes()
                .iter()
                .chain(b"::FILE::") // Separator
                .chain(fp_bytes)
                .chain(b"::TYPE::") // Separator
                .chain(type_string_repr.as_bytes())
                .copied()
                .collect();

            // Generate the UUIDv5 using the project's root namespace.
            let type_uuid = uuid::Uuid::new_v5(&PROJECT_NAMESPACE_UUID, &synthetic_data);

            // Return the Synthetic variant containing the generated UUID.
            Self::Synthetic(type_uuid)
        }

        /// Generates a temporary Synthetic TypeId specifically for usages of `Self` or
        /// generic parameters within a particular definition context (e.g., struct, fn, impl).
        /// This ensures that `Self` in `impl A` gets a different TypeId than `Self` in `impl B`,
        /// and `T` used within `Foo<T>` gets a different TypeId than `T` used within `Bar<T>`.
        ///
        /// # Arguments
        /// * `crate_namespace` - The Uuid namespace of the crate where the usage occurs.
        /// * `file_path` - The path to the file where the usage occurs.
        /// * `context_definition_id` - The `NodeId` of the item (struct, fn, impl, etc.)
        ///   within which this `Self` or generic parameter is being used.
        /// * `parameter_marker` - A byte slice distinguishing the parameter (e.g., `b"SELF"`
        ///   or `b"GENERIC:T"`).
        pub fn generate_contextual_synthetic(
            crate_namespace: Uuid,
            file_path: &Path,
            context_definition_id: NodeId,
            parameter_marker: &[u8],
        ) -> Self {
            let fp_bytes = file_path.as_os_str().as_encoded_bytes();

            // Extract the UUID bytes from the NodeId regardless of variant
            let context_id_bytes = match context_definition_id {
                NodeId::Resolved(uuid) => *uuid.as_bytes(),
                NodeId::Synthetic(uuid) => *uuid.as_bytes(),
            };

            // Combine namespace, file path, context ID, and the parameter marker.
            // Using separators helps ensure distinctness.
            let synthetic_data: Vec<u8> = crate_namespace
                .as_bytes()
                .iter()
                .chain(b"::FILE::")
                .chain(fp_bytes)
                .chain(b"::CONTEXT_ID::")
                .chain(&context_id_bytes)
                .chain(b"::PARAM::")
                .chain(parameter_marker)
                .copied()
                .collect();

            // Generate the UUIDv5 using the project's root namespace.
            let type_uuid = uuid::Uuid::new_v5(&PROJECT_NAMESPACE_UUID, &synthetic_data);

            // Return the Synthetic variant containing the generated UUID.
            Self::Synthetic(type_uuid)
        }

        // Placeholder for the Phase 3 resolved ID generation
        // pub fn generate_resolved(defining_crate_namespace: Uuid, canonical_type_path: &str) -> Self {
        //     // ... hash defining_crate_namespace + canonical_type_path ...
        //     Self::Resolved(resolved_uuid)
        // }
    }
    impl std::fmt::Display for TypeId {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            match self {
                TypeId::Resolved(uuid) => write!(f, "R:{}", short_uuid(*uuid)),
                TypeId::Synthetic(uuid) => write!(f, "S:{}", short_uuid(*uuid)),
            }
        }
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

    impl TrackingHash {
        /// Generates a TrackingHash based on crate/file context and item content.
        ///
        /// The content is hashed based on its token stream representation.
        /// WARNING: This is sensitive to formatting and minor token changes.
        /// A more robust AST-based hash might be preferable in the future.
        pub fn generate(
            crate_namespace: Uuid,
            file_path: &Path,
            item_tokens: &proc_macro2::TokenStream,
        ) -> Self {
            // Use as_encoded_bytes() for potentially non-UTF8 paths
            let fp_bytes = file_path.as_os_str().as_encoded_bytes();
            let item_string = item_tokens.to_string();

            // Combine namespace, file path bytes, and item string bytes.
            // Using separators for clarity.
            let tracking_data: Vec<u8> = crate_namespace
                .as_bytes()
                .iter()
                .chain(b"::FILE::")
                .chain(fp_bytes)
                .chain(b"::CONTENT::")
                .chain(item_string.as_bytes())
                .copied()
                .collect();

            // Generate the UUIDv5 using the project's root namespace
            // (or crate_namespace? Let's use PROJECT_NAMESPACE for consistency with other IDs)
            let hash_uuid = uuid::Uuid::new_v5(&PROJECT_NAMESPACE_UUID, &tracking_data);

            TrackingHash(hash_uuid)
        }
    }
    // Consider adding helper methods like `is_synthetic()` to NodeId if needed.
}

pub use ids::*;
