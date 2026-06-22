//! Typed call-site identifiers.
//!
//! Call sites are expression occurrences, not code definition nodes. Their
//! typed IDs therefore wrap [`CallId`] rather than [`NodeId`]. This keeps the
//! parser graph split into separate identity universes:
//!
//! ```text
//! V_node = item / code-node vertices, keyed by NodeId
//! V_type = structural type-use vertices, keyed by TypeId
//! V_call = call expression occurrence vertices, keyed by CallId
//! ```
//!
//! The shape mirrors the node/type ID discipline used elsewhere in
//! `syn_parser`: a base universe ID is refined into concrete typed wrappers,
//! and finite endpoint families encode admissible relation endpoints. A
//! [`MethodCallSiteId`] therefore proves only that a method-call expression
//! occurrence was structurally classified as a method-call site. It does not
//! prove the call's semantic target; target proof belongs in a later typed
//! call-resolution relation.
//!
//! Production constructors are intentionally parser-internal. Ordinary callers
//! should receive call-site IDs only through parsed [`CallNode`](crate::parser::nodes::CallNode)
//! payloads or relation facts, not by minting IDs from owner/name/span data.

use super::{AnyTypedId, CategoricalTypedId, FunctionNodeId, MethodNodeId, ToCozoUuid};
use cozo::{DataValue, UuidWrapper};
use ploke_core::{CallId, IdTrait, NodeId, PROJECT_NAMESPACE_UUID};
use serde::{Deserialize, Serialize};
use std::fmt::{self, Display};
use uuid::Uuid;

/// Structural syntax class for a parser-owned call-site occurrence.
///
/// This enum is the call-site analogue of `ItemKind`/`TypeKind` for identity
/// generation and diagnostics. Use this typed discriminant instead of string
/// tags when constructing stable call-site identity bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, PartialOrd, Ord)]
pub enum CallSiteKind {
    /// A path-style call expression, such as `foo()`, `crate::m::foo()`, or
    /// constructor-like path syntax. Structural classification does not imply
    /// that the path has been semantically resolved.
    Path,
    /// A method-call expression, such as `receiver.foo()` or
    /// `self.private_method()`.
    Method,
    /// A call expression whose callee is another expression rather than a
    /// simple path, such as `(f)()` or `make_fn()()`.
    Dynamic,
    /// A macro invocation, such as `println!(...)`. Macro calls are kept in a
    /// separate structural class because expansion may introduce additional
    /// calls that are not visible in the original source syntax.
    Macro,
}

impl CallSiteKind {
    #[inline]
    pub(in crate::parser) fn tag(self) -> &'static str {
        match self {
            Self::Path => "path",
            Self::Method => "method",
            Self::Dynamic => "dynamic",
            Self::Macro => "macro",
        }
    }
}

impl Display for CallSiteKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.tag())
    }
}

/// Shared bounds and base-ID access for typed call-site IDs.
///
/// This trait is deliberately parser-internal. It exists to let this module and
/// parser extraction code work generically over call-site wrappers while keeping
/// the base [`CallId`] inaccessible to ordinary callers. Public APIs should use
/// concrete wrappers or [`AnyCallSiteId`] endpoint families instead.
pub(in crate::parser) trait CallSiteId: AnyTypedId {
    fn base_id(self) -> CallId;

    #[inline]
    fn uuid(self) -> Uuid {
        self.base_id().uuid()
    }
}

macro_rules! define_call_site_id {
    ($(#[$outer:meta])* $Name:ident) => {
        $(#[$outer])*
        #[derive(
            Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, PartialOrd, Ord,
        )]
        pub struct $Name(CallId);

        impl $Name {
            /// Creates a typed call-site ID from a base [`CallId`].
            ///
            /// This is parser-internal so ordinary production code cannot mint
            /// typed call-site IDs without going through the extraction path
            /// that observes the corresponding `syn` expression.
            #[inline]
            pub(in crate::parser) fn create(id: CallId) -> Self {
                Self(id)
            }
        }

        impl CallSiteId for $Name {
            #[inline]
            fn base_id(self) -> CallId {
                self.0
            }
        }

        impl IdTrait for $Name {
            #[inline]
            fn uuid(&self) -> Uuid {
                self.0.uuid()
            }

            #[inline]
            fn is_resolved(&self) -> bool {
                self.0.is_resolved()
            }

            #[inline]
            fn is_synthetic(&self) -> bool {
                self.0.is_synthetic()
            }
        }

        impl AnyTypedId for $Name {}

        impl ToCozoUuid for $Name {
            #[inline]
            fn to_cozo_uuid(self) -> DataValue {
                DataValue::Uuid(UuidWrapper(self.uuid()))
            }
        }

        impl Display for $Name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}({})", stringify!($Name), self.0)
            }
        }
    };
}

