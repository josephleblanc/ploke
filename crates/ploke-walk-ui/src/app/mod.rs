mod panels;

use std::{sync::mpsc, thread};

use eframe::egui;
use ploke_eval::walk_client::{
    PhaseInventory, WalkClient, WalkQuerySnapshot, WalkResponse, WalkRunEntry,
};

use crate::client;
use crate::model::{
    DEFAULT_QUERY, ServiceStatus, UiButtonState, UiEvent, WalkRequestKind, WalkRequestResult,
    nonempty_path, optional_text,
};
use panels::{
    debug::DebugWindow,
    details::{DetailsAction, DetailsPanel},
    phase_rail::PhaseRail,
    query::{QueryAction, QueryPanel},
    top_bar::{TopBar, TopBarAction},
};

pub(crate) struct WalkUiApp {
    event_tx: mpsc::Sender<UiEvent>,
    event_rx: mpsc::Receiver<UiEvent>,
    socket_input: String,
    campaign_input: String,
    query_script: String,
    runs: Vec<WalkRunEntry>,
    selected_run: Option<usize>,
    run_error: Option<String>,
    client: Option<WalkClient>,
    phases: PhaseInventory,
    status: ServiceStatus,
    response: Option<WalkResponse>,
    query_result: Option<WalkQuerySnapshot>,
    selected_row: Option<usize>,
    notice: Option<String>,
    walk_pending: Option<WalkRequestKind>,
    query_pending: bool,
    debug_panel: bool,
    debug_hover: bool,
    buttons: UiButtonState,
}

impl WalkUiApp {
    pub(crate) fn new() -> Self {
        let (event_tx, event_rx) = mpsc::channel();
        let mut app = Self {
            event_tx,
            event_rx,
            socket_input: String::new(),
            campaign_input: String::new(),
            query_script: DEFAULT_QUERY.to_string(),
            runs: Vec::new(),
            selected_run: None,
            run_error: None,
            client: None,
            phases: PhaseInventory::current(),
            status: ServiceStatus::Unresolved,
            response: None,
            query_result: None,
            selected_row: None,
            notice: None,
            walk_pending: None,
            query_pending: false,
            debug_panel: false,
            debug_hover: false,
            buttons: UiButtonState::default(),
        };
        app.refresh_runs();
        app.refresh_health(None);
        app
    }

    fn refresh_runs(&mut self) {
        let selected_campaign = self
            .selected_campaign_id()
            .map(str::to_owned)
            .or_else(|| optional_text(&self.campaign_input).map(str::to_owned));

        match WalkClient::discover_runs() {
            Ok(runs) => {
                self.runs = runs;
                self.selected_run = selected_campaign
                    .as_deref()
                    .and_then(|campaign| {
                        self.runs.iter().position(|run| run.campaign_id == campaign)
                    })
                    .or_else(|| (!self.runs.is_empty()).then_some(0));
                self.run_error = None;
                self.resolve_selected_client();
            }
            Err(error) => {
                self.runs.clear();
                self.selected_run = None;
                self.run_error = Some(error.to_string());
                self.client = None;
                self.status = ServiceStatus::Error(error.to_string());
            }
        }
    }

    fn select_run(&mut self, index: usize) {
        self.selected_run = Some(index);
        self.resolve_selected_client();
        self.refresh_health(None);
    }

    fn resolve_selected_client(&mut self) {
        let Some(run) = self.selected_run().cloned() else {
            self.client = None;
            self.response = None;
            self.status = ServiceStatus::Unresolved;
            self.notice = Some("no Prototype 1 run selected".to_string());
            return;
        };

        self.campaign_input = run.campaign_id.clone();
        if run.worktree_root.is_none() {
            self.client = None;
            self.response = None;
            self.status = ServiceStatus::Unresolved;
            self.notice = Some(format!(
                "selected run '{}' has no local worktree; DB queries can still run",
                run.campaign_id
            ));
            return;
        }

        let socket = nonempty_path(&self.socket_input);
        match WalkClient::resolve_for_run(&run, socket.as_deref()) {
            Ok(client) => {
                self.notice = Some(format!("socket {}", client.socket().display()));
                self.client = Some(client);
            }
            Err(error) => {
                self.status = ServiceStatus::Error(error.to_string());
                self.client = None;
            }
        }
    }

    fn refresh_health(&mut self, repaint: Option<egui::Context>) {
        self.start_walk_request(WalkRequestKind::Health, repaint);
    }

    fn show_state(&mut self, repaint: Option<egui::Context>) {
        self.start_walk_request(WalkRequestKind::Show, repaint);
    }

