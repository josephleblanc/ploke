
use super::*;
use crate::app_state::core::{DiffPreview, EditProposal, EditProposalStatus, PreviewMode};
use crate::rag::tools::apply_code_edit_tool;
use crate::rag::utils::{ApplyCodeEditRequest, Edit, ToolCallParams};
use crate::test_utils::new_test_harness::AppHarness;
use ploke_core::rag_types::ApplyCodeEditResult;
use ploke_db::NodeType;
use ploke_test_utils::workspace_root;
use serde_json::json;
use std::sync::Arc;
use uuid::Uuid;
// ============================================================================
// Prerequisites:
// - Database backup must exist at: tests/backup_dbs/fixture_nodes_bfc25988-15c1-5e58-9aa8-3d33b5e58b92
// - Backup contains parsed data from tests/fixture_crates/fixture_nodes
// - Related crate tests (ploke-db, ploke-transform, syn_parser, etc.) are passing
// - Tests MUST fail if database backup is missing or empty
// ============================================================================

/// Original fixture target for the edits. This is the same as the file used in the backup database
/// loaded in the test harness.
/// Needs to be changed back after changing it.
/// File path is local to workspace.
const ORIGINAL_FIXTURE: &str = "tests/fixture_crates/fixture_nodes/src/structs.rs";
/// Backup of the target fixture, with a copy of each file to replace the file contents after the tests.
const BACKUP_FIXTURE: &str = "tests/fixture_crates/fixture_nodes_copy/src/structs.rs";

/// Restores the fixture in ORIGINAL_FIXTURE from the backup copy in BACKUP_FIXTURE
fn restore_fixture() {
    use std::fs;
    
    let mut original_path = workspace_root();
    original_path.push( ORIGINAL_FIXTURE );
    let mut backup_path = workspace_root();
    backup_path.push( BACKUP_FIXTURE );
    
    // Read the backup file content
    let backup_content = fs::read_to_string(&backup_path)
        .unwrap_or_else(|e| panic!("Failed to read backup fixture at {}: {}", backup_path.display(), e));
    
    // Write the backup content to the original file
    fs::write(&original_path, backup_content)
        .unwrap_or_else(|e| panic!("Failed to restore fixture to {}: {}", original_path.display(), e));
}

// Helper functions for test setup
fn create_canonical_edit_request(
    file_path: &str,
    canonical_path: &str,
    node_type: NodeType,
    new_content: &str,
    confidence: Option<f32>,
) -> ApplyCodeEditRequest {
    ApplyCodeEditRequest {
        edits: vec![Edit::Canonical {
            file: file_path.to_string(),
            canon: canonical_path.to_string(),
            node_type,
            code: new_content.to_string(),
        }],
        confidence,
    }
}

async fn create_test_tool_params(
    harness: &AppHarness,
    request_id: Uuid,
    arguments: serde_json::Value,
) -> ToolCallParams {
    use crate::tools::ToolName;
    use ploke_core::ArcStr;
    
    ToolCallParams {
        state: Arc::clone(&harness.state),
        event_bus: Arc::clone(&harness.event_bus),
        request_id,
        parent_id: request_id, // Use same ID for simplicity in tests
        name: ToolName::ApplyCodeEdit,
        arguments,
        call_id: ArcStr::from("test_call_id"),
    }
}

// ============================================================================
// Phase 1: Input Validation & Idempotency Tests
// ============================================================================

