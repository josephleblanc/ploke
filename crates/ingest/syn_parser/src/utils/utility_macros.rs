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
