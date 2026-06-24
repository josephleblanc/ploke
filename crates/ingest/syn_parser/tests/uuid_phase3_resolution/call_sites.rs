//! Paranoid tests for parser-owned call-site records and call-resolution facts.
//!
//! These tests intentionally mirror the node-level `paranoid_test_*` style:
//! each row regenerates the expected typed `CallId`, checks exact-ID and
//! value-based lookup, checks `BodyContainsCall`, and checks the resolver status
//! plus any expected typed semantic edge.

use crate::common::call_site_paranoid::{
    CallOwnerContext, ExpectedCallOutcome, ExpectedCallSite, ExpectedMethodReceiver,
};
use crate::common::{
    AssocOwner, AssocParanoidArgs, PARSED_FIXTURE_CRATE_EDGE_CASES, PARSED_FIXTURE_CRATE_IMPLS,
    PARSED_FIXTURE_CRATE_NODES, ParanoidArgs,
};
use crate::paranoid_call_site_test;
use ploke_core::ItemKind;
use syn_parser::parser::graph::GraphAccess;
use syn_parser::parser::nodes::{FunctionNodeId, StructNodeId, TypeDefNode, VariantNodeId};

const IMPLS_RS: &str = "src/impls.rs";
const CONST_STATIC_RS: &str = "src/const_static.rs";
const STRUCTS_RS: &str = "src/structs.rs";
const PATH_RESOLUTION_LIB_RS: &str = "src/lib.rs";
const CALL_GRAPH_LIB_RS: &str = "src/lib.rs";
const EDGE_CASES_LIB_RS: &str = "src/lib.rs";
const GENERICS_LIB_RS: &str = "src/lib.rs";
const TYPE_RESOLUTION_V2_LIB_RS: &str = "src/lib.rs";
const FIXTURE_IMPLS_MAIN_RS: &str = "src/main.rs";
const FIXTURE_IMPLS_FILE_MODULE_RS: &str = "src/impl_in_file_module.rs";
const SIMPLE_STRUCT_IMPL_SPAN: (usize, usize) = (520, 750);
const PRIVATE_STRUCT_IMPL_SPAN: (usize, usize) = (790, 884);
const GENERIC_STR_IMPL_SPAN: (usize, usize) = (1251, 1354);
const GENERIC_SIMPLE_TRAIT_IMPL_SPAN: (usize, usize) = (1732, 1885);
const EDGE_CASES_GENERIC_ITEM_IMPL_SPAN: (usize, usize) = (807, 1146);
const EDGE_CASES_PROCESSOR_IMPL_SPAN: (usize, usize) = (1148, 1514);
const EDGE_CASES_HELPER_IMPL_SPAN: (usize, usize) = (2014, 2071);
const GENERICS_TRAIT_IMPL_SPAN: (usize, usize) = (1106, 1485);
const TYPE_RESOLUTION_V2_ASSOC_GENERIC_IMPL_SPAN: (usize, usize) = (1273, 1446);
const FIXTURE_IMPLS_FUNC_ONE_IMPL_SPAN: (usize, usize) = (431, 507);
const FIXTURE_IMPLS_FUNC_TWO_IMPL_SPAN: (usize, usize) = (509, 585);
const FIXTURE_IMPLS_FUNC_THREE_IMPL_SPAN: (usize, usize) = (647, 752);
const FIXTURE_IMPLS_FILE_MODULE_IMPL_SPAN: (usize, usize) = (28, 162);
const SELF_PRIVATE_METHOD_CALL_SPAN: (usize, usize) = (721, 742);
const SELF_SECRET_LEN_CALL_SPAN: (usize, usize) = (859, 876);
const SELF_VALUE_LEN_CALL_SPAN: (usize, usize) = (1330, 1346);
const SELF_VALUE_INTO_CALL_SPAN: (usize, usize) = (1860, 1877);
const FIXTURE_IMPLS_FUNC_ONE_CALL_SPAN: (usize, usize) = (152, 169);
const FIXTURE_IMPLS_FUNC_TWO_CALL_SPAN: (usize, usize) = (175, 192);
const FIXTURE_IMPLS_FUNC_THREE_CALL_SPAN: (usize, usize) = (198, 217);
const FIXTURE_IMPLS_FUNC_FOUR_CALL_SPAN: (usize, usize) = (223, 255);
const FIXTURE_IMPLS_FUNC_FIVE_CALL_SPAN: (usize, usize) = (261, 279);
const FIXTURE_IMPLS_PRINTLN_CALL_SPAN: (usize, usize) = (286, 352);
const HASHMAP_NEW_CALL_SPAN: (usize, usize) = (3413, 3442);
const FS_READ_TO_STRING_CALL_SPAN: (usize, usize) = (3838, 3865);
const PATHBUF_NEW_CALL_SPAN: (usize, usize) = (3930, 3944);
const ENUM_VARIANT1_CALL_SPAN: (usize, usize) = (4007, 4032);
const ALIAS_CHECKER_CALL_SPAN: (usize, usize) = (4316, 4343);
const DOCUMENTED_MACRO_CALL_SPAN: (usize, usize) = (4894, 4935);
const DURATION_FROM_SECS_CALL_SPAN: (usize, usize) = (5235, 5257);
const ARC_NEW_CALL_SPAN: (usize, usize) = (5452, 5463);
const TUPLE_STRUCT_CALL_SPAN: (usize, usize) = (5549, 5566);
const PRINTLN_USED_CALL_SPAN: (usize, usize) = (4395, 4461);
const SUPER_RESTRICTED_FUNC_CALL_SPAN: (usize, usize) = (1936, 1960);
const ROOT_INFO_MACRO_CALL_SPAN: (usize, usize) = (3866, 3892);
const REGEX_NEW_CALL_SPAN: (usize, usize) = (4005, 4027);
const REGEX_UNWRAP_CALL_SPAN: (usize, usize) = (4005, 4036);
const TYPEID_SYNTHETIC_CALL_SPAN: (usize, usize) = (4104, 4408);
const NODEID_GENERATE_SYNTHETIC_CALL_SPAN: (usize, usize) = (4135, 4377);
const UUID_NIL_CALL_SPAN: (usize, usize) = (4179, 4196);
const NODEID_UUID_CALL_SPAN: (usize, usize) = (4135, 4397);
const STD_PATH_NEW_CALL_SPAN: (usize, usize) = (4214, 4238);
const ROOT_DEBUG_MACRO_CALL_SPAN: (usize, usize) = (4456, 4484);
const LOCAL_MACRO_CALL_SPAN: (usize, usize) = (437, 457);
const FIXTURE_MACROS_PRINTLN_CALL_SPAN: (usize, usize) = (463, 485);
const DYNAMIC_CLOSURE_BINDING_CALL_SPAN: (usize, usize) = (177, 188);
const DYNAMIC_CLOSURE_LITERAL_CALL_SPAN: (usize, usize) = (213, 222);
const CRATE_LOCAL_TARGET_CALL_SPAN: (usize, usize) = (345, 366);
const SELF_NESTED_TARGET_CALL_SPAN: (usize, usize) = (497, 518);
const UNQUALIFIED_LOCAL_TARGET_CALL_SPAN: (usize, usize) = (580, 594);
const MAKE_FN_INNER_CALL_SPAN: (usize, usize) = (697, 706);
const RETURNED_FUNCTION_DYNAMIC_CALL_SPAN: (usize, usize) = (697, 708);
const LOCAL_ASSOC_IMPL_SPAN: (usize, usize) = (736, 868);
const SELF_ASSOC_MAKE_CALL_SPAN: (usize, usize) = (848, 860);
const LOCAL_ASSOC_MAKE_CALL_SPAN: (usize, usize) = (921, 939);
const QUALIFIED_LOCAL_ASSOC_MAKE_CALL_SPAN: (usize, usize) = (1004, 1024);
const EXPLICIT_DROP_IMPL_SPAN: (usize, usize) = (16418, 16493);
const IMPORTED_ALIAS_CALL_SPAN: (usize, usize) = (1393, 1409);
const GLOBBED_TARGET_CALL_SPAN: (usize, usize) = (1461, 1477);
const REEXPORTED_TARGET_CALL_SPAN: (usize, usize) = (1526, 1545);
const MODULE_ALIAS_TARGET_CALL_SPAN: (usize, usize) = (1599, 1630);
const INSTANCE_IMPL_SPAN: (usize, usize) = (1634, 1712);
const INSTANCE_CALL_SPAN: (usize, usize) = (1780, 1802);
const TYPED_LOCAL_INSTANCE_CALL_SPAN: (usize, usize) = (1901, 1923);
const INITIALIZED_LOCAL_INSTANCE_CALL_SPAN: (usize, usize) = (2016, 2038);
const ASSOC_CONST_CARRIER_IMPL_SPAN: (usize, usize) = (2125, 2210);
const IMPL_ASSOC_CONST_CALL_SPAN: (usize, usize) = (2188, 2207);
const TRAIT_ASSOC_CONST_CALL_SPAN: (usize, usize) = (2280, 2299);
const LOCAL_DISPATCH_TRAIT_IMPL_SPAN: (usize, usize) = (2405, 2509);
const PARAM_TRAIT_METHOD_CALL_SPAN: (usize, usize) = (2583, 2602);
const TYPED_LOCAL_TRAIT_METHOD_CALL_SPAN: (usize, usize) = (2716, 2735);
const INITIALIZED_LOCAL_TRAIT_METHOD_CALL_SPAN: (usize, usize) = (2834, 2853);
const PARENTHESIZED_LOCAL_TARGET_DYNAMIC_CALL_SPAN: (usize, usize) = (2911, 2927);
const CLOSURE_BODY_LOCAL_TARGET_CALL_SPAN: (usize, usize) = (3011, 3025);
const ASYNC_BLOCK_LOCAL_TARGET_CALL_SPAN: (usize, usize) = (3129, 3143);
const TRAIT_ASSOC_FUNCTION_CALL_SPAN: (usize, usize) = (3394, 3461);
const SHADOWED_LOCAL_TARGET_BINDING_CALL_SPAN: (usize, usize) = (3553, 3567);
const LOCAL_FUNCTION_ITEM_BINDING_CALL_SPAN: (usize, usize) = (3652, 3655);
const TYPED_FUNCTION_POINTER_BINDING_CALL_SPAN: (usize, usize) = (3756, 3759);
const TYPED_FUNCTION_POINTER_ALIAS_BINDING_CALL_SPAN: (usize, usize) = (9123, 9126);
const PARENTHESIZED_TYPED_FUNCTION_POINTER_ALIAS_BINDING_CALL_SPAN: (usize, usize) = (9275, 9280);
const GENERIC_FN_ONCE_VALUE_BINDING_CALL_SPAN: (usize, usize) = (9386, 9397);
const BOXED_DYN_FN_BOX_NEW_CALL_SPAN: (usize, usize) = (9492, 9514);
const BOXED_DYN_FN_VALUE_BINDING_CALL_SPAN: (usize, usize) = (9520, 9530);
const GENERIC_IDENTITY_TURBOFISH_CALL_SPAN: (usize, usize) = (9645, 9673);
const SUPER_LOCAL_TARGET_CALL_SPAN: (usize, usize) = (9758, 9779);
const RAW_IDENTIFIER_FUNCTION_CALL_SPAN: (usize, usize) = (9878, 9887);
const RAW_METHOD_IMPL_SPAN: (usize, usize) = (9920, 9997);
const RAW_IDENTIFIER_METHOD_CALL_SPAN: (usize, usize) = (10098, 10112);
const GENERIC_METHOD_IMPL_SPAN: (usize, usize) = (10149, 10252);
const METHOD_TURBOFISH_CALL_SPAN: (usize, usize) = (10356, 10390);
const PRELUDE_DROP_CALL_SPAN: (usize, usize) = (10452, 10463);
const CRATE_SCOPED_MACRO_CALL_SPAN: (usize, usize) = (10598, 10626);
const LOCAL_DROP_SHADOW_CALL_SPAN: (usize, usize) = (10772, 10779);
const BORROWED_LOCAL_INSTANCE_CALL_SPAN: (usize, usize) = (10893, 10918);
const DEREFERENCED_LOCAL_INSTANCE_CALL_SPAN: (usize, usize) = (11013, 11038);
const PRELUDE_STRING_NEW_CALL_SPAN: (usize, usize) = (11091, 11104);
const PRELUDE_VEC_NEW_CALL_SPAN: (usize, usize) = (11156, 11166);
const PATH_RESULT_INSTANCE_CALL_SPAN: (usize, usize) = (11371, 11406);
const METHOD_RESULT_INSTANCE_CALL_SPAN: (usize, usize) = (11507, 11543);
const TUPLE_FIELD_INSTANCE_CALL_SPAN: (usize, usize) = (11710, 11734);
const TUPLE_FIELD_FUNCTION_CALL_SPAN: (usize, usize) = (11885, 11894);
const AWAIT_RESULT_INSTANCE_CALL_SPAN: (usize, usize) = (12032, 12079);
const TRY_RESULT_INSTANCE_CALL_SPAN: (usize, usize) = (12227, 12262);
const LITERAL_TO_STRING_CALL_SPAN: (usize, usize) = (12319, 12340);
const TYPED_VEC_LEN_CALL_SPAN: (usize, usize) = (12434, 12445);
const LOCAL_VEC_IMPL_SPAN: (usize, usize) = (12505, 12582);
const SHADOWED_TYPED_VEC_LEN_CALL_SPAN: (usize, usize) = (12674, 12685);
const FUNCTION_POINTER_CAST_PATH_DYNAMIC_CALL_SPAN: (usize, usize) = (12749, 12780);
const FUNCTION_POINTER_CAST_BINDING_DYNAMIC_CALL_SPAN: (usize, usize) = (12880, 12900);
const DEREFERENCED_FUNCTION_POINTER_BINDING_DYNAMIC_CALL_SPAN: (usize, usize) = (13008, 13014);
const BLOCK_FUNCTION_ITEM_DYNAMIC_CALL_SPAN: (usize, usize) = (13065, 13085);
const IF_SAME_FUNCTION_ITEM_DYNAMIC_CALL_SPAN: (usize, usize) = (13188, 13238);
const IF_AMBIGUOUS_FUNCTION_ITEM_DYNAMIC_CALL_SPAN: (usize, usize) = (13306, 13356);
const MATCH_SAME_FUNCTION_ITEM_DYNAMIC_CALL_SPAN: (usize, usize) = (13422, 13505);
const MATCH_AMBIGUOUS_FUNCTION_ITEM_DYNAMIC_CALL_SPAN: (usize, usize) = (13576, 13659);
const MATCH_GUARDED_FUNCTION_ITEM_DYNAMIC_CALL_SPAN: (usize, usize) = (13728, 13815);
const IF_CLOSURE_BRANCH_DYNAMIC_CALL_SPAN: (usize, usize) = (13874, 13916);
const MATCH_CLOSURE_ARM_DYNAMIC_CALL_SPAN: (usize, usize) = (13975, 14051);
const FUNCTION_POINTER_PARAM_CALL_SPAN: (usize, usize) = (14119, 14122);
const PARENTHESIZED_FUNCTION_POINTER_PARAM_DYNAMIC_CALL_SPAN: (usize, usize) = (14204, 14209);
const FUNCTION_POINTER_PARAM_CAST_DYNAMIC_CALL_SPAN: (usize, usize) = (14282, 14302);
const CLOSURE_BINDING_CAST_DYNAMIC_CALL_SPAN: (usize, usize) = (14379, 14405);
const DEREFERENCED_CLOSURE_BINDING_DYNAMIC_CALL_SPAN: (usize, usize) = (14490, 14502);
const FIELD_FUNCTION_PARAM_DYNAMIC_CALL_SPAN: (usize, usize) = (14638, 14657);
const INDEXED_FUNCTION_POINTER_DYNAMIC_CALL_SPAN: (usize, usize) = (14736, 14746);
const MOVE_CLOSURE_LITERAL_DYNAMIC_CALL_SPAN: (usize, usize) = (14813, 14839);
const MOVE_CLOSURE_BODY_LOCAL_TARGET_CALL_SPAN: (usize, usize) = (14822, 14836);
const IF_FUNCTION_POINTER_PARAM_BRANCH_DYNAMIC_CALL_SPAN: (usize, usize) = (14929, 14957);
const MATCH_FUNCTION_POINTER_PARAM_ARM_DYNAMIC_CALL_SPAN: (usize, usize) = (15047, 15108);
const IF_NESTED_BRANCH_EXPRESSION_DYNAMIC_CALL_SPAN: (usize, usize) = (15177, 15285);
const MATCH_NESTED_ARM_EXPRESSION_DYNAMIC_CALL_SPAN: (usize, usize) = (15354, 15516);
const CHAINED_RETURNED_FUNCTION_INNER_CALL_SPAN: (usize, usize) = (15690, 15705);
const CHAINED_RETURNED_FUNCTION_DYNAMIC_CALL_SPAN: (usize, usize) = (15690, 15708);
const VEC_MACRO_CALL_SPAN: (usize, usize) = (15754, 15767);
const UNSAFE_TARGET_CALL_SPAN: (usize, usize) = (15872, 15887);
const NEW_TYPE_CONSTRUCTOR_CALL_SPAN: (usize, usize) = (15985, 15999);
const TRAIT_DEFAULT_REQUIRED_METHOD_CALL_SPAN: (usize, usize) = (16121, 16136);
const IMPORTED_MACRO_ALIAS_CALL_SPAN: (usize, usize) = (16359, 16382);
const EXPLICIT_DROP_METHOD_CALL_SPAN: (usize, usize) = (16579, 16591);
const ITEM_MACRO_INSIDE_BODY_CALL_SPAN: (usize, usize) = (16775, 16799);
const PARENTHESIZED_GENERIC_FN_ONCE_DYNAMIC_CALL_SPAN: (usize, usize) = (16926, 16939);
const PARENTHESIZED_BOXED_DYN_FN_BOX_NEW_CALL_SPAN: (usize, usize) = (17048, 17070);
const PARENTHESIZED_BOXED_DYN_FN_DYNAMIC_CALL_SPAN: (usize, usize) = (17076, 17088);
const EXTERN_C_ABS_CALL_SPAN: (usize, usize) = (17230, 17240);
const TEST_ASSERT_EQ_MACRO_CALL_SPAN: (usize, usize) = (17334, 17354);
const ASYNC_CLOSURE_LITERAL_DYNAMIC_CALL_SPAN: (usize, usize) = (17436, 17463);
const ASYNC_CLOSURE_BODY_LOCAL_TARGET_CALL_SPAN: (usize, usize) = (17446, 17460);
const NAMED_FIELD_FUNCTION_DYNAMIC_CALL_SPAN: (usize, usize) = (17802, 17821);
const ALIASED_NAMED_FIELD_FUNCTION_DYNAMIC_CALL_SPAN: (usize, usize) = (17991, 18009);
const INDEXED_FIELD_FUNCTION_PARAM_DYNAMIC_CALL_SPAN: (usize, usize) = (18096, 18117);
const INDEXED_NAMED_FIELD_FUNCTION_DYNAMIC_CALL_SPAN: (usize, usize) = (18266, 18287);
const INDEXED_NAMED_FIELD_ARRAY_ALIAS_DYNAMIC_CALL_SPAN: (usize, usize) = (18449, 18470);
const ALIASED_INDEXED_NAMED_FIELD_DYNAMIC_CALL_SPAN: (usize, usize) = (18651, 18671);
const INDEXED_TUPLE_FIELD_FUNCTION_PARAM_DYNAMIC_CALL_SPAN: (usize, usize) = (18769, 18782);
const INDEXED_TUPLE_FIELD_FUNCTION_DYNAMIC_CALL_SPAN: (usize, usize) = (18909, 18922);
const INDEXED_TUPLE_FIELD_ARRAY_ALIAS_DYNAMIC_CALL_SPAN: (usize, usize) = (19075, 19088);
const ALIASED_INDEXED_TUPLE_FIELD_DYNAMIC_CALL_SPAN: (usize, usize) = (19247, 19259);
const INDEXED_INITIALIZED_FUNCTION_DYNAMIC_CALL_SPAN: (usize, usize) = (19357, 19367);
const TYPED_INDEXED_INITIALIZED_FUNCTION_DYNAMIC_CALL_SPAN: (usize, usize) = (19489, 19499);
const ALIASED_INDEXED_INITIALIZED_FUNCTION_DYNAMIC_CALL_SPAN: (usize, usize) = (19628, 19638);
const BLANKET_TRAIT_IMPL_SPAN: (usize, usize) = (20056, 20149);
const BLANKET_TRAIT_METHOD_CALL_SPAN: (usize, usize) = (20227, 20248);
const TRAIT_DEFAULT_ASSOC_FUNCTION_CALL_SPAN: (usize, usize) = (20435, 20457);
const TYPE_ALIAS_ASSOC_MAKE_CALL_SPAN: (usize, usize) = (20576, 20603);
const TYPE_ALIAS_CHAIN_ASSOC_MAKE_CALL_SPAN: (usize, usize) = (20733, 20761);
const TYPE_ALIAS_CHAIN_INSTANCE_CALL_SPAN: (usize, usize) = (20875, 20897);
const IMPORTED_TYPE_ALIAS_ASSOC_MAKE_CALL_SPAN: (usize, usize) = (21133, 21164);
const IMPORTED_TYPE_ALIAS_INSTANCE_CALL_SPAN: (usize, usize) = (21284, 21306);
const INLINE_BOUND_BLANKET_TRAIT_IMPL_SPAN: (usize, usize) = (21465, 21580);
const INLINE_BOUND_BLANKET_TRAIT_METHOD_CALL_SPAN: (usize, usize) = (21671, 21697);
const WHERE_BOUND_BLANKET_TRAIT_IMPL_SPAN: (usize, usize) = (21779, 21905);
const WHERE_BOUND_BLANKET_TRAIT_METHOD_CALL_SPAN: (usize, usize) = (21995, 22020);
const DEREFERENCED_PARAM_INSTANCE_CALL_SPAN: (usize, usize) = (22104, 22129);
const BORROWED_PARAM_INSTANCE_CALL_SPAN: (usize, usize) = (22209, 22231);
const REFERENCED_LOCAL_INSTANCE_CALL_SPAN: (usize, usize) = (22324, 22346);
const TYPED_REFERENCE_LOCAL_INSTANCE_CALL_SPAN: (usize, usize) = (22457, 22479);
const LOCAL_TRAIT_OBJECT_BINDING_METHOD_CALL_SPAN: (usize, usize) = (22620, 22639);
const TRAIT_IMPL_BODY_CALL_IMPL_SPAN: (usize, usize) = (22765, 22964);
const TRAIT_IMPL_BODY_SELF_METHOD_CALL_SPAN: (usize, usize) = (22931, 22956);
const CONCRETE_TRAIT_OBJECT_BINDING_METHOD_CALL_SPAN: (usize, usize) = (23093, 23112);
const ALIASED_CONCRETE_TRAIT_OBJECT_BINDING_METHOD_CALL_SPAN: (usize, usize) = (23276, 23295);
const TRANSITIVE_BOUND_BLANKET_TRAIT_IMPL_SPAN: (usize, usize) = (23575, 23708);
const TRANSITIVE_BOUND_BLANKET_TRAIT_METHOD_CALL_SPAN: (usize, usize) = (23803, 23833);
const IMPORTED_FUNCTION_ITEM_BINDING_CALL_SPAN: (usize, usize) = (23923, 23926);
const REFERENCE_ALIAS_TRAIT_OBJECT_BINDING_METHOD_CALL_SPAN: (usize, usize) = (24112, 24131);
const REFERENCE_CHAIN_TRAIT_OBJECT_BINDING_METHOD_CALL_SPAN: (usize, usize) = (24342, 24361);
const CONSTRAINED_GENERIC_SELF_TRAIT_IMPL_SPAN: (usize, usize) = (25164, 25318);
const EDGE_CASES_T_DEFAULT_CALL_SPAN: (usize, usize) = (1073, 1085);
const EDGE_CASES_FORMAT_MACRO_CALL_SPAN: (usize, usize) = (1447, 1506);
const EDGE_CASES_VISIBILITY_CFG: &str = "not (feature = \"type_bearing_ids\")";
const EDGE_CASES_INTERNAL_HELPER_CALL_SPAN: (usize, usize) = (2634, 2658);
const EDGE_CASES_SUPER_HELPER_CALL_SPAN: (usize, usize) = (2674, 2695);
const EDGE_CASES_RESTRICTED_FUNC_CALL_SPAN: (usize, usize) = (2711, 2740);
const EDGE_CASES_HELPER_HELP_CALL_SPAN: (usize, usize) = (3205, 3213);
const EDGE_CASES_UTILITY_HELPER_HELP_CALL_SPAN: (usize, usize) = (3247, 3256);
const EDGE_CASES_HELLO_TO_STRING_CALL_SPAN: (usize, usize) = (3366, 3385);
const GENERICS_T_DEFAULT_CALL_SPAN: (usize, usize) = (662, 674);
const GENERICS_FORMAT_MACRO_CALL_SPAN: (usize, usize) = (1361, 1477);
const TYPE_RESOLUTION_V2_ASSOC_CONST_PANIC_CALL_SPAN: (usize, usize) = (1392, 1443);
const AMBIGUOUS_TRAIT_METHOD_CALL_SPAN: (usize, usize) = (4200, 4215);
const INHERENT_PRECEDENCE_IMPL_SPAN: (usize, usize) = (4327, 4413);
const INHERENT_PRECEDENCE_METHOD_CALL_SPAN: (usize, usize) = (4649, 4665);
const INLINE_GENERIC_BOUND_METHOD_CALL_SPAN: (usize, usize) = (4821, 4840);
const WHERE_GENERIC_BOUND_METHOD_CALL_SPAN: (usize, usize) = (4941, 4960);
const TRAIT_OBJECT_METHOD_CALL_SPAN: (usize, usize) = (5040, 5059);
const TRAIT_DEFAULT_LOCAL_TARGET_CALL_SPAN: (usize, usize) = (5143, 5157);
const PARENTHESIZED_FUNCTION_ITEM_BINDING_CALL_SPAN: (usize, usize) = (5256, 5261);
const ALIASED_FUNCTION_ITEM_BINDING_CALL_SPAN: (usize, usize) = (5363, 5366);
const PARENTHESIZED_ALIASED_FUNCTION_ITEM_BINDING_CALL_SPAN: (usize, usize) = (5482, 5487);
const IMPL_TRAIT_METHOD_CALL_SPAN: (usize, usize) = (5565, 5584);
const SCOPED_TRAIT_IMPL_SPAN: (usize, usize) = (5762, 5882);
const DIRECT_IMPORTED_TRAIT_METHOD_CALL_SPAN: (usize, usize) = (6092, 6112);
const ALIAS_IMPORTED_TRAIT_METHOD_CALL_SPAN: (usize, usize) = (6352, 6372);
const GLOB_IMPORTED_TRAIT_METHOD_CALL_SPAN: (usize, usize) = (6584, 6604);
const UNIMPORTED_TRAIT_METHOD_CALL_SPAN: (usize, usize) = (6787, 6807);
const REEXPORTED_TRAIT_METHOD_CALL_SPAN: (usize, usize) = (19919, 19939);
const GROUPED_IMPORTED_TRAIT_METHOD_CALL_SPAN: (usize, usize) = (24562, 24582);
const CONSTRAINED_GENERIC_SELF_TRAIT_METHOD_CALL_SPAN: (usize, usize) = (25425, 25463);
const PARENTHESIZED_TYPED_LOCAL_INSTANCE_CALL_SPAN: (usize, usize) = (6936, 6960);
const PARENTHESIZED_INITIALIZED_LOCAL_TRAIT_CALL_SPAN: (usize, usize) = (7073, 7094);
const IMPORTED_ASSOC_IMPL_SPAN: (usize, usize) = (7164, 7249);
const IMPORTED_TYPE_ASSOC_MAKE_CALL_SPAN: (usize, usize) = (7477, 7503);
const GLOB_IMPORTED_TYPE_ASSOC_MAKE_CALL_SPAN: (usize, usize) = (7574, 7595);
const REEXPORTED_TYPE_ASSOC_MAKE_CALL_SPAN: (usize, usize) = (7665, 7688);
const DIRECT_IMPORTED_TRAIT_ASSOC_FUNCTION_CALL_SPAN: (usize, usize) = (8087, 8136);
const ALIAS_IMPORTED_TRAIT_ASSOC_FUNCTION_CALL_SPAN: (usize, usize) = (8382, 8430);
const GLOB_IMPORTED_TRAIT_ASSOC_FUNCTION_CALL_SPAN: (usize, usize) = (8620, 8669);
const REEXPORTED_TRAIT_ASSOC_FUNCTION_CALL_SPAN: (usize, usize) = (24853, 24904);
const CRATE_MODULE_NESTED_TARGET_CALL_SPAN: (usize, usize) = (8743, 8776);
const SELF_MODULE_NESTED_TARGET_CALL_SPAN: (usize, usize) = (8833, 8865);
const METHOD_AS_ASSOCIATED_FUNCTION_CALL_SPAN: (usize, usize) = (8954, 8988);
const FN_CALL_CONST_FIVE_CALL_SPAN: (usize, usize) = (1631, 1637);
const STATIC_FN_CALL_FIVE_CALL_SPAN: (usize, usize) = (4495, 4501);

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