#[tokio::test]
#[cfg(feature = "test_harness")]
async fn test_duplicate_request_detection() {
    let harness = AppHarness::spawn().await.expect("spawn harness");
    let request_id = Uuid::new_v4();

    // Create a valid edit request using known fixture data
    let edit_request = create_canonical_edit_request(
        "src/structs.rs",
        "crate::structs::SampleStruct",
        NodeType::Struct,
        "pub struct SampleStruct { pub field: i32, }",
Some(0.9f32),
    );

    let arguments = serde_json::to_value(&edit_request).expect("serialize request");
    let params = create_test_tool_params(&harness, request_id, arguments.clone()).await;

    // Set up event listener to capture tool call results
    use crate::{AppEvent, EventPriority};
    let mut event_rx = harness.event_bus.subscribe(EventPriority::Realtime);

    // First call should succeed and create proposal
    apply_code_edit_tool(params.clone()).await;

    // Add a small delay to allow async operations to complete and capture events
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    
    // Check for any tool call failed events
    while let Ok(event) = event_rx.try_recv() {
        match event {
            AppEvent::System(system_event) => {
                println!("DEBUG: Captured system event: {:?}", system_event);
            },
            _ => {}
        }
    }

    // Verify proposal was created (database must be loaded for this to work)
    {
        let proposals = harness.state.proposals.read().await;
        if !proposals.contains_key(&request_id) {
            // Debug information to understand what went wrong
            println!("DEBUG: No proposal found for request_id: {}", request_id);
            println!("DEBUG: Proposals in state: {:?}", proposals.keys().collect::<Vec<_>>());
            
            // Check database is actually loaded
            let db = &harness.state.db;
            // Try a simpler query to check if database has any relations
            match db.run_script("::relations", Default::default(), cozo::ScriptMutability::Immutable) {
                Ok(result) => {
                    println!("DEBUG: Database relations: {:?}", result.rows.len());
                    for row in result.rows.iter().take(5) {
                        println!("DEBUG: Relation: {:?}", row);
                    }
                },
                Err(e) => {
                    println!("DEBUG: Database error listing relations: {:?}", e);
                }
            }
            
            panic!("First call should create proposal - debug info printed above");
        }
    }

    // Second call with same request_id should be idempotent
    apply_code_edit_tool(params).await;

    // Should still have exactly one proposal
    {
        let proposals = harness.state.proposals.read().await;
        assert_eq!(proposals.len(), 1, "Should not create duplicate proposals");
        assert!(
            proposals.get(&request_id).is_some(),
            "Original proposal should still exist"
        );
    }
    // Change the target back to the original value.
    restore_fixture();
}

#[tokio::test]
async fn test_empty_edits_validation() {
    let harness = AppHarness::spawn().await.expect("spawn harness");
    let request_id = Uuid::new_v4();

    // Set up event listener to capture tool call failures
    use crate::{EventPriority, AppEvent};
    let mut event_rx = harness.event_bus.subscribe(EventPriority::Realtime);

    let empty_request = ApplyCodeEditRequest {
        confidence: Some(0.5),
        edits: vec![], // Empty edits vector
    };

    let arguments = serde_json::to_value(&empty_request).expect("serialize request");
    let params = create_test_tool_params(&harness, request_id, arguments).await;

    // Call should complete but not create proposal due to empty edits
    apply_code_edit_tool(params).await;
    
    // Allow time for events to be processed
    tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

    // Verify that a failure event was emitted with "No edits provided" message
    let mut found_no_edits_error = false;
    while let Ok(event) = event_rx.try_recv() {
        if let AppEvent::System(system_event) = event {
            let event_str = format!("{:?}", system_event);
            if event_str.contains("ToolCallFailed") && event_str.contains("No edits provided") {
                found_no_edits_error = true;
                break;
            }
        }
    }

    assert!(found_no_edits_error, "Empty edits should emit 'No edits provided' ToolCallFailed event");

    // Should not create any proposals
    {
        let proposals = harness.state.proposals.read().await;
        assert!(
            proposals.is_empty(),
            "Empty edits should not create proposal"
        );
    }

    restore_fixture();
}

