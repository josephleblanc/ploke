#![cfg(feature = "typed_type_graph")]

//! Paranoid call-site test helpers.
//!
//! This module is the call-site analogue of the node-level paranoid helpers in
//! `macro_rule_tests.rs`: it regenerates the expected typed call-site ID,
//! checks exact ID lookup, checks value lookup, checks containment relation
//! facts, and checks resolver status/edge facts.

use crate::common::{
    AssocParanoidArgs, PARSED_FIXTURE_CRATE_DIR_DETECTION, PARSED_FIXTURE_CRATE_NODES,
    PARSED_FIXTURE_CRATE_PATH_RESOLUTION, PARSED_FIXTURE_CRATE_SPP_EDGE_CASES,
    PARSED_FIXTURE_CRATE_SPP_EDGE_CASES_NO_CFG, PARSED_FIXTURE_CRATE_TYPES,
};
use syn_parser::error::SynParserError;
use syn_parser::parser::ParsedCodeGraph;
use syn_parser::parser::graph::GraphAccess;
use syn_parser::parser::nodes::test_ids::{TestCallIds, generate_test_call_id};
use syn_parser::parser::nodes::{
    AnyCallSiteId, CallBodyOwnerId, CallNode, CallSiteKind, FunctionNodeId, MacroCallSiteId,
    MethodCallReceiver, MethodCallSiteId, MethodNodeId, PathCallSiteId,
};
use syn_parser::parser::relations::{
    CallRelation, CallResolutionKind, CallResolutionStatus, CallSiteRelation,
};
use syn_parser::resolve::call_resolution::CallResolutionReport;

/// The function-like body that should own an expected call site.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallOwnerContext {
    /// Typed owner endpoint used by call-site IDs and containment relations.
    pub id: CallBodyOwnerId,
    /// Source span of the owner body/item; used as a sanity check around the
    /// call-site expression span.
    pub span: (usize, usize),
    /// Human-readable owner label for failure messages.
    pub label: String,
}

/// Expected structural method receiver classifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExpectedMethodReceiver<'a> {
    /// The receiver expression is literal `self`.
    SelfValue,
    /// The receiver expression is a field projection rooted at `self`.
    SelfField { field_path: &'a [&'a str] },
}

impl ExpectedMethodReceiver<'_> {
    fn to_actual(self) -> MethodCallReceiver {
        match self {
            Self::SelfValue => MethodCallReceiver::SelfValue,
            Self::SelfField { field_path } => MethodCallReceiver::SelfField {
                field_path: field_path.iter().copied().map(String::from).collect(),
            },
        }
    }
}

/// Expected structural call-site class and class-specific fields.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExpectedCallKind<'a> {
    /// Path-style call expression, e.g. `foo()` or `Type::new()`.
    Path {
        path: &'a [&'a str],
        arg_count: usize,
        generic_arg_count: usize,
    },
    /// Method-call expression, e.g. `self.foo()` or `self.field.len()`.
    Method {
        method_name: &'a str,
        receiver: ExpectedMethodReceiver<'a>,
        arg_count: usize,
        generic_arg_count: usize,
    },
    /// Macro invocation expression/statement, e.g. `println!(...)`.
    Macro { macro_name: &'a str },
}

/// Expected resolver outcome for a call site.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExpectedCallOutcome {
    /// Resolver should fail closed with `Unsupported` and no semantic edge.
    Unsupported,
    /// Resolver should fail closed with `Unresolved` and no semantic edge.
    Unresolved,
    /// Resolver should fail closed with `Ambiguous` and no semantic edge.
    Ambiguous,
    /// Resolver should classify the target as external and emit no local edge.
    External,
    /// Resolver should produce a local exact method edge.
    ResolvedMethodLocalExact { target: MethodNodeId },
    /// Resolver should produce a local exact function edge.
    ResolvedFunctionLocalExact { target: FunctionNodeId },
}

/// Full expected call-site fact row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExpectedCallSite<'a> {
    /// Structural kind and kind-specific fields.
    pub kind: ExpectedCallKind<'a>,
    /// Byte span of the full call expression/invocation.
    pub span: (usize, usize),
    /// Effective cfg strings expected on the call occurrence.
    pub cfgs: &'a [&'a str],
    /// Expected resolver status and edge policy.
    pub outcome: ExpectedCallOutcome,
}

