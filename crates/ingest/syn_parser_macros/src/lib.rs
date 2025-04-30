extern crate proc_macro;

use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::{parse_macro_input, Data, DeriveInput, Fields, Ident, Type, Visibility};

/// Derives a `pub(crate) struct *NodeInfo` based on the fields of the struct this is applied to.
///
/// The generated `*NodeInfo` struct will have:
/// - The same visibility as the original struct (assumed `pub` within the crate).
/// - A name derived by appending "Info" to the original struct name (e.g., `StructNode` -> `StructNodeInfo`).
/// - A `pub id: ploke_core::NodeId` field.
/// - All other public fields from the original struct, preserving their names and types.
/// - Basic derives: `Debug`, `Clone`, `PartialEq`.
///
/// # Example Usage
/// ```ignore
/// use syn_parser_macros::GenerateNodeInfo;
/// use ploke_core::{NodeId, StructNodeId}; // Assuming these exist
/// // ... other necessary imports ...
///
/// #[derive(GenerateNodeInfo)]
/// pub struct StructNode {
///     pub id: StructNodeId, // Typed ID in the original struct
///     pub name: String,
///     pub span: (usize, usize),
///     // ... other fields
/// }
///
/// // This will generate:
/// // #[derive(Debug, Clone, PartialEq)]
/// // pub(crate) struct StructNodeInfo {
/// //     pub id: ploke_core::NodeId, // Raw NodeId
/// //     pub name: String,
/// //     pub span: (usize, usize),
/// //     // ... other fields copied from StructNode
/// // }
/// ```
///
/// The original struct must still define a `pub(crate) fn new(info: *NodeInfo) -> Self` constructor
/// that takes the generated `*NodeInfo` struct and correctly wraps the `info.id` into the typed ID.
#[proc_macro_derive(GenerateNodeInfo)]
pub fn generate_node_info_derive(input: TokenStream) -> TokenStream {
    // Parse the input tokens into a syntax tree (DeriveInput represents a struct/enum/union)
    let input = parse_macro_input!(input as DeriveInput);

    // Extract the name of the struct the derive is attached to (e.g., "StructNode")
    let original_struct_name = &input.ident;

    // Create the name for the generated *NodeInfo struct (e.g., "StructNodeInfo")
    let info_struct_name = format_ident!("{}Info", original_struct_name);

    // Ensure the input is a struct
    let fields = match &input.data {
        Data::Struct(data_struct) => match &data_struct.fields {
            Fields::Named(fields_named) => &fields_named.named,
            _ => panic!("GenerateNodeInfo can only be derived on structs with named fields"),
        },
        _ => panic!("GenerateNodeInfo can only be derived on structs"),
    };

    // --- Generate the fields for the *NodeInfo struct ---
    // We iterate through the fields of the original struct and copy them,
    // except for the 'id' field, which we replace.

    let mut info_fields = Vec::new();
    // Add the mandatory 'id: NodeId' field first.
    // IMPORTANT: Adjust the path `ploke_core::NodeId` if NodeId is located elsewhere
    // relative to the crate where the *Node structs are defined (syn_parser).
    info_fields.push(quote! {
        /// The raw NodeId before being wrapped into a typed ID.
        pub id: ::ploke_core::NodeId
    });

    // Iterate over the fields of the original struct
    for field in fields {
        let field_name = field.ident.as_ref().expect("Named fields should have names");
        let field_type = &field.ty;
        let field_vis = &field.vis; // Capture original visibility

        // Skip the original 'id' field, as we've already added our own 'id: NodeId'
        if field_name == "id" {
            continue;
        }

        // Add the field to our list for the generated struct, preserving visibility
        info_fields.push(quote! {
            #field_vis #field_name : #field_type
        });
    }

    // --- Generate the *NodeInfo struct definition ---
    // We use the 'quote' macro to construct the Rust code as tokens.

    let generated_struct = quote! {
        // Apply standard derives. Add more if needed (e.g., Serialize, Deserialize)
        // but keep them minimal for these intermediate structs unless necessary.
        #[derive(Debug, Clone, PartialEq)]
        // Make the generated struct pub(crate)
        pub(crate) struct #info_struct_name {
            // Add the collected fields, separated by commas
            #(#info_fields),*
        }
    };

    // Convert the generated tokens back into a TokenStream and return it
    TokenStream::from(generated_struct)
}
