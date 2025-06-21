use super::*;
use std::fmt;

#[derive(Debug)]
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
    /// Selected Child is the default selection for the next navigation down
    /// Useful for `move_selection_down`
    pub selected_child: Option<Uuid>,
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
            selected_child: None,
            content: content.to_string(),
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
    pub fn add_sibling(&mut self, sibling_id: Uuid, content: &str) -> Result<Uuid, ChatError> {
        let sibling = self
            .messages
            .get(&sibling_id)
            .ok_or(ChatError::SiblingNotFound(sibling_id))?;

        let parent_id = sibling.parent.ok_or(ChatError::RootHasNoSiblings)?;

        // Reuse add_child but with the sibling's parent
        self.add_child(parent_id, content)
    }

    // Add documentation AI
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