define_call_site_id!(
    /// Typed ID for a path-style call-site occurrence.
    ///
    /// This wrapper proves membership in the path-call subset of `V_call`.
    /// Examples include `foo()`, `module::foo()`, `Type::new()`, and
    /// tuple-constructor-like path syntax. It does not prove that the path
    /// resolves to a local function, associated function, constructor, or any
    /// other callable target.
    PathCallSiteId
);
define_call_site_id!(
    /// Typed ID for a method-call-site occurrence.
    ///
    /// This wrapper proves membership in the method-call subset of `V_call`,
    /// such as `receiver.method()` or `self.private_method()`. It does not
    /// prove which method definition is called; that requires a separate
    /// call-resolution fact.
    MethodCallSiteId
);
define_call_site_id!(
    /// Typed ID for a dynamically shaped call-site occurrence.
    ///
    /// This wrapper is for call expressions whose callee is not a simple path,
    /// for example `(f)()` or `make_fn()()`. These sites are intentionally kept
    /// visible rather than discarded, but they should not produce direct
    /// function/method call edges without later semantic proof.
    DynamicCallSiteId
);
define_call_site_id!(
    /// Typed ID for a macro invocation occurrence.
    ///
    /// Macro invocations are represented separately from ordinary path calls
    /// because macro expansion changes which calls are actually present in the
    /// compiled program. A `MacroCallSiteId` names the invocation site, not the
    /// expanded call/effect graph behind it.
    MacroCallSiteId
);

/// Function-like item body that can own call-site expressions.
///
/// This endpoint family stays in the node universe because owners are real code
/// graph nodes. It is intentionally narrower than `AnyNodeId`: arbitrary
/// modules, structs, impls, or fields cannot own body call sites. If future
/// extraction supports const/static initializers or closure bodies, this family
/// should be extended deliberately with the corresponding owner proof.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, PartialOrd, Ord)]
pub enum CallBodyOwnerId {
    /// A standalone function body, identified by its typed node ID.
    Function(FunctionNodeId),
    /// An associated function or method body, identified by its typed node ID.
    Method(MethodNodeId),
}

impl CallBodyOwnerId {
    #[inline]
    pub fn uuid(self) -> Uuid {
        self.base_id().uuid()
    }

    #[inline]
    pub(in crate::parser) fn base_id(self) -> NodeId {
        match self {
            CallBodyOwnerId::Function(id) => id.base_id(),
            CallBodyOwnerId::Method(id) => id.base_id(),
        }
    }
}

impl Display for CallBodyOwnerId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            CallBodyOwnerId::Function(id) => write!(f, "CallBodyOwnerId::Function({})", id),
            CallBodyOwnerId::Method(id) => write!(f, "CallBodyOwnerId::Method({})", id),
        }
    }
}

impl From<FunctionNodeId> for CallBodyOwnerId {
    #[inline]
    fn from(id: FunctionNodeId) -> Self {
        CallBodyOwnerId::Function(id)
    }
}

impl From<MethodNodeId> for CallBodyOwnerId {
    #[inline]
    fn from(id: MethodNodeId) -> Self {
        CallBodyOwnerId::Method(id)
    }
}

/// Finite union of all parser-owned call-site ID classes.
///
/// This is the top endpoint family for the call-site identity universe. It is
/// analogous to `AnyTypeId` for structural type occurrences, not `AnyNodeId`:
/// widening a specific call-site ID into this enum preserves the fact that the
/// value is a call expression occurrence and not an item node.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, PartialOrd, Ord)]
pub enum AnyCallSiteId {
    /// Path-style call occurrence.
    Path(PathCallSiteId),
    /// Method-call occurrence.
    Method(MethodCallSiteId),
    /// Dynamic expression-call occurrence.
    Dynamic(DynamicCallSiteId),
    /// Macro invocation occurrence.
    Macro(MacroCallSiteId),
}

impl AnyTypedId for AnyCallSiteId {}
impl CategoricalTypedId for AnyCallSiteId {}

impl AnyCallSiteId {
    #[inline]
    pub fn uuid(self) -> Uuid {
        self.base_id().uuid()
    }

    #[inline]
    pub fn kind(self) -> CallSiteKind {
        match self {
            AnyCallSiteId::Path(_) => CallSiteKind::Path,
            AnyCallSiteId::Method(_) => CallSiteKind::Method,
            AnyCallSiteId::Dynamic(_) => CallSiteKind::Dynamic,
            AnyCallSiteId::Macro(_) => CallSiteKind::Macro,
        }
    }

    #[inline]
    pub(in crate::parser) fn base_id(self) -> CallId {
        match self {
            AnyCallSiteId::Path(id) => id.base_id(),
            AnyCallSiteId::Method(id) => id.base_id(),
            AnyCallSiteId::Dynamic(id) => id.base_id(),
            AnyCallSiteId::Macro(id) => id.base_id(),
        }
    }
}

