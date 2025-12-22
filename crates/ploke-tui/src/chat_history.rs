use crate::app_state::ListNavigation;
use crate::llm::LLMMetadata;
use crate::rag::context::PROMPT_HEADER;
use crate::{AppEvent, llm::manager::Role};

use fxhash::FxHashMap as HashMap;
use once_cell::sync::Lazy;
use std::fmt;
use std::hash::RandomState;
use std::io::Write as _;

use color_eyre::Result;
use ploke_core::ArcStr;

#[derive(Debug, Clone, Copy)]
pub enum NavigationDirection {
    Next,
    Previous,
}

#[derive(Debug, Clone, Copy)]
pub enum ChatError {
    ParentNotFound(Uuid),
    SiblingNotFound(Uuid),
    RootHasNoSiblings,
}

impl fmt::Display for ChatError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ChatError::ParentNotFound(id) => write!(f, "Parent message not found: {}", id),
            ChatError::SiblingNotFound(id) => write!(f, "Sibling message not found: {}", id),
            ChatError::RootHasNoSiblings => write!(f, "Root messages cannot have siblings"),
        }
    }
}

impl std::error::Error for ChatError {}

impl fmt::Display for MessageStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MessageStatus::Pending => write!(f, "Pending"),
            MessageStatus::Generating => write!(f, "Generating"),
            MessageStatus::Completed => write!(f, "Completed"),
            MessageStatus::Error { .. } => write!(f, "Error"),
        }
    }
}

use ratatui::widgets::ScrollbarState;
use serde::{Deserialize, Serialize};
use tempfile::NamedTempFile;
use thiserror::Error;
use uuid::Uuid;

/// Represents the possible states of a message during its lifecycle.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MessageStatus {
    /// The message is waiting to be processed by the LLM.
    Pending,
    /// The LLM is actively generating a response for this message.
    Generating,
    /// The message is complete and the response was successful.
    Completed,
    /// An error occurred during generation.
    Error { description: String },
}

/// Validation errors for message updates
#[derive(Debug, Error, PartialEq, Eq, Clone)]
pub enum UpdateError {
    #[error("Cannot append content when replacing entire message")]
    ContentConflict,
    #[error("Cannot update completed message")]
    ImmutableMessage,
    #[error("Invalid status transition: {0} -> {1}")]
    InvalidStatusTransition(MessageStatus, MessageStatus),
    #[error("Completed message cannot have empty content")]
    EmptyContentCompletion,
    #[error("Under development, add proper error handling")]
    Placeholder,
}

/// A structure containing optional fields to update on an existing Message.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MessageUpdate {
    /// Replaces the entire content of the message (mutually exclusive with append)
    pub content: Option<String>,

    /// Appends content to the existing message (mutually exclusive with content replacement)
    pub append_content: Option<String>,

    /// Changes the status of the message
    pub status: Option<MessageStatus>,

    /// Attaches or updates LLM execution metadata
    pub metadata: Option<LLMMetadata>,
}

impl MessageUpdate {
    /// Validates the update against current message state
    // TODO: Add a path for MessageStatus::Generating => MessageStatus::Completed
    pub fn validate(&self, current_status: &MessageStatus) -> Result<(), UpdateError> {
        // Completed messages are immutable
        if matches!(current_status, MessageStatus::Completed)
            && (self.content.is_some()
                || self.append_content.is_some()
                || self.status.is_some()
                || self.metadata.is_some())
        {
            return Err(UpdateError::ImmutableMessage);
        }

        // TODO: Consider whether this is a good idea or not - I like the idea of having some kind
        // of message or spinner while we are waiting for the response, and don't want to have this
        // be locked into an error state like the below commented out code.
        // Check for content conflict
        // if self.content.is_some() && self.append_content.is_some() {
        //     return Err(UpdateError::ContentConflict);
        // }

        // Validate status transitions
        if let Some(new_status) = &self.status {
            match (current_status, new_status) {
                // Completed messages are terminal (handled above)
                (MessageStatus::Generating, MessageStatus::Completed) => Ok(()),
                (MessageStatus::Generating, MessageStatus::Error { .. }) => Ok(()),
                (MessageStatus::Pending, MessageStatus::Error { .. }) => Ok(()),
                // Can only complete generating messages
                (_, MessageStatus::Completed)
                    if !matches!(current_status, MessageStatus::Generating) =>
                {
                    Err(UpdateError::InvalidStatusTransition(
                        current_status.clone(),
                        new_status.clone(),
                    ))
                }

                // Can only retry from error state
                (MessageStatus::Error { .. }, MessageStatus::Pending) => {
                    Err(UpdateError::Placeholder)
                }

                // Invalid transitions
                (from, to) if from != to => Err(UpdateError::InvalidStatusTransition(
                    from.clone(),
                    to.clone(),
                )),

                _ => Ok(()),
            }?;
        }

        Ok(())
    }
}

/// Event fired when a MessageUpdate command fails validation.
///
/// Contains the ID of the message that was targeted and the specific
/// validation error that occurred. This is crucial for providing
/// targeted feedback to the user and for telemetry.
#[derive(Debug, Clone)]
pub struct UpdateFailedEvent {
    pub message_id: Uuid,
    pub error: UpdateError, // The structured error from your previous code
}
impl From<UpdateFailedEvent> for AppEvent {
    fn from(event: UpdateFailedEvent) -> Self {
        AppEvent::UpdateFailed(event)
    }
}

impl UpdateFailedEvent {
    pub fn new(message_id: Uuid, error: UpdateError) -> Self {
        Self { message_id, error }
    }
}

/// Represents an individual message in the branching conversation tree.
///
/// Each message forms a node in the hierarchical chat history with:
/// - Links to its parent message (if any)
/// - List of child messages forming conversation branches
/// - Unique identifier and content storage
#[derive(Debug, Clone)]
pub struct Message {
    /// Unique identifier for the message
    pub id: Uuid,
    pub status: MessageStatus,
    // TODO: Maybe change Message to be LLM/human, or create a wrapper to differentiate.
    /// Metadata on LLM message
    pub metadata: Option<LLMMetadata>,
    /// Parent message UUID (None for root messages)
    pub parent: Option<Uuid>,
    /// Child message UUIDs forming conversation branches
    pub children: Vec<Uuid>,
    /// Selected Child is the default selection for the next navigation down
    /// Useful for `move_selection_down`
    pub selected_child: Option<Uuid>,
    /// Text content of the message
    pub content: String,
    /// The kind of the message's speaker, e.g. User, Assistant, System, etc
    pub kind: MessageKind,
    /// If this is a Tool message that came from a tool call, the originating call id.
    /// Optional to preserve backward compatibility and allow SysInfo-style tool logs.
    pub tool_call_id: Option<ArcStr>,
    /// Optional structured payload for rendering tool messages in the UI.
    pub tool_payload: Option<crate::tools::ToolUiPayload>,
    /// The status of the message in the current LLM context window.
    pub context_status: ContextStatus,
}

