//! Test the new truncation controls for approvals overlay

use std::{collections::HashMap, sync::Arc};
use tokio::sync::RwLock;
use uuid::Uuid;
use ratatui::{backend::TestBackend, Terminal};

use crate::app_state::core::{EditProposal, EditProposalStatus, DiffPreview, BeforeAfter};
use crate::app::view::components::approvals::{render_approvals_overlay, ApprovalsState};

/// Test basic truncation functionality without panics
#[tokio::test(flavor = "multi_thread")]
async fn test_truncation_controls_no_panic() {
    // Create a mock app state with a proposal
    let mut proposals = HashMap::new();
    let id = Uuid::new_v4();
    
    let proposal = EditProposal {
        request_id: id,
        parent_id: Uuid::new_v4(), 
        call_id: "test-truncation".into(),
        proposed_at_ms: chrono::Utc::now().timestamp_millis(),
        edits: vec![],
        files: vec![std::path::PathBuf::from("test.rs")],
        preview: DiffPreview::UnifiedDiff {
            text: "Line 1\nLine 2\nLine 3\nLine 4\nLine 5".to_string(),
        },
        status: EditProposalStatus::Pending,
    };
    proposals.insert(id, proposal);

    let db = Arc::new(ploke_db::Database::init_with_schema().expect("db init"));
    let io_handle = ploke_io::IoManagerHandle::new();
    let cfg = crate::user_config::UserConfig::default();
    let embedder = Arc::new(cfg.load_embedding_processor().expect("embedder"));

    let state = Arc::new(crate::app_state::AppState {
        chat: crate::app_state::core::ChatState::new(crate::chat_history::ChatHistory::new()),
        config: crate::app_state::core::ConfigState::new(crate::app_state::core::RuntimeConfig::from(cfg)),
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
    });

    // Test different truncation settings
    let test_cases = vec![
        0,  // unlimited
        1,  // very limited 
        10, // moderate
        100 // high
    ];

    for view_lines in test_cases {
        let ui_state = ApprovalsState { 
            selected: 0, 
            help_visible: false, 
            view_lines 
        };

        let mut terminal = Terminal::new(TestBackend::new(120, 40)).unwrap();

        // This should not panic
        let result = terminal.draw(|frame| {
            let _ = render_approvals_overlay(frame, frame.area(), &state, &ui_state);
        });

        assert!(result.is_ok(), "Rendering should succeed with view_lines={}", view_lines);
    }
}

/// Test help display with different truncation settings
#[tokio::test(flavor = "multi_thread")] 
async fn test_help_display_truncation() {
    let proposals = HashMap::new();
    
    let db = Arc::new(ploke_db::Database::init_with_schema().expect("db init"));
    let io_handle = ploke_io::IoManagerHandle::new();
    let cfg = crate::user_config::UserConfig::default();
    let embedder = Arc::new(cfg.load_embedding_processor().expect("embedder"));

    let state = Arc::new(crate::app_state::AppState {
        chat: crate::app_state::core::ChatState::new(crate::chat_history::ChatHistory::new()),
        config: crate::app_state::core::ConfigState::new(crate::app_state::core::RuntimeConfig::from(cfg)),
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
    });

    // Test help visible with different settings
    for view_lines in [0, 20, 100] {
        let ui_state = ApprovalsState { 
            selected: 0, 
            help_visible: true, 
            view_lines 
        };

        let mut terminal = Terminal::new(TestBackend::new(120, 40)).unwrap();

        let result = terminal.draw(|frame| {
            let _ = render_approvals_overlay(frame, frame.area(), &state, &ui_state);
        });

        assert!(result.is_ok(), "Help display should succeed with view_lines={}", view_lines);
    }
}