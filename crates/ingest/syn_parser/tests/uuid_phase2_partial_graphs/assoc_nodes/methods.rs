//! Exhaustive [`MethodNode`] paranoid tests for `tests/fixture_crates/fixture_nodes` (modules
//! wired from `fixture_nodes/src/lib.rs` only). Impl blocks are keyed by [`ImplNode::span`];
//! updating source lines above an `impl` can change spans and require test updates.
//!
//! **Inventory:** 16 methods from `impl` blocks, 4 trait-definition methods in `impls.rs`, and
//! 21 trait items in `traits.rs` (**41** total). Inner `impl` blocks inside `imports.rs` are not
//! represented as top-level graph items in the current visitor (0 methods); see
//! `fixture_assoc_method_node_total_matches_graph`.

#![cfg(test)]

use crate::assoc_paranoid_test_fields_and_values;
use crate::common::{AssocOwner, AssocParanoidArgs, PARSED_FIXTURE_CRATE_NODES};
use lazy_static::lazy_static;
use std::collections::HashMap;
use syn_parser::parser::graph::{GraphAccess, GraphNode};
use syn_parser::parser::nodes::ExpectedMethodNode;
use syn_parser::parser::types::VisibilityKind;

pub const LOG_TEST_ASSOC_METHOD: &str = "log_test_assoc_method";

const FIXTURE: &str = "fixture_nodes";

