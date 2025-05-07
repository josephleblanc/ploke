//! Procedural macros for reducing boilerplate in ploke tests.

extern crate proc_macro;

use proc_macro::TokenStream;
use proc_macro2::Span;
use proc_macro_error::{abort, proc_macro_error};
use quote::quote; // Removed unused format_ident, ToTokens
use syn::{
    parse::{Parse, ParseStream},
    parse_macro_input,
    Expr,
    ItemFn,
    Lit,
    Meta,
    MetaNameValue,
    // Removed NestedMeta
    Result as SynResult,
    Token,
    // Removed unused types: AttributeArgs, FnArg, Ident, Pat, PatType, Path, Stmt, Type, Visibility
};

// Assuming ItemKind is accessible via syn_parser::ItemKind
use ploke_core::ItemKind;
// Removed unused: use syn_parser;

// Helper struct to parse the arguments to the paranoid_test attribute
#[derive(Debug)] // Added Debug derive
struct ParanoidTestArgs {
    kind: ItemKind,
    ident: String,
    fixture: String,
    relative_file_path: String,
    expected_path: Vec<String>,        // Store as Vec<String> directly
    expected_cfg: Option<Vec<String>>, // Store as Option<Vec<String>>
}

impl Parse for ParanoidTestArgs {
    fn parse(input: ParseStream) -> SynResult<Self> {
        let mut kind: Option<ItemKind> = None;
        let mut ident: Option<String> = None;
        let mut fixture: Option<String> = None;
        let mut relative_file_path: Option<String> = None;
        let mut expected_path: Option<Vec<String>> = None;
        let mut expected_cfg: Option<Vec<String>> = None; // Initialize as None

        // Parse the input stream as a punctuated sequence of Meta items
        let args = syn::punctuated::Punctuated::<Meta, Token![,]>::parse_terminated(input)?;

        for arg in args {
            match arg {
                // Match directly on Meta::NameValue
                Meta::NameValue(MetaNameValue { path, value, .. }) => {
                    let key = path
                        .get_ident()
                        .ok_or_else(|| syn::Error::new_spanned(&path, "Expected identifier key"))?;
                    let key_str = key.to_string();

                    // Ensure the value is a literal expression
                    let lit = match value {
                        Expr::Lit(expr_lit) => expr_lit.lit,
                        _ => return Err(syn::Error::new_spanned(value, "Expected literal value")),
                    };

                    match key_str.as_str() {
                        "kind" => {
                            if let Lit::Str(lit_str) = &lit {
                                // Borrow lit here
                                // Map string to ItemKind variant
                                kind = match lit_str.value().as_str() {
                                    "Const" => Some(ItemKind::Const),
                                    "Static" => Some(ItemKind::Static),
                                    "Function" => Some(ItemKind::Function),
                                    "Struct" => Some(ItemKind::Struct),
                                    "Enum" => Some(ItemKind::Enum),
                                    "Union" => Some(ItemKind::Union),
                                    "TypeAlias" => Some(ItemKind::TypeAlias),
                                    "Trait" => Some(ItemKind::Trait),
                                    "Impl" => Some(ItemKind::Impl),
                                    "Module" => Some(ItemKind::Module),
                                    "Macro" => Some(ItemKind::Macro),
                                    "Import" => Some(ItemKind::Import),
                                    // Add other kinds as needed
                                    _ => {
                                        return Err(syn::Error::new_spanned(
                                            lit_str,
                                            "Unsupported ItemKind string",
                                        ))
                                    }
                                };
                            } else {
                                return Err(syn::Error::new_spanned(
                                    &lit,
                                    "Expected string literal for kind",
                                )); // Pass borrowed lit
                            }
                        }
                        "ident" => {
                            if let Lit::Str(lit_str) = &lit {
                                // Borrow lit here
                                ident = Some(lit_str.value());
                            } else {
                                return Err(syn::Error::new_spanned(
                                    &lit,
                                    "Expected string literal for ident",
                                )); // Pass borrowed lit
                            }
                        }
                        "fixture" => {
                            if let Lit::Str(lit_str) = &lit {
                                // Borrow lit here
                                fixture = Some(lit_str.value());
                            } else {
                                return Err(syn::Error::new_spanned(
                                    &lit,
                                    "Expected string literal for fixture",
                                )); // Pass borrowed lit
                            }
                        }
                        "relative_file_path" => {
                            if let Lit::Str(lit_str) = &lit {
                                // Borrow lit here
                                relative_file_path = Some(lit_str.value());
                            } else {
                                return Err(syn::Error::new_spanned(
                                    &lit,
                                    "Expected string literal for relative_file_path",
                                )); // Pass borrowed lit
                            }
                        }
                        "expected_path" => {
                            if let Lit::Str(lit_str) = &lit {
                                // Borrow lit here
                                // Assume comma-separated string like "crate,module,item"
                                expected_path =
                                    Some(lit_str.value().split(',').map(String::from).collect());
                            } else {
                                return Err(syn::Error::new_spanned(&lit, "Expected string literal for expected_path (e.g., \"crate,module\")"));
                                // Pass borrowed lit
                            }
                        }
                        "expected_cfg" => {
                            if let Lit::Str(lit_str) = &lit {
                                // Borrow lit here
                                // Assume comma-separated string like "cfg1,cfg2"
                                expected_cfg =
                                    Some(lit_str.value().split(',').map(String::from).collect());
                            } else {
                                return Err(syn::Error::new_spanned(&lit, "Expected string literal for expected_cfg (e.g., \"cfg1,cfg2\")"));
                                // Pass borrowed lit
                            }
                        }
                        _ => return Err(syn::Error::new_spanned(key, "Unknown argument name")),
                    }
                }
                // Handle other Meta types if necessary, or error out
                _ => {
                    return Err(syn::Error::new_spanned(
                        arg,
                        "Expected key = \"value\" format",
                    ))
                }
            }
        }

        // Check for missing mandatory arguments
        let kind =
            kind.ok_or_else(|| syn::Error::new(Span::call_site(), "Missing 'kind' argument"))?;
        let ident =
            ident.ok_or_else(|| syn::Error::new(Span::call_site(), "Missing 'ident' argument"))?;
        let fixture = fixture
            .ok_or_else(|| syn::Error::new(Span::call_site(), "Missing 'fixture' argument"))?;
        let relative_file_path = relative_file_path.ok_or_else(|| {
            syn::Error::new(Span::call_site(), "Missing 'relative_file_path' argument")
        })?;
        let expected_path = expected_path.ok_or_else(|| {
            syn::Error::new(Span::call_site(), "Missing 'expected_path' argument")
        })?;
        // expected_cfg is optional, defaults to None if not provided

        Ok(ParanoidTestArgs {
            kind,
            ident,
            fixture,
            relative_file_path,
            expected_path,
            expected_cfg,
        })
    }
}