/// Running aggregates for the conversation.
#[derive(Debug, Default, Clone)]
pub struct ConversationTotals {
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
    pub total_tokens: u64,
    pub cost: f64,
}

/// Tracks how a context token count was derived.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenKind {
    Estimated,
    Actual,
}

/// Represents the most recent prompt/context token count.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ContextTokens {
    pub count: usize,
    pub kind: TokenKind,
}

impl ContextTokens {
    pub const fn new(count: usize, kind: TokenKind) -> Self {
        Self { count, kind }
    }
}

/// The status of the message in the current LLM context window.
/// Pinned indicates that the message should be kept in the messages sent to the LLM, while
/// Unpinned messages are left out of messages sent to the LLM.
/// Pinned messages must have a reason for the pin, which will be evaluated when they run out of
/// `turns_to_live`.
/// A "turn to live" is decremented each time the conversation history is retrieved.
#[derive(Debug, Clone, PartialEq, PartialOrd)]
pub enum ContextStatus {
    /// The information on why an item is pinned, and how long until it will either be automatically
    /// be removed from the context or reviewed and potentially re-pinned.
    Pinned {
        /// Number of LLM calls this message will live, defaults to 15
        turns_to_live: TurnsToLive,
        /// Optional reason this item is pinned
        reason: Option<ArcStr>,
        /// Optional field to indicate this message was pinned by a particular role.
        pinned_by: Option<Role>,
    },
    /// Unpinned message not kept in LLM context window.
    Unpinned,
}

impl Default for ContextStatus {
    fn default() -> Self {
        Self::Pinned {
            // sane default, somewhat arbitrary
            turns_to_live: TurnsToLive::default(),
            reason: None,
            pinned_by: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, PartialOrd)]
pub enum TurnsToLive {
    Limited(u16),
    NoneRemaining,
    Unlimited,
}

impl Default for TurnsToLive {
    fn default() -> Self {
        // sane default, somewhat arbitrary
        Self::Limited(15)
    }
}

/// Defines the author of a message.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MessageKind {
    /// A message from the end-user.
    User,
    /// A message generated by the language model.
    Assistant,
    /// A system-level message providing context or instructions (often hidden).
    System,
    /// A message generated by the tui system and shown to the user.
    SysInfo,
    /// A message generated by a tool or agent.
    Tool,
}

impl std::fmt::Display for MessageKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MessageKind::User => write!(f, "user"),
            MessageKind::Assistant => write!(f, "assistant"),
            MessageKind::System => write!(f, "system"),
            MessageKind::Tool => write!(f, "tool"),
            MessageKind::SysInfo => write!(f, "sysInfo"),
        }
    }
}

impl From<MessageKind> for &'static str {
    fn from(val: MessageKind) -> Self {
        match val {
            MessageKind::User => "user",
            MessageKind::Assistant => "assistant",
            MessageKind::System => "system",
            MessageKind::Tool => "tool",
            MessageKind::SysInfo => "sysInfo",
        }
    }
}

impl Message {
    /// Attempts to apply an update with validation
    pub fn try_update(&mut self, update: MessageUpdate) -> Result<(), UpdateError> {
        // Validate before applying
        update.validate(&self.status)?;

        // Apply updates
        if let Some(content) = update.content {
            self.content = content;
        }

        if let Some(chunk) = update.append_content {
            self.content.push_str(&chunk);
        }

        if let Some(status) = update.status {
            self.status = status;
        }

        if let Some(metadata) = update.metadata {
            self.merge_metadata(metadata);
        }

        // Post-update consistency check
        self.enforce_consistency()
    }

    /// Merges new metadata with existing
    fn merge_metadata(&mut self, new_metadata: LLMMetadata) {
        if let Some(existing) = &mut self.metadata {
            // Implementation depends on your metadata structure
            existing.usage.prompt_tokens += new_metadata.usage.prompt_tokens;
            existing.usage.completion_tokens += new_metadata.usage.completion_tokens;
            existing.cost += new_metadata.cost;
            // ... other fields
        } else {
            self.metadata = Some(new_metadata);
        }
    }

    /// Enforces business rules after update
    fn enforce_consistency(&mut self) -> Result<(), UpdateError> {
        // Completed messages must have content
        if matches!(self.status, MessageStatus::Completed) && self.content.is_empty() {
            self.status = MessageStatus::Error {
                description: "Empty content on completion".into(),
            };
            return Err(UpdateError::EmptyContentCompletion);
        }

        Ok(())
    }
}

pub(crate) static BASE_SYSTEM_PROMPT: Lazy<Message> = Lazy::new(|| {
    let context_status = ContextStatus::Pinned {
        turns_to_live: TurnsToLive::Unlimited,
        reason: Some(ArcStr::from(
            "Base system prompt should be pinned for entire conversation.",
        )),
        pinned_by: Some(Role::System),
    };
    Message {
        id: Uuid::new_v4(),
        status: crate::chat_history::MessageStatus::Completed,
        metadata: None,
        parent: None,
        children: Vec::new(),
        selected_child: None,
        content: PROMPT_HEADER.to_string(),
        kind: MessageKind::System,
        tool_call_id: None,
        tool_payload: None,
        context_status,
    }
});