lazy_static! {
    static ref EXPECTED_METHOD_ARGS: HashMap<&'static str, AssocParanoidArgs<'static>> = {
        let mut m = HashMap::new();
        m.insert("crate::impls::inherent_SimpleStruct::new",
            AssocParanoidArgs {
                fixture: FIXTURE,
                relative_file_path: "src/impls.rs",
                expected_path: &["crate", "impls"],
                owner: AssocOwner::Impl { span: (520, 750) },
                ident: "new",
                expected_cfg: None,
            }
        );
        m.insert("crate::impls::inherent_SimpleStruct::private_method",
            AssocParanoidArgs {
                fixture: FIXTURE,
                relative_file_path: "src/impls.rs",
                expected_path: &["crate", "impls"],
                owner: AssocOwner::Impl { span: (520, 750) },
                ident: "private_method",
                expected_cfg: None,
            }
        );
        m.insert("crate::impls::inherent_SimpleStruct::public_method",
            AssocParanoidArgs {
                fixture: FIXTURE,
                relative_file_path: "src/impls.rs",
                expected_path: &["crate", "impls"],
                owner: AssocOwner::Impl { span: (520, 750) },
                ident: "public_method",
                expected_cfg: None,
            }
        );
        m.insert("crate::impls::inherent_PrivateStruct::get_secret_len",
            AssocParanoidArgs {
                fixture: FIXTURE,
                relative_file_path: "src/impls.rs",
                expected_path: &["crate", "impls"],
                owner: AssocOwner::Impl { span: (790, 884) },
                ident: "get_secret_len",
                expected_cfg: None,
            }
        );
        m.insert("crate::impls::inherent_GenericStruct_T::get_value_ref",
            AssocParanoidArgs {
                fixture: FIXTURE,
                relative_file_path: "src/impls.rs",
                expected_path: &["crate", "impls"],
                owner: AssocOwner::Impl { span: (968, 1062) },
                ident: "get_value_ref",
                expected_cfg: None,
            }
        );
        m.insert("crate::impls::inherent_GenericStruct_T_Debug::print_value",
            AssocParanoidArgs {
                fixture: FIXTURE,
                relative_file_path: "src/impls.rs",
                expected_path: &["crate", "impls"],
                owner: AssocOwner::Impl { span: (1106, 1217) },
                ident: "print_value",
                expected_cfg: None,
            }
        );
        m.insert("crate::impls::inherent_GenericStruct_str::get_str_len",
            AssocParanoidArgs {
                fixture: FIXTURE,
                relative_file_path: "src/impls.rs",
                expected_path: &["crate", "impls"],
                owner: AssocOwner::Impl { span: (1251, 1354) },
                ident: "get_str_len",
                expected_cfg: None,
            }
        );
        m.insert("crate::impls::impl_SimpleTrait_for_SimpleStruct::trait_method",
            AssocParanoidArgs {
                fixture: FIXTURE,
                relative_file_path: "src/impls.rs",
                expected_path: &["crate", "impls"],
                owner: AssocOwner::Impl { span: (1411, 1508) },
                ident: "trait_method",
                expected_cfg: None,
            }
        );
        m.insert("crate::impls::impl_PrivateTrait_for_SimpleStruct::private_trait_method",
            AssocParanoidArgs {
                fixture: FIXTURE,
                relative_file_path: "src/impls.rs",
                expected_path: &["crate", "impls"],
                owner: AssocOwner::Impl { span: (1543, 1679) },
                ident: "private_trait_method",
                expected_cfg: None,
            }
        );
        m.insert("crate::impls::impl_SimpleTrait_for_GenericStruct::trait_method",
            AssocParanoidArgs {
                fixture: FIXTURE,
                relative_file_path: "src/impls.rs",
                expected_path: &["crate", "impls"],
                owner: AssocOwner::Impl { span: (1732, 1885) },
                ident: "trait_method",
                expected_cfg: None,
            }
        );
        m.insert("crate::impls::impl_GenericTrait_for_GenericStruct::generic_trait_method",
            AssocParanoidArgs {
                fixture: FIXTURE,
                relative_file_path: "src/impls.rs",
                expected_path: &["crate", "impls"],
                owner: AssocOwner::Impl { span: (1928, 2097) },
                ident: "generic_trait_method",
                expected_cfg: None,
            }
        );
        m.insert("crate::impls::impl_SimpleTrait_for_i32::trait_method",
            AssocParanoidArgs {
                fixture: FIXTURE,
                relative_file_path: "src/impls.rs",
                expected_path: &["crate", "impls"],
                owner: AssocOwner::Impl { span: (2354, 2438) },
                ident: "trait_method",
                expected_cfg: None,
            }
        );
        m.insert("crate::impls::impl_AssocTrait_for_SimpleStruct::create_output",
            AssocParanoidArgs {
                fixture: FIXTURE,
                relative_file_path: "src/impls.rs",
                expected_path: &["crate", "impls"],
                owner: AssocOwner::Impl { span: (2487, 2666) },
                ident: "create_output",
                expected_cfg: None,
            }
        );
        m.insert("crate::impls::impl_SimpleTrait_for_ref_SimpleStruct::trait_method",
            AssocParanoidArgs {
                fixture: FIXTURE,
                relative_file_path: "src/impls.rs",
                expected_path: &["crate", "impls"],
                owner: AssocOwner::Impl { span: (2697, 2802) },
                ident: "trait_method",
                expected_cfg: None,
            }
        );
        m.insert("crate::impls::inner::inherent_SimpleStruct::method_in_module",
            AssocParanoidArgs {
                fixture: FIXTURE,
                relative_file_path: "src/impls.rs",
                expected_path: &["crate", "impls", "inner"],
                owner: AssocOwner::Impl { span: (2958, 3074) },
                ident: "method_in_module",
                expected_cfg: None,
            }
        );
        m.insert("crate::impls::inner::impl_SimpleTrait_for_InnerStruct::trait_method",
            AssocParanoidArgs {
                fixture: FIXTURE,
                relative_file_path: "src/impls.rs",
                expected_path: &["crate", "impls", "inner"],
                owner: AssocOwner::Impl { span: (3136, 3241) },
                ident: "trait_method",
                expected_cfg: None,
            }
        );
        m.insert("crate::impls::trait_SimpleTrait::trait_method",
            AssocParanoidArgs {
                fixture: FIXTURE,
                relative_file_path: "src/impls.rs",
                expected_path: &["crate", "impls"],
                owner: AssocOwner::Trait { trait_name: "SimpleTrait" },
                ident: "trait_method",
                expected_cfg: None,
            }
        );
        m.insert("crate::impls::trait_PrivateTrait::private_trait_method",
            AssocParanoidArgs {
                fixture: FIXTURE,
                relative_file_path: "src/impls.rs",
                expected_path: &["crate", "impls"],
                owner: AssocOwner::Trait { trait_name: "PrivateTrait" },
                ident: "private_trait_method",
                expected_cfg: None,
            }
        );
        m.insert("crate::impls::trait_GenericTrait::generic_trait_method",
            AssocParanoidArgs {
                fixture: FIXTURE,
                relative_file_path: "src/impls.rs",
                expected_path: &["crate", "impls"],
                owner: AssocOwner::Trait { trait_name: "GenericTrait" },
                ident: "generic_trait_method",
                expected_cfg: None,
            }
        );
        m.insert("crate::impls::trait_AssocTrait::create_output",
            AssocParanoidArgs {
                fixture: FIXTURE,
                relative_file_path: "src/impls.rs",
                expected_path: &["crate", "impls"],
                owner: AssocOwner::Trait { trait_name: "AssocTrait" },
                ident: "create_output",
                expected_cfg: None,
            }
        );
        m.insert("crate::traits::SimpleTrait::required_method",
            AssocParanoidArgs {
                fixture: FIXTURE,
                relative_file_path: "src/traits.rs",
                expected_path: &["crate", "traits"],
                owner: AssocOwner::Trait { trait_name: "SimpleTrait" },
                ident: "required_method",
                expected_cfg: None,
            }
        );
        m.insert("crate::traits::InternalTrait::default_method",
            AssocParanoidArgs {
                fixture: FIXTURE,
                relative_file_path: "src/traits.rs",
                expected_path: &["crate", "traits"],
                owner: AssocOwner::Trait { trait_name: "InternalTrait" },
                ident: "default_method",
                expected_cfg: None,
            }
        );
        m.insert("crate::traits::CrateTrait::crate_method",
            AssocParanoidArgs {
                fixture: FIXTURE,
                relative_file_path: "src/traits.rs",
                expected_path: &["crate", "traits"],
                owner: AssocOwner::Trait { trait_name: "CrateTrait" },
                ident: "crate_method",
                expected_cfg: None,
            }
        );
        m.insert("crate::traits::DocumentedTrait::documented_method",
            AssocParanoidArgs {
                fixture: FIXTURE,
                relative_file_path: "src/traits.rs",
                expected_path: &["crate", "traits"],
                owner: AssocOwner::Trait { trait_name: "DocumentedTrait" },
                ident: "documented_method",
                expected_cfg: None,
            }
        );
        m.insert("crate::traits::GenericTrait::process",
            AssocParanoidArgs {
                fixture: FIXTURE,
                relative_file_path: "src/traits.rs",
                expected_path: &["crate", "traits"],
                owner: AssocOwner::Trait { trait_name: "GenericTrait" },
                ident: "process",
                expected_cfg: None,
            }
        );
        m.insert("crate::traits::LifetimeTrait::get_ref",
            AssocParanoidArgs {
                fixture: FIXTURE,
                relative_file_path: "src/traits.rs",
                expected_path: &["crate", "traits"],
                owner: AssocOwner::Trait { trait_name: "LifetimeTrait" },
                ident: "get_ref",
                expected_cfg: None,
            }
        );
        m.insert("crate::traits::ComplexGenericTrait::complex_process",
            AssocParanoidArgs {
                fixture: FIXTURE,
                relative_file_path: "src/traits.rs",
                expected_path: &["crate", "traits"],
                owner: AssocOwner::Trait { trait_name: "ComplexGenericTrait" },
                ident: "complex_process",
                expected_cfg: None,
            }
        );
        m.insert("crate::traits::AssocTypeTrait::generate",
            AssocParanoidArgs {
                fixture: FIXTURE,
                relative_file_path: "src/traits.rs",
                expected_path: &["crate", "traits"],
                owner: AssocOwner::Trait { trait_name: "AssocTypeTrait" },
                ident: "generate",
                expected_cfg: None,
            }
        );
        m.insert("crate::traits::AssocTypeWithBounds::generate_bounded",
            AssocParanoidArgs {
                fixture: FIXTURE,
                relative_file_path: "src/traits.rs",
                expected_path: &["crate", "traits"],
                owner: AssocOwner::Trait { trait_name: "AssocTypeWithBounds" },
                ident: "generate_bounded",
                expected_cfg: None,
            }
        );
        m.insert("crate::traits::AssocConstTrait::get_id",
            AssocParanoidArgs {
                fixture: FIXTURE,
                relative_file_path: "src/traits.rs",
                expected_path: &["crate", "traits"],
                owner: AssocOwner::Trait { trait_name: "AssocConstTrait" },
                ident: "get_id",
                expected_cfg: None,
            }
        );
        m.insert("crate::traits::SuperTrait::super_method",
            AssocParanoidArgs {
                fixture: FIXTURE,
                relative_file_path: "src/traits.rs",
                expected_path: &["crate", "traits"],
                owner: AssocOwner::Trait { trait_name: "SuperTrait" },
                ident: "super_method",
                expected_cfg: None,
            }
        );
        m.insert("crate::traits::MultiSuperTrait::multi_super_method",
            AssocParanoidArgs {
                fixture: FIXTURE,
                relative_file_path: "src/traits.rs",
                expected_path: &["crate", "traits"],
                owner: AssocOwner::Trait { trait_name: "MultiSuperTrait" },
                ident: "multi_super_method",
                expected_cfg: None,
            }
        );
        m.insert("crate::traits::GenericSuperTrait::generic_super_method",
            AssocParanoidArgs {
                fixture: FIXTURE,
                relative_file_path: "src/traits.rs",
                expected_path: &["crate", "traits"],
                owner: AssocOwner::Trait { trait_name: "GenericSuperTrait" },
                ident: "generic_super_method",
                expected_cfg: None,
            }
        );
        m.insert("crate::traits::AttributedTrait::calculate",
            AssocParanoidArgs {
                fixture: FIXTURE,
                relative_file_path: "src/traits.rs",
                expected_path: &["crate", "traits"],
                owner: AssocOwner::Trait { trait_name: "AttributedTrait" },
                ident: "calculate",
                expected_cfg: None,
            }
        );
        m.insert("crate::traits::UnsafeTrait::unsafe_method",
            AssocParanoidArgs {
                fixture: FIXTURE,
                relative_file_path: "src/traits.rs",
                expected_path: &["crate", "traits"],
                owner: AssocOwner::Trait { trait_name: "UnsafeTrait" },
                ident: "unsafe_method",
                expected_cfg: None,
            }
        );
        m.insert("crate::traits::SelfUsageTrait::returns_self",
            AssocParanoidArgs {
                fixture: FIXTURE,
                relative_file_path: "src/traits.rs",
                expected_path: &["crate", "traits"],
                owner: AssocOwner::Trait { trait_name: "SelfUsageTrait" },
                ident: "returns_self",
                expected_cfg: None,
            }
        );
        m.insert("crate::traits::SelfUsageTrait::takes_self",
            AssocParanoidArgs {
                fixture: FIXTURE,
                relative_file_path: "src/traits.rs",
                expected_path: &["crate", "traits"],
                owner: AssocOwner::Trait { trait_name: "SelfUsageTrait" },
                ident: "takes_self",
                expected_cfg: None,
            }
        );
        m.insert("crate::traits::SelfInAssocBound::get_related",
            AssocParanoidArgs {
                fixture: FIXTURE,
                relative_file_path: "src/traits.rs",
                expected_path: &["crate", "traits"],
                owner: AssocOwner::Trait { trait_name: "SelfInAssocBound" },
                ident: "get_related",
                expected_cfg: None,
            }
        );
        m.insert("crate::traits::inner::InnerSecretTrait::secret_op",
            AssocParanoidArgs {
                fixture: FIXTURE,
                relative_file_path: "src/traits.rs",
                expected_path: &["crate", "traits", "inner"],
                owner: AssocOwner::Trait { trait_name: "InnerSecretTrait" },
                ident: "secret_op",
                expected_cfg: None,
            }
        );
        m.insert("crate::traits::inner::InnerPublicTrait::public_inner_op",
            AssocParanoidArgs {
                fixture: FIXTURE,
                relative_file_path: "src/traits.rs",
                expected_path: &["crate", "traits", "inner"],
                owner: AssocOwner::Trait { trait_name: "InnerPublicTrait" },
                ident: "public_inner_op",
                expected_cfg: None,
            }
        );
        m.insert("crate::traits::inner::SuperGraphNodeTrait::super_visible_op",
            AssocParanoidArgs {
                fixture: FIXTURE,
                relative_file_path: "src/traits.rs",
                expected_path: &["crate", "traits", "inner"],
                owner: AssocOwner::Trait { trait_name: "SuperGraphNodeTrait" },
                ident: "super_visible_op",
                expected_cfg: None,
            }
        );
        m
    };
    static ref EXPECTED_METHOD_DATA: HashMap<&'static str, ExpectedMethodNode> = {
        let mut m = HashMap::new();
        m.insert("crate::impls::inherent_SimpleStruct::new",
            ExpectedMethodNode {
            name: "new",
            visibility: VisibilityKind::Public,
            attributes: vec![],
            docstring: None,
            cfgs: vec![],
            tracking_hash_check: true,
            parameter_count: 1,
            generic_param_count: 0,
            return_type_is_some: true,
            body_is_some: true,
        }
        );
        m.insert("crate::impls::inherent_SimpleStruct::private_method",
            ExpectedMethodNode {
            name: "private_method",
            visibility: VisibilityKind::Inherited,
            attributes: vec![],
            docstring: None,
            cfgs: vec![],
            tracking_hash_check: true,
            parameter_count: 1,
            generic_param_count: 0,
            return_type_is_some: true,
            body_is_some: true,
        }
        );
        m.insert("crate::impls::inherent_SimpleStruct::public_method",
            ExpectedMethodNode {
            name: "public_method",
            visibility: VisibilityKind::Public,
            attributes: vec![],
            docstring: None,
            cfgs: vec![],
            tracking_hash_check: true,
            parameter_count: 1,
            generic_param_count: 0,
            return_type_is_some: true,
            body_is_some: true,
        }
        );
        m.insert("crate::impls::inherent_PrivateStruct::get_secret_len",
            ExpectedMethodNode {
            name: "get_secret_len",
            visibility: VisibilityKind::Inherited,
            attributes: vec![],
            docstring: None,
            cfgs: vec![],
            tracking_hash_check: true,
            parameter_count: 1,
            generic_param_count: 0,
            return_type_is_some: true,
            body_is_some: true,
        }
        );
        m.insert("crate::impls::inherent_GenericStruct_T::get_value_ref",
            ExpectedMethodNode {
            name: "get_value_ref",
            visibility: VisibilityKind::Public,
            attributes: vec![],
            docstring: None,
            cfgs: vec![],
            tracking_hash_check: true,
            parameter_count: 1,
            generic_param_count: 0,
            return_type_is_some: true,
            body_is_some: true,
        }
        );
        m.insert("crate::impls::inherent_GenericStruct_T_Debug::print_value",
            ExpectedMethodNode {
            name: "print_value",
            visibility: VisibilityKind::Public,
            attributes: vec![],
            docstring: None,
            cfgs: vec![],
            tracking_hash_check: true,
            parameter_count: 1,
            generic_param_count: 0,
            return_type_is_some: false,
            body_is_some: true,
        }
        );
        m.insert("crate::impls::inherent_GenericStruct_str::get_str_len",
            ExpectedMethodNode {
            name: "get_str_len",
            visibility: VisibilityKind::Inherited,
            attributes: vec![],
            docstring: None,
            cfgs: vec![],
            tracking_hash_check: true,
            parameter_count: 1,
            generic_param_count: 0,
            return_type_is_some: true,
            body_is_some: true,
        }
        );
        m.insert("crate::impls::impl_SimpleTrait_for_SimpleStruct::trait_method",
            ExpectedMethodNode {
            name: "trait_method",
            visibility: VisibilityKind::Inherited,
            attributes: vec![],
            docstring: None,
            cfgs: vec![],
            tracking_hash_check: true,
            parameter_count: 1,
            generic_param_count: 0,
            return_type_is_some: true,
            body_is_some: true,
        }
        );
        m.insert("crate::impls::impl_PrivateTrait_for_SimpleStruct::private_trait_method",
            ExpectedMethodNode {
            name: "private_trait_method",
            visibility: VisibilityKind::Inherited,
            attributes: vec![],
            docstring: None,
            cfgs: vec![],
            tracking_hash_check: true,
            parameter_count: 1,
            generic_param_count: 0,
            return_type_is_some: false,
            body_is_some: true,
        }
        );
        m.insert("crate::impls::impl_SimpleTrait_for_GenericStruct::trait_method",
            ExpectedMethodNode {
            name: "trait_method",
            visibility: VisibilityKind::Inherited,
            attributes: vec![],
            docstring: None,
            cfgs: vec![],
            tracking_hash_check: true,
            parameter_count: 1,
            generic_param_count: 0,
            return_type_is_some: true,
            body_is_some: true,
        }
        );
        m.insert("crate::impls::impl_GenericTrait_for_GenericStruct::generic_trait_method",
            ExpectedMethodNode {
            name: "generic_trait_method",
            visibility: VisibilityKind::Inherited,
            attributes: vec![],
            docstring: None,
            cfgs: vec![],
            tracking_hash_check: true,
            parameter_count: 2,
            generic_param_count: 0,
            return_type_is_some: false,
            body_is_some: true,
        }
        );
        m.insert("crate::impls::impl_SimpleTrait_for_i32::trait_method",
            ExpectedMethodNode {
            name: "trait_method",
            visibility: VisibilityKind::Inherited,
            attributes: vec![],
            docstring: None,
            cfgs: vec![],
            tracking_hash_check: true,
            parameter_count: 1,
            generic_param_count: 0,
            return_type_is_some: true,
            body_is_some: true,
        }
        );
        m.insert("crate::impls::impl_AssocTrait_for_SimpleStruct::create_output",
            ExpectedMethodNode {
            name: "create_output",
            visibility: VisibilityKind::Inherited,
            attributes: vec![],
            docstring: None,
            cfgs: vec![],
            tracking_hash_check: true,
            parameter_count: 1,
            generic_param_count: 0,
            return_type_is_some: true,
            body_is_some: true,
        }
        );
        m.insert("crate::impls::impl_SimpleTrait_for_ref_SimpleStruct::trait_method",
            ExpectedMethodNode {
            name: "trait_method",
            visibility: VisibilityKind::Inherited,
            attributes: vec![],
            docstring: None,
            cfgs: vec![],
            tracking_hash_check: true,
            parameter_count: 1,
            generic_param_count: 0,
            return_type_is_some: true,
            body_is_some: true,
        }
        );
        m.insert("crate::impls::inner::inherent_SimpleStruct::method_in_module",
            ExpectedMethodNode {
            name: "method_in_module",
            visibility: VisibilityKind::Restricted(vec!["super".to_string()]),
            attributes: vec![],
            docstring: None,
            cfgs: vec![],
            tracking_hash_check: true,
            parameter_count: 1,
            generic_param_count: 0,
            return_type_is_some: true,
            body_is_some: true,
        }
        );
        m.insert("crate::impls::inner::impl_SimpleTrait_for_InnerStruct::trait_method",
            ExpectedMethodNode {
            name: "trait_method",
            visibility: VisibilityKind::Inherited,
            attributes: vec![],
            docstring: None,
            cfgs: vec![],
            tracking_hash_check: true,
            parameter_count: 1,
            generic_param_count: 0,
            return_type_is_some: true,
            body_is_some: true,
        }
        );
        m.insert("crate::impls::trait_SimpleTrait::trait_method",
            ExpectedMethodNode {
            name: "trait_method",
            visibility: VisibilityKind::Public,
            attributes: vec![],
            docstring: None,
            cfgs: vec![],
            tracking_hash_check: true,
            parameter_count: 1,
            generic_param_count: 0,
            return_type_is_some: true,
            body_is_some: false,
        }
        );
        m.insert("crate::impls::trait_PrivateTrait::private_trait_method",
            ExpectedMethodNode {
            name: "private_trait_method",
            visibility: VisibilityKind::Inherited,
            attributes: vec![],
            docstring: None,
            cfgs: vec![],
            tracking_hash_check: true,
            parameter_count: 1,
            generic_param_count: 0,
            return_type_is_some: false,
            body_is_some: false,
        }
        );
        m.insert("crate::impls::trait_GenericTrait::generic_trait_method",
            ExpectedMethodNode {
            name: "generic_trait_method",
            visibility: VisibilityKind::Public,
            attributes: vec![],
            docstring: None,
            cfgs: vec![],
            tracking_hash_check: true,
            parameter_count: 2,
            generic_param_count: 0,
            return_type_is_some: false,
            body_is_some: false,
        }
        );
        m.insert("crate::impls::trait_AssocTrait::create_output",
            ExpectedMethodNode {
            name: "create_output",
            visibility: VisibilityKind::Public,
            attributes: vec![],
            docstring: None,
            cfgs: vec![],
            tracking_hash_check: true,
            parameter_count: 1,
            generic_param_count: 0,
            return_type_is_some: true,
            body_is_some: false,
        }
        );
        m.insert("crate::traits::SimpleTrait::required_method",
            ExpectedMethodNode {
            name: "required_method",
            visibility: VisibilityKind::Public,
            attributes: vec![],
            docstring: None,
            cfgs: vec![],
            tracking_hash_check: true,
            parameter_count: 1,
            generic_param_count: 0,
            return_type_is_some: true,
            body_is_some: false,
        }
        );
        m.insert("crate::traits::InternalTrait::default_method",
            ExpectedMethodNode {
            name: "default_method",
            visibility: VisibilityKind::Inherited,
            attributes: vec![],
            docstring: None,
            cfgs: vec![],
            tracking_hash_check: true,
            parameter_count: 1,
            generic_param_count: 0,
            return_type_is_some: true,
            body_is_some: true,
        }
        );
        m.insert("crate::traits::CrateTrait::crate_method",
            ExpectedMethodNode {
            name: "crate_method",
            visibility: VisibilityKind::Crate,
            attributes: vec![],
            docstring: None,
            cfgs: vec![],
            tracking_hash_check: true,
            parameter_count: 1,
            generic_param_count: 0,
            return_type_is_some: false,
            body_is_some: false,
        }
        );
        m.insert("crate::traits::DocumentedTrait::documented_method",
            ExpectedMethodNode {
            name: "documented_method",
            visibility: VisibilityKind::Public,
            attributes: vec![],
            docstring: Some("Required method documentation"),
            cfgs: vec![],
            tracking_hash_check: true,
            parameter_count: 1,
            generic_param_count: 0,
            return_type_is_some: false,
            body_is_some: false,
        }
        );
        m.insert("crate::traits::GenericTrait::process",
            ExpectedMethodNode {
            name: "process",
            visibility: VisibilityKind::Public,
            attributes: vec![],
            docstring: None,
            cfgs: vec![],
            tracking_hash_check: true,
            parameter_count: 2,
            generic_param_count: 0,
            return_type_is_some: true,
            body_is_some: false,
        }
        );
        m.insert("crate::traits::LifetimeTrait::get_ref",
            ExpectedMethodNode {
            name: "get_ref",
            visibility: VisibilityKind::Public,
            attributes: vec![],
            docstring: None,
            cfgs: vec![],
            tracking_hash_check: true,
            parameter_count: 1,
            generic_param_count: 0,
            return_type_is_some: true,
            body_is_some: false,
        }
        );
        m.insert("crate::traits::ComplexGenericTrait::complex_process",
            ExpectedMethodNode {
            name: "complex_process",
            visibility: VisibilityKind::Public,
            attributes: vec![],
            docstring: None,
            cfgs: vec![],
            tracking_hash_check: true,
            parameter_count: 3,
            generic_param_count: 0,
            return_type_is_some: true,
            body_is_some: false,
        }
        );
        m.insert("crate::traits::AssocTypeTrait::generate",
            ExpectedMethodNode {
            name: "generate",
            visibility: VisibilityKind::Public,
            attributes: vec![],
            docstring: None,
            cfgs: vec![],
            tracking_hash_check: true,
            parameter_count: 1,
            generic_param_count: 0,
            return_type_is_some: true,
            body_is_some: false,
        }
        );
        m.insert("crate::traits::AssocTypeWithBounds::generate_bounded",
            ExpectedMethodNode {
            name: "generate_bounded",
            visibility: VisibilityKind::Public,
            attributes: vec![],
            docstring: None,
            cfgs: vec![],
            tracking_hash_check: true,
            parameter_count: 1,
            generic_param_count: 0,
            return_type_is_some: true,
            body_is_some: false,
        }
        );
        m.insert("crate::traits::AssocConstTrait::get_id",
            ExpectedMethodNode {
            name: "get_id",
            visibility: VisibilityKind::Public,
            attributes: vec![],
            docstring: None,
            cfgs: vec![],
            tracking_hash_check: true,
            parameter_count: 1,
            generic_param_count: 0,
            return_type_is_some: true,
            body_is_some: true,
        }
        );
        m.insert("crate::traits::SuperTrait::super_method",
            ExpectedMethodNode {
            name: "super_method",
            visibility: VisibilityKind::Public,
            attributes: vec![],
            docstring: None,
            cfgs: vec![],
            tracking_hash_check: true,
            parameter_count: 1,
            generic_param_count: 0,
            return_type_is_some: false,
            body_is_some: false,
        }
        );
        m.insert("crate::traits::MultiSuperTrait::multi_super_method",
            ExpectedMethodNode {
            name: "multi_super_method",
            visibility: VisibilityKind::Public,
            attributes: vec![],
            docstring: None,
            cfgs: vec![],
            tracking_hash_check: true,
            parameter_count: 1,
            generic_param_count: 0,
            return_type_is_some: false,
            body_is_some: false,
        }
        );
        m.insert("crate::traits::GenericSuperTrait::generic_super_method",
            ExpectedMethodNode {
            name: "generic_super_method",
            visibility: VisibilityKind::Public,
            attributes: vec![],
            docstring: None,
            cfgs: vec![],
            tracking_hash_check: true,
            parameter_count: 2,
            generic_param_count: 0,
            return_type_is_some: false,
            body_is_some: false,
        }
        );
        m.insert("crate::traits::AttributedTrait::calculate",
            ExpectedMethodNode {
            name: "calculate",
            visibility: VisibilityKind::Public,
            attributes: vec![],
            docstring: None,
            cfgs: vec![],
            tracking_hash_check: true,
            parameter_count: 1,
            generic_param_count: 0,
            return_type_is_some: true,
            body_is_some: false,
        }
        );
        m.insert("crate::traits::UnsafeTrait::unsafe_method",
            ExpectedMethodNode {
            name: "unsafe_method",
            visibility: VisibilityKind::Public,
            attributes: vec![],
            docstring: None,
            cfgs: vec![],
            tracking_hash_check: true,
            parameter_count: 1,
            generic_param_count: 0,
            return_type_is_some: false,
            body_is_some: false,
        }
        );
        m.insert("crate::traits::SelfUsageTrait::returns_self",
            ExpectedMethodNode {
            name: "returns_self",
            visibility: VisibilityKind::Public,
            attributes: vec![],
            docstring: None,
            cfgs: vec![],
            tracking_hash_check: true,
            parameter_count: 1,
            generic_param_count: 0,
            return_type_is_some: true,
            body_is_some: false,
        }
        );
        m.insert("crate::traits::SelfUsageTrait::takes_self",
            ExpectedMethodNode {
            name: "takes_self",
            visibility: VisibilityKind::Public,
            attributes: vec![],
            docstring: None,
            cfgs: vec![],
            tracking_hash_check: true,
            parameter_count: 2,
            generic_param_count: 0,
            return_type_is_some: false,
            body_is_some: false,
        }
        );
        m.insert("crate::traits::SelfInAssocBound::get_related",
            ExpectedMethodNode {
            name: "get_related",
            visibility: VisibilityKind::Public,
            attributes: vec![],
            docstring: None,
            cfgs: vec![],
            tracking_hash_check: true,
            parameter_count: 1,
            generic_param_count: 0,
            return_type_is_some: true,
            body_is_some: false,
        }
        );
        m.insert("crate::traits::inner::InnerSecretTrait::secret_op",
            ExpectedMethodNode {
            name: "secret_op",
            visibility: VisibilityKind::Inherited,
            attributes: vec![],
            docstring: None,
            cfgs: vec![],
            tracking_hash_check: true,
            parameter_count: 1,
            generic_param_count: 0,
            return_type_is_some: false,
            body_is_some: false,
        }
        );
        m.insert("crate::traits::inner::InnerPublicTrait::public_inner_op",
            ExpectedMethodNode {
            name: "public_inner_op",
            visibility: VisibilityKind::Public,
            attributes: vec![],
            docstring: None,
            cfgs: vec![],
            tracking_hash_check: true,
            parameter_count: 1,
            generic_param_count: 0,
            return_type_is_some: false,
            body_is_some: false,
        }
        );
        m.insert("crate::traits::inner::SuperGraphNodeTrait::super_visible_op",
            ExpectedMethodNode {
            name: "super_visible_op",
            visibility: VisibilityKind::Restricted(vec!["super".to_string()]),
            attributes: vec![],
            docstring: None,
            cfgs: vec![],
            tracking_hash_check: true,
            parameter_count: 1,
            generic_param_count: 0,
            return_type_is_some: false,
            body_is_some: false,
        }
        );
        m
    };
}

