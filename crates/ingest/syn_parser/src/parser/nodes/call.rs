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

    /// Returns true when this call occurrence is lexically inside an unsafe block.
    pub fn unsafe_block(&self) -> bool {
        match self {
            CallNode::PathCall(call) => call.unsafe_block,
            CallNode::MethodCall(call) => call.unsafe_block,
            CallNode::DynamicCall(call) => call.unsafe_block,
            CallNode::MacroCall(call) => call.unsafe_block,
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
    /// Whether this call occurrence is lexically inside an unsafe block.
    #[serde(default)]
    pub unsafe_block: bool,
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
    /// Conservative argument summaries used only for exact local value-flow
    /// proof. Missing or unsupported argument shapes are represented as
    /// `Other`; arity remains authoritative in `arg_count`.
    #[serde(default)]
    pub arguments: Vec<CallArgument>,
}

/// Coarse argument categories for path-call sites.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, Default)]
pub enum CallArgument {
    /// The argument expression is a path such as `local_target`.
    Path { path: Vec<String> },
    /// The argument expression is a reference to a path such as `&local_target`.
    ReferencedPath { path: Vec<String> },
    /// The argument expression boxes a path such as `Box::new(local_target)`.
    BoxedPath { path: Vec<String> },
    /// The argument expression is an inline non-async closure literal with a
    /// known executable body owner.
    Closure { closure_id: ExecutableBodyId },
    /// The argument expression is a local non-async closure binding, or a
    /// direct `clone()` of that binding, with a known executable body owner.
    ClosureBinding {
        path: Vec<String>,
        closure_id: ExecutableBodyId,
    },
    /// The argument expression constructs a local value with path-valued field
    /// initializers, such as `CallbackHolder { callback: local_target }`.
    Constructed {
        type_path: Vec<String>,
        fields: Vec<ArgumentFieldInit>,
    },
    /// The argument expression is an array with path-valued element
    /// initializers, such as `[local_target]`.
    Array {
        element_init_paths: Vec<Option<Vec<String>>>,
    },
    /// The argument expression is not represented by this conservative slice.
    #[default]
    Other,
}

/// Path-valued field initializer evidence for a constructed call argument.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct ArgumentFieldInit {
    pub field_path: Vec<String>,
    pub init_path: Vec<String>,
}

