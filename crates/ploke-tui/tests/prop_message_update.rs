//! Property tests for `MessageUpdate::validate` and `Message::try_update`.
//!
//! Covered (exactly):
//! - `content`/`append_content`: arbitrary Unicode strings (0..200 chars).
//! - `Error.description`: arbitrary Unicode strings (0..80 chars).
//! - `metadata` ranges:
//!   - `model`: Unicode string (0..40 chars).
//!   - `usage`: each field in 0..1_000_000 (total_tokens in 0..2_000_000).
//!   - `processing_time`: 0..120_000 ms.
//!   - `cost`: 0.0..1_000_000.0.
//!   - `performance`: tokens/sec 0.0..50_000.0, times 0..120_000 ms.
//! - `status` transitions: all pairs are checked against explicit rules.
//! - Invariants:
//!   - Completed messages reject any update where any field is set.
//!   - Only `Generating -> Completed` is accepted.
//!   - `Pending -> Error` and `Generating -> Error` are accepted.
//!   - `Error -> Pending` is rejected with `Placeholder`.
//!   - All other transitions are rejected with `InvalidStatusTransition`.
//! - `Message::try_update`:
//!   - Merges metadata prompt/completion tokens and cost.
//!   - Empty completion content returns `EmptyContentCompletion` and sets status to `Error`.
//!   - Once a message reaches `Completed`, subsequent updates are rejected.
//!
//! Not covered:
//! - True concurrency or race conditions (tests are single-threaded sequences only).
//! - Extremely long strings beyond the bounded generators.
use proptest::prelude::*;
use std::time::Duration;
use uuid::Uuid;

use ploke_tui::chat_history::{
    ContextStatus, Message, MessageKind, MessageStatus, MessageUpdate, UpdateError,
};
use ploke_llm::response::{FinishReason, TokenUsage};
use ploke_llm::types::meta::{LLMMetadata, PerformanceMetrics};

fn any_string(max: usize) -> impl Strategy<Value = String> {
    proptest::collection::vec(any::<char>(), 0..max).prop_map(|chars| chars.into_iter().collect())
}

fn finish_reason_strategy() -> impl Strategy<Value = FinishReason> {
    prop_oneof![
        Just(FinishReason::Stop),
        Just(FinishReason::Length),
        Just(FinishReason::ContentFilter),
        Just(FinishReason::ToolCalls),
        Just(FinishReason::Timeout),
        any_string(40).prop_map(FinishReason::Error),
    ]
}

fn metadata_strategy() -> impl Strategy<Value = LLMMetadata> {
    (
        any_string(40),
        (0u32..1_000_000, 0u32..1_000_000, 0u32..2_000_000),
        finish_reason_strategy(),
        0u64..120_000u64,
        0.0f64..1_000_000.0f64,
        (0.0f32..50_000.0f32, 0u64..120_000u64, 0u64..120_000u64),
    )
        .prop_map(
            |(model, (prompt, completion, total), finish_reason, proc_ms, cost, perf)| {
                let usage = TokenUsage {
                    prompt_tokens: prompt,
                    completion_tokens: completion,
                    total_tokens: total,
                };
                let performance = PerformanceMetrics {
                    tokens_per_second: perf.0,
                    time_to_first_token: Duration::from_millis(perf.1),
                    queue_time: Duration::from_millis(perf.2),
                };
                LLMMetadata {
                    model,
                    usage,
                    finish_reason,
                    processing_time: Duration::from_millis(proc_ms),
                    cost,
                    performance,
                }
            },
        )
}

fn status_strategy() -> impl Strategy<Value = MessageStatus> {
    prop_oneof![
        Just(MessageStatus::Pending),
        Just(MessageStatus::Generating),
        Just(MessageStatus::Completed),
        any_string(80).prop_map(|description| MessageStatus::Error { description }),
    ]
}

fn expected_transition(
    current: &MessageStatus,
    next: &MessageStatus,
) -> Result<(), UpdateError> {
    if matches!(current, MessageStatus::Completed) {
        return Err(UpdateError::ImmutableMessage);
    }

    match (current, next) {
        (MessageStatus::Generating, MessageStatus::Completed) => Ok(()),
        (MessageStatus::Generating, MessageStatus::Error { .. }) => Ok(()),
        (MessageStatus::Pending, MessageStatus::Error { .. }) => Ok(()),
        (_, MessageStatus::Completed) if !matches!(current, MessageStatus::Generating) => {
            Err(UpdateError::InvalidStatusTransition(current.clone(), next.clone()))
        }
        (MessageStatus::Error { .. }, MessageStatus::Pending) => Err(UpdateError::Placeholder),
        (from, to) if from != to => Err(UpdateError::InvalidStatusTransition(
            from.clone(),
            to.clone(),
        )),
        _ => Ok(()),
    }
}

