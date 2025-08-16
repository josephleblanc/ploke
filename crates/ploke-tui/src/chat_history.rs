use crate::AppEvent;
use crate::app_state::ListNavigation;
use crate::llm::LLMMetadata;

use std::collections::HashMap;
use std::io::Write as _;
use std::fmt;

use color_eyre::Result;

#[derive(Debug, Clone, Copy)]
pub enum NavigationDirection {
    Next,
    Previous,
}

#[derive(Debug)]
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
    /// A message generated by a tool or agent.
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
}

impl ChatHistory {
    /// Creates a new ChatHistory with an empty root message.
    ///
    /// The root message serves as the starting point for all conversations.
    /// Its content is intentionally left empty to allow natural branching.
    pub fn new() -> Self {
        let root_id = Uuid::new_v4();
        let root = Message {
            id: root_id,
            status: MessageStatus::Completed,
            metadata: None,
            parent: None,
            children: Vec::new(),
            selected_child: None,
            content: String::new(),
            kind: MessageKind::System,
        };
        let root_id = root.id;

        let mut messages = HashMap::new();
        messages.insert(root.id, root);
        Self {
            messages,
            current: root_id,
            // new list has same root/tail
            tail: root_id,
            // initialize cached path with root
            path_cache: vec![root_id],
            scroll_bar: None,
        }
    }

    /// Rebuilds the cached root -> tail path.
    fn rebuild_path_cache(&mut self) {
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
        self.path_cache.iter().filter_map(|id| self.messages.get(id))
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
        self.add_child(parent_id, child_id, &content, status, kind)
    }

    pub fn add_message_llm(
        &mut self,
        parent_id: Uuid,
        child_id: Uuid,
        kind: MessageKind,
        content: String,
    ) -> Result<Uuid, ChatError> {
        let status = MessageStatus::Completed;
        self.add_child(parent_id, child_id, &content, status, kind)
    }

    pub fn add_message_system(
        &mut self,
        parent_id: Uuid,
        child_id: Uuid,
        kind: MessageKind,
        content: String,
    ) -> Result<Uuid, ChatError> {
        let status = MessageStatus::Completed;
        self.add_child(parent_id, child_id, &content, status, kind)
    }

    /// Adds a new child message to the conversation tree.
    /// Takes some data from the state of the chat history, modifies chat history state to include
    /// the new child and message, and returns the new child id.
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
        status: MessageStatus,
    ) -> Result<Uuid, ChatError> {
        let sibling = self
            .messages
            .get(&sibling_id)
            .ok_or(ChatError::SiblingNotFound(sibling_id))?;

        let parent_id = sibling.parent.ok_or(ChatError::RootHasNoSiblings)?;

        // Reuse add_child but with the sibling's parent, and generate a new message id
        // NOTE: Assumes the same kind (safe for sibling of message)
        let new_id = Uuid::new_v4();
        self.add_child(parent_id, new_id, content, status, sibling.kind)
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
            md.push_str(&format!(
                "## [{}] {}\n\n{}\n\n",
                message.kind,
                chrono::Utc::now().to_rfc3339(),
                message.content
            ));
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
}

/// Atomically writes file contents using tempfile and rename
pub(crate) async fn atomic_write(
    path: &std::path::Path,
    content: String,
) -> Result<(), std::io::Error> {
    let dir = path
        .parent()
        .unwrap_or_else(|| std::path::Path::new("."));
    let mut temp = NamedTempFile::new_in(dir)?;
    temp.write_all(content.as_bytes())?;
    // Map PersistError into a plain io::Error to satisfy the return type
    temp.persist(path)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
    Ok(())
}