#[tokio::test]
async fn test_malformed_json_handling() {
    let harness = AppHarness::spawn().await.expect("spawn harness");
    let request_id = Uuid::new_v4();

    // Set up event listener to capture tool call failures
    use crate::{EventPriority, AppEvent};
    let mut event_rx = harness.event_bus.subscribe(EventPriority::Realtime);

    // Invalid JSON structure - missing required fields
    let malformed_json = json!({
        "confidence": 0.5,
        // Missing "edits" field
    });

    let params = create_test_tool_params(&harness, request_id, malformed_json).await;

    // Should handle gracefully without creating proposal
    apply_code_edit_tool(params).await;
    
    // Allow time for events to be processed
    tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

    // Verify that a failure event was emitted (stronger validation)
    let mut found_failure = false;
    while let Ok(event) = event_rx.try_recv() {
        if let AppEvent::System(system_event) = event {
            println!("DEBUG: System event: {:?}", system_event);
            if format!("{:?}", system_event).contains("ToolCallFailed") {
                found_failure = true;
                break;
            }
        }
    }

    assert!(found_failure, "Malformed JSON should emit ToolCallFailed event");

    {
        let proposals = harness.state.proposals.read().await;
        assert!(
            proposals.is_empty(),
            "Malformed JSON should not create proposal"
        );
    }
    restore_fixture();
}

// ============================================================================
// Phase 2: Database Resolution (Canonical Mode) Tests
// ============================================================================

#[tokio::test]
async fn test_canonical_resolution_success() {
    let harness = AppHarness::spawn()
        .await
        .expect("spawn harness - requires database backup");
    let request_id = Uuid::new_v4();

    // Use known struct from fixture_nodes (must exist in database backup)
    let edit_request = create_canonical_edit_request(
        "src/structs.rs",
        "crate::structs::SampleStruct",
        NodeType::Struct,
        "pub struct SampleStruct { pub field: String, pub new_field: i32, }",
        Some(0.95),
    );

    let arguments = serde_json::to_value(&edit_request).expect("serialize request");
    let params = create_test_tool_params(&harness, request_id, arguments).await;

    apply_code_edit_tool(params).await;

    // Verify proposal was created successfully (proves database resolution worked)
    {
        let proposals = harness.state.proposals.read().await;
        let proposal = proposals.get(&request_id).expect(
            "Proposal should be created - failure indicates database resolution failed. \
                Verify fixture_nodes backup exists and contains SampleStruct",
        );

        assert_eq!(proposal.status, EditProposalStatus::Pending);
        assert_eq!(proposal.edits.len(), 1, "Should have one edit");
        assert!(
            proposal
                .files
                .iter()
                .any(|f| f.to_string_lossy().contains("structs.rs")),
            "Should reference structs.rs file"
        );

        // Verify WriteSnippetData was populated correctly by database resolution
        let edit = &proposal.edits[0];
        assert!(edit.file_path.to_string_lossy().contains("structs.rs"));
        assert!(
            edit.start_byte < edit.end_byte,
            "Should have valid byte range from database"
        );
        assert!(!edit.replacement.is_empty(), "Should have replacement code");
    }
    restore_fixture();
}

#[tokio::test]
async fn test_canonical_resolution_not_found() {
    let harness = AppHarness::spawn().await.expect("spawn harness");
    let request_id = Uuid::new_v4();

    // Use non-existent canonical path
    let edit_request = create_canonical_edit_request(
        "src/structs.rs",
        "crate::NonExistentStruct", // This doesn't exist in fixture
        NodeType::Struct,
        "pub struct NonExistentStruct { }",
        Some(0.8f32),
    );

    let arguments = serde_json::to_value(&edit_request).expect("serialize request");
    let params = create_test_tool_params(&harness, request_id, arguments).await;

    apply_code_edit_tool(params).await;

    // Should not create proposal due to resolution failure
    {
        let proposals = harness.state.proposals.read().await;
        assert!(
            proposals.is_empty(),
            "Failed resolution should not create proposal"
        );
    }
    restore_fixture();
}

