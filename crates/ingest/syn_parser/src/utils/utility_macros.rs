pub(crate) use ploke_core::NodeId;
pub(crate) use serde::{Deserialize, Serialize};

pub(crate) use crate::parser::nodes::{GraphId, NodeError};

/// Macro to define a newtype wrapper around NodeId with common implementations.
///
/// Generates:
/// - A public struct `StructName(NodeId)`.
/// - Derives: Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, PartialOrd, Ord.
/// - `impl StructName`:
///   - `new(NodeId) -> Self`
///   - `into_inner(self) -> NodeId`
///   - `as_inner(&self) -> &NodeId`
///   - `to_graph_id(self) -> GraphId`
/// - `impl Display for StructName` (delegates to inner NodeId).
/// - `impl TryFrom<GraphId> for StructName` (errors on GraphId::Type).
///
/// # Usage
/// ```ignore
/// // Assuming NodeId, GraphId, NodeError, Serialize, Deserialize are in scope
/// define_node_id_wrapper!(MyNodeIdWrapper);
/// ```
#[macro_export]
macro_rules! define_node_id_wrapper {
    ($(#[$outer:meta])* $NewTypeId:ident) => {
        $(#[$outer])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, PartialOrd,  Ord)]
        pub struct $NewTypeId(NodeId);

        impl $NewTypeId {
            /// Create from raw NodeId.
            #[inline]
            pub fn new(id: NodeId) -> Self {
                Self(id)
            }

            /// Consume the wrapper and return the inner NodeId.
            #[inline]
            pub fn into_inner(self) -> NodeId {
                self.0
            }

            /// Get a reference to the inner NodeId.
            #[inline]
            pub fn as_inner(&self) -> &NodeId {
                &self.0
            }

            /// Converts this ID into a GraphId::Node.
            #[inline]
            pub fn to_graph_id(self) -> GraphId {
                GraphId::Node(self.0)
            }
        }

        impl std::fmt::Display for $NewTypeId {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                // Delegate to the inner NodeId's Display implementation
                write!(f, "{}", self.0)
            }
        }

        impl TryFrom<GraphId> for $NewTypeId {
            type Error = NodeError; // Assumes NodeError is in scope

            fn try_from(value: GraphId) -> Result<Self, Self::Error> {
                match value {
                    GraphId::Node(id) => Ok($NewTypeId::new(id)),
                    // Assumes NodeError has a variant suitable for this conversion error
                    GraphId::Type(type_id) => Err(NodeError::Conversion(type_id)),
                }
            }
        }
    };
}
