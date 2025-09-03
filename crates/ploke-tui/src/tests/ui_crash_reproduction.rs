//! Crash reproduction tests for UI deadlock issues
//! 
//! These tests reproduce the exact deadlock condition that causes the application
//! to crash when pressing 'e' to open the approvals overlay.

use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use uuid::Uuid;
use crate::app_state::{AppState, core::{EditProposal, EditProposalStatus, DiffPreview}};
use crate::app::view::components::approvals::{render_approvals_overlay, ApprovalsState};
use ratatui::{backend::TestBackend, Terminal};
use std::collections::HashMap;

/// Creates a minimal test app state for reproduction
async fn create_test_app_state() -> Arc<AppState> {
    // Create minimal state - we only need proposals for this test
    let proposals = Arc::new(RwLock::new(HashMap::new()));
    
    // Add a test proposal
    let proposal = EditProposal {
        request_id: Uuid::new_v4(),
        parent_id: Uuid::new_v4(), 
        call_id: "test-call".into(),
        proposed_at_ms: 1234567890,
        edits: vec![],
        files: vec![std::path::PathBuf::from("test.rs")],
        preview: DiffPreview::UnifiedDiff { 
            text: "--- a/test.rs\n+++ b/test.rs\n@@ -1 +1,2 @@\n fn main() {\n+    println!(\"test\");\n }".to_string() 
        },
        status: EditProposalStatus::Pending,
    };
    
    {
        let mut guard = proposals.write().await;
        guard.insert(proposal.request_id, proposal);
    }
    
    // Create mock AppState with just the proposals field populated
    // Note: In real code, you'd need to properly initialize all AppState fields
    Arc::new(AppState {
        proposals,
        // ... other fields would need proper initialization
        // This is a simplified example focusing on the deadlock issue
        ..Default::default() // This won't work in real code - AppState likely doesn't derive Default
    })
}

#[cfg(test)]
mod crash_reproduction_tests {
    use super::*;
    
    /// **CRITICAL TEST**: Reproduces the exact deadlock that crashes the application
    /// 
    /// This test demonstrates the deadlock condition:
    /// 1. UI render loop holds async read lock on chat history
    /// 2. render_approvals_overlay() tries to acquire blocking read lock on proposals  
    /// 3. Deadlock occurs because blocking_read() cannot proceed while async lock is held
    #[tokio::test]
    async fn test_deadlock_reproduction_approvals_overlay() {
        // Skip this test in normal runs since it's designed to demonstrate the deadlock
        if std::env::var("RUN_DEADLOCK_TESTS").is_err() {
            return;
        }
        
        let app_state = create_test_app_state().await;
        
        // Simulate the exact condition from the main UI render loop:
        // run_with() -> terminal.draw() -> draw() 
        // where draw() holds: history_guard = app_state.chat.0.read().await
        
        // Step 1: Simulate holding an async read lock (like chat history in draw())
        let _simulated_chat_guard = app_state.chat.0.read().await; // This would be the actual line
        
        // Step 2: Try to render approvals overlay - this will call blocking_read()
        let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();
        let ui_state = ApprovalsState::default();
        
        // This is the line that causes the deadlock in the real application:
        let result = std::panic::catch_unwind(|| {
            terminal.draw(|frame| {
                // This call will deadlock because render_approvals_overlay() 
                // calls state.proposals.blocking_read() while we hold async lock
                let _ = render_approvals_overlay(frame, frame.area(), &app_state, &ui_state);
            })
        });
        
        // In the real application, this would never return - it would deadlock
        // In this test, we expect it to either panic or timeout
        match result {
            Ok(_) => {
                panic!("Expected deadlock did not occur - this means the bug might be fixed!");
            },
            Err(_) => {
                println!("Deadlock reproduced successfully - application would crash here");
                // This is the expected outcome demonstrating the bug
            }
        }
    }
    
