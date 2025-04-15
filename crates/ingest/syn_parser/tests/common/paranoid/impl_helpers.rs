use ploke_common::fixtures_crates_dir;
use ploke_core::{NodeId, TypeId};
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

    // 3. Generate expected TypeIds based on parsing the input strings
    //    This mimics how the visitor likely generated the ID using get_or_create_type
    let expected_self_type_id = {
        let parsed_type = syn::parse_str::<syn::Type>(self_type_str)
            .expect("Failed to parse self_type_str for TypeId generation");
        // Generate the synthetic ID based on the parsed type's string representation
        TypeId::generate_synthetic(
            crate_namespace,
            file_path,
            &parsed_type.to_token_stream().to_string(),
        )
    };

    let expected_trait_type_id: Option<TypeId> = trait_type_str.map(|tts| {
        let parsed_type = syn::parse_str::<syn::Type>(tts)
            .expect("Failed to parse trait_type_str for TypeId generation");
        TypeId::generate_synthetic(
            crate_namespace,
            file_path,
            &parsed_type.to_token_stream().to_string(),
        )
    });

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
        &expected_name,   // Use the generated name
        ItemKind::Impl,   // Pass the correct ItemKind
        None,             // Pass None for parent_scope_id (temporary)
    );

    // We compare the regenerated ID against the actual ID found on the node.
    // NOTE: This check might be brittle if the `expected_name` generation here
    // doesn't perfectly match the one used inside the visitor's `add_contains_rel` call.
    assert_eq!(
        impl_id, regenerated_id,
        "Mismatch between node's actual ID ({}) and regenerated ID ({}) for impl block '{}' in file '{}' (ItemKind: {:?}, ParentScope: None). Name generation might be the cause.",
        impl_id, regenerated_id, expected_name, file_path.display(), ItemKind::Impl
    );

    // 8. Return the validated node
    impl_node
}
