use super::*;
use std::fmt;

#[derive(Debug)]
pub enum ChatError {
    ParentNotFound(Uuid),
}

impl fmt::Display for ChatError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ChatError::ParentNotFound(id) => write!(f, "Parent message not found: {}", id),
        }
    }
}

impl std::error::Error for ChatError {}

/// Represents an individual message in the branching conversation tree.
/// 
/// Each message forms a node in the hierarchical chat history with:
/// - Links to its parent message (if any)
/// - List of child messages forming conversation branches
/// - Unique identifier and content storage
#[derive(Debug, Default)]
pub struct Message {
    /// Unique identifier for the message
    pub id: Uuid,
    /// Parent message UUID (None for root messages)
    pub parent: Option<Uuid>,
    /// Child message UUIDs forming conversation branches
    pub children: Vec<Uuid>,
    /// Text content of the message
    pub content: String,
}

/// Manages the complete branching conversation history using a tree structure.
///
/// Stores all messages in a HashMap for efficient lookup and maintains
/// the current position in the conversation tree.
#[derive(Debug, Default)]
pub struct ChatHistory {
    /// All messages in the conversation history, indexed by UUID
    pub messages: HashMap<Uuid, Message>,
    /// UUID of the currently active message in the conversation flow
    pub current: Uuid,
}

impl ChatHistory {
    /// Creates a new ChatHistory with an empty root message.
    ///
    /// The root message serves as the starting point for all conversations.
    /// Its content is intentionally left empty to allow natural branching.
    pub fn new() -> Self {
        let root = Message {
            id: Uuid::new_v4(),
            parent: None,
            children: Vec::new(),
            content: String::new(),
        };
        let root_id = root.id;

        let mut messages = HashMap::new();
        messages.insert(root.id, root);
        Self {
            messages,
            current: root_id,
        }
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
    pub fn add_child(&mut self, parent_id: Uuid, content: &str) -> Result<Uuid, ChatError> {
        let child_id = Uuid::new_v4();
        let child = Message {
            id: child_id,
            parent: Some(parent_id),
            children: Vec::new(),
            content: content.to_string(),
        };

        let parent = self.messages.get_mut(&parent_id)
            .ok_or(ChatError::ParentNotFound(parent_id))?;
        
        parent.children.push(child_id);
        self.messages.insert(child_id, child);
        Ok(child_id)
    }

    /// Gets the current conversation path from root to active message.
    ///
    /// Traverses the message hierarchy from the currently active message
    /// back to the root, then reverses the order for display purposes.
    ///
    /// # Example
    /// For a conversation path A -> B -> C (where C is current):
    /// Returns [A, B, C]
    pub fn get_current_path(&self) -> Vec<&Message> {
        let mut path = Vec::new();
        let mut current_id = Some(self.current);

        while let Some(id) = current_id {
            if let Some(msg) = self.messages.get(&id) {
                path.push(msg);
                current_id = msg.parent;
            } else {
                break;
            }
        }

        path.reverse();
        path
    }
}