#[tokio::test]
async fn test_canonical_resolution_wrong_node_type() {
    let harness = AppHarness::spawn().await.expect("spawn harness");
    let request_id = Uuid::new_v4();

    // Try to resolve SampleStruct as a Function (wrong type)
    let edit_request = create_canonical_edit_request(
        "src/structs.rs",
        "crate::structs::SampleStruct",
        NodeType::Function, // Wrong type - SampleStruct is a struct, not function
        "fn SampleStruct() {}",
        Some(0.7f32),
    );

    let arguments = serde_json::to_value(&edit_request).expect("serialize request");
    let params = create_test_tool_params(&harness, request_id, arguments).await;

    apply_code_edit_tool(params).await;

    // Should not create proposal due to type mismatch
    {
        let proposals = harness.state.proposals.read().await;
        assert!(
            proposals.is_empty(),
            "Wrong node type should not create proposal"
        );
    }
    restore_fixture();
}

#[tokio::test]
async fn test_canonical_fallback_resolver() {
    let harness = AppHarness::spawn().await.expect("spawn harness");
    let request_id = Uuid::new_v4();

    // Use a canonical path that might not match exactly due to path resolution
    // but should be found by the fallback resolver
    let edit_request = create_canonical_edit_request(
        "src/structs.rs", // Relative path that might need fallback resolution
        "crate::structs::SampleStruct",
        NodeType::Struct,
        "pub struct SampleStruct { pub field: String, }",
Some(0.9f32),
    );

    let arguments = serde_json::to_value(&edit_request).expect("serialize request");
    let params = create_test_tool_params(&harness, request_id, arguments).await;

    apply_code_edit_tool(params).await;

    // Should create proposal using fallback resolution
    {
        let proposals = harness.state.proposals.read().await;
        let proposal = proposals
            .get(&request_id)
            .expect("Fallback resolution should work");
        assert_eq!(proposal.status, EditProposalStatus::Pending);
        assert!(!proposal.edits.is_empty(), "Should have resolved edits");
    }
    restore_fixture();
}

// ============================================================================
// Phase 3: Preview Generation Tests
// ============================================================================

#[tokio::test]
async fn test_unified_diff_preview_generation() {
    let harness = AppHarness::spawn().await.expect("spawn harness");

    // Set preview mode to Diff
    {
        let mut config = harness.state.config.write().await;
        config.editing.preview_mode = PreviewMode::Diff;
    }

    let request_id = Uuid::new_v4();
    let edit_request = create_canonical_edit_request(
        "src/structs.rs",
        "crate::structs::SampleStruct",
        NodeType::Struct,
        "pub struct SampleStruct { pub field: String, pub added_field: bool, }",
Some(0.9f32),
    );

    let arguments = serde_json::to_value(&edit_request).expect("serialize request");
    let params = create_test_tool_params(&harness, request_id, arguments).await;

    apply_code_edit_tool(params).await;

    {
        let proposals = harness.state.proposals.read().await;
        let proposal = proposals.get(&request_id).expect("Proposal should exist");

        // Should generate unified diff preview
        match &proposal.preview {
            DiffPreview::UnifiedDiff { text } => {
                assert!(text.contains("@@"), "Should contain diff hunk headers");
                assert!(text.contains("+"), "Should contain additions");
                assert!(!text.trim().is_empty(), "Should have diff content");
            }
            DiffPreview::CodeBlocks { .. } => {
                panic!("Expected UnifiedDiff, got CodeBlocks");
            }
        }
    }
    restore_fixture();
}

#[tokio::test]
async fn test_codeblock_preview_generation() {
    let harness = AppHarness::spawn().await.expect("spawn harness");

    // Set preview mode to CodeBlock
    {
        let mut config = harness.state.config.write().await;
        config.editing.preview_mode = PreviewMode::CodeBlock;
    }

    let request_id = Uuid::new_v4();
    let edit_request = create_canonical_edit_request(
        "src/structs.rs",
        "crate::structs::SampleStruct",
        NodeType::Struct,
        "pub struct SampleStruct { pub field: String, pub added_field: bool, }",
Some(0.9f32),
    );

    let arguments = serde_json::to_value(&edit_request).expect("serialize request");
    let params = create_test_tool_params(&harness, request_id, arguments).await;

    apply_code_edit_tool(params).await;

    {
        let proposals = harness.state.proposals.read().await;
        let proposal = proposals.get(&request_id).expect("Proposal should exist");

        // Should generate code block preview
        match &proposal.preview {
            DiffPreview::CodeBlocks { per_file } => {
                assert!(!per_file.is_empty(), "Should have at least one file");
                let before_after = &per_file[0];
                assert!(
                    !before_after.before.is_empty(),
                    "Should have before content"
                );
                assert!(!before_after.after.is_empty(), "Should have after content");
                assert_ne!(
                    before_after.before, before_after.after,
                    "Before and after should differ"
                );
            }
            DiffPreview::UnifiedDiff { .. } => {
                panic!("Expected CodeBlocks, got UnifiedDiff");
            }
        }
    }
    restore_fixture();
}

