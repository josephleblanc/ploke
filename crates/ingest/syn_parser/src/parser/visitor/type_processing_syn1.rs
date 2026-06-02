use super::state::VisitorState;
use crate::parser::type_slots::{OrdinaryTypeUseId, TraitTypeUseId};
use crate::parser::utils::convert_type_syn1_to_syn2;
use quote::ToTokens;

/// Gets or creates a TypeId for a given syn1::Type.
///
/// This function converts the syn1::Type to syn::Type and delegates to the
/// shared type processing logic in type_processing.rs.
///
/// # Arguments
/// * `state` - Mutable visitor state containing the type cache and code graph.
/// * `ty` - The syn1::Type to get an ID for.
///
/// # Returns
/// The `TypeId` (Synthetic variant in Phase 2) for the given type.
pub(crate) fn get_or_create_type(state: &mut VisitorState, ty: &syn1::Type) -> OrdinaryTypeUseId {
    // Convert syn1::Type to syn::Type, then delegate to shared processing
    let syn2_ty = convert_type_syn1_to_syn2(ty);
    super::type_processing::get_or_create_type(state, &syn2_ty)
}

pub(crate) fn get_or_create_trait_bound_type(
    state: &mut VisitorState,
    bound: &syn1::TraitBound,
) -> TraitTypeUseId {
    let syn2_bound = syn::parse2::<syn::TraitBound>(bound.to_token_stream())
        .expect("syn1 trait bound should parse as syn2 trait bound");
    super::type_processing::get_or_create_trait_bound_type(state, &syn2_bound)
}
