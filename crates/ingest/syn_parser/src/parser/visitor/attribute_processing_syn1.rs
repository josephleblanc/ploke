use crate::parser::nodes::Attribute;
use crate::parser::utils::convert_attribute_syn1_to_syn2;

// --- Functions for Item-Level (Outer) Attributes ---

/// Extracts the outer docstring (`///` or `/** ... */`) from item attributes.
pub(crate) fn extract_docstring(attrs: &[syn1::Attribute]) -> Option<String> {
    // Convert syn1::Attribute to syn::Attribute, skipping unconvertible ones
    let converted: Vec<syn::Attribute> = attrs
        .iter()
        .filter_map(|attr| convert_attribute_syn1_to_syn2(attr).ok())
        .collect();
    super::attribute_processing::extract_docstring(&converted)
}

/// Parses attributes into our custom Attribute struct.
pub(crate) fn extract_attributes(attrs: &[syn1::Attribute]) -> Vec<Attribute> {
    // Convert syn1::Attribute to syn::Attribute, skipping unconvertible ones
    let converted: Vec<syn::Attribute> = attrs
        .iter()
        .filter_map(|attr| convert_attribute_syn1_to_syn2(attr).ok())
        .collect();
    super::attribute_processing::extract_attributes(&converted)
}

/// Checks if an item should be included based on its cfg attributes.
pub(crate) fn should_include_item(
    attrs: &[syn1::Attribute],
    active_cfg: &super::cfg_evaluator::ActiveCfg,
) -> bool {
    // Convert syn1::Attribute to syn::Attribute, skipping unconvertible ones
    let converted: Vec<syn::Attribute> = attrs
        .iter()
        .filter_map(|attr| convert_attribute_syn1_to_syn2(attr).ok())
        .collect();
    super::attribute_processing::should_include_item(&converted, active_cfg)
}

/// Extracts cfg strings from attributes.
pub(crate) fn extract_cfg_strings(attrs: &[syn1::Attribute]) -> Vec<String> {
    // Convert syn1::Attribute to syn::Attribute, skipping unconvertible ones
    let converted: Vec<syn::Attribute> = attrs
        .iter()
        .filter_map(|attr| convert_attribute_syn1_to_syn2(attr).ok())
        .collect();
    super::attribute_processing::extract_cfg_strings(&converted)
}

/// Extracts file-level docstring from inner attributes (`#!`).
pub(crate) fn extract_file_level_docstring(attrs: &[syn1::Attribute]) -> Option<String> {
    // Convert syn1::Attribute to syn::Attribute, skipping unconvertible ones
    let converted: Vec<syn::Attribute> = attrs
        .iter()
        .filter_map(|attr| convert_attribute_syn1_to_syn2(attr).ok())
        .collect();
    super::attribute_processing::extract_file_level_docstring(&converted)
}

/// Extracts file-level attributes from inner attributes (`#!`).
pub(crate) fn extract_file_level_attributes(attrs: &[syn1::Attribute]) -> Vec<Attribute> {
    // Convert syn1::Attribute to syn::Attribute, skipping unconvertible ones
    let converted: Vec<syn::Attribute> = attrs
        .iter()
        .filter_map(|attr| convert_attribute_syn1_to_syn2(attr).ok())
        .collect();
    super::attribute_processing::extract_file_level_attributes(&converted)
}

/// Parses cfg expression from inner tokens.
/// This function is syn-independent and can be re-exported from attribute_processing.
pub fn parse_cfg_expr_from_inner_tokens(inner: &str) -> Option<super::cfg_evaluator::CfgExpr> {
    super::attribute_processing::parse_cfg_expr_from_inner_tokens(inner)
}
