// TODO: The exact inputs for generating the `CanonId` and `PubPathId` needs design attention. For
// now we are defaulting to a more restrictive choice of parameters.
// - We know the current set of parameters (filepath, path, cfg, typekind) might lead to overly
// sensitive Ids in terms of requiring more frequent database updates than necessary.
// - We prefer a correct database first, then we can refine later once this is tested further.
// - Avoid conflation first, optimize for minimum inputs second.

// Define a stable PROJECT_NAMESPACE UUID.
// Moved from syn_parser::discovery
// Generated via `uuidgen`: f7f4a9a0-1b1a-4b0e-9c1a-1a1a1a1a1a1a
pub const PROJECT_NAMESPACE_UUID: uuid::Uuid = uuid::Uuid::from_bytes([
    0xf7, 0xf4, 0xa9, 0xa0, 0x1b, 0x1a, 0x4b, 0x0e, 0x9c, 0x1a, 0x1a, 0x1a, 0x1a, 0x1a, 0x1a, 0x1a,
]);

// Add top-level serde imports for derives
use serde::{Deserialize, Serialize};

// Helper Hasher to collect bytes for UUID generation
pub mod byte_hasher {
    use std::hash::Hasher;

    #[derive(Default)]
    pub struct ByteHasher {
        bytes: Vec<u8>,
    }

    impl ByteHasher {
        pub fn finish_bytes(self) -> Vec<u8> {
            self.bytes
        }
    }

    impl Hasher for ByteHasher {
        fn finish(&self) -> u64 {
            // Not used, we collect bytes directly
            unimplemented!("ByteHasher does not produce a u64 hash")
        }

        fn write(&mut self, bytes: &[u8]) {
            self.bytes.extend_from_slice(bytes);
        }

        // Override other write_* methods for potential efficiency gains
        // if the derived Hash implementations use them directly.
        fn write_u8(&mut self, i: u8) {
            self.bytes.push(i);
        }
        fn write_u16(&mut self, i: u16) {
            self.bytes.extend_from_slice(&i.to_le_bytes());
        }
        fn write_u32(&mut self, i: u32) {
            self.bytes.extend_from_slice(&i.to_le_bytes());
        }
        fn write_u64(&mut self, i: u64) {
            self.bytes.extend_from_slice(&i.to_le_bytes());
        }
        fn write_usize(&mut self, i: usize) {
            self.bytes.extend_from_slice(&i.to_le_bytes());
        }
        fn write_i8(&mut self, i: i8) {
            self.bytes.push(i as u8);
        }
        fn write_i16(&mut self, i: i16) {
            self.bytes.extend_from_slice(&i.to_le_bytes());
        }
        fn write_i32(&mut self, i: i32) {
            self.bytes.extend_from_slice(&i.to_le_bytes());
        }
        fn write_i64(&mut self, i: i64) {
            self.bytes.extend_from_slice(&i.to_le_bytes());
        }
        fn write_isize(&mut self, i: isize) {
            self.bytes.extend_from_slice(&i.to_le_bytes());
        }
    }
}

mod ids {
    use crate::byte_hasher::ByteHasher; // Import the custom hasher
    use std::hash::{Hash, Hasher}; // Import Hash traits
    use std::path::Path;
    // Import TypeKind into the ids module scope
    use crate::{IdConversionError, TypeKind}; // Add IdConversionError

    use serde::{Deserialize, Serialize};
    use uuid::Uuid;

    use crate::{ItemKind, PROJECT_NAMESPACE_UUID}; // Import ItemKind
                                                   // Removed unused std::io import

