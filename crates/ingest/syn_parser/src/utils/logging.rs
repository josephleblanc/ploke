pub const LOG_TARGET_VIS: &str = "mod_tree_vis"; // Define log target for visibility checks
pub const LOG_TARGET_BUILD: &str = "mod_tree_build"; // Define log target for build checks
pub const LOG_TARGET_PATH_ATTR: &str = "mod_tree_path"; // Define log target for path attribute handling
pub const LOG_TARGET_PATH_CFGS: &str = "mod_tree_cfgs"; // Define log target for path attribute handling
pub const LOG_TARGET_BFS: &str = "mod_tree_bfs"; // Define log target for path attribute handling
pub const LOG_TARGET_GRAPH_FIND: &str = "graph_find"; // Define log target for this file
pub const LOG_TARGET_MOD_TREE_BUILD: &str = "mod_tree_build"; // Define log target for tree build
pub const LOG_TARGET_NODE: &str = "node_info"; // Define log target for visibility checks
pub const LOG_TARGET_RELS: &str = "rels"; // Define log target for relation checks
pub const LOG_TARGET_NODE_ID: &str = "node_id";

// Color scheme constants (Tokyo Night inspired)
const COLOR_HEADER: Color = Color::TrueColor {
    r: 122,
    g: 162,
    b: 247,
}; // Soft blue
const COLOR_NAME: Color = Color::TrueColor {
    r: 255,
    g: 202,
    b: 158,
}; // Peach
const COLOR_ID: Color = Color::TrueColor {
    r: 187,
    g: 154,
    b: 247,
}; // Light purple
const COLOR_VIS: Color = Color::TrueColor {
    r: 158,
    g: 206,
    b: 255,
}; // Sky blue
const COLOR_PATH: Color = Color::TrueColor {
    r: 158,
    g: 206,
    b: 255,
}; // Sky blue
const COLOR_ERROR: Color = Color::TrueColor {
    r: 247,
    g: 118,
    b: 142,
}; // Soft red
const COLOR_GREEN: Color = Color::TrueColor {
    r: 158,
    g: 255,
    b: 158,
}; // Soft green
const COLOR_YELLOW: Color = Color::TrueColor {
    r: 255,
    g: 255,
    b: 158,
}; // Soft yellow
const COLOR_MAGENTA: Color = Color::TrueColor {
    r: 255,
    g: 158,
    b: 255,
}; // Soft magenta

// Additional Tokyo Night Colors
const COLOR_FOREGROUND_PRIMARY: Color = Color::TrueColor {
    r: 192,
    g: 202,
    b: 245,
}; // #c0caf5 - Default Text
const COLOR_FOREGROUND_SECONDARY: Color = Color::TrueColor {
    r: 169,
    g: 177,
    b: 214,
}; // #a9b1d6 - Lighter Text
const COLOR_COMMENT: Color = Color::TrueColor {
    r: 86,
    g: 95,
    b: 137,
}; // #565f89 - Comments
const COLOR_ORANGE: Color = Color::TrueColor {
    r: 255,
    g: 158,
    b: 100,
}; // #ff9e64 - Orange (Constants, numbers)
const COLOR_SPRING_GREEN: Color = Color::TrueColor {
    r: 115,
    g: 218,
    b: 202,
}; // #73daca - Teal/Spring Green (Types)

use crate::{
    parser::nodes::{AnyNodeId, ImportNode, ImportNodeId, NodePath},
    resolve::module_tree::{ModuleTree, ModuleTreeError},
};
pub use colored::Colorize;

use colored::{Color, ColoredString};
use log::debug;
use std::{
    fmt::Debug,
    path::{Path, PathBuf},
};

use crate::parser::{
    nodes::{GraphNode, ModuleKind, ModuleNode, ModuleNodeId},
    relations::SyntacticRelation,
    types::VisibilityKind,
};

// ... (keep color constants)

