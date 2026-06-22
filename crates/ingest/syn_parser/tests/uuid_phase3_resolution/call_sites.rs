#![cfg(feature = "typed_type_graph")]

//! Structural RED tests for parser-owned call-site records.
//!
//! This module intentionally names the intended typed call-site API before it
//! exists. Task 1.1 should therefore fail to compile until the parser adds the
//! call-site nodes, IDs, relations, and graph accessors asserted below.

use syn_parser::error::SynParserError;
use syn_parser::parser::graph::GraphAccess;
use syn_parser::parser::nodes::test_ids::{TestCallIds, generate_test_call_id};
use syn_parser::parser::nodes::{
    AnyCallSiteId, CallBodyOwnerId, CallNode, CallSiteKind, MacroCallSiteId, MethodCallReceiver,
    MethodCallSiteId, PathCallSiteId,
};
use syn_parser::parser::relations::{
    CallRelation, CallResolutionKind, CallResolutionStatus, CallSiteRelation,
};
use syn_parser::resolve::call_resolution::{
    CallResolutionReport, resolve_call_relations_after_tree,
};

use crate::common::{
    AssocOwner, AssocParanoidArgs, PARSED_FIXTURE_CRATE_NODES, build_tree_for_tests,
};

const IMPLS_RS: &str = "src/impls.rs";
const SIMPLE_STRUCT_IMPL_SPAN: (usize, usize) = (520, 750);
const SELF_PRIVATE_METHOD_CALL_SPAN: (usize, usize) = (721, 742);
const HASHMAP_NEW_CALL_SPAN: (usize, usize) = (3413, 3442);
const FS_READ_TO_STRING_CALL_SPAN: (usize, usize) = (3838, 3865);
const PATHBUF_NEW_CALL_SPAN: (usize, usize) = (3930, 3944);
const ENUM_VARIANT1_CALL_SPAN: (usize, usize) = (4007, 4032);
const DOCUMENTED_MACRO_CALL_SPAN: (usize, usize) = (4894, 4935);
const DURATION_FROM_SECS_CALL_SPAN: (usize, usize) = (5235, 5257);
const ARC_NEW_CALL_SPAN: (usize, usize) = (5452, 5463);
const TUPLE_STRUCT_CALL_SPAN: (usize, usize) = (5549, 5566);
const PRINTLN_USED_CALL_SPAN: (usize, usize) = (4395, 4461);

fn simple_struct_inherent_method_args(ident: &'static str) -> AssocParanoidArgs<'static> {
    AssocParanoidArgs {
        fixture: "fixture_nodes",
        relative_file_path: IMPLS_RS,
        expected_path: &["crate", "impls"],
        owner: AssocOwner::Impl {
            span: SIMPLE_STRUCT_IMPL_SPAN,
        },
        ident,
        expected_cfg: None,
    }
}

fn fixture_nodes_function<'a>(
    graph: &'a impl GraphAccess,
    module_path: &[&str],
    function_name: &str,
) -> &'a syn_parser::parser::nodes::FunctionNode {
    let module_path = module_path
        .iter()
        .copied()
        .map(String::from)
        .collect::<Vec<_>>();
    let module = graph
        .find_module_by_path_checked(&module_path)
        .expect("fixture module should exist");
    let matches = graph
        .functions()
        .iter()
        .filter(|function| {
            function.name == function_name
                && graph.module_contains_node(module.id, function.id.into())
        })
        .collect::<Vec<_>>();
    match matches.as_slice() {
        [function] => function,
        [] => panic!(
            "expected function {function_name:?} in module path {}",
            module_path.join("::")
        ),
        many => panic!(
            "expected exactly one function {function_name:?} in module path {}, found {}",
            module_path.join("::"),
            many.len()
        ),
    }
}

struct PathCallExpectation<'a> {
    path: &'a [&'a str],
    span: (usize, usize),
    arg_count: usize,
    generic_arg_count: usize,
}

