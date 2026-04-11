use colored::Colorize;
use ploke_core::ItemKind;
use ploke_core::byte_hasher::ByteHasher;
use quote::ToTokens;
use std::{collections::HashMap, hash::Hasher};
use syn::visit::Visit;
use tracing::instrument;
mod attribute_processing;
mod attribute_processing_syn1;
mod cfg_evaluator;
#[cfg(feature = "cfg_eval")]
pub use attribute_processing::parse_cfg_expr_from_inner_tokens;
#[cfg(feature = "cfg_eval")]
pub use cfg_evaluator::ActiveCfg;
mod code_visitor;
mod code_visitor_syn1;
mod state;
mod type_processing;
mod type_processing_syn1;

pub use code_visitor::CodeVisitor;
pub use state::VisitorState;

use crate::{
    error::SynParserError,
    parser::{
        diagnostics::{TRACE_TARGET_INVARIANTS, emit_json_diagnostic},
        graph::GraphAccess,
        nodes::{ModuleNodeInfo, PrimaryNodeId},
        relations::SyntacticRelation,
    },
    utils::{LOG_TARGET_RELS, LogStyle, LogStyleDebug},
};

use std::path::{Component, Path, PathBuf}; // Add Path and Component

#[cfg(feature = "convert_keyword_2015")]
#[derive(Debug, Clone)]
struct LegacyKeywordRewrite {
    insertion_points: Vec<usize>,
}

#[cfg(feature = "convert_keyword_2015")]
fn try_parse_file_with_legacy_keyword_fallback(
    file_content: &str,
    crate_context: &crate::discovery::CrateContext,
) -> Result<(syn::File, Option<LegacyKeywordRewrite>), syn::Error> {
    // Historical motivation and before/after behavior are covered in
    // `crates/ploke-eval/src/tests/replay.rs` via the feature-on/off ripgrep
    // setup replay variants.
    match syn::parse_file(file_content) {
        Ok(file) => Ok((file, None)),
        Err(original_err) => {
            if !should_attempt_legacy_keyword_fallback(crate_context, &original_err, file_content) {
                return Err(original_err);
            }

            tracing::warn!(
                target: TRACE_TARGET_INVARIANTS,
                file_path = %crate_context.root_path.display(),
                error = %original_err,
                "convert_keyword_2015: retrying parse with Rust 2015 keyword rewrite fallback"
            );

            let Some((rewritten, insertion_points)) =
                rewrite_edition_2015_keywords(file_content, &["async"])
            else {
                tracing::warn!(
                    target: TRACE_TARGET_INVARIANTS,
                    file_path = %crate_context.root_path.display(),
                    "convert_keyword_2015: fallback matched error shape but found no rewrite candidates"
                );
                return Err(original_err);
            };

            tracing::info!(
                target: TRACE_TARGET_INVARIANTS,
                file_path = %crate_context.root_path.display(),
                rewrite_count = insertion_points.len(),
                insertion_points = ?insertion_points,
                "convert_keyword_2015: generated rewritten source for retry"
            );

            match syn::parse_file(&rewritten) {
                Ok(file) => {
                    tracing::info!(
                        target: TRACE_TARGET_INVARIANTS,
                        file_path = %crate_context.root_path.display(),
                        "convert_keyword_2015: fallback parse succeeded"
                    );
                    Ok((file, Some(LegacyKeywordRewrite { insertion_points })))
                }
                Err(retry_err) => {
                    tracing::warn!(
                        target: TRACE_TARGET_INVARIANTS,
                        file_path = %crate_context.root_path.display(),
                        retry_error = %retry_err,
                        "convert_keyword_2015: fallback parse failed; returning original parse error"
                    );
                    Err(original_err)
                }
            }
        }
    }
}

#[cfg(not(feature = "convert_keyword_2015"))]
fn try_parse_file_with_legacy_keyword_fallback(
    file_content: &str,
    _crate_context: &crate::discovery::CrateContext,
) -> Result<(syn::File, Option<()>), syn::Error> {
    syn::parse_file(file_content).map(|file| (file, None))
}

#[cfg(feature = "convert_keyword_2015")]
fn should_attempt_legacy_keyword_fallback(
    crate_context: &crate::discovery::CrateContext,
    err: &syn::Error,
    file_content: &str,
) -> bool {
    err.to_string()
        .contains("expected identifier, found keyword")
        && file_content.contains("async")
        && crate_effective_edition(crate_context)
            .is_some_and(|edition| edition == cargo_toml::Edition::E2015)
}

#[cfg(feature = "convert_keyword_2015")]
fn crate_effective_edition(
    crate_context: &crate::discovery::CrateContext,
) -> Option<cargo_toml::Edition> {
    crate_effective_edition_inner(crate_context)
}

// Unconditional version for dual-syn dispatch
fn crate_effective_edition_inner(
    crate_context: &crate::discovery::CrateContext,
) -> Option<cargo_toml::Edition> {
    let manifest_path = crate_context.root_path.join("Cargo.toml");
    let manifest = cargo_toml::Manifest::from_path(&manifest_path).ok()?;
    manifest.package.as_ref().map(cargo_toml::Package::edition)
}

#[cfg(feature = "convert_keyword_2015")]
fn rewrite_edition_2015_keywords(source: &str, keywords: &[&str]) -> Option<(String, Vec<usize>)> {
    let bytes = source.as_bytes();
    let mut out = String::with_capacity(source.len());
    let mut insertions = Vec::new();
    let mut i = 0usize;

    while i < bytes.len() {
        if let Some(next) = consume_line_comment(source, i, &mut out) {
            i = next;
            continue;
        }
        if let Some(next) = consume_block_comment(source, i, &mut out) {
            i = next;
            continue;
        }
        if let Some(next) = consume_raw_string(source, i, &mut out) {
            i = next;
            continue;
        }
        if let Some(next) = consume_quoted_literal(source, i, &mut out) {
            i = next;
            continue;
        }
        if let Some(next) = consume_byte_quoted_literal(source, i, &mut out) {
            i = next;
            continue;
        }
        if let Some(next) = consume_lifetime(source, i, &mut out) {
            i = next;
            continue;
        }
        if let Some(next) = consume_raw_identifier(source, i, &mut out) {
            i = next;
            continue;
        }

        if is_ascii_ident_start(bytes[i]) {
            let start = i;
            i += 1;
            while i < bytes.len() && is_ascii_ident_continue(bytes[i]) {
                i += 1;
            }
            let ident = &source[start..i];
            if keywords.contains(&ident) {
                insertions.push(out.len());
                out.push_str("r#");
            }
            out.push_str(ident);
            continue;
        }

        out.push(bytes[i] as char);
        i += 1;
    }

    if insertions.is_empty() {
        None
    } else {
        Some((out, insertions))
    }
}