/// Only implement for string types                                                 
#[allow(warnings)]
pub trait LogStyle: AsRef<str> {
    fn log_header(&self) -> ColoredString {
        self.as_ref().color(COLOR_HEADER).bold()
    }
    fn log_name(&self) -> ColoredString {
        self.as_ref().color(COLOR_NAME)
    }
    fn log_id(&self) -> ColoredString {
        self.as_ref().color(COLOR_ID)
    }
    fn log_vis(&self) -> ColoredString {
        self.as_ref().color(COLOR_VIS)
    }
    fn log_path(&self) -> ColoredString {
        self.as_ref().color(COLOR_PATH)
    }
    fn log_error(&self) -> ColoredString {
        self.as_ref().color(COLOR_ERROR).bold()
    }
    fn log_green(&self) -> ColoredString {
        self.as_ref().color(COLOR_GREEN)
    }
    fn log_yellow(&self) -> ColoredString {
        self.as_ref().color(COLOR_YELLOW)
    }
    fn log_magenta(&self) -> ColoredString {
        self.as_ref().color(COLOR_MAGENTA)
    }
    fn log_foreground_primary(&self) -> ColoredString {
        self.as_ref().color(COLOR_FOREGROUND_PRIMARY)
    }
    fn log_foreground_secondary(&self) -> ColoredString {
        self.as_ref().color(COLOR_FOREGROUND_SECONDARY)
    }
    fn log_comment(&self) -> ColoredString {
        self.as_ref().color(COLOR_COMMENT)
    }
    fn log_orange(&self) -> ColoredString {
        self.as_ref().color(COLOR_ORANGE)
    }
    fn log_spring_green(&self) -> ColoredString {
        self.as_ref().color(COLOR_SPRING_GREEN)
    }
    fn log_step(&self) -> ColoredString {
        self.as_ref().color(COLOR_YELLOW).italic() // Using yellow italic for steps
    }
    fn debug_fmt(&self) -> ColoredString
    where
        Self: Debug,
    {
        format!("{:?}", self).normal()
    }
}
impl<T: AsRef<str> + ?Sized> LogStyle for T {}

#[allow(warnings)]
pub trait LogStyleDebug: Debug {
    fn log_header_debug(&self) -> ColoredString {
        format!("{:?}", self).color(COLOR_HEADER).bold()
    }
    fn log_name_debug(&self) -> ColoredString {
        format!("{:?}", self).color(COLOR_NAME)
    }
    fn log_id_debug(&self) -> ColoredString {
        format!("{:?}", self).color(COLOR_ID)
    }
    fn log_vis_debug(&self) -> ColoredString {
        format!("{:?}", self).color(COLOR_VIS)
    }
    fn log_path_debug(&self) -> ColoredString {
        format!("{:?}", self).color(COLOR_PATH)
    }
    fn log_error_debug(&self) -> ColoredString {
        format!("{:?}", self).color(COLOR_ERROR).bold()
    }
    fn log_green_debug(&self) -> ColoredString {
        format!("{:?}", self).color(COLOR_GREEN)
    }
    fn log_yellow_debug(&self) -> ColoredString {
        format!("{:?}", self).color(COLOR_YELLOW)
    }
    fn log_magenta_debug(&self) -> ColoredString {
        format!("{:?}", self).color(COLOR_MAGENTA)
    }
    fn log_foreground_primary_debug(&self) -> ColoredString {
        format!("{:?}", self).color(COLOR_FOREGROUND_PRIMARY)
    }
    fn log_foreground_secondary_debug(&self) -> ColoredString {
        format!("{:?}", self).color(COLOR_FOREGROUND_SECONDARY)
    }
    fn log_comment_debug(&self) -> ColoredString {
        format!("{:?}", self).color(COLOR_COMMENT)
    }
    fn log_orange_debug(&self) -> ColoredString {
        format!("{:?}", self).color(COLOR_ORANGE)
    }
    fn log_spring_green_debug(&self) -> ColoredString {
        format!("{:?}", self).color(COLOR_SPRING_GREEN)
    }
    fn log_step_debug(&self) -> ColoredString {
        format!("{:?}", self).color(COLOR_YELLOW).italic() // Using yellow italic for steps
    }
}

impl<T: Debug> LogStyleDebug for T {}

/// Trait for ergonomic logging of inner processes of data structures
pub trait LogDataStructure {
    /// Logs the details of an accessibility check using the provided context.
    fn log_access(
        &self,               // Keep &self if needed for other lookups, otherwise remove
        context: &AccLogCtx, // Pass context by reference
        step: &str,          // Description of the check step
        result: bool,
    ) {
        // Use debug! macro with the specific target
        debug!(target: LOG_TARGET_VIS,
            "{} {} -> {} | Target Vis: {} | Step: {} | Result: {}",
            "Accessibility Check:".bold(),
            context.source_name.yellow(), // Get name from context
            context.target_name.blue(),   // Get name from context
            context.effective_vis.map(|v| format!("{:?}", v).magenta()).unwrap_or_else(|| "NotFound".red().bold()), // Get visibility from context
            step.white().italic(),
            if result { "Accessible".green().bold() } else { "Inaccessible".red().bold() }
        );
    }