fn base_message(status: MessageStatus, content: String) -> Message {
    Message {
        id: Uuid::new_v4(),
        branch_id: Uuid::nil(),
        status,
        metadata: None,
        parent: None,
        children: Vec::new(),
        selected_child: None,
        content,
        kind: MessageKind::User,
        tool_call_id: None,
        tool_payload: None,
        context_status: ContextStatus::default(),
        last_included_turn: None,
        include_count: 0,
    }
}

proptest! {
    #[test]
    fn completed_rejects_any_update_fields(
        content in any_string(200),
        append in any_string(200),
        status in status_strategy(),
        metadata in metadata_strategy(),
        set_content in any::<bool>(),
        set_append in any::<bool>(),
        set_status in any::<bool>(),
        set_metadata in any::<bool>(),
    ) {
        let update = MessageUpdate {
            content: set_content.then_some(content),
            append_content: set_append.then_some(append),
            status: set_status.then_some(status),
            metadata: set_metadata.then_some(metadata),
        };

        let result = update.validate(&MessageStatus::Completed);
        if set_content || set_append || set_status || set_metadata {
            prop_assert_eq!(result, Err(UpdateError::ImmutableMessage));
        } else {
            prop_assert!(result.is_ok());
        }
    }

    #[test]
    fn completion_requires_generating(current in status_strategy()) {
        let update = MessageUpdate {
            status: Some(MessageStatus::Completed),
            ..Default::default()
        };
        let result = update.validate(&current);
        let expected = expected_transition(&current, &MessageStatus::Completed);
        prop_assert_eq!(result, expected);
    }

    #[test]
    fn status_transition_matrix_matches_rules(
        current in status_strategy(),
        next in status_strategy(),
    ) {
        let update = MessageUpdate {
            status: Some(next.clone()),
            ..Default::default()
        };
        let result = update.validate(&current);
        let expected = expected_transition(&current, &next);
        prop_assert_eq!(result, expected);
    }

    #[test]
    fn append_content_allowed_for_non_completed(
        current in status_strategy().prop_filter("exclude completed", |s| !matches!(s, MessageStatus::Completed)),
        append in any_string(200),
    ) {
        let update = MessageUpdate {
            append_content: Some(append),
            ..Default::default()
        };
        prop_assert!(update.validate(&current).is_ok());
    }

    #[test]
    fn empty_completion_content_errors(
        content in Just(String::new()),
    ) {
        let mut msg = base_message(MessageStatus::Generating, content);
        let update = MessageUpdate {
            status: Some(MessageStatus::Completed),
            ..Default::default()
        };
        let result = msg.try_update(update);
        prop_assert_eq!(result, Err(UpdateError::EmptyContentCompletion));
        prop_assert!(
            matches!(msg.status, MessageStatus::Error { .. }),
            "expected error status after empty completion content"
        );
    }

    #[test]
    fn metadata_merge_adds_prompt_completion_and_cost(
        base in metadata_strategy(),
        delta in metadata_strategy(),
    ) {
        let mut msg = base_message(MessageStatus::Generating, "seed".to_string());
        msg.metadata = Some(base.clone());

        let update = MessageUpdate {
            metadata: Some(delta.clone()),
            ..Default::default()
        };

        let result = msg.try_update(update);
        prop_assert!(result.is_ok());

        let merged = msg.metadata.expect("metadata");
        prop_assert_eq!(
            merged.usage.prompt_tokens,
            base.usage.prompt_tokens.saturating_add(delta.usage.prompt_tokens)
        );
        prop_assert_eq!(
            merged.usage.completion_tokens,
            base.usage.completion_tokens.saturating_add(delta.usage.completion_tokens)
        );
        prop_assert!((merged.cost - (base.cost + delta.cost)).abs() < f64::EPSILON);
    }

    #[test]
    fn completed_state_blocks_future_updates(
        updates in proptest::collection::vec(
            (any_string(80), any_string(80), status_strategy(), prop::option::of(metadata_strategy())),
            1..20
        ),
    ) {
        let mut msg = base_message(MessageStatus::Generating, "seed".to_string());

        for (content, append, status, metadata) in updates {
            let update = MessageUpdate {
                content: Some(content),
                append_content: Some(append),
                status: Some(status),
                metadata,
            };

            let was_completed = matches!(msg.status, MessageStatus::Completed);
            let result = msg.try_update(update);

            if was_completed {
                prop_assert_eq!(result, Err(UpdateError::ImmutableMessage));
                prop_assert!(matches!(msg.status, MessageStatus::Completed));
            }
        }
    }
}

#[test]
fn generating_can_complete() {
    let update = MessageUpdate {
        status: Some(MessageStatus::Completed),
        ..Default::default()
    };

    assert!(update.validate(&MessageStatus::Generating).is_ok());
}