#[test]
fn fixture_assoc_method_node_total_matches_graph() {
    let mut total = 0usize;
    for g in PARSED_FIXTURE_CRATE_NODES.iter() {
        let p = g.file_path.display().to_string();
        if !p.ends_with("src/impls.rs") && !p.ends_with("src/traits.rs") {
            continue;
        }
        total += g
            .graph
            .impls()
            .iter()
            .map(|i| i.methods.len())
            .sum::<usize>();
        total += g
            .graph
            .traits()
            .iter()
            .map(|t| t.methods.len())
            .sum::<usize>();
    }
    assert_eq!(
        total,
        41,
        "Update EXPECTED_METHOD_* maps and this assert when fixture_nodes method inventory changes"
    );
}

assoc_paranoid_test_fields_and_values!(
    assoc_impls_inherent_simple_struct_new,
    "crate::impls::inherent_SimpleStruct::new",
    EXPECTED_METHOD_ARGS,
    EXPECTED_METHOD_DATA,
    syn_parser::parser::nodes::MethodNode,
    syn_parser::parser::nodes::ExpectedMethodNode,
    as_method,
    LOG_TEST_ASSOC_METHOD
);

assoc_paranoid_test_fields_and_values!(
    assoc_impls_inherent_simple_struct_private_method,
    "crate::impls::inherent_SimpleStruct::private_method",
    EXPECTED_METHOD_ARGS,
    EXPECTED_METHOD_DATA,
    syn_parser::parser::nodes::MethodNode,
    syn_parser::parser::nodes::ExpectedMethodNode,
    as_method,
    LOG_TEST_ASSOC_METHOD
);