#[proc_macro_error]
#[proc_macro_attribute]
pub fn paranoid_test(args: TokenStream, input: TokenStream) -> TokenStream {
    let test_fn = parse_macro_input!(input as ItemFn);
    let test_fn_name = &test_fn.sig.ident;
    let test_fn_vis = &test_fn.vis;
    let test_fn_attrs = &test_fn.attrs; // Keep original attributes like #[ignore]

    let parsed_args = parse_macro_input!(args as ParanoidTestArgs);

    // Extract values from parsed_args
    let kind_enum = parsed_args.kind; // This is ItemKind enum value
    let ident_str = parsed_args.ident;
    let fixture_str = parsed_args.fixture;
    let rel_path_str = parsed_args.relative_file_path;
    let expected_path_vec = parsed_args.expected_path;
    let expected_cfg_opt_vec = parsed_args.expected_cfg;

    // --- Mapping from ItemKind to Test Data ---
    // We need the path to the test module where Expected*Data and maps are defined.
    // This assumes the macro is invoked from within a test module inside
    // `syn_parser/tests/uuid_phase2_partial_graphs/nodes/`.
    // If invoked elsewhere, these paths need adjustment.
    let test_module_path = quote! { crate::uuid_phase2_partial_graphs::nodes::const_static }; // Adjust if needed

    // Construct the path to the ItemKind variant for quoting
    let kind_path = match kind_enum {
        ItemKind::Const => quote! { ploke_core::ItemKind::Const },
        ItemKind::Static => quote! { ploke_core::ItemKind::Static },
        ItemKind::Function => quote! { ploke_core::ItemKind::Function },
        ItemKind::Struct => quote! { ploke_core::ItemKind::Struct },
        ItemKind::Enum => quote! { ploke_core::ItemKind::Enum },
        ItemKind::Union => quote! { ploke_core::ItemKind::Union },
        ItemKind::TypeAlias => quote! { ploke_core::ItemKind::TypeAlias },
        ItemKind::Trait => quote! { ploke_core::ItemKind::Trait },
        ItemKind::Impl => quote! { ploke_core::ItemKind::Impl },
        ItemKind::Module => quote! { ploke_core::ItemKind::Module },
        ItemKind::Macro => quote! { ploke_core::ItemKind::Macro },
        ItemKind::Import => quote! { ploke_core::ItemKind::Import },
        // Add other kinds as needed
        _ => abort!(
            Span::call_site(),
            "Unsupported ItemKind for #[paranoid_test]: {:?}",
            kind_enum
        ),
    };

    let (node_type, expected_data_type, expected_data_map, downcast_method, specific_checks) =
        match kind_enum {
            // Match on the enum value here
            ItemKind::Const => (
                // NOTE: Removing lines that will be brough into scope by test invocation site that uses references
                // quote! { syn_parser::parser::nodes::ConstNode },
                quote! { ConstNode },
                quote! { #test_module_path::ExpectedConstData },
                quote! { #test_module_path::EXPECTED_CONSTS_DATA },
                quote! { as_const },
                // Specific checks for ConstNode
                quote! {
                    assert!(expected_data.is_type_id_check_match_debug(specific_node), "Type ID check mismatch");
                    assert!(expected_data.is_value_match_debug(specific_node), "Value mismatch");
                },
            ),
            ItemKind::Static => (
                // NOTE: Removing lines that will be brough into scope by test invocation site that uses references
                // quote! { syn_parser::parser::nodes::StaticNode },
                quote! { StaticNode },
                quote! { #test_module_path::ExpectedStaticData },
                quote! { #test_module_path::EXPECTED_STATICS_DATA },
                quote! { as_static },
                // Specific checks for StaticNode
                quote! {
                    assert!(expected_data.is_type_id_check_match_debug(specific_node), "Type ID check mismatch");
                    assert!(expected_data.is_mutable_match_debug(specific_node), "Mutability mismatch");
                    assert!(expected_data.is_value_match_debug(specific_node), "Value mismatch");
                },
            ),
            // Add other ItemKind mappings here...
            // ItemKind::Function => (quote! { FunctionNode }, quote! { ExpectedFunctionData }, ...),
            _ => abort!(
                Span::call_site(),
                "Unsupported ItemKind for #[paranoid_test]: {:?}",
                kind_enum // Use the correct variable name
            ),
        };

    // Convert expected_path Vec<String> to &[&str] literal for ParanoidArgs
    let expected_path_strs: Vec<syn::LitStr> = expected_path_vec
        .iter()
        .map(|s| syn::LitStr::new(s, Span::call_site()))
        .collect();
    let expected_path_slice = quote! { &[#(#expected_path_strs),*] };

    // Convert optional expected_cfg Vec<String> to Option<&[&str]> literal
    let expected_cfg_slice = match expected_cfg_opt_vec {
        Some(cfgs) => {
            let cfg_strs: Vec<syn::LitStr> = cfgs
                .iter()
                .map(|s| syn::LitStr::new(s, Span::call_site()))
                .collect();
            quote! { Some(&[#(#cfg_strs),*]) }
        }
        None => quote! { None },
    };

    // Generate the test function body
    // NOTE: Removing lines that will be brough into scope by test invocation site that uses references
    let expanded = quote! {
        #(#test_fn_attrs)* // Keep original attributes
        #test_fn_vis fn #test_fn_name() -> Result<(), SynParserError> {
            // NOTE: Removing the following dependencies, which are assumed to have been imported
            // in the test file where these macros are invoked.
            // Use fully qualified paths to avoid import issues in generated code
            // Use crate:: instead of syn_parser:: where possible if invoked from syn_parser tests
            // use syn_parser::parser::graph::GraphAccess;
            // use syn_parser::parser::nodes::GraphNode;
            // use syn_parser::parser::nodes::HasAttributes; // Needed for ExpectedNodeData trait bounds
            // use syn_parser::parser::nodes::PrimaryNodeIdTrait; // Needed for to_pid()
            // use #test_module_path::ExpectedNodeData; // Import the base trait

            let _ = env_logger::builder()
                .is_test(true)
                .format_timestamp(None)
                .try_init();

            // 1. Run phases
            let successful_graphs = run_phases_and_collect(#fixture_str);

            // Removing this line, relying on `ParanoidArgs` being in scope at test site.
            // let args = syn_parser::tests::common::ParanoidArgs {
            // 2. Define ParanoidArgs
            let args = ParanoidArgs {
                fixture: #fixture_str,
                relative_file_path: #rel_path_str,
                ident: #ident_str,
                expected_path: #expected_path_slice,
                item_kind: #kind_path, // Use the quoted path to the ItemKind variant
                expected_cfg: #expected_cfg_slice,
            };

            // 3. Get Expected Data
            let expected_data = #expected_data_map
                .get(args.ident)
                .unwrap_or_else(|| panic!("{} not found for ident: {}", stringify!(#expected_data_type), args.ident));

            // 4. Find the target ParsedCodeGraph
            let target_graph_data = successful_graphs
                .iter()
                .find(|pg| pg.file_path.ends_with(args.relative_file_path))
                .unwrap_or_else(|| {
                    panic!(
                        "Target graph '{}' not found for item '{}'.",
                        args.relative_file_path, args.ident
                    )
                });

            args.check_graph(target_graph_data)?; // Log graph context

            // 5. Attempt ID-based lookup and individual field checks
            match args.generate_pid(&successful_graphs) {
                 Ok(test_info) => {
                    match test_info.target_data().find_node_unique(test_info.test_pid().into()) {
                        Ok(node) => {
                            // Downcast based on kind
                            if let Some(specific_node) = node.#downcast_method() {
                                log::info!(target: LOG_TEST_CONST, "Performing individual field checks for '{}' via ID lookup.", args.ident);
                                // Call all check methods from the base trait
                                assert!(expected_data.is_name_match_debug(specific_node), "Name mismatch");
                                assert!(expected_data.is_vis_match_debug(specific_node), "Visibility mismatch");
                                assert!(expected_data.is_attr_match_debug(specific_node), "Attribute mismatch");
                                assert!(expected_data.is_docstring_contains_match_debug(specific_node), "Docstring mismatch");
                                assert!(expected_data.is_tracking_hash_check_match_debug(specific_node), "Tracking hash mismatch");
                                assert!(expected_data.is_cfgs_match_debug(specific_node), "CFG mismatch");

                                // Call specific check methods generated based on kind
                                #specific_checks

                                // --- Relation Check ---
                                let expected_path_vec: Vec<String> = args.expected_path.iter().map(|s| s.to_string()).collect();
                                let parent_module = target_graph_data.find_module_by_path_checked(&expected_path_vec)?;
                                let parent_module_id = parent_module.module_id();
                                // Assuming the specific node type has an `id` field that returns the specific typed ID
                                let node_primary_id = specific_node.id.to_pid();

                                let relation_found = target_graph_data.relations().iter().any(|rel| {
                                    matches!(rel, syn_parser::parser::relations::SyntacticRelation::Contains { source, target }
                                        if *source == parent_module_id && *target == node_primary_id)
                                });

                                assert!(
                                    relation_found,
                                    "Missing SyntacticRelation::Contains from parent module {} to node {}",
                                    parent_module_id, specific_node.id // Assuming specific_node has `id`
                                );
                                log::debug!(target: LOG_TEST_CONST, "   Relation Check: Found Contains relation from parent module.");
                                // --- End Relation Check ---

                            } else {
                                panic!("Node found by ID for '{}' was not a {}.", args.ident, stringify!(#node_type));
                            }
                        }
                        Err(e) => {
                             log::warn!(target: LOG_TEST_CONST, "Node lookup by PID '{}' failed for '{}' (Error: {:?}). Proceeding with value-based check only.", test_info.test_pid(), args.ident, e);
                        }
                    }
                }
                Err(e) => {
                     log::warn!(target: LOG_TEST_CONST, "PID generation failed for '{}' (Error: {:?}). Proceeding with value-based check only.", args.ident, e);
                }
            }

            // 6. Perform value-based lookup and count assertion
            log::info!(target: LOG_TEST_CONST, "Performing value-based lookup for '{}'.", args.ident);
            // Use the find_node_by_values method from the trait
            let matching_nodes_by_value: Vec<_> = expected_data.find_node_by_values(target_graph_data).collect();
            assert_eq!(
                matching_nodes_by_value.len(),
                1,
                "Expected to find exactly one {} matching values for '{}'. Found {}.",
                stringify!(#node_type),
                args.ident,
                matching_nodes_by_value.len()
            );

            Ok(())
        }
    };

    expanded.into()
}