fn generic_str_inherent_method_args(ident: &'static str) -> AssocParanoidArgs<'static> {
    AssocParanoidArgs {
        fixture: "fixture_nodes",
        relative_file_path: IMPLS_RS,
        expected_path: &["crate", "impls"],
        owner: AssocOwner::Impl {
            span: GENERIC_STR_IMPL_SPAN,
        },
        ident,
        expected_cfg: None,
    }
}

fn generic_simple_trait_method_args(ident: &'static str) -> AssocParanoidArgs<'static> {
    AssocParanoidArgs {
        fixture: "fixture_nodes",
        relative_file_path: IMPLS_RS,
        expected_path: &["crate", "impls"],
        owner: AssocOwner::Impl {
            span: GENERIC_SIMPLE_TRAIT_IMPL_SPAN,
        },
        ident,
        expected_cfg: None,
    }
}

fn fixture_nodes_struct_args(ident: &'static str) -> ParanoidArgs<'static> {
    ParanoidArgs {
        fixture: "fixture_nodes",
        relative_file_path: STRUCTS_RS,
        expected_path: &["crate", "structs"],
        ident,
        item_kind: ItemKind::Struct,
        expected_cfg: None,
    }
}

fn fixture_nodes_enum_variant_id(enum_name: &str, variant_name: &str) -> VariantNodeId {
    let mut matches = PARSED_FIXTURE_CRATE_NODES
        .iter()
        .flat_map(|parsed| parsed.graph.defined_types())
        .filter_map(|type_def| match type_def {
            TypeDefNode::Enum(enum_node) => Some(enum_node),
            _ => None,
        })
        .filter(|enum_node| enum_node.name == enum_name)
        .flat_map(|enum_node| enum_node.variants.iter())
        .filter(|variant| variant.name == variant_name)
        .map(|variant| variant.id)
        .collect::<Vec<_>>();
    matches.sort_unstable();
    matches.dedup();
    match matches.as_slice() {
        [id] => *id,
        [] => panic!("expected variant {enum_name}::{variant_name} in fixture_nodes"),
        many => panic!(
            "expected exactly one variant {enum_name}::{variant_name}, found {}",
            many.len()
        ),
    }
}

fn fixture_nodes_const_args(ident: &'static str) -> ParanoidArgs<'static> {
    ParanoidArgs {
        fixture: "fixture_nodes",
        relative_file_path: CONST_STATIC_RS,
        expected_path: &["crate", "const_static"],
        ident,
        item_kind: ItemKind::Const,
        expected_cfg: None,
    }
}

fn fixture_nodes_static_args(ident: &'static str) -> ParanoidArgs<'static> {
    ParanoidArgs {
        fixture: "fixture_nodes",
        relative_file_path: CONST_STATIC_RS,
        expected_path: &["crate", "const_static"],
        ident,
        item_kind: ItemKind::Static,
        expected_cfg: None,
    }
}

fn fixture_nodes_const_static_function_args(ident: &'static str) -> ParanoidArgs<'static> {
    ParanoidArgs {
        fixture: "fixture_nodes",
        relative_file_path: CONST_STATIC_RS,
        expected_path: &["crate", "const_static"],
        ident,
        item_kind: ItemKind::Function,
        expected_cfg: None,
    }
}

fn path_resolution_function_args(
    expected_path: &'static [&'static str],
    ident: &'static str,
) -> ParanoidArgs<'static> {
    ParanoidArgs {
        fixture: "fixture_path_resolution",
        relative_file_path: PATH_RESOLUTION_LIB_RS,
        expected_path,
        ident,
        item_kind: ItemKind::Function,
        expected_cfg: None,
    }
}

fn fixture_call_graph_function_args(
    expected_path: &'static [&'static str],
    ident: &'static str,
) -> ParanoidArgs<'static> {
    ParanoidArgs {
        fixture: "fixture_call_graph",
        relative_file_path: CALL_GRAPH_LIB_RS,
        expected_path,
        ident,
        item_kind: ItemKind::Function,
        expected_cfg: None,
    }
}

fn fixture_call_graph_struct_args(ident: &'static str) -> ParanoidArgs<'static> {
    ParanoidArgs {
        fixture: "fixture_call_graph",
        relative_file_path: CALL_GRAPH_LIB_RS,
        expected_path: &["crate"],
        ident,
        item_kind: ItemKind::Struct,
        expected_cfg: None,
    }
}

fn fixture_impls_root_method_args(
    ident: &'static str,
    span: (usize, usize),
) -> AssocParanoidArgs<'static> {
    AssocParanoidArgs {
        fixture: "fixture_impls",
        relative_file_path: FIXTURE_IMPLS_MAIN_RS,
        expected_path: &["crate"],
        owner: AssocOwner::Impl { span },
        ident,
        expected_cfg: None,
    }
}

fn fixture_impls_nested_method_args(ident: &'static str) -> AssocParanoidArgs<'static> {
    AssocParanoidArgs {
        fixture: "fixture_impls",
        relative_file_path: FIXTURE_IMPLS_MAIN_RS,
        expected_path: &["crate", "nested_impl_block"],
        owner: AssocOwner::Impl {
            span: FIXTURE_IMPLS_FUNC_THREE_IMPL_SPAN,
        },
        ident,
        expected_cfg: None,
    }
}

fn fixture_impls_file_module_method_args(ident: &'static str) -> AssocParanoidArgs<'static> {
    AssocParanoidArgs {
        fixture: "fixture_impls",
        relative_file_path: FIXTURE_IMPLS_FILE_MODULE_RS,
        expected_path: &["crate", "impl_in_file_module"],
        owner: AssocOwner::Impl {
            span: FIXTURE_IMPLS_FILE_MODULE_IMPL_SPAN,
        },
        ident,
        expected_cfg: None,
    }
}

fn fixture_edge_cases_helper_method_args(ident: &'static str) -> AssocParanoidArgs<'static> {
    AssocParanoidArgs {
        fixture: "fixture_edge_cases",
        relative_file_path: EDGE_CASES_LIB_RS,
        expected_path: &["crate", "internal", "utils"],
        owner: AssocOwner::Impl {
            span: EDGE_CASES_HELPER_IMPL_SPAN,
        },
        ident,
        expected_cfg: None,
    }
}

fn fixture_edge_cases_processor_impl_method_args(
    ident: &'static str,
) -> AssocParanoidArgs<'static> {
    AssocParanoidArgs {
        fixture: "fixture_edge_cases",
        relative_file_path: EDGE_CASES_LIB_RS,
        expected_path: &["crate"],
        owner: AssocOwner::Impl {
            span: EDGE_CASES_PROCESSOR_IMPL_SPAN,
        },
        ident,
        expected_cfg: None,
    }
}

fn fixture_edge_cases_generic_item_method_args(ident: &'static str) -> AssocParanoidArgs<'static> {
    AssocParanoidArgs {
        fixture: "fixture_edge_cases",
        relative_file_path: EDGE_CASES_LIB_RS,
        expected_path: &["crate"],
        owner: AssocOwner::Impl {
            span: EDGE_CASES_GENERIC_ITEM_IMPL_SPAN,
        },
        ident,
        expected_cfg: None,
    }
}

fn fixture_edge_cases_function_args(
    expected_path: &'static [&'static str],
    ident: &'static str,
) -> ParanoidArgs<'static> {
    ParanoidArgs {
        fixture: "fixture_edge_cases",
        relative_file_path: EDGE_CASES_LIB_RS,
        expected_path,
        ident,
        item_kind: ItemKind::Function,
        expected_cfg: None,
    }
}

fn fixture_generics_trait_impl_method_args(ident: &'static str) -> AssocParanoidArgs<'static> {
    AssocParanoidArgs {
        fixture: "fixture_generics",
        relative_file_path: GENERICS_LIB_RS,
        expected_path: &["crate"],
        owner: AssocOwner::Impl {
            span: GENERICS_TRAIT_IMPL_SPAN,
        },
        ident,
        expected_cfg: None,
    }
}

fn fixture_type_resolution_v2_assoc_const_args(ident: &'static str) -> AssocParanoidArgs<'static> {
    AssocParanoidArgs {
        fixture: "fixture_type_resolution_v2",
        relative_file_path: TYPE_RESOLUTION_V2_LIB_RS,
        expected_path: &["crate"],
        owner: AssocOwner::Impl {
            span: TYPE_RESOLUTION_V2_ASSOC_GENERIC_IMPL_SPAN,
        },
        ident,
        expected_cfg: None,
    }
}

fn fixture_call_graph_assoc_method_args(ident: &'static str) -> AssocParanoidArgs<'static> {
    AssocParanoidArgs {
        fixture: "fixture_call_graph",
        relative_file_path: CALL_GRAPH_LIB_RS,
        expected_path: &["crate"],
        owner: AssocOwner::Impl {
            span: LOCAL_ASSOC_IMPL_SPAN,
        },
        ident,
        expected_cfg: None,
    }
}

fn fixture_call_graph_imported_assoc_method_args(
    ident: &'static str,
) -> AssocParanoidArgs<'static> {
    AssocParanoidArgs {
        fixture: "fixture_call_graph",
        relative_file_path: CALL_GRAPH_LIB_RS,
        expected_path: &["crate", "assoc_import_targets"],
        owner: AssocOwner::Impl {
            span: IMPORTED_ASSOC_IMPL_SPAN,
        },
        ident,
        expected_cfg: None,
    }
}

fn fixture_call_graph_instance_method_args(ident: &'static str) -> AssocParanoidArgs<'static> {
    AssocParanoidArgs {
        fixture: "fixture_call_graph",
        relative_file_path: CALL_GRAPH_LIB_RS,
        expected_path: &["crate"],
        owner: AssocOwner::Impl {
            span: INSTANCE_IMPL_SPAN,
        },
        ident,
        expected_cfg: None,
    }
}

fn fixture_call_graph_explicit_drop_method_args(ident: &'static str) -> AssocParanoidArgs<'static> {
    AssocParanoidArgs {
        fixture: "fixture_call_graph",
        relative_file_path: CALL_GRAPH_LIB_RS,
        expected_path: &["crate"],
        owner: AssocOwner::Impl {
            span: EXPLICIT_DROP_IMPL_SPAN,
        },
        ident,
        expected_cfg: None,
    }
}

fn fixture_call_graph_assoc_const_impl_args(ident: &'static str) -> AssocParanoidArgs<'static> {
    AssocParanoidArgs {
        fixture: "fixture_call_graph",
        relative_file_path: CALL_GRAPH_LIB_RS,
        expected_path: &["crate"],
        owner: AssocOwner::Impl {
            span: ASSOC_CONST_CARRIER_IMPL_SPAN,
        },
        ident,
        expected_cfg: None,
    }
}

fn fixture_call_graph_assoc_const_trait_args(ident: &'static str) -> AssocParanoidArgs<'static> {
    AssocParanoidArgs {
        fixture: "fixture_call_graph",
        relative_file_path: CALL_GRAPH_LIB_RS,
        expected_path: &["crate"],
        owner: AssocOwner::Trait {
            trait_name: "LocalAssocConstTrait",
        },
        ident,
        expected_cfg: None,
    }
}

fn fixture_call_graph_assoc_function_trait_args(ident: &'static str) -> AssocParanoidArgs<'static> {
    AssocParanoidArgs {
        fixture: "fixture_call_graph",
        relative_file_path: CALL_GRAPH_LIB_RS,
        expected_path: &["crate"],
        owner: AssocOwner::Trait {
            trait_name: "LocalAssocFunctionTrait",
        },
        ident,
        expected_cfg: None,
    }
}

fn fixture_call_graph_imported_assoc_function_trait_args(
    ident: &'static str,
) -> AssocParanoidArgs<'static> {
    AssocParanoidArgs {
        fixture: "fixture_call_graph",
        relative_file_path: CALL_GRAPH_LIB_RS,
        expected_path: &["crate", "trait_assoc_import_targets"],
        owner: AssocOwner::Trait {
            trait_name: "ImportedAssocFunctionTrait",
        },
        ident,
        expected_cfg: None,
    }
}

fn fixture_call_graph_raw_method_args(ident: &'static str) -> AssocParanoidArgs<'static> {
    AssocParanoidArgs {
        fixture: "fixture_call_graph",
        relative_file_path: CALL_GRAPH_LIB_RS,
        expected_path: &["crate"],
        owner: AssocOwner::Impl {
            span: RAW_METHOD_IMPL_SPAN,
        },
        ident,
        expected_cfg: None,
    }
}

fn fixture_call_graph_generic_method_args(ident: &'static str) -> AssocParanoidArgs<'static> {
    AssocParanoidArgs {
        fixture: "fixture_call_graph",
        relative_file_path: CALL_GRAPH_LIB_RS,
        expected_path: &["crate"],
        owner: AssocOwner::Impl {
            span: GENERIC_METHOD_IMPL_SPAN,
        },
        ident,
        expected_cfg: None,
    }
}

fn fixture_call_graph_trait_impl_method_args(ident: &'static str) -> AssocParanoidArgs<'static> {
    AssocParanoidArgs {
        fixture: "fixture_call_graph",
        relative_file_path: CALL_GRAPH_LIB_RS,
        expected_path: &["crate"],
        owner: AssocOwner::Impl {
            span: LOCAL_DISPATCH_TRAIT_IMPL_SPAN,
        },
        ident,
        expected_cfg: None,
    }
}

fn fixture_call_graph_trait_impl_body_call_method_args(
    ident: &'static str,
) -> AssocParanoidArgs<'static> {
    AssocParanoidArgs {
        fixture: "fixture_call_graph",
        relative_file_path: CALL_GRAPH_LIB_RS,
        expected_path: &["crate"],
        owner: AssocOwner::Impl {
            span: TRAIT_IMPL_BODY_CALL_IMPL_SPAN,
        },
        ident,
        expected_cfg: None,
    }
}

fn fixture_call_graph_inherent_precedence_method_args(
    ident: &'static str,
) -> AssocParanoidArgs<'static> {
    AssocParanoidArgs {
        fixture: "fixture_call_graph",
        relative_file_path: CALL_GRAPH_LIB_RS,
        expected_path: &["crate"],
        owner: AssocOwner::Impl {
            span: INHERENT_PRECEDENCE_IMPL_SPAN,
        },
        ident,
        expected_cfg: None,
    }
}

fn fixture_call_graph_generic_bound_trait_args(ident: &'static str) -> AssocParanoidArgs<'static> {
    AssocParanoidArgs {
        fixture: "fixture_call_graph",
        relative_file_path: CALL_GRAPH_LIB_RS,
        expected_path: &["crate"],
        owner: AssocOwner::Trait {
            trait_name: "GenericBoundTrait",
        },
        ident,
        expected_cfg: None,
    }
}

fn fixture_call_graph_trait_default_method_args(ident: &'static str) -> AssocParanoidArgs<'static> {
    AssocParanoidArgs {
        fixture: "fixture_call_graph",
        relative_file_path: CALL_GRAPH_LIB_RS,
        expected_path: &["crate"],
        owner: AssocOwner::Trait {
            trait_name: "TraitDefaultCall",
        },
        ident,
        expected_cfg: None,
    }
}

fn fixture_call_graph_default_required_method_args(
    ident: &'static str,
) -> AssocParanoidArgs<'static> {
    AssocParanoidArgs {
        fixture: "fixture_call_graph",
        relative_file_path: CALL_GRAPH_LIB_RS,
        expected_path: &["crate"],
        owner: AssocOwner::Trait {
            trait_name: "DefaultRequiredCall",
        },
        ident,
        expected_cfg: None,
    }
}

fn fixture_call_graph_trait_default_assoc_method_args(
    ident: &'static str,
) -> AssocParanoidArgs<'static> {
    AssocParanoidArgs {
        fixture: "fixture_call_graph",
        relative_file_path: CALL_GRAPH_LIB_RS,
        expected_path: &["crate"],
        owner: AssocOwner::Trait {
            trait_name: "TraitDefaultAssocCall",
        },
        ident,
        expected_cfg: None,
    }
}

fn fixture_call_graph_scoped_trait_impl_method_args(
    ident: &'static str,
) -> AssocParanoidArgs<'static> {
    AssocParanoidArgs {
        fixture: "fixture_call_graph",
        relative_file_path: CALL_GRAPH_LIB_RS,
        expected_path: &["crate", "trait_scope"],
        owner: AssocOwner::Impl {
            span: SCOPED_TRAIT_IMPL_SPAN,
        },
        ident,
        expected_cfg: None,
    }
}

fn fixture_call_graph_blanket_trait_impl_method_args(
    ident: &'static str,
) -> AssocParanoidArgs<'static> {
    AssocParanoidArgs {
        fixture: "fixture_call_graph",
        relative_file_path: CALL_GRAPH_LIB_RS,
        expected_path: &["crate"],
        owner: AssocOwner::Impl {
            span: BLANKET_TRAIT_IMPL_SPAN,
        },
        ident,
        expected_cfg: None,
    }
}

fn fixture_call_graph_inline_blanket_impl_method_args(
    ident: &'static str,
) -> AssocParanoidArgs<'static> {
    AssocParanoidArgs {
        fixture: "fixture_call_graph",
        relative_file_path: CALL_GRAPH_LIB_RS,
        expected_path: &["crate"],
        owner: AssocOwner::Impl {
            span: INLINE_BOUND_BLANKET_TRAIT_IMPL_SPAN,
        },
        ident,
        expected_cfg: None,
    }
}

fn fixture_call_graph_where_blanket_impl_method_args(
    ident: &'static str,
) -> AssocParanoidArgs<'static> {
    AssocParanoidArgs {
        fixture: "fixture_call_graph",
        relative_file_path: CALL_GRAPH_LIB_RS,
        expected_path: &["crate"],
        owner: AssocOwner::Impl {
            span: WHERE_BOUND_BLANKET_TRAIT_IMPL_SPAN,
        },
        ident,
        expected_cfg: None,
    }
}

fn fixture_call_graph_transitive_blanket_impl_method_args(
    ident: &'static str,
) -> AssocParanoidArgs<'static> {
    AssocParanoidArgs {
        fixture: "fixture_call_graph",
        relative_file_path: CALL_GRAPH_LIB_RS,
        expected_path: &["crate"],
        owner: AssocOwner::Impl {
            span: TRANSITIVE_BOUND_BLANKET_TRAIT_IMPL_SPAN,
        },
        ident,
        expected_cfg: None,
    }
}

fn fixture_call_graph_constrained_generic_self_impl_method_args(
    ident: &'static str,
) -> AssocParanoidArgs<'static> {
    AssocParanoidArgs {
        fixture: "fixture_call_graph",
        relative_file_path: CALL_GRAPH_LIB_RS,
        expected_path: &["crate"],
        owner: AssocOwner::Impl {
            span: CONSTRAINED_GENERIC_SELF_TRAIT_IMPL_SPAN,
        },
        ident,
        expected_cfg: None,
    }
}

fn fixture_call_graph_local_shadow_vec_method_args(
    ident: &'static str,
) -> AssocParanoidArgs<'static> {
    AssocParanoidArgs {
        fixture: "fixture_call_graph",
        relative_file_path: CALL_GRAPH_LIB_RS,
        expected_path: &["crate", "local_prelude_shadow"],
        owner: AssocOwner::Impl {
            span: LOCAL_VEC_IMPL_SPAN,
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
    fixture_nodes_get_secret_len_records_self_field_len_external_method_call_site,
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
        ExpectedCallOutcome::External,
    ),
);

paranoid_call_site_test!(
    fixture_nodes_get_str_len_records_self_field_len_method_call_site,
    fixture: "fixture_nodes",
    owner: method {
        args: generic_str_inherent_method_args("get_str_len")
    },
    expected: ExpectedCallSite::method(
        "len",
        ExpectedMethodReceiver::SelfField {
            field_path: &["value"]
        },
        SELF_VALUE_LEN_CALL_SPAN,
        0,
        0,
        &[],
        ExpectedCallOutcome::Unsupported,
    ),
);

paranoid_call_site_test!(
    fixture_nodes_generic_simple_trait_method_records_self_field_into_method_call_site,
    fixture: "fixture_nodes",
    owner: method {
        args: generic_simple_trait_method_args("trait_method")
    },
    expected: ExpectedCallSite::method(
        "into",
        ExpectedMethodReceiver::SelfField {
            field_path: &["value"]
        },
        SELF_VALUE_INTO_CALL_SPAN,
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
        ExpectedCallOutcome::External,
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
        ExpectedCallOutcome::External,
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
        ExpectedCallOutcome::External,
    ),
);

paranoid_call_site_test!(
    fixture_nodes_use_imported_items_records_enum_variant1_path_call_site,
    fixture: "fixture_nodes",
    owner: function {
        module_path: &["crate", "imports"],
        name: "use_imported_items"
    },
    expected: {
        let target = fixture_nodes_enum_variant_id("EnumWithData", "Variant1");
        ExpectedCallSite::path(
            &["EnumWithData", "Variant1"],
            ENUM_VARIANT1_CALL_SPAN,
            1,
            0,
            &[],
            ExpectedCallOutcome::ResolvedEnumVariantConstructorLocalExact { target },
        )
    },
);

paranoid_call_site_test!(
    fixture_nodes_use_imported_items_records_alias_checker_value_binding_path_call_site,
    fixture: "fixture_nodes",
    owner: function {
        module_path: &["crate", "imports"],
        name: "use_imported_items"
    },
    expected: ExpectedCallSite::path_value_binding(
        &["alias_checker"],
        ALIAS_CHECKER_CALL_SPAN,
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
        ExpectedCallOutcome::External,
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
        ExpectedCallOutcome::External,
    ),
);

paranoid_call_site_test!(
    fixture_nodes_use_imported_items_records_tuple_struct_path_call_site,
    fixture: "fixture_nodes",
    owner: function {
        module_path: &["crate", "imports"],
        name: "use_imported_items"
    },
    expected: {
        let target_args = fixture_nodes_struct_args("TupleStruct");
        let target_info = target_args.generate_pid(&*PARSED_FIXTURE_CRATE_NODES)?;
        let target = StructNodeId::try_from(target_info.test_pid())
            .expect("TupleStruct should regenerate a StructNodeId");
        ExpectedCallSite::path(
            &["TupleStruct"],
            TUPLE_STRUCT_CALL_SPAN,
            2,
            0,
            &[],
            ExpectedCallOutcome::ResolvedTupleStructConstructorLocalExact { target },
        )
    },
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

paranoid_call_site_test!(
    fixture_nodes_fn_call_const_resolves_const_initializer_path_call_site,
    fixture: "fixture_nodes",
    owner: const_item {
        args: fixture_nodes_const_args("FN_CALL_CONST")
    },
    expected: {
        let target_args = fixture_nodes_const_static_function_args("five");
        let target_info = target_args.generate_pid(&*PARSED_FIXTURE_CRATE_NODES)?;
        let target = FunctionNodeId::try_from(target_info.test_pid())
            .expect("five should regenerate a FunctionNodeId");
        ExpectedCallSite::path(
            &["five"],
            FN_CALL_CONST_FIVE_CALL_SPAN,
            0,
            0,
            &[],
            ExpectedCallOutcome::ResolvedFunctionLocalExact { target },
        )
    },
);

paranoid_call_site_test!(
    fixture_nodes_static_fn_call_resolves_static_initializer_path_call_site,
    fixture: "fixture_nodes",
    owner: static_item {
        args: fixture_nodes_static_args("STATIC_FN_CALL")
    },
    expected: {
        let target_args = fixture_nodes_const_static_function_args("five");
        let target_info = target_args.generate_pid(&*PARSED_FIXTURE_CRATE_NODES)?;
        let target = FunctionNodeId::try_from(target_info.test_pid())
            .expect("five should regenerate a FunctionNodeId");
        ExpectedCallSite::path(
            &["five"],
            STATIC_FN_CALL_FIVE_CALL_SPAN,
            0,
            0,
            &[],
            ExpectedCallOutcome::ResolvedFunctionLocalExact { target },
        )
    },
);

paranoid_call_site_test!(
    fixture_path_resolution_call_restricted_resolves_super_restricted_func_path_call_site,
    fixture: "fixture_path_resolution",
    owner: function {
        module_path: &["crate", "restricted_vis_mod", "inner"],
        name: "call_restricted"
    },
    expected: {
        let target_args =
            path_resolution_function_args(&["crate", "restricted_vis_mod"], "restricted_func");
        let target_info = target_args.generate_pid(
            crate::common::call_site_paranoid::parsed_graphs_for_fixture("fixture_path_resolution"),
        )?;
        let target = FunctionNodeId::try_from(target_info.test_pid())
            .expect("restricted_func should regenerate a FunctionNodeId");
        ExpectedCallSite::path(
            &["super", "restricted_func"],
            SUPER_RESTRICTED_FUNC_CALL_SPAN,
            0,
            0,
            &[],
            ExpectedCallOutcome::ResolvedFunctionLocalExact { target },
        )
    },
);

paranoid_call_site_test!(
    fixture_path_resolution_root_func_records_std_path_new_external_path_call_site,
    fixture: "fixture_path_resolution",
    owner: function {
        module_path: &["crate"],
        name: "root_func"
    },
    expected: ExpectedCallSite::path(
        &["std", "path", "Path", "new"],
        STD_PATH_NEW_CALL_SPAN,
        1,
        0,
        &[],
        ExpectedCallOutcome::External,
    ),
);

paranoid_call_site_test!(
    fixture_path_resolution_root_func_records_regex_new_external_path_call_site,
    fixture: "fixture_path_resolution",
    owner: function {
        module_path: &["crate"],
        name: "root_func"
    },
    expected: ExpectedCallSite::path(
        &["Regex", "new"],
        REGEX_NEW_CALL_SPAN,
        1,
        0,
        &[],
        ExpectedCallOutcome::External,
    ),
);

paranoid_call_site_test!(
    fixture_path_resolution_root_func_records_regex_unwrap_external_method_call_site,
    fixture: "fixture_path_resolution",
    owner: function {
        module_path: &["crate"],
        name: "root_func"
    },
    expected: ExpectedCallSite::method(
        "unwrap",
        ExpectedMethodReceiver::PathCallResult {
            path: &["Regex", "new"],
        },
        REGEX_UNWRAP_CALL_SPAN,
        0,
        0,
        &[],
        ExpectedCallOutcome::External,
    ),
);

paranoid_call_site_test!(
    fixture_path_resolution_root_func_records_typeid_synthetic_external_path_call_site,
    fixture: "fixture_path_resolution",
    owner: function {
        module_path: &["crate"],
        name: "root_func"
    },
    expected: ExpectedCallSite::path(
        &["TypeId", "Synthetic"],
        TYPEID_SYNTHETIC_CALL_SPAN,
        1,
        0,
        &[],
        ExpectedCallOutcome::External,
    ),
);

paranoid_call_site_test!(
    fixture_path_resolution_root_func_records_nodeid_generate_synthetic_external_path_call_site,
    fixture: "fixture_path_resolution",
    owner: function {
        module_path: &["crate"],
        name: "root_func"
    },
    expected: ExpectedCallSite::path(
        &["NodeId", "generate_synthetic"],
        NODEID_GENERATE_SYNTHETIC_CALL_SPAN,
        7,
        0,
        &[],
        ExpectedCallOutcome::External,
    ),
);

paranoid_call_site_test!(
    fixture_path_resolution_root_func_records_uuid_nil_external_path_call_site,
    fixture: "fixture_path_resolution",
    owner: function {
        module_path: &["crate"],
        name: "root_func"
    },
    expected: ExpectedCallSite::path(
        &["uuid", "Uuid", "nil"],
        UUID_NIL_CALL_SPAN,
        0,
        0,
        &[],
        ExpectedCallOutcome::External,
    ),
);

paranoid_call_site_test!(
    fixture_path_resolution_root_func_records_nodeid_uuid_external_method_call_site,
    fixture: "fixture_path_resolution",
    owner: function {
        module_path: &["crate"],
        name: "root_func"
    },
    expected: ExpectedCallSite::method(
        "uuid",
        ExpectedMethodReceiver::PathCallResult {
            path: &["NodeId", "generate_synthetic"],
        },
        NODEID_UUID_CALL_SPAN,
        0,
        0,
        &[],
        ExpectedCallOutcome::External,
    ),
);

paranoid_call_site_test!(
    fixture_path_resolution_root_func_records_info_macro_call_site,
    fixture: "fixture_path_resolution",
    owner: function {
        module_path: &["crate"],
        name: "root_func"
    },
    expected: ExpectedCallSite::macro_call(
        "info",
        ROOT_INFO_MACRO_CALL_SPAN,
        &[],
        ExpectedCallOutcome::Unsupported,
    ),
);

paranoid_call_site_test!(
    fixture_path_resolution_root_func_records_debug_macro_call_site,
    fixture: "fixture_path_resolution",
    owner: function {
        module_path: &["crate"],
        name: "root_func"
    },
    expected: ExpectedCallSite::macro_call(
        "debug",
        ROOT_DEBUG_MACRO_CALL_SPAN,
        &[],
        ExpectedCallOutcome::Unsupported,
    ),
);

paranoid_call_site_test!(
    fixture_macros_use_local_macro_records_local_macro_call_site,
    fixture: "fixture_macros",
    owner: function {
        module_path: &["crate"],
        name: "use_local_macro"
    },
    expected: ExpectedCallSite::macro_call(
        "local_macro",
        LOCAL_MACRO_CALL_SPAN,
        &[],
        ExpectedCallOutcome::Unsupported,
    ),
);

paranoid_call_site_test!(
    fixture_macros_use_local_macro_records_println_macro_call_site,
    fixture: "fixture_macros",
    owner: function {
        module_path: &["crate"],
        name: "use_local_macro"
    },
    expected: ExpectedCallSite::macro_call(
        "println",
        FIXTURE_MACROS_PRINTLN_CALL_SPAN,
        &[],
        ExpectedCallOutcome::Unsupported,
    ),
);

paranoid_call_site_test!(
    fixture_impls_main_resolves_func_test_one_initialized_local_method_call_site,
    fixture: "fixture_impls",
    owner: function {
        module_path: &["crate"],
        name: "main"
    },
    expected: {
        let target_args = fixture_impls_root_method_args(
            "func_test_one",
            FIXTURE_IMPLS_FUNC_ONE_IMPL_SPAN,
        );
        let target_info = target_args.generate_method_pid(&*PARSED_FIXTURE_CRATE_IMPLS)?;
        ExpectedCallSite::method(
            "func_test_one",
            ExpectedMethodReceiver::InitializedLocalBinding {
                name: "x",
                init_path: &["TestImplStruct"],
            },
            FIXTURE_IMPLS_FUNC_ONE_CALL_SPAN,
            0,
            0,
            &[],
            ExpectedCallOutcome::ResolvedMethodLocalExact {
                target: target_info.test_method_id(),
            },
        )
    },
);

paranoid_call_site_test!(
    fixture_impls_main_resolves_func_test_two_initialized_local_method_call_site,
    fixture: "fixture_impls",
    owner: function {
        module_path: &["crate"],
        name: "main"
    },
    expected: {
        let target_args = fixture_impls_root_method_args(
            "func_test_two",
            FIXTURE_IMPLS_FUNC_TWO_IMPL_SPAN,
        );
        let target_info = target_args.generate_method_pid(&*PARSED_FIXTURE_CRATE_IMPLS)?;
        ExpectedCallSite::method(
            "func_test_two",
            ExpectedMethodReceiver::InitializedLocalBinding {
                name: "x",
                init_path: &["TestImplStruct"],
            },
            FIXTURE_IMPLS_FUNC_TWO_CALL_SPAN,
            0,
            0,
            &[],
            ExpectedCallOutcome::ResolvedMethodLocalExact {
                target: target_info.test_method_id(),
            },
        )
    },
);

paranoid_call_site_test!(
    fixture_impls_main_resolves_func_test_three_initialized_local_method_call_site,
    fixture: "fixture_impls",
    owner: function {
        module_path: &["crate"],
        name: "main"
    },
    expected: {
        let target_args = fixture_impls_nested_method_args("func_test_three");
        let target_info = target_args.generate_method_pid(&*PARSED_FIXTURE_CRATE_IMPLS)?;
        ExpectedCallSite::method(
            "func_test_three",
            ExpectedMethodReceiver::InitializedLocalBinding {
                name: "x",
                init_path: &["TestImplStruct"],
            },
            FIXTURE_IMPLS_FUNC_THREE_CALL_SPAN,
            0,
            0,
            &[],
            ExpectedCallOutcome::ResolvedMethodLocalExact {
                target: target_info.test_method_id(),
            },
        )
    },
);

paranoid_call_site_test!(
    fixture_impls_main_resolves_func_test_four_associated_function_path_call_site,
    fixture: "fixture_impls",
    owner: function {
        module_path: &["crate"],
        name: "main"
    },
    expected: {
        let target_args = fixture_impls_file_module_method_args("func_test_four");
        let target_info = target_args.generate_method_pid(&*PARSED_FIXTURE_CRATE_IMPLS)?;
        ExpectedCallSite::path(
            &["TestImplStruct", "func_test_four"],
            FIXTURE_IMPLS_FUNC_FOUR_CALL_SPAN,
            0,
            0,
            &[],
            ExpectedCallOutcome::ResolvedAssociatedFunctionLocalExact {
                target: target_info.test_method_id(),
            },
        )
    },
);

paranoid_call_site_test!(
    fixture_impls_main_resolves_func_test_five_initialized_local_method_call_site,
    fixture: "fixture_impls",
    owner: function {
        module_path: &["crate"],
        name: "main"
    },
    expected: {
        let target_args = fixture_impls_file_module_method_args("func_test_five");
        let target_info = target_args.generate_method_pid(&*PARSED_FIXTURE_CRATE_IMPLS)?;
        ExpectedCallSite::method(
            "func_test_five",
            ExpectedMethodReceiver::InitializedLocalBinding {
                name: "x",
                init_path: &["TestImplStruct"],
            },
            FIXTURE_IMPLS_FUNC_FIVE_CALL_SPAN,
            0,
            0,
            &[],
            ExpectedCallOutcome::ResolvedMethodLocalExact {
                target: target_info.test_method_id(),
            },
        )
    },
);

paranoid_call_site_test!(
    fixture_impls_main_records_println_macro_call_site,
    fixture: "fixture_impls",
    owner: function {
        module_path: &["crate"],
        name: "main"
    },
    expected: ExpectedCallSite::macro_call(
        "println",
        FIXTURE_IMPLS_PRINTLN_CALL_SPAN,
        &[],
        ExpectedCallOutcome::Unsupported,
    ),
);

paranoid_call_site_test!(
    fixture_edge_cases_use_imports_resolves_direct_imported_helper_method_call_site,
    fixture: "fixture_edge_cases",
    owner: function {
        module_path: &["crate"],
        name: "use_imports"
    },
    expected: {
        let target_args = fixture_edge_cases_helper_method_args("help");
        let target_info = target_args.generate_method_pid(&*PARSED_FIXTURE_CRATE_EDGE_CASES)?;
        ExpectedCallSite::method(
            "help",
            ExpectedMethodReceiver::InitializedLocalBinding {
                name: "h",
                init_path: &["Helper"],
            },
            EDGE_CASES_HELPER_HELP_CALL_SPAN,
            0,
            0,
            &[],
            ExpectedCallOutcome::ResolvedMethodLocalExact {
                target: target_info.test_method_id(),
            },
        )
    },
);

paranoid_call_site_test!(
    fixture_edge_cases_use_imports_resolves_reexported_helper_method_call_site,
    fixture: "fixture_edge_cases",
    owner: function {
        module_path: &["crate"],
        name: "use_imports"
    },
    expected: {
        let target_args = fixture_edge_cases_helper_method_args("help");
        let target_info = target_args.generate_method_pid(&*PARSED_FIXTURE_CRATE_EDGE_CASES)?;
        ExpectedCallSite::method(
            "help",
            ExpectedMethodReceiver::InitializedLocalBinding {
                name: "uh",
                init_path: &["UtilityHelper"],
            },
            EDGE_CASES_UTILITY_HELPER_HELP_CALL_SPAN,
            0,
            0,
            &[],
            ExpectedCallOutcome::ResolvedMethodLocalExact {
                target: target_info.test_method_id(),
            },
        )
    },
);

paranoid_call_site_test!(
    fixture_edge_cases_use_imports_records_literal_to_string_external_method_call_site,
    fixture: "fixture_edge_cases",
    owner: function {
        module_path: &["crate"],
        name: "use_imports"
    },
    expected: ExpectedCallSite::method(
        "to_string",
        ExpectedMethodReceiver::Literal,
        EDGE_CASES_HELLO_TO_STRING_CALL_SPAN,
        0,
        0,
        &[],
        ExpectedCallOutcome::External,
    ),
);

paranoid_call_site_test!(
    fixture_edge_cases_processor_trait_impl_records_format_macro_call_site,
    fixture: "fixture_edge_cases",
    owner: method {
        args: fixture_edge_cases_processor_impl_method_args("process")
    },
    expected: ExpectedCallSite::macro_call(
        "format",
        EDGE_CASES_FORMAT_MACRO_CALL_SPAN,
        &[],
        ExpectedCallOutcome::Unsupported,
    ),
);

paranoid_call_site_test!(
    fixture_edge_cases_generic_item_new_records_t_default_unsupported_path_call_site,
    fixture: "fixture_edge_cases",
    owner: method {
        args: fixture_edge_cases_generic_item_method_args("new")
    },
    expected: ExpectedCallSite::path(
        &["T", "default"],
        EDGE_CASES_T_DEFAULT_CALL_SPAN,
        0,
        0,
        &[],
        ExpectedCallOutcome::Unsupported,
    ),
);

paranoid_call_site_test!(
    fixture_edge_cases_test_visibility_resolves_internal_helper_path_call_site,
    fixture: "fixture_edge_cases",
    owner: function {
        module_path: &["crate", "internal"],
        name: "test_visibility"
    },
    expected: {
        let target_args =
            fixture_edge_cases_function_args(&["crate", "internal", "utils"], "internal_helper");
        let target_info = target_args.generate_pid(
            crate::common::call_site_paranoid::parsed_graphs_for_fixture("fixture_edge_cases"),
        )?;
        let target = FunctionNodeId::try_from(target_info.test_pid())
            .expect("internal_helper should regenerate a FunctionNodeId");
        ExpectedCallSite::path(
            &["utils", "internal_helper"],
            EDGE_CASES_INTERNAL_HELPER_CALL_SPAN,
            0,
            0,
            &[EDGE_CASES_VISIBILITY_CFG],
            ExpectedCallOutcome::ResolvedFunctionLocalExact { target },
        )
    },
);

paranoid_call_site_test!(
    fixture_edge_cases_test_visibility_resolves_super_helper_path_call_site,
    fixture: "fixture_edge_cases",
    owner: function {
        module_path: &["crate", "internal"],
        name: "test_visibility"
    },
    expected: {
        let target_args =
            fixture_edge_cases_function_args(&["crate", "internal", "utils"], "super_helper");
        let target_info = target_args.generate_pid(
            crate::common::call_site_paranoid::parsed_graphs_for_fixture("fixture_edge_cases"),
        )?;
        let target = FunctionNodeId::try_from(target_info.test_pid())
            .expect("super_helper should regenerate a FunctionNodeId");
        ExpectedCallSite::path(
            &["utils", "super_helper"],
            EDGE_CASES_SUPER_HELPER_CALL_SPAN,
            0,
            0,
            &[EDGE_CASES_VISIBILITY_CFG],
            ExpectedCallOutcome::ResolvedFunctionLocalExact { target },
        )
    },
);

paranoid_call_site_test!(
    fixture_edge_cases_test_visibility_resolves_restricted_func_path_call_site,
    fixture: "fixture_edge_cases",
    owner: function {
        module_path: &["crate", "internal"],
        name: "test_visibility"
    },
    expected: {
        let target_args =
            fixture_edge_cases_function_args(&["crate", "internal", "restricted"], "restricted_func");
        let target_info = target_args.generate_pid(
            crate::common::call_site_paranoid::parsed_graphs_for_fixture("fixture_edge_cases"),
        )?;
        let target = FunctionNodeId::try_from(target_info.test_pid())
            .expect("restricted_func should regenerate a FunctionNodeId");
        ExpectedCallSite::path(
            &["restricted", "restricted_func"],
            EDGE_CASES_RESTRICTED_FUNC_CALL_SPAN,
            0,
            0,
            &[EDGE_CASES_VISIBILITY_CFG],
            ExpectedCallOutcome::ResolvedFunctionLocalExact { target },
        )
    },
);

paranoid_call_site_test!(
    fixture_generics_generic_function_records_t_default_unsupported_path_call_site,
    fixture: "fixture_generics",
    owner: function {
        module_path: &["crate"],
        name: "generic_function"
    },
    expected: ExpectedCallSite::path(
        &["T", "default"],
        GENERICS_T_DEFAULT_CALL_SPAN,
        0,
        0,
        &[],
        ExpectedCallOutcome::Unsupported,
    ),
);

paranoid_call_site_test!(
    fixture_generics_trait_impl_process_records_format_macro_call_site,
    fixture: "fixture_generics",
    owner: method {
        args: fixture_generics_trait_impl_method_args("process")
    },
    expected: ExpectedCallSite::macro_call(
        "format",
        GENERICS_FORMAT_MACRO_CALL_SPAN,
        &[],
        ExpectedCallOutcome::Unsupported,
    ),
);

paranoid_call_site_test!(
    fixture_type_resolution_v2_generic_assoc_const_records_panic_macro_call_site,
    fixture: "fixture_type_resolution_v2",
    owner: associated_const {
        args: fixture_type_resolution_v2_assoc_const_args("TRAIT_CONST")
    },
    expected: ExpectedCallSite::macro_call(
        "panic",
        TYPE_RESOLUTION_V2_ASSOC_CONST_PANIC_CALL_SPAN,
        &[],
        ExpectedCallOutcome::Unsupported,
    ),
);

paranoid_call_site_test!(
    fixture_call_graph_dynamic_calls_records_parenthesized_binding_dynamic_call_site,
    fixture: "fixture_call_graph",
    owner: function {
        module_path: &["crate"],
        name: "dynamic_calls"
    },
    expected: ExpectedCallSite::dynamic_local_binding(
        &["closure"],
        DYNAMIC_CLOSURE_BINDING_CALL_SPAN,
        0,
        &[],
        ExpectedCallOutcome::Unsupported,
    ),
);

paranoid_call_site_test!(
    fixture_call_graph_dynamic_calls_records_closure_literal_dynamic_call_site,
    fixture: "fixture_call_graph",
    owner: function {
        module_path: &["crate"],
        name: "dynamic_calls"
    },
    expected: ExpectedCallSite::dynamic(
        DYNAMIC_CLOSURE_LITERAL_CALL_SPAN,
        0,
        &[],
        ExpectedCallOutcome::Unsupported,
    ),
);

paranoid_call_site_test!(
    fixture_call_graph_call_crate_local_target_resolves_crate_path_call_site,
    fixture: "fixture_call_graph",
    owner: function {
        module_path: &["crate"],
        name: "call_crate_local_target"
    },
    expected: {
        let target_args = fixture_call_graph_function_args(&["crate"], "local_target");
        let parsed_graphs = crate::common::run_phases_and_collect("fixture_call_graph");
        let target_info = target_args.generate_pid(&parsed_graphs)?;
        let target = FunctionNodeId::try_from(target_info.test_pid())
            .expect("local_target should regenerate a FunctionNodeId");
        ExpectedCallSite::path(
            &["crate", "local_target"],
            CRATE_LOCAL_TARGET_CALL_SPAN,
            0,
            0,
            &[],
            ExpectedCallOutcome::ResolvedFunctionLocalExact { target },
        )
    },
);

paranoid_call_site_test!(
    fixture_call_graph_call_self_nested_target_resolves_self_path_call_site,
    fixture: "fixture_call_graph",
    owner: function {
        module_path: &["crate", "local_mod"],
        name: "call_self_nested_target"
    },
    expected: {
        let target_args = fixture_call_graph_function_args(&["crate", "local_mod"], "nested_target");
        let parsed_graphs = crate::common::run_phases_and_collect("fixture_call_graph");
        let target_info = target_args.generate_pid(&parsed_graphs)?;
        let target = FunctionNodeId::try_from(target_info.test_pid())
            .expect("nested_target should regenerate a FunctionNodeId");
        ExpectedCallSite::path(
            &["self", "nested_target"],
            SELF_NESTED_TARGET_CALL_SPAN,
            0,
            0,
            &[],
            ExpectedCallOutcome::ResolvedFunctionLocalExact { target },
        )
    },
);

paranoid_call_site_test!(
    fixture_call_graph_call_crate_module_nested_target_resolves_crate_module_path_call_site,
    fixture: "fixture_call_graph",
    owner: function {
        module_path: &["crate"],
        name: "call_crate_module_nested_target"
    },
    expected: {
        let target_args =
            fixture_call_graph_function_args(&["crate", "local_mod"], "nested_target");
        let parsed_graphs = crate::common::run_phases_and_collect("fixture_call_graph");
        let target_info = target_args.generate_pid(&parsed_graphs)?;
        let target = FunctionNodeId::try_from(target_info.test_pid())
            .expect("local_mod::nested_target should regenerate a FunctionNodeId");
        ExpectedCallSite::path(
            &["crate", "local_mod", "nested_target"],
            CRATE_MODULE_NESTED_TARGET_CALL_SPAN,
            0,
            0,
            &[],
            ExpectedCallOutcome::ResolvedFunctionLocalExact { target },
        )
    },
);

paranoid_call_site_test!(
    fixture_call_graph_call_self_module_nested_target_resolves_self_module_path_call_site,
    fixture: "fixture_call_graph",
    owner: function {
        module_path: &["crate"],
        name: "call_self_module_nested_target"
    },
    expected: {
        let target_args =
            fixture_call_graph_function_args(&["crate", "local_mod"], "nested_target");
        let parsed_graphs = crate::common::run_phases_and_collect("fixture_call_graph");
        let target_info = target_args.generate_pid(&parsed_graphs)?;
        let target = FunctionNodeId::try_from(target_info.test_pid())
            .expect("local_mod::nested_target should regenerate a FunctionNodeId");
        ExpectedCallSite::path(
            &["self", "local_mod", "nested_target"],
            SELF_MODULE_NESTED_TARGET_CALL_SPAN,
            0,
            0,
            &[],
            ExpectedCallOutcome::ResolvedFunctionLocalExact { target },
        )
    },
);

paranoid_call_site_test!(
    fixture_call_graph_call_super_local_target_resolves_super_path_call_site,
    fixture: "fixture_call_graph",
    owner: function {
        module_path: &["crate", "super_path_scope"],
        name: "call_super_local_target"
    },
    expected: {
        let target_args = fixture_call_graph_function_args(&["crate"], "local_target");
        let parsed_graphs = crate::common::run_phases_and_collect("fixture_call_graph");
        let target_info = target_args.generate_pid(&parsed_graphs)?;
        let target = FunctionNodeId::try_from(target_info.test_pid())
            .expect("local_target should regenerate a FunctionNodeId");
        ExpectedCallSite::path(
            &["super", "local_target"],
            SUPER_LOCAL_TARGET_CALL_SPAN,
            0,
            0,
            &[],
            ExpectedCallOutcome::ResolvedFunctionLocalExact { target },
        )
    },
);

paranoid_call_site_test!(
    fixture_call_graph_call_raw_identifier_function_resolves_raw_identifier_path_call_site,
    fixture: "fixture_call_graph",
    owner: function {
        module_path: &["crate"],
        name: "call_raw_identifier_function"
    },
    expected: {
        let target_args = fixture_call_graph_function_args(&["crate"], "r#match");
        let parsed_graphs = crate::common::run_phases_and_collect("fixture_call_graph");
        let target_info = target_args.generate_pid(&parsed_graphs)?;
        let target = FunctionNodeId::try_from(target_info.test_pid())
            .expect("r#match should regenerate a FunctionNodeId");
        ExpectedCallSite::path(
            &["r#match"],
            RAW_IDENTIFIER_FUNCTION_CALL_SPAN,
            0,
            0,
            &[],
            ExpectedCallOutcome::ResolvedFunctionLocalExact { target },
        )
    },
);

paranoid_call_site_test!(
    fixture_call_graph_call_raw_identifier_method_resolves_raw_identifier_method_call_site,
    fixture: "fixture_call_graph",
    owner: function {
        module_path: &["crate"],
        name: "call_raw_identifier_method"
    },
    expected: {
        let target_args = fixture_call_graph_raw_method_args("r#type");
        let parsed_graphs = crate::common::run_phases_and_collect("fixture_call_graph");
        let target_info = target_args.generate_method_pid(&parsed_graphs)?;
        ExpectedCallSite::method(
            "r#type",
            ExpectedMethodReceiver::TypedLocalBinding {
                name: "value",
                type_path: &["RawMethodTarget"],
            },
            RAW_IDENTIFIER_METHOD_CALL_SPAN,
            0,
            0,
            &[],
            ExpectedCallOutcome::ResolvedMethodLocalExact {
                target: target_info.test_method_id(),
            },
        )
    },
);

paranoid_call_site_test!(
    fixture_call_graph_call_method_turbofish_resolves_generic_method_call_site,
    fixture: "fixture_call_graph",
    owner: function {
        module_path: &["crate"],
        name: "call_method_turbofish"
    },
    expected: {
        let target_args = fixture_call_graph_generic_method_args("generic_instance");
        let parsed_graphs = crate::common::run_phases_and_collect("fixture_call_graph");
        let target_info = target_args.generate_method_pid(&parsed_graphs)?;
        ExpectedCallSite::method(
            "generic_instance",
            ExpectedMethodReceiver::TypedLocalBinding {
                name: "value",
                type_path: &["GenericMethodTarget"],
            },
            METHOD_TURBOFISH_CALL_SPAN,
            1,
            1,
            &[],
            ExpectedCallOutcome::ResolvedMethodLocalExact {
                target: target_info.test_method_id(),
            },
        )
    },
);

paranoid_call_site_test!(
    fixture_call_graph_call_prelude_drop_value_records_external_path_call_site,
    fixture: "fixture_call_graph",
    owner: function {
        module_path: &["crate"],
        name: "call_prelude_drop_value"
    },
    expected: ExpectedCallSite::path(
        &["drop"],
        PRELUDE_DROP_CALL_SPAN,
        1,
        0,
        &[],
        ExpectedCallOutcome::External,
    ),
);

paranoid_call_site_test!(
    fixture_call_graph_call_local_drop_shadow_resolves_local_function_path_call_site,
    fixture: "fixture_call_graph",
    owner: function {
        module_path: &["crate", "prelude_shadow_scope"],
        name: "call_local_drop_shadow"
    },
    expected: {
        let target_args =
            fixture_call_graph_function_args(&["crate", "prelude_shadow_scope"], "drop");
        let parsed_graphs = crate::common::run_phases_and_collect("fixture_call_graph");
        let target_info = target_args.generate_pid(&parsed_graphs)?;
        let target = FunctionNodeId::try_from(target_info.test_pid())
            .expect("prelude_shadow_scope::drop should regenerate a FunctionNodeId");
        ExpectedCallSite::path(
            &["drop"],
            LOCAL_DROP_SHADOW_CALL_SPAN,
            1,
            0,
            &[],
            ExpectedCallOutcome::ResolvedFunctionLocalExact { target },
        )
    },
);

paranoid_call_site_test!(
    fixture_call_graph_call_explicit_drop_method_resolves_inherent_method_call_site,
    fixture: "fixture_call_graph",
    owner: function {
        module_path: &["crate"],
        name: "call_explicit_drop_method"
    },
    expected: {
        let target_args = fixture_call_graph_explicit_drop_method_args("drop");
        let parsed_graphs = crate::common::run_phases_and_collect("fixture_call_graph");
        let target_info = target_args.generate_method_pid(&parsed_graphs)?;
        ExpectedCallSite::method(
            "drop",
            ExpectedMethodReceiver::InitializedLocalBinding {
                name: "value",
                init_path: &["ExplicitDropTarget"],
            },
            EXPLICIT_DROP_METHOD_CALL_SPAN,
            0,
            0,
            &[],
            ExpectedCallOutcome::ResolvedMethodLocalExact {
                target: target_info.test_method_id(),
            },
        )
    },
);

paranoid_call_site_test!(
    fixture_call_graph_call_crate_scoped_macro_records_crate_path_macro_call_site,
    fixture: "fixture_call_graph",
    owner: function {
        module_path: &["crate"],
        name: "call_crate_scoped_macro"
    },
    expected: ExpectedCallSite::macro_call(
        "crate::crate_scoped_macro",
        CRATE_SCOPED_MACRO_CALL_SPAN,
        &[],
        ExpectedCallOutcome::Unsupported,
    ),
);

paranoid_call_site_test!(
    fixture_call_graph_call_vec_macro_records_macro_call_site,
    fixture: "fixture_call_graph",
    owner: function {
        module_path: &["crate"],
        name: "call_vec_macro"
    },
    expected: ExpectedCallSite::macro_call(
        "vec",
        VEC_MACRO_CALL_SPAN,
        &[],
        ExpectedCallOutcome::Unsupported,
    ),
);

paranoid_call_site_test!(
    fixture_call_graph_call_imported_macro_alias_records_macro_call_site,
    fixture: "fixture_call_graph",
    owner: function {
        module_path: &["crate"],
        name: "call_imported_macro_alias"
    },
    expected: ExpectedCallSite::macro_call(
        "imported_macro_alias",
        IMPORTED_MACRO_ALIAS_CALL_SPAN,
        &[],
        ExpectedCallOutcome::Unsupported,
    ),
);

paranoid_call_site_test!(
    fixture_call_graph_call_item_macro_inside_body_records_macro_call_site,
    fixture: "fixture_call_graph",
    owner: function {
        module_path: &["crate"],
        name: "call_item_macro_inside_body"
    },
    expected: ExpectedCallSite::macro_call(
        "call_graph_item_macro",
        ITEM_MACRO_INSIDE_BODY_CALL_SPAN,
        &[],
        ExpectedCallOutcome::Unsupported,
    ),
);

paranoid_call_site_test!(
    fixture_call_graph_call_borrowed_typed_local_instance_method_resolves_borrowed_typed_local_binding_method_call_site,
    fixture: "fixture_call_graph",
    owner: function {
        module_path: &["crate"],
        name: "call_borrowed_typed_local_instance_method"
    },
    expected: {
        let target_args = fixture_call_graph_instance_method_args("instance_value");
        let parsed_graphs = crate::common::run_phases_and_collect("fixture_call_graph");
        let target_info = target_args.generate_method_pid(&parsed_graphs)?;
        ExpectedCallSite::method(
            "instance_value",
            ExpectedMethodReceiver::BorrowedTypedLocalBinding {
                name: "value",
                type_path: &["LocalAssoc"],
            },
            BORROWED_LOCAL_INSTANCE_CALL_SPAN,
            0,
            0,
            &[],
            ExpectedCallOutcome::ResolvedMethodLocalExact {
                target: target_info.test_method_id(),
            },
        )
    },
);

paranoid_call_site_test!(
    fixture_call_graph_call_dereferenced_local_instance_method_resolves_dereferenced_initialized_local_binding_method_call_site,
    fixture: "fixture_call_graph",
    owner: function {
        module_path: &["crate"],
        name: "call_dereferenced_local_instance_method"
    },
    expected: {
        let target_args = fixture_call_graph_instance_method_args("instance_value");
        let parsed_graphs = crate::common::run_phases_and_collect("fixture_call_graph");
        let target_info = target_args.generate_method_pid(&parsed_graphs)?;
        ExpectedCallSite::method(
            "instance_value",
            ExpectedMethodReceiver::DereferencedInitializedLocalBinding {
                name: "value",
                init_path: &["LocalAssoc"],
            },
            DEREFERENCED_LOCAL_INSTANCE_CALL_SPAN,
            0,
            0,
            &[],
            ExpectedCallOutcome::ResolvedMethodLocalExact {
                target: target_info.test_method_id(),
            },
        )
    },
);

paranoid_call_site_test!(
    fixture_call_graph_call_dereferenced_param_instance_method_resolves_dereferenced_param_method_call_site,
    fixture: "fixture_call_graph",
    owner: function {
        module_path: &["crate"],
        name: "call_dereferenced_param_instance_method"
    },
    expected: {
        let target_args = fixture_call_graph_instance_method_args("instance_value");
        let parsed_graphs = crate::common::run_phases_and_collect("fixture_call_graph");
        let target_info = target_args.generate_method_pid(&parsed_graphs)?;
        ExpectedCallSite::method(
            "instance_value",
            ExpectedMethodReceiver::DereferencedLocalBinding { name: "value" },
            DEREFERENCED_PARAM_INSTANCE_CALL_SPAN,
            0,
            0,
            &[],
            ExpectedCallOutcome::ResolvedMethodLocalExact {
                target: target_info.test_method_id(),
            },
        )
    },
);

paranoid_call_site_test!(
    fixture_call_graph_call_borrowed_param_instance_method_resolves_borrowed_param_method_call_site,
    fixture: "fixture_call_graph",
    owner: function {
        module_path: &["crate"],
        name: "call_borrowed_param_instance_method"
    },
    expected: {
        let target_args = fixture_call_graph_instance_method_args("instance_value");
        let parsed_graphs = crate::common::run_phases_and_collect("fixture_call_graph");
        let target_info = target_args.generate_method_pid(&parsed_graphs)?;
        ExpectedCallSite::method(
            "instance_value",
            ExpectedMethodReceiver::LocalBinding { name: "value" },
            BORROWED_PARAM_INSTANCE_CALL_SPAN,
            0,
            0,
            &[],
            ExpectedCallOutcome::ResolvedMethodLocalExact {
                target: target_info.test_method_id(),
            },
        )
    },
);

paranoid_call_site_test!(
    fixture_call_graph_call_referenced_local_instance_method_resolves_referenced_local_method_call_site,
    fixture: "fixture_call_graph",
    owner: function {
        module_path: &["crate"],
        name: "call_referenced_local_instance_method"
    },
    expected: {
        let target_args = fixture_call_graph_instance_method_args("instance_value");
        let parsed_graphs = crate::common::run_phases_and_collect("fixture_call_graph");
        let target_info = target_args.generate_method_pid(&parsed_graphs)?;
        ExpectedCallSite::method(
            "instance_value",
            ExpectedMethodReceiver::InitializedLocalBinding {
                name: "value",
                init_path: &["LocalAssoc"],
            },
            REFERENCED_LOCAL_INSTANCE_CALL_SPAN,
            0,
            0,
            &[],
            ExpectedCallOutcome::ResolvedMethodLocalExact {
                target: target_info.test_method_id(),
            },
        )
    },
);

paranoid_call_site_test!(
    fixture_call_graph_call_typed_reference_local_instance_method_resolves_typed_reference_local_method_call_site,
    fixture: "fixture_call_graph",
    owner: function {
        module_path: &["crate"],
        name: "call_typed_reference_local_instance_method"
    },
    expected: {
        let target_args = fixture_call_graph_instance_method_args("instance_value");
        let parsed_graphs = crate::common::run_phases_and_collect("fixture_call_graph");
        let target_info = target_args.generate_method_pid(&parsed_graphs)?;
        ExpectedCallSite::method(
            "instance_value",
            ExpectedMethodReceiver::TypedLocalBinding {
                name: "value",
                type_path: &["LocalAssoc"],
            },
            TYPED_REFERENCE_LOCAL_INSTANCE_CALL_SPAN,
            0,
            0,
            &[],
            ExpectedCallOutcome::ResolvedMethodLocalExact {
                target: target_info.test_method_id(),
            },
        )
    },
);

paranoid_call_site_test!(
    fixture_call_graph_call_prelude_string_new_records_external_path_call_site,
    fixture: "fixture_call_graph",
    owner: function {
        module_path: &["crate"],
        name: "call_prelude_string_new"
    },
    expected: ExpectedCallSite::path(
        &["String", "new"],
        PRELUDE_STRING_NEW_CALL_SPAN,
        0,
        0,
        &[],
        ExpectedCallOutcome::External,
    ),
);

paranoid_call_site_test!(
    fixture_call_graph_call_prelude_vec_new_records_external_path_call_site,
    fixture: "fixture_call_graph",
    owner: function {
        module_path: &["crate"],
        name: "call_prelude_vec_new"
    },
    expected: ExpectedCallSite::path(
        &["Vec", "new"],
        PRELUDE_VEC_NEW_CALL_SPAN,
        0,
        0,
        &[],
        ExpectedCallOutcome::External,
    ),
);

paranoid_call_site_test!(
    fixture_call_graph_call_path_result_instance_method_resolves_returned_type_method_call_site,
    fixture: "fixture_call_graph",
    owner: function {
        module_path: &["crate"],
        name: "call_path_result_instance_method"
    },
    expected: {
        let target_args = fixture_call_graph_instance_method_args("instance_value");
        let parsed_graphs = crate::common::run_phases_and_collect("fixture_call_graph");
        let target_info = target_args.generate_method_pid(&parsed_graphs)?;
        ExpectedCallSite::method(
            "instance_value",
            ExpectedMethodReceiver::PathCallResult {
                path: &["make_local_assoc"],
            },
            PATH_RESULT_INSTANCE_CALL_SPAN,
            0,
            0,
            &[],
            ExpectedCallOutcome::ResolvedMethodLocalExact {
                target: target_info.test_method_id(),
            },
        )
    },
);

paranoid_call_site_test!(
    fixture_call_graph_call_method_result_instance_method_resolves_returned_type_method_call_site,
    fixture: "fixture_call_graph",
    owner: function {
        module_path: &["crate"],
        name: "call_method_result_instance_method"
    },
    expected: {
        let target_args = fixture_call_graph_instance_method_args("instance_value");
        let parsed_graphs = crate::common::run_phases_and_collect("fixture_call_graph");
        let target_info = target_args.generate_method_pid(&parsed_graphs)?;
        ExpectedCallSite::method(
            "instance_value",
            ExpectedMethodReceiver::MethodCallResult {
                method_name: "clone_assoc",
            },
            METHOD_RESULT_INSTANCE_CALL_SPAN,
            0,
            0,
            &[],
            ExpectedCallOutcome::ResolvedMethodLocalExact {
                target: target_info.test_method_id(),
            },
        )
    },
);

paranoid_call_site_test!(
    fixture_call_graph_call_tuple_field_instance_method_resolves_field_type_method_call_site,
    fixture: "fixture_call_graph",
    owner: function {
        module_path: &["crate"],
        name: "call_tuple_field_instance_method"
    },
    expected: {
        let target_args = fixture_call_graph_instance_method_args("instance_value");
        let parsed_graphs = crate::common::run_phases_and_collect("fixture_call_graph");
        let target_info = target_args.generate_method_pid(&parsed_graphs)?;
        ExpectedCallSite::method(
            "instance_value",
            ExpectedMethodReceiver::FieldInitializedLocalBinding {
                name: "value",
                init_path: &["TupleFieldMethodReceiver"],
                field_path: &["0"],
            },
            TUPLE_FIELD_INSTANCE_CALL_SPAN,
            0,
            0,
            &[],
            ExpectedCallOutcome::ResolvedMethodLocalExact {
                target: target_info.test_method_id(),
            },
        )
    },
);

paranoid_call_site_test!(
    fixture_call_graph_call_tuple_field_function_resolves_constructed_field_dynamic_call_site,
    fixture: "fixture_call_graph",
    owner: function {
        module_path: &["crate"],
        name: "call_tuple_field_function"
    },
    expected: {
        let target_args = fixture_call_graph_function_args(&["crate"], "local_target");
        let parsed_graphs = crate::common::run_phases_and_collect("fixture_call_graph");
        let target_info = target_args.generate_pid(&parsed_graphs)?;
        let target = FunctionNodeId::try_from(target_info.test_pid())
            .expect("local_target should regenerate a FunctionNodeId");
        ExpectedCallSite::dynamic_field_initialized_local_binding(
            &["value", "0"],
            &["local_target"],
            TUPLE_FIELD_FUNCTION_CALL_SPAN,
            0,
            &[],
            ExpectedCallOutcome::ResolvedDynamicFunctionLocalExact { target },
        )
    },
);

paranoid_call_site_test!(
    fixture_call_graph_call_await_path_result_instance_method_resolves_returned_type_method_call_site,
    fixture: "fixture_call_graph",
    owner: function {
        module_path: &["crate"],
        name: "call_await_result_instance_method"
    },
    expected: {
        let target_args = fixture_call_graph_instance_method_args("instance_value");
        let parsed_graphs = crate::common::run_phases_and_collect("fixture_call_graph");
        let target_info = target_args.generate_method_pid(&parsed_graphs)?;
        ExpectedCallSite::method(
            "instance_value",
            ExpectedMethodReceiver::AwaitPathCallResult {
                path: &["make_ready_local_assoc"],
            },
            AWAIT_RESULT_INSTANCE_CALL_SPAN,
            0,
            0,
            &[],
            ExpectedCallOutcome::ResolvedMethodLocalExact {
                target: target_info.test_method_id(),
            },
        )
    },
);

paranoid_call_site_test!(
    fixture_call_graph_call_try_path_result_instance_method_resolves_result_ok_type_method_call_site,
    fixture: "fixture_call_graph",
    owner: function {
        module_path: &["crate"],
        name: "call_try_result_instance_method"
    },
    expected: {
        let target_args = fixture_call_graph_instance_method_args("instance_value");
        let parsed_graphs = crate::common::run_phases_and_collect("fixture_call_graph");
        let target_info = target_args.generate_method_pid(&parsed_graphs)?;
        ExpectedCallSite::method(
            "instance_value",
            ExpectedMethodReceiver::TryPathCallResult {
                path: &["try_local_assoc"],
            },
            TRY_RESULT_INSTANCE_CALL_SPAN,
            0,
            0,
            &[],
            ExpectedCallOutcome::ResolvedMethodLocalExact {
                target: target_info.test_method_id(),
            },
        )
    },
);

paranoid_call_site_test!(
    fixture_call_graph_call_literal_str_to_string_records_external_method_call_site,
    fixture: "fixture_call_graph",
    owner: function {
        module_path: &["crate"],
        name: "call_literal_str_to_string"
    },
    expected: ExpectedCallSite::method(
        "to_string",
        ExpectedMethodReceiver::Literal,
        LITERAL_TO_STRING_CALL_SPAN,
        0,
        0,
        &[],
        ExpectedCallOutcome::External,
    ),
);

paranoid_call_site_test!(
    fixture_call_graph_call_typed_vec_len_records_external_method_call_site,
    fixture: "fixture_call_graph",
    owner: function {
        module_path: &["crate"],
        name: "call_typed_vec_len_external"
    },
    expected: ExpectedCallSite::method(
        "len",
        ExpectedMethodReceiver::TypedLocalBinding {
            name: "value",
            type_path: &["Vec"],
        },
        TYPED_VEC_LEN_CALL_SPAN,
        0,
        0,
        &[],
        ExpectedCallOutcome::External,
    ),
);

paranoid_call_site_test!(
    fixture_call_graph_call_shadowed_typed_vec_len_resolves_local_method_call_site,
    fixture: "fixture_call_graph",
    owner: function {
        module_path: &["crate", "local_prelude_shadow"],
        name: "call_shadowed_typed_vec_len"
    },
    expected: {
        let target_args = fixture_call_graph_local_shadow_vec_method_args("len");
        let parsed_graphs = crate::common::run_phases_and_collect("fixture_call_graph");
        let target_info = target_args.generate_method_pid(&parsed_graphs)?;
        ExpectedCallSite::method(
            "len",
            ExpectedMethodReceiver::TypedLocalBinding {
                name: "value",
                type_path: &["Vec"],
            },
            SHADOWED_TYPED_VEC_LEN_CALL_SPAN,
            0,
            0,
            &[],
            ExpectedCallOutcome::ResolvedMethodLocalExact {
                target: target_info.test_method_id(),
            },
        )
    },
);

paranoid_call_site_test!(
    fixture_call_graph_call_unqualified_local_target_resolves_local_path_call_site,
    fixture: "fixture_call_graph",
    owner: function {
        module_path: &["crate"],
        name: "call_unqualified_local_target"
    },
    expected: {
        let target_args = fixture_call_graph_function_args(&["crate"], "local_target");
        let parsed_graphs = crate::common::run_phases_and_collect("fixture_call_graph");
        let target_info = target_args.generate_pid(&parsed_graphs)?;
        let target = FunctionNodeId::try_from(target_info.test_pid())
            .expect("local_target should regenerate a FunctionNodeId");
        ExpectedCallSite::path(
            &["local_target"],
            UNQUALIFIED_LOCAL_TARGET_CALL_SPAN,
            0,
            0,
            &[],
            ExpectedCallOutcome::ResolvedFunctionLocalExact { target },
        )
    },
);

paranoid_call_site_test!(
    fixture_call_graph_call_returned_function_resolves_inner_make_fn_path_call_site,
    fixture: "fixture_call_graph",
    owner: function {
        module_path: &["crate"],
        name: "call_returned_function"
    },
    expected: {
        let target_args = fixture_call_graph_function_args(&["crate"], "make_fn");
        let parsed_graphs = crate::common::run_phases_and_collect("fixture_call_graph");
        let target_info = target_args.generate_pid(&parsed_graphs)?;
        let target = FunctionNodeId::try_from(target_info.test_pid())
            .expect("make_fn should regenerate a FunctionNodeId");
        ExpectedCallSite::path(
            &["make_fn"],
            MAKE_FN_INNER_CALL_SPAN,
            0,
            0,
            &[],
            ExpectedCallOutcome::ResolvedFunctionLocalExact { target },
        )
    },
);

paranoid_call_site_test!(
    fixture_call_graph_call_returned_function_records_outer_dynamic_call_site,
    fixture: "fixture_call_graph",
    owner: function {
        module_path: &["crate"],
        name: "call_returned_function"
    },
    expected: ExpectedCallSite::dynamic(
        RETURNED_FUNCTION_DYNAMIC_CALL_SPAN,
        0,
        &[],
        ExpectedCallOutcome::Unsupported,
    ),
);

paranoid_call_site_test!(
    fixture_call_graph_call_chained_returned_function_resolves_inner_make_unary_fn_path_call_site,
    fixture: "fixture_call_graph",
    owner: function {
        module_path: &["crate"],
        name: "call_chained_returned_function"
    },
    expected: {
        let target_args = fixture_call_graph_function_args(&["crate"], "make_unary_fn");
        let parsed_graphs = crate::common::run_phases_and_collect("fixture_call_graph");
        let target_info = target_args.generate_pid(&parsed_graphs)?;
        let target = FunctionNodeId::try_from(target_info.test_pid())
            .expect("make_unary_fn should regenerate a FunctionNodeId");
        ExpectedCallSite::path(
            &["make_unary_fn"],
            CHAINED_RETURNED_FUNCTION_INNER_CALL_SPAN,
            0,
            0,
            &[],
            ExpectedCallOutcome::ResolvedFunctionLocalExact { target },
        )
    },
);

paranoid_call_site_test!(
    fixture_call_graph_call_chained_returned_function_records_outer_dynamic_call_site,
    fixture: "fixture_call_graph",
    owner: function {
        module_path: &["crate"],
        name: "call_chained_returned_function"
    },
    expected: ExpectedCallSite::dynamic(
        CHAINED_RETURNED_FUNCTION_DYNAMIC_CALL_SPAN,
        1,
        &[],
        ExpectedCallOutcome::Unsupported,
    ),
);

paranoid_call_site_test!(
    fixture_call_graph_call_unsafe_function_resolves_unsafe_target_path_call_site,
    fixture: "fixture_call_graph",
    owner: function {
        module_path: &["crate"],
        name: "call_unsafe_function"
    },
    expected: {
        let target_args = fixture_call_graph_function_args(&["crate"], "unsafe_target");
        let parsed_graphs = crate::common::run_phases_and_collect("fixture_call_graph");
        let target_info = target_args.generate_pid(&parsed_graphs)?;
        let target = FunctionNodeId::try_from(target_info.test_pid())
            .expect("unsafe_target should regenerate a FunctionNodeId");
        ExpectedCallSite::path(
            &["unsafe_target"],
            UNSAFE_TARGET_CALL_SPAN,
            0,
            0,
            &[],
            ExpectedCallOutcome::ResolvedFunctionLocalExact { target },
        )
    },
);

paranoid_call_site_test!(
    fixture_call_graph_call_new_type_constructor_resolves_tuple_struct_constructor_call_site,
    fixture: "fixture_call_graph",
    owner: function {
        module_path: &["crate"],
        name: "call_new_type_constructor"
    },
    expected: {
        let target_args = fixture_call_graph_struct_args("NewType");
        let parsed_graphs = crate::common::run_phases_and_collect("fixture_call_graph");
        let target_info = target_args.generate_pid(&parsed_graphs)?;
        let target = StructNodeId::try_from(target_info.test_pid())
            .expect("NewType should regenerate a StructNodeId");
        ExpectedCallSite::path(
            &["NewType"],
            NEW_TYPE_CONSTRUCTOR_CALL_SPAN,
            1,
            0,
            &[],
            ExpectedCallOutcome::ResolvedTupleStructConstructorLocalExact { target },
        )
    },
);

paranoid_call_site_test!(
    fixture_call_graph_call_self_make_resolves_self_associated_function_path_call_site,
    fixture: "fixture_call_graph",
    owner: method {
        args: fixture_call_graph_assoc_method_args("call_self_make")
    },
    expected: {
        let target_args = fixture_call_graph_assoc_method_args("make");
        let parsed_graphs = crate::common::run_phases_and_collect("fixture_call_graph");
        let target_info = target_args.generate_method_pid(&parsed_graphs)?;
        let target = target_info.test_method_id();
        ExpectedCallSite::path(
            &["Self", "make"],
            SELF_ASSOC_MAKE_CALL_SPAN,
            0,
            0,
            &[],
            ExpectedCallOutcome::ResolvedAssociatedFunctionLocalExact { target },
        )
    },
);

paranoid_call_site_test!(
    fixture_call_graph_call_local_assoc_make_resolves_type_associated_function_path_call_site,
    fixture: "fixture_call_graph",
    owner: function {
        module_path: &["crate"],
        name: "call_local_assoc_make"
    },
    expected: {
        let target_args = fixture_call_graph_assoc_method_args("make");
        let parsed_graphs = crate::common::run_phases_and_collect("fixture_call_graph");
        let target_info = target_args.generate_method_pid(&parsed_graphs)?;
        let target = target_info.test_method_id();
        ExpectedCallSite::path(
            &["LocalAssoc", "make"],
            LOCAL_ASSOC_MAKE_CALL_SPAN,
            0,
            0,
            &[],
            ExpectedCallOutcome::ResolvedAssociatedFunctionLocalExact { target },
        )
    },
);

paranoid_call_site_test!(
    fixture_call_graph_call_qualified_local_assoc_make_resolves_type_associated_function_path_call_site,
    fixture: "fixture_call_graph",
    owner: function {
        module_path: &["crate"],
        name: "call_qualified_local_assoc_make"
    },
    expected: {
        let target_args = fixture_call_graph_assoc_method_args("make");
        let parsed_graphs = crate::common::run_phases_and_collect("fixture_call_graph");
        let target_info = target_args.generate_method_pid(&parsed_graphs)?;
        let target = target_info.test_method_id();
        ExpectedCallSite::path(
            &["LocalAssoc", "make"],
            QUALIFIED_LOCAL_ASSOC_MAKE_CALL_SPAN,
            0,
            0,
            &[],
            ExpectedCallOutcome::ResolvedAssociatedFunctionLocalExact { target },
        )
    },
);

paranoid_call_site_test!(
    fixture_call_graph_call_imported_type_assoc_make_resolves_imported_type_associated_function_path_call_site,
    fixture: "fixture_call_graph",
    owner: function {
        module_path: &["crate"],
        name: "call_imported_type_assoc_make"
    },
    expected: {
        let target_args = fixture_call_graph_imported_assoc_method_args("make");
        let parsed_graphs = crate::common::run_phases_and_collect("fixture_call_graph");
        let target_info = target_args.generate_method_pid(&parsed_graphs)?;
        ExpectedCallSite::path(
            &["ImportedAssocAlias", "make"],
            IMPORTED_TYPE_ASSOC_MAKE_CALL_SPAN,
            0,
            0,
            &[],
            ExpectedCallOutcome::ResolvedAssociatedFunctionLocalExact {
                target: target_info.test_method_id(),
            },
        )
    },
);

paranoid_call_site_test!(
    fixture_call_graph_call_glob_imported_type_assoc_make_resolves_imported_type_associated_function_path_call_site,
    fixture: "fixture_call_graph",
    owner: function {
        module_path: &["crate"],
        name: "call_glob_imported_type_assoc_make"
    },
    expected: {
        let target_args = fixture_call_graph_imported_assoc_method_args("make");
        let parsed_graphs = crate::common::run_phases_and_collect("fixture_call_graph");
        let target_info = target_args.generate_method_pid(&parsed_graphs)?;
        ExpectedCallSite::path(
            &["ImportedAssoc", "make"],
            GLOB_IMPORTED_TYPE_ASSOC_MAKE_CALL_SPAN,
            0,
            0,
            &[],
            ExpectedCallOutcome::ResolvedAssociatedFunctionLocalExact {
                target: target_info.test_method_id(),
            },
        )
    },
);

paranoid_call_site_test!(
    fixture_call_graph_call_reexported_type_assoc_make_resolves_imported_type_associated_function_path_call_site,
    fixture: "fixture_call_graph",
    owner: function {
        module_path: &["crate"],
        name: "call_reexported_type_assoc_make"
    },
    expected: {
        let target_args = fixture_call_graph_imported_assoc_method_args("make");
        let parsed_graphs = crate::common::run_phases_and_collect("fixture_call_graph");
        let target_info = target_args.generate_method_pid(&parsed_graphs)?;
        ExpectedCallSite::path(
            &["ReexportedAssoc", "make"],
            REEXPORTED_TYPE_ASSOC_MAKE_CALL_SPAN,
            0,
            0,
            &[],
            ExpectedCallOutcome::ResolvedAssociatedFunctionLocalExact {
                target: target_info.test_method_id(),
            },
        )
    },
);

paranoid_call_site_test!(
    fixture_call_graph_call_imported_alias_target_resolves_imported_path_call_site,
    fixture: "fixture_call_graph",
    owner: function {
        module_path: &["crate"],
        name: "call_imported_alias_target"
    },
    expected: {
        let target_args =
            fixture_call_graph_function_args(&["crate", "import_targets"], "imported_target");
        let parsed_graphs = crate::common::run_phases_and_collect("fixture_call_graph");
        let target_info = target_args.generate_pid(&parsed_graphs)?;
        let target = FunctionNodeId::try_from(target_info.test_pid())
            .expect("imported_target should regenerate a FunctionNodeId");
        ExpectedCallSite::path(
            &["imported_alias"],
            IMPORTED_ALIAS_CALL_SPAN,
            0,
            0,
            &[],
            ExpectedCallOutcome::ResolvedFunctionLocalExact { target },
        )
    },
);

paranoid_call_site_test!(
    fixture_call_graph_call_glob_imported_target_resolves_glob_path_call_site,
    fixture: "fixture_call_graph",
    owner: function {
        module_path: &["crate"],
        name: "call_glob_imported_target"
    },
    expected: {
        let target_args =
            fixture_call_graph_function_args(&["crate", "import_targets"], "globbed_target");
        let parsed_graphs = crate::common::run_phases_and_collect("fixture_call_graph");
        let target_info = target_args.generate_pid(&parsed_graphs)?;
        let target = FunctionNodeId::try_from(target_info.test_pid())
            .expect("globbed_target should regenerate a FunctionNodeId");
        ExpectedCallSite::path(
            &["globbed_target"],
            GLOBBED_TARGET_CALL_SPAN,
            0,
            0,
            &[],
            ExpectedCallOutcome::ResolvedFunctionLocalExact { target },
        )
    },
);

paranoid_call_site_test!(
    fixture_call_graph_call_reexported_target_resolves_reexport_path_call_site,
    fixture: "fixture_call_graph",
    owner: function {
        module_path: &["crate"],
        name: "call_reexported_target"
    },
    expected: {
        let target_args =
            fixture_call_graph_function_args(&["crate", "import_targets"], "imported_target");
        let parsed_graphs = crate::common::run_phases_and_collect("fixture_call_graph");
        let target_info = target_args.generate_pid(&parsed_graphs)?;
        let target = FunctionNodeId::try_from(target_info.test_pid())
            .expect("imported_target should regenerate a FunctionNodeId");
        ExpectedCallSite::path(
            &["reexported_target"],
            REEXPORTED_TARGET_CALL_SPAN,
            0,
            0,
            &[],
            ExpectedCallOutcome::ResolvedFunctionLocalExact { target },
        )
    },
);

paranoid_call_site_test!(
    fixture_call_graph_call_imported_module_target_resolves_module_alias_path_call_site,
    fixture: "fixture_call_graph",
    owner: function {
        module_path: &["crate"],
        name: "call_imported_module_target"
    },
    expected: {
        let target_args =
            fixture_call_graph_function_args(&["crate", "import_targets"], "globbed_target");
        let parsed_graphs = crate::common::run_phases_and_collect("fixture_call_graph");
        let target_info = target_args.generate_pid(&parsed_graphs)?;
        let target = FunctionNodeId::try_from(target_info.test_pid())
            .expect("globbed_target should regenerate a FunctionNodeId");
        ExpectedCallSite::path(
            &["targets_alias", "globbed_target"],
            MODULE_ALIAS_TARGET_CALL_SPAN,
            0,
            0,
            &[],
            ExpectedCallOutcome::ResolvedFunctionLocalExact { target },
        )
    },
);

paranoid_call_site_test!(
    fixture_call_graph_call_param_instance_method_resolves_local_binding_method_call_site,
    fixture: "fixture_call_graph",
    owner: function {
        module_path: &["crate"],
        name: "call_param_instance_method"
    },
    expected: {
        let target_args = fixture_call_graph_instance_method_args("instance_value");
        let parsed_graphs = crate::common::run_phases_and_collect("fixture_call_graph");
        let target_info = target_args.generate_method_pid(&parsed_graphs)?;
        ExpectedCallSite::method(
            "instance_value",
            ExpectedMethodReceiver::LocalBinding { name: "value" },
            INSTANCE_CALL_SPAN,
            0,
            0,
            &[],
            ExpectedCallOutcome::ResolvedMethodLocalExact {
                target: target_info.test_method_id(),
            },
        )
    },
);

paranoid_call_site_test!(
    fixture_call_graph_call_typed_local_instance_method_resolves_typed_local_binding_method_call_site,
    fixture: "fixture_call_graph",
    owner: function {
        module_path: &["crate"],
        name: "call_typed_local_instance_method"
    },
    expected: {
        let target_args = fixture_call_graph_instance_method_args("instance_value");
        let parsed_graphs = crate::common::run_phases_and_collect("fixture_call_graph");
        let target_info = target_args.generate_method_pid(&parsed_graphs)?;
        ExpectedCallSite::method(
            "instance_value",
            ExpectedMethodReceiver::TypedLocalBinding {
                name: "value",
                type_path: &["LocalAssoc"],
            },
            TYPED_LOCAL_INSTANCE_CALL_SPAN,
            0,
            0,
            &[],
            ExpectedCallOutcome::ResolvedMethodLocalExact {
                target: target_info.test_method_id(),
            },
        )
    },
);

paranoid_call_site_test!(
    fixture_call_graph_call_initialized_local_instance_method_resolves_initialized_local_binding_method_call_site,
    fixture: "fixture_call_graph",
    owner: function {
        module_path: &["crate"],
        name: "call_initialized_local_instance_method"
    },
    expected: {
        let target_args = fixture_call_graph_instance_method_args("instance_value");
        let parsed_graphs = crate::common::run_phases_and_collect("fixture_call_graph");
        let target_info = target_args.generate_method_pid(&parsed_graphs)?;
        ExpectedCallSite::method(
            "instance_value",
            ExpectedMethodReceiver::InitializedLocalBinding {
                name: "value",
                init_path: &["LocalAssoc"],
            },
            INITIALIZED_LOCAL_INSTANCE_CALL_SPAN,
            0,
            0,
            &[],
            ExpectedCallOutcome::ResolvedMethodLocalExact {
                target: target_info.test_method_id(),
            },
        )
    },
);

paranoid_call_site_test!(
    fixture_call_graph_call_parenthesized_typed_local_instance_method_resolves_typed_local_binding_method_call_site,
    fixture: "fixture_call_graph",
    owner: function {
        module_path: &["crate"],
        name: "call_parenthesized_typed_local_instance_method"
    },
    expected: {
        let target_args = fixture_call_graph_instance_method_args("instance_value");
        let parsed_graphs = crate::common::run_phases_and_collect("fixture_call_graph");
        let target_info = target_args.generate_method_pid(&parsed_graphs)?;
        ExpectedCallSite::method(
            "instance_value",
            ExpectedMethodReceiver::TypedLocalBinding {
                name: "value",
                type_path: &["LocalAssoc"],
            },
            PARENTHESIZED_TYPED_LOCAL_INSTANCE_CALL_SPAN,
            0,
            0,
            &[],
            ExpectedCallOutcome::ResolvedMethodLocalExact {
                target: target_info.test_method_id(),
            },
        )
    },
);

paranoid_call_site_test!(
    fixture_call_graph_impl_assoc_const_resolves_initializer_path_call_site,
    fixture: "fixture_call_graph",
    owner: associated_const {
        args: fixture_call_graph_assoc_const_impl_args("IMPL_ASSOC_VALUE")
    },
    expected: {
        let target_args = fixture_call_graph_function_args(&["crate"], "assoc_const_value");
        let parsed_graphs = crate::common::run_phases_and_collect("fixture_call_graph");
        let target_info = target_args.generate_pid(&parsed_graphs)?;
        let target = FunctionNodeId::try_from(target_info.test_pid())
            .expect("assoc_const_value should regenerate a FunctionNodeId");
        ExpectedCallSite::path(
            &["assoc_const_value"],
            IMPL_ASSOC_CONST_CALL_SPAN,
            0,
            0,
            &[],
            ExpectedCallOutcome::ResolvedFunctionLocalExact { target },
        )
    },
);

paranoid_call_site_test!(
    fixture_call_graph_trait_assoc_const_resolves_initializer_path_call_site,
    fixture: "fixture_call_graph",
    owner: associated_const {
        args: fixture_call_graph_assoc_const_trait_args("TRAIT_ASSOC_VALUE")
    },
    expected: {
        let target_args = fixture_call_graph_function_args(&["crate"], "assoc_const_value");
        let parsed_graphs = crate::common::run_phases_and_collect("fixture_call_graph");
        let target_info = target_args.generate_pid(&parsed_graphs)?;
        let target = FunctionNodeId::try_from(target_info.test_pid())
            .expect("assoc_const_value should regenerate a FunctionNodeId");
        ExpectedCallSite::path(
            &["assoc_const_value"],
            TRAIT_ASSOC_CONST_CALL_SPAN,
            0,
            0,
            &[],
            ExpectedCallOutcome::ResolvedFunctionLocalExact { target },
        )
    },
);

paranoid_call_site_test!(
    fixture_call_graph_call_param_trait_method_resolves_local_trait_impl_method_call_site,
    fixture: "fixture_call_graph",
    owner: function {
        module_path: &["crate"],
        name: "call_param_trait_method"
    },
    expected: {
        let target_args = fixture_call_graph_trait_impl_method_args("trait_value");
        let parsed_graphs = crate::common::run_phases_and_collect("fixture_call_graph");
        let target_info = target_args.generate_method_pid(&parsed_graphs)?;
        ExpectedCallSite::method(
            "trait_value",
            ExpectedMethodReceiver::LocalBinding { name: "value" },
            PARAM_TRAIT_METHOD_CALL_SPAN,
            0,
            0,
            &[],
            ExpectedCallOutcome::ResolvedMethodLocalExact {
                target: target_info.test_method_id(),
            },
        )
    },
);

paranoid_call_site_test!(
    fixture_call_graph_call_typed_local_trait_method_resolves_local_trait_impl_method_call_site,
    fixture: "fixture_call_graph",
    owner: function {
        module_path: &["crate"],
        name: "call_typed_local_trait_method"
    },
    expected: {
        let target_args = fixture_call_graph_trait_impl_method_args("trait_value");
        let parsed_graphs = crate::common::run_phases_and_collect("fixture_call_graph");
        let target_info = target_args.generate_method_pid(&parsed_graphs)?;
        ExpectedCallSite::method(
            "trait_value",
            ExpectedMethodReceiver::TypedLocalBinding {
                name: "value",
                type_path: &["TraitDispatchTarget"],
            },
            TYPED_LOCAL_TRAIT_METHOD_CALL_SPAN,
            0,
            0,
            &[],
            ExpectedCallOutcome::ResolvedMethodLocalExact {
                target: target_info.test_method_id(),
            },
        )
    },
);

paranoid_call_site_test!(
    fixture_call_graph_call_initialized_local_trait_method_resolves_local_trait_impl_method_call_site,
    fixture: "fixture_call_graph",
    owner: function {
        module_path: &["crate"],
        name: "call_initialized_local_trait_method"
    },
    expected: {
        let target_args = fixture_call_graph_trait_impl_method_args("trait_value");
        let parsed_graphs = crate::common::run_phases_and_collect("fixture_call_graph");
        let target_info = target_args.generate_method_pid(&parsed_graphs)?;
        ExpectedCallSite::method(
            "trait_value",
            ExpectedMethodReceiver::InitializedLocalBinding {
                name: "value",
                init_path: &["TraitDispatchTarget"],
            },
            INITIALIZED_LOCAL_TRAIT_METHOD_CALL_SPAN,
            0,
            0,
            &[],
            ExpectedCallOutcome::ResolvedMethodLocalExact {
                target: target_info.test_method_id(),
            },
        )
    },
);

paranoid_call_site_test!(
    fixture_call_graph_call_parenthesized_initialized_local_trait_method_resolves_local_trait_impl_method_call_site,
    fixture: "fixture_call_graph",
    owner: function {
        module_path: &["crate"],
        name: "call_parenthesized_initialized_local_trait_method"
    },
    expected: {
        let target_args = fixture_call_graph_trait_impl_method_args("trait_value");
        let parsed_graphs = crate::common::run_phases_and_collect("fixture_call_graph");
        let target_info = target_args.generate_method_pid(&parsed_graphs)?;
        ExpectedCallSite::method(
            "trait_value",
            ExpectedMethodReceiver::InitializedLocalBinding {
                name: "value",
                init_path: &["TraitDispatchTarget"],
            },
            PARENTHESIZED_INITIALIZED_LOCAL_TRAIT_CALL_SPAN,
            0,
            0,
            &[],
            ExpectedCallOutcome::ResolvedMethodLocalExact {
                target: target_info.test_method_id(),
            },
        )
    },
);

paranoid_call_site_test!(
    fixture_call_graph_call_parenthesized_local_target_resolves_dynamic_function_call_site,
    fixture: "fixture_call_graph",
    owner: function {
        module_path: &["crate"],
        name: "call_parenthesized_local_target"
    },
    expected: {
        let target_args = fixture_call_graph_function_args(&["crate"], "local_target");
        let parsed_graphs = crate::common::run_phases_and_collect("fixture_call_graph");
        let target_info = target_args.generate_pid(&parsed_graphs)?;
        let target = FunctionNodeId::try_from(target_info.test_pid())
            .expect("local_target should regenerate a FunctionNodeId");
        ExpectedCallSite::dynamic_path(
            &["local_target"],
            PARENTHESIZED_LOCAL_TARGET_DYNAMIC_CALL_SPAN,
            0,
            &[],
            ExpectedCallOutcome::ResolvedDynamicFunctionLocalExact { target },
        )
    },
);

paranoid_call_site_test!(
    fixture_call_graph_call_function_pointer_cast_path_resolves_dynamic_function_call_site,
    fixture: "fixture_call_graph",
    owner: function {
        module_path: &["crate"],
        name: "call_function_pointer_cast_path"
    },
    expected: {
        let target_args = fixture_call_graph_function_args(&["crate"], "local_target");
        let parsed_graphs = crate::common::run_phases_and_collect("fixture_call_graph");
        let target_info = target_args.generate_pid(&parsed_graphs)?;
        let target = FunctionNodeId::try_from(target_info.test_pid())
            .expect("local_target should regenerate a FunctionNodeId");
        ExpectedCallSite::dynamic_fn_pointer_cast_path(
            &["local_target"],
            FUNCTION_POINTER_CAST_PATH_DYNAMIC_CALL_SPAN,
            0,
            &[],
            ExpectedCallOutcome::ResolvedDynamicFunctionLocalExact { target },
        )
    },
);

paranoid_call_site_test!(
    fixture_call_graph_call_function_pointer_cast_binding_resolves_initialized_dynamic_function_call_site,
    fixture: "fixture_call_graph",
    owner: function {
        module_path: &["crate"],
        name: "call_function_pointer_cast_binding"
    },
    expected: {
        let target_args = fixture_call_graph_function_args(&["crate"], "local_target");
        let parsed_graphs = crate::common::run_phases_and_collect("fixture_call_graph");
        let target_info = target_args.generate_pid(&parsed_graphs)?;
        let target = FunctionNodeId::try_from(target_info.test_pid())
            .expect("local_target should regenerate a FunctionNodeId");
        ExpectedCallSite::dynamic_fn_pointer_cast_initialized_local_binding(
            &["f"],
            &["local_target"],
            FUNCTION_POINTER_CAST_BINDING_DYNAMIC_CALL_SPAN,
            0,
            &[],
            ExpectedCallOutcome::ResolvedDynamicFunctionLocalExact { target },
        )
    },
);

paranoid_call_site_test!(
    fixture_call_graph_call_dereferenced_function_pointer_binding_resolves_initialized_dynamic_function_call_site,
    fixture: "fixture_call_graph",
    owner: function {
        module_path: &["crate"],
        name: "call_dereferenced_function_pointer_binding"
    },
    expected: {
        let target_args = fixture_call_graph_function_args(&["crate"], "local_target");
        let parsed_graphs = crate::common::run_phases_and_collect("fixture_call_graph");
        let target_info = target_args.generate_pid(&parsed_graphs)?;
        let target = FunctionNodeId::try_from(target_info.test_pid())
            .expect("local_target should regenerate a FunctionNodeId");
        ExpectedCallSite::dynamic_dereferenced_initialized_local_binding(
            &["f"],
            &["local_target"],
            DEREFERENCED_FUNCTION_POINTER_BINDING_DYNAMIC_CALL_SPAN,
            0,
            &[],
            ExpectedCallOutcome::ResolvedDynamicFunctionLocalExact { target },
        )
    },
);

paranoid_call_site_test!(
    fixture_call_graph_call_block_function_item_resolves_dynamic_function_call_site,
    fixture: "fixture_call_graph",
    owner: function {
        module_path: &["crate"],
        name: "call_block_function_item"
    },
    expected: {
        let target_args = fixture_call_graph_function_args(&["crate"], "local_target");
        let parsed_graphs = crate::common::run_phases_and_collect("fixture_call_graph");
        let target_info = target_args.generate_pid(&parsed_graphs)?;
        let target = FunctionNodeId::try_from(target_info.test_pid())
            .expect("local_target should regenerate a FunctionNodeId");
        ExpectedCallSite::dynamic_path(
            &["local_target"],
            BLOCK_FUNCTION_ITEM_DYNAMIC_CALL_SPAN,
            0,
            &[],
            ExpectedCallOutcome::ResolvedDynamicFunctionLocalExact { target },
        )
    },
);

paranoid_call_site_test!(
    fixture_call_graph_call_if_same_function_item_resolves_dynamic_function_call_site,
    fixture: "fixture_call_graph",
    owner: function {
        module_path: &["crate"],
        name: "call_if_same_function_item"
    },
    expected: {
        let target_args = fixture_call_graph_function_args(&["crate"], "local_target");
        let parsed_graphs = crate::common::run_phases_and_collect("fixture_call_graph");
        let target_info = target_args.generate_pid(&parsed_graphs)?;
        let target = FunctionNodeId::try_from(target_info.test_pid())
            .expect("local_target should regenerate a FunctionNodeId");
        ExpectedCallSite::dynamic_if_branch_paths(
            &[&["local_target"], &["local_target"]],
            IF_SAME_FUNCTION_ITEM_DYNAMIC_CALL_SPAN,
            0,
            &[],
            ExpectedCallOutcome::ResolvedDynamicFunctionLocalExact { target },
        )
    },
);

paranoid_call_site_test!(
    fixture_call_graph_call_if_ambiguous_function_item_records_ambiguous_dynamic_call_site,
    fixture: "fixture_call_graph",
    owner: function {
        module_path: &["crate"],
        name: "call_if_ambiguous_function_item"
    },
    expected: {
        ExpectedCallSite::dynamic_if_branch_paths(
            &[&["local_target"], &["other_target"]],
            IF_AMBIGUOUS_FUNCTION_ITEM_DYNAMIC_CALL_SPAN,
            0,
            &[],
            ExpectedCallOutcome::Ambiguous,
        )
    },
);

paranoid_call_site_test!(
    fixture_call_graph_call_match_same_function_item_resolves_dynamic_function_call_site,
    fixture: "fixture_call_graph",
    owner: function {
        module_path: &["crate"],
        name: "call_match_same_function_item"
    },
    expected: {
        let target_args = fixture_call_graph_function_args(&["crate"], "local_target");
        let parsed_graphs = crate::common::run_phases_and_collect("fixture_call_graph");
        let target_info = target_args.generate_pid(&parsed_graphs)?;
        let target = FunctionNodeId::try_from(target_info.test_pid())
            .expect("local_target should regenerate a FunctionNodeId");
        ExpectedCallSite::dynamic_match_arm_paths(
            &[&["local_target"], &["local_target"]],
            MATCH_SAME_FUNCTION_ITEM_DYNAMIC_CALL_SPAN,
            0,
            &[],
            ExpectedCallOutcome::ResolvedDynamicFunctionLocalExact { target },
        )
    },
);

paranoid_call_site_test!(
    fixture_call_graph_call_match_ambiguous_function_item_records_ambiguous_dynamic_call_site,
    fixture: "fixture_call_graph",
    owner: function {
        module_path: &["crate"],
        name: "call_match_ambiguous_function_item"
    },
    expected: {
        ExpectedCallSite::dynamic_match_arm_paths(
            &[&["local_target"], &["other_target"]],
            MATCH_AMBIGUOUS_FUNCTION_ITEM_DYNAMIC_CALL_SPAN,
            0,
            &[],
            ExpectedCallOutcome::Ambiguous,
        )
    },
);

paranoid_call_site_test!(
    fixture_call_graph_call_match_guarded_function_item_fails_closed_dynamic_call_site,
    fixture: "fixture_call_graph",
    owner: function {
        module_path: &["crate"],
        name: "call_match_guarded_function_item"
    },
    expected: {
        ExpectedCallSite::dynamic(
            MATCH_GUARDED_FUNCTION_ITEM_DYNAMIC_CALL_SPAN,
            0,
            &[],
            ExpectedCallOutcome::Unsupported,
        )
    },
);

paranoid_call_site_test!(
    fixture_call_graph_call_if_closure_branch_fails_closed_dynamic_call_site,
    fixture: "fixture_call_graph",
    owner: function {
        module_path: &["crate"],
        name: "call_if_closure_branch"
    },
    expected: {
        ExpectedCallSite::dynamic(
            IF_CLOSURE_BRANCH_DYNAMIC_CALL_SPAN,
            0,
            &[],
            ExpectedCallOutcome::Unsupported,
        )
    },
);

paranoid_call_site_test!(
    fixture_call_graph_call_match_closure_arm_fails_closed_dynamic_call_site,
    fixture: "fixture_call_graph",
    owner: function {
        module_path: &["crate"],
        name: "call_match_closure_arm"
    },
    expected: {
        ExpectedCallSite::dynamic(
            MATCH_CLOSURE_ARM_DYNAMIC_CALL_SPAN,
            0,
            &[],
            ExpectedCallOutcome::Unsupported,
        )
    },
);

paranoid_call_site_test!(
    fixture_call_graph_call_if_function_pointer_param_branch_fails_closed_dynamic_call_site,
    fixture: "fixture_call_graph",
    owner: function {
        module_path: &["crate"],
        name: "call_if_function_pointer_param_branch"
    },
    expected: {
        ExpectedCallSite::dynamic(
            IF_FUNCTION_POINTER_PARAM_BRANCH_DYNAMIC_CALL_SPAN,
            0,
            &[],
            ExpectedCallOutcome::Unsupported,
        )
    },
);

paranoid_call_site_test!(
    fixture_call_graph_call_match_function_pointer_param_arm_fails_closed_dynamic_call_site,
    fixture: "fixture_call_graph",
    owner: function {
        module_path: &["crate"],
        name: "call_match_function_pointer_param_arm"
    },
    expected: {
        ExpectedCallSite::dynamic(
            MATCH_FUNCTION_POINTER_PARAM_ARM_DYNAMIC_CALL_SPAN,
            0,
            &[],
            ExpectedCallOutcome::Unsupported,
        )
    },
);

paranoid_call_site_test!(
    fixture_call_graph_call_if_nested_branch_expression_fails_closed_dynamic_call_site,
    fixture: "fixture_call_graph",
    owner: function {
        module_path: &["crate"],
        name: "call_if_nested_branch_expression"
    },
    expected: {
        ExpectedCallSite::dynamic(
            IF_NESTED_BRANCH_EXPRESSION_DYNAMIC_CALL_SPAN,
            0,
            &[],
            ExpectedCallOutcome::Unsupported,
        )
    },
);

paranoid_call_site_test!(
    fixture_call_graph_call_match_nested_arm_expression_fails_closed_dynamic_call_site,
    fixture: "fixture_call_graph",
    owner: function {
        module_path: &["crate"],
        name: "call_match_nested_arm_expression"
    },
    expected: {
        ExpectedCallSite::dynamic(
            MATCH_NESTED_ARM_EXPRESSION_DYNAMIC_CALL_SPAN,
            0,
            &[],
            ExpectedCallOutcome::Unsupported,
        )
    },
);

paranoid_call_site_test!(
    fixture_call_graph_call_move_closure_literal_with_body_call_records_outer_dynamic_call_site,
    fixture: "fixture_call_graph",
    owner: function {
        module_path: &["crate"],
        name: "call_move_closure_literal_with_body_call"
    },
    expected: {
        ExpectedCallSite::dynamic(
            MOVE_CLOSURE_LITERAL_DYNAMIC_CALL_SPAN,
            0,
            &[],
            ExpectedCallOutcome::Unsupported,
        )
    },
);

paranoid_call_site_test!(
    fixture_call_graph_call_async_closure_literal_with_body_call_records_outer_dynamic_call_site,
    fixture: "fixture_call_graph",
    owner: function {
        module_path: &["crate"],
        name: "call_async_closure_literal_with_body_call"
    },
    expected: {
        ExpectedCallSite::dynamic(
            ASYNC_CLOSURE_LITERAL_DYNAMIC_CALL_SPAN,
            0,
            &[],
            ExpectedCallOutcome::Unsupported,
        )
    },
);

#[test]
fn fixture_call_graph_closure_body_call_is_not_recorded_as_outer_call_site() {
    let (graph, _tree) = crate::common::build_tree_for_tests("fixture_call_graph");
    let owner = crate::common::call_site_paranoid::function_owner_context(
        &graph,
        &["crate"],
        "closure_body_call_is_not_outer_call_site",
    );

    assert_no_call_site_owned_at_span(&graph, &owner, CLOSURE_BODY_LOCAL_TARGET_CALL_SPAN);
}

#[test]
fn fixture_call_graph_move_closure_body_call_is_not_recorded_as_outer_call_site() {
    let (graph, _tree) = crate::common::build_tree_for_tests("fixture_call_graph");
    let owner = crate::common::call_site_paranoid::function_owner_context(
        &graph,
        &["crate"],
        "call_move_closure_literal_with_body_call",
    );

    assert_no_call_site_owned_at_span(&graph, &owner, MOVE_CLOSURE_BODY_LOCAL_TARGET_CALL_SPAN);
}

#[test]
fn fixture_call_graph_async_block_call_is_not_recorded_as_outer_call_site() {
    let (graph, _tree) = crate::common::build_tree_for_tests("fixture_call_graph");
    let owner = crate::common::call_site_paranoid::function_owner_context(
        &graph,
        &["crate"],
        "async_block_call_is_not_outer_call_site",
    );

    assert_no_call_site_owned_at_span(&graph, &owner, ASYNC_BLOCK_LOCAL_TARGET_CALL_SPAN);
}

#[test]
fn fixture_call_graph_async_closure_body_call_is_not_recorded_as_outer_call_site() {
    let (graph, _tree) = crate::common::build_tree_for_tests("fixture_call_graph");
    let owner = crate::common::call_site_paranoid::function_owner_context(
        &graph,
        &["crate"],
        "call_async_closure_literal_with_body_call",
    );

    assert_no_call_site_owned_at_span(&graph, &owner, ASYNC_CLOSURE_BODY_LOCAL_TARGET_CALL_SPAN);
}

fn assert_no_call_site_owned_at_span(
    graph: &impl GraphAccess,
    owner: &CallOwnerContext,
    span: (usize, usize),
) {
    let matches = graph
        .call_sites()
        .iter()
        .filter(|call| call.owner() == owner.id && call.span() == span)
        .collect::<Vec<_>>();
    assert!(
        matches.is_empty(),
        "expected no call site owned by {} at {span:?}, found {matches:#?}",
        owner.label
    );
}

paranoid_call_site_test!(
    fixture_call_graph_call_trait_associated_function_resolves_trait_assoc_function_path_call_site,
    fixture: "fixture_call_graph",
    owner: function {
        module_path: &["crate"],
        name: "call_trait_associated_function"
    },
    expected: {
        let target_args = fixture_call_graph_assoc_function_trait_args("trait_make");
        let parsed_graphs = crate::common::run_phases_and_collect("fixture_call_graph");
        let target_info = target_args.generate_method_pid(&parsed_graphs)?;
        ExpectedCallSite::path(
            &["LocalAssocFunctionTrait", "trait_make"],
            TRAIT_ASSOC_FUNCTION_CALL_SPAN,
            0,
            0,
            &[],
            ExpectedCallOutcome::ResolvedAssociatedFunctionLocalExact {
                target: target_info.test_method_id(),
            },
        )
    },
);

paranoid_call_site_test!(
    fixture_call_graph_call_direct_imported_trait_associated_function_resolves_trait_assoc_function_path_call_site,
    fixture: "fixture_call_graph",
    owner: function {
        module_path: &["crate", "trait_assoc_function_scope", "with_direct_import"],
        name: "call_direct_imported_trait_associated_function"
    },
    expected: {
        let target_args =
            fixture_call_graph_imported_assoc_function_trait_args("imported_trait_make");
        let parsed_graphs = crate::common::run_phases_and_collect("fixture_call_graph");
        let target_info = target_args.generate_method_pid(&parsed_graphs)?;
        ExpectedCallSite::path(
            &["ImportedAssocFunctionTrait", "imported_trait_make"],
            DIRECT_IMPORTED_TRAIT_ASSOC_FUNCTION_CALL_SPAN,
            0,
            0,
            &[],
            ExpectedCallOutcome::ResolvedAssociatedFunctionLocalExact {
                target: target_info.test_method_id(),
            },
        )
    },
);

paranoid_call_site_test!(
    fixture_call_graph_call_alias_imported_trait_associated_function_resolves_trait_assoc_function_path_call_site,
    fixture: "fixture_call_graph",
    owner: function {
        module_path: &["crate", "trait_assoc_function_scope", "with_alias_import"],
        name: "call_alias_imported_trait_associated_function"
    },
    expected: {
        let target_args =
            fixture_call_graph_imported_assoc_function_trait_args("imported_trait_make");
        let parsed_graphs = crate::common::run_phases_and_collect("fixture_call_graph");
        let target_info = target_args.generate_method_pid(&parsed_graphs)?;
        ExpectedCallSite::path(
            &["VisibleAssocFunctionTrait", "imported_trait_make"],
            ALIAS_IMPORTED_TRAIT_ASSOC_FUNCTION_CALL_SPAN,
            0,
            0,
            &[],
            ExpectedCallOutcome::ResolvedAssociatedFunctionLocalExact {
                target: target_info.test_method_id(),
            },
        )
    },
);

paranoid_call_site_test!(
    fixture_call_graph_call_glob_imported_trait_associated_function_resolves_trait_assoc_function_path_call_site,
    fixture: "fixture_call_graph",
    owner: function {
        module_path: &["crate", "trait_assoc_function_scope", "with_glob_import"],
        name: "call_glob_imported_trait_associated_function"
    },
    expected: {
        let target_args =
            fixture_call_graph_imported_assoc_function_trait_args("imported_trait_make");
        let parsed_graphs = crate::common::run_phases_and_collect("fixture_call_graph");
        let target_info = target_args.generate_method_pid(&parsed_graphs)?;
        ExpectedCallSite::path(
            &["ImportedAssocFunctionTrait", "imported_trait_make"],
            GLOB_IMPORTED_TRAIT_ASSOC_FUNCTION_CALL_SPAN,
            0,
            0,
            &[],
            ExpectedCallOutcome::ResolvedAssociatedFunctionLocalExact {
                target: target_info.test_method_id(),
            },
        )
    },
);

paranoid_call_site_test!(
    fixture_call_graph_call_reexported_trait_associated_function_resolves_trait_assoc_function_path_call_site,
    fixture: "fixture_call_graph",
    owner: function {
        module_path: &["crate", "trait_assoc_reexport_scope"],
        name: "call_reexported_trait_associated_function"
    },
    expected: {
        let target_args =
            fixture_call_graph_imported_assoc_function_trait_args("imported_trait_make");
        let parsed_graphs = crate::common::run_phases_and_collect("fixture_call_graph");
        let target_info = target_args.generate_method_pid(&parsed_graphs)?;
        ExpectedCallSite::path(
            &["ReexportedAssocFunctionTrait", "imported_trait_make"],
            REEXPORTED_TRAIT_ASSOC_FUNCTION_CALL_SPAN,
            0,
            0,
            &[],
            ExpectedCallOutcome::ResolvedAssociatedFunctionLocalExact {
                target: target_info.test_method_id(),
            },
        )
    },
);

paranoid_call_site_test!(
    fixture_call_graph_call_method_as_associated_function_resolves_inherent_method_path_call_site,
    fixture: "fixture_call_graph",
    owner: function {
        module_path: &["crate"],
        name: "call_method_as_associated_function"
    },
    expected: {
        let target_args = fixture_call_graph_instance_method_args("instance_value");
        let parsed_graphs = crate::common::run_phases_and_collect("fixture_call_graph");
        let target_info = target_args.generate_method_pid(&parsed_graphs)?;
        ExpectedCallSite::path(
            &["LocalAssoc", "instance_value"],
            METHOD_AS_ASSOCIATED_FUNCTION_CALL_SPAN,
            1,
            0,
            &[],
            ExpectedCallOutcome::ResolvedAssociatedFunctionLocalExact {
                target: target_info.test_method_id(),
            },
        )
    },
);

paranoid_call_site_test!(
    fixture_call_graph_call_shadowed_local_target_binding_records_value_binding_path_call_site,
    fixture: "fixture_call_graph",
    owner: function {
        module_path: &["crate"],
        name: "call_shadowed_local_target_binding"
    },
    expected: ExpectedCallSite::path_value_binding(
        &["local_target"],
        SHADOWED_LOCAL_TARGET_BINDING_CALL_SPAN,
        0,
        0,
        &[],
        ExpectedCallOutcome::Unsupported,
    ),
);

paranoid_call_site_test!(
    fixture_call_graph_call_local_function_item_binding_resolves_initialized_value_binding_path_call_site,
    fixture: "fixture_call_graph",
    owner: function {
        module_path: &["crate"],
        name: "call_local_function_item_binding"
    },
    expected: {
        let target_args = fixture_call_graph_function_args(&["crate"], "local_target");
        let parsed_graphs = crate::common::run_phases_and_collect("fixture_call_graph");
        let target_info = target_args.generate_pid(&parsed_graphs)?;
        let target = FunctionNodeId::try_from(target_info.test_pid())
            .expect("local_target should regenerate a FunctionNodeId");
        ExpectedCallSite::path_initialized_value_binding(
            &["f"],
            &["local_target"],
            LOCAL_FUNCTION_ITEM_BINDING_CALL_SPAN,
            0,
            0,
            &[],
            ExpectedCallOutcome::ResolvedFunctionLocalExact { target },
        )
    },
);

paranoid_call_site_test!(
    fixture_call_graph_call_imported_function_item_binding_resolves_initialized_value_binding_path_call_site,
    fixture: "fixture_call_graph",
    owner: function {
        module_path: &["crate"],
        name: "call_imported_function_item_binding"
    },
    expected: {
        let target_args = fixture_call_graph_function_args(
            &["crate", "import_targets"],
            "imported_target",
        );
        let parsed_graphs = crate::common::run_phases_and_collect("fixture_call_graph");
        let target_info = target_args.generate_pid(&parsed_graphs)?;
        let target = FunctionNodeId::try_from(target_info.test_pid())
            .expect("imported_target should regenerate a FunctionNodeId");
        ExpectedCallSite::path_initialized_value_binding(
            &["f"],
            &["imported_alias"],
            IMPORTED_FUNCTION_ITEM_BINDING_CALL_SPAN,
            0,
            0,
            &[],
            ExpectedCallOutcome::ResolvedFunctionLocalExact { target },
        )
    },
);

paranoid_call_site_test!(
    fixture_call_graph_call_typed_function_pointer_binding_resolves_initialized_value_binding_path_call_site,
    fixture: "fixture_call_graph",
    owner: function {
        module_path: &["crate"],
        name: "call_typed_function_pointer_binding"
    },
    expected: {
        let target_args = fixture_call_graph_function_args(&["crate"], "local_target");
        let parsed_graphs = crate::common::run_phases_and_collect("fixture_call_graph");
        let target_info = target_args.generate_pid(&parsed_graphs)?;
        let target = FunctionNodeId::try_from(target_info.test_pid())
            .expect("local_target should regenerate a FunctionNodeId");
        ExpectedCallSite::path_initialized_value_binding(
            &["f"],
            &["local_target"],
            TYPED_FUNCTION_POINTER_BINDING_CALL_SPAN,
            0,
            0,
            &[],
            ExpectedCallOutcome::ResolvedFunctionLocalExact { target },
        )
    },
);

paranoid_call_site_test!(
    fixture_call_graph_call_typed_function_pointer_alias_binding_resolves_initialized_value_binding_path_call_site,
    fixture: "fixture_call_graph",
    owner: function {
        module_path: &["crate"],
        name: "call_typed_function_pointer_alias_binding"
    },
    expected: {
        let target_args = fixture_call_graph_function_args(&["crate"], "local_target");
        let parsed_graphs = crate::common::run_phases_and_collect("fixture_call_graph");
        let target_info = target_args.generate_pid(&parsed_graphs)?;
        let target = FunctionNodeId::try_from(target_info.test_pid())
            .expect("local_target should regenerate a FunctionNodeId");
        ExpectedCallSite::path_initialized_value_binding(
            &["g"],
            &["local_target"],
            TYPED_FUNCTION_POINTER_ALIAS_BINDING_CALL_SPAN,
            0,
            0,
            &[],
            ExpectedCallOutcome::ResolvedFunctionLocalExact { target },
        )
    },
);

paranoid_call_site_test!(
    fixture_call_graph_call_parenthesized_typed_function_pointer_alias_binding_resolves_dynamic_function_call_site,
    fixture: "fixture_call_graph",
    owner: function {
        module_path: &["crate"],
        name: "call_parenthesized_typed_function_pointer_alias_binding"
    },
    expected: {
        let target_args = fixture_call_graph_function_args(&["crate"], "local_target");
        let parsed_graphs = crate::common::run_phases_and_collect("fixture_call_graph");
        let target_info = target_args.generate_pid(&parsed_graphs)?;
        let target = FunctionNodeId::try_from(target_info.test_pid())
            .expect("local_target should regenerate a FunctionNodeId");
        ExpectedCallSite::dynamic_initialized_local_binding(
            &["g"],
            &["local_target"],
            PARENTHESIZED_TYPED_FUNCTION_POINTER_ALIAS_BINDING_CALL_SPAN,
            0,
            &[],
            ExpectedCallOutcome::ResolvedDynamicFunctionLocalExact { target },
        )
    },
);

paranoid_call_site_test!(
    fixture_call_graph_call_function_pointer_param_records_value_binding_path_call_site,
    fixture: "fixture_call_graph",
    owner: function {
        module_path: &["crate"],
        name: "call_function_pointer_param"
    },
    expected: ExpectedCallSite::path_value_binding(
        &["f"],
        FUNCTION_POINTER_PARAM_CALL_SPAN,
        0,
        0,
        &[],
        ExpectedCallOutcome::Unsupported,
    ),
);

paranoid_call_site_test!(
    fixture_call_graph_call_parenthesized_function_pointer_param_records_dynamic_local_binding_call_site,
    fixture: "fixture_call_graph",
    owner: function {
        module_path: &["crate"],
        name: "call_parenthesized_function_pointer_param"
    },
    expected: ExpectedCallSite::dynamic_local_binding(
        &["f"],
        PARENTHESIZED_FUNCTION_POINTER_PARAM_DYNAMIC_CALL_SPAN,
        0,
        &[],
        ExpectedCallOutcome::Unsupported,
    ),
);

paranoid_call_site_test!(
    fixture_call_graph_call_function_pointer_param_cast_fails_closed_dynamic_call_site,
    fixture: "fixture_call_graph",
    owner: function {
        module_path: &["crate"],
        name: "call_function_pointer_param_cast"
    },
    expected: ExpectedCallSite::dynamic(
        FUNCTION_POINTER_PARAM_CAST_DYNAMIC_CALL_SPAN,
        0,
        &[],
        ExpectedCallOutcome::Unsupported,
    ),
);

paranoid_call_site_test!(
    fixture_call_graph_call_closure_binding_cast_fails_closed_dynamic_call_site,
    fixture: "fixture_call_graph",
    owner: function {
        module_path: &["crate"],
        name: "call_closure_binding_cast"
    },
    expected: ExpectedCallSite::dynamic(
        CLOSURE_BINDING_CAST_DYNAMIC_CALL_SPAN,
        0,
        &[],
        ExpectedCallOutcome::Unsupported,
    ),
);

paranoid_call_site_test!(
    fixture_call_graph_call_dereferenced_closure_binding_fails_closed_dynamic_call_site,
    fixture: "fixture_call_graph",
    owner: function {
        module_path: &["crate"],
        name: "call_dereferenced_closure_binding"
    },
    expected: ExpectedCallSite::dynamic(
        DEREFERENCED_CLOSURE_BINDING_DYNAMIC_CALL_SPAN,
        0,
        &[],
        ExpectedCallOutcome::Unsupported,
    ),
);

paranoid_call_site_test!(
    fixture_call_graph_call_field_function_param_records_dynamic_field_call_site,
    fixture: "fixture_call_graph",
    owner: function {
        module_path: &["crate"],
        name: "call_field_function_param"
    },
    expected: ExpectedCallSite::dynamic_field_local_binding(
        &["holder", "callback"],
        FIELD_FUNCTION_PARAM_DYNAMIC_CALL_SPAN,
        0,
        &[],
        ExpectedCallOutcome::Unsupported,
    ),
);

paranoid_call_site_test!(
    fixture_call_graph_call_named_field_function_binding_resolves_initialized_field_dynamic_call_site,
    fixture: "fixture_call_graph",
    owner: function {
        module_path: &["crate"],
        name: "call_named_field_function_binding"
    },
    expected: {
        let target_args = fixture_call_graph_function_args(&["crate"], "local_target");
        let parsed_graphs = crate::common::run_phases_and_collect("fixture_call_graph");
        let target_info = target_args.generate_pid(&parsed_graphs)?;
        let target = FunctionNodeId::try_from(target_info.test_pid())
            .expect("local_target should regenerate a FunctionNodeId");
        ExpectedCallSite::dynamic_field_initialized_local_binding(
            &["holder", "callback"],
            &["local_target"],
            NAMED_FIELD_FUNCTION_DYNAMIC_CALL_SPAN,
            0,
            &[],
            ExpectedCallOutcome::ResolvedDynamicFunctionLocalExact { target },
        )
    },
);

paranoid_call_site_test!(
    fixture_call_graph_call_aliased_named_field_function_binding_resolves_dynamic_call_site,
    fixture: "fixture_call_graph",
    owner: function {
        module_path: &["crate"],
        name: "call_aliased_named_field_function_binding"
    },
    expected: {
        let target_args = fixture_call_graph_function_args(&["crate"], "local_target");
        let parsed_graphs = crate::common::run_phases_and_collect("fixture_call_graph");
        let target_info = target_args.generate_pid(&parsed_graphs)?;
        let target = FunctionNodeId::try_from(target_info.test_pid())
            .expect("local_target should regenerate a FunctionNodeId");
        ExpectedCallSite::dynamic_field_initialized_local_binding(
            &["alias", "callback"],
            &["local_target"],
            ALIASED_NAMED_FIELD_FUNCTION_DYNAMIC_CALL_SPAN,
            0,
            &[],
            ExpectedCallOutcome::ResolvedDynamicFunctionLocalExact { target },
        )
    },
);

paranoid_call_site_test!(
    fixture_call_graph_call_indexed_field_function_param_fails_closed_dynamic_call_site,
    fixture: "fixture_call_graph",
    owner: function {
        module_path: &["crate"],
        name: "call_indexed_field_function_param"
    },
    expected: ExpectedCallSite::dynamic(
        INDEXED_FIELD_FUNCTION_PARAM_DYNAMIC_CALL_SPAN,
        0,
        &[],
        ExpectedCallOutcome::Unsupported,
    ),
);

paranoid_call_site_test!(
    fixture_call_graph_call_indexed_named_field_function_binding_resolves_dynamic_call_site,
    fixture: "fixture_call_graph",
    owner: function {
        module_path: &["crate"],
        name: "call_indexed_named_field_function_binding"
    },
    expected: {
        let target_args = fixture_call_graph_function_args(&["crate"], "local_target");
        let parsed_graphs = crate::common::run_phases_and_collect("fixture_call_graph");
        let target_info = target_args.generate_pid(&parsed_graphs)?;
        let target = FunctionNodeId::try_from(target_info.test_pid())
            .expect("local_target should regenerate a FunctionNodeId");
        ExpectedCallSite::dynamic_indexed_initialized_local_binding(
            &["holder", "callbacks", "0"],
            &["local_target"],
            INDEXED_NAMED_FIELD_FUNCTION_DYNAMIC_CALL_SPAN,
            0,
            &[],
            ExpectedCallOutcome::ResolvedDynamicFunctionLocalExact { target },
        )
    },
);

paranoid_call_site_test!(
    fixture_call_graph_call_indexed_named_field_array_alias_binding_resolves_dynamic_call_site,
    fixture: "fixture_call_graph",
    owner: function {
        module_path: &["crate"],
        name: "call_indexed_named_field_array_alias_binding"
    },
    expected: {
        let target_args = fixture_call_graph_function_args(&["crate"], "local_target");
        let parsed_graphs = crate::common::run_phases_and_collect("fixture_call_graph");
        let target_info = target_args.generate_pid(&parsed_graphs)?;
        let target = FunctionNodeId::try_from(target_info.test_pid())
            .expect("local_target should regenerate a FunctionNodeId");
        ExpectedCallSite::dynamic_indexed_initialized_local_binding(
            &["holder", "callbacks", "0"],
            &["local_target"],
            INDEXED_NAMED_FIELD_ARRAY_ALIAS_DYNAMIC_CALL_SPAN,
            0,
            &[],
            ExpectedCallOutcome::ResolvedDynamicFunctionLocalExact { target },
        )
    },
);

paranoid_call_site_test!(
    fixture_call_graph_call_aliased_indexed_named_field_function_binding_resolves_dynamic_call_site,
    fixture: "fixture_call_graph",
    owner: function {
        module_path: &["crate"],
        name: "call_aliased_indexed_named_field_function_binding"
    },
    expected: {
        let target_args = fixture_call_graph_function_args(&["crate"], "local_target");
        let parsed_graphs = crate::common::run_phases_and_collect("fixture_call_graph");
        let target_info = target_args.generate_pid(&parsed_graphs)?;
        let target = FunctionNodeId::try_from(target_info.test_pid())
            .expect("local_target should regenerate a FunctionNodeId");
        ExpectedCallSite::dynamic_indexed_initialized_local_binding(
            &["alias", "callbacks", "0"],
            &["local_target"],
            ALIASED_INDEXED_NAMED_FIELD_DYNAMIC_CALL_SPAN,
            0,
            &[],
            ExpectedCallOutcome::ResolvedDynamicFunctionLocalExact { target },
        )
    },
);

paranoid_call_site_test!(
    fixture_call_graph_call_indexed_tuple_field_function_param_fails_closed_dynamic_call_site,
    fixture: "fixture_call_graph",
    owner: function {
        module_path: &["crate"],
        name: "call_indexed_tuple_field_function_param"
    },
    expected: ExpectedCallSite::dynamic(
        INDEXED_TUPLE_FIELD_FUNCTION_PARAM_DYNAMIC_CALL_SPAN,
        0,
        &[],
        ExpectedCallOutcome::Unsupported,
    ),
);

paranoid_call_site_test!(
    fixture_call_graph_call_indexed_tuple_field_function_binding_resolves_dynamic_call_site,
    fixture: "fixture_call_graph",
    owner: function {
        module_path: &["crate"],
        name: "call_indexed_tuple_field_function_binding"
    },
    expected: {
        let target_args = fixture_call_graph_function_args(&["crate"], "local_target");
        let parsed_graphs = crate::common::run_phases_and_collect("fixture_call_graph");
        let target_info = target_args.generate_pid(&parsed_graphs)?;
        let target = FunctionNodeId::try_from(target_info.test_pid())
            .expect("local_target should regenerate a FunctionNodeId");
        ExpectedCallSite::dynamic_indexed_initialized_local_binding(
            &["holder", "0", "0"],
            &["local_target"],
            INDEXED_TUPLE_FIELD_FUNCTION_DYNAMIC_CALL_SPAN,
            0,
            &[],
            ExpectedCallOutcome::ResolvedDynamicFunctionLocalExact { target },
        )
    },
);

paranoid_call_site_test!(
    fixture_call_graph_call_indexed_tuple_field_array_alias_binding_resolves_dynamic_call_site,
    fixture: "fixture_call_graph",
    owner: function {
        module_path: &["crate"],
        name: "call_indexed_tuple_field_array_alias_binding"
    },
    expected: {
        let target_args = fixture_call_graph_function_args(&["crate"], "local_target");
        let parsed_graphs = crate::common::run_phases_and_collect("fixture_call_graph");
        let target_info = target_args.generate_pid(&parsed_graphs)?;
        let target = FunctionNodeId::try_from(target_info.test_pid())
            .expect("local_target should regenerate a FunctionNodeId");
        ExpectedCallSite::dynamic_indexed_initialized_local_binding(
            &["holder", "0", "0"],
            &["local_target"],
            INDEXED_TUPLE_FIELD_ARRAY_ALIAS_DYNAMIC_CALL_SPAN,
            0,
            &[],
            ExpectedCallOutcome::ResolvedDynamicFunctionLocalExact { target },
        )
    },
);

paranoid_call_site_test!(
    fixture_call_graph_call_aliased_indexed_tuple_field_function_binding_resolves_dynamic_call_site,
    fixture: "fixture_call_graph",
    owner: function {
        module_path: &["crate"],
        name: "call_aliased_indexed_tuple_field_function_binding"
    },
    expected: {
        let target_args = fixture_call_graph_function_args(&["crate"], "local_target");
        let parsed_graphs = crate::common::run_phases_and_collect("fixture_call_graph");
        let target_info = target_args.generate_pid(&parsed_graphs)?;
        let target = FunctionNodeId::try_from(target_info.test_pid())
            .expect("local_target should regenerate a FunctionNodeId");
        ExpectedCallSite::dynamic_indexed_initialized_local_binding(
            &["alias", "0", "0"],
            &["local_target"],
            ALIASED_INDEXED_TUPLE_FIELD_DYNAMIC_CALL_SPAN,
            0,
            &[],
            ExpectedCallOutcome::ResolvedDynamicFunctionLocalExact { target },
        )
    },
);

paranoid_call_site_test!(
    fixture_call_graph_call_indexed_function_pointer_fails_closed_dynamic_call_site,
    fixture: "fixture_call_graph",
    owner: function {
        module_path: &["crate"],
        name: "call_indexed_function_pointer"
    },
    expected: ExpectedCallSite::dynamic(
        INDEXED_FUNCTION_POINTER_DYNAMIC_CALL_SPAN,
        0,
        &[],
        ExpectedCallOutcome::Unsupported,
    ),
);

paranoid_call_site_test!(
    fixture_call_graph_call_indexed_initialized_function_array_resolves_dynamic_call_site,
    fixture: "fixture_call_graph",
    owner: function {
        module_path: &["crate"],
        name: "call_indexed_initialized_function_array"
    },
    expected: {
        let target_args = fixture_call_graph_function_args(&["crate"], "local_target");
        let parsed_graphs = crate::common::run_phases_and_collect("fixture_call_graph");
        let target_info = target_args.generate_pid(&parsed_graphs)?;
        let target = FunctionNodeId::try_from(target_info.test_pid())
            .expect("local_target should regenerate a FunctionNodeId");
        ExpectedCallSite::dynamic_indexed_initialized_local_binding(
            &["funcs", "0"],
            &["local_target"],
            INDEXED_INITIALIZED_FUNCTION_DYNAMIC_CALL_SPAN,
            0,
            &[],
            ExpectedCallOutcome::ResolvedDynamicFunctionLocalExact { target },
        )
    },
);

paranoid_call_site_test!(
    fixture_call_graph_call_typed_indexed_initialized_function_array_resolves_dynamic_call_site,
    fixture: "fixture_call_graph",
    owner: function {
        module_path: &["crate"],
        name: "call_typed_indexed_initialized_function_array"
    },
    expected: {
        let target_args = fixture_call_graph_function_args(&["crate"], "local_target");
        let parsed_graphs = crate::common::run_phases_and_collect("fixture_call_graph");
        let target_info = target_args.generate_pid(&parsed_graphs)?;
        let target = FunctionNodeId::try_from(target_info.test_pid())
            .expect("local_target should regenerate a FunctionNodeId");
        ExpectedCallSite::dynamic_indexed_initialized_local_binding(
            &["funcs", "0"],
            &["local_target"],
            TYPED_INDEXED_INITIALIZED_FUNCTION_DYNAMIC_CALL_SPAN,
            0,
            &[],
            ExpectedCallOutcome::ResolvedDynamicFunctionLocalExact { target },
        )
    },
);

paranoid_call_site_test!(
    fixture_call_graph_call_aliased_indexed_initialized_function_array_resolves_dynamic_call_site,
    fixture: "fixture_call_graph",
    owner: function {
        module_path: &["crate"],
        name: "call_aliased_indexed_initialized_function_array"
    },
    expected: {
        let target_args = fixture_call_graph_function_args(&["crate"], "local_target");
        let parsed_graphs = crate::common::run_phases_and_collect("fixture_call_graph");
        let target_info = target_args.generate_pid(&parsed_graphs)?;
        let target = FunctionNodeId::try_from(target_info.test_pid())
            .expect("local_target should regenerate a FunctionNodeId");
        ExpectedCallSite::dynamic_indexed_initialized_local_binding(
            &["alias", "0"],
            &["local_target"],
            ALIASED_INDEXED_INITIALIZED_FUNCTION_DYNAMIC_CALL_SPAN,
            0,
            &[],
            ExpectedCallOutcome::ResolvedDynamicFunctionLocalExact { target },
        )
    },
);

paranoid_call_site_test!(
    fixture_call_graph_call_generic_fn_once_value_binding_records_value_binding_path_call_site,
    fixture: "fixture_call_graph",
    owner: function {
        module_path: &["crate"],
        name: "call_generic_fn_once_value_binding"
    },
    expected: ExpectedCallSite::path_value_binding(
        &["generic_f"],
        GENERIC_FN_ONCE_VALUE_BINDING_CALL_SPAN,
        0,
        0,
        &[],
        ExpectedCallOutcome::Unsupported,
    ),
);

paranoid_call_site_test!(
    fixture_call_graph_call_boxed_dyn_fn_value_binding_records_box_new_external_path_call_site,
    fixture: "fixture_call_graph",
    owner: function {
        module_path: &["crate"],
        name: "call_boxed_dyn_fn_value_binding"
    },
    expected: ExpectedCallSite::path(
        &["Box", "new"],
        BOXED_DYN_FN_BOX_NEW_CALL_SPAN,
        1,
        0,
        &[],
        ExpectedCallOutcome::External,
    ),
);

paranoid_call_site_test!(
    fixture_call_graph_call_boxed_dyn_fn_value_binding_records_value_binding_path_call_site,
    fixture: "fixture_call_graph",
    owner: function {
        module_path: &["crate"],
        name: "call_boxed_dyn_fn_value_binding"
    },
    expected: ExpectedCallSite::path_value_binding(
        &["boxed_fn"],
        BOXED_DYN_FN_VALUE_BINDING_CALL_SPAN,
        0,
        0,
        &[],
        ExpectedCallOutcome::Unsupported,
    ),
);

paranoid_call_site_test!(
    fixture_call_graph_call_parenthesized_generic_fn_once_value_binding_fails_closed_dynamic_call_site,
    fixture: "fixture_call_graph",
    owner: function {
        module_path: &["crate"],
        name: "call_parenthesized_generic_fn_once_value_binding"
    },
    expected: ExpectedCallSite::dynamic_local_binding(
        &["generic_f"],
        PARENTHESIZED_GENERIC_FN_ONCE_DYNAMIC_CALL_SPAN,
        0,
        &[],
        ExpectedCallOutcome::Unsupported,
    ),
);

paranoid_call_site_test!(
    fixture_call_graph_call_parenthesized_boxed_dyn_fn_value_binding_records_box_new_external_path_call_site,
    fixture: "fixture_call_graph",
    owner: function {
        module_path: &["crate"],
        name: "call_parenthesized_boxed_dyn_fn_value_binding"
    },
    expected: ExpectedCallSite::path(
        &["Box", "new"],
        PARENTHESIZED_BOXED_DYN_FN_BOX_NEW_CALL_SPAN,
        1,
        0,
        &[],
        ExpectedCallOutcome::External,
    ),
);

paranoid_call_site_test!(
    fixture_call_graph_call_parenthesized_boxed_dyn_fn_value_binding_fails_closed_dynamic_call_site,
    fixture: "fixture_call_graph",
    owner: function {
        module_path: &["crate"],
        name: "call_parenthesized_boxed_dyn_fn_value_binding"
    },
    expected: ExpectedCallSite::dynamic_local_binding(
        &["boxed_fn"],
        PARENTHESIZED_BOXED_DYN_FN_DYNAMIC_CALL_SPAN,
        0,
        &[],
        ExpectedCallOutcome::Unsupported,
    ),
);

paranoid_call_site_test!(
    fixture_call_graph_call_extern_c_function_records_external_path_call_site,
    fixture: "fixture_call_graph",
    owner: function {
        module_path: &["crate"],
        name: "call_extern_c_function"
    },
    expected: ExpectedCallSite::path(
        &["abs"],
        EXTERN_C_ABS_CALL_SPAN,
        1,
        0,
        &[],
        ExpectedCallOutcome::External,
    ),
);

paranoid_call_site_test!(
    fixture_call_graph_assert_eq_macro_call_records_test_body_macro_call_site,
    fixture: "fixture_call_graph",
    owner: function {
        module_path: &["crate", "call_graph_tests"],
        name: "assert_eq_macro_call"
    },
    expected: ExpectedCallSite::macro_call(
        "assert_eq",
        TEST_ASSERT_EQ_MACRO_CALL_SPAN,
        &["test"],
        ExpectedCallOutcome::Unsupported,
    ),
);

paranoid_call_site_test!(
    fixture_call_graph_call_generic_identity_turbofish_resolves_generic_function_path_call_site,
    fixture: "fixture_call_graph",
    owner: function {
        module_path: &["crate"],
        name: "call_generic_identity_turbofish"
    },
    expected: {
        let target_args = fixture_call_graph_function_args(&["crate"], "generic_identity");
        let parsed_graphs = crate::common::run_phases_and_collect("fixture_call_graph");
        let target_info = target_args.generate_pid(&parsed_graphs)?;
        let target = FunctionNodeId::try_from(target_info.test_pid())
            .expect("generic_identity should regenerate a FunctionNodeId");
        ExpectedCallSite::path(
            &["generic_identity"],
            GENERIC_IDENTITY_TURBOFISH_CALL_SPAN,
            1,
            1,
            &[],
            ExpectedCallOutcome::ResolvedFunctionLocalExact { target },
        )
    },
);

paranoid_call_site_test!(
    fixture_call_graph_call_ambiguous_trait_method_records_ambiguous_method_call_site,
    fixture: "fixture_call_graph",
    owner: function {
        module_path: &["crate"],
        name: "call_ambiguous_trait_method"
    },
    expected: ExpectedCallSite::method(
        "overlap",
        ExpectedMethodReceiver::LocalBinding { name: "value" },
        AMBIGUOUS_TRAIT_METHOD_CALL_SPAN,
        0,
        0,
        &[],
        ExpectedCallOutcome::Ambiguous,
    ),
);

paranoid_call_site_test!(
    fixture_call_graph_call_inherent_over_trait_method_resolves_inherent_method_call_site,
    fixture: "fixture_call_graph",
    owner: function {
        module_path: &["crate"],
        name: "call_inherent_over_trait_method"
    },
    expected: {
        let target_args = fixture_call_graph_inherent_precedence_method_args("priority");
        let parsed_graphs = crate::common::run_phases_and_collect("fixture_call_graph");
        let target_info = target_args.generate_method_pid(&parsed_graphs)?;
        ExpectedCallSite::method(
            "priority",
            ExpectedMethodReceiver::TypedLocalBinding {
                name: "value",
                type_path: &["InherentPrecedenceTarget"],
            },
            INHERENT_PRECEDENCE_METHOD_CALL_SPAN,
            0,
            0,
            &[],
            ExpectedCallOutcome::ResolvedMethodLocalExact {
                target: target_info.test_method_id(),
            },
        )
    },
);

paranoid_call_site_test!(
    fixture_call_graph_call_inline_generic_bound_method_resolves_trait_method_call_site,
    fixture: "fixture_call_graph",
    owner: function {
        module_path: &["crate"],
        name: "call_inline_generic_bound_method"
    },
    expected: {
        let target_args = fixture_call_graph_generic_bound_trait_args("bound_value");
        let parsed_graphs = crate::common::run_phases_and_collect("fixture_call_graph");
        let target_info = target_args.generate_method_pid(&parsed_graphs)?;
        ExpectedCallSite::method(
            "bound_value",
            ExpectedMethodReceiver::LocalBinding { name: "value" },
            INLINE_GENERIC_BOUND_METHOD_CALL_SPAN,
            0,
            0,
            &[],
            ExpectedCallOutcome::ResolvedMethodLocalExact {
                target: target_info.test_method_id(),
            },
        )
    },
);

paranoid_call_site_test!(
    fixture_call_graph_call_where_generic_bound_method_resolves_trait_method_call_site,
    fixture: "fixture_call_graph",
    owner: function {
        module_path: &["crate"],
        name: "call_where_generic_bound_method"
    },
    expected: {
        let target_args = fixture_call_graph_generic_bound_trait_args("bound_value");
        let parsed_graphs = crate::common::run_phases_and_collect("fixture_call_graph");
        let target_info = target_args.generate_method_pid(&parsed_graphs)?;
        ExpectedCallSite::method(
            "bound_value",
            ExpectedMethodReceiver::LocalBinding { name: "value" },
            WHERE_GENERIC_BOUND_METHOD_CALL_SPAN,
            0,
            0,
            &[],
            ExpectedCallOutcome::ResolvedMethodLocalExact {
                target: target_info.test_method_id(),
            },
        )
    },
);

paranoid_call_site_test!(
    fixture_call_graph_call_trait_object_method_resolves_trait_method_call_site,
    fixture: "fixture_call_graph",
    owner: function {
        module_path: &["crate"],
        name: "call_trait_object_method"
    },
    expected: {
        let target_args = fixture_call_graph_generic_bound_trait_args("bound_value");
        let parsed_graphs = crate::common::run_phases_and_collect("fixture_call_graph");
        let target_info = target_args.generate_method_pid(&parsed_graphs)?;
        ExpectedCallSite::method(
            "bound_value",
            ExpectedMethodReceiver::LocalBinding { name: "value" },
            TRAIT_OBJECT_METHOD_CALL_SPAN,
            0,
            0,
            &[],
            ExpectedCallOutcome::ResolvedMethodLocalExact {
                target: target_info.test_method_id(),
            },
        )
    },
);

paranoid_call_site_test!(
    fixture_call_graph_call_local_trait_object_binding_method_resolves_trait_method_call_site,
    fixture: "fixture_call_graph",
    owner: function {
        module_path: &["crate"],
        name: "call_local_trait_object_binding_method"
    },
    expected: {
        let target_args = fixture_call_graph_generic_bound_trait_args("bound_value");
        let parsed_graphs = crate::common::run_phases_and_collect("fixture_call_graph");
        let target_info = target_args.generate_method_pid(&parsed_graphs)?;
        ExpectedCallSite::method(
            "bound_value",
            ExpectedMethodReceiver::TypedLocalBinding {
                name: "value",
                type_path: &["GenericBoundTrait"],
            },
            LOCAL_TRAIT_OBJECT_BINDING_METHOD_CALL_SPAN,
            0,
            0,
            &[],
            ExpectedCallOutcome::ResolvedMethodLocalExact {
                target: target_info.test_method_id(),
            },
        )
    },
);

paranoid_call_site_test!(
    fixture_call_graph_call_concrete_trait_object_binding_method_resolves_impl_method_call_site,
    fixture: "fixture_call_graph",
    owner: function {
        module_path: &["crate"],
        name: "call_concrete_trait_object_binding_method"
    },
    expected: {
        let target_args = fixture_call_graph_trait_impl_method_args("trait_value");
        let parsed_graphs = crate::common::run_phases_and_collect("fixture_call_graph");
        let target_info = target_args.generate_method_pid(&parsed_graphs)?;
        ExpectedCallSite::method(
            "trait_value",
            ExpectedMethodReceiver::InitializedLocalBinding {
                name: "value",
                init_path: &["TraitDispatchTarget"],
            },
            CONCRETE_TRAIT_OBJECT_BINDING_METHOD_CALL_SPAN,
            0,
            0,
            &[],
            ExpectedCallOutcome::ResolvedMethodLocalExact {
                target: target_info.test_method_id(),
            },
        )
    },
);

paranoid_call_site_test!(
    fixture_call_graph_call_aliased_concrete_trait_object_binding_method_resolves_impl_method_call_site,
    fixture: "fixture_call_graph",
    owner: function {
        module_path: &["crate"],
        name: "call_aliased_concrete_trait_object_binding_method"
    },
    expected: {
        let target_args = fixture_call_graph_trait_impl_method_args("trait_value");
        let parsed_graphs = crate::common::run_phases_and_collect("fixture_call_graph");
        let target_info = target_args.generate_method_pid(&parsed_graphs)?;
        ExpectedCallSite::method(
            "trait_value",
            ExpectedMethodReceiver::InitializedLocalBinding {
                name: "value",
                init_path: &["TraitDispatchTarget"],
            },
            ALIASED_CONCRETE_TRAIT_OBJECT_BINDING_METHOD_CALL_SPAN,
            0,
            0,
            &[],
            ExpectedCallOutcome::ResolvedMethodLocalExact {
                target: target_info.test_method_id(),
            },
        )
    },
);

paranoid_call_site_test!(
    fixture_call_graph_call_reference_alias_trait_object_binding_method_resolves_impl_method_call_site,
    fixture: "fixture_call_graph",
    owner: function {
        module_path: &["crate"],
        name: "call_reference_alias_trait_object_binding_method"
    },
    expected: {
        let target_args = fixture_call_graph_trait_impl_method_args("trait_value");
        let parsed_graphs = crate::common::run_phases_and_collect("fixture_call_graph");
        let target_info = target_args.generate_method_pid(&parsed_graphs)?;
        ExpectedCallSite::method(
            "trait_value",
            ExpectedMethodReceiver::InitializedLocalBinding {
                name: "value",
                init_path: &["TraitDispatchTarget"],
            },
            REFERENCE_ALIAS_TRAIT_OBJECT_BINDING_METHOD_CALL_SPAN,
            0,
            0,
            &[],
            ExpectedCallOutcome::ResolvedMethodLocalExact {
                target: target_info.test_method_id(),
            },
        )
    },
);

paranoid_call_site_test!(
    fixture_call_graph_call_reference_chain_trait_object_binding_method_resolves_impl_method_call_site,
    fixture: "fixture_call_graph",
    owner: function {
        module_path: &["crate"],
        name: "call_reference_chain_trait_object_binding_method"
    },
    expected: {
        let target_args = fixture_call_graph_trait_impl_method_args("trait_value");
        let parsed_graphs = crate::common::run_phases_and_collect("fixture_call_graph");
        let target_info = target_args.generate_method_pid(&parsed_graphs)?;
        ExpectedCallSite::method(
            "trait_value",
            ExpectedMethodReceiver::InitializedLocalBinding {
                name: "value",
                init_path: &["TraitDispatchTarget"],
            },
            REFERENCE_CHAIN_TRAIT_OBJECT_BINDING_METHOD_CALL_SPAN,
            0,
            0,
            &[],
            ExpectedCallOutcome::ResolvedMethodLocalExact {
                target: target_info.test_method_id(),
            },
        )
    },
);

paranoid_call_site_test!(
    fixture_call_graph_trait_default_method_body_resolves_local_target_path_call_site,
    fixture: "fixture_call_graph",
    owner: method {
        args: fixture_call_graph_trait_default_method_args("default_calls_local")
    },
    expected: {
        let target_args = fixture_call_graph_function_args(&["crate"], "local_target");
        let parsed_graphs = crate::common::run_phases_and_collect("fixture_call_graph");
        let target_info = target_args.generate_pid(&parsed_graphs)?;
        let target = FunctionNodeId::try_from(target_info.test_pid())
            .expect("local_target should regenerate a FunctionNodeId");
        ExpectedCallSite::path(
            &["local_target"],
            TRAIT_DEFAULT_LOCAL_TARGET_CALL_SPAN,
            0,
            0,
            &[],
            ExpectedCallOutcome::ResolvedFunctionLocalExact { target },
        )
    },
);

paranoid_call_site_test!(
    fixture_call_graph_trait_impl_method_body_resolves_same_impl_self_method_call_site,
    fixture: "fixture_call_graph",
    owner: method {
        args: fixture_call_graph_trait_impl_body_call_method_args("impl_calls_required")
    },
    expected: {
        let target_args = fixture_call_graph_trait_impl_body_call_method_args("required_impl_call");
        let parsed_graphs = crate::common::run_phases_and_collect("fixture_call_graph");
        let target_info = target_args.generate_method_pid(&parsed_graphs)?;
        ExpectedCallSite::method(
            "required_impl_call",
            ExpectedMethodReceiver::SelfValue,
            TRAIT_IMPL_BODY_SELF_METHOD_CALL_SPAN,
            0,
            0,
            &[],
            ExpectedCallOutcome::ResolvedMethodLocalExact {
                target: target_info.test_method_id(),
            },
        )
    },
);

paranoid_call_site_test!(
    fixture_call_graph_trait_default_method_body_records_required_self_method_call_site,
    fixture: "fixture_call_graph",
    owner: method {
        args: fixture_call_graph_default_required_method_args("default_calls_required")
    },
    expected: {
        let target_args = fixture_call_graph_default_required_method_args("required");
        let parsed_graphs = crate::common::run_phases_and_collect("fixture_call_graph");
        let target_info = target_args.generate_method_pid(&parsed_graphs)?;
        ExpectedCallSite::method(
            "required",
            ExpectedMethodReceiver::SelfValue,
            TRAIT_DEFAULT_REQUIRED_METHOD_CALL_SPAN,
            0,
            0,
            &[],
            ExpectedCallOutcome::ResolvedMethodLocalExact {
                target: target_info.test_method_id(),
            },
        )
    },
);

paranoid_call_site_test!(
    fixture_call_graph_call_parenthesized_function_item_binding_resolves_dynamic_function_call_site,
    fixture: "fixture_call_graph",
    owner: function {
        module_path: &["crate"],
        name: "call_parenthesized_function_item_binding"
    },
    expected: {
        let target_args = fixture_call_graph_function_args(&["crate"], "local_target");
        let parsed_graphs = crate::common::run_phases_and_collect("fixture_call_graph");
        let target_info = target_args.generate_pid(&parsed_graphs)?;
        let target = FunctionNodeId::try_from(target_info.test_pid())
            .expect("local_target should regenerate a FunctionNodeId");
        ExpectedCallSite::dynamic_initialized_local_binding(
            &["f"],
            &["local_target"],
            PARENTHESIZED_FUNCTION_ITEM_BINDING_CALL_SPAN,
            0,
            &[],
            ExpectedCallOutcome::ResolvedDynamicFunctionLocalExact { target },
        )
    },
);

paranoid_call_site_test!(
    fixture_call_graph_call_aliased_function_item_binding_resolves_initialized_value_binding_path_call_site,
    fixture: "fixture_call_graph",
    owner: function {
        module_path: &["crate"],
        name: "call_aliased_function_item_binding"
    },
    expected: {
        let target_args = fixture_call_graph_function_args(&["crate"], "local_target");
        let parsed_graphs = crate::common::run_phases_and_collect("fixture_call_graph");
        let target_info = target_args.generate_pid(&parsed_graphs)?;
        let target = FunctionNodeId::try_from(target_info.test_pid())
            .expect("local_target should regenerate a FunctionNodeId");
        ExpectedCallSite::path_initialized_value_binding(
            &["g"],
            &["local_target"],
            ALIASED_FUNCTION_ITEM_BINDING_CALL_SPAN,
            0,
            0,
            &[],
            ExpectedCallOutcome::ResolvedFunctionLocalExact { target },
        )
    },
);

paranoid_call_site_test!(
    fixture_call_graph_call_parenthesized_aliased_function_item_binding_resolves_dynamic_function_call_site,
    fixture: "fixture_call_graph",
    owner: function {
        module_path: &["crate"],
        name: "call_parenthesized_aliased_function_item_binding"
    },
    expected: {
        let target_args = fixture_call_graph_function_args(&["crate"], "local_target");
        let parsed_graphs = crate::common::run_phases_and_collect("fixture_call_graph");
        let target_info = target_args.generate_pid(&parsed_graphs)?;
        let target = FunctionNodeId::try_from(target_info.test_pid())
            .expect("local_target should regenerate a FunctionNodeId");
        ExpectedCallSite::dynamic_initialized_local_binding(
            &["g"],
            &["local_target"],
            PARENTHESIZED_ALIASED_FUNCTION_ITEM_BINDING_CALL_SPAN,
            0,
            &[],
            ExpectedCallOutcome::ResolvedDynamicFunctionLocalExact { target },
        )
    },
);

paranoid_call_site_test!(
    fixture_call_graph_call_impl_trait_method_resolves_trait_method_call_site,
    fixture: "fixture_call_graph",
    owner: function {
        module_path: &["crate"],
        name: "call_impl_trait_method"
    },
    expected: {
        let target_args = fixture_call_graph_generic_bound_trait_args("bound_value");
        let parsed_graphs = crate::common::run_phases_and_collect("fixture_call_graph");
        let target_info = target_args.generate_method_pid(&parsed_graphs)?;
        ExpectedCallSite::method(
            "bound_value",
            ExpectedMethodReceiver::LocalBinding { name: "value" },
            IMPL_TRAIT_METHOD_CALL_SPAN,
            0,
            0,
            &[],
            ExpectedCallOutcome::ResolvedMethodLocalExact {
                target: target_info.test_method_id(),
            },
        )
    },
);

paranoid_call_site_test!(
    fixture_call_graph_call_direct_imported_trait_method_resolves_visible_trait_impl_method_call_site,
    fixture: "fixture_call_graph",
    owner: function {
        module_path: &["crate", "trait_scope", "with_direct_import"],
        name: "call_direct_imported_trait_method"
    },
    expected: {
        let target_args = fixture_call_graph_scoped_trait_impl_method_args("scoped_value");
        let parsed_graphs = crate::common::run_phases_and_collect("fixture_call_graph");
        let target_info = target_args.generate_method_pid(&parsed_graphs)?;
        ExpectedCallSite::method(
            "scoped_value",
            ExpectedMethodReceiver::LocalBinding { name: "value" },
            DIRECT_IMPORTED_TRAIT_METHOD_CALL_SPAN,
            0,
            0,
            &[],
            ExpectedCallOutcome::ResolvedMethodLocalExact {
                target: target_info.test_method_id(),
            },
        )
    },
);

paranoid_call_site_test!(
    fixture_call_graph_call_alias_imported_trait_method_resolves_visible_trait_impl_method_call_site,
    fixture: "fixture_call_graph",
    owner: function {
        module_path: &["crate", "trait_scope", "with_alias_import"],
        name: "call_alias_imported_trait_method"
    },
    expected: {
        let target_args = fixture_call_graph_scoped_trait_impl_method_args("scoped_value");
        let parsed_graphs = crate::common::run_phases_and_collect("fixture_call_graph");
        let target_info = target_args.generate_method_pid(&parsed_graphs)?;
        ExpectedCallSite::method(
            "scoped_value",
            ExpectedMethodReceiver::LocalBinding { name: "value" },
            ALIAS_IMPORTED_TRAIT_METHOD_CALL_SPAN,
            0,
            0,
            &[],
            ExpectedCallOutcome::ResolvedMethodLocalExact {
                target: target_info.test_method_id(),
            },
        )
    },
);

paranoid_call_site_test!(
    fixture_call_graph_call_glob_imported_trait_method_resolves_visible_trait_impl_method_call_site,
    fixture: "fixture_call_graph",
    owner: function {
        module_path: &["crate", "trait_scope", "with_glob_import"],
        name: "call_glob_imported_trait_method"
    },
    expected: {
        let target_args = fixture_call_graph_scoped_trait_impl_method_args("scoped_value");
        let parsed_graphs = crate::common::run_phases_and_collect("fixture_call_graph");
        let target_info = target_args.generate_method_pid(&parsed_graphs)?;
        ExpectedCallSite::method(
            "scoped_value",
            ExpectedMethodReceiver::LocalBinding { name: "value" },
            GLOB_IMPORTED_TRAIT_METHOD_CALL_SPAN,
            0,
            0,
            &[],
            ExpectedCallOutcome::ResolvedMethodLocalExact {
                target: target_info.test_method_id(),
            },
        )
    },
);

paranoid_call_site_test!(
    fixture_call_graph_call_unimported_trait_method_fails_closed_without_visible_trait_call_site,
    fixture: "fixture_call_graph",
    owner: function {
        module_path: &["crate", "trait_scope", "without_trait_import"],
        name: "call_unimported_trait_method"
    },
    expected: ExpectedCallSite::method(
        "scoped_value",
        ExpectedMethodReceiver::LocalBinding { name: "value" },
        UNIMPORTED_TRAIT_METHOD_CALL_SPAN,
        0,
        0,
        &[],
        ExpectedCallOutcome::Unsupported,
    ),
);

paranoid_call_site_test!(
    fixture_call_graph_call_reexported_trait_method_resolves_visible_trait_impl_method_call_site,
    fixture: "fixture_call_graph",
    owner: function {
        module_path: &["crate", "trait_reexport_scope"],
        name: "call_reexported_trait_method"
    },
    expected: {
        let target_args = fixture_call_graph_scoped_trait_impl_method_args("scoped_value");
        let parsed_graphs = crate::common::run_phases_and_collect("fixture_call_graph");
        let target_info = target_args.generate_method_pid(&parsed_graphs)?;
        ExpectedCallSite::method(
            "scoped_value",
            ExpectedMethodReceiver::LocalBinding { name: "value" },
            REEXPORTED_TRAIT_METHOD_CALL_SPAN,
            0,
            0,
            &[],
            ExpectedCallOutcome::ResolvedMethodLocalExact {
                target: target_info.test_method_id(),
            },
        )
    },
);

paranoid_call_site_test!(
    fixture_call_graph_call_grouped_imported_trait_method_resolves_visible_trait_impl_method_call_site,
    fixture: "fixture_call_graph",
    owner: function {
        module_path: &["crate", "grouped_trait_import_scope"],
        name: "call_grouped_imported_trait_method"
    },
    expected: {
        let target_args = fixture_call_graph_scoped_trait_impl_method_args("scoped_value");
        let parsed_graphs = crate::common::run_phases_and_collect("fixture_call_graph");
        let target_info = target_args.generate_method_pid(&parsed_graphs)?;
        ExpectedCallSite::method(
            "scoped_value",
            ExpectedMethodReceiver::LocalBinding { name: "value" },
            GROUPED_IMPORTED_TRAIT_METHOD_CALL_SPAN,
            0,
            0,
            &[],
            ExpectedCallOutcome::ResolvedMethodLocalExact {
                target: target_info.test_method_id(),
            },
        )
    },
);

paranoid_call_site_test!(
    fixture_call_graph_call_constrained_generic_self_trait_method_resolves_exact_generic_arg_call_site,
    fixture: "fixture_call_graph",
    owner: function {
        module_path: &["crate"],
        name: "call_constrained_generic_self_trait_method"
    },
    expected: {
        let target_args = fixture_call_graph_constrained_generic_self_impl_method_args(
            "constrained_generic_self_value",
        );
        let parsed_graphs = crate::common::run_phases_and_collect("fixture_call_graph");
        let target_info = target_args.generate_method_pid(&parsed_graphs)?;
        ExpectedCallSite::method(
            "constrained_generic_self_value",
            ExpectedMethodReceiver::LocalBinding { name: "value" },
            CONSTRAINED_GENERIC_SELF_TRAIT_METHOD_CALL_SPAN,
            0,
            0,
            &[],
            ExpectedCallOutcome::ResolvedMethodLocalExact {
                target: target_info.test_method_id(),
            },
        )
    },
);

paranoid_call_site_test!(
    fixture_call_graph_call_blanket_trait_method_resolves_blanket_impl_method_call_site,
    fixture: "fixture_call_graph",
    owner: function {
        module_path: &["crate"],
        name: "call_blanket_trait_method"
    },
    expected: {
        let target_args = fixture_call_graph_blanket_trait_impl_method_args("blanket_value");
        let parsed_graphs = crate::common::run_phases_and_collect("fixture_call_graph");
        let target_info = target_args.generate_method_pid(&parsed_graphs)?;
        ExpectedCallSite::method(
            "blanket_value",
            ExpectedMethodReceiver::LocalBinding { name: "value" },
            BLANKET_TRAIT_METHOD_CALL_SPAN,
            0,
            0,
            &[],
            ExpectedCallOutcome::ResolvedMethodLocalExact {
                target: target_info.test_method_id(),
            },
        )
    },
);

paranoid_call_site_test!(
    fixture_call_graph_call_inline_bound_blanket_trait_method_resolves_constrained_blanket_impl_call_site,
    fixture: "fixture_call_graph",
    owner: function {
        module_path: &["crate"],
        name: "call_inline_bound_blanket_trait_method"
    },
    expected: {
        let target_args = fixture_call_graph_inline_blanket_impl_method_args("inline_bound_value");
        let parsed_graphs = crate::common::run_phases_and_collect("fixture_call_graph");
        let target_info = target_args.generate_method_pid(&parsed_graphs)?;
        ExpectedCallSite::method(
            "inline_bound_value",
            ExpectedMethodReceiver::LocalBinding { name: "value" },
            INLINE_BOUND_BLANKET_TRAIT_METHOD_CALL_SPAN,
            0,
            0,
            &[],
            ExpectedCallOutcome::ResolvedMethodLocalExact {
                target: target_info.test_method_id(),
            },
        )
    },
);

paranoid_call_site_test!(
    fixture_call_graph_call_where_bound_blanket_trait_method_resolves_constrained_blanket_impl_call_site,
    fixture: "fixture_call_graph",
    owner: function {
        module_path: &["crate"],
        name: "call_where_bound_blanket_trait_method"
    },
    expected: {
        let target_args = fixture_call_graph_where_blanket_impl_method_args("where_bound_value");
        let parsed_graphs = crate::common::run_phases_and_collect("fixture_call_graph");
        let target_info = target_args.generate_method_pid(&parsed_graphs)?;
        ExpectedCallSite::method(
            "where_bound_value",
            ExpectedMethodReceiver::LocalBinding { name: "value" },
            WHERE_BOUND_BLANKET_TRAIT_METHOD_CALL_SPAN,
            0,
            0,
            &[],
            ExpectedCallOutcome::ResolvedMethodLocalExact {
                target: target_info.test_method_id(),
            },
        )
    },
);

paranoid_call_site_test!(
    fixture_call_graph_call_transitive_bound_blanket_trait_method_resolves_constrained_blanket_impl_call_site,
    fixture: "fixture_call_graph",
    owner: function {
        module_path: &["crate"],
        name: "call_transitive_bound_blanket_trait_method"
    },
    expected: {
        let target_args =
            fixture_call_graph_transitive_blanket_impl_method_args("transitive_bound_value");
        let parsed_graphs = crate::common::run_phases_and_collect("fixture_call_graph");
        let target_info = target_args.generate_method_pid(&parsed_graphs)?;
        ExpectedCallSite::method(
            "transitive_bound_value",
            ExpectedMethodReceiver::LocalBinding { name: "value" },
            TRANSITIVE_BOUND_BLANKET_TRAIT_METHOD_CALL_SPAN,
            0,
            0,
            &[],
            ExpectedCallOutcome::ResolvedMethodLocalExact {
                target: target_info.test_method_id(),
            },
        )
    },
);

paranoid_call_site_test!(
    fixture_call_graph_trait_default_method_body_resolves_same_trait_assoc_function_path_call_site,
    fixture: "fixture_call_graph",
    owner: method {
        args: fixture_call_graph_trait_default_assoc_method_args("default_calls_assoc")
    },
    expected: {
        let target_args = fixture_call_graph_trait_default_assoc_method_args("required_assoc");
        let parsed_graphs = crate::common::run_phases_and_collect("fixture_call_graph");
        let target_info = target_args.generate_method_pid(&parsed_graphs)?;
        ExpectedCallSite::path(
            &["Self", "required_assoc"],
            TRAIT_DEFAULT_ASSOC_FUNCTION_CALL_SPAN,
            0,
            0,
            &[],
            ExpectedCallOutcome::ResolvedAssociatedFunctionLocalExact {
                target: target_info.test_method_id(),
            },
        )
    },
);

paranoid_call_site_test!(
    fixture_call_graph_call_type_alias_assoc_make_resolves_aliased_type_associated_function_path_call_site,
    fixture: "fixture_call_graph",
    owner: function {
        module_path: &["crate"],
        name: "call_type_alias_assoc_make"
    },
    expected: {
        let target_args = fixture_call_graph_assoc_method_args("make");
        let parsed_graphs = crate::common::run_phases_and_collect("fixture_call_graph");
        let target_info = target_args.generate_method_pid(&parsed_graphs)?;
        ExpectedCallSite::path(
            &["LocalAssocTypeAlias", "make"],
            TYPE_ALIAS_ASSOC_MAKE_CALL_SPAN,
            0,
            0,
            &[],
            ExpectedCallOutcome::ResolvedAssociatedFunctionLocalExact {
                target: target_info.test_method_id(),
            },
        )
    },
);

paranoid_call_site_test!(
    fixture_call_graph_call_type_alias_chain_assoc_make_resolves_aliased_type_associated_function_path_call_site,
    fixture: "fixture_call_graph",
    owner: function {
        module_path: &["crate"],
        name: "call_type_alias_chain_assoc_make"
    },
    expected: {
        let target_args = fixture_call_graph_assoc_method_args("make");
        let parsed_graphs = crate::common::run_phases_and_collect("fixture_call_graph");
        let target_info = target_args.generate_method_pid(&parsed_graphs)?;
        ExpectedCallSite::path(
            &["LocalAssocAliasChain", "make"],
            TYPE_ALIAS_CHAIN_ASSOC_MAKE_CALL_SPAN,
            0,
            0,
            &[],
            ExpectedCallOutcome::ResolvedAssociatedFunctionLocalExact {
                target: target_info.test_method_id(),
            },
        )
    },
);

paranoid_call_site_test!(
    fixture_call_graph_call_type_alias_chain_instance_method_resolves_aliased_type_method_call_site,
    fixture: "fixture_call_graph",
    owner: function {
        module_path: &["crate"],
        name: "call_type_alias_chain_instance_method"
    },
    expected: {
        let target_args = fixture_call_graph_instance_method_args("instance_value");
        let parsed_graphs = crate::common::run_phases_and_collect("fixture_call_graph");
        let target_info = target_args.generate_method_pid(&parsed_graphs)?;
        ExpectedCallSite::method(
            "instance_value",
            ExpectedMethodReceiver::TypedLocalBinding {
                name: "value",
                type_path: &["LocalAssocAliasChain"],
            },
            TYPE_ALIAS_CHAIN_INSTANCE_CALL_SPAN,
            0,
            0,
            &[],
            ExpectedCallOutcome::ResolvedMethodLocalExact {
                target: target_info.test_method_id(),
            },
        )
    },
);

paranoid_call_site_test!(
    fixture_call_graph_call_imported_type_alias_assoc_make_resolves_aliased_type_associated_function_path_call_site,
    fixture: "fixture_call_graph",
    owner: function {
        module_path: &["crate"],
        name: "call_imported_type_alias_assoc_make"
    },
    expected: {
        let target_args = fixture_call_graph_assoc_method_args("make");
        let parsed_graphs = crate::common::run_phases_and_collect("fixture_call_graph");
        let target_info = target_args.generate_method_pid(&parsed_graphs)?;
        ExpectedCallSite::path(
            &["ImportedLocalAssocAlias", "make"],
            IMPORTED_TYPE_ALIAS_ASSOC_MAKE_CALL_SPAN,
            0,
            0,
            &[],
            ExpectedCallOutcome::ResolvedAssociatedFunctionLocalExact {
                target: target_info.test_method_id(),
            },
        )
    },
);

paranoid_call_site_test!(
    fixture_call_graph_call_imported_type_alias_instance_method_resolves_aliased_type_method_call_site,
    fixture: "fixture_call_graph",
    owner: function {
        module_path: &["crate"],
        name: "call_imported_type_alias_instance_method"
    },
    expected: {
        let target_args = fixture_call_graph_instance_method_args("instance_value");
        let parsed_graphs = crate::common::run_phases_and_collect("fixture_call_graph");
        let target_info = target_args.generate_method_pid(&parsed_graphs)?;
        ExpectedCallSite::method(
            "instance_value",
            ExpectedMethodReceiver::TypedLocalBinding {
                name: "value",
                type_path: &["ImportedLocalAssocAlias"],
            },
            IMPORTED_TYPE_ALIAS_INSTANCE_CALL_SPAN,
            0,
            0,
            &[],
            ExpectedCallOutcome::ResolvedMethodLocalExact {
                target: target_info.test_method_id(),
            },
        )
    },
);
