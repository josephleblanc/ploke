//! Procedural derive macro (`ExpectedData`) for generating test helper structs
//! (`Expected*Data`) and associated checking/finding methods used in
//! syn_parser tests.

extern crate proc_macro;

use proc_macro::TokenStream;
use proc_macro_error::{abort, proc_macro_error};
use quote::{format_ident, quote};
use syn::{parse_macro_input, spanned::Spanned, Data, DeriveInput, Fields, Type}; // Removed FieldsNamed

// Placeholder for the base trait that generated structs will implement.
// The actual trait definition might live here or be defined in syn_parser::tests::common
// and referenced via a known path in the generated code. For now, just a comment.
//
// trait ExpectedNodeData<N> {
//     fn find_node_by_values<'a>(...) -> ...;
//     fn check_all_fields(...) -> bool;
// }

// --- Derive Macro Implementation ---

#[proc_macro_error]
#[proc_macro_derive(ExpectedData)]
pub fn derive_expected_data(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let node_struct_name = &input.ident;

    // Generate the corresponding Expected*Data struct name (e.g., ConstNode -> ExpectedConstData)
    let expected_data_struct_name = format_ident!("Expected{}", node_struct_name);

    // Ensure the input is a struct with named fields
    let fields = match &input.data {
        Data::Struct(s) => match &s.fields {
            Fields::Named(f) => &f.named,
            _ => abort!(
                s.fields.span(),
                "ExpectedData derive only supports structs with named fields"
            ),
        },
        _ => abort!(input.span(), "ExpectedData derive only supports structs"),
    };

    // --- Generate Fields for Expected*Data Struct ---
    let mut expected_fields_defs = Vec::new();
    // --- Generate Implementations for Inherent Check Methods ---
    let mut inherent_check_method_impls = Vec::new();
    // --- Generate Logic for check_all_fields ---
    let mut check_all_fields_logics = Vec::new();
    // --- Generate Filter Steps for find_node_by_values (selective) ---
    let mut find_node_by_values_filters = Vec::new();

    // Define the log target (can be customized later)
    let log_target = quote! { "log_test_node" }; // Use a generic target, e.g., from const_static.rs LOG_TEST_CONST

    for field in fields {
        let field_ident = field.ident.as_ref().unwrap(); // We know fields are named
        let field_name_str = field_ident.to_string();
        let field_type = &field.ty;

        // Generate check method name (e.g., name -> is_name_match_debug)
        let check_method_name_ident = format_ident!("is_{}_match_debug", field_ident);

        // Map original field to expected field and generate checks/filters
        match field_name_str.as_str() {
            "id" | "span" | "fields" | "variants" | "methods" | "imports" | "exports"
            | "generic_params" | "module_def" | "parameters" | "return_type" | "super_traits"
            | "kind" => {
                // These fields are typically not directly compared by value in Expected*Data,
                // or are handled by ID regeneration or specific relation checks.
                // We will not generate `is_*_match_debug` for them by default.
            }
            "name" => {
                expected_fields_defs.push(quote! { pub name: &'static str });
                inherent_check_method_impls.push(quote! {
                    pub fn #check_method_name_ident(&self, node: &crate::parser::nodes::#node_struct_name) -> bool {
                        // Assuming node implements GraphNode trait which has name()
                        let check = self.name == node.name();
                        log::debug!(target: #log_target, // Use the specific log target
                            "   {} {} | Expected '{}' == Actual '{}'",
                            "Name Match?".to_string().log_step(), check.log_bool(),
                            self.name.log_name(), node.name().log_name()
                        );
                        check
                    }
                });
                check_all_fields_logics.push(quote! {
                    if !self.#check_method_name_ident(node) { all_passed = false; }
                });
                find_node_by_values_filters
                    .push(quote! { .filter(|n| self.#check_method_name_ident(n)) });
            }
            "visibility" => {
                expected_fields_defs
                    .push(quote! { pub visibility: crate::parser::types::VisibilityKind });
                inherent_check_method_impls.push(quote! {
                    pub fn #check_method_name_ident(&self, node: &crate::parser::nodes::#node_struct_name) -> bool {
                        // Assuming node implements GraphNode trait which has visibility()
                        let check = self.visibility == *node.visibility();
                        log::debug!(target: #log_target,
                            "   {} {} | Expected '{}' == Actual '{}'",
                            "Visibility Match?".to_string().log_step(), check.log_bool(),
                            self.visibility.log_vis_debug(), node.visibility().log_vis_debug()
                        );
                        check
                    }
                });
                check_all_fields_logics.push(quote! {
                    if !self.#check_method_name_ident(node) { all_passed = false; }
                });
                find_node_by_values_filters
                    .push(quote! { .filter(|n| self.#check_method_name_ident(n)) });
            }
            "attributes" => {
                expected_fields_defs
                    .push(quote! { pub attributes: Vec<crate::parser::nodes::Attribute> });
                inherent_check_method_impls.push(quote! {
                    pub fn #check_method_name_ident(&self, node: &crate::parser::nodes::#node_struct_name) -> bool {
                        // Assuming node implements HasAttributes trait
                        let check = self.attributes == node.attributes();
                        log::debug!(target: #log_target,
                            "   {} {} | Expected '{:?}' == Actual '{:?}'",
                            "Attributes Match?".to_string().log_step(), check.log_bool(),
                            self.attributes.log_green_debug(), node.attributes().log_green_debug() // Using log_green_debug like manual
                        );
                        check
                    }
                });
                check_all_fields_logics.push(quote! {
                    if !self.#check_method_name_ident(node) { all_passed = false; }
                });
                find_node_by_values_filters
                    .push(quote! { .filter(|n| self.#check_method_name_ident(n)) });
            }
            "docstring" => {
                // The field in Expected*Data is docstring_contains: Option<&'static str>
                expected_fields_defs.push(quote! { pub docstring_contains: Option<&'static str> });
                inherent_check_method_impls.push(quote! {
                    pub fn #check_method_name_ident(&self, node: &crate::parser::nodes::#node_struct_name) -> bool {
                        // Assuming node implements GraphNode trait which has docstring() -> Option<String> or Option<&str>
                        // The manual impl uses node.docstring.as_deref() which implies node.docstring is Option<String>
                        let actual_docstring = node.docstring.as_deref(); // Access field directly
                        let check_passes = match self.docstring_contains {
                            Some(expected_substr) => actual_docstring.map_or(false, |s| s.contains(expected_substr)),
                            None => actual_docstring.is_none(),
                        };
                        log::debug!(target: #log_target,
                            "   {} {} | Expected contains '{}' in Actual '{}'",
                            "Docstring Contains Match?".to_string().log_step(), check_passes.log_bool(),
                            self.docstring_contains.unwrap_or("None").log_foreground_primary(), // Manual style
                            actual_docstring.unwrap_or("None").log_foreground_secondary() // Manual style
                        );
                        check_passes
                    }
                });
                check_all_fields_logics.push(quote! {
                    if !self.#check_method_name_ident(node) { all_passed = false; }
                });
                find_node_by_values_filters
                    .push(quote! { .filter(|n| self.#check_method_name_ident(n)) });
            }
            "cfgs" => {
                expected_fields_defs.push(quote! { pub cfgs: Vec<String> });
                inherent_check_method_impls.push(quote! {
                    pub fn #check_method_name_ident(&self, node: &crate::parser::nodes::#node_struct_name) -> bool {
                        // Assuming node implements GraphNode trait which has cfgs()
                        let mut actual_cfgs = node.cfgs().to_vec();
                        actual_cfgs.sort_unstable();
                        let mut expected_cfgs_sorted = self.cfgs.clone();
                        expected_cfgs_sorted.sort_unstable();
                        let check = expected_cfgs_sorted == actual_cfgs;
                        log::debug!(target: #log_target, // Manual impl uses LOG_TEST_CONST, here using generic log_target
                            "   {} {} | Expected (sorted) '{:?}' == Actual (sorted) '{:?}'", // Adjusted log to match manual closer
                            "CFGs Match?".to_string().log_step(), check.log_bool(), // Manual uses log_green, then log_bool
                            expected_cfgs_sorted.log_green_debug(), actual_cfgs.log_green_debug()
                        );
                        check
                    }
                });
                check_all_fields_logics.push(quote! {
                    if !self.#check_method_name_ident(node) { all_passed = false; }
                });
                find_node_by_values_filters
                    .push(quote! { .filter(|n| self.#check_method_name_ident(n)) });
            }
            "tracking_hash" => {
                // The field in Expected*Data is tracking_hash_check: bool
                expected_fields_defs.push(quote! { pub tracking_hash_check: bool });
                inherent_check_method_impls.push(quote! {
                    pub fn #check_method_name_ident(&self, node: &crate::parser::nodes::#node_struct_name) -> bool {
                        // Assuming node has a public field tracking_hash: Option<TrackingHash>
                        let actual_check_passes = node.tracking_hash.is_some(); // Access field directly
                        let check = self.tracking_hash_check == actual_check_passes;
                        log::debug!(target: #log_target,
                            "   {} {} | Expected check pass '{}' == Actual check pass '{}'",
                            "TrackingHash Check Match?".to_string().log_step(), check.log_bool(),
                            self.tracking_hash_check.to_string().log_name(),
                            actual_check_passes.to_string().log_name()
                        );
                        check
                    }
                });
                check_all_fields_logics.push(quote! {
                    if !self.#check_method_name_ident(node) { all_passed = false; }
                });
                find_node_by_values_filters
                    .push(quote! { .filter(|n| self.#check_method_name_ident(n)) });
            }
            "type_id" => {
                // The field in Expected*Data is type_id_check: bool
                expected_fields_defs.push(quote! { pub type_id_check: bool });
                inherent_check_method_impls.push(quote! {
                    pub fn #check_method_name_ident(&self, node: &crate::parser::nodes::#node_struct_name) -> bool {
                        // Assuming node has a public field type_id: TypeId
                        // And TypeId has an is_synthetic() method (from ploke_core::IdTrait)
                        let actual_check_passes = node.type_id.is_synthetic(); // Access field directly
                        let check = self.type_id_check == actual_check_passes;
                        log::debug!(target: #log_target,
                            "   {} {} | Expected check pass '{}' == Actual check pass '{}'",
                            "TypeId Check Match?".to_string().log_step(), check.log_bool(),
                            self.type_id_check.to_string().log_name(),
                            actual_check_passes.to_string().log_name()
                        );
                        check
                    }
                });
                check_all_fields_logics.push(quote! {
                    if !self.#check_method_name_ident(node) { all_passed = false; }
                });
                // Not adding to find_node_by_values_filters by default, as per instructions.
            }
            // Handle `value: Option<String>` for ConstNode
            "value"
                if (node_struct_name == "ConstNode"
                    || node_struct_name == "StaticNode"
                    || node_struct_name == "StructNode")
                    && matches!(field_type, Type::Path(p) if p.path.segments.last().is_some_and(|seg| seg.ident == "Option")) =>
            {
                expected_fields_defs.push(quote! { pub value: Option<&'static str> });
                inherent_check_method_impls.push(quote! {
                    pub fn #check_method_name_ident(&self, node: &crate::parser::nodes::#node_struct_name) -> bool {
                        // Assuming node has public field value: Option<String>
                        let actual_value = node.value.as_deref(); // Access field directly
                        let check = self.value == actual_value;
                        log::debug!(target: #log_target,
                            "   {} {} | Expected '{}' == Actual '{}'",
                            "Value Match?".to_string().log_step(), check.log_bool(),
                            self.value.unwrap_or("None").log_foreground_primary(), // Manual style
                            actual_value.unwrap_or("None").log_foreground_secondary() // Manual style
                        );
                        check
                    }
                });
                check_all_fields_logics.push(quote! {
                    if !self.#check_method_name_ident(node) { all_passed = false; }
                });
                // Not adding to find_node_by_values_filters by default.
            }
            // Handle `is_mutable: bool` for StaticNode
            "is_mutable"
                if node_struct_name == "StaticNode"
                    && matches!(field_type, Type::Path(p) if p.path.segments.last().is_some_and(|seg| seg.ident == "bool")) =>
            {
                expected_fields_defs.push(quote! { pub is_mutable: bool });
                inherent_check_method_impls.push(quote! {
                    pub fn #check_method_name_ident(&self, node: &crate::parser::nodes::#node_struct_name) -> bool {
                        // Assuming node has public field is_mutable: bool
                        let actual_value = node.is_mutable; // Access field directly
                        let check = self.is_mutable == actual_value;
                        log::debug!(target: #log_target,
                            "   {} {} | Expected '{}' == Actual '{}'",
                            "Is Mutable Match?".to_string().log_step(), check.log_bool(),
                            self.is_mutable.to_string().log_name(), // Manual style for bools might differ
                            actual_value.to_string().log_name()
                        );
                        check
                    }
                });
                check_all_fields_logics.push(quote! {
                    if !self.#check_method_name_ident(node) { all_passed = false; }
                });
                // Not adding to find_node_by_values_filters by default.
            }
            _ => {
                // Optionally warn or ignore unknown fields
            }
        }
    }

    // --- Generate the Expected*Data Struct Definition ---
    let expected_struct_def = quote! {
        #[derive(Debug, Clone, PartialEq)]
        pub struct #expected_data_struct_name {
            #(#expected_fields_defs),*
        }
    };

    // Determine the graph collection field name and any necessary TypeDefNode filtering
    let graph_collection_field = match node_struct_name.to_string().as_str() {
        "ConstNode" => quote! { consts },
        "StaticNode" => quote! { statics },
        "FunctionNode" => quote! { functions },
        "StructNode" | "EnumNode" | "UnionNode" | "TypeAliasNode" => quote! { defined_types },
        "TraitNode" => quote! { traits },
        "ImplNode" => quote! { impls },
        "ModuleNode" => quote! { modules },
        "ImportNode" => quote! { use_statements },
        "MacroNode" => quote! { macros },
        _ => abort!(
            node_struct_name.span(),
            "Cannot determine graph collection field for {}",
            node_struct_name
        ),
    };

    let type_def_filter = match node_struct_name.to_string().as_str() {
        "StructNode" => {
            quote! { .filter_map(|n| if let crate::parser::nodes::TypeDefNode::Struct(s) = n { Some(s) } else { None }) }
        }
        "EnumNode" => {
            quote! { .filter_map(|n| if let crate::parser::nodes::TypeDefNode::Enum(e) = n { Some(e) } else { None }) }
        }
        "UnionNode" => {
            quote! { .filter_map(|n| if let crate::parser::nodes::TypeDefNode::Union(u) = n { Some(u) } else { None }) }
        }
        "TypeAliasNode" => {
            quote! { .filter_map(|n| if let crate::parser::nodes::TypeDefNode::TypeAlias(t) = n { Some(t) } else { None }) }
        }
        _ => quote! {}, // No extra filtering needed for other node types
    };

    // --- Generate the inherent impl block for Expected*Data ---
    let expected_data_inherent_impl = quote! {
        use crate::utils::{LogStyle, LogStyleBool, LogStyleDebug}; // For logging styles
        use crate::parser::nodes::{GraphNode, HasAttributes}; // For accessing node fields via traits
        use ::ploke_core::IdTrait; // For TypeId::is_synthetic, etc.
        // Import PrimaryNodeIdTrait if needed for to_pid() in log_target_id
        use crate::parser::nodes::PrimaryNodeIdTrait;
         impl #expected_data_struct_name {
             // These use statements are for the *body* of ALL generated inherent methods

             // Define the inherent check methods
             #(#inherent_check_method_impls)*

             // --- Helper logging methods ---
             fn log_target_id(&self, node: &crate::parser::nodes::#node_struct_name) {
                 // Assuming node.id implements PrimaryNodeIdTrait for .to_pid()
                 log::debug!(target: #log_target,
                     "Checking {}",
                     node.id.to_pid().to_string().log_id(),
                 );
             }

             fn log_all_match(&self, node: &crate::parser::nodes::#node_struct_name) {
                 // Note: The original manual impl logged the node details here.
                 // We keep it simpler for the generated version, just confirming the pass.
                 log::debug!(target: #log_target,
                     "       {}: {}",
                     "All Filters Passed for Node".to_string().log_step(),
                     true.log_bool() // Indicate pass
                 );
             }
             // --- End Helper logging methods ---


             // Define find_node_by_values as an inherent method
             pub fn find_node_by_values<'a>(
                 &'a self,
                 parsed: &'a crate::parser::ParsedCodeGraph,
             // Change return type to use impl Trait
             ) -> impl Iterator<Item = &'a crate::parser::nodes::#node_struct_name> + 'a {
                  // Use helper methods directly in inspect calls
                  parsed.graph.#graph_collection_field.iter()
                      #type_def_filter // Apply TypeDefNode filtering if necessary
                      .inspect(move |node_candidate| self.log_target_id(node_candidate)) // Use helper method
                      #(#find_node_by_values_filters)* // Apply *selective* filters
                      .inspect(move |node_candidate| self.log_all_match(node_candidate)) // Use helper method
                  // No Box::new needed
             }

             // Define check_all_fields as an inherent method
             pub fn check_all_fields(&self, node: &crate::parser::nodes::#node_struct_name) -> bool {
                 let mut all_passed = true;
                 #(#check_all_fields_logics)*
                 all_passed
            }
         }
    };

    // --- Combine Generated Code ---
    let output = quote! {
        #expected_struct_def
        #expected_data_inherent_impl // Inherent methods for Expected*Data, including find/check
        // No trait implementation block needed anymore
    };

    output.into()
}