/// Manages the complete branching conversation history using a tree structure.
///
/// Stores all messages in a HashMap for efficient lookup and maintains
/// the current position in the conversation tree.
///
/// Description:
/// The messages are parallel tracks that can be navigated with the `leftarrow`, `rightarrow`
/// keys across conversation tracks. New message tracks are created by the user whenever they
/// would like by selecting a previous message (navigating up/down the conversation track with
/// `uparrow` and `downarrow`).
/// New messages tracks are also created when multiple responses are desired to a user input.
// TODO: Needs updating for concurrency (DashMap? Something else?)
#[derive(Debug, Default)]
pub struct ChatHistory {
    /// All messages in the conversation history, indexed by UUID
    pub messages: HashMap<Uuid, Message>,
    /// UUID of the currently active message in the conversation flow
    pub current: Uuid,
    /// Final message in the currently selected message list.
    pub tail: Uuid,
    /// Cached path from root to tail for fast, zero-alloc iteration during render
    path_cache: Vec<Uuid>,
    /// Scroll bar support
    pub scroll_bar: Option<ScrollbarState>,
    /// Approximate running totals for the conversation.
    pub totals: ConversationTotals,
    /// Latest counted tokens for the currently constructed prompt/context.
    pub current_context_tokens: Option<ContextTokens>,
}

impl ChatHistory {
    /// Returns the conversation path (root → current) mapped to llm RequestMessage.
    ///
    /// Rules:
    /// - Includes User, Assistant, and System messages.
    /// - Skips System messages with empty content (root sentinel).
    /// - Skips SysInfo (UI/diagnostic) messages.
    /// - Skips Tool messages for now (requires tool_call_id which is not tracked here).
    pub(crate) fn current_path_as_llm_request_messages(
        &self,
    ) -> Vec<crate::llm::manager::RequestMessage> {
        use crate::llm::manager::RequestMessage as ReqMsg;

        self.current_path_ids()
            .filter_map(|id| self.messages.get(&id))
            .filter_map(|m| match m.kind {
                MessageKind::User => Some(ReqMsg::new_user(m.content.clone())),
                MessageKind::Assistant => Some(ReqMsg::new_assistant(m.content.clone())),
                MessageKind::System => {
                    if m.content.trim().is_empty() {
                        None
                    } else {
                        Some(ReqMsg::new_system(m.content.clone()))
                    }
                }
                // Tool messages.
                MessageKind::Tool => Some(ReqMsg::new_tool(
                    m.content.clone(),
                    m.tool_call_id
                        .clone()
                        .expect("Tool calls must have Some tool_call_id"),
                )),
                // UI/system info messages are not part of the API payload; omit.
                // NOTE: 2025-12-16
                // Adding the sysinfo messages for now, want to try this out.
                MessageKind::SysInfo => Some(ReqMsg::new_system(m.content.clone())),
            })
            .collect()
    }
    /// Creates a new ChatHistory with the systm prompt message.
    ///
    /// The root message serves as the starting point for all conversations.
    /// Its content is intentionally left empty to allow natural branching.
    pub fn new() -> Self {
        let root = BASE_SYSTEM_PROMPT.clone();
        let root_id = root.id;
        let mut messages = HashMap::default();
        messages.insert(root.id, root);
        Self {
            messages,
            current: root_id,
            // new list has same root/tail
            tail: root_id,
            // initialize cached path with root
            path_cache: vec![root_id],
            scroll_bar: None,
            totals: ConversationTotals::default(),
            current_context_tokens: None,
        }
    }

    /// Update running totals by adding the provided deltas.
    pub fn record_usage_delta(&mut self, prompt: u32, completion: u32, cost: f64) {
        self.totals.prompt_tokens = self
            .totals
            .prompt_tokens
            .saturating_add(prompt as u64);
        self.totals.completion_tokens = self
            .totals
            .completion_tokens
            .saturating_add(completion as u64);
        // If total_tokens is not explicitly provided, approximate as prompt+completion.
        self.totals.total_tokens = self
            .totals
            .total_tokens
            .saturating_add((prompt as u64).saturating_add(completion as u64));
        self.totals.cost += cost;
    }

    /// Overwrite current context token count (prompt being sent).
    pub fn set_current_context_tokens(&mut self, tokens: ContextTokens) {
        self.current_context_tokens = Some(tokens);
    }

    /// Rebuilds the cached root -> tail path.
    pub(crate) fn rebuild_path_cache(&mut self) {
        let mut path = Vec::new();
        let mut cur = Some(self.tail);
        while let Some(id) = cur {
            path.push(id);
            cur = self.messages.get(&id).and_then(|m| m.parent);
        }
        path.reverse();
        self.path_cache = path;
    }

    /// Returns an iterator of Messages on the cached root -> tail path.
    pub fn iter_path(&self) -> impl Iterator<Item = &Message> {
        self.path_cache
            .iter()
            .filter_map(|id| self.messages.get(id))
    }

    /// Fast path length (root -> tail).
    pub fn path_len(&self) -> usize {
        self.path_cache.len()
    }

    // TODO: Documentation, actually implement this (needs async?)
    pub fn add_message_user(
        &mut self,
        parent_id: Uuid,
        child_id: Uuid,
        content: String,
    ) -> Result<Uuid, ChatError> {
        let status = MessageStatus::Completed;
        let kind = MessageKind::User;
        self.add_child(parent_id, child_id, &content, status, kind, None, None)
    }

    pub fn add_message_llm(
        &mut self,
        parent_id: Uuid,
        child_id: Uuid,
        kind: MessageKind,
        content: String,
    ) -> Result<Uuid, ChatError> {
        let status = MessageStatus::Completed;
        self.add_child(parent_id, child_id, &content, status, kind, None, None)
    }

    pub fn add_message_sysinfo(
        &mut self,
        parent_id: Uuid,
        child_id: Uuid,
        kind: MessageKind,
        content: String,
    ) -> Result<Uuid, ChatError> {
        let status = MessageStatus::Completed;
        self.add_child(parent_id, child_id, &content, status, kind, None, None)
    }

    pub fn add_message_system(
        &mut self,
        parent_id: Uuid,
        child_id: Uuid,
        kind: MessageKind,
        content: String,
    ) -> Result<Uuid, ChatError> {
        let status = MessageStatus::Completed;
        self.add_child(parent_id, child_id, &content, status, kind, None, None)
    }

    pub fn add_message_tool(
        &mut self,
        parent_id: Uuid,
        child_id: Uuid,
        kind: MessageKind,
        content: String,
        tool_call_id: Option<ArcStr>,
        tool_payload: Option<crate::tools::ToolUiPayload>,
    ) -> Result<Uuid, ChatError> {
        let status = MessageStatus::Completed;
        self.add_child(
            parent_id,
            child_id,
            &content,
            status,
            kind,
            tool_call_id,
            tool_payload,
        )
    }

