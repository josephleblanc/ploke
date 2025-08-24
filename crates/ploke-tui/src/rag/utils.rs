use super::*;

pub(crate) fn calc_top_k_for_budget(token_budget: u32) -> usize {
    let top_k = (token_budget / 200) as usize;
    top_k.clamp(5, 20)
}


#[derive(Debug, Deserialize)]
pub(super) struct ApplyCodeEditArgs {
    pub(super) edits: Vec<EditInput>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum Action {
    CodeEdit,
    // Create, // not supported yet
}

#[derive(Debug, Deserialize)]
pub(super) struct EditInput {
    pub(super) action: Action,
    /// File path relative to the project root (or absolute). Example: "example_crate/src/main.rs"
    pub(super) file: String,
    /// Canonical path of the target item without leading 'crate'. Example: "module_one::foo::Bar"
    pub(super) canon: String,
    /// Relation name for the node type. Example: "function", "struct", ...
    pub(super) node_type: String,
    /// Full rewritten item text (attributes/docs included if applicable)
    pub(super) code: String,
}

#[derive(Debug, Serialize)]
struct PerEditResult {
    file_path: String,
    ok: bool,
    error: Option<String>,
    new_file_hash: Option<String>,
}

#[derive(Debug, Serialize)]
pub(super) struct ApplyCodeEditResult {
    ok: bool,
    applied: usize,
    results: Vec<PerEditResult>,
}

pub(super) const ALLOWED_RELATIONS: &[&str] = &[
    "function",
    "const",
    "enum",
    "impl",
    "import",
    "macro",
    "module",
    "static",
    "struct",
    "trait",
    "type_alias",
    "union",
];

#[derive(Clone, Debug)]
pub struct ToolCallParams<'a> {
    pub state: &'a Arc<AppState>,
    pub event_bus: &'a Arc<EventBus>,
    pub request_id: Uuid,
    pub parent_id: Uuid,
    pub vendor: llm::ToolVendor,
    pub name: String,
    pub arguments: serde_json::Value,
    pub call_id: String,
}

impl<'a> ToolCallParams<'a> {
    pub(super) fn tool_call_failed(&self, error: String) {
        let _ = self
            .event_bus
            .realtime_tx
            .send(AppEvent::System(SystemEvent::ToolCallFailed {
                request_id: self.request_id,
                parent_id: self.parent_id,
                call_id: self.call_id.clone(),
                error,
            }));
    }
}