    fn start_walk_request(&mut self, kind: WalkRequestKind, repaint: Option<egui::Context>) {
        let Some(client) = self.client.clone() else {
            self.status = ServiceStatus::Unresolved;
            return;
        };
        if self.walk_pending.is_some() {
            self.notice = Some("walk request already pending".to_string());
            return;
        }

        self.walk_pending = Some(kind);
        self.status = ServiceStatus::Busy(format!("{} pending", kind.label()));
        self.notice = Some(format!("{} request pending", kind.label()));
        let tx = self.event_tx.clone();
        thread::spawn(move || {
            let result = client::run_walk_request(kind, client);
            let _ = tx.send(UiEvent::Walk { kind, result });
            if let Some(ctx) = repaint {
                ctx.request_repaint();
            }
        });
    }

    fn run_query(&mut self, repaint: Option<egui::Context>) {
        let Some(campaign) = self.query_campaign() else {
            self.notice = Some("select a run or enter a campaign id".to_string());
            return;
        };
        if self.query_pending {
            self.notice = Some("query already pending".to_string());
            return;
        }
        let Some(client) = self.client.clone() else {
            self.notice = Some(
                "the selected run has no walk service; start or select its server before querying"
                    .to_string(),
            );
            return;
        };

        self.query_pending = true;
        self.notice = Some("query pending".to_string());
        let script = self.query_script.clone();
        let tx = self.event_tx.clone();
        thread::spawn(move || {
            let result = client::query_db(client, &campaign, &script);
            let _ = tx.send(UiEvent::Query(result));
            if let Some(ctx) = repaint {
                ctx.request_repaint();
            }
        });
    }

    fn poll_events(&mut self) {
        while let Ok(event) = self.event_rx.try_recv() {
            match event {
                UiEvent::Walk { kind, result } => self.finish_walk_request(kind, result),
                UiEvent::Query(result) => self.finish_query(result),
            }
        }
    }

    fn finish_walk_request(&mut self, kind: WalkRequestKind, result: WalkRequestResult) {
        if self.walk_pending == Some(kind) {
            self.walk_pending = None;
        }
        match result {
            WalkRequestResult::Response(response) => {
                self.status = match &response {
                    WalkResponse::Error { detail, .. } => ServiceStatus::Error(detail.clone()),
                    _ => ServiceStatus::Online,
                };
                self.notice = Some(format!("{} response received", kind.label()));
                self.response = Some(response);
            }
            WalkRequestResult::Offline(socket) => {
                self.status = ServiceStatus::Offline;
                self.response = None;
                self.notice = Some(format!("walk server is offline at {}", socket.display()));
            }
            WalkRequestResult::TimedOut => {
                self.status = ServiceStatus::Busy("walk server did not respond".to_string());
                self.notice = Some(format!(
                    "{} timed out after {}s; the walk server may be busy with a live step",
                    kind.label(),
                    crate::model::WALK_REQUEST_TIMEOUT.as_secs()
                ));
            }
            WalkRequestResult::ClientError(error) => {
                self.status = ServiceStatus::Error(error);
            }
        }
    }

    fn finish_query(&mut self, result: Result<WalkQuerySnapshot, String>) {
        self.query_pending = false;
        match result {
            Ok(result) => {
                self.selected_row = None;
                self.notice = Some(format!("{} row(s)", result.result.row_count));
                self.query_result = Some(result);
            }
            Err(error) => {
                self.notice = Some(error);
            }
        }
    }

    fn selected_run(&self) -> Option<&WalkRunEntry> {
        self.selected_run.and_then(|index| self.runs.get(index))
    }

    fn selected_campaign_id(&self) -> Option<&str> {
        self.selected_run().map(|run| run.campaign_id.as_str())
    }

    fn query_campaign(&self) -> Option<String> {
        optional_text(&self.campaign_input)
            .map(str::to_owned)
            .or_else(|| self.selected_campaign_id().map(str::to_owned))
    }

    fn handle_top_bar_action(&mut self, action: TopBarAction, ctx: &egui::Context) {
        if let Some(index) = action.selected_run {
            self.select_run(index);
        }
        if action.refresh_runs {
            self.refresh_runs();
            self.refresh_health(Some(ctx.clone()));
        }
    }

    fn handle_query_action(&mut self, action: QueryAction, ctx: &egui::Context) {
        if action.run_query {
            self.run_query(Some(ctx.clone()));
        }
    }

    fn handle_details_action(&mut self, action: DetailsAction, ctx: &egui::Context) {
        if action.refresh_health {
            self.refresh_health(Some(ctx.clone()));
        }
        if action.show_state {
            self.show_state(Some(ctx.clone()));
        }
    }
}