    /// Adds a new child message to the conversation tree.
    /// Takes some data from the state of the chat history, modifies chat history state to include
    /// the new child and message, and returns the new child id.
    ///
    /// By default, the message is pinned for 15 turns of the conversation.
    ///
    /// # Panics
    /// No explicit panics, but invalid parent_ids will result in orphaned messages
    // TODO: Consider changing to builder pattern
    pub fn add_child(
        &mut self,
        parent_id: Uuid,
        child_id: Uuid,
        content: &str,
        status: MessageStatus,
        kind: MessageKind,
        tool_call_id: Option<ArcStr>,
        tool_payload: Option<crate::tools::ToolUiPayload>,
    ) -> Result<Uuid, ChatError> {
        let child = Message {
            id: child_id,
            parent: Some(parent_id),
            children: Vec::new(),
            selected_child: None,
            content: content.to_string(),
            status,
            metadata: None,
            kind,
            tool_call_id,
            tool_payload,
            context_status: ContextStatus::default(),
        };

        let parent = self
            .messages
            .get_mut(&parent_id)
            .ok_or(ChatError::ParentNotFound(parent_id))?;

        parent.children.push(child_id);
        parent.selected_child = Some(child_id);
        self.messages.insert(child_id, child);
        // NOTE: This could be problematic, maybe?
        // Consider a case where multiple children are being added simultaneously.
        // Forget it, we would likely need a different function for that.
        self.tail = child_id;
        self.rebuild_path_cache();
        Ok(child_id)
    }

    /// Adds a new child message to the conversation tree.
    /// Takes some data from the state of the chat history, modifies chat history state to include
    /// the new child and message, and returns the new child id.
    ///
    /// Pins the message with a context status, including a given `TurnsToLive`, indicating the
    /// number of turns of the conversation before the message is either reviewed or automatically
    /// removed from the conversation. May be pinned with an Unlimited `TurnsToLive` if no
    /// automatic context management is desired.
    ///
    /// # Panics
    /// No explicit panics, but invalid parent_ids will result in orphaned messages
    pub fn add_child_message(&mut self, child: Message) -> Result<Uuid, ChatError> {
        let child_id = child.id;
        let parent_id = child.parent.expect("child message must have parent");

        let parent = self
            .messages
            .get_mut(&parent_id)
            .ok_or(ChatError::ParentNotFound(parent_id))?;

        parent.children.push(child_id);
        parent.selected_child = Some(child_id);
        self.messages.insert(child_id, child);
        // NOTE: This could be problematic, maybe?
        // Consider a case where multiple children are being added simultaneously.
        // Forget it, we would likely need a different function for that.
        self.tail = child_id;
        self.rebuild_path_cache();
        Ok(child_id)
    }

    /// Adds a new sibling message to the conversation tree.
    ///
    /// Creates a new message that shares the same parent as the specified sibling,
    /// allowing for parallel conversation branches.
    ///
    /// # Returns
    /// UUID of the newly created sibling message
    ///
    /// # Errors
    /// Returns `ChatError::SiblingNotFound` if the reference sibling doesn't exist
    /// Returns `ChatError::RootHasNoSiblings` if trying to add siblings to root message
    pub fn add_sibling(
        &mut self,
        sibling_id: Uuid,
        content: &str,
        kind: MessageKind,
        status: MessageStatus,
    ) -> Result<Uuid, ChatError> {
        let sibling = self
            .messages
            .get(&sibling_id)
            .ok_or(ChatError::SiblingNotFound(sibling_id))?;

        let parent_id = sibling.parent.ok_or(ChatError::RootHasNoSiblings)?;

        // Reuse add_child but with the sibling's parent, and generate a new message id
        let new_id = Uuid::new_v4();
        self.add_child(parent_id, new_id, content, status, kind, None, None)
    }

    /// Deletes a message and its descendant subtree from the conversation history.
    ///
    /// - The root message cannot be deleted.
    /// - If the deleted subtree contains `current` or `tail`, they are moved to the parent.
    /// - Rebuilds the cached path after mutation.
    ///
    /// Returns the new `current` message id if deletion occurred, otherwise `None`.
    pub fn delete_message(&mut self, id: Uuid) -> Option<Uuid> {
        // Cannot delete if not found or if root
        let parent_id = self.messages.get(&id).and_then(|m| m.parent)?;

        // Collect subtree nodes to delete (DFS)
        let mut stack = vec![id];
        let mut to_delete: Vec<Uuid> = Vec::new();
        while let Some(node_id) = stack.pop() {
            if let Some(msg) = self.messages.get(&node_id) {
                for &child in &msg.children {
                    stack.push(child);
                }
            }
            to_delete.push(node_id);
        }

        // Update parent's children and selected_child
        if let Some(parent) = self.messages.get_mut(&parent_id) {
            parent.children.retain(|cid| *cid != id);
            if parent.selected_child == Some(id) {
                // Prefer the last remaining child if any
                parent.selected_child = parent.children.last().copied();
            }
        }

        // Track whether current/tail are being deleted
        let deletes_current = to_delete.contains(&self.current);
        let deletes_tail = to_delete.contains(&self.tail);

        // Remove all collected nodes
        for node in to_delete {
            self.messages.remove(&node);
        }

        // Adjust tail/current if they were part of the deleted subtree
        if deletes_tail {
            self.tail = parent_id;
        }
        if deletes_current {
            self.current = parent_id;
        }

        // Rebuild path cache to reflect new tree structure
        self.rebuild_path_cache();

        Some(self.current)
    }