assoc_paranoid_test_fields_and_values!(
    assoc_impls_inherent_simple_struct_public_method,
    "crate::impls::inherent_SimpleStruct::public_method",
    EXPECTED_METHOD_ARGS,
    EXPECTED_METHOD_DATA,
    syn_parser::parser::nodes::MethodNode,
    syn_parser::parser::nodes::ExpectedMethodNode,
    as_method,
    LOG_TEST_ASSOC_METHOD
);

assoc_paranoid_test_fields_and_values!(
    assoc_impls_inherent_private_struct_get_secret_len,
    "crate::impls::inherent_PrivateStruct::get_secret_len",
    EXPECTED_METHOD_ARGS,
    EXPECTED_METHOD_DATA,
    syn_parser::parser::nodes::MethodNode,
    syn_parser::parser::nodes::ExpectedMethodNode,
    as_method,
    LOG_TEST_ASSOC_METHOD
);

assoc_paranoid_test_fields_and_values!(
    assoc_impls_inherent_generic_struct_get_value_ref,
    "crate::impls::inherent_GenericStruct_T::get_value_ref",
    EXPECTED_METHOD_ARGS,
    EXPECTED_METHOD_DATA,
    syn_parser::parser::nodes::MethodNode,
    syn_parser::parser::nodes::ExpectedMethodNode,
    as_method,
    LOG_TEST_ASSOC_METHOD
);

