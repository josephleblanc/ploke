use crate::app_state::StateError;
use crate::llm::LLMMetadata;

use super::*;
use std::fmt;

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
            ChatError::SiblingNotFound(id) => write!(f, "Sibling messa not found: {}", id),
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

use serde::{Deserialize, Serialize};
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

        // Check for content conflict
        if self.content.is_some() && self.append_content.is_some() {
            return Err(UpdateError::ContentConflict);
        }

        // Validate status transitions
        if let Some(new_status) = &self.status {
            match (current_status, new_status) {
                // Completed messages are terminal (handled above)

                // Can only complete generating messages
                (_, MessageStatus::Completed)
                    if !matches!(current_status, MessageStatus::Generating) =>
                {
                    return Err(UpdateError::InvalidStatusTransition(
                        current_status.clone(),
                        new_status.clone(),
                    ));
                }

                // Can only retry from error state
                (MessageStatus::Error { .. }, MessageStatus::Pending) => (),

                // Invalid transitions
                (from, to) if from != to => {
                    return Err(UpdateError::InvalidStatusTransition(
                        from.clone(),
                        to.clone(),
                    ));
                }

                _ => {}
            }
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
#[derive(Debug)]
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
// TODO: Needs updating for concurrency (DashMap? Something else?)
#[derive(Debug, Default)]
pub struct ChatHistory {
    /// All messages in the conversation history, indexed by UUID
    pub messages: HashMap<Uuid, Message>,
    /// UUID of the currently active message in the conversation flow
    pub current: Uuid,
    /// Final message in the currently selected message list.
    pub tail: Uuid,
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
        };
        let root_id = root.id;

        let mut messages = HashMap::new();
        messages.insert(root.id, root);
        Self {
            messages,
            current: root_id,
            // new list has same root/tail
            tail: root_id,
        }
    }

    // TODO: Documentation
    pub fn add_message(&mut self, parent_id: Uuid, content: String) -> Result<(), StateError> {
        todo!();
        Ok(())
    }

    /// Adds a new child message to the conversation tree.
    ///
    /// # Arguments
    /// * `parent_id` - UUID of the parent message to attach to
    /// * `content` - Text content for the new message
    ///
    /// # Returns
    /// UUID of the newly created child message
    ///
    /// # Panics
    /// No explicit panics, but invalid parent_ids will result in orphaned messages
    pub fn add_child(
        &mut self,
        parent_id: Uuid,
        content: &str,
        status: MessageStatus,
    ) -> Result<Uuid, ChatError> {
        let child_id = Uuid::new_v4();
        let child = Message {
            id: child_id,
            parent: Some(parent_id),
            children: Vec::new(),
            selected_child: None,
            content: content.to_string(),
            status,
            metadata: None,
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
        Ok(child_id)
    }

    /// Adds a new sibling message to the conversation tree.
    ///
    /// Creates a new message that shares the same parent as the specified sibling,
    /// allowing for parallel conversation branches.
    ///
    /// # Arguments
    /// * `sibling_id` - UUID of an existing sibling message to reference
    /// * `content` - Text content for the new sibling message
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

        // Reuse add_child but with the sibling's parent
        self.add_child(parent_id, content, status)
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

    /// Returns an iterator of UUIDs in the full path from root to current message
    pub fn full_path_ids(&self) -> impl Iterator<Item = Uuid> + '_ {
        let mut tail = Some(self.current);
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
        self.full_path_ids()
            .filter_map(|id| self.messages.get(&id))
            .collect()
    }

    /// Returns an iterator of UUIDs in the full path from root to current message
    pub fn current_path_ids(&self) -> impl Iterator<Item = Uuid> + '_ {
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
}
