//! Parser-owned call-site nodes.
//!
//! These nodes record structural call expressions found inside function-like
//! bodies. They are deliberately occurrence-level facts: a [`MethodCallNode`]
//! says that source syntax contained a method call at a span, not which
//! [`MethodNode`](crate::parser::nodes::MethodNode) is ultimately invoked.
//! Semantic target proof belongs in later call-resolution relations.

use serde::{Deserialize, Serialize};

use super::*;

/// Erased structural call-site node.
///
/// Each variant contains a typed call-site ID whose wrapper proves the parser's
/// structural classification of the expression occurrence. Use this enum for
/// graph storage and iteration over all call-site classes; use the concrete
/// payload types when constructing class-specific facts.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub enum CallNode {
    /// Path-style call syntax, such as `foo()` or `crate::m::foo()`.
    PathCall(PathCallNode),
    /// Method-call syntax, such as `receiver.foo()`.
    MethodCall(MethodCallNode),
    /// Expression-call syntax whose callee is not a simple path.
    DynamicCall(DynamicCallNode),
    /// Macro invocation syntax, such as `println!(...)`.
    MacroCall(MacroCallNode),
}

impl CallNode {
    /// Returns this call-site's ID widened to the call-site universe umbrella.
    pub fn id(&self) -> AnyCallSiteId {
        match self {
            CallNode::PathCall(call) => call.id.into(),
            CallNode::MethodCall(call) => call.id.into(),
            CallNode::DynamicCall(call) => call.id.into(),
            CallNode::MacroCall(call) => call.id.into(),
        }
    }

    /// Returns the function-like body that owns this call-site occurrence.
    pub fn owner(&self) -> CallBodyOwnerId {
        match self {
            CallNode::PathCall(call) => call.owner,
            CallNode::MethodCall(call) => call.owner,
            CallNode::DynamicCall(call) => call.owner,
            CallNode::MacroCall(call) => call.owner,
        }
    }

    /// Returns the byte span for the whole call expression in the source file.
    pub fn span(&self) -> (usize, usize) {
        match self {
            CallNode::PathCall(call) => call.span,
            CallNode::MethodCall(call) => call.span,
            CallNode::DynamicCall(call) => call.span,
            CallNode::MacroCall(call) => call.span,
        }
    }

    /// Returns effective cfg strings attached to this call occurrence.
    pub fn cfgs(&self) -> &[String] {
        match self {
            CallNode::PathCall(call) => &call.cfgs,
            CallNode::MethodCall(call) => &call.cfgs,
            CallNode::DynamicCall(call) => &call.cfgs,
            CallNode::MacroCall(call) => &call.cfgs,
        }
    }
}

/// Structural record for a path-style call-site occurrence.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct PathCallNode {
    /// Typed call-site ID in the path-call subset of `V_call`.
    pub id: PathCallSiteId,
    /// Function-like body containing this expression.
    pub owner: CallBodyOwnerId,
    /// Byte span for the whole call expression.
    pub span: (usize, usize),
    /// Effective cfg strings for this occurrence.
    pub cfgs: Vec<String>,
    /// Parsed callee path segments as written/resolved structurally by the
    /// parser pass. This is not a semantic target proof.
    pub path: Vec<String>,
    /// Conservative classification of the path callee's value namespace.
    #[serde(default)]
    pub callee: PathCallCallee,
    /// Number of value arguments at the call site.
    pub arg_count: usize,
    /// Number of explicit generic arguments on the callee, if represented by
    /// the syntax class.
    pub generic_arg_count: usize,
}

/// Coarse callee categories for syntactic path-call sites.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, Default)]
pub enum PathCallCallee {
    /// The callee is not shadowed by a visible local value binding.
    #[default]
    ItemPath,
    /// The callee path names a visible local value binding or parameter.
    ValueBinding { path: Vec<String> },
    /// The callee path names a visible local binding initialized from a path.
    InitializedValueBinding {
        path: Vec<String>,
        init_path: Vec<String>,
    },
}