assoc_paranoid_test_fields_and_values!(
    assoc_impls_inherent_generic_struct_print_value,
    "crate::impls::inherent_GenericStruct_T_Debug::print_value",
    EXPECTED_METHOD_ARGS,
    EXPECTED_METHOD_DATA,
    syn_parser::parser::nodes::MethodNode,
    syn_parser::parser::nodes::ExpectedMethodNode,
    as_method,
    LOG_TEST_ASSOC_METHOD
);

assoc_paranoid_test_fields_and_values!(
    assoc_impls_inherent_generic_struct_str_get_str_len,
    "crate::impls::inherent_GenericStruct_str::get_str_len",
    EXPECTED_METHOD_ARGS,
    EXPECTED_METHOD_DATA,
    syn_parser::parser::nodes::MethodNode,
    syn_parser::parser::nodes::ExpectedMethodNode,
    as_method,
    LOG_TEST_ASSOC_METHOD
);

assoc_paranoid_test_fields_and_values!(
    assoc_impls_trait_impl_simple_trait_simple_struct_trait_method,
    "crate::impls::impl_SimpleTrait_for_SimpleStruct::trait_method",
    EXPECTED_METHOD_ARGS,
    EXPECTED_METHOD_DATA,
    syn_parser::parser::nodes::MethodNode,
    syn_parser::parser::nodes::ExpectedMethodNode,
    as_method,
    LOG_TEST_ASSOC_METHOD
);