impl<'a> ExpectedCallSite<'a> {
    /// Constructor for a path-call expectation.
    pub const fn path(
        path: &'a [&'a str],
        span: (usize, usize),
        arg_count: usize,
        generic_arg_count: usize,
        cfgs: &'a [&'a str],
        outcome: ExpectedCallOutcome,
    ) -> Self {
        Self {
            kind: ExpectedCallKind::Path {
                path,
                arg_count,
                generic_arg_count,
            },
            span,
            cfgs,
            outcome,
        }
    }

    /// Constructor for a method-call expectation.
    pub const fn method(
        method_name: &'a str,
        receiver: ExpectedMethodReceiver<'a>,
        span: (usize, usize),
        arg_count: usize,
        generic_arg_count: usize,
        cfgs: &'a [&'a str],
        outcome: ExpectedCallOutcome,
    ) -> Self {
        Self {
            kind: ExpectedCallKind::Method {
                method_name,
                receiver,
                arg_count,
                generic_arg_count,
            },
            span,
            cfgs,
            outcome,
        }
    }

    /// Constructor for a macro-call expectation.
    pub const fn macro_call(
        macro_name: &'a str,
        span: (usize, usize),
        cfgs: &'a [&'a str],
        outcome: ExpectedCallOutcome,
    ) -> Self {
        Self {
            kind: ExpectedCallKind::Macro { macro_name },
            span,
            cfgs,
            outcome,
        }
    }

    fn expected_id(&self, owner: CallBodyOwnerId, cfgs: &[String]) -> AnyCallSiteId {
        match self.kind {
            ExpectedCallKind::Path { path, .. } => {
                let discriminator = path.join("::");
                PathCallSiteId::new_call_test(generate_test_call_id(
                    owner,
                    CallSiteKind::Path,
                    discriminator.as_str(),
                    self.span,
                    cfgs,
                ))
                .into()
            }
            ExpectedCallKind::Method { method_name, .. } => MethodCallSiteId::new_call_test(
                generate_test_call_id(owner, CallSiteKind::Method, method_name, self.span, cfgs),
            )
            .into(),
            ExpectedCallKind::Macro { macro_name } => MacroCallSiteId::new_call_test(
                generate_test_call_id(owner, CallSiteKind::Macro, macro_name, self.span, cfgs),
            )
            .into(),
        }
    }

    fn matches_values(
        &self,
        call: &CallNode,
        owner: CallBodyOwnerId,
        expected_cfgs: &[String],
    ) -> bool {
        if call.owner() != owner || call.span() != self.span || call.cfgs() != expected_cfgs {
            return false;
        }

        match (self.kind, call) {
            (
                ExpectedCallKind::Path {
                    path,
                    arg_count,
                    generic_arg_count,
                },
                CallNode::PathCall(actual),
            ) => {
                path_matches(&actual.path, path)
                    && actual.arg_count == arg_count
                    && actual.generic_arg_count == generic_arg_count
            }
            (
                ExpectedCallKind::Method {
                    method_name,
                    receiver,
                    arg_count,
                    generic_arg_count,
                },
                CallNode::MethodCall(actual),
            ) => {
                actual.method_name == method_name
                    && actual.receiver == receiver.to_actual()
                    && actual.arg_count == arg_count
                    && actual.generic_arg_count == generic_arg_count
            }
            (ExpectedCallKind::Macro { macro_name }, CallNode::MacroCall(actual)) => {
                actual.macro_name == macro_name
            }
            _ => false,
        }
    }

    fn assert_fields(
        &self,
        call: &CallNode,
        owner: &CallOwnerContext,
        expected_cfgs: &[String],
        expected_id: AnyCallSiteId,
    ) {
        assert_eq!(
            call.owner(),
            owner.id,
            "call-site owner mismatch for {}",
            self.label()
        );
        assert_eq!(
            call.id(),
            expected_id,
            "call-site typed ID mismatch for {}",
            self.label()
        );
        assert_eq!(
            call.span(),
            self.span,
            "call-site span mismatch for {}",
            self.label()
        );
        assert_eq!(
            call.cfgs(),
            expected_cfgs,
            "call-site cfgs mismatch for {}",
            self.label()
        );
        assert!(
            owner.span.0 <= self.span.0 && self.span.1 <= owner.span.1,
            "call-site span {:?} for {} should be inside owner {} span {:?}",
            self.span,
            self.label(),
            owner.label,
            owner.span
        );

        match (self.kind, call, expected_id) {
            (
                ExpectedCallKind::Path {
                    path,
                    arg_count,
                    generic_arg_count,
                },
                CallNode::PathCall(actual),
                AnyCallSiteId::Path(expected_path_id),
            ) => {
                assert_eq!(actual.id, expected_path_id);
                assert_eq!(actual.owner, owner.id);
                assert!(
                    path_matches(&actual.path, path),
                    "path-call path mismatch for {}: expected {:?}, actual {:?}",
                    self.label(),
                    path,
                    actual.path
                );
                assert_eq!(actual.arg_count, arg_count);
                assert_eq!(actual.generic_arg_count, generic_arg_count);
            }
            (
                ExpectedCallKind::Method {
                    method_name,
                    receiver,
                    arg_count,
                    generic_arg_count,
                },
                CallNode::MethodCall(actual),
                AnyCallSiteId::Method(expected_method_id),
            ) => {
                assert_eq!(actual.id, expected_method_id);
                assert_eq!(actual.owner, owner.id);
                assert_eq!(actual.method_name, method_name);
                assert_eq!(actual.receiver, receiver.to_actual());
                assert_eq!(actual.arg_count, arg_count);
                assert_eq!(actual.generic_arg_count, generic_arg_count);
            }
            (
                ExpectedCallKind::Macro { macro_name },
                CallNode::MacroCall(actual),
                AnyCallSiteId::Macro(expected_macro_id),
            ) => {
                assert_eq!(actual.id, expected_macro_id);
                assert_eq!(actual.owner, owner.id);
                assert_eq!(actual.macro_name, macro_name);
            }
            _ => panic!(
                "call-site variant mismatch for {}: expected {:?}, actual {}",
                self.label(),
                self.kind,
                describe_call(call)
            ),
        }
    }

    fn label(&self) -> String {
        match self.kind {
            ExpectedCallKind::Path { path, .. } => format!("path call {}()", path.join("::")),
            ExpectedCallKind::Method { method_name, .. } => {
                format!("method call {method_name}()")
            }
            ExpectedCallKind::Macro { macro_name } => format!("macro call {macro_name}!(...)"),
        }
    }
}