/// Coarse callee categories for syntactic path-call sites.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, Default)]
pub enum PathCallCallee {
    /// The callee is not shadowed by a visible local value binding.
    #[default]
    ItemPath,
    /// The callee path names a visible local value binding or parameter.
    ValueBinding { path: Vec<String> },
    /// The callee path names a visible local closure binding with a known
    /// executable body owner.
    ClosureBinding {
        path: Vec<String>,
        closure_id: ExecutableBodyId,
    },
    /// The callee path names a visible local async-closure binding whose
    /// returned future is not immediately awaited.
    AsyncClosureBinding {
        path: Vec<String>,
        closure_id: ExecutableBodyId,
    },
    /// The callee path names a visible local async-closure binding whose
    /// returned future is immediately awaited.
    AwaitedAsyncClosureBinding {
        path: Vec<String>,
        closure_id: ExecutableBodyId,
    },
    /// The callee path names a visible block-local function item with a known
    /// executable body owner.
    LocalFunctionBinding {
        path: Vec<String>,
        body_id: ExecutableBodyId,
    },
    /// The callee path names a visible local binding initialized from a path.
    InitializedValueBinding {
        path: Vec<String>,
        init_path: Vec<String>,
    },
    /// The callee path names a visible local binding whose branch initializer
    /// proves more than one possible local function item.
    AmbiguousInitializedValueBinding {
        path: Vec<String>,
        init_paths: Vec<Vec<String>>,
    },
    /// The callee path names a local binding destructured from a self field,
    /// such as `if let Some(f) = self.callback { f() }`.
    SelfFieldBinding {
        path: Vec<String>,
        field_path: Vec<String>,
    },
    /// The callee path names a visible local binding that aliases another
    /// visible local value binding or parameter.
    AliasedValueBinding {
        path: Vec<String>,
        source_path: Vec<String>,
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
    /// Whether this call occurrence is lexically inside an unsafe block.
    #[serde(default)]
    pub unsafe_block: bool,
    /// Method name token from the call syntax.
    pub method_name: String,
    /// Coarse structural receiver classification.
    pub receiver: MethodCallReceiver,
    /// Number of value arguments after the receiver.
    pub arg_count: usize,
    /// Number of explicit generic arguments on the method call.
    pub generic_arg_count: usize,
    /// Conservative argument summaries used only for exact local value-flow
    /// proof. Missing or unsupported argument shapes are represented as
    /// `Other`; arity remains authoritative in `arg_count`.
    #[serde(default)]
    pub arguments: Vec<CallArgument>,
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
    /// Whether this call occurrence is lexically inside an unsafe block.
    #[serde(default)]
    pub unsafe_block: bool,
    /// Number of value arguments at the call site.
    pub arg_count: usize,
    /// Conservative classification of the callee expression.
    #[serde(default)]
    pub callee: DynamicCallCallee,
}

/// A branch expression target that can participate in a dynamic call.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub enum DynamicBranchTarget {
    /// A branch evaluates to an unshadowed item path.
    Path { path: Vec<String> },
    /// A branch evaluates to an inline closure literal.
    Closure { closure_id: ExecutableBodyId },
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
    /// The callee expression is the result of calling a path, such as
    /// `make_fn()()`. This records only the inner path call plus whether the
    /// returned callable future is immediately awaited; semantic proof of the
    /// returned callable belongs to the resolver.
    ReturnedPathCall {
        path: Vec<String>,
        #[serde(default)]
        is_awaited: bool,
    },
    /// The callee expression is a visible local binding initialized from a path
    /// and then cast to a bare function pointer before being called, such as
    /// `let f = local_target; (f as fn() -> i32)()`.
    FnPointerCastInitializedLocalBinding {
        path: Vec<String>,
        init_path: Vec<String>,
    },
    /// The callee expression is an opaque visible local binding or parameter
    /// cast to a bare function pointer before being called, such as
    /// `(f as fn() -> i32)()` where `f` is a function-pointer parameter.
    FnPointerCastLocalBinding { path: Vec<String> },
    /// The callee expression is a local alias to another value binding or
    /// parameter and is cast to a bare function pointer before being called.
    FnPointerCastAliasedLocalBinding {
        path: Vec<String>,
        source_path: Vec<String>,
    },
    /// The callee expression is a visible local closure binding cast to a
    /// bare function pointer before being called, such as
    /// `let closure = || 1; (closure as fn() -> i32)()`.
    FnPointerCastClosureBinding {
        path: Vec<String>,
        closure_id: ExecutableBodyId,
    },
    /// The callee expression is a dereferenced visible local binding
    /// initialized from a path, such as `let f = local_target; (*f)()`.
    DereferencedInitializedLocalBinding {
        path: Vec<String>,
        init_path: Vec<String>,
    },
    /// The callee expression is a dereferenced visible local closure binding
    /// with a known executable body owner, such as `let closure = || 1; (*closure)()`.
    DereferencedClosureBinding {
        path: Vec<String>,
        closure_id: ExecutableBodyId,
    },
    /// The callee expression is a visible local binding or parameter, such as
    /// `(closure)()` or `(f)()`.
    LocalBinding { path: Vec<String> },
    /// The callee expression is a local alias to another value binding or
    /// parameter.
    AliasedLocalBinding {
        path: Vec<String>,
        source_path: Vec<String>,
    },
    /// The callee expression is a visible local closure binding with a known
    /// executable body owner, such as `(closure)()`.
    ClosureBinding {
        path: Vec<String>,
        closure_id: ExecutableBodyId,
    },
    /// The callee expression is a visible local async-closure binding whose
    /// returned future is not immediately awaited, such as `closure()`.
    AsyncClosureBinding {
        path: Vec<String>,
        closure_id: ExecutableBodyId,
    },
    /// The callee expression is a visible local async-closure binding whose
    /// returned future is immediately awaited, such as `closure().await`.
    AwaitedAsyncClosureBinding {
        path: Vec<String>,
        closure_id: ExecutableBodyId,
    },
    /// The callee expression is an inline non-async closure literal with a
    /// known executable body owner, such as `(|| value)()`.
    ClosureLiteral { closure_id: ExecutableBodyId },
    /// The callee expression is an inline async closure literal whose returned
    /// future is immediately awaited, such as `(async || value)().await`.
    AwaitedAsyncClosureLiteral { closure_id: ExecutableBodyId },
    /// The callee expression is a visible local binding initialized from a
    /// path, such as `let f = local_target; (f)()`.
    InitializedLocalBinding {
        path: Vec<String>,
        init_path: Vec<String>,
    },
    /// The callee expression is a field projection rooted at a local binding,
    /// such as `value.0()`.
    FieldLocalBinding { path: Vec<String> },
    /// The callee expression is a field projection rooted at `self`, such as
    /// `(self.callback)()`.
    SelfField { path: Vec<String> },
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
    /// The callee expression is an if expression whose supported branches
    /// evaluate to a mix of item paths and inline closure literals.
    IfBranchTargets { targets: Vec<DynamicBranchTarget> },
    /// The callee expression is a match expression whose supported arms each
    /// evaluate to a path expression, such as
    /// `(match flag { true => local_target, false => other_target })()`.
    MatchArmPaths { paths: Vec<Vec<String>> },
    /// The callee expression is a match expression whose supported arms
    /// evaluate to a mix of item paths and inline closure literals.
    MatchArmTargets { targets: Vec<DynamicBranchTarget> },
    /// The callee expression is an if expression whose supported branches each
    /// evaluate to the same visible callable parameter, such as
    /// `(if flag { f } else { f })()`.
    IfBranchParameter { path: Vec<String> },
    /// The callee expression is a match expression whose supported arms each
    /// evaluate to the same visible callable parameter, such as
    /// `(match flag { true => f, false => f })()`.
    MatchArmParameter { path: Vec<String> },
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
    /// Whether this call occurrence is lexically inside an unsafe block.
    #[serde(default)]
    pub unsafe_block: bool,
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
    /// The receiver is a local binding that aliases another visible value
    /// binding or parameter, such as `let alias = value; alias.method()`.
    AliasedLocalBinding {
        /// Binding identifier used as the receiver expression.
        name: String,
        /// Structural source path for the aliased value.
        source_path: Vec<String>,
    },
    /// The receiver is a local binding destructured from one element of a
    /// local function's tuple return value.
    TupleReturnBinding {
        /// Binding identifier used as the receiver expression.
        name: String,
        /// Structural path of the tuple-returning function initializer.
        path: Vec<String>,
        /// Zero-based tuple element index bound to `name`.
        index: usize,
    },
    /// The receiver is a local binding destructured from one element of a
    /// local method's tuple return value.
    TupleMethodReturn {
        /// Binding identifier used as the receiver expression.
        name: String,
        /// Method name used by the tuple-returning initializer call.
        method_name: String,
        /// Byte span of the tuple-returning initializer method call.
        method_span: (usize, usize),
        /// Zero-based tuple element index bound to `name`.
        index: usize,
    },
    /// The receiver is a local binding whose initializer was another method
    /// call, such as `let iter = iter.into_iter(); iter.size_hint()`.
    MethodResultLocalBinding {
        /// Binding identifier used as the receiver expression.
        name: String,
        /// Method name used by the initializer call.
        method_name: String,
        /// Byte span of the initializer method call.
        method_span: (usize, usize),
    },
    /// The receiver is a local binding destructured from a tuple enum variant
    /// field, such as `Self::Variant(value) => value.method()`.
    EnumVariantBinding {
        /// Binding identifier used as the receiver expression.
        name: String,
        /// Path naming the enum container in the pattern, commonly `Self`.
        enum_path: Vec<String>,
        /// Variant name that introduced the binding.
        variant_name: String,
        /// Zero-based tuple field index in the variant.
        field_index: usize,
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
    /// The receiver is a borrowed local binding whose initializer is a path
    /// expression visible at the call site.
    BorrowedInitializedLocalBinding {
        /// Binding identifier inside the borrow expression.
        name: String,
        /// Structural initializer path carried to the resolver for exact local
        /// type proof.
        init_path: Vec<String>,
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
    /// The receiver is the awaited result of a method call, such as
    /// `value.make_future().await`.
    AwaitMethodCallResult {
        /// Method name used by the awaited receiver call.
        method_name: String,
    },
    /// The receiver is the result of a try expression, such as `value?`.
    TryResult,
    /// The receiver is the try result of a path call, such as
    /// `make_value()?`.
    TryPathCallResult {
        /// Path used as the tried receiver call's callee.
        path: Vec<String>,
    },
    /// The receiver is the try result of a method call, such as
    /// `value.make_result()?`.
    TryMethodCallResult {
        /// Method name used by the tried receiver call.
        method_name: String,
    },
    /// The receiver is an if expression with path-valued branches.
    IfBranchPaths {
        /// Structural type paths from each branch expression.
        paths: Vec<Vec<String>>,
    },
    /// The receiver is a literal expression, such as `"x"`.
    Literal,
    /// The receiver expression is visible but outside this conservative
    /// structural classifier.
    Unsupported,
}