    /// Removes only the specified node, preserving and re-parenting its children to the node's parent.
    ///
    /// Semantics:
    /// - Does NOT delete the subtree. Instead, the node's children are spliced into the parent's
    ///   children at the same index where the deleted node was located, preserving order.
    /// - Root node cannot be deleted.
    /// - Selection updates:
    ///   - If `current` was the deleted node, it becomes the first re-parented child, or the parent if no children.
    ///   - If `tail` was the deleted node, it becomes the last re-parented child, or the parent if no children.
    /// - Rebuilds the cached path after mutation.
    ///
    /// Returns the new `current` message id if deletion occurred, otherwise `None`.
    pub fn delete_node(&mut self, id: Uuid) -> Option<Uuid> {
        // Cannot delete root or missing node
        let (parent_id, children_ids) = {
            let node = self.messages.get(&id)?;
            let pid = node.parent?;
            (pid, node.children.clone())
        };

        // Update each child's parent pointer
        for child_id in &children_ids {
            if let Some(child) = self.messages.get_mut(child_id) {
                child.parent = Some(parent_id);
            }
        }

        // Splice children into the parent's children list at the position of the deleted node
        if let Some(parent) = self.messages.get_mut(&parent_id) {
            if let Some(pos) = parent.children.iter().position(|&cid| cid == id) {
                // Remove the node placeholder
                parent.children.remove(pos);
                // Insert children in its place preserving order
                parent
                    .children
                    .splice(pos..pos, children_ids.iter().copied());

                // If the parent's selected_child pointed at the deleted node, choose a reasonable replacement
                if parent.selected_child == Some(id) {
                    parent.selected_child = children_ids
                        .first()
                        .copied()
                        .or_else(|| parent.children.last().copied());
                }
            } else {
                // If position not found, append children to parent (fallback)
                parent.children.extend(children_ids.iter().copied());
                if parent.selected_child == Some(id) {
                    parent.selected_child = children_ids
                        .first()
                        .copied()
                        .or_else(|| parent.children.last().copied());
                }
            }
        }

        // Track selection adjustments
        let deletes_current = self.current == id;
        let deletes_tail = self.tail == id;

        // Remove the node itself
        self.messages.remove(&id);

        // Adjust tail/current if they were the deleted node
        if deletes_tail {
            self.tail = children_ids.last().copied().unwrap_or(parent_id);
        }
        if deletes_current {
            self.current = children_ids.first().copied().unwrap_or(parent_id);
        }

        // Rebuild path cache to reflect new structure
        self.rebuild_path_cache();

        Some(self.current)
    }

    /// Gets the index position of a message within its parent's children list
    ///
    /// # Arguments
    /// * `message_id` - UUID of the message to locate
    ///
    /// # Returns
    /// `Some(usize)` with the index if message exists and has a parent,  
    /// `None` if message is root or parent not found
    fn get_sibling_index(&self, message_id: Uuid) -> Option<usize> {
        self.get_parent(message_id).and_then(|parent_id| {
            self.messages[&parent_id]
                .children
                .iter()
                .position(|&id| id == message_id)
        })
    }

