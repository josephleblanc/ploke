use std::sync::Arc;

use ploke_core::ArcStr;
use ploke_core::rag_types::ApplyCodeEditResult;
use ploke_core::tool_types::ToolName;
use ploke_db::NodeType;
use ploke_tui::{
    AppEvent, EventPriority,
    app_state::core::derive_edit_proposal_id,
    app_state::events::SystemEvent,
    rag::{
        tools::apply_code_edit_tool,
        utils::{ApplyCodeEditRequest, Edit, ToolCallParams},
    },
    test_utils::new_test_harness::AppHarness,
    tools::ToolErrorWire,
};
use tokio::time::{Duration, timeout};
use uuid::Uuid;

fn build_params(
    harness: &AppHarness,
    request_id: Uuid,
    node_type: NodeType,
    canon: &str,
    code: &str,
) -> ToolCallParams {
    let typed_req = ApplyCodeEditRequest {
        confidence: Some(0.9),
        edits: vec![Edit::Canonical {
            file: "src/impls.rs".to_string(),
            canon: canon.to_string(),
            node_type,
            code: code.to_string(),
        }],
    };

    ToolCallParams {
        state: Arc::clone(&harness.state),
        event_bus: Arc::clone(&harness.event_bus),
        request_id,
        parent_id: Uuid::new_v4(),
        name: ToolName::ApplyCodeEdit,
        typed_req,
        call_id: ArcStr::from("method-semantic-edit-regression"),
    }
}

fn proposal_id_for(request_id: Uuid) -> Uuid {
    derive_edit_proposal_id(request_id, &ArcStr::from("method-semantic-edit-regression"))
}

async fn recv_matching_event(
    event_rx: &mut tokio::sync::broadcast::Receiver<AppEvent>,
    request_id: Uuid,
) -> AppEvent {
    let deadline = Duration::from_secs(5);
    timeout(deadline, async move {
        loop {
            let event = event_rx.recv().await.expect("event bus dropped");
            match &event {
                AppEvent::System(SystemEvent::ToolCallCompleted {
                    request_id: event_request_id,
                    ..
                })
                | AppEvent::System(SystemEvent::ToolCallFailed {
                    request_id: event_request_id,
                    ..
                }) if *event_request_id == request_id => return event,
                _ => {}
            }
        }
    })
    .await
    .expect("timed out waiting for matching tool event")
}

