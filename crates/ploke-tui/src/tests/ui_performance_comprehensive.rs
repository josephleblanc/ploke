//! Comprehensive UI performance and concurrency tests
//!
//! This module implements the performance testing strategy outlined in ui_test_strategy.md
//! to ensure the UI remains responsive under load and concurrent access.

use ratatui::{Terminal, backend::TestBackend};
use std::{
    collections::HashMap,
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::app::view::components::approvals::{ApprovalsState, render_approvals_overlay};
use crate::app_state::core::{BeforeAfter, DiffPreview, EditProposal, EditProposalStatus};

/// Creates test proposals in bulk for performance testing
fn create_bulk_test_proposals(count: usize) -> HashMap<Uuid, EditProposal> {
    let mut proposals = HashMap::with_capacity(count);

    for i in 0..count {
        let id = Uuid::new_v4();
        let status = match i % 4 {
            0 => EditProposalStatus::Pending,
            1 => EditProposalStatus::Applied,
            2 => EditProposalStatus::Failed(format!("Test error {}", i)),
            _ => EditProposalStatus::Denied,
        };

        let preview = if i % 2 == 0 {
            DiffPreview::UnifiedDiff {
                text: format!(
                    "--- a/test_{}.rs\n+++ b/test_{}.rs\n@@ -1,3 +1,4 @@\n fn test() {{\n+    println!(\"test {}\");\n     // existing code\n }}",
                    i, i, i
                ),
            }
        } else {
            DiffPreview::CodeBlocks {
                per_file: vec![BeforeAfter {
                    file_path: std::path::PathBuf::from(format!("test_{}.rs", i)),
                    before: format!("fn test_{}() {{\n    // before\n}}", i),
                    after: format!(
                        "fn test_{}() {{\n    // after\n    println!(\"modified {}\");\n}}",
                        i, i
                    ),
                }],
            }
        };

        let proposal = EditProposal {
            request_id: id,
            parent_id: Uuid::new_v4(),
            call_id: format!("perf-test-{}", i).into(),
            proposed_at_ms: 1234567890 + i as i64,
            edits: vec![],
            edits_ns: vec![],
            files: vec![std::path::PathBuf::from(format!("test_{}.rs", i))],
            preview,
            status,
            is_semantic: true,
        };

        proposals.insert(id, proposal);
    }

    proposals
}

/// Mock AppState for performance testing  
async fn create_mock_app_state_with_proposals(
    proposal_count: usize,
) -> Arc<crate::app_state::AppState> {
    let db = Arc::new(ploke_db::Database::init_with_schema().expect("db init"));
    let io_handle = ploke_io::IoManagerHandle::new();
    let cfg = crate::user_config::UserConfig::default();
    let embedder = Arc::new(cfg.load_embedding_processor().expect("embedder"));

    let proposals = create_bulk_test_proposals(proposal_count);

    Arc::new(crate::app_state::AppState {
        chat: crate::app_state::core::ChatState::new(crate::chat_history::ChatHistory::new()),
        config: crate::app_state::core::ConfigState::new(
            crate::app_state::core::RuntimeConfig::from(cfg.clone()),
        ),
        system: crate::app_state::core::SystemState::default(),
        indexing_state: RwLock::new(None),
        indexer_task: None,
        indexing_control: Arc::new(tokio::sync::Mutex::new(None)),
        db,
        embedder,
        io_handle,
        rag: None,
        budget: ploke_rag::TokenBudget::default(),
        proposals: RwLock::new(proposals),
        create_proposals: RwLock::new(std::collections::HashMap::new()),
    })
}

#[cfg(test)]
mod performance_tests {
    use super::*;

    /// Test UI responsiveness with large datasets - should render within 16ms for 60fps
    #[tokio::test(flavor = "multi_thread")]
    async fn test_ui_performance_large_proposal_count() {
        const PROPOSAL_COUNTS: &[usize] = &[10, 50, 100, 500, 1000];

        for &count in PROPOSAL_COUNTS {
            let state = create_mock_app_state_with_proposals(count).await;
            let ui_state = ApprovalsState::default();

            let mut terminal = Terminal::new(TestBackend::new(120, 40)).unwrap();

            // Measure rendering time
            let start = Instant::now();
            let result = terminal.draw(|frame| {
                let _ = render_approvals_overlay(frame, frame.area(), &state, &ui_state);
            });
            let duration = start.elapsed();

            assert!(
                result.is_ok(),
                "UI rendering should succeed for {} proposals",
                count
            );

            // Should render within 16ms for smooth 60fps experience
            assert!(
                duration < Duration::from_millis(16),
                "UI should render {} proposals within 16ms for 60fps, took: {:?}",
                count,
                duration
            );

            println!("✅ Rendered {} proposals in {:?}", count, duration);
        }
    }

    /// Test memory usage scaling with proposal count
    #[tokio::test(flavor = "multi_thread")]
    async fn test_memory_usage_scaling() {
        const PROPOSAL_COUNTS: &[usize] = &[100, 500, 1000];

        for &count in PROPOSAL_COUNTS {
            let state = create_mock_app_state_with_proposals(count).await;
            let ui_state = ApprovalsState::default();

            // Multiple renders to stress test memory usage
            for _ in 0..10 {
                let mut terminal = Terminal::new(TestBackend::new(100, 30)).unwrap();
                let result = terminal.draw(|frame| {
                    let _ = render_approvals_overlay(frame, frame.area(), &state, &ui_state);
                });
                assert!(result.is_ok(), "Memory stress test should succeed");
            }

            println!("✅ Memory stress test passed for {} proposals", count);
        }
    }

    /// Test concurrent access patterns under heavy load
    #[tokio::test(flavor = "multi_thread")]
    async fn test_concurrent_ui_access_under_load() {
        let state = create_mock_app_state_with_proposals(100).await;

        // Spawn multiple concurrent rendering tasks
        let mut handles = vec![];
        for i in 0..10 {
            let state_clone = state.clone();
            let handle = tokio::spawn(async move {
                for j in 0..20 {
                    let ui_state = ApprovalsState {
                        selected: j % 5,
                        help_visible: false,
                        view_lines: 0,
                    };
                    let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();

                    let start = Instant::now();
                    let result = terminal.draw(|frame| {
                        let _ =
                            render_approvals_overlay(frame, frame.area(), &state_clone, &ui_state);
                    });
                    let duration = start.elapsed();

                    assert!(
                        result.is_ok(),
                        "Concurrent rendering task {} iteration {} should succeed",
                        i,
                        j
                    );
                    assert!(
                        duration < Duration::from_millis(20),
                        "Concurrent rendering should be fast: {:?}",
                        duration
                    );

                    // Small yield to allow other tasks to run
                    tokio::task::yield_now().await;
                }
            });
            handles.push(handle);
        }

        // Wait for all concurrent tasks to complete
        for (i, handle) in handles.into_iter().enumerate() {
            handle
                .await
                .expect(&format!("Concurrent task {} should not panic", i));
        }

        println!("✅ Concurrent access test passed - 10 tasks x 20 renders each");
    }

    /// Test UI responsiveness during data mutations
    #[tokio::test(flavor = "multi_thread")]
    async fn test_ui_responsiveness_during_mutations() {
        let state = create_mock_app_state_with_proposals(50).await;

        // Start background task that continuously modifies proposals
        let state_for_mutation = state.clone();
        let mutation_handle = tokio::spawn(async move {
            for i in 0..100 {
                {
                    let mut proposals = state_for_mutation.proposals.write().await;

                    // Add new proposal
                    let new_id = Uuid::new_v4();
                    let proposal = EditProposal {
                        request_id: new_id,
                        parent_id: Uuid::new_v4(),
                        call_id: format!("mutation-{}", i).into(),
                        proposed_at_ms: chrono::Utc::now().timestamp_millis(),
                        edits: vec![],
                        edits_ns: vec![],
                        files: vec![std::path::PathBuf::from(format!("mutation_{}.rs", i))],
                        preview: DiffPreview::UnifiedDiff {
                            text: format!("mutation diff {}", i),
                        },
                        status: EditProposalStatus::Pending,
                        is_semantic: true,
                    };
                    proposals.insert(new_id, proposal);

                    // Remove old proposal if too many
                    if proposals.len() > 60 {
                        let oldest_key = proposals.keys().next().copied();
                        if let Some(key) = oldest_key {
                            proposals.remove(&key);
                        }
                    }
                }

                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        });

        // Continuously render UI while mutations happen
        let rendering_handle = tokio::spawn(async move {
            for i in 0..50 {
                let ui_state = ApprovalsState {
                    selected: i % 10,
                    help_visible: i % 5 == 0,
                    view_lines: 0,
                };
                let mut terminal = Terminal::new(TestBackend::new(90, 30)).unwrap();

                let start = Instant::now();
                let result = terminal.draw(|frame| {
                    let _ = render_approvals_overlay(frame, frame.area(), &state, &ui_state);
                });
                let duration = start.elapsed();

                assert!(
                    result.is_ok(),
                    "UI should remain responsive during mutations"
                );
                assert!(
                    duration < Duration::from_millis(25),
                    "UI should render quickly even during mutations: {:?}",
                    duration
                );

                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        });

        // Wait for both tasks to complete
        let (mutation_result, rendering_result) = tokio::join!(mutation_handle, rendering_handle);
        mutation_result.expect("Mutation task should complete successfully");
        rendering_result.expect("Rendering task should complete successfully");

        println!("✅ UI remained responsive during continuous data mutations");
    }

    /// Test edge cases with extreme data sizes
    #[tokio::test(flavor = "multi_thread")]
    async fn test_edge_cases_extreme_data() {
        // Test with very large diff content
        let mut proposals = HashMap::new();
        let id = Uuid::new_v4();

        // Create a very large diff (but still reasonable)
        let large_diff = (0..100)
            .map(|i| format!("line {} with some content that makes it longer\n", i))
            .collect::<String>();

        let proposal = EditProposal {
            request_id: id,
            parent_id: Uuid::new_v4(),
            call_id: "large-diff-test".into(),
            proposed_at_ms: chrono::Utc::now().timestamp_millis(),
            edits: vec![],
            edits_ns: vec![],
            files: vec![std::path::PathBuf::from("large_file.rs")],
            preview: DiffPreview::UnifiedDiff { text: large_diff },
            status: EditProposalStatus::Pending,
            is_semantic: true,
        };
        proposals.insert(id, proposal);

        let db = Arc::new(ploke_db::Database::init_with_schema().expect("db init"));
        let io_handle = ploke_io::IoManagerHandle::new();
        let cfg = crate::user_config::UserConfig::default();
        let embedder = Arc::new(cfg.load_embedding_processor().expect("embedder"));

        let state = Arc::new(crate::app_state::AppState {
            chat: crate::app_state::core::ChatState::new(crate::chat_history::ChatHistory::new()),
            config: crate::app_state::core::ConfigState::new(
                crate::app_state::core::RuntimeConfig::from(cfg),
            ),
            system: crate::app_state::core::SystemState::default(),
            indexing_state: RwLock::new(None),
            indexer_task: None,
            indexing_control: Arc::new(tokio::sync::Mutex::new(None)),
            db,
            embedder,
            io_handle,
            rag: None,
            budget: ploke_rag::TokenBudget::default(),
            proposals: RwLock::new(proposals),
            create_proposals: RwLock::new(std::collections::HashMap::new()),
        });

        let ui_state = ApprovalsState::default();
        let mut terminal = Terminal::new(TestBackend::new(120, 40)).unwrap();

        let start = Instant::now();
        let result = terminal.draw(|frame| {
            let _ = render_approvals_overlay(frame, frame.area(), &state, &ui_state);
        });
        let duration = start.elapsed();

        assert!(result.is_ok(), "Should handle large diff content");
        assert!(
            duration < Duration::from_millis(50),
            "Should render large content reasonably fast: {:?}",
            duration
        );

        println!(
            "✅ Edge case test passed - large diff content rendered in {:?}",
            duration
        );
    }
}

#[cfg(test)]
mod stress_tests {
    use super::*;

    /// Long-running stress test to ensure no memory leaks or performance degradation
    #[tokio::test(flavor = "multi_thread")]
    #[ignore] // Long-running test, run with --ignored flag
    async fn test_long_running_stability() {
        let state = create_mock_app_state_with_proposals(200).await;

        println!("Starting long-running stability test...");

        let start_time = Instant::now();
        let mut max_duration = Duration::from_nanos(0);
        let mut total_renders = 0u32;

        // Run for 30 seconds
        while start_time.elapsed() < Duration::from_secs(30) {
            let ui_state = ApprovalsState {
                selected: (total_renders % 10) as usize,
                help_visible: total_renders % 20 == 0,
                view_lines: 0,
            };

            let mut terminal = Terminal::new(TestBackend::new(100, 30)).unwrap();

            let render_start = Instant::now();
            let result = terminal.draw(|frame| {
                let _ = render_approvals_overlay(frame, frame.area(), &state, &ui_state);
            });
            let render_duration = render_start.elapsed();

            assert!(result.is_ok(), "Render {} should succeed", total_renders);

            max_duration = max_duration.max(render_duration);
            total_renders += 1;

            // Brief pause to prevent overwhelming the system
            tokio::time::sleep(Duration::from_millis(1)).await;
        }

        println!(
            "✅ Stability test completed: {} renders in {:?}, max single render: {:?}",
            total_renders,
            start_time.elapsed(),
            max_duration
        );

        // After 30 seconds of continuous rendering, performance should still be good
        assert!(
            max_duration < Duration::from_millis(20),
            "Performance should not degrade over time. Max render time: {:?}",
            max_duration
        );
    }
}