    fn log_bfs_step(&self, g_node: &dyn GraphNode, step: &str) {
        debug!( target: LOG_TARGET_BFS,
            "{} {: <14} {: <12} {: <20} | {: <12} | {: <15}",
            "BFS ".log_header(),
            step.white().italic(),
            g_node.name().log_name(),
            g_node.any_id().to_string().log_id(),
            g_node.kind().log_vis_debug(),
            g_node.visibility().log_name_debug(),
        )
    }
    fn log_bfs_path(&self, id: ModuleNodeId, path: &[String], step: &str) {
        debug!( target: LOG_TARGET_BFS,
            "{} {: <14} {: <12} {: <20} | {: <12}",
            "BFS ".log_header(),
            step.white().italic(),
            "",
            id.to_string().log_id(),
            path.log_vis_debug(),
        )
    }

    /// Logs the details of path cfg processing using the provided context.
    #[allow(dead_code, reason = "useful later for cfg-aware handling")]
    fn log_cfgs(&self, context: &CfgLogCtx, step: &str, result: Option<String>) {
        debug!(target: LOG_TARGET_PATH_CFGS,
            "{: <12} {: <20} {} | cfgs : {} | Attr: {} | Resolved: {} | {}",
            "Cfgs".log_header(),
            context.module_name.log_name(),
            format!("({})", context.module_id).log_id(),
            context.module_cfgs.join(",").log_path(),
            format!("{:?}", context.module_path).log_path(),
            step.log_name(),  // Using name color for step for visual distinction
            result.unwrap_or_else(|| "✓".to_string()).log_vis()  // Using vis color for result
        );
    }

    fn log_module_insert(&self, module: &ModuleNode) {
        // Get the string representation of the module definition kind
        let def_kind_str = get_module_def_kind_str(module);
        debug!(target: LOG_TARGET_BUILD, "{} {} {} | {} | {}", // Added one more {} placeholder
            "Insert".log_header(),
            module.name.log_name(),
            format!("({})", module.id).log_id(),
            def_kind_str.log_name(), // Log the kind using name style
            module.visibility.log_vis_debug()
        );
    }

    fn log_duplicate(&self, module: &ModuleNode) {
        debug!(target: LOG_TARGET_BUILD, "{} {} {}",
            "Duplicate ID".log_error(),
            module.name.log_name(),
            format!("({})", module.id).log_id()
        );
    }

    fn log_path_resolution(
        &self,
        module: &ModuleNode,
        path: &[String],
        status: &str,
        details: Option<&str>,
    ) {
        debug!(target: LOG_TARGET_PATH_ATTR,
            "{: <12} {: <20} {} | Path: {} | {} {}",
            "PathResolve".log_header(),
            module.name.log_name(),
            format!("({})", module.id).log_id(),
            format!("{:?}", path).log_path(),
            status.log_vis(),
            details.unwrap_or("").log_name()
        );
    }

    fn log_unlinked_module(&self, module: &ModuleNode, path: &[String]) {
        self.log_path_resolution(module, path, "Unlinked", Some("No declaration found"));
    }

    fn log_path_processing(&self, ctx: &PathProcessingContext, step: &str, result: Option<&str>) {
        debug!(target: LOG_TARGET_PATH_ATTR,
            "{} {} {} | Attr: {} | Resolved: {} | {} | {}",
            "PathAttr".log_header(),
            ctx.module_name.log_name(),
            format!("({})", ctx.module_id).log_id(),
            ctx.attr_value.unwrap_or("-").log_path(),
            ctx.resolved_path
                .map(|p| p.display().to_string())
                .unwrap_or_else(|| "-".to_string())
                .log_path(),
            step.log_name(),
            result.unwrap_or("✓").log_vis()
        );
    }
    fn log_relation(&self, relation: SyntacticRelation, note: Option<&str>) {
        debug!(target: LOG_TARGET_PATH_ATTR,
            "{} | {} | {}",
            "Relation".log_header(),
            relation,
            note.map(|n| format!("({})", n)).unwrap_or_default().log_vis()
        );
    }

    fn log_module_error(&self, module_id: ModuleNodeId, message: &str) {
        debug!(target: LOG_TARGET_PATH_ATTR,
            "{} {} {} | {}",
            "Error".log_error(),
            "module".log_name(),
            format!("({})", module_id).log_id(),
            message.log_vis()
        );
    }

    /// Logs the entry or exit point of the path attribute resolution process.
    fn log_resolve_entry_exit(&self, is_entry: bool) {
        let action = if is_entry { "Entering" } else { "Finished" };
        // Simplified format
        debug!(target: LOG_TARGET_PATH_ATTR, "{} {}",
            action.log_header(),
            "resolve_pending_path_attrs path attribute resolution.".log_name()
        );
    }