fn path_matches(path: &[String], expected: &[&str]) -> bool {
    path.iter().map(String::as_str).eq(expected.iter().copied())
}

fn expect_unsupported_path_call<'a>(
    graph: &'a impl GraphAccess,
    report: &CallResolutionReport,
    owner: CallBodyOwnerId,
    expectation: &PathCallExpectation<'_>,
) -> &'a syn_parser::parser::nodes::PathCallNode {
    let matching_path_calls = graph
        .call_sites()
        .iter()
        .filter_map(|call| match call {
            CallNode::PathCall(path_call)
                if path_call.owner == owner
                    && path_matches(&path_call.path, expectation.path)
                    && path_call.span == expectation.span =>
            {
                Some(path_call)
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        matching_path_calls.len(),
        1,
        "expected exactly one path call {:?} at {:?}",
        expectation.path,
        expectation.span
    );
    let path_call = matching_path_calls[0];

    assert_eq!(path_call.owner, owner);
    assert!(path_matches(&path_call.path, expectation.path));
    assert_eq!(path_call.arg_count, expectation.arg_count);
    assert_eq!(path_call.generic_arg_count, expectation.generic_arg_count);
    assert_eq!(path_call.span, expectation.span);

    let path_discriminator = expectation.path.join("::");
    let regenerated_call_id = PathCallSiteId::new_call_test(generate_test_call_id(
        owner,
        CallSiteKind::Path,
        path_discriminator.as_str(),
        path_call.span,
        path_call.cfgs.as_slice(),
    ));
    assert_eq!(
        path_call.id, regenerated_call_id,
        "path call site id should be deterministic from owner + path + span + cfgs"
    );

    let parsed_call_id = AnyCallSiteId::Path(path_call.id);
    let body_contains_count = graph
        .call_site_relations()
        .iter()
        .filter(|relation| {
            matches!(
                relation,
                CallSiteRelation::BodyContainsCall { source, target }
                    if *source == owner && *target == parsed_call_id
            )
        })
        .count();
    assert_eq!(
        body_contains_count, 1,
        "expected exactly one BodyContainsCall relation for path call {:?}",
        expectation.path
    );

    let resolved_edge_count = report
        .relations
        .iter()
        .filter(|relation| {
            matches!(relation, CallRelation::Function { source, .. } if *source == path_call.id)
        })
        .count();
    assert_eq!(
        resolved_edge_count, 0,
        "path-call coverage rows should not emit resolved function edges yet"
    );

    let unsupported_status_count = report
        .statuses
        .iter()
        .filter(|status| {
            matches!(
                status,
                CallResolutionStatus::Unsupported { source }
                    if *source == parsed_call_id
            )
        })
        .count();
    assert_eq!(
        unsupported_status_count, 1,
        "path call {:?} should currently receive exactly one Unsupported status",
        expectation.path
    );

    path_call
}

fn expect_unsupported_macro_call<'a>(
    graph: &'a impl GraphAccess,
    report: &CallResolutionReport,
    owner: CallBodyOwnerId,
    macro_name: &str,
    span: (usize, usize),
) -> &'a syn_parser::parser::nodes::MacroCallNode {
    let matching_macro_calls = graph
        .call_sites()
        .iter()
        .filter_map(|call| match call {
            CallNode::MacroCall(macro_call)
                if macro_call.owner == owner
                    && macro_call.macro_name == macro_name
                    && macro_call.span == span =>
            {
                Some(macro_call)
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        matching_macro_calls.len(),
        1,
        "expected exactly one macro call {macro_name}! at {span:?}"
    );
    let macro_call = matching_macro_calls[0];

    let regenerated_call_id = MacroCallSiteId::new_call_test(generate_test_call_id(
        owner,
        CallSiteKind::Macro,
        macro_call.macro_name.as_str(),
        macro_call.span,
        macro_call.cfgs.as_slice(),
    ));
    assert_eq!(macro_call.id, regenerated_call_id);

    let parsed_call_id = AnyCallSiteId::Macro(macro_call.id);
    let body_contains_count = graph
        .call_site_relations()
        .iter()
        .filter(|relation| {
            matches!(
                relation,
                CallSiteRelation::BodyContainsCall { source, target }
                    if *source == owner && *target == parsed_call_id
            )
        })
        .count();
    assert_eq!(body_contains_count, 1);

    let unsupported_status_count = report
        .statuses
        .iter()
        .filter(|status| {
            matches!(
                status,
                CallResolutionStatus::Unsupported { source }
                    if *source == parsed_call_id
            )
        })
        .count();
    assert_eq!(unsupported_status_count, 1);

    macro_call
}

#[test]
fn fixture_nodes_public_method_records_self_private_method_call_site() -> Result<(), SynParserError>
{
    let public_args = simple_struct_inherent_method_args("public_method");
    let private_args = simple_struct_inherent_method_args("private_method");

    let public_info = public_args.generate_method_pid(&*PARSED_FIXTURE_CRATE_NODES)?;
    let private_info = private_args.generate_method_pid(&*PARSED_FIXTURE_CRATE_NODES)?;

    let graph = public_info.target_data();
    assert_eq!(
        private_info.target_data().file_path,
        graph.file_path,
        "public_method and private_method should be parsed from the same fixture row"
    );

    let public_method_id = public_info.test_method_id();
    let private_method_id = private_info.test_method_id();

    let public_method = graph
        .find_node_unique(public_info.test_any_id())
        .expect("public_method id should resolve to a unique graph node")
        .as_method()
        .expect("public_method id should resolve to MethodNode");
    assert_eq!(public_method.name, "public_method");

    let private_method = graph
        .find_node_unique(private_info.test_any_id())
        .expect("private_method id should resolve to a unique graph node")
        .as_method()
        .expect("private_method id should resolve to MethodNode");
    assert_eq!(private_method.name, "private_method");

    let public_owner = CallBodyOwnerId::Method(public_method_id);
    let private_owner = CallBodyOwnerId::Method(private_method_id);

    let public_owned_calls: Vec<&CallNode> = graph
        .call_sites()
        .iter()
        .filter(|call| call.owner() == public_owner)
        .collect();
    assert_eq!(
        public_owned_calls.len(),
        1,
        "SimpleStruct::public_method should own exactly one parsed call site"
    );

    let method_call = match public_owned_calls[0] {
        CallNode::MethodCall(method_call) => method_call,
        other => panic!("expected self.private_method() to be a method call, got {other:?}"),
    };

    assert_eq!(method_call.owner, public_owner);
    assert_eq!(method_call.method_name, "private_method");
    assert_eq!(method_call.receiver, MethodCallReceiver::SelfValue);
    assert_eq!(method_call.arg_count, 0);
    assert_eq!(method_call.generic_arg_count, 0);
    assert_eq!(method_call.span, SELF_PRIVATE_METHOD_CALL_SPAN);
    assert!(
        public_method.span.0 <= method_call.span.0 && method_call.span.1 <= public_method.span.1,
        "call span {:?} should be inside public_method span {:?}",
        method_call.span,
        public_method.span
    );
    assert!(
        method_call.cfgs.is_empty(),
        "fixture_nodes impls.rs public_method call site should have no cfgs"
    );

    let regenerated_call_id = MethodCallSiteId::new_call_test(generate_test_call_id(
        public_owner,
        CallSiteKind::Method,
        method_call.method_name.as_str(),
        method_call.span,
        method_call.cfgs.as_slice(),
    ));
    assert_eq!(
        method_call.id, regenerated_call_id,
        "method call site id should be deterministic from owner + name + span + cfgs"
    );

    let parsed_call_id = AnyCallSiteId::Method(method_call.id);
    let body_contains_count = graph
        .call_site_relations()
        .iter()
        .filter(|relation| {
            matches!(
                relation,
                CallSiteRelation::BodyContainsCall { source, target }
                    if *source == public_owner && *target == parsed_call_id
            )
        })
        .count();
    assert_eq!(
        body_contains_count, 1,
        "expected exactly one non-duplicate BodyContainsCall relation from public_method to self.private_method()"
    );

    let private_owned_call_count = graph
        .call_sites()
        .iter()
        .filter(|call| call.owner() == private_owner)
        .count();
    assert_eq!(
        private_owned_call_count, 0,
        "SimpleStruct::private_method should own zero parsed call sites in this fixture row"
    );

    Ok(())
}

#[test]
fn fixture_nodes_use_imported_items_records_pathbuf_new_path_call_site()
-> Result<(), SynParserError> {
    let (graph, tree) = build_tree_for_tests("fixture_nodes");
    let function = fixture_nodes_function(&graph, &["crate", "imports"], "use_imported_items");
    let owner = CallBodyOwnerId::Function(function.id);

    let matching_path_calls = graph
        .call_sites()
        .iter()
        .filter_map(|call| match call {
            CallNode::PathCall(path_call)
                if path_call.owner == owner
                    && path_call.path == ["PathBuf", "new"]
                    && path_call.span == PATHBUF_NEW_CALL_SPAN =>
            {
                Some(path_call)
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        matching_path_calls.len(),
        1,
        "use_imported_items should record exactly one PathBuf::new() path call site"
    );
    let path_call = matching_path_calls[0];

    assert_eq!(path_call.owner, owner);
    assert_eq!(path_call.path, ["PathBuf", "new"]);
    assert_eq!(path_call.arg_count, 0);
    assert_eq!(path_call.generic_arg_count, 0);
    assert_eq!(path_call.span, PATHBUF_NEW_CALL_SPAN);
    assert!(
        function.span.0 <= path_call.span.0 && path_call.span.1 <= function.span.1,
        "call span {:?} should be inside use_imported_items span {:?}",
        path_call.span,
        function.span
    );
    assert!(
        path_call.cfgs.is_empty(),
        "fixture_nodes imports path call should have no cfgs"
    );

    let path_discriminator = path_call.path.join("::");
    let regenerated_call_id = PathCallSiteId::new_call_test(generate_test_call_id(
        owner,
        CallSiteKind::Path,
        path_discriminator.as_str(),
        path_call.span,
        path_call.cfgs.as_slice(),
    ));
    assert_eq!(
        path_call.id, regenerated_call_id,
        "path call site id should be deterministic from owner + path + span + cfgs"
    );

    let parsed_call_id = AnyCallSiteId::Path(path_call.id);
    let body_contains_count = graph
        .call_site_relations()
        .iter()
        .filter(|relation| {
            matches!(
                relation,
                CallSiteRelation::BodyContainsCall { source, target }
                    if *source == owner && *target == parsed_call_id
            )
        })
        .count();
    assert_eq!(
        body_contains_count, 1,
        "expected exactly one BodyContainsCall relation from use_imported_items to PathBuf::new()"
    );

    let report = resolve_call_relations_after_tree(&graph, &tree)?;
    let resolved_edge_count = report
        .relations
        .iter()
        .filter(|relation| matches!(relation, CallRelation::Function { source, .. } if *source == path_call.id))
        .count();
    assert_eq!(
        resolved_edge_count, 0,
        "path-call extraction slice should not emit a resolved function edge yet"
    );

    let unsupported_status_count = report
        .statuses
        .iter()
        .filter(|status| {
            matches!(
                status,
                CallResolutionStatus::Unsupported { source }
                    if *source == AnyCallSiteId::Path(path_call.id)
            )
        })
        .count();
    assert_eq!(
        unsupported_status_count, 1,
        "path calls should currently receive exactly one Unsupported status"
    );

    Ok(())
}

#[test]
fn fixture_nodes_use_imported_items_path_call_fixture_matrix() -> Result<(), SynParserError> {
    let (graph, tree) = build_tree_for_tests("fixture_nodes");
    let report = resolve_call_relations_after_tree(&graph, &tree)?;
    let function = fixture_nodes_function(&graph, &["crate", "imports"], "use_imported_items");
    let owner = CallBodyOwnerId::Function(function.id);

    let cases = [
        PathCallExpectation {
            path: &["HashMap", "new"],
            span: HASHMAP_NEW_CALL_SPAN,
            arg_count: 0,
            generic_arg_count: 2,
        },
        PathCallExpectation {
            path: &["fs", "read_to_string"],
            span: FS_READ_TO_STRING_CALL_SPAN,
            arg_count: 1,
            generic_arg_count: 0,
        },
        PathCallExpectation {
            path: &["EnumWithData", "Variant1"],
            span: ENUM_VARIANT1_CALL_SPAN,
            arg_count: 1,
            generic_arg_count: 0,
        },
        PathCallExpectation {
            path: &["Duration", "from_secs"],
            span: DURATION_FROM_SECS_CALL_SPAN,
            arg_count: 1,
            generic_arg_count: 0,
        },
        PathCallExpectation {
            path: &["Arc", "new"],
            span: ARC_NEW_CALL_SPAN,
            arg_count: 1,
            generic_arg_count: 0,
        },
        PathCallExpectation {
            path: &["TupleStruct"],
            span: TUPLE_STRUCT_CALL_SPAN,
            arg_count: 2,
            generic_arg_count: 0,
        },
    ];

    for case in &cases {
        let call = expect_unsupported_path_call(&graph, &report, owner, case);
        assert!(
            function.span.0 <= call.span.0 && call.span.1 <= function.span.1,
            "call span {:?} should be inside use_imported_items span {:?}",
            call.span,
            function.span
        );
    }

    Ok(())
}

#[test]
fn fixture_nodes_use_imported_items_records_documented_macro_call_site()
-> Result<(), SynParserError> {
    let (graph, tree) = build_tree_for_tests("fixture_nodes");
    let function = fixture_nodes_function(&graph, &["crate", "imports"], "use_imported_items");
    let owner = CallBodyOwnerId::Function(function.id);

    let matching_macro_calls = graph
        .call_sites()
        .iter()
        .filter_map(|call| match call {
            CallNode::MacroCall(macro_call)
                if macro_call.owner == owner
                    && macro_call.macro_name == "documented_macro"
                    && macro_call.span == DOCUMENTED_MACRO_CALL_SPAN =>
            {
                Some(macro_call)
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        matching_macro_calls.len(),
        1,
        "use_imported_items should record exactly one documented_macro!(...) macro call site"
    );
    let macro_call = matching_macro_calls[0];

    assert_eq!(macro_call.owner, owner);
    assert_eq!(macro_call.macro_name, "documented_macro");
    assert_eq!(macro_call.span, DOCUMENTED_MACRO_CALL_SPAN);
    assert!(
        function.span.0 <= macro_call.span.0 && macro_call.span.1 <= function.span.1,
        "macro call span {:?} should be inside use_imported_items span {:?}",
        macro_call.span,
        function.span
    );
    assert!(
        macro_call.cfgs.is_empty(),
        "fixture_nodes imports macro call should have no cfgs"
    );

    let regenerated_call_id = MacroCallSiteId::new_call_test(generate_test_call_id(
        owner,
        CallSiteKind::Macro,
        macro_call.macro_name.as_str(),
        macro_call.span,
        macro_call.cfgs.as_slice(),
    ));
    assert_eq!(
        macro_call.id, regenerated_call_id,
        "macro call site id should be deterministic from owner + macro name + span + cfgs"
    );

    let parsed_call_id = AnyCallSiteId::Macro(macro_call.id);
    let body_contains_count = graph
        .call_site_relations()
        .iter()
        .filter(|relation| {
            matches!(
                relation,
                CallSiteRelation::BodyContainsCall { source, target }
                    if *source == owner && *target == parsed_call_id
            )
        })
        .count();
    assert_eq!(
        body_contains_count, 1,
        "expected exactly one BodyContainsCall relation from use_imported_items to documented_macro!(...)"
    );

    let report = resolve_call_relations_after_tree(&graph, &tree)?;
    let unsupported_status_count = report
        .statuses
        .iter()
        .filter(|status| {
            matches!(
                status,
                CallResolutionStatus::Unsupported { source }
                    if *source == AnyCallSiteId::Macro(macro_call.id)
            )
        })
        .count();
    assert_eq!(
        unsupported_status_count, 1,
        "macro calls should currently receive exactly one Unsupported status"
    );

    Ok(())
}

#[test]
fn fixture_nodes_use_all_const_static_records_println_macro_call_site() -> Result<(), SynParserError>
{
    let (graph, tree) = build_tree_for_tests("fixture_nodes");
    let report = resolve_call_relations_after_tree(&graph, &tree)?;
    let function =
        fixture_nodes_function(&graph, &["crate", "const_static"], "use_all_const_static");
    let owner = CallBodyOwnerId::Function(function.id);

    let macro_call =
        expect_unsupported_macro_call(&graph, &report, owner, "println", PRINTLN_USED_CALL_SPAN);
    assert!(
        function.span.0 <= macro_call.span.0 && macro_call.span.1 <= function.span.1,
        "println! span {:?} should be inside use_all_const_static span {:?}",
        macro_call.span,
        function.span
    );

    Ok(())
}

#[test]
fn fixture_nodes_public_method_resolves_self_private_method_edge() -> Result<(), SynParserError> {
    let public_args = simple_struct_inherent_method_args("public_method");
    let private_args = simple_struct_inherent_method_args("private_method");

    let public_info = public_args.generate_method_pid(&*PARSED_FIXTURE_CRATE_NODES)?;
    let private_info = private_args.generate_method_pid(&*PARSED_FIXTURE_CRATE_NODES)?;
    let public_method_id = public_info.test_method_id();
    let private_method_id = private_info.test_method_id();
    let public_owner = CallBodyOwnerId::Method(public_method_id);

    let (graph, tree) = build_tree_for_tests("fixture_nodes");
    let public_owned_calls: Vec<&CallNode> = graph
        .call_sites()
        .iter()
        .filter(|call| call.owner() == public_owner)
        .collect();
    assert_eq!(
        public_owned_calls.len(),
        1,
        "resolver test requires the structural call-site fact to be present first"
    );

    let method_call = match public_owned_calls[0] {
        CallNode::MethodCall(method_call) => method_call,
        other => panic!("expected self.private_method() to be a method call, got {other:?}"),
    };
    assert_eq!(method_call.method_name, "private_method");
    assert_eq!(method_call.receiver, MethodCallReceiver::SelfValue);

    let report = resolve_call_relations_after_tree(&graph, &tree)?;
    let relation_count = report
        .relations
        .iter()
        .filter(|relation| {
            matches!(
                relation,
                CallRelation::Method { source, target }
                    if *source == method_call.id && *target == private_method_id
            )
        })
        .count();
    assert_eq!(
        relation_count, 1,
        "expected exactly one resolved method edge from self.private_method() to private_method"
    );

    let parsed_call_id = AnyCallSiteId::Method(method_call.id);
    let resolved_status_count = report
        .statuses
        .iter()
        .filter(|status| {
            matches!(
                status,
                CallResolutionStatus::Resolved { source, kind }
                    if *source == parsed_call_id && *kind == CallResolutionKind::LocalExact
            )
        })
        .count();
    assert_eq!(
        resolved_status_count, 1,
        "expected exactly one local-exact resolved status for self.private_method()"
    );

    let non_resolved_status_count = report
        .statuses
        .iter()
        .filter(|status| {
            status.source() == parsed_call_id
                && !matches!(
                    status,
                    CallResolutionStatus::Resolved {
                        kind: CallResolutionKind::LocalExact,
                        ..
                    }
                )
        })
        .count();
    assert_eq!(
        non_resolved_status_count, 0,
        "resolved call site should not also have unresolved/ambiguous/external/unsupported status"
    );

    Ok(())
}