impl Display for AnyCallSiteId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            AnyCallSiteId::Path(id) => write!(f, "AnyCallSiteId::Path({})", id),
            AnyCallSiteId::Method(id) => write!(f, "AnyCallSiteId::Method({})", id),
            AnyCallSiteId::Dynamic(id) => write!(f, "AnyCallSiteId::Dynamic({})", id),
            AnyCallSiteId::Macro(id) => write!(f, "AnyCallSiteId::Macro({})", id),
        }
    }
}

impl From<PathCallSiteId> for AnyCallSiteId {
    #[inline]
    fn from(id: PathCallSiteId) -> Self {
        AnyCallSiteId::Path(id)
    }
}

impl From<MethodCallSiteId> for AnyCallSiteId {
    #[inline]
    fn from(id: MethodCallSiteId) -> Self {
        AnyCallSiteId::Method(id)
    }
}

impl From<DynamicCallSiteId> for AnyCallSiteId {
    #[inline]
    fn from(id: DynamicCallSiteId) -> Self {
        AnyCallSiteId::Dynamic(id)
    }
}

impl From<MacroCallSiteId> for AnyCallSiteId {
    #[inline]
    fn from(id: MacroCallSiteId) -> Self {
        AnyCallSiteId::Macro(id)
    }
}

/// Generates deterministic parser-local identity for one call-site occurrence.
///
/// Inputs are intentionally expression-occurrence oriented: typed owner, typed
/// call-site kind, a syntax-class discriminator such as callee/method name,
/// source byte span, and effective cfg strings. This is not a public production
/// constructor for typed call-site wrappers; the extraction visitor should call
/// narrow helpers after observing the matching `syn` expression shape.
pub(super) fn generate_call_id(
    owner: CallBodyOwnerId,
    kind: CallSiteKind,
    discriminator: &str,
    span: (usize, usize),
    cfgs: &[String],
) -> CallId {
    let mut synthetic_data = Vec::new();
    synthetic_data.extend_from_slice(b"syn_parser.call_site.v2");
    synthetic_data.extend_from_slice(owner.base_id().uuid().as_bytes());
    synthetic_data.extend_from_slice(kind.tag().as_bytes());
    synthetic_data.push(0);
    synthetic_data.extend_from_slice(discriminator.as_bytes());
    synthetic_data.push(0);
    synthetic_data.extend_from_slice(&span.0.to_le_bytes());
    synthetic_data.extend_from_slice(&span.1.to_le_bytes());
    for cfg in cfgs {
        synthetic_data.push(0);
        synthetic_data.extend_from_slice(cfg.as_bytes());
    }
    CallId::Synthetic(Uuid::new_v5(&PROJECT_NAMESPACE_UUID, &synthetic_data))
}

/// Parser-internal constructor for path-call site IDs.
///
/// Call this only after structural extraction has classified an expression as a
/// path-style call. The path segments are joined with `::` for the identity
/// discriminator; this records the structural callee spelling and does not
/// imply semantic target resolution.
#[inline]
pub(in crate::parser) fn generate_path_call_site_id(
    owner: CallBodyOwnerId,
    path: &[String],
    span: (usize, usize),
    cfgs: &[String],
) -> PathCallSiteId {
    let discriminator = path.join("::");
    PathCallSiteId::create(generate_call_id(
        owner,
        CallSiteKind::Path,
        discriminator.as_str(),
        span,
        cfgs,
    ))
}

/// Parser-internal constructor for macro invocation site IDs.
///
/// Call this only after structural extraction has classified an expression as a
/// macro invocation. The macro name/path is a structural discriminator for the
/// invocation site, not proof of a resolved macro definition or expanded calls.
#[inline]
pub(in crate::parser) fn generate_macro_call_site_id(
    owner: CallBodyOwnerId,
    macro_name: &str,
    span: (usize, usize),
    cfgs: &[String],
) -> MacroCallSiteId {
    MacroCallSiteId::create(generate_call_id(
        owner,
        CallSiteKind::Macro,
        macro_name,
        span,
        cfgs,
    ))
}

/// Parser-internal constructor for method-call site IDs.
///
/// Call this only after structural extraction has classified an expression as a
/// method call. Tests that need deterministic regeneration should use the
/// explicit test helper in `test_ids` instead of widening this constructor's
/// visibility.
#[inline]
pub(in crate::parser) fn generate_method_call_site_id(
    owner: CallBodyOwnerId,
    method_name: &str,
    span: (usize, usize),
    cfgs: &[String],
) -> MethodCallSiteId {
    MethodCallSiteId::create(generate_call_id(
        owner,
        CallSiteKind::Method,
        method_name,
        span,
        cfgs,
    ))
}
