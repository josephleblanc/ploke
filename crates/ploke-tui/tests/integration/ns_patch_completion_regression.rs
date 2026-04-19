use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use ploke_core::ArcStr;
use ploke_core::rag_types::ApplyCodeEditResult;
use ploke_core::tool_types::{FunctionMarker, ToolName};
use ploke_llm::response::{FunctionCall, ToolCall};
use ploke_tui::app_state::events::SystemEvent;
use ploke_tui::test_utils::new_test_harness::AppHarness;
use ploke_tui::{AppEvent, EventPriority};
use tempfile::tempdir;
use tokio::time::{Instant, timeout};
use uuid::Uuid;

const RIPGREP_NS_PATCH_DIFF: &str = r#"diff --git a/crates/printer/src/util.rs b/crates/printer/src/util.rs
index 1234567..89abcde 100644
--- a/crates/printer/src/util.rs
+++ b/crates/printer/src/util.rs
@@ -78,14 +78,24 @@ impl<M: Matcher> Replacer<M> {
             dst.clear();
             matches.clear();

-            matcher
-                .replace_with_captures_at(
-                    subject,
-                    range.start,
-                    caps,
-                    dst,
-                    |caps, dst| {
-                        let start = dst.len();
+            let range_end = range.end;
+            matcher
+                .replace_with_captures_at(
+                    subject,
+                    range.start,
+                    caps,
+                    dst,
+                    |caps, dst| {
+                        // This strange dance is to account for the possibility of 
+                        // look-ahead in the regex, just like in find_iter_at_in_context.
+                        // We need to make sure we don't include matches that extend
+                        // beyond our original range, even if they were found due to 
+                        // our kludge of extending the search area.
+                        let match_start = caps.get(0).map_or(0, |m| m.start());
+                        if match_start >= range_end {
+                            return false;
+                        }
+                        
+                        let start = dst.len();
                         caps.interpolate(
                             |name| matcher.capture_index(name),
                             subject,
@@ -96,7 +106,7 @@ impl<M: Matcher> Replacer<M> {
                         matches.push(Match::new(start, end));
                         true
                     },
-                )
+                )
                 .map_err(io::Error::error_message)?;
         }
        Ok(())
"#;

const SIMPLE_NS_PATCH_DIFF: &str = r#"--- a/notes.txt
+++ b/notes.txt
@@ -1,3 +1,3 @@
 alpha
-beta
+delta
 gamma
"#;

const TODO_NS_PATCH_DIFF: &str = r#"--- a/todo.txt
+++ b/todo.txt
@@ -1,3 +1,3 @@
 one
-two
+done
 three
"#;

async fn configure_temp_workspace(harness: &AppHarness, workspace_root: &Path) {
    let workspace_root = workspace_root.to_path_buf();
    let _ = harness
        .state
        .with_system_txn(|txn| {
            txn.set_loaded_workspace(
                workspace_root.clone(),
                vec![workspace_root.clone()],
                Some(workspace_root.clone()),
            );
            txn.set_pwd(workspace_root.clone());
        })
        .await;

    let policy = harness
        .state
        .with_system_read(|sys| sys.derive_path_policy(&[]).expect("path policy after load"))
        .await;

    harness
        .state
        .io_handle
        .update_roots(Some(policy.roots.clone()), Some(policy.symlink_policy))
        .await;
}

fn write_ripgrep_fixture(workspace_root: &Path) -> PathBuf {
    let file_path = workspace_root.join("crates/printer/src/util.rs");
    fs::create_dir_all(file_path.parent().expect("fixture parent")).expect("create fixture dirs");

    let mut lines: Vec<String> = (1..=77).map(|i| format!("// filler line {i}")).collect();
    lines.extend(
        [
            "            dst.clear();",
            "            matches.clear();",
            "",
            "            matcher",
            "                .replace_with_captures_at(",
            "                    subject,",
            "                    range.start,",
            "                    caps,",
            "                    dst,",
            "                    |caps, dst| {",
            "                        let start = dst.len();",
            "                        caps.interpolate(",
            "                            |name| matcher.capture_index(name),",
            "                            subject,",
            "                            replacement,",
            "                            caps,",
            "                            dst,",
            "                        let end = dst.len();",
            "                        matches.push(Match::new(start, end));",
            "                        true",
            "                    },",
            "                )",
            "                .map_err(io::Error::error_message)?;",
            "        }",
            "        Ok(())",
            "    }",
            "}",
        ]
        .into_iter()
        .map(str::to_string),
    );
    fs::write(&file_path, lines.join("\n") + "\n").expect("write ripgrep fixture");
    file_path
}

fn write_simple_fixture(workspace_root: &Path) -> PathBuf {
    write_named_fixture(workspace_root, "notes.txt", "alpha\nbeta\ngamma\n")
}

fn write_named_fixture(workspace_root: &Path, relative_path: &str, contents: &str) -> PathBuf {
    let file_path = workspace_root.join(relative_path);
    if let Some(parent) = file_path.parent() {
        fs::create_dir_all(parent).expect("create fixture parent");
    }
    fs::write(&file_path, contents).expect("write named fixture");
    file_path
}

#[tokio::test(flavor = "multi_thread")]
async fn ns_patch_stages_multiple_files_in_one_request() {
    let harness = AppHarness::spawn().await.expect("spawn harness");
    let temp_dir = tempdir().expect("temp workspace");
    let workspace_root = temp_dir.path().join("ripgrep");
    let _notes_path = write_simple_fixture(&workspace_root);
    let _todo_path = write_named_fixture(&workspace_root, "todo.txt", "one\ntwo\nthree\n");
    configure_temp_workspace(&harness, &workspace_root).await;

    let request_id = Uuid::new_v4();
    let parent_id = Uuid::new_v4();
    let call_id = ArcStr::from("ns-patch-multi-file");
    let tool_call = ToolCall {
        call_id: call_id.clone(),
        call_type: FunctionMarker,
        function: FunctionCall {
            name: ToolName::NsPatch,
            arguments: serde_json::json!({
                "patches": [
                    {
                        "file": "notes.txt",
                        "diff": SIMPLE_NS_PATCH_DIFF,
                        "reasoning": "Batch coverage for notes.txt",
                    },
                    {
                        "file": "todo.txt",
                        "diff": TODO_NS_PATCH_DIFF,
                        "reasoning": "Batch coverage for todo.txt",
                    }
                ]
            })
            .to_string(),
        },
    };

    let mut event_rx = harness.event_bus.subscribe(EventPriority::Realtime);
    harness
        .event_bus
        .send(AppEvent::System(SystemEvent::ToolCallRequested {
            tool_call,
            request_id,
            parent_id,
        }));

    let mut completed_event: Option<ApplyCodeEditResult> = None;
    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline {
        match timeout(Duration::from_millis(100), event_rx.recv()).await {
            Ok(Ok(AppEvent::System(SystemEvent::ToolCallCompleted {
                request_id: event_request_id,
                call_id: event_call_id,
                content,
                ..
            }))) if event_request_id == request_id && event_call_id == call_id => {
                let parsed_event: ApplyCodeEditResult =
                    serde_json::from_str(&content).expect("parse ToolCallCompleted payload");
                completed_event = Some(parsed_event);
                break;
            }
            Ok(Ok(AppEvent::System(SystemEvent::ToolCallFailed {
                request_id: event_request_id,
                call_id: event_call_id,
                error,
                ..
            }))) if event_request_id == request_id && event_call_id == call_id => {
                panic!("ns_patch unexpectedly failed: {error}");
            }
            Ok(Ok(_)) => {}
            Ok(Err(_)) | Err(_) => {}
        }
    }

    let completed_event = completed_event.expect("expected ToolCallCompleted event for ns_patch");
    assert!(completed_event.ok, "completion event should report success");
    assert_eq!(completed_event.staged, 2, "should stage both patch entries");
    assert_eq!(
        completed_event.files,
        vec!["notes.txt".to_string(), "todo.txt".to_string()],
        "completion event should preserve requested relative paths"
    );

    let proposal = harness
        .state
        .proposals
        .read()
        .await
        .get(&request_id)
        .cloned()
        .expect("ns_patch should stage a proposal");
    assert_eq!(
        proposal.edits_ns.len(),
        2,
        "proposal should store both ns edits"
    );
    assert_eq!(
        proposal.files,
        vec![
            workspace_root.join("notes.txt"),
            workspace_root.join("todo.txt")
        ],
        "proposal should resolve both file paths inside the temp workspace"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn ns_patch_emits_completed_event_for_exact_ripgrep_diff() {
    let harness = AppHarness::spawn().await.expect("spawn harness");
    let temp_dir = tempdir().expect("temp workspace");
    let workspace_root = temp_dir.path().join("ripgrep");
    let _fixture_path = write_ripgrep_fixture(&workspace_root);
    configure_temp_workspace(&harness, &workspace_root).await;

    let request_id = Uuid::new_v4();
    let parent_id = Uuid::new_v4();
    let call_id = ArcStr::from("ns-patch-ripgrep-regression");
    let tool_call = ToolCall {
        call_id: call_id.clone(),
        call_type: FunctionMarker,
        function: FunctionCall {
            name: ToolName::NsPatch,
            arguments: serde_json::json!({
            "patches": [{
                "file": "crates/printer/src/util.rs",
                "diff": RIPGREP_NS_PATCH_DIFF,
                "reasoning": "Regression coverage for the ripgrep eval diff",
            }]
                })
            .to_string(),
        },
    };

    let mut event_rx = harness.event_bus.subscribe(EventPriority::Realtime);
    harness
        .event_bus
        .send(AppEvent::System(SystemEvent::ToolCallRequested {
            tool_call,
            request_id,
            parent_id,
        }));

    let mut completed_event: Option<ApplyCodeEditResult> = None;
    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline {
        match timeout(Duration::from_millis(100), event_rx.recv()).await {
            Ok(Ok(AppEvent::System(SystemEvent::ToolCallCompleted {
                request_id: event_request_id,
                call_id: event_call_id,
                content,
                ..
            }))) if event_request_id == request_id && event_call_id == call_id => {
                let parsed_event: ApplyCodeEditResult =
                    serde_json::from_str(&content).expect("parse ToolCallCompleted payload");
                completed_event = Some(parsed_event);
                break;
            }
            Ok(Ok(AppEvent::System(SystemEvent::ToolCallFailed {
                request_id: event_request_id,
                call_id: event_call_id,
                error,
                ..
            }))) if event_request_id == request_id && event_call_id == call_id => {
                panic!("ns_patch unexpectedly failed: {error}");
            }
            Ok(Ok(_)) => {}
            Ok(Err(_)) | Err(_) => {}
        }
    }

    let completed_event = completed_event.expect("expected ToolCallCompleted event for ns_patch");
    assert!(completed_event.ok, "completion event should report success");
    assert_eq!(
        completed_event.staged, 1,
        "completion event should report one staged edit"
    );
    assert_eq!(
        completed_event.applied, 0,
        "completion event should stage the proposal without applying it"
    );
    assert_eq!(
        completed_event.files,
        vec!["crates/printer/src/util.rs".to_string()],
        "completion event should preserve the ripgrep relative file path"
    );

    let proposal = harness
        .state
        .proposals
        .read()
        .await
        .get(&request_id)
        .cloned()
        .expect("ns_patch should stage a proposal");
    assert_eq!(
        proposal.edits_ns.len(),
        1,
        "proposal should store one ns edit"
    );
    assert_eq!(
        proposal.files,
        vec![workspace_root.join("crates/printer/src/util.rs")],
        "proposal should resolve the ripgrep file path inside the temp workspace"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn ns_patch_auto_confirm_applies_staged_patch() {
    let harness = AppHarness::spawn().await.expect("spawn harness");
    {
        let mut config = harness.state.config.write().await;
        config.editing.auto_confirm_edits = true;
    }

    let temp_dir = tempdir().expect("temp workspace");
    let workspace_root = temp_dir.path().join("ripgrep");
    let fixture_path = write_simple_fixture(&workspace_root);
    configure_temp_workspace(&harness, &workspace_root).await;

    let request_id = Uuid::new_v4();
    let parent_id = Uuid::new_v4();
    let call_id = ArcStr::from("ns-patch-auto-confirm");
    let tool_call = ToolCall {
        call_id: call_id.clone(),
        call_type: FunctionMarker,
        function: FunctionCall {
            name: ToolName::NsPatch,
            arguments: serde_json::json!({
            "patches": [{
                "file": "notes.txt",
                "diff": SIMPLE_NS_PATCH_DIFF,
                "reasoning": "Regression coverage for auto-confirmed ns_patch",
            }]
                })
            .to_string(),
        },
    };

    let mut event_rx = harness.event_bus.subscribe(EventPriority::Realtime);
    harness
        .event_bus
        .send(AppEvent::System(SystemEvent::ToolCallRequested {
            tool_call,
            request_id,
            parent_id,
        }));

    let deadline = Instant::now() + Duration::from_secs(5);
    let mut saw_completed = false;
    while Instant::now() < deadline {
        match timeout(Duration::from_millis(100), event_rx.recv()).await {
            Ok(Ok(AppEvent::System(SystemEvent::ToolCallCompleted {
                request_id: event_request_id,
                call_id: event_call_id,
                ..
            }))) if event_request_id == request_id && event_call_id == call_id => {
                saw_completed = true;
                break;
            }
            Ok(Ok(AppEvent::System(SystemEvent::ToolCallFailed {
                request_id: event_request_id,
                call_id: event_call_id,
                error,
                ..
            }))) if event_request_id == request_id && event_call_id == call_id => {
                panic!("ns_patch unexpectedly failed: {error}");
            }
            Ok(Ok(_)) => {}
            Ok(Err(_)) | Err(_) => {}
        }
    }
    assert!(
        saw_completed,
        "expected ToolCallCompleted event for ns_patch"
    );

    let apply_deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let contents = fs::read_to_string(&fixture_path).expect("read patched fixture");
        if contents.contains("delta") {
            break;
        }
        if Instant::now() < apply_deadline {
            tokio::time::sleep(Duration::from_millis(50)).await;
        } else {
            let proposal = harness
                .state
                .proposals
                .read()
                .await
                .get(&request_id)
                .cloned();
            panic!(
                "expected auto-confirmed ns_patch to modify file; final proposal state: {proposal:?}"
            );
        }
    }

    let contents = fs::read_to_string(&fixture_path).expect("read patched fixture");
    assert!(
        contents.contains("delta"),
        "auto-confirmed ns_patch should apply the diff to disk"
    );
    assert!(
        !contents.contains("beta"),
        "patched file should no longer contain the original line"
    );
}
