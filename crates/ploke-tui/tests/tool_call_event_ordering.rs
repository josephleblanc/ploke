use ploke_tui::app_state::events::SystemEvent;
use ploke_tui::test_utils::new_test_harness::AppHarness;
use ploke_tui::{AppEvent, EventPriority};
use ploke_core::ArcStr;
use ploke_core::tool_types::{FunctionMarker, ToolName};
use ploke_llm::response::{FunctionCall, ToolCall};
use std::sync::Once;
use tokio::sync::oneshot;
use tokio::time::{Duration, Instant};
use tracing_subscriber::EnvFilter;
use uuid::Uuid;

fn init_test_tracing() {
    static INIT: Once = Once::new();
    INIT.call_once(|| {
        let log_path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/reports/tool_call_event_ordering_headless_trace.log");
        let file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(log_path);
        if let Ok(file) = file {
            let _ = tracing_subscriber::fmt()
                .with_env_filter(EnvFilter::from_default_env())
                .with_writer(file)
                .try_init();
        }
    });
}

#[tokio::test(flavor = "multi_thread")]
async fn tool_call_events_preserve_order_in_headless_app() {
    init_test_tracing();
    let harness = AppHarness::spawn().await.expect("spawn harness");
    let request_id = Uuid::new_v4();
    let parent_id = Uuid::new_v4();
    let call_id = ArcStr::from("ordering-check-1");
    let tool_call = ToolCall {
        call_id: call_id.clone(),
        call_type: FunctionMarker,
        function: FunctionCall {
            name: ToolName::ApplyCodeEdit,
            arguments: "{}".to_string(),
        },
    };

    let (result_tx, result_rx) = oneshot::channel::<(bool, bool, bool)>();
    let event_bus = harness.event_bus.clone();
    let call_id_observer = call_id.clone();
    tokio::spawn(async move {
        let mut rt_rx = event_bus.subscribe(EventPriority::Realtime);
        let mut bg_rx = event_bus.subscribe(EventPriority::Background);
        let mut seen_requested = false;
        let mut seen_completed = false;
        let mut violation = false;
        let deadline = Instant::now() + Duration::from_millis(200);

        loop {
            if let Ok(ev) = rt_rx.try_recv() {
                handle_tool_event(&ev, &call_id_observer, request_id, &mut seen_requested, &mut seen_completed, &mut violation);
            } else if let Ok(ev) = bg_rx.try_recv() {
                handle_tool_event(&ev, &call_id_observer, request_id, &mut seen_requested, &mut seen_completed, &mut violation);
            } else if Instant::now() >= deadline {
                break;
            } else {
                tokio::select! {
                    biased;
                    Ok(ev) = rt_rx.recv() => {
                        handle_tool_event(&ev, &call_id_observer, request_id, &mut seen_requested, &mut seen_completed, &mut violation);
                    }
                    Ok(ev) = bg_rx.recv() => {
                        handle_tool_event(&ev, &call_id_observer, request_id, &mut seen_requested, &mut seen_completed, &mut violation);
                    }
                    _ = tokio::time::sleep(Duration::from_millis(5)) => {}
                }
            }

            if seen_requested && seen_completed {
                break;
            }
        }

        let _ = result_tx.send((seen_requested, seen_completed, violation));
    });

    harness.event_bus.send(AppEvent::System(SystemEvent::ToolCallRequested {
        tool_call,
        request_id,
        parent_id,
    }));
    harness.event_bus.send(AppEvent::System(SystemEvent::ToolCallCompleted {
        request_id,
        parent_id,
        call_id: call_id.clone(),
        content: "{\"ok\":true}".to_string(),
        ui_payload: None,
    }));

    let (seen_requested, seen_completed, violation) = result_rx
        .await
        .expect("ordering observer dropped");
    assert!(seen_requested, "ToolCallRequested was not observed");
    assert!(seen_completed, "ToolCallCompleted was not observed");
    assert!(
        !violation,
        "ToolCallCompleted observed before ToolCallRequested (ordering violated)"
    );
}

fn handle_tool_event(
    event: &AppEvent,
    call_id: &ArcStr,
    request_id: Uuid,
    seen_requested: &mut bool,
    seen_completed: &mut bool,
    violation: &mut bool,
) {
    match event {
        AppEvent::System(SystemEvent::ToolCallRequested {
            request_id: evt_req_id,
            tool_call,
            ..
        }) if evt_req_id == &request_id && tool_call.call_id == *call_id => {
            if *seen_completed {
                *violation = true;
            }
            *seen_requested = true;
        }
        AppEvent::System(SystemEvent::ToolCallCompleted {
            request_id: evt_req_id,
            call_id: evt_call_id,
            ..
        }) if evt_req_id == &request_id && evt_call_id == call_id => {
            if !*seen_requested {
                *violation = true;
            }
            *seen_completed = true;
        }
        _ => {}
    }
}