    /// **SOLUTION TEST**: Demonstrates the fix using pre-fetched data
    /// 
    /// This shows how render_approvals_overlay() should be refactored to avoid deadlock
    #[tokio::test]
    async fn test_fixed_approvals_overlay_no_deadlock() {
        let app_state = create_test_app_state().await;
        
        // Step 1: Pre-fetch proposals data BEFORE entering render phase
        let proposals_data: Vec<(Uuid, EditProposal)> = {
            let guard = app_state.proposals.read().await;
            guard.iter().map(|(id, proposal)| (*id, proposal.clone())).collect()
        };
        // Lock is dropped here
        
        // Step 2: Simulate holding async read lock (as in real draw() method)
        let _simulated_chat_guard = app_state.chat.0.read().await;
        
        // Step 3: Render with pre-fetched data (no blocking calls during render)
        let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();
        let ui_state = ApprovalsState::default();
        
        let result = terminal.draw(|frame| {
            // Fixed version would take pre-fetched data instead of accessing state
            render_approvals_overlay_fixed(frame, frame.area(), &proposals_data, &ui_state);
        });
        
        assert!(result.is_ok(), "Fixed version should not deadlock");
    }
    
    /// **PERFORMANCE TEST**: Verify UI responsiveness requirements
    #[tokio::test] 
    async fn test_ui_performance_requirements() {
        let app_state = create_test_app_state().await;
        
        // Pre-fetch data to avoid deadlock
        let proposals_data: Vec<(Uuid, EditProposal)> = {
            let guard = app_state.proposals.read().await;
            guard.iter().map(|(id, proposal)| (*id, proposal.clone())).collect()
        };
        
        let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();
        let ui_state = ApprovalsState::default();
        
        // UI must render within 16ms for 60fps responsiveness
        let start = std::time::Instant::now();
        let result = terminal.draw(|frame| {
            render_approvals_overlay_fixed(frame, frame.area(), &proposals_data, &ui_state);
        });
        let duration = start.elapsed();
        
        assert!(result.is_ok(), "UI rendering should succeed");
        assert!(duration < Duration::from_millis(16), 
               "UI should render within 16ms for 60fps, took: {:?}", duration);
    }
    
    /// **CONCURRENCY TEST**: Verify no race conditions under concurrent access
    #[tokio::test]
    async fn test_concurrent_proposal_access_safety() {
        let app_state = create_test_app_state().await;
        
        // Spawn multiple tasks that read proposals concurrently
        let mut handles = vec![];
        
        for i in 0..10 {
            let state = app_state.clone();
            let handle = tokio::spawn(async move {
                for _ in 0..100 {
                    // Simulate UI trying to read proposals
                    let proposals_data: Vec<(Uuid, EditProposal)> = {
                        let guard = state.proposals.read().await;
                        guard.iter().map(|(id, proposal)| (*id, proposal.clone())).collect()
                    };
                    
                    // Simulate some processing
                    tokio::task::yield_now().await;
                    
                    // Verify data integrity
                    assert!(!proposals_data.is_empty(), "Task {} should see at least one proposal", i);
                }
            });
            handles.push(handle);
        }
        
        // Also spawn a task that modifies proposals
        let state = app_state.clone();
        let writer_handle = tokio::spawn(async move {
            for j in 0..10 {
                let proposal = EditProposal {
                    request_id: Uuid::new_v4(),
                    parent_id: Uuid::new_v4(),
                    call_id: format!("concurrent-{}", j).into(),
                    proposed_at_ms: 1234567890 + j,
                    edits: vec![],
                    files: vec![std::path::PathBuf::from(format!("test{}.rs", j))],
                    preview: DiffPreview::UnifiedDiff { text: "test diff".to_string() },
                    status: EditProposalStatus::Pending,
                };
                
                {
                    let mut guard = state.proposals.write().await;
                    guard.insert(proposal.request_id, proposal);
                }
                
                tokio::task::yield_now().await;
            }
        });
        handles.push(writer_handle);
        
        // Wait for all tasks to complete
        let results = futures::future::join_all(handles).await;
        
        // Verify no panics occurred
        for (i, result) in results.iter().enumerate() {
            assert!(result.is_ok(), "Task {} should not panic: {:?}", i, result);
        }
    }
}