    pub struct ResolvedIds {
        canon: CanonId,
        short_pub: Option<PubPathId>,
        tracking_hash: TrackingHash,
    }
    pub trait IdTrait {
        fn uuid(&self) -> Uuid;
        fn is_resolved(&self) -> bool;
        fn is_synthetic(&self) -> bool;
    }

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
        #[deprecated(
            since = "0.1.0",
            note = "Use CanonId::generate_resolved or PubPathId::generate_resolved instead"
        )]
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
            cfg_bytes: Option<&[u8]>,
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
                .chain(cfg_bytes.unwrap_or(&[0u8; 0]))
                .copied()
                .collect();

            Self::Synthetic(uuid::Uuid::new_v5(&PROJECT_NAMESPACE_UUID, &synthetic_data))
        }
    }
    impl IdTrait for NodeId {
        /// Returns the inner Uuid regardless of the variant (Resolved or Synthetic).
        fn uuid(&self) -> Uuid {
            match self {
                NodeId::Resolved(uuid) => *uuid,
                NodeId::Synthetic(uuid) => *uuid,
            }
        }

        /// Check if this is a Resolved variant
        fn is_resolved(&self) -> bool {
            matches!(self, NodeId::Resolved(_))
        }

        /// Check if this is a Synthetic variant
        fn is_synthetic(&self) -> bool {
            matches!(self, NodeId::Synthetic(_))
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

    /// Unique identifier for a specific type structure *within a specific crate version*.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, PartialOrd, Ord)]
    pub enum TypeId {
        Resolved(Uuid),
        Synthetic(Uuid),
    }
    impl TypeId {
        /// Generates a temporary `Synthetic` `TypeId` based on structural information.
        ///
        /// This ID is used during Phase 2 parsing before full type resolution. It aims
        /// to be deterministic and stable against formatting changes by hashing the
        /// structural components of the type (`TypeKind`, related `TypeId`s) rather
        /// than its string representation.
        ///
        /// # Arguments
        /// * `crate_namespace` - The UUID namespace of the crate where the type usage occurs.
        /// * `file_path` - The absolute path to the file where the type usage occurs.
        /// * `type_kind` - The structural kind of the type (e.g., `TypeKind::Named`, `TypeKind::Reference`).
        /// * `related_type_ids` - A slice of `TypeId`s for nested types (e.g., generic arguments, tuple elements).
        ///
        /// # Returns
        /// A `TypeId::Synthetic` variant containing a UUIDv5 hash derived from the inputs.
        ///
        /// # Hashing Strategy
        /// The hash incorporates:
        /// - Crate namespace UUID bytes.
        /// - File path bytes.
        /// - The discriminant of the `TypeKind` enum variant.
        /// - Bytes representing the specific data within the `TypeKind` variant (e.g., path segments, mutability flags).
        /// - Bytes of all `related_type_ids` UUIDs in order.
        ///
        /// **Note on `Self` and Generic Parameters:** This function currently generates generic IDs
        /// for usages of `Self` and generic parameters based on their simple names (e.g.,
        /// `TypeKind::Named { path: ["Self"], .. }` or `TypeKind::Named { path: ["T"], .. }`).
        /// This means `Self` used in `impl A` might temporarily get the same `Synthetic` `TypeId`
        /// as `Self` used in `impl B` during Phase 2. Full contextual disambiguation to distinguish
        /// these cases is deferred until Phase 3 (name resolution) or Step 3 (`Enhance VisitorState Context`).
        pub fn generate_synthetic(
            crate_namespace: Uuid,
            file_path: &Path,
            type_kind: &TypeKind, // Use TypeKind from this crate
            related_type_ids: &[TypeId],
            parent_scope_id: Option<NodeId>, // NEW: Add parent scope context
        ) -> Self {
            // Create our custom hasher
            let mut hasher = ByteHasher::default();

            // Hash the context components
            // Note: Hashing Path/PathBuf directly includes OS-specific separators.
            // Hashing the string representation might be more portable if needed,
            // but for now, direct hashing is simpler.
            crate_namespace.hash(&mut hasher);
            file_path.hash(&mut hasher);

            // Hash the structural components using their derived Hash impls
            type_kind.hash(&mut hasher);
            related_type_ids.hash(&mut hasher);

            // // Conditionally hash the parent scope ID using the uuid() method
            if let Some(parent_id) = parent_scope_id {
                hasher.write(parent_id.uuid().as_bytes()); // Use the uuid() method
            }
            // else: hash nothing extra for the None case

            // Retrieve the collected bytes
            let collected_bytes = hasher.finish_bytes();

            // Generate UUIDv5 using the collected bytes
            let type_uuid = uuid::Uuid::new_v5(&PROJECT_NAMESPACE_UUID, &collected_bytes);
            Self::Synthetic(type_uuid)
        }

        // Removed generate_contextual_synthetic function.
        // Contextual disambiguation for Self/Generics is deferred to Step 3.

        // Placeholder for the Phase 3 resolved ID generation
        // pub fn generate_resolved(defining_crate_namespace: Uuid, canonical_type_path: &str) -> Self {
        //     // ... hash defining_crate_namespace + canonical_type_path ...
        //     Self::Resolved(resolved_uuid)
        // }
    }
    impl IdTrait for TypeId {
        /// Returns the inner Uuid regardless of the variant (Resolved or Synthetic).
        fn uuid(&self) -> Uuid {
            match self {
                TypeId::Resolved(uuid) => *uuid,
                TypeId::Synthetic(uuid) => *uuid,
            }
        }

        /// Check if this is a Resolved variant
        fn is_resolved(&self) -> bool {
            matches!(self, TypeId::Resolved(_))
        }

        /// Check if this is a Synthetic variant
        fn is_synthetic(&self) -> bool {
            matches!(self, TypeId::Synthetic(_))
        }
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

    /// Trait for generating resolved path-based IDs (CanonId, PubPathId).
    pub trait ResolvedId: Sized + IdTrait + Copy {
        /// Generates a resolved ID based on crate, path, and item details.
        ///
        /// # Arguments
        /// * `crate_namespace` - UUID namespace of the defining crate.
        /// * `file_path`
        ///     - Absolute path to the source file containing the definition.
        ///     - For `CanonId`, not really needed. Need to revisit this point.
        ///     - Required to distinguish between items with #[path]
        /// * `logical_item_path`
        ///     - The specific path used for this ID type
        ///     - (e.g., canonical path for `CanonId`, shortest public path for `PubPathId`).
        ///     - e.g. for canonical: `crate::module_a::Item`
        ///     - e.g. for public path: `my_project::module_a::Item`
        ///     - Note: if public path had in main.rs, `pub use my_project::module_a::Item`, the
        ///     shortest public path would be `my_project::Item`, but canonical path would still be
        ///     `crate::module_a::Item`
        /// * `item_kind` - The kind of item, used to determine Node vs. Type variant.
        ///     - Note: Might remove this. Needs design attention
        /// * `cfg`: Required to distinguish identical nodes that have mutually exclusive cfgs
        ///
        /// # Returns
        /// `Ok(Self)` with the generated ID, or `Err(IdConversionError)` if generation fails
        /// (e.g., I/O error during canonicalization).
        fn generate_resolved(
            crate_namespace: Uuid,
            id_info: IdInfo,
        ) -> Result<Self, IdConversionError>;
    }

    pub struct IdInfo<'a> {
        file_path: &'a Path,
        logical_item_path: &'a [String],
        cfgs: &'a [String], // placeholder &str type, needs design attention
        item_kind: ItemKind,
    }

    impl<'a> IdInfo<'a> {
        pub fn new(
            file_path: &'a Path,
            logical_item_path: &'a [String],
            cfgs: &'a [String],
            item_kind: ItemKind,
        ) -> Self {
            Self {
                file_path,
                logical_item_path,
                cfgs,
                item_kind,
            }
        }

        pub fn file_path(&self) -> &Path {
            self.file_path
        }

        pub fn logical_item_path(&self) -> &[String] {
            self.logical_item_path
        }

        pub fn cfgs(&self) -> &[String] {
            self.cfgs
        }

        pub fn item_kind(&self) -> ItemKind {
            self.item_kind
        }
    }

    /// Unique identifier derived from the canonical path within the defining crate,
    /// distinguishing between Node and Type items.
    /// Generated *after* name resolution (Phase 3).
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, PartialOrd, Ord)]
    pub enum CanonId {
        Node(Uuid),
        Type(Uuid),
    }

    impl IdTrait for CanonId {
        /// Returns the inner Uuid regardless of the variant.
        fn uuid(&self) -> Uuid {
            match self {
                CanonId::Node(uuid) => *uuid,
                CanonId::Type(uuid) => *uuid,
            }
        }

        fn is_resolved(&self) -> bool {
            // hardcoded, should always be resolved
            true
        }

        fn is_synthetic(&self) -> bool {
            // hardcoded, should never be synthetic
            false
        }
    }

    impl std::fmt::Display for CanonId {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            match self {
                CanonId::Node(uuid) => write!(f, "P:C:N:{}", short_uuid(*uuid)),
                CanonId::Type(uuid) => write!(f, "P:C:T:{}", short_uuid(*uuid)),
            }
        }
    }

    impl ResolvedId for CanonId {
        fn generate_resolved(
            crate_namespace: Uuid,
            id_info: IdInfo<'_>,
            // file_path: &Path,
            // logical_item_path: &[String], // Canonical path for CanonId
            // item_name: &str,
            // item_kind: ItemKind,
            // replaced fields with `IdInfo`
        ) -> Result<Self, IdConversionError> {
            // Canonicalize the file path

            let canonical_file_path = id_info.file_path().canonicalize().map_err(|e| {
                IdConversionError::IoError(id_info.file_path().display().to_string(), e.to_string())
            })?;
            let fp_bytes: &[u8] = canonical_file_path.as_os_str().as_encoded_bytes();

            // Combine components for hashing
            let resolved_data: Vec<u8> = crate_namespace
                .as_bytes()
                .iter()
                .chain(b"::CANON_FILE::")
                .chain(fp_bytes)
                .chain(b"::CANON_PATH::")
                .chain(id_info.logical_item_path().join("::").as_bytes())
                .chain(b"::CANNON_CFG::")
                .chain(id_info.cfgs().join("::").as_bytes())
                .copied()
                .collect();

            let resolved_uuid = uuid::Uuid::new_v5(&PROJECT_NAMESPACE_UUID, &resolved_data);

            // Determine Node or Type variant based on ItemKind
            // TODO: Refine ItemKind mapping if necessary (e.g., handle fields/variants differently?)
            match id_info.item_kind() {
                ItemKind::Function
                | ItemKind::Struct
                | ItemKind::Enum
                | ItemKind::Union
                | ItemKind::Trait
                | ItemKind::Impl
                | ItemKind::Module
                | ItemKind::Field // Treat fields as nodes for now
                | ItemKind::Variant // Treat variants as nodes for now
                | ItemKind::GenericParam // Treat generic params as nodes for now
                | ItemKind::Const
                | ItemKind::Static
                | ItemKind::Macro
                | ItemKind::Import
                | ItemKind::ExternCrate => Ok(CanonId::Node(resolved_uuid)),
                ItemKind::TypeAlias => Ok(CanonId::Type(resolved_uuid)), // Type aliases represent types
            }
        }
    }

    /// Unique identifier derived from the shortest public path (considering re-exports),
    /// distinguishing between Node and Type items.
    /// Generated *after* name resolution (Phase 3).
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, PartialOrd, Ord)]
    pub enum PubPathId {
        Node(Uuid),
        Type(Uuid),
    }

    impl IdTrait for PubPathId {
        /// Returns the inner Uuid regardless of the variant.
        fn uuid(&self) -> Uuid {
            match self {
                PubPathId::Node(uuid) => *uuid,
                PubPathId::Type(uuid) => *uuid,
            }
        }

        fn is_resolved(&self) -> bool {
            // hardcoded, should always be resolved
            true
        }

        fn is_synthetic(&self) -> bool {
            // hardcoded, should never be synthetic
            false
        }
    }

    impl std::fmt::Display for PubPathId {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            match self {
                PubPathId::Node(uuid) => write!(f, "P:S:N:{}", short_uuid(*uuid)),
                PubPathId::Type(uuid) => write!(f, "P:S:T:{}", short_uuid(*uuid)),
            }
        }
    }

    impl ResolvedId for PubPathId {
        fn generate_resolved(
            crate_namespace: Uuid,
            id_info: IdInfo<'_>,
            // file_path: &Path,             // Use original file path for SPP
            // logical_item_path: &[String], // Shortest public path for PubPathId
            // item_name: &str,
            // item_kind: ItemKind,
            // replaced fields with `IdInfo`
        ) -> Result<Self, IdConversionError> {
            // Use the provided file_path directly, no canonicalization
            let fp_bytes: &[u8] = id_info.file_path().as_os_str().as_encoded_bytes();

            // Combine components for hashing
            let resolved_data: Vec<u8> = crate_namespace
                .as_bytes()
                .iter()
                .chain(b"::ORIG_FILE::") // Indicate original file path used
                .chain(fp_bytes)
                .chain(b"::SPP_PATH::") // Indicate shortest public path used
                .chain(id_info.logical_item_path().join("::").as_bytes())
                .chain(id_info.cfgs().join("::").as_bytes())
                .copied()
                .collect();

            let resolved_uuid = uuid::Uuid::new_v5(&PROJECT_NAMESPACE_UUID, &resolved_data);

            // Determine Node or Type variant based on ItemKind
            match id_info.item_kind() {
                ItemKind::Function
                | ItemKind::Struct
                | ItemKind::Enum
                | ItemKind::Union
                | ItemKind::Trait
                | ItemKind::Impl
                | ItemKind::Module
                | ItemKind::Field
                | ItemKind::Variant
                | ItemKind::GenericParam
                | ItemKind::Const
                | ItemKind::Static
                | ItemKind::Macro
                | ItemKind::Import
                | ItemKind::ExternCrate => Ok(PubPathId::Node(resolved_uuid)),
                ItemKind::TypeAlias => Ok(PubPathId::Type(resolved_uuid)),
            }
        }
    }

    // --- TryFrom Implementations for CanonId ---

    impl TryFrom<NodeId> for CanonId {
        type Error = IdConversionError;

        fn try_from(node_id: NodeId) -> Result<Self, Self::Error> {
            match node_id {
                NodeId::Resolved(uuid) => Ok(CanonId::Node(uuid)), // Create Node variant
                NodeId::Synthetic(_) => Err(IdConversionError::SyntheticNode(node_id)),
            }
        }
    }

    impl TryFrom<TypeId> for CanonId {
        type Error = IdConversionError;

        fn try_from(type_id: TypeId) -> Result<Self, Self::Error> {
            match type_id {
                TypeId::Resolved(uuid) => Ok(CanonId::Type(uuid)), // Create Type variant
                TypeId::Synthetic(_) => Err(IdConversionError::SyntheticType(type_id)),
            }
        }
    }

    // --- TryFrom Implementations for PubPathId ---

    impl TryFrom<NodeId> for PubPathId {
        type Error = IdConversionError;

        fn try_from(node_id: NodeId) -> Result<Self, Self::Error> {
            match node_id {
                NodeId::Resolved(uuid) => Ok(PubPathId::Node(uuid)), // Create Node variant
                NodeId::Synthetic(_) => Err(IdConversionError::SyntheticNode(node_id)),
            }
        }
    }

    impl TryFrom<TypeId> for PubPathId {
        type Error = IdConversionError;

        fn try_from(type_id: TypeId) -> Result<Self, Self::Error> {
            match type_id {
                TypeId::Resolved(uuid) => Ok(PubPathId::Type(uuid)), // Create Type variant
                TypeId::Synthetic(_) => Err(IdConversionError::SyntheticType(type_id)),
            }
        }
    }
}