/// Returns parsed phase-2 fixture rows for a known fixture name.
pub fn parsed_graphs_for_fixture(fixture: &str) -> &'static [ParsedCodeGraph] {
    match fixture {
        "fixture_nodes" => &PARSED_FIXTURE_CRATE_NODES,
        "file_dir_detection" => &PARSED_FIXTURE_CRATE_DIR_DETECTION,
        "fixture_path_resolution" => &PARSED_FIXTURE_CRATE_PATH_RESOLUTION,
        "fixture_spp_edge_cases_no_cfg" => &PARSED_FIXTURE_CRATE_SPP_EDGE_CASES_NO_CFG,
        "fixture_spp_edge_cases" => &PARSED_FIXTURE_CRATE_SPP_EDGE_CASES,
        "fixture_types" => &PARSED_FIXTURE_CRATE_TYPES,
        _ => panic!(
            "Unknown fixture name for lazy_static lookup: {fixture}. Ensure it is registered in tests/common/parsed_fixtures.rs and call_site_paranoid.rs."
        ),
    }
}

/// Builds a function owner context from module path and function name.
pub fn function_owner_context(
    graph: &impl GraphAccess,
    module_path: &[&str],
    function_name: &str,
) -> CallOwnerContext {
    let module_path_vec = module_path
        .iter()
        .copied()
        .map(String::from)
        .collect::<Vec<_>>();
    let module = graph
        .find_module_by_path_checked(&module_path_vec)
        .expect("call-site test fixture module should exist");
    let matches = graph
        .functions()
        .iter()
        .filter(|function| {
            function.name == function_name
                && graph.module_contains_node(module.id, function.id.into())
        })
        .collect::<Vec<_>>();
    let function = match matches.as_slice() {
        [function] => function,
        [] => panic!(
            "expected function {function_name:?} in module path {}",
            module_path_vec.join("::")
        ),
        many => panic!(
            "expected exactly one function {function_name:?} in module path {}, found {}",
            module_path_vec.join("::"),
            many.len()
        ),
    };

    CallOwnerContext {
        id: CallBodyOwnerId::Function(function.id),
        span: function.span,
        label: format!("function {}::{}", module_path_vec.join("::"), function.name),
    }
}