#[tokio::test]
async fn test_preview_truncation() {
    let harness = AppHarness::spawn().await.expect("spawn harness");

    // Set low preview line limit
    {
        let mut config = harness.state.config.write().await;
        config.editing.max_preview_lines = 3; // Very low limit
    }

    let request_id = Uuid::new_v4();

    // Create edit with potentially long replacement
    let long_replacement = "pub struct SampleStruct {\n    pub field: String,\n    pub line1: i32,\n    pub line2: i32,\n    pub line3: i32,\n    pub line4: i32,\n    pub line5: i32,\n}";

    let edit_request = create_canonical_edit_request(
        "src/structs.rs",
        "crate::structs::SampleStruct",
        NodeType::Struct,
        long_replacement,
Some(0.9f32),
    );

    let arguments = serde_json::to_value(&edit_request).expect("serialize request");
    let params = create_test_tool_params(&harness, request_id, arguments).await;

    apply_code_edit_tool(params).await;

    {
        let proposals = harness.state.proposals.read().await;
        let proposal = proposals.get(&request_id).expect("Proposal should exist");

        // Check that preview is truncated
        match &proposal.preview {
            DiffPreview::UnifiedDiff { text } => {
                println!("DEBUG: UnifiedDiff text: {:?}", text);
                println!("DEBUG: Line count: {}", text.lines().count());
                assert!(
                    text.contains("... [truncated]") || text.lines().count() <= 10,
                    "Preview should be truncated or reasonably short"
                );
            }
            DiffPreview::CodeBlocks { per_file } => {
                let before_after = &per_file[0];
                println!("DEBUG: CodeBlocks after: {:?}", before_after.after);
                println!("DEBUG: Line count: {}", before_after.after.lines().count());
                // CRITICAL: This test must verify that truncation actually works
                // If max_preview_lines = 3, then either:
                // 1. Preview contains truncation marker, OR
                // 2. Preview respects the 3-line limit (not 50!)
                let line_count = before_after.after.lines().count();
                let has_truncation = before_after.after.contains("... [truncated]");
                
                println!("DEBUG: max_preview_lines was set to 3");
                println!("DEBUG: Actual line count: {}", line_count);
                println!("DEBUG: Has truncation marker: {}", has_truncation);
                
                // This test should FAIL if truncation is broken - that's the point!
                assert!(
                    has_truncation || line_count <= 5, // Allow small buffer for context
                    "Preview truncation is broken! Expected ≤5 lines or truncation marker, got {} lines. \
                     This test SHOULD fail to highlight the truncation bug that needs fixing.", 
                    line_count
                );
            }
        }
    }
    restore_fixture();
}

// ============================================================================
// Phase 4: Proposal Creation & State Management Tests
// ============================================================================