assoc_paranoid_test_fields_and_values!(
    assoc_impls_trait_impl_private_trait_simple_struct,
    "crate::impls::impl_PrivateTrait_for_SimpleStruct::private_trait_method",
    EXPECTED_METHOD_ARGS,
    EXPECTED_METHOD_DATA,
    syn_parser::parser::nodes::MethodNode,
    syn_parser::parser::nodes::ExpectedMethodNode,
    as_method,
    LOG_TEST_ASSOC_METHOD
);

assoc_paranoid_test_fields_and_values!(
    assoc_impls_trait_impl_simple_trait_generic_struct,
    "crate::impls::impl_SimpleTrait_for_GenericStruct::trait_method",
    EXPECTED_METHOD_ARGS,
    EXPECTED_METHOD_DATA,
    syn_parser::parser::nodes::MethodNode,
    syn_parser::parser::nodes::ExpectedMethodNode,
    as_method,
    LOG_TEST_ASSOC_METHOD
);

assoc_paranoid_test_fields_and_values!(
    assoc_impls_trait_impl_generic_trait_generic_struct,
    "crate::impls::impl_GenericTrait_for_GenericStruct::generic_trait_method",
    EXPECTED_METHOD_ARGS,
    EXPECTED_METHOD_DATA,
    syn_parser::parser::nodes::MethodNode,
    syn_parser::parser::nodes::ExpectedMethodNode,
    as_method,
    LOG_TEST_ASSOC_METHOD
);