/// Builds a method owner context from associated-item paranoid args.
pub fn method_owner_context(
    graph: &impl GraphAccess,
    parsed_graphs: &[ParsedCodeGraph],
    args: &AssocParanoidArgs<'_>,
) -> Result<CallOwnerContext, SynParserError> {
    let method_info = args.generate_method_pid(parsed_graphs)?;
    let method = graph
        .find_node_unique(method_info.test_any_id())
        .unwrap_or_else(|_| panic!("{} id should resolve to a unique graph node", args.ident))
        .as_method()
        .unwrap_or_else(|| panic!("{} id should resolve to MethodNode", args.ident));

    Ok(CallOwnerContext {
        id: CallBodyOwnerId::Method(method_info.test_method_id()),
        span: method.span,
        label: format!("method {}", args.ident),
    })
}

/// Performs exact-ID, value, relation, and resolver checks for one expected
/// call-site occurrence.
pub fn assert_paranoid_call_site(
    graph: &impl GraphAccess,
    report: &CallResolutionReport,
    owner: &CallOwnerContext,
    expected: &ExpectedCallSite<'_>,
) {
    let expected_cfgs = expected
        .cfgs
        .iter()
        .copied()
        .map(String::from)
        .collect::<Vec<_>>();
    let expected_id = expected.expected_id(owner.id, &expected_cfgs);

    let id_matches = graph
        .call_sites()
        .iter()
        .filter(|call| call.id() == expected_id)
        .collect::<Vec<_>>();
    assert_eq!(
        id_matches.len(),
        1,
        "expected exactly one call site with regenerated ID {expected_id:?} for {}. Matches: {:#?}",
        expected.label(),
        id_matches
    );
    let call_by_id = id_matches[0];
    expected.assert_fields(call_by_id, owner, &expected_cfgs, expected_id);

    let value_matches = graph
        .call_sites()
        .iter()
        .filter(|call| expected.matches_values(call, owner.id, &expected_cfgs))
        .collect::<Vec<_>>();
    assert!(
        !value_matches.is_empty(),
        "expected at least one value match for {} owned by {}",
        expected.label(),
        owner.label
    );
    for duplicate in value_matches
        .iter()
        .copied()
        .filter(|call| call.id() != expected_id)
    {
        log::warn!(
            "Duplicate call-site values with different ID for {}: expected {:?}, duplicate {}",
            expected.label(),
            expected_id,
            describe_call(duplicate)
        );
    }
    let value_and_id_matches = value_matches
        .iter()
        .copied()
        .filter(|call| call.id() == expected_id)
        .collect::<Vec<_>>();
    assert_eq!(
        value_and_id_matches.len(),
        1,
        "expected exactly one call site matching values and regenerated ID for {}. Value matches: {:#?}",
        expected.label(),
        value_matches
    );

    assert_body_contains_call(graph, owner.id, expected_id, expected);
    assert_resolution_outcome(report, expected_id, expected.outcome);
}

fn assert_body_contains_call(
    graph: &impl GraphAccess,
    owner: CallBodyOwnerId,
    expected_id: AnyCallSiteId,
    expected: &ExpectedCallSite<'_>,
) {
    let relation_count = graph
        .call_site_relations()
        .iter()
        .filter(|relation| {
            matches!(
                relation,
                CallSiteRelation::BodyContainsCall { source, target }
                    if *source == owner && *target == expected_id
            )
        })
        .count();
    assert_eq!(
        relation_count,
        1,
        "expected exactly one BodyContainsCall relation for {}",
        expected.label()
    );
}