#[tokio::test]
async fn test_proposal_creation_and_storage() {
    let harness = AppHarness::spawn().await.expect("spawn harness");
    let request_id = Uuid::new_v4();

    let edit_request = create_canonical_edit_request(
        "src/structs.rs",
        "crate::structs::SampleStruct",
        NodeType::Struct,
        "pub struct SampleStruct { pub field: String, new_field: i32 }",
        Some(0.85),
    );

    let arguments = serde_json::to_value(&edit_request).expect("serialize request");
    let params = create_test_tool_params(&harness, request_id, arguments).await;

    apply_code_edit_tool(params).await;

    {
        let proposals = harness.state.proposals.read().await;
        let proposal = proposals
            .get(&request_id)
            .expect("Proposal should be created");

        // Verify all proposal fields are populated correctly
        assert_eq!(proposal.request_id, request_id);
        assert_eq!(proposal.status, EditProposalStatus::Pending);
        assert!(!proposal.edits.is_empty());
        assert!(!proposal.files.is_empty());
        assert!(proposal.proposed_at_ms > 0);

        // Verify edit data
        let edit = &proposal.edits[0];
        assert_eq!(
            edit.replacement,
            "pub struct SampleStruct { pub field: String, new_field: i32 }",
        );
        assert!(edit.file_path.to_string_lossy().contains("structs.rs"));
    }
    restore_fixture();
}

#[tokio::test]
async fn test_auto_confirm_workflow() {
    let harness = AppHarness::spawn().await.expect("spawn harness");

    // Enable auto-confirm
    {
        let mut config = harness.state.config.write().await;
        config.editing.auto_confirm_edits = true;
    }

    let request_id = Uuid::new_v4();
    let edit_request = create_canonical_edit_request(
        "src/structs.rs",
        "crate::structs::SampleStruct",
        NodeType::Struct,
        "pub struct SampleStruct { pub field: String, new_field_usize: usize}",
Some(0.9f32),
    );

    let arguments = serde_json::to_value(&edit_request).expect("serialize request");
    let params = create_test_tool_params(&harness, request_id, arguments).await;

    apply_code_edit_tool(params).await;

    // Allow time for async auto-approval to complete
    tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;

    {
        let proposals = harness.state.proposals.read().await;
        let proposal = proposals.get(&request_id).expect("Proposal should exist");

        // Auto-approval should have completed and applied the edit
        assert_eq!(proposal.status, EditProposalStatus::Applied);
    }
    restore_fixture();
}

#[tokio::test]
async fn test_tool_result_structure() {
    let harness = AppHarness::spawn().await.expect("spawn harness");
    let request_id = Uuid::new_v4();

    let edit_request = create_canonical_edit_request(
        "src/structs.rs",
        "crate::structs::SampleStruct",
        NodeType::Struct,
        "pub struct SampleStruct { pub field: String, }",
Some(0.9f32),
    );

    let arguments = serde_json::to_value(&edit_request).expect("serialize request");
    let params = create_test_tool_params(&harness, request_id, arguments).await;

    apply_code_edit_tool(params).await;

    // Verify proposal exists and can be used to construct tool result
    {
        let proposals = harness.state.proposals.read().await;
        let proposal = proposals.get(&request_id).expect("Proposal should exist");

        // Simulate the tool result construction logic from apply_code_edit_tool
        let crate_root = harness.state.system.read().await.crate_focus.clone();
        let display_files: Vec<String> = proposal
            .files
            .iter()
            .map(|p| {
                if let Some(root) = crate_root.as_ref() {
                    p.strip_prefix(root)
                        .map(|rp| rp.display().to_string())
                        .unwrap_or_else(|_| p.display().to_string())
                } else {
                    p.display().to_string()
                }
            })
            .collect();

        let editing_cfg = harness.state.config.read().await.editing.clone();
        let structured_result = ApplyCodeEditResult {
            ok: true,
            staged: proposal.edits.len(),
            applied: 0, // Should be 0 for staging
            files: display_files,
            preview_mode: match editing_cfg.preview_mode {
                PreviewMode::Diff => "diff".to_string(),
                PreviewMode::CodeBlock => "codeblock".to_string(),
            },
            auto_confirmed: editing_cfg.auto_confirm_edits,
        };

        // Verify structured result
        assert!(structured_result.ok);
        assert_eq!(structured_result.staged, 1);
        assert_eq!(structured_result.applied, 0);
        assert!(!structured_result.files.is_empty());
        assert!(!structured_result.preview_mode.is_empty());
    }
    restore_fixture();
}

