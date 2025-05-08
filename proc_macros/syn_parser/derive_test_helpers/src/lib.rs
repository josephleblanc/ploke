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
            // == ModuleNode Specific Handlers (Placed BEFORE general handlers) ==
            "path" // For ModuleNode
                if node_struct_name == "ModuleNode"
                   && matches!(field_type, Type::Path(p) if p.path.segments.last().is_some_and(|seg| seg.ident == "Vec")) => // Simplified Vec check
            {
                expected_fields_defs.push(quote! { pub path: &'static [&'static str] });
                inherent_check_method_impls.push(quote! {
                    pub fn #check_method_name_ident(&self, node: &crate::parser::nodes::#node_struct_name) -> bool {
                        let expected_vec: Vec<String> = self.path.iter().map(|s| s.to_string()).collect();
                        let check = expected_vec == node.path; // Compare Vec<String> == Vec<String>
                        log::debug!(target: #log_target,
                            "   {: <23} {} | \n{: >35} {}\n{: >35} {}",
                            "Path Match?".to_string().log_step(), check.log_bool(),
                            "│ Expected: ",
                            expected_vec.log_name_debug(),
                            "│ Actual:   ",
                            node.path.log_name_debug()
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
            "imports" // For ModuleNode
                if node_struct_name == "ModuleNode"
                   && matches!(field_type, Type::Path(p) if p.path.segments.last().is_some_and(|seg| seg.ident == "Vec")) => // Simplified Vec check
            {
                expected_fields_defs.push(quote! { pub imports_count: usize });
                inherent_check_method_impls.push(quote! {
                    pub fn #check_method_name_ident(&self, node: &crate::parser::nodes::#node_struct_name) -> bool {
                        let actual_count = node.imports.len();
                        let mut check = self.imports_count == actual_count;
                        if node.is_decl() && actual_count != 0 { // ModuleKind::Declaration should have 0 imports
                            log::warn!(target: #log_target,
                                "   {: <23} {} | ModuleKind::Declaration should have 0 imports, but found {}.",
                                "Imports Count Warning".to_string().log_warning(), // This is a String, log_warning() is fine
                                actual_count.to_string().log_error(), // This is a String, log_error() is fine
                                actual_count.to_string().log_error() // Provide the argument for the third placeholder
                            );
                            check = false; // Fail the check if a declaration has imports
                        }
                        log::debug!(target: #log_target,
                            "   {: <23} {} | Expected count '{}' == Actual count '{}'",
                            "Imports Count Match?".to_string().log_step(), check.log_bool(), // String, bool
                            self.imports_count.to_string().log_name(), // String
                            actual_count.to_string().log_name() // String
                        );
                        check
                    }
                });
                check_all_fields_logics.push(quote! {
                    if !self.#check_method_name_ident(node) { all_passed = false; }
                });
            }
            "exports" // For ModuleNode
                if node_struct_name == "ModuleNode"
                    && matches!(field_type, Type::Path(p) if p.path.segments.last().is_some_and(|seg| seg.ident == "Vec")) => // Simplified Vec check
            {
                expected_fields_defs.push(quote! { pub exports_count: usize });
                inherent_check_method_impls.push(quote! {
                    pub fn #check_method_name_ident(&self, node: &crate::parser::nodes::#node_struct_name) -> bool {
                        let actual_count = node.exports.len();
                        let check = self.exports_count == actual_count;
                        log::debug!(target: #log_target,
                            "   {: <23} {} | Expected count '{}' == Actual count '{}'",
                            "Exports Count Match?".to_string().log_step(), check.log_bool(),
                            self.exports_count.to_string().log_name(),
                            actual_count.to_string().log_name()
                        );
                        check
                    }
                });
                check_all_fields_logics.push(quote! {
                    if !self.#check_method_name_ident(node) { all_passed = false; }
                });
            }
            "module_def" // For ModuleNode
                if node_struct_name == "ModuleNode"
                    && matches!(field_type, Type::Path(p) if p.path.segments.last().is_some_and(|seg| seg.ident == "ModuleKind")) =>
            {
                // Add ModDisc field to ExpectedModuleNode
                expected_fields_defs.push(quote! { pub mod_disc: crate::parser::nodes::ModDisc });
                // Add expected_file_path_suffix field (Option)
                expected_fields_defs.push(quote! { pub expected_file_path_suffix: Option<&'static str> });
                // Add items_count field
                expected_fields_defs.push(quote! { pub items_count: usize });
                // Add file_attrs_count field
                expected_fields_defs.push(quote! { pub file_attrs_count: usize });
                // Add file_docs_is_some field
                expected_fields_defs.push(quote! { pub file_docs_is_some: bool });


                inherent_check_method_impls.push(quote! {
                    pub fn #check_method_name_ident(&self, node: &crate::parser::nodes::#node_struct_name) -> bool {
                        let mut overall_check = true;
                        let actual_mod_disc = match node.module_def {
                            crate::parser::nodes::ModuleKind::FileBased { .. } => crate::parser::nodes::ModDisc::FileBased,
                            crate::parser::nodes::ModuleKind::Inline { .. } => crate::parser::nodes::ModDisc::Inline,
                            crate::parser::nodes::ModuleKind::Declaration { .. } => crate::parser::nodes::ModDisc::Declaration,
                        };

                        let disc_check = self.mod_disc == actual_mod_disc;
                        log::debug!(target: #log_target,
                            "   {: <23} {} | Expected disc '{}' == Actual disc '{}'",
                            "ModuleKind Disc Match?".to_string().log_step(), disc_check.log_bool(),
                            self.mod_disc.log_name_debug(), // Use log_*_debug
                            actual_mod_disc.log_name_debug() // Use log_*_debug
                        );
                        if !disc_check { overall_check = false; }

                        // Check items_count
                        let actual_items_count = node.items().map_or(0, |items_slice| items_slice.len());
                        let items_count_check = self.items_count == actual_items_count;
                        log::debug!(target: #log_target,
                            "   {: <23} {} | Expected items count '{}' == Actual items count '{}'",
                            "Items Count Match?".to_string().log_step(), items_count_check.log_bool(),
                            self.items_count.log_name_debug(), // Use log_*_debug
                            actual_items_count.log_name_debug() // Use log_*_debug
                        );
                        if !items_count_check { overall_check = false; }

                        if actual_mod_disc == crate::parser::nodes::ModDisc::FileBased {
                            // Check expected_file_path_suffix
                            if let Some(expected_suffix) = self.expected_file_path_suffix {
                                let path_check = node.file_path().map_or(false, |fp| fp.ends_with(expected_suffix));
                                log::debug!(target: #log_target,
                                    "   {: <23} {} | Expected path suffix '{}' in Actual path '{}'",
                                    "File Path Suffix Match?".to_string().log_step(), path_check.log_bool(),
                                    expected_suffix.log_path(), // Use log_path for path strings
                                    node.file_path().log_path_debug() // Use log_*_debug for Option<PathBuf>
                                );
                                if !path_check { overall_check = false; }
                            } else {
                                // If expected_file_path_suffix is None for a FileBased module, it's an error in test data
                                log::warn!(target: #log_target, "Expected file path suffix is None for a FileBased module. This is likely an error in test data."); // String literal, no formatting needed
                                overall_check = false;
                            }

                            // Check file_attrs_count
                            let actual_file_attrs_count = node.file_attrs().map_or(0, |attrs| attrs.len());
                            let attrs_count_check = self.file_attrs_count == actual_file_attrs_count;
                            log::debug!(target: #log_target,
                                "   {: <23} {} | Expected file_attrs count '{}' == Actual file_attrs count '{}'",
                                "File Attrs Count Match?".to_string().log_step(), attrs_count_check.log_bool(),
                                self.file_attrs_count.log_name_debug(), // Use log_*_debug
                                actual_file_attrs_count.log_name_debug() // Use log_*_debug
                            );
                            if !attrs_count_check { overall_check = false; }

                            // Check file_docs_is_some
                            let actual_file_docs_is_some = node.file_docs().is_some();
                            let docs_is_some_check = self.file_docs_is_some == actual_file_docs_is_some;
                            log::debug!(target: #log_target,
                                "   {: <23} {} | Expected file_docs_is_some '{}' == Actual file_docs_is_some '{}'",
                                "File Docs Is Some Match?".to_string().log_step(), docs_is_some_check.log_bool(),
                                self.file_docs_is_some.log_name_debug(), // Use log_*_debug
                                actual_file_docs_is_some.log_name_debug() // Use log_*_debug
                            );
                            if !docs_is_some_check { overall_check = false; }

                        } else {
                            // If not FileBased, expected_file_path_suffix should be None
                            if self.expected_file_path_suffix.is_some() {
                                log::warn!(target: #log_target, "Expected file path suffix is Some for a non-FileBased module. This is likely an error in test data."); // String literal
                                overall_check = false;
                            }
                        }
                        overall_check
                    }
                });
                check_all_fields_logics.push(quote! {
                    if !self.#check_method_name_ident(node) { all_passed = false; }
                });
                // Add mod_disc to find_node_by_values_filters
                find_node_by_values_filters.push(quote! {
                    .filter(|n| {
                        let actual_disc = match n.module_def {
                            crate::parser::nodes::ModuleKind::FileBased { .. } => crate::parser::nodes::ModDisc::FileBased,
                            crate::parser::nodes::ModuleKind::Inline { .. } => crate::parser::nodes::ModDisc::Inline,
                            crate::parser::nodes::ModuleKind::Declaration { .. } => crate::parser::nodes::ModDisc::Declaration,
                        };
                        self.mod_disc == actual_disc
                    })
                });
            }

            // == FunctionNode Specific Handlers (Placed BEFORE the general skip arm) ==
            "parameters" // For FunctionNode
                if node_struct_name == "FunctionNode"
                   && matches!(field_type, Type::Path(p) if p.path.segments.last().is_some_and(|seg| seg.ident == "Vec")) =>
            {
                expected_fields_defs.push(quote! { pub parameter_count: usize });
                inherent_check_method_impls.push(quote! {
                    pub fn #check_method_name_ident(&self, node: &crate::parser::nodes::#node_struct_name) -> bool {
                        let actual_count = node.parameters.len();
                        let check = self.parameter_count == actual_count;
                        log::debug!(target: #log_target,
                            "   {: <23} {} | Expected count '{}' == Actual count '{}'",
                            "Parameter Count Match?".to_string().log_step(), check.log_bool(),
                            self.parameter_count.to_string().log_name(),
                            actual_count.to_string().log_name()
                        );
                        check
                    }
                });
                check_all_fields_logics.push(quote! {
                    if !self.#check_method_name_ident(node) { all_passed = false; }
                });
            }
            "generic_params" // For FunctionNode
                if node_struct_name == "FunctionNode"
                   && matches!(field_type, Type::Path(p) if p.path.segments.last().is_some_and(|seg| seg.ident == "Vec")) =>
            {
                expected_fields_defs.push(quote! { pub generic_param_count: usize });
                inherent_check_method_impls.push(quote! {
                    pub fn #check_method_name_ident(&self, node: &crate::parser::nodes::#node_struct_name) -> bool {
                        let actual_count = node.generic_params.len();
                        let check = self.generic_param_count == actual_count;
                        log::debug!(target: #log_target,
                            "   {: <23} {} | Expected count '{}' == Actual count '{}'",
                            "Generic Param Count Match?".to_string().log_step(), check.log_bool(),
                            self.generic_param_count.to_string().log_name(),
                            actual_count.to_string().log_name()
                        );
                        check
                    }
                });
                check_all_fields_logics.push(quote! {
                    if !self.#check_method_name_ident(node) { all_passed = false; }
                });
            }
            "return_type" // For FunctionNode
                if node_struct_name == "FunctionNode"
                   && matches!(field_type, Type::Path(p) if p.path.segments.last().is_some_and(|seg| seg.ident == "Option")) =>
            {
                expected_fields_defs.push(quote! { pub return_type_is_some: bool });
                inherent_check_method_impls.push(quote! {
                    pub fn #check_method_name_ident(&self, node: &crate::parser::nodes::#node_struct_name) -> bool {
                        let actual_is_some = node.return_type.is_some();
                        let check = self.return_type_is_some == actual_is_some;
                        log::debug!(target: #log_target,
                            "   {: <23} {} | Expected is_some '{}' == Actual is_some '{}'",
                            "Return Type Is Some Match?".to_string().log_step(), check.log_bool(),
                            self.return_type_is_some.to_string().log_name(),
                            actual_is_some.to_string().log_name()
                        );
                        check
                    }
                });
                check_all_fields_logics.push(quote! {
                    if !self.#check_method_name_ident(node) { all_passed = false; }
                });
            }
            "body" // For FunctionNode
                if node_struct_name == "FunctionNode"
                   && matches!(field_type, Type::Path(p) if p.path.segments.last().is_some_and(|seg| seg.ident == "Option")) =>
            {
                expected_fields_defs.push(quote! { pub body_is_some: bool });
                inherent_check_method_impls.push(quote! {
                    pub fn #check_method_name_ident(&self, node: &crate::parser::nodes::#node_struct_name) -> bool {
                        let actual_is_some = node.body.is_some();
                        let check = self.body_is_some == actual_is_some;
                        log::debug!(target: #log_target,
                            "   {: <23} {} | Expected is_some '{}' == Actual is_some '{}'",
                            "Body Is Some Match?".to_string().log_step(), check.log_bool(),
                            self.body_is_some.to_string().log_name(),
                            actual_is_some.to_string().log_name()
                        );
                        check
                    }
                });
                check_all_fields_logics.push(quote! {
                    if !self.#check_method_name_ident(node) { all_passed = false; }
                });
            }

            // == General Skip Arm ==
            // Fields skipped by default unless specifically handled below or for a specific node type
            "id" | "span" | "fields" | "variants" | "methods" | 
            // "imports" | "exports" | "module_def" | // Handled above for ModuleNode
            "super_traits"
            // Note: `kind` is handled below for ImportNode
             => {
                // These fields are typically not directly compared by value in Expected*Data,
                // or are handled by ID regeneration or specific relation checks.
                // Special skip for ModuleNode fields handled above
                if node_struct_name == "ModuleNode" && (field_name_str == "imports" || field_name_str == "exports" || field_name_str == "module_def" || field_name_str == "path") {
                    // Already handled, do nothing
                } else {
                    // Original skip logic for other nodes or unhandled fields
                }
                // We will not generate `is_*_match_debug` for them by default.
            }

            // == General Handlers ==
            "name" => {
                expected_fields_defs.push(quote! { pub name: &'static str });
                inherent_check_method_impls.push(quote! {
                    pub fn #check_method_name_ident(&self, node: &crate::parser::nodes::#node_struct_name) -> bool {
                        // Assuming node implements GraphNode trait which has name()
                        let check = self.name == node.name();
                        log::debug!(target: #log_target,
                            "   {: <23} {} | Expected '{}' == Actual '{}'",
                            "Name Match?".to_string().log_step(), check.log_bool(),
                            self.name.log_name(), node.name().log_name() // String, String
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
                            "   {: <23} {} | Expected '{}' == Actual '{}'",
                            "Visibility Match?".to_string().log_step(), check.log_bool(),
                            self.visibility.log_vis_debug(), node.visibility().log_vis_debug() // VisibilityKind, VisibilityKind
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
                            "   {: <23} {} | Expected '{}' == Actual '{}'",
                            "Attributes Match?".to_string().log_step(), check.log_bool(),
                            self.attributes.log_green_debug(), node.attributes().log_green_debug() // Vec<Attribute>, &[Attribute]
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
                // The field in Expected*Data is docstring: Option<&'static str>
                expected_fields_defs.push(quote! { pub docstring: Option<&'static str> });
                inherent_check_method_impls.push(quote! {
                    pub fn #check_method_name_ident(&self, node: &crate::parser::nodes::#node_struct_name) -> bool {
                        // Assuming node implements GraphNode trait which has docstring() -> Option<String> or Option<&str>
                        // The manual impl uses node.docstring.as_deref() which implies node.docstring is Option<String>
                        let actual_docstring = node.docstring.as_deref(); // Access field directly
                        let check_passes = match self.docstring {
                            Some(expected_doc) => actual_docstring.map_or(false, |s| s == expected_doc),
                            None => actual_docstring.is_none(),
                        };
                        log::debug!(target: #log_target,
                            "   {: <23} {} | Expected '{}' == Actual '{}'",
                            "Docstring Match?".to_string().log_step(), check_passes.log_bool(),
                            self.docstring.log_foreground_primary_debug(), // Option<&str>
                            actual_docstring.log_foreground_secondary_debug() // Option<&str>
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
                        format!("{:?}", actual_cfgs.sort_unstable());
                        let mut expected_cfgs_sorted = self.cfgs.clone();
                        format!("{:?}", expected_cfgs_sorted.sort_unstable());
                        let check = expected_cfgs_sorted == actual_cfgs;
                        log::debug!(target: #log_target,
                            "   {: <23} {} | Expected (sorted) '{}' == Actual (sorted) '{}'",
                            "CFGs Match?".to_string().log_step(), check.log_bool(),
                            expected_cfgs_sorted.log_green_debug(), actual_cfgs.log_green_debug() // Vec<String>, Vec<String>
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
                            "   {: <23} {} | Expected check pass '{}' == Actual check pass '{}'",
                            "TrackingHash Check Match?".to_string().log_step(), check.log_bool(),
                            self.tracking_hash_check.log_name_debug(), // bool
                            actual_check_passes.log_name_debug() // bool
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
                            "   {: <23} {} | Expected check pass '{}' == Actual check pass '{}'",
                            "TypeId Check Match?".to_string().log_step(), check.log_bool(),
                            self.type_id_check.log_name_debug(), // bool
                            actual_check_passes.log_name_debug() // bool
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
                            "   {: <23} {} | Expected '{}' == Actual '{}'",
                            "Value Match?".to_string().log_step(), check.log_bool(),
                            self.value.log_foreground_primary_debug(), // Option<&str>
                            actual_value.log_foreground_secondary_debug() // Option<&str>
                        );
                        check
                    }
                });
                check_all_fields_logics.push(quote! {
                    if !self.#check_method_name_ident(node) { all_passed = false; }
                });
                // Not adding to find_node_by_values_filters by default.
            }
            // == ImportNode Specific Handlers ==
            "source_path"
                if node_struct_name == "ImportNode"
                   && matches!(field_type, Type::Path(p) if p.path.segments.last().is_some_and(|seg| seg.ident == "Vec")) =>
            {
                expected_fields_defs.push(quote! { pub source_path: &'static [&'static str] });
                inherent_check_method_impls.push(quote! {
                    pub fn #check_method_name_ident(&self, node: &crate::parser::nodes::#node_struct_name) -> bool {
                        let expected_vec: Vec<String> = self.source_path.iter().map(|s| s.to_string()).collect();
                        let check = expected_vec == node.source_path; // Compare Vec<String> == Vec<String>
                        log::debug!(target: #log_target,
                            "   {: <23} {} | \n{: >35} {}\n{: >35} {}",
                            "Source Path Match?".to_string().log_step(), check.log_bool(),
                            "│ Expected: ",
                            expected_vec.log_name_debug(), // Log the Vec<String>
                            "│ Actual:   ",
                            node.source_path.log_name_debug()
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
            "visible_name"
                if node_struct_name == "ImportNode"
                   && matches!(field_type, Type::Path(p) if p.path.segments.last().is_some_and(|seg| seg.ident == "String")) =>
            {
                // Use the existing 'name' handler logic but target 'visible_name' field
                expected_fields_defs.push(quote! { pub visible_name: &'static str });
                inherent_check_method_impls.push(quote! {
                    pub fn #check_method_name_ident(&self, node: &crate::parser::nodes::#node_struct_name) -> bool {
                        let check = self.visible_name == node.visible_name; // Access field directly
                        log::debug!(target: #log_target,
                            "   {: <23} {} | Expected '{}' == Actual '{}'",
                            "Visible Name Match?".to_string().log_step(), check.log_bool(),
                            self.visible_name.log_name(), node.visible_name.log_name() // String, String
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
             "original_name"
                if node_struct_name == "ImportNode"
                   && matches!(field_type, Type::Path(p) if p.path.segments.last().is_some_and(|seg| seg.ident == "Option")) =>
            {
                expected_fields_defs.push(quote! { pub original_name: Option<&'static str> });
                inherent_check_method_impls.push(quote! {
                    pub fn #check_method_name_ident(&self, node: &crate::parser::nodes::#node_struct_name) -> bool {
                        let actual_original = node.original_name.as_deref();
                        let check = self.original_name == actual_original;
                        log::debug!(target: #log_target,
                            "   {: <23} {} | Expected '{}' == Actual '{}'",
                            "Original Name Match?".to_string().log_step(), check.log_bool(),
                            self.original_name.log_name_debug(), 
                            actual_original.log_name_debug() // Option<&str>, Option<&str>
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
             "is_glob"
                if node_struct_name == "ImportNode"
                   && matches!(field_type, Type::Path(p) if p.path.segments.last().is_some_and(|seg| seg.ident == "bool")) =>
            {
                expected_fields_defs.push(quote! { pub is_glob: bool });
                inherent_check_method_impls.push(quote! {
                    pub fn #check_method_name_ident(&self, node: &crate::parser::nodes::#node_struct_name) -> bool {
                        let check = self.is_glob == node.is_glob;
                        log::debug!(target: #log_target,
                            "   {: <23} {} | Expected '{}' == Actual '{}'",
                            "Is Glob Match?".to_string().log_step(), check.log_bool(),
                            self.is_glob.log_green_debug(), 
                            node.is_glob.log_green_debug() // bool, bool
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
             "is_self_import"
                if node_struct_name == "ImportNode"
                   && matches!(field_type, Type::Path(p) if p.path.segments.last().is_some_and(|seg| seg.ident == "bool")) =>
            {
                expected_fields_defs.push(quote! { pub is_self_import: bool });
                inherent_check_method_impls.push(quote! {
                    pub fn #check_method_name_ident(&self, node: &crate::parser::nodes::#node_struct_name) -> bool {
                        let check = self.is_self_import == node.is_self_import;
                        log::debug!(target: #log_target,
                            "   {: <23} {} | Expected '{}' == Actual '{}'",
                            "Is Self Import Match?".to_string().log_step(), check.log_bool(),
                            self.is_self_import.log_bool(), 
                            node.is_self_import.log_bool() // bool, bool
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
            "kind" // For ImportNode
                if node_struct_name == "ImportNode"
                   && matches!(field_type, Type::Path(p) if p.path.segments.last().is_some_and(|seg| seg.ident == "ImportKind")) =>
            {
                // Use full path in generated code for clarity
                expected_fields_defs.push(quote! { pub kind: crate::parser::nodes::ImportKind });
                inherent_check_method_impls.push(quote! {
                    pub fn #check_method_name_ident(&self, node: &crate::parser::nodes::#node_struct_name) -> bool {
                        let check = self.kind == node.kind;
                        log::debug!(target: #log_target,
                            "   {: <23} {} | Expected '{}' == Actual '{}'",
                            "Kind Match?".to_string().log_step(), check.log_bool(),
                            self.kind.log_vis_debug(), 
                            node.kind.log_vis_debug() // ImportKind, ImportKind
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

            // == End ImportNode Specific Handlers ==

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
                            "   {: <23} {} | Expected '{}' == Actual '{}'",
                            "Is Mutable Match?".to_string().log_step(), check.log_bool(),
                            self.is_mutable.log_name_debug(), // bool
                            actual_value.log_name_debug() // bool
                        );
                        check
                    }
                });
                check_all_fields_logics.push(quote! {
                    if !self.#check_method_name_ident(node) { all_passed = false; }
                });
                // Not adding to find_node_by_values_filters by default.
            }
            // Handle `parameters: Vec<ParamData>` for FunctionNode and MethodNode
            "parameters"
                if (node_struct_name == "FunctionNode" || node_struct_name == "MethodNode")
                    && matches!(field_type, Type::Path(p) if p.path.segments.last().is_some_and(|seg| seg.ident == "Vec")) =>
            {
                expected_fields_defs.push(quote! { pub parameter_count: usize });
                inherent_check_method_impls.push(quote! {
                    pub fn #check_method_name_ident(&self, node: &crate::parser::nodes::#node_struct_name) -> bool {
                        let actual_count = node.parameters.len();
                        let check = self.parameter_count == actual_count;
                        log::debug!(target: #log_target,
                            "   {: <23} {} | Expected count '{}' == Actual count '{}'",
                            "Parameter Count Match?".to_string().log_step(), check.log_bool(),
                            self.parameter_count.to_string().log_name(),
                            actual_count.to_string().log_name()
                        );
                        check
                    }
                });
                check_all_fields_logics.push(quote! {
                    if !self.#check_method_name_ident(node) { all_passed = false; }
                });
            }
            // Handle `generic_params: Vec<GenericParamNode>` for FunctionNode and MethodNode
            "generic_params"
                if (node_struct_name == "FunctionNode" || node_struct_name == "MethodNode")
                    && matches!(field_type, Type::Path(p) if p.path.segments.last().is_some_and(|seg| seg.ident == "Vec")) =>
            {
                expected_fields_defs.push(quote! { pub generic_param_count: usize });
                inherent_check_method_impls.push(quote! {
                    pub fn #check_method_name_ident(&self, node: &crate::parser::nodes::#node_struct_name) -> bool {
                        let actual_count = node.generic_params.len();
                        let check = self.generic_param_count == actual_count;
                        log::debug!(target: #log_target,
                            "   {: <23} {} | Expected count '{}' == Actual count '{}'",
                            "Generic Param Count Match?".to_string().log_step(), check.log_bool(),
                            self.generic_param_count.to_string().log_name(),
                            actual_count.to_string().log_name()
                        );
                        check
                    }
                });
                check_all_fields_logics.push(quote! {
                    if !self.#check_method_name_ident(node) { all_passed = false; }
                });
            }
            // Handle `return_type: Option<TypeId>` for FunctionNode and MethodNode
            "return_type"
                if (node_struct_name == "FunctionNode" || node_struct_name == "MethodNode")
                    && matches!(field_type, Type::Path(p) if p.path.segments.last().is_some_and(|seg| seg.ident == "Option")) =>
            {
                expected_fields_defs.push(quote! { pub return_type_is_some: bool });
                inherent_check_method_impls.push(quote! {
                    pub fn #check_method_name_ident(&self, node: &crate::parser::nodes::#node_struct_name) -> bool {
                        let actual_is_some = node.return_type.is_some();
                        let check = self.return_type_is_some == actual_is_some;
                        log::debug!(target: #log_target,
                            "   {: <23} {} | Expected is_some '{}' == Actual is_some '{}'",
                            "Return Type Is Some Match?".to_string().log_step(), check.log_bool(),
                            self.return_type_is_some.to_string().log_name(),
                            actual_is_some.to_string().log_name()
                        );
                        check
                    }
                });
                check_all_fields_logics.push(quote! {
                    if !self.#check_method_name_ident(node) { all_passed = false; }
                });
            }
            // Handle `body: Option<String>` for FunctionNode and MethodNode
            "body"
                if (node_struct_name == "FunctionNode" || node_struct_name == "MethodNode")
                    && matches!(field_type, Type::Path(p) if p.path.segments.last().is_some_and(|seg| seg.ident == "Option")) =>
            {
                expected_fields_defs.push(quote! { pub body_is_some: bool });
                inherent_check_method_impls.push(quote! {
                    pub fn #check_method_name_ident(&self, node: &crate::parser::nodes::#node_struct_name) -> bool {
                        let actual_is_some = node.body.is_some();
                        let check = self.body_is_some == actual_is_some;
                        log::debug!(target: #log_target,
                            "   {: <23} {} | Expected is_some '{}' == Actual is_some '{}'",
                            "Body Is Some Match?".to_string().log_step(), check.log_bool(),
                            self.body_is_some.to_string().log_name(),
                            actual_is_some.to_string().log_name()
                        );
                        check
                    }
                });
                check_all_fields_logics.push(quote! {
                    if !self.#check_method_name_ident(node) { all_passed = false; }
                });
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
        "MethodNode" => quote! { impls }, // Placeholder, find_node_by_values will be special-cased
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
        "MethodNode" => quote! {}, // No extra filtering needed, find_node_by_values is special-cased
        _ => quote! {},            // No extra filtering needed for other node types
    };

    // --- Generate the inherent impl block for Expected*Data ---
    let find_node_by_values_body = if node_struct_name == "MethodNode" {
        quote! {
            // For MethodNode, value-based search across a single top-level collection is not straightforward
            // as methods are nested. Return an empty iterator for now.
            // Tests will rely on ID-based lookup.
            std::iter::empty()
        }
    } else {
        quote! {
             // Use helper methods directly in inspect calls
             parsed.graph.#graph_collection_field.iter()
                 #type_def_filter // Apply TypeDefNode filtering if necessary
                 .inspect(move |node_candidate| self.log_target_id(node_candidate)) // Use helper method
                 #(#find_node_by_values_filters)* // Apply *selective* filters
                 .inspect(move |node_candidate| self.log_all_match(node_candidate)) // Use helper method
             // No Box::new needed
        }
    };

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
                #find_node_by_values_body
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
