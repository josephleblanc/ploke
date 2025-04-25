pub(crate) const LOG_TARGET_VIS: &str = "mod_tree_vis"; // Define log target for visibility checks
pub(crate) const LOG_TARGET_BUILD: &str = "mod_tree_build"; // Define log target for build checks
pub(crate) const LOG_TARGET_PATH_ATTR: &str = "mod_tree_path"; // Define log target for path attribute handling
pub(crate) const LOG_TARGET_PATH_CFGS: &str = "mod_tree_cfgs"; // Define log target for path attribute handling
pub(crate) const LOG_TARGET_BFS: &str = "mod_tree_bfs"; // Define log target for path attribute handling

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

use crate::resolve::module_tree::ModuleTree;
pub use colored::Colorize;

use colored::{Color, ColoredString};
use log::debug;
use ploke_core::NodeId;
use std::{
    fmt::Debug,
    path::{Path, PathBuf},
};

use crate::parser::{
    nodes::{GraphNode, ModuleDef, ModuleNode, ModuleNodeId},
    relations::Relation,
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
            g_node.id().to_string().log_id(),
            g_node.kind().log_vis_debug(),
            g_node.visibility().log_name_debug(),
        )
    }
    fn log_bfs_path(&self, id: NodeId, path: &[String], step: &str) {
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

    fn log_module_insert(&self, module: &ModuleNode, id: ModuleNodeId) {
        // Get the string representation of the module definition kind
        let def_kind_str = get_module_def_kind_str(module);
        debug!(target: LOG_TARGET_BUILD, "{} {} {} | {} | {}", // Added one more {} placeholder
            "Insert".log_header(),
            module.name.log_name(),
            format!("({})", id).log_id(),
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
    fn log_relation(&self, relation: Relation, note: Option<&str>) {
        debug!(target: LOG_TARGET_PATH_ATTR,
            "{} {}: {} → {} {}",
            "Relation".log_header(),
            format!("{:?}", relation.kind).log_name(),
            relation.source.to_string().log_id(),
            relation.target.to_string().log_id(),
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
        debug!(target: LOG_TARGET_PATH_ATTR, "{: <12} {: <20} {} | {}",
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
}

/// Helper struct to hold context for accessibility logging.
pub(crate) struct AccLogCtx<'a> {
    pub source_name: &'a str,
    pub target_name: &'a str,
    pub effective_vis: Option<&'a VisibilityKind>, // Store as Option<&VisibilityKind>
}

impl<'a> AccLogCtx<'a> {
    /// Creates a new context for logging accessibility checks.
    pub(crate) fn new(
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

/// Helper function to get a string representation of the ModuleDef kind.
pub(crate) fn get_module_def_kind_str(module: &ModuleNode) -> &'static str {
    match module.module_def {
        ModuleDef::FileBased { .. } => "File",
        ModuleDef::Inline { .. } => "Inline",
        ModuleDef::Declaration { .. } => "Decl",
    }
}

/// Helper struct to hold context for path attribute logging.
pub(crate) struct PathProcessingContext<'a> {
    pub module_id: ModuleNodeId,
    pub module_name: &'a str,
    pub attr_value: Option<&'a str>,
    pub resolved_path: Option<&'a PathBuf>,
}

/// Helper struct to hold context for path attribute logging.
pub(crate) struct CfgLogCtx<'a> {
    pub module_id: ModuleNodeId,
    pub module_name: &'a str,
    pub module_path: &'a [String], // Use slice for efficiency
    pub module_cfgs: &'a [String],
    // module_attrs: &'a [Attribute],
}

#[allow(dead_code, reason = "useful later for cfg-aware handling")]
impl<'a> CfgLogCtx<'a> {
    /// Creates a new context for logging path attribute processing.
    fn new(module_node: &'a ModuleNode) -> Self {
        Self {
            module_id: ModuleNodeId::new(module_node.id()),
            module_name: &module_node.name,
            module_path: &module_node.path,
            module_cfgs: module_node.cfgs(),
            // module_attrs: &module_node.attributes(),
        }
    }
}