fn assert_resolution_outcome(
    report: &CallResolutionReport,
    expected_id: AnyCallSiteId,
    expected_outcome: ExpectedCallOutcome,
) {
    let statuses = report
        .statuses
        .iter()
        .copied()
        .filter(|status| status.source() == expected_id)
        .collect::<Vec<_>>();
    assert_eq!(
        statuses.len(),
        1,
        "expected exactly one resolver status for {expected_id:?}, found {statuses:#?}"
    );
    let status = statuses[0];

    match expected_outcome {
        ExpectedCallOutcome::Unsupported => assert!(
            matches!(status, CallResolutionStatus::Unsupported { source } if source == expected_id),
            "expected Unsupported status for {expected_id:?}, got {status:?}"
        ),
        ExpectedCallOutcome::Unresolved => assert!(
            matches!(status, CallResolutionStatus::Unresolved { source } if source == expected_id),
            "expected Unresolved status for {expected_id:?}, got {status:?}"
        ),
        ExpectedCallOutcome::Ambiguous => assert!(
            matches!(status, CallResolutionStatus::Ambiguous { source } if source == expected_id),
            "expected Ambiguous status for {expected_id:?}, got {status:?}"
        ),
        ExpectedCallOutcome::External => assert!(
            matches!(status, CallResolutionStatus::External { source } if source == expected_id),
            "expected External status for {expected_id:?}, got {status:?}"
        ),
        ExpectedCallOutcome::ResolvedMethodLocalExact { .. }
        | ExpectedCallOutcome::ResolvedFunctionLocalExact { .. } => assert!(
            matches!(
                status,
                CallResolutionStatus::Resolved {
                    source,
                    kind: CallResolutionKind::LocalExact,
                } if source == expected_id
            ),
            "expected Resolved(LocalExact) status for {expected_id:?}, got {status:?}"
        ),
    }

    let relations = report
        .relations
        .iter()
        .copied()
        .filter(|relation| relation_source(*relation) == expected_id)
        .collect::<Vec<_>>();

    match expected_outcome {
        ExpectedCallOutcome::Unsupported
        | ExpectedCallOutcome::Unresolved
        | ExpectedCallOutcome::Ambiguous
        | ExpectedCallOutcome::External => assert_eq!(
            relations.len(),
            0,
            "non-resolved call site {expected_id:?} should not emit semantic call edges; got {relations:#?}"
        ),
        ExpectedCallOutcome::ResolvedMethodLocalExact { target } => {
            let source = match expected_id {
                AnyCallSiteId::Method(source) => source,
                other => panic!("method relation expected a method call-site ID, got {other:?}"),
            };
            assert_eq!(
                relations.len(),
                1,
                "resolved method call site {expected_id:?} should emit exactly one semantic edge; got {relations:#?}"
            );
            assert!(
                matches!(relations[0], CallRelation::Method { source: actual_source, target: actual_target }
                    if actual_source == source && actual_target == target),
                "expected Method edge {source:?} -> {target:?}, got {:?}",
                relations[0]
            );
        }
        ExpectedCallOutcome::ResolvedFunctionLocalExact { target } => {
            let source = match expected_id {
                AnyCallSiteId::Path(source) => source,
                other => panic!("function relation expected a path call-site ID, got {other:?}"),
            };
            assert_eq!(
                relations.len(),
                1,
                "resolved function call site {expected_id:?} should emit exactly one semantic edge; got {relations:#?}"
            );
            assert!(
                matches!(relations[0], CallRelation::Function { source: actual_source, target: actual_target }
                    if actual_source == source && actual_target == target),
                "expected Function edge {source:?} -> {target:?}, got {:?}",
                relations[0]
            );
        }
    }
}

fn relation_source(relation: CallRelation) -> AnyCallSiteId {
    match relation {
        CallRelation::Function { source, .. } => source.into(),
        CallRelation::Method { source, .. } => source.into(),
    }
}

fn path_matches(actual: &[String], expected: &[&str]) -> bool {
    actual
        .iter()
        .map(String::as_str)
        .eq(expected.iter().copied())
}

fn describe_call(call: &CallNode) -> String {
    match call {
        CallNode::PathCall(call) => format!(
            "PathCall(id={:?}, owner={:?}, path={:?}, span={:?}, args={}, generics={})",
            call.id, call.owner, call.path, call.span, call.arg_count, call.generic_arg_count
        ),
        CallNode::MethodCall(call) => format!(
            "MethodCall(id={:?}, owner={:?}, method={}, receiver={:?}, span={:?}, args={}, generics={})",
            call.id,
            call.owner,
            call.method_name,
            call.receiver,
            call.span,
            call.arg_count,
            call.generic_arg_count
        ),
        CallNode::DynamicCall(call) => format!(
            "DynamicCall(id={:?}, owner={:?}, span={:?}, args={})",
            call.id, call.owner, call.span, call.arg_count
        ),
        CallNode::MacroCall(call) => format!(
            "MacroCall(id={:?}, owner={:?}, macro={}, span={:?})",
            call.id, call.owner, call.macro_name, call.span
        ),
    }
}