#[cfg(feature = "convert_keyword_2015")]
fn consume_line_comment(source: &str, i: usize, out: &mut String) -> Option<usize> {
    let bytes = source.as_bytes();
    if !bytes[i..].starts_with(b"//") {
        return None;
    }
    let mut end = i + 2;
    while end < bytes.len() && bytes[end] != b'\n' {
        end += 1;
    }
    out.push_str(&source[i..end]);
    Some(end)
}

#[cfg(feature = "convert_keyword_2015")]
fn consume_block_comment(source: &str, i: usize, out: &mut String) -> Option<usize> {
    let bytes = source.as_bytes();
    if !bytes[i..].starts_with(b"/*") {
        return None;
    }
    let mut depth = 1usize;
    let mut end = i + 2;
    while end < bytes.len() && depth > 0 {
        if bytes[end..].starts_with(b"/*") {
            depth += 1;
            end += 2;
        } else if bytes[end..].starts_with(b"*/") {
            depth -= 1;
            end += 2;
        } else {
            end += 1;
        }
    }
    out.push_str(&source[i..end.min(bytes.len())]);
    Some(end.min(bytes.len()))
}

#[cfg(feature = "convert_keyword_2015")]
fn consume_raw_string(source: &str, i: usize, out: &mut String) -> Option<usize> {
    let bytes = source.as_bytes();
    let prefix_len = if bytes[i..].starts_with(b"br") {
        2
    } else if bytes[i] == b'r' {
        1
    } else {
        return None;
    };

    let mut cursor = i + prefix_len;
    while cursor < bytes.len() && bytes[cursor] == b'#' {
        cursor += 1;
    }
    if cursor >= bytes.len() || bytes[cursor] != b'"' {
        return None;
    }

    let hash_count = cursor - (i + prefix_len);
    cursor += 1;
    while cursor < bytes.len() {
        if bytes[cursor] == b'"' {
            let mut matches = true;
            for idx in 0..hash_count {
                if cursor + 1 + idx >= bytes.len() || bytes[cursor + 1 + idx] != b'#' {
                    matches = false;
                    break;
                }
            }
            if matches {
                let end = (cursor + 1 + hash_count).min(bytes.len());
                out.push_str(&source[i..end]);
                return Some(end);
            }
        }
        cursor += 1;
    }

    out.push_str(&source[i..bytes.len()]);
    Some(bytes.len())
}

#[cfg(feature = "convert_keyword_2015")]
fn consume_quoted_literal(source: &str, i: usize, out: &mut String) -> Option<usize> {
    let bytes = source.as_bytes();
    let quote = bytes[i];
    if quote != b'"' && quote != b'\'' {
        return None;
    }
    if quote == b'\'' && i + 1 < bytes.len() && is_ascii_ident_start(bytes[i + 1]) {
        return None;
    }

    let mut end = i + 1;
    let mut escaped = false;
    while end < bytes.len() {
        let byte = bytes[end];
        if escaped {
            escaped = false;
        } else if byte == b'\\' {
            escaped = true;
        } else if byte == quote {
            end += 1;
            out.push_str(&source[i..end]);
            return Some(end);
        }
        end += 1;
    }

    out.push_str(&source[i..bytes.len()]);
    Some(bytes.len())
}

#[cfg(feature = "convert_keyword_2015")]
fn consume_byte_quoted_literal(source: &str, i: usize, out: &mut String) -> Option<usize> {
    let bytes = source.as_bytes();
    if i + 1 >= bytes.len() || bytes[i] != b'b' || (bytes[i + 1] != b'"' && bytes[i + 1] != b'\'') {
        return None;
    }
    let mut cursor = i + 1;
    let quote = bytes[cursor];
    cursor += 1;
    let mut escaped = false;
    while cursor < bytes.len() {
        let byte = bytes[cursor];
        if escaped {
            escaped = false;
        } else if byte == b'\\' {
            escaped = true;
        } else if byte == quote {
            cursor += 1;
            out.push_str(&source[i..cursor]);
            return Some(cursor);
        }
        cursor += 1;
    }
    out.push_str(&source[i..bytes.len()]);
    Some(bytes.len())
}

#[cfg(feature = "convert_keyword_2015")]
fn consume_lifetime(source: &str, i: usize, out: &mut String) -> Option<usize> {
    let bytes = source.as_bytes();
    if bytes[i] != b'\'' || i + 1 >= bytes.len() || !is_ascii_ident_start(bytes[i + 1]) {
        return None;
    }
    let mut end = i + 2;
    while end < bytes.len() && is_ascii_ident_continue(bytes[end]) {
        end += 1;
    }
    out.push_str(&source[i..end]);
    Some(end)
}

#[cfg(feature = "convert_keyword_2015")]
fn consume_raw_identifier(source: &str, i: usize, out: &mut String) -> Option<usize> {
    let bytes = source.as_bytes();
    if i + 2 >= bytes.len()
        || bytes[i] != b'r'
        || bytes[i + 1] != b'#'
        || !is_ascii_ident_start(bytes[i + 2])
    {
        return None;
    }
    let mut end = i + 3;
    while end < bytes.len() && is_ascii_ident_continue(bytes[end]) {
        end += 1;
    }
    out.push_str(&source[i..end]);
    Some(end)
}

#[cfg(feature = "convert_keyword_2015")]
const fn is_ascii_ident_start(byte: u8) -> bool {
    byte == b'_' || byte.is_ascii_alphabetic()
}

#[cfg(feature = "convert_keyword_2015")]
const fn is_ascii_ident_continue(byte: u8) -> bool {
    is_ascii_ident_start(byte) || byte.is_ascii_digit()
}