    /// Logs the status of pending path attributes found.
    fn log_resolve_pending_status(&self, count: Option<usize>) {
        let header = "Pending".log_header();
        match count {
            Some(n) => {
                // Simplified format
                debug!(target: LOG_TARGET_PATH_ATTR, "{} | Found {} pending path attribute IDs.",
                    header,
                    n.to_string().log_id()
                );
            }
            None => {
                // Simplified format
                debug!(target: LOG_TARGET_PATH_ATTR, "{} | No pending path attributes found (list was None or empty).",
                    header
                );
            }
        }
    }

    /// Logs a specific step during the resolution of a single module's path attribute.
    fn log_resolve_step(&self, module_id: ModuleNodeId, step: &str, outcome: &str, is_error: bool) {
        let status_indicator = if is_error {
            "Error".log_error()
        } else {
            "Step".log_header()
        };
        let outcome_styled = if is_error {
            outcome.log_error()
        } else {
            outcome.log_vis()
        };
        let id_str = format!("({})", module_id).log_id();
        let step_str = step.log_name();

        // Reordered format, removed padding
        log::trace!(target: LOG_TARGET_PATH_ATTR, "{: <12} {: <20} {} | {}",
            status_indicator,
            step_str,
            id_str,
            outcome_styled
        );
    }

    /// Logs the successful insertion of a resolved path attribute.
    fn log_resolve_insert(module_id: ModuleNodeId, resolved_path: &Path) {
        let header = "Insert".log_header();
        let id_str = format!("({})", module_id).log_id();
        let action_str = "Resolved Path".log_name();
        let path_str = resolved_path.display().to_string().log_path();

        // Reordered format, removed padding
        debug!(target: LOG_TARGET_PATH_ATTR, "{: <12} {: <20} {} | {}",
            header,
            action_str,
            id_str,
            path_str
        );
    }

    /// Logs the detection of a duplicate path attribute entry.
    fn log_resolve_duplicate(&self, module_id: ModuleNodeId, existing: &Path, conflicting: &Path) {
        let header = "Duplicate".log_error();
        let id_str = format!("({})", module_id).log_id();
        let existing_str = existing.display().to_string().log_path();
        let conflicting_str = conflicting.display().to_string().log_path();

        // Reordered format, removed padding
        debug!(target: LOG_TARGET_PATH_ATTR, "{} {} | Existing: {} | Conflicting: {}",
            header,
            id_str,
            existing_str,
            conflicting_str
        );
    }

    /// Logs when a module with a path attribute is identified and added to the pending list.
    fn log_add_pending_path(&self, module_id: ModuleNodeId, module_name: &str) {
        let header = "Pending Path".log_header();
        let name_str = module_name.log_name();
        let id_str = format!("({})", module_id).log_id();
        let detail_str = "Added to pending list".log_vis();

        // Reordered format, removed padding, similar to log_path_resolution
        debug!(target: LOG_TARGET_PATH_ATTR, "{: <12} {: <20} {} | {}",
            header,
            name_str,
            id_str,
            detail_str
        );
    }
    fn log_path_attr_not_found(module_id: ModuleNodeId) {
        log::error!(target: LOG_TARGET_BUILD, "Inconsistent ModuleTree: Parent not found for module {} processed with path attribute during file dir search.", module_id);
    }

    // --- Private Logging Helpers for resolve_path_relative_to ---

    fn log_resolve_segment_start(
        &self,
        segment: &str,
        search_in_module_id: ModuleNodeId,
        relation_count: usize,
    ) {
        debug!(target: LOG_TARGET_MOD_TREE_BUILD,
            "{} {} in module {} ({} relations found)",
            "Resolving segment:".log_header(),
            segment.log_name(),
            search_in_module_id.to_string().log_id(),
            relation_count.to_string().log_id()
        );
    }

    fn log_resolve_segment_relation(&self, target_id: AnyNodeId) {
        debug!(target: LOG_TARGET_MOD_TREE_BUILD,
            "  {} Relation Target ID: {}",
            "->".log_comment(),
            target_id.to_string().log_id()
        );
    }

    fn log_resolve_segment_found_node(
        &self,
        target_node: &dyn GraphNode,
        segment: &str,
        name_matches: bool,
    ) {
        debug!(target: LOG_TARGET_MOD_TREE_BUILD,
            "    {} Found Node: '{}' ({}), Name matches '{}': {}",
            "✓".log_green(),
            target_node.name().log_name(),
            target_node.kind().log_vis_debug(),
            segment.log_name(),
            name_matches.to_string().log_vis()
        );
    }

    // --- Logging Helpers for Shortest Public Path (SPP) ---

    fn log_spp_start(&self, item_node: &dyn GraphNode) {
        self.log_bfs_step(item_node, "Starting SPP");
    }