#[tokio::test(flavor = "multi_thread")]
async fn apply_code_edit_accepts_method_targets() {
    let harness = AppHarness::spawn().await.expect("spawn harness");
    let request_id = Uuid::new_v4();
    let mut event_rx = harness.event_bus.subscribe(EventPriority::Realtime);

    let params = build_params(
        &harness,
        request_id,
        NodeType::Method,
        "crate::impls::SimpleStruct::new",
        "pub fn new(data: i32) -> Self { Self { data } }",
    );

    apply_code_edit_tool(params).await;

    let event = recv_matching_event(&mut event_rx, request_id).await;
    let (content, ui_payload) = match event {
        AppEvent::System(SystemEvent::ToolCallCompleted {
            content,
            ui_payload,
            ..
        }) => (content, ui_payload),
        AppEvent::System(SystemEvent::ToolCallFailed { error, .. }) => {
            panic!("method target should have staged successfully: {error}")
        }
        other => panic!("unexpected event: {other:?}"),
    };

    let result: ApplyCodeEditResult =
        serde_json::from_str(&content).expect("parse ToolCallCompleted payload");
    assert!(result.ok);
    assert_eq!(result.staged, 1);
    assert_eq!(result.applied, 0);
    assert!(
        result.files.iter().any(|file| file.ends_with("impls.rs")),
        "completion payload should include the method file"
    );
    assert_eq!(
        ui_payload
            .and_then(|payload| payload.error_code)
            .map(|code| format!("{code:?}")),
        None
    );

    let proposals = harness.state.proposals.read().await;
    let proposal = proposals
        .get(&proposal_id_for(request_id))
        .expect("method edit should stage a proposal");
    assert!(proposal.is_semantic);
    assert_eq!(
        proposal.status,
        ploke_tui::app_state::core::EditProposalStatus::Pending
    );
    assert!(
        proposal.files.iter().any(|file| file.ends_with("impls.rs")),
        "proposal should reference the method file"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn apply_code_edit_returns_structured_method_hint_for_function_mismatch() {
    let harness = AppHarness::spawn().await.expect("spawn harness");
    let request_id = Uuid::new_v4();
    let mut event_rx = harness.event_bus.subscribe(EventPriority::Realtime);

    let params = build_params(
        &harness,
        request_id,
        NodeType::Function,
        "crate::impls::SimpleStruct::new",
        "pub fn new(data: i32) -> Self { Self { data } }",
    );

    apply_code_edit_tool(params).await;

    let event = recv_matching_event(&mut event_rx, request_id).await;
    let error = match event {
        AppEvent::System(SystemEvent::ToolCallFailed { error, .. }) => error,
        AppEvent::System(SystemEvent::ToolCallCompleted { content, .. }) => {
            panic!("function mismatch should not succeed: {content}")
        }
        other => panic!("unexpected event: {other:?}"),
    };

    let wire = ToolErrorWire::parse(&error).expect("parse tool error wire payload");
    assert_eq!(wire.llm["code"].as_str(), Some("WrongType"));
    assert_eq!(wire.llm["field"].as_str(), Some("node_type"));
    assert_eq!(wire.llm["expected"].as_str(), Some("method"));
    assert_eq!(wire.llm["received"].as_str(), Some("function"));
    assert_eq!(
        wire.llm["retry_hint"].as_str(),
        Some("Retry with node_type=method for this canonical path.")
    );

    let retry_context = wire.llm["retry_context"]
        .as_object()
        .expect("retry_context object");
    assert_eq!(
        retry_context
            .get("requested_node_type")
            .and_then(|v| v.as_str()),
        Some("function")
    );
    assert_eq!(
        retry_context
            .get("suggested_node_type")
            .and_then(|v| v.as_str()),
        Some("method")
    );
    assert_eq!(
        retry_context.get("owner_name").and_then(|v| v.as_str()),
        Some("SimpleStruct")
    );
    assert_eq!(
        retry_context.get("canon").and_then(|v| v.as_str()),
        Some("crate::impls::SimpleStruct::new")
    );
    assert_eq!(
        retry_context.get("reason").and_then(|v| v.as_str()),
        Some("unique method target exists at the same coordinates")
    );

    let proposals = harness.state.proposals.read().await;
    assert!(
        !proposals.contains_key(&proposal_id_for(request_id)),
        "function mismatch should not stage a proposal"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn apply_code_edit_reports_ambiguous_method_targets_explicitly() {
    let harness = AppHarness::spawn().await.expect("spawn harness");
    let request_id = Uuid::new_v4();
    let mut event_rx = harness.event_bus.subscribe(EventPriority::Realtime);

    let params = build_params(
        &harness,
        request_id,
        NodeType::Method,
        "crate::impls::SimpleTrait::trait_method",
        "fn trait_method(&self) -> i32 { 0 }",
    );

    apply_code_edit_tool(params).await;

    let event = recv_matching_event(&mut event_rx, request_id).await;
    let error = match event {
        AppEvent::System(SystemEvent::ToolCallFailed { error, .. }) => error,
        AppEvent::System(SystemEvent::ToolCallCompleted { content, .. }) => {
            panic!("ambiguous method target should not succeed: {content}")
        }
        other => panic!("unexpected event: {other:?}"),
    };

    let wire = ToolErrorWire::parse(&error).expect("parse tool error wire payload");
    assert_eq!(wire.llm["code"].as_str(), Some("InvalidFormat"));
    let message = wire.llm["message"].as_str().expect("message string");
    assert!(
        message.starts_with(
            "Ambiguous method target for canon=crate::impls::SimpleTrait::trait_method in file=/home/brasides/code/ploke/tests/fixture_crates/fixture_nodes/src/impls.rs;"
        ),
        "unexpected ambiguity message: {message}"
    );
    assert!(
        message.ends_with("candidates matched after method parsing."),
        "unexpected ambiguity message: {message}"
    );

    let retry_context = wire.llm["retry_context"]
        .as_object()
        .expect("retry_context object");
    assert_eq!(
        retry_context
            .get("requested_node_type")
            .and_then(|v| v.as_str()),
        Some("method")
    );
    assert_eq!(
        retry_context.get("owner_name").and_then(|v| v.as_str()),
        Some("SimpleTrait")
    );
    assert!(
        retry_context
            .get("candidate_count")
            .and_then(|v| v.as_u64())
            .expect("candidate_count present")
            >= 2
    );

    let proposals = harness.state.proposals.read().await;
    assert!(
        !proposals.contains_key(&request_id),
        "ambiguous method target should not stage a proposal"
    );
}