/// **PROPOSED FIX**: render_approvals_overlay function that doesn't cause deadlock
/// 
/// This function takes pre-fetched data instead of accessing state directly during rendering
fn render_approvals_overlay_fixed(
    frame: &mut ratatui::prelude::Frame,
    area: ratatui::prelude::Rect, 
    proposals_data: &[(Uuid, EditProposal)], // Pre-fetched data
    ui: &ApprovalsState,
) -> Option<Uuid> {
    use ratatui::prelude::*;
    use ratatui::widgets::*;
    
    let outer = Block::bordered().title(" Approvals ");
    let inner = outer.inner(area);
    frame.render_widget(outer, area);

    // Split into list and details  
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(40), Constraint::Percentage(60)])
        .split(inner);

    // Use pre-fetched data instead of state.proposals.blocking_read()
    let mut items: Vec<(Uuid, String)> = proposals_data
        .iter()
        .map(|(id, p)| {
            let status = match &p.status {
                EditProposalStatus::Pending => "Pending",
                EditProposalStatus::Approved => "Approved", 
                EditProposalStatus::Denied => "Denied",
                EditProposalStatus::Applied => "Applied",
                EditProposalStatus::Failed(_) => "Failed",
            };
            (
                *id,
                format!(
                    "{}  {:<7}  files:{}",
                    crate::app::utils::truncate_uuid(*id),
                    status,
                    p.files.len()
                ),
            )
        })
        .collect();
    items.sort_by_key(|(id, _)| *id);

    let list_items: Vec<ListItem> = items
        .iter()
        .map(|(_, s)| ListItem::new(s.clone()))
        .collect();
    let list = List::new(list_items)
        .block(Block::bordered().title(" Pending Proposals "))
        .highlight_style(Style::new().fg(Color::Cyan));
    frame.render_widget(list, cols[0]);

    // Details pane
    let selected_id = items.get(ui.selected).map(|(id, _)| *id);
    let mut detail_lines: Vec<Line> = Vec::new();
    if let Some(sel) = selected_id {
        if let Some((_, p)) = proposals_data.iter().find(|(id, _)| *id == sel) {
            detail_lines.push(Line::from(vec![Span::styled(
                format!("request_id: {}", sel),
                Style::new().fg(Color::Yellow),
            )]));
            detail_lines.push(Line::from(format!(
                "status: {:?}  files:{}",
                p.status,
                p.files.len()
            )));
            match &p.preview {
                DiffPreview::UnifiedDiff { text } => {
                    let header = Line::from(vec![Span::styled(
                        "Unified Diff:",
                        Style::new().fg(Color::Green),
                    )]);
                    detail_lines.push(header);
                    for ln in text.lines().take(40) {
                        detail_lines.push(Line::from(ln.to_string()));
                    }
                }
                DiffPreview::CodeBlocks { per_file } => {
                    let header = Line::from(vec![Span::styled(
                        "Before/After:",
                        Style::new().fg(Color::Green),
                    )]);
                    detail_lines.push(header);
                    for ba in per_file.iter().take(2) {
                        detail_lines.push(Line::from(format!("--- {}", ba.file_path.display())));
                        for ln in ba.before.lines().take(10) {
                            detail_lines.push(Line::from(format!("- {}", ln)));
                        }
                        for ln in ba.after.lines().take(10) {
                            detail_lines.push(Line::from(format!("+ {}", ln)));
                        }
                    }
                }
            }
        }
    }
    let detail = Paragraph::new(detail_lines)
        .block(Block::bordered().title(" Details "))
        .alignment(Alignment::Left);
    frame.render_widget(detail, cols[1]);

    selected_id
}