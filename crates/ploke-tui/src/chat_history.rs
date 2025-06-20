use super::*;

#[derive(Debug, Default)]
pub struct Message {
    id: Uuid,
    parent: Option<Uuid>,
    children: Vec<Uuid>,
    content: String,
}

#[derive(Debug, Default)]
pub struct ChatHistory {
    messages: HashMap<Uuid, Message>,
    current: Uuid,
}

impl ChatHistory {
    pub fn new() -> Self {
        let root = Message {
            id: Uuid::new_v4(),
            parent: None,
            children: Vec::new(),
            content: String::new(),
        };
        // new variable for `Copy`
        let root_id = root.id;

        let mut messages = HashMap::new();
        messages.insert(root.id, root);
        Self {
            messages,
            current: root_id,
        }
    }

    pub fn add_child(&mut self, parent_id: Uuid, content: &str) -> Uuid {
        let child_id = Uuid::new_v4();
        let child = Message {
            id: child_id,
            parent: Some(parent_id),
            children: Vec::new(),
            content: content.to_string(),
        };

        if let Some(parent) = self.messages.get_mut(&parent_id) {
            parent.children.push(child_id);
        }

        self.messages.insert(child_id, child);
        child_id
    }


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