impl eframe::App for WalkUiApp {
    fn logic(&mut self, _ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.poll_events();
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        egui::Panel::top("top_bar").show_inside(ui, |ui| {
            let action = TopBar {
                status: &self.status,
                runs: &self.runs,
                selected_run: self.selected_run,
                socket_input: &mut self.socket_input,
                debug_panel: &mut self.debug_panel,
            }
            .show(ui);
            self.handle_top_bar_action(action, &ctx);
        });

        egui::Panel::left("phase_rail")
            .resizable(true)
            .default_size(280.0)
            .show_inside(ui, |ui| {
                PhaseRail {
                    phases: &self.phases,
                    current: self.response.as_ref().and_then(WalkResponse::phase),
                }
                .show(ui);
            });

        egui::Panel::right("details")
            .resizable(true)
            .default_size(360.0)
            .show_inside(ui, |ui| {
                let action = DetailsPanel {
                    runs: &self.runs,
                    selected_run: self.selected_run,
                    run_error: self.run_error.as_deref(),
                    client_available: self.client.is_some(),
                    walk_pending: self.walk_pending,
                    response: self.response.as_ref(),
                    notice: self.notice.as_deref(),
                    query_result: self.query_result.as_ref(),
                    selected_row: self.selected_row,
                    run_details: &mut self.buttons.run_details,
                }
                .show(ui);
                self.handle_details_action(action, &ctx);
            });

        egui::CentralPanel::default().show_inside(ui, |ui| {
            let selected_campaign = self.selected_campaign_id().map(str::to_owned);
            let action = QueryPanel {
                campaign_input: &mut self.campaign_input,
                query_script: &mut self.query_script,
                query_pending: self.query_pending,
                query_result: self.query_result.as_ref(),
                selected_row: &mut self.selected_row,
                selected_campaign: selected_campaign.as_deref(),
            }
            .show(ui);
            self.handle_query_action(action, &ctx);
        });

        if self.debug_panel {
            let selected_campaign = self.selected_campaign_id().map(str::to_owned);
            let client_socket = self
                .client
                .as_ref()
                .map(|client| client.socket().display().to_string());
            DebugWindow {
                debug_hover: &mut self.debug_hover,
                status: &self.status,
                selected_campaign: selected_campaign.as_deref(),
                client_socket: client_socket.as_deref(),
                walk_pending: self.walk_pending,
                query_pending: self.query_pending,
                runs_len: self.runs.len(),
                row_count: self
                    .query_result
                    .as_ref()
                    .map(|query| query.result.row_count),
            }
            .show(ui.ctx());
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        path::PathBuf,
        time::{Duration, Instant, SystemTime, UNIX_EPOCH},
    };

    use super::*;
    use crate::model::WALK_REQUEST_TIMEOUT;

    #[test]
    fn kittest_clicks_query_button_without_live_server() {
        use egui_kittest::{Harness, kittest::Queryable};

        let mut harness = Harness::builder()
            .with_size(egui::Vec2::new(1280.0, 820.0))
            .build_eframe(|_cc| test_app());

        harness.get_by_label("Run Query").click();
        harness.run();

        assert_eq!(
            harness.state().notice.as_deref(),
            Some("select a run or enter a campaign id")
        );
    }

    #[test]
    fn typed_error_response_retains_version_and_epoch() {
        let response: WalkResponse = serde_json::from_value(serde_json::json!({
            "type": "error",
            "code": "stale_version",
            "detail": "client observed an older session revision",
            "phase": "r6",
            "version": {
                "session_id": null,
                "cursor": null,
                "journal_revision": 9
            },
            "epoch": {
                "protocol_version": 6,
                "transition_graph_version": "walk-r0-r14a-v2",
                "repo_root": "/tmp/ploke-parent",
                "exe_path": "/tmp/ploke-eval",
                "exe_modified_unix_ms": 17,
                "git_head": "abc123",
                "active_branch": "successor/runtime-2",
                "source_status_hash": "def456"
            }
        }))
        .expect("typed error response");
        let mut app = test_app();

        app.finish_walk_request(
            WalkRequestKind::Health,
            WalkRequestResult::Response(response),
        );

        assert!(matches!(app.status, ServiceStatus::Error(_)));
        let Some(WalkResponse::Error {
            code,
            version: Some(version),
            epoch,
            ..
        }) = app.response.as_ref()
        else {
            panic!("UI must retain the exact structured error response");
        };
        assert_eq!(*code, ploke_eval::walk_client::WalkErrorCode::StaleVersion);
        assert_eq!(version.journal_revision(), 9);
        assert_eq!(epoch.active_branch.as_deref(), Some("successor/runtime-2"));
    }

    #[test]
    fn query_result_retains_phase_session_and_epoch_envelope() {
        let query: WalkQuerySnapshot = serde_json::from_value(serde_json::json!({
            "phase": "r6",
            "result": {
                "repo_root": "/tmp/ploke-parent",
                "campaign_id": "campaign-query",
                "db_path": "/tmp/owner.cozo.sqlite",
                "script": "::relations",
                "revision": "abc123",
                "headers": ["name"],
                "row_count": 1,
                "rows": [{"cells": ["eval_campaign"], "object": {"name": "eval_campaign"}}]
            },
            "version": {
                "session_id": null,
                "cursor": null,
                "journal_revision": 12
            },
            "epoch": {
                "protocol_version": 6,
                "transition_graph_version": "walk-r0-r14a-v2",
                "repo_root": "/tmp/ploke-parent",
                "exe_path": "/tmp/ploke-eval",
                "exe_modified_unix_ms": 17,
                "git_head": "abc123",
                "active_branch": "successor/runtime-2",
                "source_status_hash": "def456"
            }
        }))
        .expect("typed query envelope");
        let mut app = test_app();

        app.finish_query(Ok(query));

        let stored = app.query_result.as_ref().expect("stored query envelope");
        assert_eq!(stored.phase, ploke_eval::walk_client::WalkPhase::R6);
        assert_eq!(stored.version.journal_revision(), 12);
        assert_eq!(
            stored.epoch.active_branch.as_deref(),
            Some("successor/runtime-2")
        );
        assert_eq!(stored.result.row_count, 1);
    }

    #[test]
    fn show_state_treats_missing_socket_as_offline() {
        let root = unique_temp_dir("ploke-walk-ui-missing-socket");
        fs::create_dir_all(&root).expect("create temp repo root");
        let socket = root.join("walk.sock");
        let client = WalkClient::resolve(Some(&root), Some(&socket)).expect("resolve client");
        let mut app = test_app_with_client(client);

        app.show_state(None);
        wait_for_events(&mut app);

        assert!(matches!(app.status, ServiceStatus::Offline));
        assert!(app.response.is_none());
        assert!(
            app.notice
                .as_deref()
                .is_some_and(|notice| notice.contains("walk server is offline")),
            "notice should explain offline walk server, got {:?}",
            app.notice
        );

        let _ = fs::remove_dir_all(root);
    }

    #[cfg(unix)]
    #[test]
    fn show_state_times_out_without_blocking_ui_on_silent_socket() {
        let root = unique_temp_dir("ploke-walk-ui-silent-socket");
        fs::create_dir_all(&root).expect("create temp repo root");
        let socket = root.join("walk.sock");
        let listener = std::os::unix::net::UnixListener::bind(&socket).expect("bind socket");
        let server = thread::spawn(move || {
            let _accepted = listener.accept().expect("accept client");
            thread::sleep(WALK_REQUEST_TIMEOUT + Duration::from_millis(500));
        });
        let client = WalkClient::resolve(Some(&root), Some(&socket)).expect("resolve client");
        let mut app = test_app_with_client(client);

        let started = Instant::now();
        app.show_state(None);

        assert!(
            started.elapsed() < Duration::from_millis(200),
            "show_state should return immediately on the UI thread"
        );
        wait_for_events(&mut app);
        assert!(matches!(app.status, ServiceStatus::Busy(_)));
        assert!(
            app.notice
                .as_deref()
                .is_some_and(|notice| notice.contains("timed out")),
            "notice should explain timeout, got {:?}",
            app.notice
        );

        let _ = server.join();
        let _ = fs::remove_dir_all(root);
    }

    fn test_app_with_client(client: WalkClient) -> WalkUiApp {
        let mut app = test_app();
        app.client = Some(client);
        app
    }

    fn test_app() -> WalkUiApp {
        let (event_tx, event_rx) = mpsc::channel();
        WalkUiApp {
            event_tx,
            event_rx,
            socket_input: String::new(),
            campaign_input: String::new(),
            query_script: DEFAULT_QUERY.to_string(),
            runs: Vec::new(),
            selected_run: None,
            run_error: None,
            client: None,
            phases: PhaseInventory::current(),
            status: ServiceStatus::Unresolved,
            response: None,
            query_result: None,
            selected_row: None,
            notice: None,
            walk_pending: None,
            query_pending: false,
            debug_panel: false,
            debug_hover: false,
            buttons: UiButtonState::default(),
        }
    }

    fn wait_for_events(app: &mut WalkUiApp) {
        let deadline = Instant::now() + Duration::from_secs(4);
        while Instant::now() < deadline {
            app.poll_events();
            if app.walk_pending.is_none() && !app.query_pending {
                return;
            }
            thread::sleep(Duration::from_millis(10));
        }
        app.poll_events();
    }

    fn unique_temp_dir(prefix: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time")
            .as_nanos();
        std::env::temp_dir().join(format!("{prefix}-{}-{nanos}", std::process::id()))
    }
}
