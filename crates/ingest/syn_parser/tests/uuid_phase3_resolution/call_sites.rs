#![cfg(feature = "typed_type_graph")]

//! Paranoid tests for parser-owned call-site records and call-resolution facts.
//!
//! These tests intentionally mirror the node-level `paranoid_test_*` style:
//! each row regenerates the expected typed `CallId`, checks exact-ID and
//! value-based lookup, checks `BodyContainsCall`, and checks the resolver status
//! plus any expected typed semantic edge.

use crate::common::call_site_paranoid::{
    ExpectedCallOutcome, ExpectedCallSite, ExpectedMethodReceiver,
};
use crate::common::{AssocOwner, AssocParanoidArgs, PARSED_FIXTURE_CRATE_NODES};
use crate::paranoid_call_site_test;

const IMPLS_RS: &str = "src/impls.rs";
const SIMPLE_STRUCT_IMPL_SPAN: (usize, usize) = (520, 750);
const PRIVATE_STRUCT_IMPL_SPAN: (usize, usize) = (790, 884);
const SELF_PRIVATE_METHOD_CALL_SPAN: (usize, usize) = (721, 742);
const SELF_SECRET_LEN_CALL_SPAN: (usize, usize) = (859, 876);
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

fn private_struct_inherent_method_args(ident: &'static str) -> AssocParanoidArgs<'static> {
    AssocParanoidArgs {
        fixture: "fixture_nodes",
        relative_file_path: IMPLS_RS,
        expected_path: &["crate", "impls"],
        owner: AssocOwner::Impl {
            span: PRIVATE_STRUCT_IMPL_SPAN,
        },
        ident,
        expected_cfg: None,
    }
}

paranoid_call_site_test!(
    fixture_nodes_public_method_records_and_resolves_self_private_method_call_site,
    fixture: "fixture_nodes",
    owner: method {
        args: simple_struct_inherent_method_args("public_method")
    },
    expected: {
        let private_args = simple_struct_inherent_method_args("private_method");
        let private_info = private_args.generate_method_pid(&*PARSED_FIXTURE_CRATE_NODES)?;
        ExpectedCallSite::method(
            "private_method",
            ExpectedMethodReceiver::SelfValue,
            SELF_PRIVATE_METHOD_CALL_SPAN,
            0,
            0,
            &[],
            ExpectedCallOutcome::ResolvedMethodLocalExact {
                target: private_info.test_method_id(),
            },
        )
    },
);

paranoid_call_site_test!(
    fixture_nodes_get_secret_len_records_self_field_len_method_call_site,
    fixture: "fixture_nodes",
    owner: method {
        args: private_struct_inherent_method_args("get_secret_len")
    },
    expected: ExpectedCallSite::method(
        "len",
        ExpectedMethodReceiver::SelfField {
            field_path: &["secret"]
        },
        SELF_SECRET_LEN_CALL_SPAN,
        0,
        0,
        &[],
        ExpectedCallOutcome::Unsupported,
    ),
);

paranoid_call_site_test!(
    fixture_nodes_use_imported_items_records_hashmap_new_path_call_site,
    fixture: "fixture_nodes",
    owner: function {
        module_path: &["crate", "imports"],
        name: "use_imported_items"
    },
    expected: ExpectedCallSite::path(
        &["HashMap", "new"],
        HASHMAP_NEW_CALL_SPAN,
        0,
        2,
        &[],
        ExpectedCallOutcome::Unsupported,
    ),
);

paranoid_call_site_test!(
    fixture_nodes_use_imported_items_records_fs_read_to_string_path_call_site,
    fixture: "fixture_nodes",
    owner: function {
        module_path: &["crate", "imports"],
        name: "use_imported_items"
    },
    expected: ExpectedCallSite::path(
        &["fs", "read_to_string"],
        FS_READ_TO_STRING_CALL_SPAN,
        1,
        0,
        &[],
        ExpectedCallOutcome::Unsupported,
    ),
);

paranoid_call_site_test!(
    fixture_nodes_use_imported_items_records_pathbuf_new_path_call_site,
    fixture: "fixture_nodes",
    owner: function {
        module_path: &["crate", "imports"],
        name: "use_imported_items"
    },
    expected: ExpectedCallSite::path(
        &["PathBuf", "new"],
        PATHBUF_NEW_CALL_SPAN,
        0,
        0,
        &[],
        ExpectedCallOutcome::Unsupported,
    ),
);

paranoid_call_site_test!(
    fixture_nodes_use_imported_items_records_enum_variant1_path_call_site,
    fixture: "fixture_nodes",
    owner: function {
        module_path: &["crate", "imports"],
        name: "use_imported_items"
    },
    expected: ExpectedCallSite::path(
        &["EnumWithData", "Variant1"],
        ENUM_VARIANT1_CALL_SPAN,
        1,
        0,
        &[],
        ExpectedCallOutcome::Unsupported,
    ),
);

paranoid_call_site_test!(
    fixture_nodes_use_imported_items_records_duration_from_secs_path_call_site,
    fixture: "fixture_nodes",
    owner: function {
        module_path: &["crate", "imports"],
        name: "use_imported_items"
    },
    expected: ExpectedCallSite::path(
        &["Duration", "from_secs"],
        DURATION_FROM_SECS_CALL_SPAN,
        1,
        0,
        &[],
        ExpectedCallOutcome::Unsupported,
    ),
);

paranoid_call_site_test!(
    fixture_nodes_use_imported_items_records_arc_new_path_call_site,
    fixture: "fixture_nodes",
    owner: function {
        module_path: &["crate", "imports"],
        name: "use_imported_items"
    },
    expected: ExpectedCallSite::path(
        &["Arc", "new"],
        ARC_NEW_CALL_SPAN,
        1,
        0,
        &[],
        ExpectedCallOutcome::Unsupported,
    ),
);

paranoid_call_site_test!(
    fixture_nodes_use_imported_items_records_tuple_struct_path_call_site,
    fixture: "fixture_nodes",
    owner: function {
        module_path: &["crate", "imports"],
        name: "use_imported_items"
    },
    expected: ExpectedCallSite::path(
        &["TupleStruct"],
        TUPLE_STRUCT_CALL_SPAN,
        2,
        0,
        &[],
        ExpectedCallOutcome::Unsupported,
    ),
);

paranoid_call_site_test!(
    fixture_nodes_use_imported_items_records_documented_macro_call_site,
    fixture: "fixture_nodes",
    owner: function {
        module_path: &["crate", "imports"],
        name: "use_imported_items"
    },
    expected: ExpectedCallSite::macro_call(
        "documented_macro",
        DOCUMENTED_MACRO_CALL_SPAN,
        &[],
        ExpectedCallOutcome::Unsupported,
    ),
);

paranoid_call_site_test!(
    fixture_nodes_use_all_const_static_records_println_macro_call_site,
    fixture: "fixture_nodes",
    owner: function {
        module_path: &["crate", "const_static"],
        name: "use_all_const_static"
    },
    expected: ExpectedCallSite::macro_call(
        "println",
        PRINTLN_USED_CALL_SPAN,
        &[],
        ExpectedCallOutcome::Unsupported,
    ),
);