    fn log_spp_item_not_public(&self, item_node: &dyn GraphNode) {
        self.log_bfs_step(item_node, "Item not public, terminating early");
    }

    fn log_spp_check_root(&self, current_mod_id: ModuleNodeId, path_to_item: &[String]) {
        self.log_bfs_path(current_mod_id, path_to_item, "Check if root");
    }

    fn log_spp_found_root(&self, current_mod_id: ModuleNodeId, path_to_item: &[String]) {
        self.log_bfs_path(current_mod_id, path_to_item, "Found root!");
    }

    fn log_spp_explore_containment(&self, current_mod_id: ModuleNodeId, path_to_item: &[String]) {
        self.log_bfs_path(current_mod_id, path_to_item, "Explore Up (Containment)");
    }

    fn log_spp_explore_reexports(&self, current_mod_id: ModuleNodeId, path_to_item: &[String]) {
        self.log_bfs_path(current_mod_id, path_to_item, "Explore Up (Re-exports)");
    }

    // --- Logging Helpers for explore_up_via_containment ---

    fn log_spp_containment_start(&self, current_mod_node: &ModuleNode) {
        self.log_bfs_step(current_mod_node, "Start explore up");
    }

    fn log_spp_containment_vis_source(&self, current_mod_node: &ModuleNode) {
        self.log_bfs_step(current_mod_node, "Check Vis Source");
    }

    fn log_spp_containment_vis_source_decl(&self, decl_id: ModuleNodeId) {
        debug!(target: LOG_TARGET_VIS, "  {} Visibility source is Declaration: {}", "->".log_comment(), decl_id.to_string().log_id());
    }

    fn log_spp_containment_unlinked(&self, current_mod_id: ModuleNodeId) {
        log::warn!(target: LOG_TARGET_VIS, "SPP: Could not find declaration for file-based module {}, treating as inaccessible upwards.", current_mod_id);
    }

    fn log_spp_containment_vis_source_inline(&self, current_mod_node: &ModuleNode) {
        self.log_bfs_step(current_mod_node, "Inline/root, use self");
    }

    fn log_spp_containment_check_parent(&self, parent_mod_node: &ModuleNode) {
        self.log_bfs_step(parent_mod_node, "Checking Parent");
    }

    fn log_spp_containment_queue_parent(&self, parent_mod_id: ModuleNodeId, new_path: &[String]) {
        self.log_bfs_path(parent_mod_id, new_path, "Queueing Parent");
    }

    fn log_spp_containment_parent_visited(&self, parent_mod_id: ModuleNodeId) {
        debug!(target: LOG_TARGET_BFS, "  {} Parent {} already visited.", "->".log_comment(), parent_mod_id.to_string().log_id());
    }

    fn log_spp_containment_parent_inaccessible(
        &self,
        visibility_source_node: &ModuleNode,
        effective_source_id: ModuleNodeId,
        parent_mod_id: ModuleNodeId,
    ) {
        log::trace!(target: LOG_TARGET_VIS, "SPP: Module {} ({}) not accessible from parent {}, pruning containment path.", visibility_source_node.name().log_name(), effective_source_id.to_string().log_id(), parent_mod_id.to_string().log_id());
    }

    fn log_spp_containment_no_parent(&self, effective_source_id: ModuleNodeId) {
        log::warn!(target: LOG_TARGET_MOD_TREE_BUILD, "SPP: No parent found for non-root module {} via containment.", effective_source_id.to_string().log_id());
    }

    // --- Logging Helpers for explore_up_via_reexports ---

    fn log_spp_reexport_start(&self, target_id: ModuleNodeId, path_to_item: &[String]) {
        self.log_bfs_path(target_id, path_to_item, "Start Re-export Explore");
    }

    fn log_spp_reexport_missing_import_node(&self, import_node_id: ImportNodeId) {
        log::warn!(target: LOG_TARGET_MOD_TREE_BUILD, "SPP: ReExport relation points to non-existent ImportNode {}", import_node_id.to_string().log_id());
    }

    fn log_spp_reexport_is_external(&self, import_node: &ImportNode) {
        self.log_bfs_step(import_node, "Is External Crate");
    }

    fn log_spp_reexport_get_import_node(&self, import_node: &ImportNode) {
        self.log_bfs_step(import_node, "Get import node");
    }

    fn log_spp_reexport_not_public(&self, import_node: &ImportNode) {
        self.log_bfs_step(import_node, "!is_public_use");
    }

    fn log_spp_reexport_queue_module(
        &self,
        import_node: &ImportNode,
        reexporting_mod_id: ModuleNodeId,
        new_path: &[String],
    ) {
        self.log_bfs_step(import_node, "Queueing Re-exporting Module");
        self.log_bfs_path(reexporting_mod_id, new_path, "  New Path");
    }