    /// Returns an iterator of UUIDs in the full path from root to tail message
    pub fn full_path_ids(&self) -> impl Iterator<Item = Uuid> + '_ {
        let mut tail = Some(self.tail);
        std::iter::from_fn(move || {
            let id = tail?;
            tail = self.messages.get(&id).and_then(|m| m.parent);
            Some(id)
        })
        .collect::<Vec<_>>() // Collect to reverse order
        .into_iter()
        .rev()
    }

    /// Gets the full conversation path from root to tail message.
    ///
    /// Traverses the message hierarchy from the currently active message
    /// back to the root, then reverses the order for display purposes.
    ///
    /// # Example
    /// For a conversation path A -> B -> C (where C is tail):
    /// Returns [A, B, C]
    pub fn get_full_path(&self) -> Vec<&Message> {
        self.iter_path().collect()
    }

    /// Returns an iterator of UUIDs in the full path from root to current message
    pub fn current_path_ids(&self) -> impl Iterator<Item = Uuid> + '_ {
        // TODO: Figure out how to not allocate here
        let mut current = Some(self.current);
        std::iter::from_fn(move || {
            let id = current?;
            current = self.messages.get(&id).and_then(|m| m.parent);
            Some(id)
        })
        .collect::<Vec<_>>() // Collect to reverse order
        .into_iter()
        .rev()
    }

    /// Returns an iterator of UUIDs in the full path from root to current message
    pub fn current_path_ids_conv(&self) -> impl Iterator<Item = Uuid> + '_ {
        // TODO: Figure out how to not allocate here
        let mut current = Some(self.current);
        std::iter::from_fn(move || {
            let id = current?;
            current = self
                .messages
                .get(&id)
                .filter(|m| m.kind == MessageKind::User || m.kind == MessageKind::Assistant)
                .and_then(|m| m.parent);
            Some(id)
        })
        // TODO: This collect could probably be removed by implementing the double ended iterator
        // trait for something. Or maybe using VecDeque for the conversation history.
        .collect::<Vec<_>>() // Collect to reverse order
        .into_iter()
        .rev()
    }

    /// Gets the full conversation path from root to tail message.
    ///
    /// Traverses the message hierarchy from the currently active message
    /// back to the root, then reverses the order for display purposes.
    ///
    /// # Example
    /// For a conversation path A -> B -> C (where C is tail):
    /// Returns [A, B, C]
    pub fn get_current_path(&self) -> Vec<&Message> {
        self.current_path_ids()
            .filter_map(|id| self.messages.get(&id))
            .collect()
    }

    /// Gets the full conversation path from root to tail message for user and LLM messages only.
    ///
    /// Traverses the message hierarchy from the currently active message
    /// back to the root, then reverses the order for display purposes.
    ///
    /// # Example
    /// For a conversation path A -> B -> C (where C is tail):
    /// Returns [A, B, C]
    pub fn get_current_path_conv(&self) -> Vec<&Message> {
        self.current_path_ids_conv()
            .filter_map(|id| self.messages.get(&id))
            .collect()
    }

    /// Gets the full conversation path from root to tail message for user and LLM messages only.
    ///
    /// Traverses the message hierarchy from the currently active message
    /// back to the root, then reverses the order for display purposes.
    ///
    /// # Example
    /// For a conversation path A -> B -> C (where C is tail):
    /// Returns [A, B, C]
    pub fn clone_current_path_conv(&self) -> Vec<Message> {
        self.current_path_ids_conv()
            .filter_map(|id| self.messages.get(&id))
            .cloned()
            .collect()
    }

    /// Gets the parent UUID of a specified message if it exists.
    ///
    /// # Arguments
    /// * `id` - UUID of the message to check for a parent
    ///
    /// # Returns
    /// `Some(Uuid)` if the message exists and has a parent, `None` otherwise
    pub fn get_parent(&self, id: Uuid) -> Option<Uuid> {
        self.messages.get(&id).and_then(|m| m.parent)
    }

    /// Gets the first child UUID of a specified message if it exists.
    ///
    /// # Arguments
    /// * `id` - UUID of the message to check for children
    ///
    /// # Returns
    /// `Some(Uuid)` if the message exists and has at least one child, `None` otherwise
    pub fn get_first_child(&self, id: Uuid) -> Option<Uuid> {
        self.messages
            .get(&id)
            .and_then(|m| m.children.first().copied())
    }

    /// Formats chat history as Markdown for persistence
    pub fn format_for_persistence(&self) -> String {
        let mut md = String::new();
        md.push_str("# Ploke Chat History\n\n");

        for message in self.get_full_path() {
            md.push_str(&format!("## [{}]\n\n{}\n\n", message.kind, message.content));
        }

        md
    }

    /// Asynchronous persistence handler
    pub async fn persist(&self, path: &std::path::Path) -> Result<(), std::io::Error> {
        let content = self.format_for_persistence();
        atomic_write(path, content).await
    }

    /// Navigates the current path and updates the `current` message ID.
    pub fn navigate_list(&mut self, direction: ListNavigation) {
        if self.path_cache.is_empty() {
            return;
        }

        let current_index = self
            .path_cache
            .iter()
            .position(|&id| id == self.current)
            .unwrap_or(0);

        let new_index = match direction {
            ListNavigation::Up => current_index.saturating_sub(1),
            ListNavigation::Down => (current_index + 1).min(self.path_cache.len() - 1),
            ListNavigation::Top => 0,
            ListNavigation::Bottom => self.path_cache.len() - 1,
        };

        self.current = self.path_cache[new_index];
    }

    /// Navigates between sibling messages sharing the same parent
    ///
    /// # Arguments
    /// * `direction` - NavigationDirection::Next/Previous to move through siblings
    ///
    /// # Returns
    /// Result containing UUID of new current message if successful
    ///
    /// # Errors
    /// Returns `ChatError::RootHasNoSiblings` if trying to navigate from root
    /// Returns `ChatError::SiblingNotFound` if no siblings available
    pub fn navigate_sibling(&mut self, direction: NavigationDirection) -> Result<Uuid, ChatError> {
        let current_msg = self
            .messages
            .get(&self.current)
            .ok_or(ChatError::SiblingNotFound(self.current))?;

        let parent_id = current_msg.parent.ok_or(ChatError::RootHasNoSiblings)?;

        let parent = self
            .messages
            .get(&parent_id)
            .ok_or(ChatError::ParentNotFound(parent_id))?;

        let siblings = &parent.children;
        if siblings.len() < 2 {
            return Err(ChatError::SiblingNotFound(self.current)); // No other siblings navigate to
        }

        let current_idx = siblings.iter().position(|&id| id == self.current).unwrap();

        let new_idx = match direction {
            NavigationDirection::Next => (current_idx + 1) % siblings.len(),
            NavigationDirection::Previous => (current_idx + siblings.len() - 1) % siblings.len(),
        };

        self.current = siblings[new_idx];
        Ok(self.current)
    }

    /// Finds the most recent user message in the conversation chain leading to the current message.
    ///
    /// This function traverses backwards from the current message through its parent chain,
    /// looking for the first (nearest to current) message with `MessageKind::User`.
    ///
    /// # Returns
    /// - `Ok(Some((id, content)))` - The UUID and content of the most recent user message
    /// - `Ok(None)` - No user message found in the chain (only possible with root message)
    /// - `Err(ChatError)` - If message lookup fails
    pub fn last_user_msg(&self) -> Result<Option<(Uuid, String)>> {
        let mut current = self.current;
        let msg_with_id = std::iter::from_fn(move || {
            let id = current;
            current = self.messages.get(&id).and_then(|m| m.parent)?;
            Some(id)
        })
        .find_map(|id| {
            self.messages
                .get(&id)
                .filter(|m| m.kind == MessageKind::User)
                .map(|m| (m.id, m.content.clone()))
        });
        Ok(msg_with_id)
    }

    /// Adds a new Tool message and attaches a tool_call_id.
    pub fn add_tool_message(
        &mut self,
        parent_id: Uuid,
        child_id: Uuid,
        content: String,
        call_id: ArcStr,
    ) -> Result<Uuid, ChatError> {
        let status = MessageStatus::Completed;
        let kind = MessageKind::Tool;
        self.add_child(parent_id, child_id, &content, status, kind, Some(call_id), None)
    }

    /// Decrement turns to live of messages in the currently selected message history.
    /// Changes the context_status from Limited to NoneRemaining if turns to live goes from 1 to 0,
    /// indicating that the message will either be automatically left out of the LLM context or
    /// will need review and repinning.
    pub fn decrement_ttl(&mut self) {
        for msg_id in self.path_cache.iter() {
            self.messages
                .entry(*msg_id)
                .and_modify(|m| match &mut m.context_status {
                    ContextStatus::Pinned {
                        turns_to_live: ttl, ..
                    } => {
                        if let TurnsToLive::Limited(n) = ttl {
                            *n = n.saturating_sub(1);
                        }
                        if *ttl == TurnsToLive::Limited(0) {
                            *ttl = TurnsToLive::NoneRemaining
                        }
                    }
                    ContextStatus::Unpinned => {}
                });
        }
    }
}