assoc_paranoid_test_fields_and_values!(
    assoc_impls_trait_impl_simple_trait_i32,
    "crate::impls::impl_SimpleTrait_for_i32::trait_method",
    EXPECTED_METHOD_ARGS,
    EXPECTED_METHOD_DATA,
    syn_parser::parser::nodes::MethodNode,
    syn_parser::parser::nodes::ExpectedMethodNode,
    as_method,
    LOG_TEST_ASSOC_METHOD
);

assoc_paranoid_test_fields_and_values!(
    assoc_impls_trait_impl_assoc_trait_simple_struct,
    "crate::impls::impl_AssocTrait_for_SimpleStruct::create_output",
    EXPECTED_METHOD_ARGS,
    EXPECTED_METHOD_DATA,
    syn_parser::parser::nodes::MethodNode,
    syn_parser::parser::nodes::ExpectedMethodNode,
    as_method,
    LOG_TEST_ASSOC_METHOD
);

assoc_paranoid_test_fields_and_values!(
    assoc_impls_trait_impl_simple_trait_ref_simple_struct,
    "crate::impls::impl_SimpleTrait_for_ref_SimpleStruct::trait_method",
    EXPECTED_METHOD_ARGS,
    EXPECTED_METHOD_DATA,
    syn_parser::parser::nodes::MethodNode,
    syn_parser::parser::nodes::ExpectedMethodNode,
    as_method,
    LOG_TEST_ASSOC_METHOD
);

assoc_paranoid_test_fields_and_values!(
    assoc_impls_inner_inherent_method_in_module,
    "crate::impls::inner::inherent_SimpleStruct::method_in_module",
    EXPECTED_METHOD_ARGS,
    EXPECTED_METHOD_DATA,
    syn_parser::parser::nodes::MethodNode,
    syn_parser::parser::nodes::ExpectedMethodNode,
    as_method,
    LOG_TEST_ASSOC_METHOD
);

assoc_paranoid_test_fields_and_values!(
    assoc_impls_inner_trait_impl_simple_trait_inner_struct,
    "crate::impls::inner::impl_SimpleTrait_for_InnerStruct::trait_method",
    EXPECTED_METHOD_ARGS,
    EXPECTED_METHOD_DATA,
    syn_parser::parser::nodes::MethodNode,
    syn_parser::parser::nodes::ExpectedMethodNode,
    as_method,
    LOG_TEST_ASSOC_METHOD
);

assoc_paranoid_test_fields_and_values!(
    assoc_impls_trait_def_simple_trait_trait_method,
    "crate::impls::trait_SimpleTrait::trait_method",
    EXPECTED_METHOD_ARGS,
    EXPECTED_METHOD_DATA,
    syn_parser::parser::nodes::MethodNode,
    syn_parser::parser::nodes::ExpectedMethodNode,
    as_method,
    LOG_TEST_ASSOC_METHOD
);

assoc_paranoid_test_fields_and_values!(
    assoc_impls_trait_def_private_trait,
    "crate::impls::trait_PrivateTrait::private_trait_method",
    EXPECTED_METHOD_ARGS,
    EXPECTED_METHOD_DATA,
    syn_parser::parser::nodes::MethodNode,
    syn_parser::parser::nodes::ExpectedMethodNode,
    as_method,
    LOG_TEST_ASSOC_METHOD
);

assoc_paranoid_test_fields_and_values!(
    assoc_impls_trait_def_generic_trait,
    "crate::impls::trait_GenericTrait::generic_trait_method",
    EXPECTED_METHOD_ARGS,
    EXPECTED_METHOD_DATA,
    syn_parser::parser::nodes::MethodNode,
    syn_parser::parser::nodes::ExpectedMethodNode,
    as_method,
    LOG_TEST_ASSOC_METHOD
);

assoc_paranoid_test_fields_and_values!(
    assoc_impls_trait_def_assoc_trait,
    "crate::impls::trait_AssocTrait::create_output",
    EXPECTED_METHOD_ARGS,
    EXPECTED_METHOD_DATA,
    syn_parser::parser::nodes::MethodNode,
    syn_parser::parser::nodes::ExpectedMethodNode,
    as_method,
    LOG_TEST_ASSOC_METHOD
);

assoc_paranoid_test_fields_and_values!(
    assoc_traits_root_simple_trait_required_method,
    "crate::traits::SimpleTrait::required_method",
    EXPECTED_METHOD_ARGS,
    EXPECTED_METHOD_DATA,
    syn_parser::parser::nodes::MethodNode,
    syn_parser::parser::nodes::ExpectedMethodNode,
    as_method,
    LOG_TEST_ASSOC_METHOD
);

assoc_paranoid_test_fields_and_values!(
    assoc_traits_root_internal_trait_default_method,
    "crate::traits::InternalTrait::default_method",
    EXPECTED_METHOD_ARGS,
    EXPECTED_METHOD_DATA,
    syn_parser::parser::nodes::MethodNode,
    syn_parser::parser::nodes::ExpectedMethodNode,
    as_method,
    LOG_TEST_ASSOC_METHOD
);

assoc_paranoid_test_fields_and_values!(
    assoc_traits_root_crate_trait,
    "crate::traits::CrateTrait::crate_method",
    EXPECTED_METHOD_ARGS,
    EXPECTED_METHOD_DATA,
    syn_parser::parser::nodes::MethodNode,
    syn_parser::parser::nodes::ExpectedMethodNode,
    as_method,
    LOG_TEST_ASSOC_METHOD
);

assoc_paranoid_test_fields_and_values!(
    assoc_traits_root_documented_trait,
    "crate::traits::DocumentedTrait::documented_method",
    EXPECTED_METHOD_ARGS,
    EXPECTED_METHOD_DATA,
    syn_parser::parser::nodes::MethodNode,
    syn_parser::parser::nodes::ExpectedMethodNode,
    as_method,
    LOG_TEST_ASSOC_METHOD
);

assoc_paranoid_test_fields_and_values!(
    assoc_traits_root_generic_trait_process,
    "crate::traits::GenericTrait::process",
    EXPECTED_METHOD_ARGS,
    EXPECTED_METHOD_DATA,
    syn_parser::parser::nodes::MethodNode,
    syn_parser::parser::nodes::ExpectedMethodNode,
    as_method,
    LOG_TEST_ASSOC_METHOD
);

assoc_paranoid_test_fields_and_values!(
    assoc_traits_root_lifetime_trait,
    "crate::traits::LifetimeTrait::get_ref",
    EXPECTED_METHOD_ARGS,
    EXPECTED_METHOD_DATA,
    syn_parser::parser::nodes::MethodNode,
    syn_parser::parser::nodes::ExpectedMethodNode,
    as_method,
    LOG_TEST_ASSOC_METHOD
);

