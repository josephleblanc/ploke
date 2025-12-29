use proptest::prelude::*;
use uuid::Uuid;

use ploke_tui::app::view::components::conversation::ConversationView;
use ploke_tui::chat_history::{ContextStatus, Message, MessageKind, MessageStatus};
use ploke_tui::tools::ToolVerbosity;

fn message(content: String) -> Message {
    Message {
        id: Uuid::new_v4(),
        status: MessageStatus::Completed,
        metadata: None,
        parent: None,
        children: Vec::new(),
        selected_child: None,
        content,
        kind: MessageKind::User,
        tool_call_id: None,
        tool_payload: None,
        context_status: ContextStatus::default(),
    }
}

proptest! {
    #[test]
    fn conversation_offset_is_clamped(
        contents in proptest::collection::vec("[a-zA-Z0-9 .,_-]{0,200}", 0..50),
        width in 10u16..120u16,
        height in 1u16..60u16,
        selection in proptest::option::of(0usize..50),
    ) {
        let messages: Vec<Message> = contents.into_iter().map(message).collect();
        let path_len = messages.len();
        let selected_index = selection.filter(|idx| *idx < path_len);

        let mut view = ConversationView::default();
        view.prepare(
            messages.iter(),
            path_len,
            width,
            height,
            selected_index,
            ToolVerbosity::Normal,
        );

        let total_height: u32 = view.item_heights().iter().map(|h| *h as u32).sum();
        let total_height_u16 = total_height.min(u16::MAX as u32) as u16;
        let max_offset = total_height_u16.saturating_sub(height);

        prop_assert!(view.offset() <= max_offset);
        prop_assert_eq!(view.item_heights().len(), path_len);

        if path_len == 0 {
            prop_assert_eq!(view.offset(), 0);
            prop_assert!(view.item_heights().is_empty());
        }
    }
}