#[cfg(feature = "convert_keyword_2015")]
fn remap_span_to_original(span: (usize, usize), insertion_points: &[usize]) -> (usize, usize) {
    (
        remap_offset_to_original(span.0, insertion_points),
        remap_offset_to_original(span.1, insertion_points),
    )
}

#[cfg(feature = "convert_keyword_2015")]
fn remap_offset_to_original(offset: usize, insertion_points: &[usize]) -> usize {
    let inserted_before = insertion_points
        .iter()
        .take_while(|pos| **pos < offset)
        .count();
    offset.saturating_sub(inserted_before * 2)
}

#[cfg(feature = "convert_keyword_2015")]
fn normalize_rewritten_graph(graph: &mut crate::parser::CodeGraph, insertion_points: &[usize]) {
    for function in &mut graph.functions {
        function.span = remap_span_to_original(function.span, insertion_points);
        normalize_rewritten_name(&mut function.name);
        normalize_generic_params(&mut function.generic_params);
    }

    for type_def in &mut graph.defined_types {
        match type_def {
            crate::parser::nodes::TypeDefNode::Struct(node) => {
                node.span = remap_span_to_original(node.span, insertion_points);
                normalize_rewritten_name(&mut node.name);
                normalize_generic_params(&mut node.generic_params);
                for field in &mut node.fields {
                    if let Some(name) = &mut field.name {
                        normalize_rewritten_name(name);
                    }
                }
            }
            crate::parser::nodes::TypeDefNode::Enum(node) => {
                node.span = remap_span_to_original(node.span, insertion_points);
                normalize_rewritten_name(&mut node.name);
                normalize_generic_params(&mut node.generic_params);
                for variant in &mut node.variants {
                    normalize_rewritten_name(&mut variant.name);
                    for field in &mut variant.fields {
                        if let Some(name) = &mut field.name {
                            normalize_rewritten_name(name);
                        }
                    }
                }
            }
            crate::parser::nodes::TypeDefNode::TypeAlias(node) => {
                node.span = remap_span_to_original(node.span, insertion_points);
                normalize_rewritten_name(&mut node.name);
                normalize_generic_params(&mut node.generic_params);
            }
            crate::parser::nodes::TypeDefNode::Union(node) => {
                node.span = remap_span_to_original(node.span, insertion_points);
                normalize_rewritten_name(&mut node.name);
                normalize_generic_params(&mut node.generic_params);
                for field in &mut node.fields {
                    if let Some(name) = &mut field.name {
                        normalize_rewritten_name(name);
                    }
                }
            }
        }
    }

    for impl_node in &mut graph.impls {
        impl_node.span = remap_span_to_original(impl_node.span, insertion_points);
        normalize_generic_params(&mut impl_node.generic_params);
        for method in &mut impl_node.methods {
            method.span = remap_span_to_original(method.span, insertion_points);
            normalize_rewritten_name(&mut method.name);
            normalize_generic_params(&mut method.generic_params);
        }
    }

    for trait_node in &mut graph.traits {
        trait_node.span = remap_span_to_original(trait_node.span, insertion_points);
        normalize_rewritten_name(&mut trait_node.name);
        normalize_generic_params(&mut trait_node.generic_params);
        for method in &mut trait_node.methods {
            method.span = remap_span_to_original(method.span, insertion_points);
            normalize_rewritten_name(&mut method.name);
            normalize_generic_params(&mut method.generic_params);
        }
    }

    for module in &mut graph.modules {
        module.span = remap_span_to_original(module.span, insertion_points);
        normalize_rewritten_name(&mut module.name);
        for segment in &mut module.path {
            normalize_rewritten_name(segment);
        }
        match &mut module.module_def {
            crate::parser::nodes::ModuleKind::Inline { span, .. } => {
                *span = remap_span_to_original(*span, insertion_points);
            }
            crate::parser::nodes::ModuleKind::Declaration {
                declaration_span, ..
            } => {
                *declaration_span = remap_span_to_original(*declaration_span, insertion_points);
            }
            crate::parser::nodes::ModuleKind::FileBased { .. } => {}
        }
        for import in &mut module.imports {
            normalize_import_node(import, insertion_points);
        }
    }

    for const_node in &mut graph.consts {
        const_node.span = remap_span_to_original(const_node.span, insertion_points);
        normalize_rewritten_name(&mut const_node.name);
    }

    for static_node in &mut graph.statics {
        static_node.span = remap_span_to_original(static_node.span, insertion_points);
        normalize_rewritten_name(&mut static_node.name);
    }

    for macro_node in &mut graph.macros {
        macro_node.span = remap_span_to_original(macro_node.span, insertion_points);
        normalize_rewritten_name(&mut macro_node.name);
    }

    for import in &mut graph.use_statements {
        normalize_import_node(import, insertion_points);
    }

    for unresolved in &mut graph.unresolved_nodes {
        unresolved.span = remap_span_to_original(unresolved.span, insertion_points);
        normalize_rewritten_name(&mut unresolved.name);
    }
}

#[cfg(feature = "convert_keyword_2015")]
fn traced_normalize_rewritten_graph(
    graph: &mut crate::parser::CodeGraph,
    insertion_points: &[usize],
    file_path: &Path,
) {
    tracing::info!(
        target: TRACE_TARGET_INVARIANTS,
        file_path = %file_path.display(),
        rewrite_count = insertion_points.len(),
        "convert_keyword_2015: normalizing graph spans and names back to original source"
    );
    normalize_rewritten_graph(graph, insertion_points);
    tracing::debug!(
        target: TRACE_TARGET_INVARIANTS,
        file_path = %file_path.display(),
        functions = graph.functions.len(),
        impls = graph.impls.len(),
        modules = graph.modules.len(),
        "convert_keyword_2015: graph normalization complete"
    );
}

#[cfg(feature = "convert_keyword_2015")]
fn normalize_import_node(
    import: &mut crate::parser::nodes::ImportNode,
    insertion_points: &[usize],
) {
    import.span = remap_span_to_original(import.span, insertion_points);
    normalize_rewritten_name(&mut import.visible_name);
    if let Some(original_name) = &mut import.original_name {
        normalize_rewritten_name(original_name);
    }
    for segment in &mut import.source_path {
        normalize_rewritten_name(segment);
    }
}