    fn log_spp_reexport_module_visited(&self, reexporting_mod_id: ModuleNodeId) {
        debug!(target: LOG_TARGET_BFS, "  {} Re-exporting module {} already visited.", "->".log_comment(), reexporting_mod_id.to_string().log_id());
    }

    fn log_spp_reexport_no_container(&self, import_node_id: ImportNodeId) {
        log::warn!(target: LOG_TARGET_MOD_TREE_BUILD, "SPP: No containing module found for ImportNode {}", import_node_id.to_string().log_id());
    }

    // --- Logging Helpers for is_accessible ---

    fn log_access_missing_decl_node(&self, decl_id: ModuleNodeId, target_defn_id: ModuleNodeId) {
        log::warn!(target: LOG_TARGET_VIS, "Declaration node {} not found for definition {}", decl_id.to_string().log_id(), target_defn_id.to_string().log_id());
    }

    // --- Logging Helpers for find_declaring_file_dir ---

    fn log_find_decl_dir_missing_parent(&self, current_id: ModuleNodeId) {
        log::error!(target: LOG_TARGET_MOD_TREE_BUILD, "Inconsistent ModuleTree: Parent not found for module {} during file dir search.", current_id.to_string().log_id());
    }

    // --- Logging Helpers for process_path_attributes ---

    fn log_path_attr_external_not_found(
        &self,
        decl_module_id: ModuleNodeId,
        resolved_path: &PathBuf,
    ) {
        log::warn!(
            target: LOG_TARGET_PATH_ATTR,
            "{} {} | {}",
            "External Path".yellow().bold(), // Use yellow for warning
            format!("({})", decl_module_id).log_id(),
            format!(
                "Target file outside src dir not found: {}",
                resolved_path.display()
            )
            .log_vis()
        );
    }

    // --- Logging Helpers for resolve_single_export ---

    fn log_resolve_single_export_external(&self, segments_to_resolve: &[String]) {
        log::debug!(target: LOG_TARGET_MOD_TREE_BUILD, "Detected external re-export based on first segment: {:?}", segments_to_resolve.log_path_debug());
    }

    /// Wraps a resolution error from `resolve_path_relative_to` into `UnresolvedReExportTarget`.
    fn wrap_resolution_error(
        &self,
        error: ModuleTreeError,
        export_node_id: AnyNodeId,
        original_path_segments: &[String],
    ) -> ModuleTreeError {
        match error {
            // Preserve existing UnresolvedReExportTarget if it came from the helper
            ModuleTreeError::UnresolvedReExportTarget { .. } => error,
            // Otherwise, create a new UnresolvedReExportTarget with the correct path
            _ => ModuleTreeError::UnresolvedReExportTarget {
                import_node_id: Some(export_node_id),
                // Use the original full path for the error message
                path: NodePath::try_from(original_path_segments.to_vec()).unwrap_or_else(|_| {
                    NodePath::new_unchecked(vec!["<invalid path conversion>".to_string()])
                }), // Handle potential error in path conversion for error reporting
            },
        }
    }

    // --- Logging Helpers for update_path_index_for_custom_paths ---

    fn log_update_path_index_entry_exit(&self, is_entry: bool) {
        let action = if is_entry { "Entering" } else { "Finished" };
        debug!(target: LOG_TARGET_MOD_TREE_BUILD, "{} {}",
            action.log_header(),
            "update_path_index_for_custom_paths.".log_name()
        );
    }

    fn log_update_path_index_status(&self, count: Option<usize>) {
        match count {
            Some(n) => {
                debug!(target: LOG_TARGET_MOD_TREE_BUILD, "{} Found {} modules with path attributes to process for index update.",
                    "Update Path Index:".log_header(),
                    n.to_string().log_id()
                );
            }
            None => {
                debug!(target: LOG_TARGET_MOD_TREE_BUILD, "{} No path attributes found, skipping index update.",
                    "Update Path Index:".log_header()
                );
            }
        }
    }

    fn log_update_path_index_remove(&self, original_path: &NodePath, def_mod_id: ModuleNodeId) {
        debug!(target: LOG_TARGET_MOD_TREE_BUILD, "  {} Removed old path index entry: {} -> {}",
            "✓".log_green(),
            original_path.to_string().log_path(),
            def_mod_id.to_string().log_id()
        );
    }

