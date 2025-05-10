extern crate proc_macro;
use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::{parse_macro_input, Data, DeriveInput, Fields, Type};

/// Derives a `pub(crate) struct *NodeInfo` based on the fields of the struct this is applied to.
///
/// The generated `*NodeInfo` struct will have:
/// - The same visibility as the original struct (assumed `pub` within the crate).
/// - A name derived by appending "Info" to the original struct name (e.g., `StructNode` -> `StructNodeInfo`).
/// - A `pub id: ploke_core::NodeId` field.
/// - All other public fields from the original struct, preserving their names and types.
/// - Basic derives on the `*NodeInfo` struct: `Debug`, `Clone`, `PartialEq`.
/// - A `pub(crate) fn new(info: *NodeInfo) -> Self` constructor method implemented on the original struct.
///
/// # Example Usage
/// ```ignore
/// // removed GenerateNodeInfo
/// use ploke_core::{StructNodeId}; // Assuming these exist
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
        let field_name = field
            .ident
            .as_ref()
            .expect("Named fields should have names");
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

    // --- Generate the fields for the `new` method constructor body ---
    let mut constructor_fields = Vec::new();
    let mut id_type: Option<&Type> = None; // To store the type of the original 'id' field

    for field in fields {
        let field_name = field
            .ident
            .as_ref()
            .expect("Named fields should have names");
        let field_type = &field.ty;

        if field_name == "id" {
            // Store the type of the original 'id' field (e.g., StructNodeId)
            id_type = Some(field_type);
            // Generate the ID wrapping line for the constructor using the restricted ::create method
            constructor_fields.push(quote! {
                #field_name : #field_type::create(info.id) // Use the restricted constructor
            });
        } else {
            // Generate simple field copying for other fields
            // Generate simple field copying for other fields
            constructor_fields.push(quote! {
                #field_name : info.#field_name
            });
        }
    }

    // Ensure the original struct had an 'id' field
    let _ = id_type.expect("GenerateNodeInfo requires the struct to have an 'id' field.");

    // --- Generate the *NodeInfo struct definition ---
    let generated_info_struct = quote! {
        // Apply standard derives. Add more if needed (e.g., Serialize, Deserialize)
        // but keep them minimal for these intermediate structs unless necessary.
        #[derive(Debug, Clone, PartialEq)]
        // Make the generated struct pub(crate)
        pub(crate) struct #info_struct_name {
            // Add the collected fields, separated by commas
            #(#info_fields),*
        }
    };

    // --- Generate the `impl` block with the `new` constructor ---
    let generated_impl = quote! {
        impl #original_struct_name {
            /// Creates a new instance from the corresponding `*NodeInfo` struct.
            /// This constructor is typically `pub(crate)` and handles wrapping the raw `NodeId`
            /// from the info struct into the specific typed ID for this node.
            #[inline] // Suggest inlining for simple constructors
            pub(crate) fn new(info: #info_struct_name) -> Self {
                Self {
                    // Add the generated constructor field assignments
                    #(#constructor_fields),*
                }
            }
        }
    };

    // --- Combine generated struct and impl block ---
    let final_output = quote! {
        #generated_info_struct
        #generated_impl
    };

    // Convert the combined tokens back into a TokenStream and return it
    TokenStream::from(final_output)
}
