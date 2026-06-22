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
    /// Number of value arguments at the call site.
    pub arg_count: usize,
    /// Number of explicit generic arguments on the callee, if represented by
    /// the syntax class.
    pub generic_arg_count: usize,
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
}
