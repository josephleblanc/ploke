/// Macro to define a newtype wrapper around NodeId with common implementations.
///
/// Generates:
/// - A public struct `StructName(NodeId)`.
/// - Derives: Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, PartialOrd, Ord.
/// - `impl StructName`:
///   - `new(NodeId) -> Self`
///   - `into_inner(self) -> NodeId`
///   - `as_inner(&self) -> &NodeId`
/// - `impl Display for StructName` (delegates to inner NodeId).
///
/// # Usage
/// ```ignore
/// // Assuming NodeId, NodeError, Serialize, Deserialize are in scope
/// define_node_id_wrapper!(MyNodeIdWrapper);
/// ```
#[macro_export]
macro_rules! define_node_id_wrapper {
    ($(#[$outer:meta])* $NewTypeId:ident) => {
        $(#[$outer])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, PartialOrd,  Ord)]
        pub struct $NewTypeId(NodeId);

        impl $NewTypeId {
            /// Consume the wrapper and return the inner NodeId.
            /// Use sparingly, as this bypasses the type safety of the wrapper.
            #[inline]
            pub fn into_inner(self) -> NodeId {
                self.0
            }

            /// Get a reference to the inner NodeId.
            /// Use sparingly, as this bypasses the type safety of the wrapper.
            #[inline]
            pub fn as_inner(&self) -> &NodeId {
                &self.0
            }
        }

        impl std::fmt::Display for $NewTypeId {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                // Delegate to the inner NodeId's Display implementation
                write!(f, "{}", self.0)
            }
        }

        impl std::borrow::Borrow<NodeId> for $NewTypeId {
            #[inline]
            fn borrow(&self) -> &NodeId {
                &self.0
            }
        }

        impl AsRef<NodeId> for $NewTypeId {
            #[inline]
            fn as_ref(&self) -> &NodeId {
                &self.0
            }
        }
    };
}


/// Macro to define a `pub(crate)` struct containing the necessary information
/// to construct a corresponding `*Node` struct.
///
/// This macro generates an intermediate struct (e.g., `StructNodeInfo`) that
/// mirrors the fields of the target node struct (e.g., `StructNode`), with
/// the crucial difference that the `id` field is of type `NodeId` (the raw ID).
///
/// The corresponding `*Node` struct should then have a `new(info: *NodeInfo) -> Self`
/// constructor where the raw `info.id` is wrapped into the typed `*NodeId`.
///
/// # Usage Example (Illustrative)
///
/// ```ignore
/// // In the same module or accessible scope as NodeId and other field types
/// use ploke_core::NodeId;
/// // ... other necessary imports for field types ...
///
/// define_node_info_struct! {
///     /// Temporary info struct for creating a FunctionNode.
///     FunctionNodeInfo {
///         name: String,
///         span: (usize, usize),
///         visibility: VisibilityKind,
///         parameters: Vec<ParamData>,
///         return_type: Option<TypeId>,
///         generic_params: Vec<GenericParamNode>,
///         attributes: Vec<Attribute>,
///         docstring: Option<String>,
///         body: Option<String>,
///         tracking_hash: Option<TrackingHash>,
///         cfgs: Vec<String>,
///     }
/// }
///
/// // Corresponding FunctionNode would have:
/// // pub struct FunctionNode {
/// //    pub id: FunctionNodeId, // Typed ID
/// //    pub name: String,
/// //    // ... other fields matching FunctionNodeInfo ...
/// // }
/// //
/// // impl FunctionNode {
/// //    pub(crate) fn new(info: FunctionNodeInfo) -> Self { // Constructor should be pub(crate)
/// //        Self {
/// //            id: FunctionNodeId(info.id), // Wrap the ID here
/// //            name: info.name,
/// //            span: info.span,
/// //            // ... copy other fields ...
/// //        }
/// //    }
/// // }
/// ```
#[macro_export]
macro_rules! define_node_info_struct {
    (
        $(#[$outer:meta])* // Capture outer attributes like doc comments
        $InfoStructName:ident { // The name of the *NodeInfo struct to generate
            // Match field definitions (name: type)
            $($field_name:ident : $field_type:ty),* $(,)? // Allow trailing comma
        }
    ) => {
        $(#[$outer])*
        // Define the struct as pub(crate) and derive common traits.
        #[derive(Debug, Clone, PartialEq)] // Keep derives minimal for info structs
        pub(crate) struct $InfoStructName {
            /// The raw NodeId before being wrapped into a typed ID.
            pub id: $crate::parser::nodes::NodeId, // Use the raw NodeId type from nodes module

            // Define the rest of the public fields based on the macro input.
            $(pub $field_name : $field_type),*
        }
    };
}
