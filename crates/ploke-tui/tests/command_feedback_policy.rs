use std::collections::HashSet;
use std::time::{Duration, Instant};

use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
use ploke_tui::chat_history::MessageKind;
use ploke_tui::test_utils::new_test_harness::AppHarness;
use uuid::Uuid;

fn key(code: KeyCode) -> Event {
    Event::Key(KeyEvent::new(code, KeyModifiers::NONE))
}

async fn send_slash_command(harness: &AppHarness, command: &str) {
    harness
        .input_tx
        .send(Ok(key(KeyCode::Char('/'))))
        .expect("slash prefix");
    for ch in command.chars() {
        harness
            .input_tx
            .send(Ok(key(KeyCode::Char(ch))))
            .expect("send command char");
    }
    harness
        .input_tx
        .send(Ok(key(KeyCode::Enter)))
        .expect("submit command");
}

async fn snapshot_message_ids(harness: &AppHarness) -> HashSet<Uuid> {
    let guard = harness.state.chat.0.read().await;
    guard.messages.keys().copied().collect()
}

async fn wait_for_new_sysinfo_matching<F>(
    harness: &AppHarness,
    before_ids: &HashSet<Uuid>,
    timeout: Duration,
    predicate: F,
) -> bool
where
    F: Fn(&str) -> bool,
{
    let start = Instant::now();
    while start.elapsed() < timeout {
        {
            let guard = harness.state.chat.0.read().await;
            if guard.messages.values().any(|m| {
                !before_ids.contains(&m.id)
                    && m.kind == MessageKind::SysInfo
                    && predicate(m.content.as_str())
            }) {
                return true;
            }
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    false
}

#[tokio::test]
async fn non_io_commands_emit_user_feedback_within_500ms() {
    let harness = AppHarness::spawn().await.expect("spawn harness");

    let cases = [
        ("verbosity profile verbose", "conversation verbosity profile set to verbose"),
        (
            "provider tools-only on",
            "provider tools-only enforcement enabled",
        ),
        ("index pause", "indexing pause requested"),
        ("index resume", "indexing resume requested"),
        ("index cancel", "indexing cancel requested"),
    ];

    for (command, expected_substring) in cases {
        let before = snapshot_message_ids(&harness).await;
        send_slash_command(&harness, command).await;
        let expected = expected_substring.to_ascii_lowercase();
        let seen = wait_for_new_sysinfo_matching(&harness, &before, Duration::from_millis(500), |s| {
            s.to_ascii_lowercase().contains(expected.as_str())
        })
        .await;
        assert!(
            seen,
            "expected feedback for command '{}' within 500ms",
            command
        );
    }

    harness.shutdown().await;
}

#[tokio::test]
async fn io_commands_emit_user_feedback_within_500ms() {
    let harness = AppHarness::spawn().await.expect("spawn harness");

    // File I/O command: missing path should produce a fast failure response.
    let before_file = snapshot_message_ids(&harness).await;
    send_slash_command(
        &harness,
        "model load /definitely/missing/path/for-feedback-policy.toml",
    )
    .await;
    let file_seen =
        wait_for_new_sysinfo_matching(&harness, &before_file, Duration::from_millis(500), |s| {
            let lower = s.to_ascii_lowercase();
            lower.contains("loading configuration")
                || lower.contains("failed to load configuration")
                || lower.contains("no such file")
        })
        .await;
    assert!(file_seen, "expected file I/O feedback within 500ms");

    // Network-capable command path; response should still be immediate in local validation paths.
    let before_net = snapshot_message_ids(&harness).await;
    send_slash_command(&harness, "model providers invalid_model_id").await;
    let net_seen =
        wait_for_new_sysinfo_matching(&harness, &before_net, Duration::from_millis(500), |s| {
            let lower = s.to_ascii_lowercase();
            lower.contains("missing openrouter_api_key")
                || lower.contains("invalid model id")
                || lower.contains("failed to fetch endpoints")
        })
        .await;
    assert!(net_seen, "expected network-path feedback within 500ms");

    harness.shutdown().await;
}