/// Atomically writes file contents using tempfile and rename
pub(crate) async fn atomic_write(
    path: &std::path::Path,
    content: String,
) -> Result<(), std::io::Error> {
    let dir = path.parent().unwrap_or_else(|| std::path::Path::new("."));
    let mut temp = NamedTempFile::new_in(dir)?;
    temp.write_all(content.as_bytes())?;
    // Map PersistError into a plain io::Error to satisfy the return type
    temp.persist(path).map_err(std::io::Error::other)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::llm::manager::Role as LlmRole;
    use tempfile::tempdir;

    #[test]
    fn delete_root_returns_none_and_no_changes() {
        let mut ch = ChatHistory::new();
        let root = ch.current;
        let orig_len = ch.messages.len();

        let res = ch.delete_message(root);

        assert!(res.is_none());
        assert_eq!(ch.messages.len(), orig_len);
        assert_eq!(ch.current, root);
        assert_eq!(ch.tail, root);
        assert_eq!(ch.path_len(), 1);
    }

    #[test]
    fn delete_nonexistent_returns_none_and_no_effect() {
        let mut ch = ChatHistory::new();
        let root = ch.current;

        let res = ch.delete_message(Uuid::new_v4());

        assert!(res.is_none());
        assert_eq!(ch.messages.len(), 1);
        assert_eq!(ch.current, root);
        assert_eq!(ch.tail, root);
        assert_eq!(ch.path_len(), 1);
    }

    #[test]
    fn delete_leaf_updates_state() {
        let mut ch = ChatHistory::new();
        let root = ch.current;

        let u1 = Uuid::new_v4();
        ch.add_child(
            root,
            u1,
            "User: hi",
            MessageStatus::Completed,
            MessageKind::User,
            None,
            None,
        )
        .unwrap();

        // Set current to the leaf to exercise current-move behavior
        ch.current = u1;

        let res = ch.delete_message(u1);

        assert_eq!(res, Some(root));
        assert_eq!(ch.tail, root);
        assert_eq!(ch.current, root);

        // Parent should have no children and no selected child
        let parent = ch.messages.get(&root).unwrap();
        assert!(parent.children.is_empty());
        assert!(parent.selected_child.is_none());

        // Path cache should be rebuilt to just root
        assert_eq!(ch.path_len(), 1);
        let ids: Vec<Uuid> = ch.full_path_ids().collect();
        assert_eq!(ids, vec![root]);
    }

    #[test]
    fn delete_internal_node_removes_subtree() {
        let mut ch = ChatHistory::new();
        let root = ch.current;

        let u1 = Uuid::new_v4();
        ch.add_child(
            root,
            u1,
            "Q1",
            MessageStatus::Completed,
            MessageKind::User,
            None,
            None,
        )
        .unwrap();

        let a1 = Uuid::new_v4();
        ch.add_child(
            u1,
            a1,
            "A1",
            MessageStatus::Completed,
            MessageKind::Assistant,
            None,
            None,
        )
        .unwrap();

        let a2 = Uuid::new_v4();
        ch.add_child(
            u1,
            a2,
            "A2",
            MessageStatus::Completed,
            MessageKind::Assistant,
            None,
            None,
        )
        .unwrap();

        // Set current inside the subtree to be deleted
        ch.current = a1;

        let res = ch.delete_message(u1);

        assert_eq!(res, Some(root));
        assert_eq!(ch.messages.len(), 1);
        let parent = ch.messages.get(&root).unwrap();
        assert!(parent.children.is_empty());
        assert!(parent.selected_child.is_none());
        assert_eq!(ch.tail, root);
        assert_eq!(ch.current, root);
        assert_eq!(ch.path_len(), 1);
    }

    #[test]
    fn delete_sibling_updates_selected_child_and_tail() {
        let mut ch = ChatHistory::new();
        let root = ch.current;

        let u1 = Uuid::new_v4();
        ch.add_child(
            root,
            u1,
            "Q",
            MessageStatus::Completed,
            MessageKind::User,
            None,
            None,
        )
        .unwrap();

        let a1 = Uuid::new_v4();
        ch.add_child(
            u1,
            a1,
            "A1",
            MessageStatus::Completed,
            MessageKind::Assistant,
            None,
            None,
        )
        .unwrap();

        let a2 = Uuid::new_v4();
        ch.add_child(
            u1,
            a2,
            "A2",
            MessageStatus::Completed,
            MessageKind::Assistant,
            None,
            None,
        )
        .unwrap();

        // After adding a2, selected_child should point to a2
        assert_eq!(ch.messages.get(&u1).unwrap().selected_child, Some(a2));

        // Set current to a1 (the sibling that will remain)
        ch.current = a1;

        let res = ch.delete_message(a2);

        // Deletion returns Some(current) and current should remain a1
        assert_eq!(res, Some(a1));
        assert_eq!(ch.current, a1);

        // Tail should have moved up to u1 (since tail was a2)
        assert_eq!(ch.tail, u1);

        // Parent should now only contain a1 and select a1
        let parent = ch.messages.get(&u1).unwrap();
        assert_eq!(parent.children, vec![a1]);
        assert_eq!(parent.selected_child, Some(a1));

        // Path root -> u1
        assert_eq!(ch.path_len(), 2);
        let ids: Vec<Uuid> = ch.full_path_ids().collect();
        assert_eq!(ids, vec![root, u1]);
    }

    #[test]
    fn add_sibling_on_root_errors() {
        let mut ch = ChatHistory::new();
        let root = ch.current;

        let res = ch.add_sibling(root, "x", MessageKind::Assistant, MessageStatus::Completed);

        assert!(matches!(res, Err(ChatError::RootHasNoSiblings)));
    }

    #[test]
    fn navigate_sibling_requires_two_children() {
        let mut ch = ChatHistory::new();
        let root = ch.current;

        let u1 = Uuid::new_v4();
        ch.add_child(
            root,
            u1,
            "Q",
            MessageStatus::Completed,
            MessageKind::User,
            None,
            None,
        )
        .unwrap();
        ch.current = u1;

        let res = ch.navigate_sibling(NavigationDirection::Next);

        assert!(matches!(res, Err(ChatError::SiblingNotFound(_))));
    }

    #[test]
    fn last_user_msg_behaves() {
        let mut ch = ChatHistory::new();

        // No user message yet
        assert!(ch.last_user_msg().unwrap().is_none());

        let root = ch.current;

        let u1 = Uuid::new_v4();
        ch.add_child(
            root,
            u1,
            "Q1",
            MessageStatus::Completed,
            MessageKind::User,
            None,
            None,
        )
        .unwrap();

        let a1 = Uuid::new_v4();
        ch.add_child(
            u1,
            a1,
            "A1",
            MessageStatus::Completed,
            MessageKind::Assistant,
            None,
            None,
        )
        .unwrap();

        ch.current = a1;
        let (id, content) = ch
            .last_user_msg()
            .unwrap()
            .expect("should find nearest user");
        assert_eq!(id, u1);
        assert_eq!(content, "Q1");

        // Deeper conversation
        let u2 = Uuid::new_v4();
        ch.add_child(
            a1,
            u2,
            "Q2",
            MessageStatus::Completed,
            MessageKind::User,
            None,
            None,
        )
        .unwrap();

        let a2 = Uuid::new_v4();
        ch.add_child(
            u2,
            a2,
            "A2",
            MessageStatus::Completed,
            MessageKind::Assistant,
            None,
            None,
        )
        .unwrap();

        ch.current = a2;
        let (id2, content2) = ch
            .last_user_msg()
            .unwrap()
            .expect("should find deeper user");
        assert_eq!(id2, u2);
        assert_eq!(content2, "Q2");
    }

    #[test]
    fn current_path_ids_sequence() {
        let mut ch = ChatHistory::new();
        let root = ch.current;

        let u1 = Uuid::new_v4();
        ch.add_child(
            root,
            u1,
            "Q",
            MessageStatus::Completed,
            MessageKind::User,
            None,
            None,
        )
        .unwrap();

        let a1 = Uuid::new_v4();
        ch.add_child(
            u1,
            a1,
            "A",
            MessageStatus::Completed,
            MessageKind::Assistant,
            None,
            None,
        )
        .unwrap();

        ch.current = a1;

        let ids: Vec<Uuid> = ch.current_path_ids().collect();
        assert_eq!(ids, vec![root, u1, a1]);
    }

    #[test]
    fn iter_path_matches_tail_chain() {
        let mut ch = ChatHistory::new();
        let root = ch.current;

        let u1 = Uuid::new_v4();
        ch.add_child(
            root,
            u1,
            "Q",
            MessageStatus::Completed,
            MessageKind::User,
            None,
            None,
        )
        .unwrap();

        let a1 = Uuid::new_v4();
        ch.add_child(
            u1,
            a1,
            "A",
            MessageStatus::Completed,
            MessageKind::Assistant,
            None,
            None,
        )
        .unwrap();

        // Tail is a1 by construction
        let ids: Vec<Uuid> = ch.iter_path().map(|m| m.id).collect();
        assert_eq!(ch.path_len(), 3);
        assert_eq!(ids, vec![root, u1, a1]);
    }

    #[tokio::test]
    async fn persist_writes_expected_content() {
        let ch = ChatHistory::new();
        let dir = tempdir().unwrap();
        let path = dir.path().join("history.md");

        let expected = ch.format_for_persistence();
        ch.persist(&path).await.unwrap();

        let read = std::fs::read_to_string(&path).unwrap();
        assert_eq!(read, expected);
    }

    #[test]
    fn current_path_as_llm_request_messages_maps_and_filters() {
        let mut ch = ChatHistory::new();
        let root = ch.current;

        // Add a visible system message (non-empty) – should be included
        let sys1 = Uuid::new_v4();
        ch.add_message_system(
            root,
            sys1,
            MessageKind::System,
            "You are a helpful assistant.".to_string(),
        )
        .unwrap();

        // Add a user message
        let u1 = Uuid::new_v4();
        ch.add_child(
            root,
            u1,
            "Hello?",
            MessageStatus::Completed,
            MessageKind::User,
            None,
            None,
        )
        .unwrap();

        // Add an assistant message
        let a1 = Uuid::new_v4();
        ch.add_child(
            u1,
            a1,
            "Hi! How can I help?",
            MessageStatus::Completed,
            MessageKind::Assistant,
            None,
            None,
        )
        .unwrap();

        // Add a SysInfo message – should be excluded
        let info = Uuid::new_v4();
        ch.add_child(
            a1,
            info,
            "(diagnostic) not part of request",
            MessageStatus::Completed,
            MessageKind::SysInfo,
            None,
            None,
        )
        .unwrap();

        // Add a Tool message – currently excluded (missing tool_call_id context)
        let tool = Uuid::new_v4();
        ch.add_child(
            info,
            tool,
            "tool-output",
            MessageStatus::Completed,
            MessageKind::Tool,
            Some(ArcStr::from("new_tool_call:0")),
            None,
        )
        .unwrap();

        // Select the assistant message as current (root→sys1→u1→a1)
        ch.current = a1;
        ch.rebuild_path_cache();

        // Include the root system prompt when non-empty
        let msgs = ch.current_path_as_llm_request_messages();
        // Expect: [System(non-empty), User, Assistant]
        let printable = format!(
            "messages:

{:#?}
",
            msgs
        );
        eprintln!("{}", &printable);
        assert_eq!(msgs.len(), 3);
        assert_eq!(msgs[0].role, LlmRole::System);
        assert!(msgs[0].content.contains("BEGIN SYSTEM PROMPT"));
        assert_eq!(msgs[1].role, LlmRole::User);
        assert_eq!(msgs[1].content, "Hello?");
        assert_eq!(msgs[2].role, LlmRole::Assistant);
        assert_eq!(msgs[2].content, "Hi! How can I help?");
    }

    #[test]
    fn tool_messages_carry_call_ids_into_llm_requests() {
        let mut ch = ChatHistory::new();
        let root = ch.current;

        // Add a user message to parent the tool call
        let user = Uuid::new_v4();
        ch.add_message_user(root, user, "Run the tool".to_string())
            .unwrap();

        // Add a tool message with an explicit call id
        let tool_call_id = ArcStr::from("call-123");
        let tool_msg_id = Uuid::new_v4();
        ch.add_message_tool(
            user,
            tool_msg_id,
            MessageKind::Tool,
            "tool output".to_string(),
            Some(tool_call_id.clone()),
            None,
        )
        .unwrap();

        // Select the tool message to mirror how the UI renders the tail path
        ch.current = tool_msg_id;
        ch.rebuild_path_cache();

        let msgs = ch.current_path_as_llm_request_messages();
        let last = msgs.last().expect("tool message is on the path");
        assert_eq!(last.role, LlmRole::Tool);
        assert_eq!(last.tool_call_id, Some(tool_call_id));
        assert_eq!(last.content, "tool output");
    }

    #[test]
    fn record_usage_delta_accumulates() {
        let mut history = ChatHistory::new();
        history.record_usage_delta(10, 5, 1.5);
        history.record_usage_delta(2, 3, 0.5);

        assert_eq!(history.totals.prompt_tokens, 12);
        assert_eq!(history.totals.completion_tokens, 8);
        assert_eq!(history.totals.total_tokens, 20);
        assert!((history.totals.cost - 2.0).abs() < f64::EPSILON);
    }

    #[test]
    fn set_current_context_tokens_overwrites() {
        let mut history = ChatHistory::new();
        history.set_current_context_tokens(ContextTokens::new(123, TokenKind::Estimated));
        assert_eq!(
            history.current_context_tokens,
            Some(ContextTokens::new(123, TokenKind::Estimated))
        );
        history.set_current_context_tokens(ContextTokens::new(42, TokenKind::Estimated));
        assert_eq!(
            history.current_context_tokens,
            Some(ContextTokens::new(42, TokenKind::Estimated))
        );
    }
}