pub use ids::*;

/// Error type for ID conversions.
#[derive(thiserror::Error, Debug, Clone, PartialEq, Eq)] // Removed Copy
pub enum IdConversionError {
    #[error("Cannot convert Synthetic NodeId {0} to a path-based ID.")]
    SyntheticNode(NodeId),
    #[error("Cannot convert Synthetic TypeId {0} to a path-based ID.")]
    SyntheticType(TypeId),
    #[error("I/O error during ID generation for path {0}: {1}")]
    IoError(String, String), // Path and error message
}

/// Represents the specific kind of a code item associated with a `NodeId`.
/// Moved from `syn_parser::parser::nodes`.
///
/// This enum is used as part of the input for generating `NodeId::Synthetic`
/// to help disambiguate items that might otherwise have similar names or paths,
/// especially when `span` is removed as an input. It ensures that, for example,
/// a function named `foo` and a struct named `foo` in the same module scope
/// will generate distinct `NodeId`s.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum ItemKind {
    Function,
    Struct,
    Enum,
    Union,
    TypeAlias,
    Trait,
    Impl,
    Module,
    Field,        // Struct or Union field
    Variant,      // Enum variant
    GenericParam, // Type, Lifetime, or Const generic parameter definition
    Const,
    Static,
    Macro,       // Includes declarative (macro_rules!) and procedural macros
    Import, // Represents a specific item within a `use` statement (e.g., `HashMap` in `use std::collections::HashMap`)
    ExternCrate, // Represents an `extern crate` declaration
}