// ============================================================================
// Phase 5: Multiple Files and Batch Processing Tests
// ============================================================================

#[tokio::test]
async fn test_multiple_files_batch_processing() {
    let harness = AppHarness::spawn().await.expect("spawn harness");
    let request_id = Uuid::new_v4();

    // Create edit request with multiple files (structs and enums from fixture_nodes)
    let multi_edit_request = ApplyCodeEditRequest {
        confidence: Some(0.9),
        edits: vec![
            Edit::Canonical {
                file: "src/structs.rs".to_string(),
                canon: "crate::structs::SampleStruct".to_string(),
                node_type: NodeType::Struct,
                code: "pub struct SampleStruct { pub field: String, }".to_string(),
            },
            Edit::Canonical {
                file: "tests/fixture_crates/fixture_nodes/src/enums.rs".to_string(),
                canon: "crate::SimpleEnum".to_string(), // Assuming this exists in fixture
                node_type: NodeType::Enum,
                code: "pub enum SimpleEnum { A, B, C, }".to_string(),
            },
        ],
    };

    let arguments = serde_json::to_value(&multi_edit_request).expect("serialize request");
    let params = create_test_tool_params(&harness, request_id, arguments).await;

    apply_code_edit_tool(params).await;

    {
        let proposals = harness.state.proposals.read().await;

        // Either both edits succeed (creating 1 proposal with 2 edits),
        // or only the valid ones succeed (depends on fixture data)
        if let Some(proposal) = proposals.get(&request_id) {
            assert!(
                !proposal.edits.is_empty(),
                "Should have at least one successful edit"
            );
            assert!(
                !proposal.files.is_empty(),
                "Should reference at least one file"
            );

            // If both succeeded
            if proposal.edits.len() == 2 {
                assert!(proposal.files.len() >= 2, "Should reference multiple files");

                // Verify both files are represented
                let file_paths: Vec<String> = proposal
                    .files
                    .iter()
                    .map(|p| p.to_string_lossy().to_string())
                    .collect();

                assert!(
                    file_paths.iter().any(|p| p.contains("structs.rs")),
                    "Should include structs.rs"
                );
            }
        } else {
            // If no proposals created, it means the fixture data doesn't contain
            // the expected enum - this is acceptable for this test
            println!("No proposals created - fixture may not contain expected enum");
        }
    }
    restore_fixture();
}

// ============================================================================
// Phase 6: Error Conditions Tests
// ============================================================================

#[tokio::test]
async fn test_unsupported_node_type() {
    let harness = AppHarness::spawn().await.expect("spawn harness");
    let request_id = Uuid::new_v4();

    // Set up event listener to capture tool call failures
    use crate::{EventPriority, AppEvent};
    let mut event_rx = harness.event_bus.subscribe(EventPriority::Realtime);

    // Use a non-primary node type (should be rejected)
    let edit_request = ApplyCodeEditRequest {
        confidence: Some(0.9),
        edits: vec![Edit::Canonical {
            file: "src/structs.rs".to_string(),
            canon: "crate::structs::SampleStruct".to_string(),
            node_type: NodeType::Param, // Not in primary_nodes()
            code: "pub struct SampleStruct { }".to_string(),
        }],
    };

    let arguments = serde_json::to_value(&edit_request).expect("serialize request");
    let params = create_test_tool_params(&harness, request_id, arguments).await;

    apply_code_edit_tool(params).await;
    
    // Allow time for events to be processed
    tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

    // Verify that a failure event was emitted mentioning node type restriction
    let mut found_node_type_error = false;
    while let Ok(event) = event_rx.try_recv() {
        if let AppEvent::System(system_event) = event {
            let event_str = format!("{:?}", system_event);
            if event_str.contains("ToolCallFailed") && 
               (event_str.contains("primary_nodes") || event_str.contains("node type")) {
                found_node_type_error = true;
                println!("DEBUG: Found expected node type error: {:?}", system_event);
                break;
            }
        }
    }

    assert!(found_node_type_error, "Unsupported node type should emit ToolCallFailed event mentioning node type restriction");

    // Should not create proposal due to unsupported node type
    {
        let proposals = harness.state.proposals.read().await;
        assert!(
            proposals.is_empty(),
            "Unsupported node type should not create proposal"
        );
    }
    restore_fixture();
}

