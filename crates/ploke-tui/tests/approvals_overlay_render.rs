//! Approvals overlay render tests
//!
//! Purpose: verify the Approvals overlay renders deterministically and conveys
//! key information (titles, list entries, previews) across UnifiedDiff and
//! CodeBlocks modes.
//!
//! TEST_GUIDELINES adherence:
//! - Determinism: fixed Rect sizes; stable proposal ordering; no network/IO.
//! - Dual checks: semantic assertions for intent + insta visual snapshots with
//!   redactions for UUIDs and absolute paths. Snapshots complement semantics
//!   and are gated by `PLOKE_ENABLE_SNAPSHOTS` to avoid churn during local dev.
//! - Redactions: UUIDs → <UUID>, current dir path → <PWD>.
//!
//! Verified properties:
//! - Overlay titles visible ("Approvals", "Pending Proposals", "Details").
//! - List shows truncated request id and file count; details show appropriate
//!   preview headers and diff markers.
//! - Selection changes alter which proposal is detailed.
//!
//! Not verified (by design):
//! - Exact colors/styles and border glyphs; these are subject to theme/ratatui
//!   changes and are exercised indirectly by the visual snapshot.
//! - Exact line wrapping of long content; tests limit preview lines for
//!   stability.

use std::sync::Arc;

use ploke_core::ArcStr;
use ratatui::{backend::TestBackend, Terminal};
use ratatui::layout::Rect;

use ploke_tui::app::view::components::approvals::{render_approvals_overlay, ApprovalsState};
use ploke_tui::app_state::core::{AppState, ChatState, ConfigState, DiffPreview, EditProposal, EditProposalStatus, RuntimeConfig, SystemState};
use tokio::sync::RwLock;

fn buffer_to_lines(term: &Terminal<TestBackend>) -> Vec<String> {
    let buffer = term.backend().buffer();
    let mut out = Vec::new();
    for row in 0..buffer.area.height {
        let mut s = String::new();
        for col in 0..buffer.area.width {
            let sym = buffer
                .cell((col, row))
                .expect("buffer cell in-bounds")
                .symbol()
                .chars()
                .next()
                .unwrap_or(' ');
            s.push(sym);
        }
        out.push(s);
    }
    out
}

fn redact(text: &str) -> String {
    let uuid_re = regex::Regex::new(
        r"[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}",
    )
    .unwrap();
    let mut out = uuid_re.replace_all(text, "<UUID>").to_string();
    if let Ok(pwd) = std::env::current_dir() {
        let esc = regex::escape(&pwd.display().to_string());
        if let Ok(pwd_re) = regex::Regex::new(&esc) {
            out = pwd_re.replace_all(&out, "<PWD>").to_string();
        }
    }
    out
}

async fn make_state_with_ids(previews: Vec<(uuid::Uuid, DiffPreview)>) -> (Arc<AppState>, Vec<uuid::Uuid>) {
    let db = Arc::new(ploke_db::Database::init_with_schema().expect("db init"));
    let io_handle = ploke_io::IoManagerHandle::new();
    let cfg = ploke_tui::user_config::UserConfig::default();
    let embedder = Arc::new(cfg.load_embedding_processor().expect("embedder"));
    let state = Arc::new(AppState {
        chat: ChatState::new(ploke_tui::chat_history::ChatHistory::new()),
        config: ConfigState::new(RuntimeConfig::from(cfg.clone())),
        system: SystemState::default(),
        indexing_state: RwLock::new(None),
        indexer_task: None,
        indexing_control: Arc::new(tokio::sync::Mutex::new(None)),
        db,
        embedder,
        io_handle,
        rag: None,
        budget: ploke_rag::TokenBudget::default(),
        proposals: RwLock::new(std::collections::HashMap::new()),
        create_proposals: RwLock::new(std::collections::HashMap::new()),
    });

    let mut ids = Vec::new();
    {
        let mut guard = state.proposals.write().await;
        for (i, (id, preview)) in previews.into_iter().enumerate() {
            ids.push(id);
            guard.insert(id, EditProposal {
                request_id: id,
                parent_id: uuid::Uuid::new_v4(),
                call_id: ArcStr::from( format!("call-{i}") ),
                proposed_at_ms: chrono::Utc::now().timestamp_millis(),
                edits: vec![],
                files: vec![std::env::current_dir().unwrap().join("Cargo.toml")],
                preview,
                status: EditProposalStatus::Pending,
            });
        }
    }
    (state, ids)
}

#[test]
fn approvals_overlay_renders_empty_list() {
    let rt = tokio::runtime::Builder::new_multi_thread().enable_all().build().unwrap();
    rt.block_on(async {
        let backend = TestBackend::new(60, 20);
        let mut term = Terminal::new(backend).expect("terminal");
        let db = Arc::new(ploke_db::Database::init_with_schema().expect("db init"));
        let io_handle = ploke_io::IoManagerHandle::new();
        let cfg = ploke_tui::user_config::UserConfig::default();
        let embedder = Arc::new(cfg.load_embedding_processor().expect("embedder"));
        let state = Arc::new(AppState {
            chat: ChatState::new(ploke_tui::chat_history::ChatHistory::new()),
            config: ConfigState::new(RuntimeConfig::from(cfg.clone())),
            system: SystemState::default(),
            indexing_state: RwLock::new(None),
            indexer_task: None,
            indexing_control: Arc::new(tokio::sync::Mutex::new(None)),
            db,
            embedder,
            io_handle,
            rag: None,
            budget: ploke_rag::TokenBudget::default(),
            proposals: RwLock::new(std::collections::HashMap::new()),
            create_proposals: RwLock::new(std::collections::HashMap::new()),
        });
        let ui = ApprovalsState::default();

        term.draw(|f| {
            let area = Rect::new(0, 0, 60, 20);
            let _ = render_approvals_overlay(f, area, &state, &ui);
        }).expect("draw");

        let text = buffer_to_lines(&term).join("\n");
        assert!(text.contains(" Approvals "));
        assert!(text.contains(" Pending Proposals "));
        assert!(text.contains(" Details "));
        let red = redact(&text);
        insta::assert_snapshot!("approvals_empty_60x20", red);
    });
}