// ANCHOR: TypeKind_defn
/// Different kinds of types encountered during parsing.
/// Moved from `syn_parser::parser::types`.
/// Used as input for structural `TypeId::Synthetic` generation.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, Hash)] // Added Eq, Hash
pub enum TypeKind {
    // Corrected: Removed duplicate 'pub'
    //ANCHOR_END: TypeKind_defn
    Named {
        path: Vec<String>, // Full path segments
        is_fully_qualified: bool,
    },
    Reference {
        lifetime: Option<String>, // Lifetimes are strings for now
        is_mutable: bool,
        // Type being referenced is in related_types[0]
    },
    Slice {
        // Element type is in related_types[0]
    },
    Array {
        // Element type is in related_types[0]
        size: Option<String>, // Size expression as string
    },
    Tuple {
        // Element types are in related_types
    },
    // ANCHOR: ExternCrate
    Function {
        // Parameter types are in related_types (except last one)
        // Return type is in related_types[last]
        is_unsafe: bool,
        is_extern: bool,
        abi: Option<String>, // ABI as string
    },
    //ANCHOR_END: ExternCrate
    Never,
    Inferred,
    RawPointer {
        is_mutable: bool,
        // Pointee type is in related_types[0]
    },
    // ANCHOR: TraitObject
    TraitObject {
        // Trait bounds are in related_types
        dyn_token: bool,
    },
    //ANCHOR_END: TraitObject
    // ANCHOR: ImplTrait
    ImplTrait {
        // Trait bounds are in related_types
    },
    //ANCHOR_END: ImplTrait
    Paren {
        // Inner type is in related_types[0]
    },
    // ANCHOR: ItemMacro
    Macro {
        name: String,
        tokens: String, // Macro tokens as string
    },
    //ANCHOR_END: ItemMacro
    Unknown {
        type_str: String, // Fallback string representation
    },
}