#[cfg(feature = "convert_keyword_2015")]
fn normalize_generic_params(generic_params: &mut [crate::parser::types::GenericParamNode]) {
    for param in generic_params {
        match &mut param.kind {
            crate::parser::types::GenericParamKind::Type { name, .. }
            | crate::parser::types::GenericParamKind::Lifetime { name, .. }
            | crate::parser::types::GenericParamKind::Const { name, .. } => {
                normalize_rewritten_name(name);
            }
        }
    }
}

#[cfg(feature = "convert_keyword_2015")]
fn normalize_rewritten_name(name: &mut String) {
    if let Some(stripped) = name.strip_prefix("r#") {
        *name = stripped.to_string();
    }
}

/// Derives the logical module path for a source file the same way Phase 2 parallel parsing does.
///
/// Must stay in sync with [`analyze_files_parallel`]. External tools (e.g. `xtask parse debug
/// logical-paths`) use this to explain per-file path assignment before merge.
///
/// Examples (paths under the crate `src` directory):
///  `.../src/main.rs` / `lib.rs` -> `["crate"]`
///  `.../src/foo.rs` -> `["crate", "foo"]`
///  `.../src/foo/mod.rs` -> `["crate", "foo"]`
///
/// Does not apply `#[path = "..."]` on `mod` items; the module tree resolves those later.
///
/// Do not use for `NodeId::Resolved` / `TypeId::Resolved` construction.
pub fn logical_module_path_for_file(crate_src_dir: &Path, file_path: &Path) -> Vec<String> {
    let mut logical_path = vec!["crate".to_string()];

    // Get the path relative to the src directory
    if let Ok(relative_path) = file_path.strip_prefix(crate_src_dir) {
        let mut components: Vec<String> = relative_path
            .components()
            .filter_map(|comp| match comp {
                Component::Normal(name) => name.to_str().map(|s| s.to_string()),
                _ => None,
            })
            .collect();

        // Check if the last component is a filename like "mod.rs" or "lib.rs" or "main.rs"
        if let Some(last) = components.last() {
            if last == "mod.rs" || last == "lib.rs" || last == "main.rs" {
                components.pop(); // Remove "mod.rs", "lib.rs", or "main.rs"
            } else if let Some(stem) = Path::new(&last.clone())
                .file_stem()
                .and_then(|s| s.to_str())
            {
                // Replace the filename with its stem
                if let Some(last_mut) = components.last_mut() {
                    *last_mut = stem.to_string();
                }
            }
        }
        logical_path.extend(components);
    } else {
        // Fallback or error handling if strip_prefix fails
        // For now, just return ["crate"] as a basic fallback
        log::debug!(
            "Warning: Could not strip prefix '{}' from '{}'. Falling back to ['crate'].",
            crate_src_dir.display(),
            file_path.display()
        );
    }

    logical_path
}

use super::ParsedCodeGraph;

use {
    super::nodes::ModuleNode, // Moved ModuleNode import here
    crate::discovery::{DiscoveryOutput, TargetKind, TargetSpec},
    ploke_core::NodeId,
    rayon::prelude::*, // Import rayon traits
    uuid::Uuid,
};

/// Analyze a file using syn1 (for Rust 2015 edition)
fn analyze_file_phase2_syn1(
    file_content: String,
    file_path: PathBuf,
    crate_namespace: Uuid,
    logical_module_path: Vec<String>,
    crate_context: &crate::discovery::CrateContext,
) -> Result<ParsedCodeGraph, syn::Error> {
    use super::nodes::ModuleKind;
    
    // Parse with syn1
    let file = syn1::parse_file(&file_content).map_err(|e| {
        syn::Error::new(proc_macro2::Span::call_site(), format!("syn1 parse error: {}", e))
    })?;

    // Create VisitorState
    let mut state =
        state::VisitorState::new(crate_namespace, file_path.to_path_buf(), crate_context);
    state.current_module_path = logical_module_path.clone();

    // Extract CFG strings using syn1
    let file_cfgs = attribute_processing_syn1::extract_cfg_strings(&file.attrs);
    state.current_scope_cfgs = file_cfgs.clone();
    let root_cfg_bytes = calculate_cfg_hash_bytes(&file_cfgs);

    // Generate root module ID
    let root_module_name = logical_module_path
        .last()
        .cloned()
        .unwrap_or_else(|| "crate".to_string());
    let root_module_parent_path: Vec<String> = logical_module_path
        .iter()
        .take(logical_module_path.len().saturating_sub(1))
        .cloned()
        .collect();

    let root_module_node_id = NodeId::generate_synthetic(
        crate_namespace,
        &file_path,
        &root_module_parent_path,
        &root_module_name,
        ItemKind::Module,
        None,
        root_cfg_bytes.as_deref(),
    );

    // Determine visibility
    let root_visibility = if logical_module_path == ["crate"] {
        crate::parser::types::VisibilityKind::Public
    } else {
        crate::parser::types::VisibilityKind::Inherited
    };

    // Create root module info
    let root_module_info = ModuleNodeInfo {
        id: root_module_node_id,
        name: root_module_name,
        visibility: root_visibility,
        attributes: Vec::new(),
        docstring: None,
        imports: Vec::new(),
        exports: Vec::new(),
        path: logical_module_path.clone(),
        span: (0, file_content.len()),
        tracking_hash: Some(state.generate_tracking_hash(&file.to_token_stream())),
        module_def: ModuleKind::FileBased {
            items: Vec::new(),
            file_path: file_path.clone(),
            file_attrs: attribute_processing_syn1::extract_file_level_attributes(&file.attrs),
            file_docs: attribute_processing_syn1::extract_file_level_docstring(&file.attrs),
        },
        cfgs: file_cfgs,
    };

    state.code_graph.modules.push(ModuleNode::new(root_module_info));

    let root_module_pid: PrimaryNodeId = state.code_graph.modules[0].id.into();
    state.current_primary_defn_scope.push(root_module_pid);

    // Create and run syn1 visitor
    let mut visitor = code_visitor_syn1::CodeVisitor::new(&mut state);
    syn1::visit::Visit::visit_file(&mut visitor, &file);

    drop(visitor);

    Ok(ParsedCodeGraph {
        file_path,
        graph: state.code_graph,
        crate_namespace,
        crate_context: Some(crate_context.clone()),
    })
}