    fn log_update_path_index_remove_inconsistency(
        &self,
        removed_id: ModuleNodeId,
        original_path: &NodePath,
        expected_def_mod_id: ModuleNodeId,
    ) {
        log::error!(target: LOG_TARGET_MOD_TREE_BUILD, "{} Path index inconsistency: Removed ID {} for original path {} but expected definition ID {}. This indicates a major inconsistency if the removed ID doesn't match",
            "Error:".log_error(),
            removed_id.to_string().log_id(),
            original_path.to_string().log_path(),
            expected_def_mod_id.to_string().log_id()
        );
    }

    fn log_update_path_index_remove_missing(
        &self,
        original_path: &NodePath,
        def_mod_id: ModuleNodeId,
    ) {
        log::warn!(target: LOG_TARGET_MOD_TREE_BUILD, "  {} Original path {} not found in path_index for removal (Def Mod ID: {}). This might indicate an earlier indexing issue.",
            "Warning:".log_yellow(),
            original_path.to_string().log_path(),
            def_mod_id.to_string().log_id()
        );
    }

    fn log_update_path_index_insert(&self, canonical_path: &NodePath, def_mod_id: ModuleNodeId) {
        debug!(target: LOG_TARGET_MOD_TREE_BUILD, "  {} Inserted new path index entry: {} -> {}",
            "✓".log_green(),
            canonical_path.to_string().log_path(),
            def_mod_id.to_string().log_id()
        );
    }

    fn log_update_path_index_reinsert(&self, canonical_path: &NodePath, def_mod_id: ModuleNodeId) {
        debug!(target: LOG_TARGET_MOD_TREE_BUILD, "  {} Re-inserted path index entry (idempotent): {} -> {}",
            "Info:".log_comment(),
            canonical_path.to_string().log_path(),
            def_mod_id.to_string().log_id()
        );
    }

    fn log_update_path_index_insert_conflict(
        &self,
        canonical_path: &NodePath,
        def_mod_id: ModuleNodeId,
        existing_id: ModuleNodeId,
    ) {
        log::error!(target: LOG_TARGET_MOD_TREE_BUILD, "{} Path index conflict: Tried to insert canonical path {} -> {} but path already mapped to {}. {}",
            "Error:".log_error(),
            canonical_path.to_string().log_path(),
            def_mod_id.to_string().log_id(),
            existing_id.to_string().log_id(),
            "This implies a non-unique canonical path was generated or indexed incorrectly.".log_comment()
        );
    }
}

// --- New Trait for Error Logging ---

use crate::parser::nodes::AnyNodeIdConversionError;
use crate::parser::visitor::VisitorState; // Import VisitorState
use ploke_core::ItemKind; // Import ItemKind

/// Trait for logging specific conversion errors within the VisitorState.
pub trait LogErrorConversion {
    /// Logs an error when AnyNodeId fails to convert to GenericParamNodeId.
    fn log_generic_param_id_conversion_error(
        &self,
        generic_param_name: &str,
        item_kind: ItemKind, // Add item_kind for context
        error: AnyNodeIdConversionError,
    );

    /// Logs an error when AnyNodeId fails to convert to ImportNodeId.
    fn log_import_id_conversion_error(
        &self,
        import_name: &str,      // Name or placeholder like "<glob>" or "*"
        import_path: &[String], // The path leading to the import
        error: AnyNodeIdConversionError,
    );

    /// Logs an error when AnyNodeId fails to convert to FunctionNodeId.
    fn log_function_id_conversion_error(
        &self,
        function_name: &str,
        error: AnyNodeIdConversionError,
    );

    /// Logs an error when AnyNodeId fails to convert to MacroNodeId.
    fn log_macro_id_conversion_error(&self, macro_name: &str, error: AnyNodeIdConversionError);
}

impl LogErrorConversion for VisitorState {
    fn log_generic_param_id_conversion_error(
        &self,
        generic_param_name: &str,
        item_kind: ItemKind,
        _error: AnyNodeIdConversionError, // Error itself doesn't carry much info yet
    ) {
        log::error!(target: LOG_TARGET_NODE_ID,
            "{} Failed to convert {} to {} for generic parameter '{}' ({:?}) in file '{}' at module path '[{}]'. This indicates an internal inconsistency.",
            "ID Conversion Error:".log_error(),
            "AnyNodeId".log_id(),
            "GenericParamNodeId".log_id(),
            generic_param_name.log_name(),
            item_kind.log_vis_debug(),
            self.current_file_path.display().to_string().log_path(),
            self.current_module_path.join("::").log_path()
        );
        // Consider adding more context like parent_scope_id if helpful
    }