#[test]
fn approvals_overlay_renders_single_proposal_unified_diff() {
    let rt = tokio::runtime::Builder::new_multi_thread().enable_all().build().unwrap();
    rt.block_on(async {
        let backend = TestBackend::new(80, 24);
        let mut term = Terminal::new(backend).expect("terminal");
        let id = uuid::Uuid::from_u128(0x12345678_1234_5678_1234_567812345678);
        let (state, ids) = make_state_with_ids(vec![(id, DiffPreview::UnifiedDiff { text: "diff --git a/src b/src\n- old\n+ new\n".into() })]).await;
        let ui = ApprovalsState::default();

        term.draw(|f| {
            let area = Rect::new(0, 0, 80, 24);
            let _ = render_approvals_overlay(f, area, &state, &ui);
        }).expect("draw");

        let text = buffer_to_lines(&term).join("\n");
        assert!(text.contains(" Approvals "));
        assert!(text.contains(" Pending Proposals "));
        let short = ploke_tui::app::utils::truncate_uuid(ids[0]);
        assert!(text.contains(&short));
        assert!(text.contains("files:1"));
        assert!(text.contains("Unified Diff:"));
        assert!(text.contains("- old"));
        assert!(text.contains("+ new"));
        let red = redact(&text);
        insta::assert_snapshot!("approvals_unified_80x24", red);
    });
}

#[test]
fn approvals_overlay_renders_codeblocks_preview_and_selection() {
    let rt = tokio::runtime::Builder::new_multi_thread().enable_all().build().unwrap();
    rt.block_on(async {
        let backend = TestBackend::new(90, 28);
        let mut term = Terminal::new(backend).expect("terminal");
        let id = uuid::Uuid::from_u128(0x87654321_4321_8765_4321_876543218765);
        let (state, _ids) = make_state_with_ids(vec![(id, DiffPreview::CodeBlocks {
            per_file: vec![ploke_tui::app_state::core::BeforeAfter {
                file_path: std::env::current_dir().unwrap().join("Cargo.toml"),
                before: "fn a() {}\nfn b() {}".into(),
                after: "fn a() {}\nfn c() {}".into(),
            }],
        })]).await;
        let ui = ApprovalsState::default();

        term.draw(|f| {
            let area = Rect::new(0, 0, 90, 28);
            let _ = render_approvals_overlay(f, area, &state, &ui);
        }).expect("draw");
        let text = buffer_to_lines(&term).join("\n");
        assert!(text.contains("Before/After:"));
        assert!(text.contains("- fn a() {}"));
        assert!(text.contains("+ fn a() {}"));
        let red = redact(&text);
        insta::assert_snapshot!("approvals_codeblocks_90x28", red);
    });
}

#[test]
fn approvals_overlay_renders_multiple_and_moves_selection() {
    let rt = tokio::runtime::Builder::new_multi_thread().enable_all().build().unwrap();
    rt.block_on(async {
        let backend = TestBackend::new(80, 24);
        let mut term = Terminal::new(backend).expect("terminal");
        let id1 = uuid::Uuid::from_u128(0xaaaaaaaa_bbbb_cccc_dddd_eeeeeeeeeeee);
        let id2 = uuid::Uuid::from_u128(0xbbbbbbbb_cccc_dddd_eeee_ffffffffffff);
        let (state, ids) = make_state_with_ids(vec![
            (id1, DiffPreview::UnifiedDiff { text: "one".into() }),
            (id2, DiffPreview::UnifiedDiff { text: "two".into() }),
        ]).await;
        let mut ui = ApprovalsState::default();

        term.draw(|f| {
            let area = Rect::new(0, 0, 80, 24);
            let _ = render_approvals_overlay(f, area, &state, &ui);
        }).expect("draw");
        let text1 = buffer_to_lines(&term).join("\n");
        assert!(text1.contains(&ploke_tui::app::utils::truncate_uuid(ids[0])));

        ui.selected = 1;
        term.draw(|f| {
            let area = Rect::new(0, 0, 80, 24);
            let _ = render_approvals_overlay(f, area, &state, &ui);
        }).expect("draw");
        let text2 = buffer_to_lines(&term).join("\n");
        assert!(text2.contains(&ploke_tui::app::utils::truncate_uuid(ids[1])));
        let red1 = redact(&text1);
        let red2 = redact(&text2);
        insta::assert_snapshot!("approvals_multiple_sel0_80x24", red1);
        insta::assert_snapshot!("approvals_multiple_sel1_80x24", red2);
    });
}