/// Analyze a single file for Phase 2 (UUID Path) - The Worker Function
/// Receives context from analyze_files_parallel.
#[cfg(feature = "cfg_eval")]
#[instrument(target = TRACE_TARGET_INVARIANTS, skip(crate_context), fields(root_module_name))]
pub fn analyze_file_phase2(
    file_path: PathBuf,
    crate_namespace: Uuid,            // Context passed from caller
    logical_module_path: Vec<String>, // NEW: The derived logical path for this file
    crate_context: &crate::discovery::CrateContext,
) -> Result<ParsedCodeGraph, syn::Error> {
    // Consider a more specific Phase2Error later

    use super::nodes::ModuleKind;
    use attribute_processing::{
        extract_cfg_strings, // NEW: Import raw string extractor
        extract_file_level_attributes,
        extract_file_level_docstring,
        // Removed parse_and_combine_cfgs_from_attrs import
    };

    let file_content = std::fs::read_to_string(&file_path).map_err(|e| {
        syn::Error::new(
            proc_macro2::Span::call_site(),
            format!("Failed to read file {}: {}", file_path.display(), e),
        )
    })?;

    // Check edition for dual-syn dispatch
    let edition = crate_effective_edition_inner(crate_context);
    if edition == Some(cargo_toml::Edition::E2015) {
        // Use syn1 for Rust 2015 edition (handles bare trait objects)
        return analyze_file_phase2_syn1(
            file_content,
            file_path,
            crate_namespace,
            logical_module_path,
            crate_context,
        );
    }

    // Use syn2 for Rust 2018+ editions
    let (file, legacy_rewrite) =
        try_parse_file_with_legacy_keyword_fallback(&file_content, crate_context)?;

    // 1. Create VisitorState with the provided context
    let mut state =
        state::VisitorState::new(crate_namespace, file_path.to_path_buf(), crate_context);
    // Set the correct initial module path for the visitor
    state.current_module_path = logical_module_path.clone();

    // Extract raw file-level CFG strings (#![cfg(...)])
    let file_cfgs = extract_cfg_strings(&file.attrs);
    // Set the initial scope CFGs for the visitor state
    state.current_scope_cfgs = file_cfgs.clone();
    // Hash the file-level CFG strings for the root module ID
    let root_cfg_bytes = calculate_cfg_hash_bytes(&file_cfgs);

    // 2. Generate root module ID using the derived logical path context AND CFG context
    let root_module_name = logical_module_path
        .last()
        .cloned()
        .unwrap_or_else(|| "crate".to_string()); // Use last segment as name, fallback to "crate"
    let root_module_parent_path: Vec<String> = logical_module_path
        .iter()
        .take(logical_module_path.len().saturating_sub(1)) // Get parent path segments
        .cloned()
        .collect();

    let root_module_node_id = NodeId::generate_synthetic(
        crate_namespace,
        &file_path,
        &root_module_parent_path, // Use parent path for ID generation context
        &root_module_name,
        ItemKind::Module,          // Pass correct ItemKind
        None,                      // Root module has no parent scope ID within the file context
        root_cfg_bytes.as_deref(), // Pass hashed file-level CFG bytes
    );
    // #[cfg(test)]
    debug_file_module_id_gen(
        crate_namespace,
        &file_path,
        &root_module_parent_path,
        &root_module_name,
        ItemKind::Module,
        None,
        root_cfg_bytes.as_deref(),
    );

    // 3. Create the root module node using the derived path and name
    // Determine visibility: Public only for crate root (main.rs/lib.rs), Inherited otherwise
    let root_visibility = if logical_module_path == ["crate"] {
        crate::parser::types::VisibilityKind::Public
    } else {
        crate::parser::types::VisibilityKind::Inherited
    };

    let root_module_info = ModuleNodeInfo {
        id: root_module_node_id,
        name: root_module_name,      // Use derived name
        visibility: root_visibility, // Use determined visibility
        attributes: Vec::new(),
        docstring: None,
        imports: Vec::new(),
        exports: Vec::new(),
        path: logical_module_path.clone(), // Use derived path
        span: (0, file_content.len()),
        tracking_hash: Some(state.generate_tracking_hash(&file.to_token_stream())),
        module_def: ModuleKind::FileBased {
            items: Vec::new(),
            file_path: file_path.clone(),
            file_attrs: extract_file_level_attributes(&file.attrs), // Non-CFG attributes
            file_docs: extract_file_level_docstring(&file.attrs),
            // cfgs removed from here, belongs on ModuleNode
        },
        cfgs: file_cfgs, // Store raw file-level CFGs on the ModuleNode
    };

    state
        .code_graph
        .modules
        .push(ModuleNode::new(root_module_info));

    let root_module_pid: PrimaryNodeId = state.code_graph.modules[0].id.into();

    // Default parent scope for top-level items visited next.
    state.current_primary_defn_scope.push(root_module_pid);

    // 4. Create and run the visitor
    let mut visitor = code_visitor::CodeVisitor::new(&mut state);
    visitor.visit_file(&file);

    #[cfg(feature = "temp_target")]
    debug_relationships(&visitor);

    #[cfg(not(feature = "validate"))]
    log::trace!(target: "parse_target", "parsing target: {}
validate_unique_rels = {}", file_path.display(), &visitor.validate_unique_rels());
    #[cfg(feature = "validate")]
    log::trace!(target: "parse_target", "parsing target: {}
validate_unique_rels = <deferred>", file_path.display());
    // Release the mutable borrow on `state` held by `visitor` before any validation-time
    // graph normalization.
    drop(visitor);
    #[cfg(feature = "convert_keyword_2015")]
    if let Some(rewrite) = legacy_rewrite {
        traced_normalize_rewritten_graph(
            &mut state.code_graph,
            &rewrite.insertion_points,
            &file_path,
        );
    }
    #[cfg(feature = "validate")]
    {
        if !state.code_graph.validate_unique_rels() {
            let _ = emit_json_diagnostic(
                "analyze_file_phase2_validate_unique_rels_failed",
                &serde_json::json!({
                    "function": "analyze_file_phase2",
                    "file_path": file_path.display().to_string(),
                    "relation_count": state.code_graph.relations.len(),
                    "module_count": state.code_graph.modules.len(),
                }),
            );
        }
        assert!(state.code_graph.validate_unique_rels());
    }

    // let module_ids: Vec<NodeId> = state.code_graph.modules.iter().map(|m| m.id).collect();
    // for module_id in module_ids {
    //     if module_id != root_module_id {
    //         state.code_graph.relations.push(Relation {
    //             source: root_module_id,
    //             target: module_id,
    //             kind: crate::parser::relations::RelationKind::Contains,
    //         });
    //     }
    // }

    Ok(ParsedCodeGraph::new(
        file_path,
        crate_namespace,
        state.code_graph,
    ))
}

/// Analyze a single file for Phase 2 (UUID Path) - The Worker Function
/// Receives context from analyze_files_parallel.
#[cfg(not(feature = "cfg_eval"))]
pub fn analyze_file_phase2(
    file_path: PathBuf,
    crate_namespace: Uuid,            // Context passed from caller
    logical_module_path: Vec<String>, // NEW: The derived logical path for this file
) -> Result<ParsedCodeGraph, syn::Error> {
    // Consider a more specific Phase2Error later

    use super::nodes::ModuleKind;
    use attribute_processing::{
        extract_cfg_strings, // NEW: Import raw string extractor
        extract_file_level_attributes,
        extract_file_level_docstring,
        // Removed parse_and_combine_cfgs_from_attrs import
    };

    let file_content = std::fs::read_to_string(&file_path).map_err(|e| {
        syn::Error::new(
            proc_macro2::Span::call_site(),
            format!("Failed to read file {}: {}", file_path.display(), e),
        )
    })?;
    let file = syn::parse_file(&file_content)?;

    // 1. Create VisitorState with the provided context
    let mut state = state::VisitorState::new(crate_namespace, file_path.to_path_buf());
    // Set the correct initial module path for the visitor
    state.current_module_path = logical_module_path.clone();

    // Extract raw file-level CFG strings (#![cfg(...)])
    let file_cfgs = extract_cfg_strings(&file.attrs);
    // Set the initial scope CFGs for the visitor state
    state.current_scope_cfgs = file_cfgs.clone();
    // Hash the file-level CFG strings for the root module ID
    let root_cfg_bytes = calculate_cfg_hash_bytes(&file_cfgs);

    // 2. Generate root module ID using the derived logical path context AND CFG context
    let root_module_name = logical_module_path
        .last()
        .cloned()
        .unwrap_or_else(|| "crate".to_string()); // Use last segment as name, fallback to "crate"
    let root_module_parent_path: Vec<String> = logical_module_path
        .iter()
        .take(logical_module_path.len().saturating_sub(1)) // Get parent path segments
        .cloned()
        .collect();

    let root_module_node_id = NodeId::generate_synthetic(
        crate_namespace,
        &file_path,
        &root_module_parent_path, // Use parent path for ID generation context
        &root_module_name,
        ItemKind::Module,          // Pass correct ItemKind
        None,                      // Root module has no parent scope ID within the file context
        root_cfg_bytes.as_deref(), // Pass hashed file-level CFG bytes
    );
    // #[cfg(test)]
    debug_file_module_id_gen(
        crate_namespace,
        &file_path,
        &root_module_parent_path,
        &root_module_name,
        ItemKind::Module,
        None,
        root_cfg_bytes.as_deref(),
    );

    // 3. Create the root module node using the derived path and name
    // Determine visibility: Public only for crate root (main.rs/lib.rs), Inherited otherwise
    let root_visibility = if logical_module_path == ["crate"] {
        crate::parser::types::VisibilityKind::Public
    } else {
        crate::parser::types::VisibilityKind::Inherited
    };

    let root_module_info = ModuleNodeInfo {
        id: root_module_node_id,
        name: root_module_name,      // Use derived name
        visibility: root_visibility, // Use determined visibility
        attributes: Vec::new(),
        docstring: None,
        imports: Vec::new(),
        exports: Vec::new(),
        path: logical_module_path.clone(), // Use derived path
        span: (0, file_content.len()),
        tracking_hash: Some(state.generate_tracking_hash(&file.to_token_stream())),
        module_def: ModuleKind::FileBased {
            items: Vec::new(),
            file_path: file_path.clone(),
            file_attrs: extract_file_level_attributes(&file.attrs), // Non-CFG attributes
            file_docs: extract_file_level_docstring(&file.attrs),
            // cfgs removed from here, belongs on ModuleNode
        },
        cfgs: file_cfgs, // Store raw file-level CFGs on the ModuleNode
    };

    state
        .code_graph
        .modules
        .push(ModuleNode::new(root_module_info));

    let root_module_pid: PrimaryNodeId = state.code_graph.modules[0].id.into();

    // Default parent scope for top-level items visited next.
    state.current_primary_defn_scope.push(root_module_pid);

    // 4. Create and run the visitor
    let mut visitor = code_visitor::CodeVisitor::new(&mut state);
    visitor.visit_file(&file);

    #[cfg(feature = "temp_target")]
    debug_relationships(&visitor);

    #[cfg(feature = "validate")]
    {
        if !visitor.validate_unique_rels() {
            let _ = emit_json_diagnostic(
                "analyze_file_phase2_validate_unique_rels_failed",
                &serde_json::json!({
                    "function": "analyze_file_phase2",
                    "file_path": file_path.display().to_string(),
                    "relation_count": state.code_graph.relations.len(),
                    "module_count": state.code_graph.modules.len(),
                }),
            );
        }
        assert!(&visitor.validate_unique_rels());
    }

    // let module_ids: Vec<NodeId> = state.code_graph.modules.iter().map(|m| m.id).collect();
    // for module_id in module_ids {
    //     if module_id != root_module_id {
    //         state.code_graph.relations.push(Relation {
    //             source: root_module_id,
    //             target: module_id,
    //             kind: crate::parser::relations::RelationKind::Contains,
    //         });
    //     }
    // }

    Ok(ParsedCodeGraph::new(
        file_path,
        crate_namespace,
        state.code_graph,
    ))
}

// TODO: Figure out how to get the test cfg working correctly
// #[cfg(test)]
fn debug_file_module_id_gen(
    crate_namespace: uuid::Uuid,
    file_path: &std::path::Path,
    relative_path: &[String],
    item_name: &str,
    item_kind: ItemKind, // Use ItemKind from this crate
    parent_scope_id: Option<NodeId>,
    cfg_bytes: Option<&[u8]>,
) {
    use log::debug;

    use crate::utils::logging::LOG_TEST_ID_REGEN;

    if let Ok(debug_target_item) = std::env::var("ID_REGEN_TARGET") {
        if log::log_enabled!(target: LOG_TEST_ID_REGEN, log::Level::Debug)
            && debug_target_item == item_name
        // allow for filtering by command env variable
        {
            // Check if specific log is enabled
            debug!(target: LOG_TEST_ID_REGEN, "{:=^60}", " FileBased Id Generation ".log_header());
            debug!(target: LOG_TEST_ID_REGEN,
                "  Inputs for '{}' ({}):\n    crate_namespace: {}\n    file_path: {}\n    relative_path: {}\n    item_name: {}\n    item_kind: {}\n    parent_scope_id: {}\n    cfg_bytes: {}\n",
                item_name.log_name(), // item name being processed by visitor
                item_kind.log_comment_debug(),
                crate_namespace,
                file_path.as_os_str().log_comment_debug(),
                relative_path.log_path_debug(), // This is the 'relative_path' for the item's ID context
                item_name.log_name(),
                item_kind.log_comment_debug(),
                parent_scope_id.log_id_debug(), // The actual parent_scope_id used by visitor
                cfg_bytes.log_comment_debug() // The actual cfg_bytes used by visitor
            );
        }
    }
}

#[allow(dead_code, reason = "Useful for debugging")]
fn debug_relationships(visitor: &CodeVisitor<'_>) {
    let unique_rels = visitor.relations().iter().fold(Vec::new(), |mut acc, r| {
        if !acc.contains(r) {
            acc.push(*r)
        }
        acc
    });
    let has_duplicate = unique_rels.len() == visitor.relations().len();
    log::debug!(target: "temp",
        "{} {} {}: {} | {}: {} | {}: {}",
        "Relations are unique?".log_header(),
        if has_duplicate {
            "Yes!".log_spring_green().bold()
        } else {
            "NOOOO".log_error()
        },
        "Unique".log_step(),
        unique_rels.len().to_string().log_magenta_debug(),
        "Total".log_step(),
        visitor.relations().len().to_string().log_magenta_debug(),
        "Difference".log_step(),
        (visitor.relations().len() - unique_rels.len() ).to_string().log_magenta_debug(),
    );
    // Update HashMap key type to SyntacticRelation
    let rel_map: HashMap<SyntacticRelation, usize> =
        visitor
            .relations()
            .iter()
            .copied()
            .fold(HashMap::new(), |mut hmap, r| {
                match hmap.entry(r) {
                    std::collections::hash_map::Entry::Occupied(mut occupied_entry) => {
                        let existing_count = occupied_entry.get();
                        occupied_entry.insert(existing_count + 1);
                    }
                    std::collections::hash_map::Entry::Vacant(vacant_entry) => {
                        vacant_entry.insert(1);
                    }
                };
                hmap
            });
    for (rel, count) in rel_map {
        if count > 1 {
            // Use the helper methods to get base NodeIds for logging
            log::debug!(target: LOG_TARGET_RELS,
                "{} | {}: {} | {}", // Log the full relation variant for kind info
                "Duplicate!".log_header(),
                "Count".log_step(),
                count.to_string().log_error(),
                rel, // Log the whole enum variant
            );
        }
    }
}

/// Process multiple files in parallel using rayon
///
/// Takes DiscoveryOutput and distributes work to analyze_file_phase2.
#[instrument(skip_all, fields(crate_count = discovery_output.crate_contexts.len()))]
pub fn analyze_files_parallel(
    discovery_output: &DiscoveryOutput, // Takes output from Phase 1
    _num_workers: usize, // May not be directly used if relying on rayon's default pool size
) -> Vec<Result<ParsedCodeGraph, SynParserError>> {
    // Adjust error type if needed

    log::debug!(target: "crate_context",
        // Temporary debug print
        "Starting Phase 2 Parallel Parse for {} crates...",
        discovery_output.crate_contexts.len()
    );

    let parsed_results: Vec<Result<ParsedCodeGraph, SynParserError>> = discovery_output
        .crate_contexts
        .values()
        .par_bridge()
        .flat_map(|crate_context| {
            let parse_inputs = build_parse_inputs(crate_context);
            let selected_roots = selected_target_roots(crate_context);

            parse_inputs.into_par_iter().map(move |input| {
                // Call the single-file worker function with its specific context + logical path
                #[cfg(not(feature = "cfg_eval"))]
                let parsed = analyze_file_phase2(
                    input.file_path.clone(),
                    crate_context.namespace,
                    input.logical_path, // Pass the derived path

                )
                .map(|pg| set_root_context(crate_context, &selected_roots, pg))
                .map_err(|e| SynParserError::syn_parse_in_file(input.file_path.clone(), e))
                .inspect(|pg| { log::debug!(target: "crate_context", "{}", info_crate_context(crate_context, pg)) });

                log::debug!(target: "debug_dup", "file path in par_iter: {}", input.file_path.display());
                #[cfg(feature = "cfg_eval")]
                let parsed = analyze_file_phase2(
                    input.file_path.clone(),
                    crate_context.namespace,
                    input.logical_path, // Pass the derived path
                    crate_context
                )
                .map(|pg| set_root_context(crate_context, &selected_roots, pg))
                .map_err(|e| {
                    tracing::trace!("Error found: {} (file: {})", e, input.file_path.display());
                    SynParserError::syn_parse_in_file(input.file_path.clone(), e)
                })
                .inspect(|pg| { log::debug!(target: "crate_context", "{}", info_crate_context(crate_context, pg)) });
                parsed
            })
        })
        .collect(); // Collect all results (Result<ParsedCodeGraph, Error>) into a Vec

    let crate_count = parsed_results
        .iter()
        .filter_map(|pr| pr.as_ref().ok())
        .filter_map(|pr| pr.crate_context.as_ref())
        .inspect(|pr| {
            log::trace!(target: "crate_context", "root graph contains files: {:#?}", pr);
        })
        .count();
    if crate_count != 1 {
        log::trace!(target: "crate_context", "total crate count of graphs with crate_context: {}", crate_count);
    }
    // NOTE:2025-12-26
    // Commenting out the below so this function will not panic on finding a crate_context, as in
    // the case of an error in the syntax of the `lib.rs` for the target crate.
    // .find(|pr| pr.crate_context.is_some());
    // .expect("At least one crate must carry the context");
    // log::trace!(target: "crate_context", "root graph contains files: {:#?}", root_graph.crate_context);

    parsed_results
}

#[derive(Debug, Clone)]
struct ParseInput {
    file_path: PathBuf,
    logical_path: Vec<String>,
}

fn build_parse_inputs(crate_context: &crate::discovery::CrateContext) -> Vec<ParseInput> {
    let src_dir = crate_context.root_path.join("src");
    let src_mod_rs = src_dir.join("mod.rs");
    let mut parse_inputs: Vec<ParseInput> = Vec::new();
    let mut seen_files = std::collections::HashSet::<PathBuf>::new();
    let exclude_crate_root_relative_src_mod = crate_context.targets.iter().any(|target| {
        matches!(target.kind, TargetKind::Lib)
            && !target.root.starts_with(&src_dir)
            && target.root.file_name() == Some("lib.rs".as_ref())
    });

    let has_primary_target = crate_context
        .targets
        .iter()
        .any(|t| matches!(t.kind, TargetKind::Lib | TargetKind::Bin));
    let parse_targets: Vec<&TargetSpec> = if has_primary_target {
        crate_context
            .targets
            .iter()
            .filter(|t| matches!(t.kind, TargetKind::Lib | TargetKind::Bin))
            .collect()
    } else {
        crate_context.targets.iter().collect()
    };

    if has_primary_target {
        let skipped_targets = crate_context
            .targets
            .len()
            .saturating_sub(parse_targets.len());
        if skipped_targets > 0 {
            log::warn!(
                "Skipping {skipped_targets} non-primary targets for crate '{}' in legacy parse mode",
                crate_context.name
            );
        }
    }

    for target in parse_targets {
        match target.kind {
            TargetKind::Lib | TargetKind::Bin => {
                if seen_files.insert(target.root.clone()) {
                    parse_inputs.push(ParseInput {
                        file_path: target.root.clone(),
                        logical_path: logical_path_for_target_root(&src_dir, target),
                    });
                }
                for file_path in &crate_context.files {
                    if !file_path.starts_with(&src_dir) {
                        continue;
                    }
                    if exclude_crate_root_relative_src_mod && file_path == &src_mod_rs {
                        // Non-standard root lib crates can legitimately keep src/mod.rs as an
                        // implementation file via #[path], but parsing it as a separate root
                        // would collide with the actual lib root at logical path ["crate"].
                        // Restrict this exclusion to the crate-root-relative src/mod.rs only.
                        continue;
                    }
                    if !seen_files.insert(file_path.clone()) {
                        continue;
                    }
                    parse_inputs.push(ParseInput {
                        file_path: file_path.clone(),
                        logical_path: logical_module_path_for_file(&src_dir, file_path),
                    });
                }
            }
            TargetKind::Test | TargetKind::Example | TargetKind::Bench => {
                if seen_files.insert(target.root.clone()) {
                    parse_inputs.push(ParseInput {
                        file_path: target.root.clone(),
                        logical_path: vec![
                            "crate".to_string(),
                            target_kind_segment(&target.kind).to_string(),
                            target.name.clone(),
                        ],
                    });
                }
            }
        }
    }

    parse_inputs
}

fn logical_path_for_target_root(src_dir: &Path, target: &TargetSpec) -> Vec<String> {
    if target.root.starts_with(src_dir) {
        return logical_module_path_for_file(src_dir, &target.root);
    }
    if target
        .root
        .file_name()
        .is_some_and(|f| f == "lib.rs" || f == "main.rs")
    {
        return vec!["crate".to_string()];
    }
    vec![
        "crate".to_string(),
        target_kind_segment(&target.kind).to_string(),
        target.name.clone(),
    ]
}

fn selected_target_roots(
    crate_context: &crate::discovery::CrateContext,
) -> std::collections::HashSet<PathBuf> {
    let has_primary_target = crate_context
        .targets
        .iter()
        .any(|t| matches!(t.kind, TargetKind::Lib | TargetKind::Bin));

    crate_context
        .targets
        .iter()
        .filter(|t| {
            if has_primary_target {
                matches!(t.kind, TargetKind::Lib | TargetKind::Bin)
            } else {
                true
            }
        })
        .map(|t| t.root.clone())
        .collect()
}

fn target_kind_segment(kind: &TargetKind) -> &'static str {
    match kind {
        TargetKind::Lib => "lib",
        TargetKind::Bin => "bin",
        TargetKind::Test => "test",
        TargetKind::Example => "example",
        TargetKind::Bench => "bench",
    }
}

fn set_root_context(
    crate_context: &crate::discovery::CrateContext,
    selected_roots: &std::collections::HashSet<PathBuf>,
    mut pg: ParsedCodeGraph,
) -> ParsedCodeGraph {
    if selected_roots.contains(&pg.file_path) {
        pg.crate_context = Some(crate_context.clone());
    }
    pg
}

fn info_crate_context(
    crate_context: &crate::discovery::CrateContext,
    pg: &ParsedCodeGraph,
) -> String {
    let crate_root = &crate_context.root_path;
    format!(
        "parsed_graph file_path: {}, crate_context: {:#?}",
        pg.file_path
            .strip_prefix(crate_root)
            .as_ref()
            .log_path_debug(),
        pg.crate_context
    )
}

/// Calculates a hash for a list of raw CFG strings.
/// Sorts the strings before joining and hashing to ensure deterministic output.
/// Returns None if the input slice is empty.
pub fn calculate_cfg_hash_bytes(cfgs: &[String]) -> Option<Vec<u8>> {
    if cfgs.is_empty() {
        return None;
    }

    // Clone and sort for determinism
    let mut sorted_cfgs = cfgs.to_vec();
    sorted_cfgs.sort_unstable();

    // Join with a separator (important if a cfg string could contain the separator)
    let joined_cfgs = sorted_cfgs.join("::CFG::");

    // Hash the joined string
    let mut hasher = ByteHasher::default();
    hasher.write(joined_cfgs.as_bytes());
    Some(hasher.finish_bytes())
}