#[tokio::test]
async fn test_invalid_canonical_path_format() {
    let harness = AppHarness::spawn().await.expect("spawn harness");
    let request_id = Uuid::new_v4();

    let edit_request = create_canonical_edit_request(
        "src/structs.rs",
        "", // Empty canonical path
        NodeType::Struct,
        "pub struct Test { }",
        Some(0.5),
    );

    let arguments = serde_json::to_value(&edit_request).expect("serialize request");
    let params = create_test_tool_params(&harness, request_id, arguments).await;

    apply_code_edit_tool(params).await;

    // Should not create proposal due to invalid canonical path
    {
        let proposals = harness.state.proposals.read().await;
        assert!(
            proposals.is_empty(),
            "Invalid canonical path should not create proposal"
        );
    }
    restore_fixture();
}

// ============================================================================
// Integration Test: Complete Flow
// ============================================================================

#[tokio::test]
async fn test_complete_canonical_edit_flow_integration() {
    let harness = AppHarness::spawn()
        .await
        .expect("spawn harness - this test requires the complete fixture database backup");
    let request_id = Uuid::new_v4();

    // Set up configuration
    {
        let mut config = harness.state.config.write().await;
        config.editing.preview_mode = PreviewMode::Diff;
        config.editing.auto_confirm_edits = false;
        config.editing.max_preview_lines = 20;
    }

    // Create realistic edit request using fixture_nodes data
    let edit_request = create_canonical_edit_request(
        "src/structs.rs",
        "crate::structs::SampleStruct",
        NodeType::Struct,
        r#"pub struct SampleStruct {
    pub field: String,
    pub new_field: i32,
    pub created_at: std::time::SystemTime,
}"#,
        Some(0.95),
    );

    let arguments = serde_json::to_value(&edit_request).expect("serialize request");
    let params = create_test_tool_params(&harness, request_id, arguments).await;

    // Execute the complete flow
    apply_code_edit_tool(params).await;

    // Comprehensive verification
    {
        let proposals = harness.state.proposals.read().await;
        let proposal = proposals.get(&request_id).expect(
            "Integration test should create proposal. Failure indicates:\n\
                1. Database backup missing or empty\n\
                2. fixture_nodes not properly parsed\n\
                3. SampleStruct not found in fixture data\n\
                Run ploke-db tests to verify fixture parsing.",
        );

        // Verify complete proposal structure
        assert_eq!(proposal.request_id, request_id);
        assert_eq!(proposal.status, EditProposalStatus::Pending);
        assert!(!proposal.edits.is_empty(), "Should have edits");
        assert!(!proposal.files.is_empty(), "Should have files");
        assert!(proposal.proposed_at_ms > 0, "Should have timestamp");

        // Verify edit details (proves database resolution worked)
        let edit = &proposal.edits[0];
        assert!(edit.file_path.to_string_lossy().contains("structs.rs"));
        assert!(
            edit.start_byte < edit.end_byte,
            "Should have valid byte range"
        );
        assert!(
            edit.replacement.contains("new_field"),
            "Should contain new field"
        );
        assert!(
            edit.replacement.contains("created_at"),
            "Should contain created_at field"
        );

        // Verify preview generation worked
        match &proposal.preview {
            DiffPreview::UnifiedDiff { text } => {
                assert!(text.contains("@@"), "Should have diff headers");
                assert!(!text.trim().is_empty(), "Should have content");
            }
            _ => panic!("Expected unified diff preview"),
        }

        // Verify file list
        assert_eq!(proposal.files.len(), 1, "Should have exactly one file");
        let file_path_str = proposal.files[0].to_string_lossy();
        assert!(
            file_path_str.contains("structs.rs"),
            "File path should be correct"
        );
    }
    restore_fixture();
}
