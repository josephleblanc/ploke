//! Procedural derive macro (`ExpectedData`) for generating test helper structs
//! (`Expected*Data`) and associated checking/finding methods used in
//! syn_parser tests.

extern crate proc_macro;

use proc_macro::TokenStream;
use proc_macro_error::{abort, proc_macro_error};
use quote::{format_ident, quote};
use syn::{parse_macro_input, spanned::Spanned, Data, DeriveInput, Fields, FieldsNamed, Type};

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
    let mut expected_fields = Vec::new();
    // --- Generate Implementations for Check Methods ---
    let mut check_methods = Vec::new();
    // --- Generate Filter Steps for find_node_by_values ---
    let mut find_filters = Vec::new();

    // Define the log target (can be customized later)
    let log_target = quote! { "log_test_node" }; // Use a generic target

    for field in fields {
        let field_name = field.ident.as_ref().unwrap(); // We know fields are named
        let field_name_str = field_name.to_string();
        let field_type = &field.ty;

        // Generate check method name (e.g., name -> is_name_match_debug)
        let check_method_name = format_ident!("is_{}_match_debug", field_name);

        // Map original field to expected field and generate checks/filters
        match field_name_str.as_str() {
            "id" | "span" | "fields" | "variants" | "methods" | "imports" | "exports"
            | "generic_params" | "module_def" | "parameters" | "return_type" | "super_traits"
            | "kind" => {
                // Ignore these fields for the Expected*Data struct and checks
            }
            "name" => {
                expected_fields.push(quote! { pub name: &'static str });
                check_methods.push(quote! {
                    fn #check_method_name(&self, node: &N) -> bool {
                        let check = self.name == node.name(); // Assuming N impl GraphNode
                        log::debug!(target: #log_target,
                            "   {} {} | Expected '{}' == Actual '{}'",
                            "Name Match?".to_string().log_step(), check.log_bool(),
                            self.name.log_name(), node.name().log_name()
                        );
                        check
                    }
                });
                find_filters.push(quote! { .filter(|n| n.name() == self.name) });
            }
            "visibility" => {
                expected_fields
                    .push(quote! { pub visibility: syn_parser::parser::types::VisibilityKind }); // Use qualified path
                check_methods.push(quote! {
                    fn #check_method_name(&self, node: &N) -> bool {
                        let check = self.visibility == *node.visibility(); // Assuming N impl GraphNode
                        log::debug!(target: #log_target,
                            "   {} {} | Expected '{}' == Actual '{}'",
                            "Visibility Match?".to_string().log_step(), check.log_bool(),
                            self.visibility.log_vis_debug(), node.visibility().log_vis_debug()
                        );
                        check
                    }
                });
                find_filters.push(quote! { .filter(|n| *n.visibility() == self.visibility) });
            }
            "attributes" => {
                expected_fields
                    .push(quote! { pub attributes: Vec<syn_parser::parser::nodes::Attribute> }); // Use qualified path
                check_methods.push(quote! {
                    fn #check_method_name(&self, node: &N) -> bool {
                        let check = self.attributes == node.attributes(); // Assuming N impl HasAttributes
                        log::debug!(target: #log_target,
                            "   {} {} | Expected '{:?}' == Actual '{:?}'",
                            "Attributes Match?".to_string().log_step(), check.log_bool(),
                            self.attributes, node.attributes()
                        );
                        check
                    }
                });
                find_filters.push(quote! { .filter(|n| n.attributes() == self.attributes) });
            }
            "docstring" => {
                expected_fields.push(quote! { pub docstring_contains: Option<&'static str> });
                check_methods.push(quote! {
                    fn #check_method_name(&self, node: &N) -> bool {
                        let actual_docstring = node.docstring(); // Assuming N impl GraphNode provides docstring() -> Option<&str>
                        let check_passes = match self.docstring_contains {
                            Some(expected_substr) => actual_docstring.map_or(false, |s| s.contains(expected_substr)),
                            None => actual_docstring.is_none(),
                        };
                        log::debug!(target: #log_target,
                            "   {} {} | Expected contains '{:?}' in Actual '{:?}'",
                            "Docstring Contains Match?".to_string().log_step(), check_passes.log_bool(),
                            self.docstring_contains, actual_docstring
                        );
                        check_passes
                    }
                });
                find_filters.push(quote! {
                     .filter(|n| {
                         let actual_docstring = n.docstring();
                         match self.docstring_contains {
                            Some(expected_substr) => actual_docstring.map_or(false, |s| s.contains(expected_substr)),
                            None => actual_docstring.is_none(),
                         }
                     })
                 });
            }
            "cfgs" => {
                expected_fields.push(quote! { pub cfgs: Vec<String> });
                check_methods.push(quote! {
                    fn #check_method_name(&self, node: &N) -> bool {
                        let mut actual_cfgs = node.cfgs().to_vec(); // Assuming N impl GraphNode
                        actual_cfgs.sort_unstable();
                        let mut expected_cfgs_sorted = self.cfgs.clone();
                        expected_cfgs_sorted.sort_unstable();
                        let check = expected_cfgs_sorted == actual_cfgs;
                        log::debug!(target: #log_target,
                            "   {} {} | Expected (sorted) '{:?}' == Actual (sorted) '{:?}'",
                            "CFGs Match?".to_string().log_step(), check.log_bool(),
                            expected_cfgs_sorted, actual_cfgs
                        );
                        check
                    }
                });
                find_filters.push(quote! {
                    .filter(|n| {
                        let mut actual_cfgs = n.cfgs().to_vec();
                        actual_cfgs.sort_unstable();
                        let mut expected_cfgs_sorted = self.cfgs.clone();
                        expected_cfgs_sorted.sort_unstable();
                        expected_cfgs_sorted == actual_cfgs
                    })
                });
            }
            "tracking_hash" => {
                expected_fields.push(quote! { pub tracking_hash_check: bool });
                check_methods.push(quote! {
                    fn #check_method_name(&self, node: &N) -> bool {
                        let actual_check_passes = node.tracking_hash().is_some(); // Assuming N impl GraphNode provides tracking_hash() -> Option<TrackingHash>
                         let check = self.tracking_hash_check == actual_check_passes;
                         log::debug!(target: #log_target,
                            "   {} {} | Expected check pass '{}' == Actual check pass '{}'",
                            "TrackingHash Check Match?".to_string().log_step(), check.log_bool(),
                            self.tracking_hash_check, actual_check_passes
                        );
                        check
                    }
                });
                find_filters.push(
                    quote! { .filter(|n| n.tracking_hash().is_some() == self.tracking_hash_check) },
                );
            }
            "type_id" => {
                expected_fields.push(quote! { pub type_id_check: bool });
                check_methods.push(quote! {
                    fn #check_method_name(&self, node: &N) -> bool {
                        // Need a way to get type_id from N. Add a trait?
                        // Assuming a method `get_type_id()` exists for relevant nodes.
                        let actual_check_passes = node.get_type_id().is_synthetic(); // Assuming N has get_type_id() -> TypeId
                        let check = self.type_id_check == actual_check_passes;
                        log::debug!(target: #log_target,
                            "   {} {} | Expected check pass '{}' == Actual check pass '{}'",
                            "TypeId Check Match?".to_string().log_step(), check.log_bool(),
                            self.type_id_check, actual_check_passes
                        );
                        check
                    }
                });
                find_filters.push(
                    quote! { .filter(|n| n.get_type_id().is_synthetic() == self.type_id_check) },
                );
            }
            "value" if matches!(field_type, Type::Path(p) if p.path.segments.last().is_some_and(|seg| seg.ident == "Option")) =>
            {
                // Handle Option<String> fields like 'value'
                expected_fields.push(quote! { pub value: Option<&'static str> });
                check_methods.push(quote! {
                    fn #check_method_name(&self, node: &N) -> bool {
                        let actual_value = node.get_value(); // Assuming N has get_value() -> Option<&str>
                        let check = self.value == actual_value;
                        log::debug!(target: #log_target,
                            "   {} {} | Expected '{:?}' == Actual '{:?}'",
                            "Value Match?".to_string().log_step(), check.log_bool(),
                            self.value, actual_value
                        );
                        check
                    }
                });
                find_filters.push(quote! { .filter(|n| n.get_value() == self.value) });
            }
            "is_mutable" if matches!(field_type, Type::Path(p) if p.path.segments.last().is_some_and(|seg| seg.ident == "bool")) =>
            {
                // Handle simple bool fields like 'is_mutable'
                expected_fields.push(quote! { pub is_mutable: bool });
                check_methods.push(quote! {
                    fn #check_method_name(&self, node: &N) -> bool {
                        let actual_value = node.is_mutable(); // Assuming N has is_mutable() -> bool
                        let check = self.is_mutable == actual_value;
                        log::debug!(target: #log_target,
                            "   {} {} | Expected '{}' == Actual '{}'",
                            "Is Mutable Match?".to_string().log_step(), check.log_bool(),
                            self.is_mutable, actual_value
                        );
                        check
                    }
                });
                find_filters.push(quote! { .filter(|n| n.is_mutable() == self.is_mutable) });
            }
            // Add more field handlers here...
            _ => {
                // Optionally warn or ignore unknown fields
                // log::warn!("Ignoring field {} in ExpectedData derive", field_name_str);
            }
        }
    }

    // --- Generate the Expected*Data Struct Definition ---
    let expected_struct_def = quote! {
        #[derive(Debug, Clone, PartialEq)] // Add derives as needed
        pub struct #expected_data_struct_name {
            #(#expected_fields),* // Expand generated fields
        }
    };

    // --- Generate the `impl ExpectedNodeData` block ---
    // Determine the node collection field name (e.g., `graph.consts`, `graph.statics`)
    // This requires mapping the node struct name to the field name in CodeGraph.
    let graph_collection_field = match node_struct_name.to_string().as_str() {
        "ConstNode" => quote! { consts },
        "StaticNode" => quote! { statics },
        "FunctionNode" => quote! { functions },
        "StructNode" | "EnumNode" | "UnionNode" | "TypeAliasNode" => quote! { defined_types }, // Need further filtering for these
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

    // Special handling for TypeDefNode variants
    let type_def_filter = match node_struct_name.to_string().as_str() {
        "StructNode" => {
            quote! { .filter_map(|n| if let syn_parser::parser::nodes::TypeDefNode::Struct(s) = n { Some(s) } else { None }) }
        }
        "EnumNode" => {
            quote! { .filter_map(|n| if let syn_parser::parser::nodes::TypeDefNode::Enum(e) = n { Some(e) } else { None }) }
        }
        "UnionNode" => {
            quote! { .filter_map(|n| if let syn_parser::parser::nodes::TypeDefNode::Union(u) = n { Some(u) } else { None }) }
        }
        "TypeAliasNode" => {
            quote! { .filter_map(|n| if let syn_parser::parser::nodes::TypeDefNode::TypeAlias(t) = n { Some(t) } else { None }) }
        }
        _ => quote! {}, // No extra filtering needed for other node types
    };

    let expected_node_data_impl = quote! {
        // Use fully qualified paths for traits/structs from syn_parser
        impl crate::ExpectedNodeData<syn_parser::parser::nodes::#node_struct_name> for #expected_data_struct_name {
            fn find_node_by_values<'a>(
                &'a self,
                parsed: &'a syn_parser::parser::ParsedCodeGraph,
            ) -> Box<dyn Iterator<Item = &'a syn_parser::parser::nodes::#node_struct_name> + 'a> {
                 // Import necessary traits for logging/comparison if not already in scope
                 use syn_parser::utils::{LogStyle, LogStyleBool, LogStyleDebug};
                 use syn_parser::parser::nodes::{GraphNode, HasAttributes}; // Assuming these provide necessary methods

                 Box::new(parsed.graph.#graph_collection_field.iter()
                    #type_def_filter // Apply TypeDefNode filtering if necessary
                    // Apply generated filters
                    #(#find_filters)*
                 )
            }

            fn check_all_fields(&self, node: &syn_parser::parser::nodes::#node_struct_name) -> bool {
                 // Import necessary traits for logging/comparison if not already in scope
                 use syn_parser::utils::{LogStyle, LogStyleBool, LogStyleDebug};
                 use syn_parser::parser::nodes::{GraphNode, HasAttributes}; // Assuming these provide necessary methods
                 use ploke_core::IdTrait; // For TypeId::is_synthetic

                // Call all generated check methods defined in the separate impl block
                let mut all_passed = true;
                #( // Iterate through the check_method_name idents generated earlier
                    if !self.#check_methods(node) { all_passed = false; }
                )*
                all_passed
           }
       }
    };

    // --- Generate the `impl Expected*Data` block with check methods ---
    let expected_data_impl = quote! {
         // Use fully qualified paths for traits/structs from syn_parser
         impl #expected_data_struct_name {
             // Define helper methods needed by check methods (e.g., log_step)
             // Or assume they are brought into scope via use statements where the derive is applied.
             // For simplicity, let's assume LogStyle utils are imported.

             // Define the check methods
             #( // Repeat for each generated check method
                 // Add necessary trait bounds to N if methods assume them (e.g., GraphNode, HasAttributes)
                 fn #check_methods<N: syn_parser::parser::nodes::GraphNode + syn_parser::parser::nodes::HasAttributes + std::fmt::Debug>(&self, node: &N) -> bool {
                     // Implementation generated above
                 }
             )*
         }
    };

    // --- Combine Generated Code ---
    let output = quote! {
        #expected_struct_def
        #expected_node_data_impl
        // #expected_data_impl // Impl block with check methods - currently defined within ExpectedNodeData impl
    };

    output.into()
}