assoc_paranoid_test_fields_and_values!(
    assoc_traits_root_complex_generic_trait,
    "crate::traits::ComplexGenericTrait::complex_process",
    EXPECTED_METHOD_ARGS,
    EXPECTED_METHOD_DATA,
    syn_parser::parser::nodes::MethodNode,
    syn_parser::parser::nodes::ExpectedMethodNode,
    as_method,
    LOG_TEST_ASSOC_METHOD
);

assoc_paranoid_test_fields_and_values!(
    assoc_traits_root_assoc_type_trait,
    "crate::traits::AssocTypeTrait::generate",
    EXPECTED_METHOD_ARGS,
    EXPECTED_METHOD_DATA,
    syn_parser::parser::nodes::MethodNode,
    syn_parser::parser::nodes::ExpectedMethodNode,
    as_method,
    LOG_TEST_ASSOC_METHOD
);

assoc_paranoid_test_fields_and_values!(
    assoc_traits_root_assoc_type_with_bounds,
    "crate::traits::AssocTypeWithBounds::generate_bounded",
    EXPECTED_METHOD_ARGS,
    EXPECTED_METHOD_DATA,
    syn_parser::parser::nodes::MethodNode,
    syn_parser::parser::nodes::ExpectedMethodNode,
    as_method,
    LOG_TEST_ASSOC_METHOD
);

assoc_paranoid_test_fields_and_values!(
    assoc_traits_root_assoc_const_trait,
    "crate::traits::AssocConstTrait::get_id",
    EXPECTED_METHOD_ARGS,
    EXPECTED_METHOD_DATA,
    syn_parser::parser::nodes::MethodNode,
    syn_parser::parser::nodes::ExpectedMethodNode,
    as_method,
    LOG_TEST_ASSOC_METHOD
);

assoc_paranoid_test_fields_and_values!(
    assoc_traits_root_super_trait,
    "crate::traits::SuperTrait::super_method",
    EXPECTED_METHOD_ARGS,
    EXPECTED_METHOD_DATA,
    syn_parser::parser::nodes::MethodNode,
    syn_parser::parser::nodes::ExpectedMethodNode,
    as_method,
    LOG_TEST_ASSOC_METHOD
);

assoc_paranoid_test_fields_and_values!(
    assoc_traits_root_multi_super_trait,
    "crate::traits::MultiSuperTrait::multi_super_method",
    EXPECTED_METHOD_ARGS,
    EXPECTED_METHOD_DATA,
    syn_parser::parser::nodes::MethodNode,
    syn_parser::parser::nodes::ExpectedMethodNode,
    as_method,
    LOG_TEST_ASSOC_METHOD
);

assoc_paranoid_test_fields_and_values!(
    assoc_traits_root_generic_super_trait,
    "crate::traits::GenericSuperTrait::generic_super_method",
    EXPECTED_METHOD_ARGS,
    EXPECTED_METHOD_DATA,
    syn_parser::parser::nodes::MethodNode,
    syn_parser::parser::nodes::ExpectedMethodNode,
    as_method,
    LOG_TEST_ASSOC_METHOD
);

assoc_paranoid_test_fields_and_values!(
    assoc_traits_root_attributed_trait,
    "crate::traits::AttributedTrait::calculate",
    EXPECTED_METHOD_ARGS,
    EXPECTED_METHOD_DATA,
    syn_parser::parser::nodes::MethodNode,
    syn_parser::parser::nodes::ExpectedMethodNode,
    as_method,
    LOG_TEST_ASSOC_METHOD
);

assoc_paranoid_test_fields_and_values!(
    assoc_traits_root_unsafe_trait,
    "crate::traits::UnsafeTrait::unsafe_method",
    EXPECTED_METHOD_ARGS,
    EXPECTED_METHOD_DATA,
    syn_parser::parser::nodes::MethodNode,
    syn_parser::parser::nodes::ExpectedMethodNode,
    as_method,
    LOG_TEST_ASSOC_METHOD
);

assoc_paranoid_test_fields_and_values!(
    assoc_traits_root_self_usage_returns_self,
    "crate::traits::SelfUsageTrait::returns_self",
    EXPECTED_METHOD_ARGS,
    EXPECTED_METHOD_DATA,
    syn_parser::parser::nodes::MethodNode,
    syn_parser::parser::nodes::ExpectedMethodNode,
    as_method,
    LOG_TEST_ASSOC_METHOD
);

assoc_paranoid_test_fields_and_values!(
    assoc_traits_root_self_usage_takes_self,
    "crate::traits::SelfUsageTrait::takes_self",
    EXPECTED_METHOD_ARGS,
    EXPECTED_METHOD_DATA,
    syn_parser::parser::nodes::MethodNode,
    syn_parser::parser::nodes::ExpectedMethodNode,
    as_method,
    LOG_TEST_ASSOC_METHOD
);

assoc_paranoid_test_fields_and_values!(
    assoc_traits_root_self_in_assoc_bound,
    "crate::traits::SelfInAssocBound::get_related",
    EXPECTED_METHOD_ARGS,
    EXPECTED_METHOD_DATA,
    syn_parser::parser::nodes::MethodNode,
    syn_parser::parser::nodes::ExpectedMethodNode,
    as_method,
    LOG_TEST_ASSOC_METHOD
);

assoc_paranoid_test_fields_and_values!(
    assoc_traits_inner_secret,
    "crate::traits::inner::InnerSecretTrait::secret_op",
    EXPECTED_METHOD_ARGS,
    EXPECTED_METHOD_DATA,
    syn_parser::parser::nodes::MethodNode,
    syn_parser::parser::nodes::ExpectedMethodNode,
    as_method,
    LOG_TEST_ASSOC_METHOD
);

assoc_paranoid_test_fields_and_values!(
    assoc_traits_inner_public,
    "crate::traits::inner::InnerPublicTrait::public_inner_op",
    EXPECTED_METHOD_ARGS,
    EXPECTED_METHOD_DATA,
    syn_parser::parser::nodes::MethodNode,
    syn_parser::parser::nodes::ExpectedMethodNode,
    as_method,
    LOG_TEST_ASSOC_METHOD
);

assoc_paranoid_test_fields_and_values!(
    assoc_traits_inner_super_graph,
    "crate::traits::inner::SuperGraphNodeTrait::super_visible_op",
    EXPECTED_METHOD_ARGS,
    EXPECTED_METHOD_DATA,
    syn_parser::parser::nodes::MethodNode,
    syn_parser::parser::nodes::ExpectedMethodNode,
    as_method,
    LOG_TEST_ASSOC_METHOD
);