/// Structural record for a method-call-site occurrence.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct MethodCallNode {
    /// Typed call-site ID in the method-call subset of `V_call`.
    pub id: MethodCallSiteId,
    /// Function-like body containing this expression.
    pub owner: CallBodyOwnerId,
    /// Byte span for the whole method-call expression.
    pub span: (usize, usize),
    /// Effective cfg strings for this occurrence.
    pub cfgs: Vec<String>,
    /// Method name token from the call syntax.
    pub method_name: String,
    /// Coarse structural receiver classification.
    pub receiver: MethodCallReceiver,
    /// Number of value arguments after the receiver.
    pub arg_count: usize,
    /// Number of explicit generic arguments on the method call.
    pub generic_arg_count: usize,
}

/// Structural record for an expression-call occurrence whose callee is dynamic
/// or otherwise not represented as a path terminal.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct DynamicCallNode {
    /// Typed call-site ID in the dynamic-call subset of `V_call`.
    pub id: DynamicCallSiteId,
    /// Function-like body containing this expression.
    pub owner: CallBodyOwnerId,
    /// Byte span for the whole call expression.
    pub span: (usize, usize),
    /// Effective cfg strings for this occurrence.
    pub cfgs: Vec<String>,
    /// Number of value arguments at the call site.
    pub arg_count: usize,
    /// Conservative classification of the callee expression.
    #[serde(default)]
    pub callee: DynamicCallCallee,
}

/// Coarse callee categories for dynamic expression-call sites.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, Default)]
pub enum DynamicCallCallee {
    /// The callee expression is a parenthesized path that is not shadowed by a
    /// visible local binding at the call site.
    Path { path: Vec<String> },
    /// The callee expression is a path cast to a bare function pointer before
    /// being called, such as `(local_target as fn() -> i32)()`.
    FnPointerCastPath { path: Vec<String> },
    /// The callee expression is a visible local binding initialized from a path
    /// and then cast to a bare function pointer before being called, such as
    /// `let f = local_target; (f as fn() -> i32)()`.
    FnPointerCastInitializedLocalBinding {
        path: Vec<String>,
        init_path: Vec<String>,
    },
    /// The callee expression is a dereferenced visible local binding
    /// initialized from a path, such as `let f = local_target; (*f)()`.
    DereferencedInitializedLocalBinding {
        path: Vec<String>,
        init_path: Vec<String>,
    },
    /// The callee expression is a visible local binding or parameter, such as
    /// `(closure)()` or `(f)()`.
    LocalBinding { path: Vec<String> },
    /// The callee expression is a visible local binding initialized from a
    /// path, such as `let f = local_target; (f)()`.
    InitializedLocalBinding {
        path: Vec<String>,
        init_path: Vec<String>,
    },
    /// The callee expression is a field projection rooted at a local binding,
    /// such as `value.0()`.
    FieldLocalBinding { path: Vec<String> },
    /// The callee expression is a field projection rooted at a constructed
    /// local binding whose selected constructor argument is a path expression,
    /// such as `let value = Tuple(local_target); value.0()`.
    FieldInitializedLocalBinding {
        path: Vec<String>,
        init_path: Vec<String>,
    },
    /// The callee expression indexes a visible local binding initialized from
    /// path-valued array elements, such as
    /// `let funcs = [local_target]; funcs[0]()`.
    IndexedInitializedLocalBinding {
        path: Vec<String>,
        init_path: Vec<String>,
    },
    /// The callee expression is an if expression whose supported branches each
    /// evaluate to a path expression, such as
    /// `(if flag { local_target } else { other_target })()`.
    IfBranchPaths { paths: Vec<Vec<String>> },
    /// The callee expression is a match expression whose supported arms each
    /// evaluate to a path expression, such as
    /// `(match flag { true => local_target, false => other_target })()`.
    MatchArmPaths { paths: Vec<Vec<String>> },
    /// The callee expression is not represented by this conservative slice.
    #[default]
    Other,
}

/// Structural record for a macro invocation occurrence.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct MacroCallNode {
    /// Typed call-site ID in the macro-call subset of `V_call`.
    pub id: MacroCallSiteId,
    /// Function-like body containing this invocation.
    pub owner: CallBodyOwnerId,
    /// Byte span for the whole macro invocation.
    pub span: (usize, usize),
    /// Effective cfg strings for this occurrence.
    pub cfgs: Vec<String>,
    /// Macro path/name as structurally observed at the invocation site.
    pub macro_name: String,
}