    fn log_import_id_conversion_error(
        &self,
        import_name: &str,
        import_path: &[String],
        _error: AnyNodeIdConversionError, // Error itself doesn't carry much info yet
    ) {
        log::error!(target: LOG_TARGET_NODE_ID,
            "{} Failed to convert {} to {} for import '{}' (path: [{}]) in file '{}' at module path '[{}]'. This indicates an internal inconsistency.",
            "ID Conversion Error:".log_error(),
            "AnyNodeId".log_id(),
            "ImportNodeId".log_id(),
            import_name.log_name(),
            import_path.join("::").log_path(),
            self.current_file_path.display().to_string().log_path(),
            self.current_module_path.join("::").log_path()
        );
    }

    fn log_function_id_conversion_error(
        &self,
        function_name: &str,
        _error: AnyNodeIdConversionError,
    ) {
        log::error!(target: LOG_TARGET_NODE_ID,
            "{} Failed to convert {} to {} for function '{}' in file '{}' at module path '[{}]'. This indicates an internal inconsistency.",
            "ID Conversion Error:".log_error(),
            "AnyNodeId".log_id(),
            "FunctionNodeId".log_id(),
            function_name.log_name(),
            self.current_file_path.display().to_string().log_path(),
            self.current_module_path.join("::").log_path()
        );
    }

    fn log_macro_id_conversion_error(&self, macro_name: &str, _error: AnyNodeIdConversionError) {
        log::error!(target: LOG_TARGET_NODE_ID,
            "{} Failed to convert {} to {} for macro '{}' in file '{}' at module path '[{}]'. This indicates an internal inconsistency.",
            "ID Conversion Error:".log_error(),
            "AnyNodeId".log_id(),
            "MacroNodeId".log_id(),
            macro_name.log_name(),
            self.current_file_path.display().to_string().log_path(),
            self.current_module_path.join("::").log_path()
        );
    }
}

/// Helper struct to hold context for accessibility logging.
pub struct AccLogCtx<'a> {
    pub source_name: &'a str,
    pub target_name: &'a str,
    pub effective_vis: Option<&'a VisibilityKind>, // Store as Option<&VisibilityKind>
}

impl<'a> AccLogCtx<'a> {
    /// Creates a new context for logging accessibility checks.
    pub fn new(
        source_id: ModuleNodeId,                   // Keep ID args for name lookup
        target_id: ModuleNodeId,                   // Keep ID args for name lookup
        effective_vis: Option<&'a VisibilityKind>, // Accept Option<&VisibilityKind>
        tree: &'a ModuleTree,                      // Need tree to look up names
    ) -> Self {
        let source_name = tree
            .modules()
            .get(&source_id)
            .map(|m| m.name.as_str())
            .unwrap_or("?");
        let target_name = tree
            .modules()
            .get(&target_id)
            .map(|m| m.name.as_str())
            .unwrap_or("?");
        Self {
            // source_id, // Removed unused field
            // target_id, // Removed unused field
            source_name,
            target_name,
            effective_vis,
        }
    }
}

/// Helper function to get a string representation of the ModuleKind kind.
pub fn get_module_def_kind_str(module: &ModuleNode) -> &'static str {
    match module.module_def {
        ModuleKind::FileBased { .. } => "File",
        ModuleKind::Inline { .. } => "Inline",
        ModuleKind::Declaration { .. } => "Decl",
    }
}

/// Helper struct to hold context for path attribute logging.
pub struct PathProcessingContext<'a> {
    pub module_id: ModuleNodeId,
    pub module_name: &'a str,
    pub attr_value: Option<&'a str>,
    pub resolved_path: Option<&'a PathBuf>,
}

/// Helper struct to hold context for path attribute logging.
pub struct CfgLogCtx<'a> {
    pub module_id: ModuleNodeId,
    pub module_name: &'a str,
    pub module_path: &'a [String], // Use slice for efficiency
    pub module_cfgs: &'a [String], // Changed to owned Vec<String>
                                   // module_attrs: &'a [Attribute],
}

#[allow(dead_code, reason = "useful later for cfg-aware handling")]
impl<'a> CfgLogCtx<'a> {
    /// Creates a new context for logging path attribute processing.
    fn new(module_node: &'a ModuleNode) -> Self {
        Self {
            module_id: module_node.id, // Use the ID from the passed ModuleNode
            module_name: &module_node.name,
            module_path: &module_node.path,
            module_cfgs: module_node.cfgs(), // Assign the owned Vec<String> directly
                                             // module_attrs: &module_node.attributes(),
        }
    }
}

pub trait LogStyleBool {
    fn log_bool(&self) -> ColoredString;
}

impl LogStyleBool for bool {
    fn log_bool(&self) -> ColoredString {
        if *self {
            self.to_string().log_green()
        } else {
            self.to_string().log_error()
        }
    }
}
