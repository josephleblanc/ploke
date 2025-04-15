use ploke_common::fixtures_crates_dir;
use ploke_core::{ItemKind, NodeId, TypeId};
use quote::ToTokens;
use syn_parser::parser::nodes::*;
use syn_parser::parser::visitor::ParsedCodeGraph;

/// Finds the specific ParsedCodeGraph for the target file, then finds the ImplNode
/// within that graph based on type info, performs paranoid checks, and returns a reference.
/// Panics if the graph or node is not found, or if uniqueness checks fail.
pub fn find_impl_node_paranoid<'a>(
    parsed_graphs: &'a [ParsedCodeGraph], // Operate on the collection
    fixture_name: &str,                   // Needed to construct expected path
    relative_file_path: &str,             // e.g., "src/lib.rs" or "src/impls.rs"
    expected_module_path: &[String],      // Module path within the target file
    self_type_str: &str, // Expected type string (e.g., "SimpleStruct", "GenericStruct<T>")
    trait_type_str: Option<&str>, // Expected trait string (e.g., "SimpleTrait", "GenericTrait<T>")
) -> &'a ImplNode {
    // 1. Construct the absolute expected file path
    let fixture_root = fixtures_crates_dir().join(fixture_name);
    let target_file_path = fixture_root.join(relative_file_path);

    // 2. Find the specific ParsedCodeGraph for the target file
    let target_data = parsed_graphs
        .iter()
        .find(|data| data.file_path == target_file_path)
        .unwrap_or_else(|| {
            panic!(
                "ParsedCodeGraph for '{}' not found in results",
                target_file_path.display()
            )
        });

    let graph = &target_data.graph;
    let crate_namespace = target_data.crate_namespace;
    let file_path = &target_data.file_path; // Use the path from the found graph data

    // 3. Generate expected TypeIds by simulating the structural analysis
    //    Helper closure to perform the simulation
    let generate_expected_type_id_for_test = |type_str: &str| -> TypeId {
        let parsed_type = syn::parse_str::<syn::Type>(type_str)
            .unwrap_or_else(|_| panic!("Failed to parse type string for TypeId generation: {}", type_str));

        match parsed_type {
            syn::Type::Path(type_path) => {
                // Extract base path segments (e.g., ["GenericStruct"])
                let base_path: Vec<String> = type_path.path.segments.iter().map(|seg| seg.ident.to_string()).collect();
                let mut related_ids = Vec::new();

                // Extract generic arguments if present
                if let Some(last_segment) = type_path.path.segments.last() {
                    if let syn::PathArguments::AngleBracketed(args) = &last_segment.arguments {
                        for arg in &args.args {
                            if let syn::GenericArgument::Type(gen_type) = arg {
                                // Simulate getting TypeId for the generic argument (e.g., "T")
                                // Generate ID based on its name directly for test simplicity
                                let gen_type_str = gen_type.to_token_stream().to_string();
                                let gen_type_kind = ploke_core::TypeKind::Named {
                                    path: vec![gen_type_str], // Use the generic param name as path
                                    is_fully_qualified: false, // Assume false for simple generic param
                                };
                                let gen_related_ids: &[TypeId] = &[]; // Generic param itself has no related types here
                                related_ids.push(TypeId::generate_synthetic(
                                    crate_namespace,
                                    file_path,
                                    &gen_type_kind,
                                    gen_related_ids,
                                ));
                            }
                            // TODO: Handle other GenericArgument types (Lifetime, Const) if needed for future tests
                        }
                    }
                    // TODO: Handle PathArguments::Parenthesized if needed
                }

                // Construct the TypeKind for the main path
                let type_kind = ploke_core::TypeKind::Named {
                    path: base_path,
                    is_fully_qualified: type_path.qself.is_some(),
                };

                // Generate the final TypeId using the base TypeKind and collected related IDs
                TypeId::generate_synthetic(
                    crate_namespace,
                    file_path,
                    &type_kind,
                    &related_ids, // Pass collected related IDs
                )
            }
            // TODO: Handle other syn::Type variants (Reference, Tuple, etc.) if needed by tests using this helper
            _ => {
                 panic!("generate_expected_type_id_for_test only handles Type::Path currently, received: {}", type_str);
            }
        }
    };

    // Use the helper closure to generate expected IDs
    let expected_self_type_id = generate_expected_type_id_for_test(self_type_str);
    let expected_trait_type_id: Option<TypeId> = trait_type_str.map(generate_expected_type_id_for_test);


    // 4. Filter candidates by matching self_type and trait_type IDs
    let type_candidates: Vec<&ImplNode> = graph
        .impls
        .iter()
        .filter(|imp| {
            imp.self_type == expected_self_type_id && imp.trait_type == expected_trait_type_id
        })
        .collect();

    assert!(
        !type_candidates.is_empty(),
        "No ImplNode found matching self_type '{}' ({:?}) and trait_type '{:?}' ({:?}) in file '{}'",
        self_type_str, expected_self_type_id, trait_type_str, expected_trait_type_id, file_path.display()
    );

    // 5. Filter further by module association
    let module_node = graph
        .modules
        .iter()
        .find(|m| m.path == expected_module_path)
        .unwrap_or_else(|| {
            panic!(
                "ModuleNode not found for path: {:?} in file '{}'",
                expected_module_path,
                file_path.display()
            )
        });

    let module_candidates: Vec<&ImplNode> = type_candidates
        .into_iter()
        .filter(|imp| module_node.items().is_some_and(|m| m.contains(&imp.id())))
        .collect();

    // 6. PARANOID CHECK: Assert exactly ONE candidate remains
    assert_eq!(
        module_candidates.len(),
        1,
        "Expected exactly one ImplNode matching types and associated with module path {:?} in file '{}', found {}",
        expected_module_path,
        file_path.display(),
        module_candidates.len()
    );

    let impl_node = module_candidates[0];
    let impl_id = impl_node.id();
    // let actual_span = impl_node.span; // Span no longer used for ID generation

    // 7. PARANOID CHECK: Regenerate expected ID using node's context and ItemKind
    //    Need to generate the expected name based on type strings.
    let expected_name = match trait_type_str {
        Some(t) => format!("impl {} for {}", t, self_type_str),
        None => format!("impl {}", self_type_str),
    };
    // Note: This name generation might differ slightly from the visitor if to_string() representations vary.
    // It assumes simple type strings are sufficient.

    let regenerated_id = NodeId::generate_synthetic(
        crate_namespace,
        file_path,
        expected_module_path,
        &expected_name,       // Use the generated name
        ItemKind::Impl,       // Pass the correct ItemKind
        Some(module_node.id), // Pass the containing module's ID as parent scope
    );

    // We compare the regenerated ID against the actual ID found on the node.
    // NOTE: This check might be brittle if the `expected_name` generation here
    // doesn't perfectly match the one used inside the visitor's `add_contains_rel` call.
    assert_eq!(
        impl_id, regenerated_id,
        "Mismatch between node's actual ID ({}) and regenerated ID ({}) for impl block '{}' in file '{}' (ItemKind: {:?}, ParentScope: {:?}). Name generation might be the cause.",
        impl_id, regenerated_id, expected_name, file_path.display(), ItemKind::Impl, Some(module_node.id)
    );

    // 8. Return the validated node
    impl_node
}