/// Coarse receiver categories for method-call sites.
///
/// This is intentionally a small structural classifier, not full Rust method
/// resolution. It should grow only as resolver tests require more conservative
/// receiver proof.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub enum MethodCallReceiver {
    /// The receiver expression is the literal `self` value.
    SelfValue,
    /// The receiver is a field projection rooted at `self`, such as
    /// `self.secret` or `self.inner.value`.
    SelfField {
        /// Field/member projection path after `self`.
        field_path: Vec<String>,
    },
    /// The receiver is a simple local binding name admitted by the current
    /// body owner, such as a named function parameter.
    LocalBinding {
        /// Binding identifier used as the receiver expression.
        name: String,
    },
    /// The receiver is a simple local binding with an explicit local type
    /// annotation visible at the call site.
    TypedLocalBinding {
        /// Binding identifier used as the receiver expression.
        name: String,
        /// Structural type path from the local binding annotation.
        type_path: Vec<String>,
    },
    /// The receiver is a simple local binding whose initializer is a path
    /// expression visible at the call site, such as `let value = LocalAssoc;`.
    InitializedLocalBinding {
        /// Binding identifier used as the receiver expression.
        name: String,
        /// Structural initializer path carried to the resolver for exact local
        /// type proof.
        init_path: Vec<String>,
    },
    /// The receiver is a borrowed local binding, such as `&value`.
    BorrowedLocalBinding {
        /// Binding identifier inside the borrow expression.
        name: String,
    },
    /// The receiver is a borrowed local binding with an explicit local type
    /// annotation visible at the call site.
    BorrowedTypedLocalBinding {
        /// Binding identifier inside the borrow expression.
        name: String,
        /// Structural type path from the local binding annotation.
        type_path: Vec<String>,
    },
    /// The receiver is a dereferenced local binding, such as `*value`.
    DereferencedLocalBinding {
        /// Binding identifier inside the dereference expression.
        name: String,
    },
    /// The receiver is a dereferenced local binding whose initializer is a
    /// direct reference to a path expression, such as `let value = &LocalType;`.
    DereferencedInitializedLocalBinding {
        /// Binding identifier inside the dereference expression.
        name: String,
        /// Structural initializer path carried to the resolver for exact local
        /// type proof after dereferencing.
        init_path: Vec<String>,
    },
    /// The receiver is a field projection rooted at a local binding, such as
    /// `value.0`.
    FieldLocalBinding {
        /// Binding identifier at the root of the field projection.
        name: String,
        /// Field/member projection path after the binding.
        field_path: Vec<String>,
    },
    /// The receiver is a field projection rooted at a local binding with an
    /// explicit local type annotation visible at the call site.
    FieldTypedLocalBinding {
        /// Binding identifier at the root of the field projection.
        name: String,
        /// Structural type path from the local binding annotation.
        type_path: Vec<String>,
        /// Field/member projection path after the binding.
        field_path: Vec<String>,
    },
    /// The receiver is a field projection rooted at a local binding whose
    /// initializer is a path expression visible at the call site.
    FieldInitializedLocalBinding {
        /// Binding identifier at the root of the field projection.
        name: String,
        /// Structural initializer path carried to the resolver for exact local
        /// type proof.
        init_path: Vec<String>,
        /// Field/member projection path after the binding.
        field_path: Vec<String>,
    },
    /// The receiver is the result of a path call, such as `make_value()`.
    PathCallResult {
        /// Path used as the receiver call's callee.
        path: Vec<String>,
    },
    /// The receiver is the result of a method call, such as `value.make()`.
    MethodCallResult {
        /// Method name used by the receiver call.
        method_name: String,
    },
    /// The receiver is the result of an await expression, such as
    /// `future.await`.
    AwaitResult,
    /// The receiver is the awaited result of a path call, such as
    /// `make_value().await`.
    AwaitPathCallResult {
        /// Path used as the awaited receiver call's callee.
        path: Vec<String>,
    },
    /// The receiver is the result of a try expression, such as `value?`.
    TryResult,
    /// The receiver is the try result of a path call, such as
    /// `make_value()?`.
    TryPathCallResult {
        /// Path used as the tried receiver call's callee.
        path: Vec<String>,
    },
    /// The receiver is a literal expression, such as `"x"`.
    Literal,
}
