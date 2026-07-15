mod panels;

use std::{sync::mpsc, thread};

use eframe::egui;
use ploke_eval::walk_client::{
    EvaluationRunCoordinate, EvaluationTraceIndex, EvaluationTraceSnapshot, EvaluationTraceState,
    PhaseInventory, WalkClient, WalkQuerySnapshot, WalkResponse, WalkRunEntry, WalkSessionSnapshot,
};

use crate::client;
use crate::model::{
    CenterView, DEFAULT_QUERY, ServiceStatus, TraceRequestToken, UiButtonState, UiEvent,
    WalkRequestKind, WalkRequestResult, WalkRequestToken, nonempty_path, optional_text,
};
use panels::{
    debug::DebugWindow,
    details::{DetailsAction, DetailsPanel},
    phase_rail::PhaseRail,
    query::{QueryAction, QueryPanel},
    top_bar::{TopBar, TopBarAction},
    trace::{TraceAction, TracePanel},
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
    trace_index: Option<EvaluationTraceIndex>,
    selected_trace: Option<usize>,
    trace_snapshot: Option<EvaluationTraceSnapshot>,
    notice: Option<String>,
    client_generation: u64,
    walk_pending: Option<WalkRequestToken>,
    query_pending: Option<u64>,
    trace_index_pending: Option<TraceRequestToken>,
    trace_pending: Option<(TraceRequestToken, EvaluationRunCoordinate)>,
    trace_serial: u64,
    center_view: CenterView,
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
            trace_index: None,
            selected_trace: None,
            trace_snapshot: None,
            notice: None,
            client_generation: 0,
            walk_pending: None,
            query_pending: None,
            trace_index_pending: None,
            trace_pending: None,
            trace_serial: 0,
            center_view: CenterView::default(),
            debug_panel: false,
            debug_hover: false,
            buttons: UiButtonState::default(),
        };
        app.refresh_runs();
        if app.client.is_some() {
            app.refresh_health(None);
        }
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
                self.invalidate_client_state();
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
        if self.client.is_some() {
            self.refresh_health(None);
        }
    }

    fn resolve_selected_client(&mut self) {
        self.invalidate_client_state();
        let Some(run) = self.selected_run().cloned() else {
            self.client = None;
            self.status = ServiceStatus::Unresolved;
            self.notice = Some("no Prototype 1 run selected".to_string());
            return;
        };

        self.campaign_input = run.campaign_id.clone();
        if run.worktree_root.is_none() {
            self.client = None;
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
                self.status = ServiceStatus::Unresolved;
                self.notice = Some(format!("socket {}", client.socket().display()));
                self.client = Some(client);
            }
            Err(error) => {
                let detail = error.to_string();
                self.status = ServiceStatus::Error(detail.clone());
                self.notice = Some(format!("socket resolution failed: {detail}"));
                self.client = None;
            }
        }
    }

    fn invalidate_client_state(&mut self) {
        self.client_generation = self.client_generation.wrapping_add(1);
        self.walk_pending = None;
        self.query_pending = None;
        self.trace_index_pending = None;
        self.trace_pending = None;
        self.response = None;
        self.query_result = None;
        self.selected_row = None;
        self.trace_index = None;
        self.selected_trace = None;
        self.trace_snapshot = None;
        self.notice = None;
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

        let token = WalkRequestToken {
            generation: self.client_generation,
            kind,
        };
        self.response = None;
        self.walk_pending = Some(token);
        self.status = ServiceStatus::Busy(format!("{} pending", kind.label()));
        self.notice = Some(format!("{} request pending", kind.label()));
        let tx = self.event_tx.clone();
        thread::spawn(move || {
            let result = client::run_walk_request(kind, client);
            let _ = tx.send(UiEvent::Walk { token, result });
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
        if self.query_pending.is_some() {
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

        let generation = self.client_generation;
        self.query_pending = Some(generation);
        self.query_result = None;
        self.selected_row = None;
        self.notice = Some("query pending".to_string());
        let script = self.query_script.clone();
        let tx = self.event_tx.clone();
        thread::spawn(move || {
            let result = client::query_db(client, &campaign, &script);
            let _ = tx.send(UiEvent::Query { generation, result });
            if let Some(ctx) = repaint {
                ctx.request_repaint();
            }
        });
    }

    fn refresh_trace_index(&mut self, repaint: Option<egui::Context>) {
        let Some(client) = self.client.clone() else {
            self.notice = Some("select a run with an available walk service".to_string());
            return;
        };
        if self.trace_index_pending.is_some() {
            self.notice = Some("completed-run index request already pending".to_string());
            return;
        }

        let token = self.next_trace_token();
        self.trace_index_pending = Some(token);
        self.trace_pending = None;
        self.trace_index = None;
        self.selected_trace = None;
        self.trace_snapshot = None;
        self.notice = Some("completed-run index pending".to_string());
        let tx = self.event_tx.clone();
        thread::spawn(move || {
            let result = client::trace_index(client);
            let _ = tx.send(UiEvent::TraceIndex { token, result });
            if let Some(ctx) = repaint {
                ctx.request_repaint();
            }
        });
    }

    fn load_trace(&mut self, coordinate: EvaluationRunCoordinate, repaint: Option<egui::Context>) {
        let Some(client) = self.client.clone() else {
            self.notice = Some("select a run with an available walk service".to_string());
            return;
        };
        if self.trace_pending.is_some() {
            self.notice = Some("evaluation trace request already pending".to_string());
            return;
        }

        let token = self.next_trace_token();
        self.trace_snapshot = None;
        self.trace_pending = Some((token, coordinate.clone()));
        self.notice = Some(format!("loading sealed trace {}", coordinate.run_id));
        let tx = self.event_tx.clone();
        thread::spawn(move || {
            let result = client::trace(client, coordinate.clone());
            let _ = tx.send(UiEvent::Trace {
                token,
                coordinate,
                result,
            });
            if let Some(ctx) = repaint {
                ctx.request_repaint();
            }
        });
    }

    fn poll_events(&mut self) {
        while let Ok(event) = self.event_rx.try_recv() {
            match event {
                UiEvent::Walk { token, result } => self.finish_walk_request(token, result),
                UiEvent::Query { generation, result } => self.finish_query(generation, result),
                UiEvent::TraceIndex { token, result } => self.finish_trace_index(token, result),
                UiEvent::Trace {
                    token,
                    coordinate,
                    result,
                } => self.finish_trace(token, coordinate, result),
            }
        }
    }

    fn finish_walk_request(&mut self, token: WalkRequestToken, result: WalkRequestResult) {
        if token.generation != self.client_generation || self.walk_pending != Some(token) {
            return;
        }
        self.walk_pending = None;
        let kind = token.kind;
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
                self.response = None;
                self.status = ServiceStatus::Busy("walk server did not respond".to_string());
                self.notice = Some(format!(
                    "{} timed out after {}s; the walk server may be busy with a live step",
                    kind.label(),
                    crate::model::WALK_REQUEST_TIMEOUT.as_secs()
                ));
            }
            WalkRequestResult::ClientError(error) => {
                self.response = None;
                self.status = ServiceStatus::Error(error.clone());
                self.notice = Some(format!("{} request failed: {error}", kind.label()));
            }
        }
    }

    fn finish_query(&mut self, generation: u64, result: Result<WalkQuerySnapshot, String>) {
        if generation != self.client_generation || self.query_pending != Some(generation) {
            return;
        }
        self.query_pending = None;
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

    fn finish_trace_index(
        &mut self,
        token: TraceRequestToken,
        result: Result<EvaluationTraceIndex, String>,
    ) {
        if token.generation != self.client_generation || self.trace_index_pending != Some(token) {
            return;
        }
        self.trace_index_pending = None;
        match result {
            Ok(index) => {
                let selected = self.selected_campaign_id();
                if selected != Some(index.campaign.as_str()) {
                    self.notice = Some(format!(
                        "trace index campaign '{}' disagrees with selected campaign '{}'",
                        index.campaign,
                        selected.unwrap_or("-")
                    ));
                    return;
                }
                self.selected_trace = None;
                self.trace_snapshot = None;
                self.notice = Some(format!("{} completed run(s)", index.runs.len()));
                self.trace_index = Some(index);
            }
            Err(error) => {
                self.trace_index = None;
                self.selected_trace = None;
                self.trace_snapshot = None;
                self.notice = Some(error);
            }
        }
    }

    fn finish_trace(
        &mut self,
        token: TraceRequestToken,
        coordinate: EvaluationRunCoordinate,
        result: Result<EvaluationTraceSnapshot, String>,
    ) {
        if token.generation != self.client_generation
            || !self
                .trace_pending
                .as_ref()
                .is_some_and(|pending| pending.0 == token && pending.1 == coordinate)
        {
            return;
        }
        self.trace_pending = None;
        match result {
            Ok(snapshot) if snapshot.coordinate == coordinate => {
                let label = match &snapshot.trace {
                    EvaluationTraceState::Completed { .. } => "sealed trace",
                    EvaluationTraceState::NotCompleted { .. } => "lifecycle evidence",
                };
                self.notice = Some(format!("{label} {} loaded", coordinate.run_id));
                self.trace_snapshot = Some(snapshot);
            }
            Ok(snapshot) => {
                self.trace_snapshot = None;
                self.notice = Some(format!(
                    "trace response '{}' disagrees with requested run '{}'",
                    snapshot.coordinate.run_id, coordinate.run_id
                ));
            }
            Err(error) => {
                self.trace_snapshot = None;
                self.notice = Some(error);
            }
        }
    }

    fn next_trace_token(&mut self) -> TraceRequestToken {
        self.trace_serial = self.trace_serial.wrapping_add(1);
        TraceRequestToken {
            generation: self.client_generation,
            serial: self.trace_serial,
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

    fn status_snapshot(&self) -> Option<&WalkSessionSnapshot> {
        match self.response.as_ref() {
            Some(WalkResponse::Status { snapshot, .. }) => Some(snapshot),
            _ => None,
        }
    }

    fn handle_top_bar_action(&mut self, action: TopBarAction, ctx: &egui::Context) {
        if let Some(index) = action.selected_run {
            self.select_run(index);
        }
        if action.socket_changed {
            self.resolve_selected_client();
        }
        if action.refresh_runs {
            self.refresh_runs();
            if self.client.is_some() {
                self.refresh_health(Some(ctx.clone()));
            }
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

    fn handle_trace_action(&mut self, action: TraceAction, ctx: &egui::Context) {
        if action.refresh {
            self.refresh_trace_index(Some(ctx.clone()));
        }
        if let Some(coordinate) = action.load {
            self.load_trace(coordinate, Some(ctx.clone()));
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
                    position: self.status_snapshot().map(|snapshot| &snapshot.position),
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
                    walk_pending: self.walk_pending.map(|token| token.kind),
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
            ui.horizontal(|ui| {
                ui.selectable_value(
                    &mut self.center_view,
                    CenterView::Trace,
                    "Evaluation Traces",
                );
                ui.selectable_value(&mut self.center_view, CenterView::Query, "Database Query");
            });
            ui.separator();
            match self.center_view {
                CenterView::Trace => {
                    let action = TracePanel {
                        index: self.trace_index.as_ref(),
                        selected: &mut self.selected_trace,
                        snapshot: self.trace_snapshot.as_ref(),
                        index_pending: self.trace_index_pending.is_some(),
                        trace_pending: self.trace_pending.is_some(),
                    }
                    .show(ui);
                    self.handle_trace_action(action, &ctx);
                }
                CenterView::Query => {
                    let selected_campaign = self.selected_campaign_id().map(str::to_owned);
                    let action = QueryPanel {
                        campaign_input: &mut self.campaign_input,
                        query_script: &mut self.query_script,
                        query_pending: self.query_pending.is_some(),
                        query_result: self.query_result.as_ref(),
                        selected_row: &mut self.selected_row,
                        selected_campaign: selected_campaign.as_deref(),
                    }
                    .show(ui);
                    self.handle_query_action(action, &ctx);
                }
            }
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
                walk_pending: self.walk_pending.map(|token| token.kind),
                query_pending: self.query_pending.is_some(),
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

        harness.get_by_label("Database Query").click();
        harness.run();
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

        finish_walk_request(
            &mut app,
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
    fn authority_snapshot_is_retained_only_for_status_responses() {
        let epoch = ploke_eval::walk_client::ServerEpoch {
            protocol_version: 9,
            transition_graph_version: "walk-r0-r14a-v2".to_string(),
            repo_root: PathBuf::from("/tmp/ploke-parent"),
            exe_path: PathBuf::from("/tmp/ploke-eval"),
            exe_modified_unix_ms: Some(17),
            git_head: Some("abc123".to_string()),
            active_branch: Some("parent/runtime-1".to_string()),
            source_status_hash: Some("def456".to_string()),
        };
        let status = WalkResponse::Status {
            message: "online".to_string(),
            snapshot: WalkSessionSnapshot {
                position: ploke_eval::walk_client::WalkPosition::Reconstruction {
                    phase: ploke_eval::walk_client::WalkPhase::R4c,
                },
                controller_attached: true,
                authority: ploke_eval::walk_client::WalkAuthority::Active,
                job: None,
                blocker: None,
                actions: Vec::new(),
            },
            epoch: epoch.clone(),
        };
        let mut app = test_app();

        finish_walk_request(
            &mut app,
            WalkRequestKind::Health,
            WalkRequestResult::Response(status.clone()),
        );
        assert!(matches!(
            app.status_snapshot().map(|snapshot| &snapshot.position),
            Some(ploke_eval::walk_client::WalkPosition::Reconstruction {
                phase: ploke_eval::walk_client::WalkPhase::R4c
            })
        ));

        finish_walk_request(
            &mut app,
            WalkRequestKind::Show,
            WalkRequestResult::Response(WalkResponse::Ok {
                phase: ploke_eval::walk_client::WalkPhase::R4c,
                result: ploke_eval::walk_client::WalkOkPayload::Show {
                    report: "same reconstructed phase".to_string(),
                },
                epoch: epoch.clone(),
            }),
        );
        assert!(app.status_snapshot().is_none());

        finish_walk_request(
            &mut app,
            WalkRequestKind::Show,
            WalkRequestResult::Response(WalkResponse::Ok {
                phase: ploke_eval::walk_client::WalkPhase::R5,
                result: ploke_eval::walk_client::WalkOkPayload::Show {
                    report: "new phase".to_string(),
                },
                epoch: epoch.clone(),
            }),
        );
        assert!(app.status_snapshot().is_none());

        finish_walk_request(
            &mut app,
            WalkRequestKind::Health,
            WalkRequestResult::Response(status),
        );
        finish_walk_request(
            &mut app,
            WalkRequestKind::Health,
            WalkRequestResult::Offline(PathBuf::from("/tmp/walk.sock")),
        );
        assert!(app.status_snapshot().is_none());
    }

    #[test]
    fn failed_refresh_clears_authority_and_replaces_pending_notice() {
        let mut app = test_app();
        let status: WalkResponse = serde_json::from_value(serde_json::json!({
            "type": "status",
            "message": "online",
            "snapshot": {
                "phase": "r4c",
                "version": {
                    "session_id": null,
                    "cursor": null,
                    "journal_revision": 0
                },
                "position": {"source": "reconstruction", "phase": "r4c"},
                "controller_attached": true,
                "authority": "active",
                "job": null,
                "blocker": null,
                "actions": []
            },
            "epoch": {
                "protocol_version": 9,
                "transition_graph_version": "walk-r0-r14a-v2",
                "repo_root": "/tmp/ploke-parent",
                "exe_path": "/tmp/ploke-eval",
                "exe_modified_unix_ms": 17,
                "git_head": "abc123",
                "active_branch": "parent/runtime-1",
                "source_status_hash": "def456"
            }
        }))
        .expect("typed status response");
        finish_walk_request(
            &mut app,
            WalkRequestKind::Health,
            WalkRequestResult::Response(status.clone()),
        );
        assert!(app.status_snapshot().is_some());

        finish_walk_request(
            &mut app,
            WalkRequestKind::Health,
            WalkRequestResult::TimedOut,
        );
        assert!(app.response.is_none());
        assert!(app.status_snapshot().is_none());
        assert!(
            app.notice
                .as_deref()
                .is_some_and(|notice| notice.contains("timed out"))
        );

        finish_walk_request(
            &mut app,
            WalkRequestKind::Health,
            WalkRequestResult::Response(status),
        );
        finish_walk_request(
            &mut app,
            WalkRequestKind::Health,
            WalkRequestResult::ClientError("protocol mismatch".to_string()),
        );
        assert!(app.response.is_none());
        assert!(app.status_snapshot().is_none());
        assert_eq!(
            app.notice.as_deref(),
            Some("health request failed: protocol mismatch")
        );
    }

    #[test]
    fn response_from_previous_run_is_discarded_after_selection() {
        let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        let mut app = test_app();
        app.socket_input = unique_temp_dir("ploke-walk-ui-generation")
            .join("walk.sock")
            .display()
            .to_string();
        app.runs = vec![test_run("run-a", &repo_root), test_run("run-b", &repo_root)];
        app.selected_run = Some(0);
        app.resolve_selected_client();
        let previous = WalkRequestToken {
            generation: app.client_generation,
            kind: WalkRequestKind::Health,
        };
        app.walk_pending = Some(previous);
        app.query_pending = Some(previous.generation);

        app.select_run(1);

        let current = app.walk_pending.expect("new run health request pending");
        assert_ne!(current.generation, previous.generation);
        assert_eq!(current.kind, WalkRequestKind::Health);
        app.finish_walk_request(
            previous,
            WalkRequestResult::ClientError("response from run A".to_string()),
        );
        app.finish_query(
            previous.generation,
            Err("query response from run A".to_string()),
        );
        assert_eq!(app.selected_campaign_id(), Some("run-b"));
        assert_eq!(app.walk_pending, Some(current));
        assert!(app.query_pending.is_none());
        assert!(!matches!(app.status, ServiceStatus::Error(_)));
        assert!(
            app.notice
                .as_deref()
                .is_none_or(|notice| !notice.contains("run A"))
        );
    }

    #[test]
    fn trace_reply_from_previous_run_is_discarded_after_selection() {
        let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        let mut app = test_app();
        app.socket_input = unique_temp_dir("ploke-walk-ui-trace-generation")
            .join("walk.sock")
            .display()
            .to_string();
        app.runs = vec![test_run("run-a", &repo_root), test_run("run-b", &repo_root)];
        app.selected_run = Some(0);
        app.resolve_selected_client();
        let coordinate = test_coordinate("run-a", "evaluation-a");
        let index_token = app.next_trace_token();
        let trace_token = app.next_trace_token();
        app.trace_index_pending = Some(index_token);
        app.trace_pending = Some((trace_token, coordinate.clone()));

        app.select_run(1);

        app.finish_trace_index(index_token, Err("trace index from run A".to_string()));
        app.finish_trace(
            trace_token,
            coordinate,
            Err("trace response from run A".to_string()),
        );
        assert_eq!(app.selected_campaign_id(), Some("run-b"));
        assert!(app.trace_index.is_none());
        assert!(app.trace_snapshot.is_none());
        assert!(app.trace_index_pending.is_none());
        assert!(app.trace_pending.is_none());
        assert!(
            app.notice
                .as_deref()
                .is_none_or(|notice| !notice.contains("run A"))
        );
    }

    #[test]
    fn same_run_trace_reply_requires_current_request_serial() {
        let mut app = test_app();
        let coordinate = test_coordinate("run-a", "evaluation-a");
        let old = app.next_trace_token();
        app.trace_pending = Some((old, coordinate.clone()));

        app.trace_pending = None;
        let current = app.next_trace_token();
        app.trace_pending = Some((current, coordinate.clone()));
        app.notice = Some("new request pending".to_string());

        app.finish_trace(old, coordinate, Err("stale same-run response".to_string()));

        assert_eq!(
            app.trace_pending.as_ref().map(|pending| pending.0),
            Some(current)
        );
        assert_eq!(app.notice.as_deref(), Some("new request pending"));
        assert!(app.trace_snapshot.is_none());
    }

    #[test]
    fn socket_edit_rebinds_client_and_invalidates_pending_evidence() {
        let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        let mut app = test_app();
        app.runs = vec![test_run("run-a", &repo_root)];
        app.selected_run = Some(0);
        let first = unique_temp_dir("ploke-walk-ui-socket-a").join("walk.sock");
        app.socket_input = first.display().to_string();
        app.resolve_selected_client();
        let generation = app.client_generation;
        app.walk_pending = Some(WalkRequestToken {
            generation,
            kind: WalkRequestKind::Health,
        });
        let second = unique_temp_dir("ploke-walk-ui-socket-b").join("walk.sock");
        app.socket_input = second.display().to_string();

        app.handle_top_bar_action(
            TopBarAction {
                socket_changed: true,
                ..TopBarAction::default()
            },
            &egui::Context::default(),
        );

        assert_ne!(app.client_generation, generation);
        assert!(app.walk_pending.is_none());
        assert!(app.response.is_none());
        assert_eq!(
            app.client.as_ref().map(WalkClient::socket),
            Some(second.as_path())
        );
        assert!(matches!(app.status, ServiceStatus::Unresolved));
    }

    #[test]
    fn failed_run_resolution_keeps_the_error_visible() {
        let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        let mut app = test_app();
        app.runs = vec![test_run("run-a", &repo_root)];
        app.socket_input = format!("/tmp/{}.sock", "x".repeat(256));
        app.notice = Some("stale notice".to_string());

        app.select_run(0);

        assert!(app.client.is_none());
        assert!(app.walk_pending.is_none());
        assert!(matches!(app.status, ServiceStatus::Error(_)));
        assert!(
            app.notice
                .as_deref()
                .is_some_and(|notice| notice.starts_with("socket resolution failed:")),
            "resolution error must replace stale evidence: {:?}",
            app.notice
        );
    }

    #[test]
    fn query_result_retains_phase_session_and_epoch_envelope() {
        let query = test_query_snapshot();
        let mut app = test_app();

        finish_query(&mut app, Ok(query));

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
    fn failed_query_replacement_does_not_retain_old_rows() {
        let root = unique_temp_dir("ploke-walk-ui-query-failure");
        fs::create_dir_all(&root).expect("create temp repo root");
        let socket = root.join("missing.sock");
        let client = WalkClient::resolve(Some(&root), Some(&socket)).expect("resolve client");
        let mut app = test_app_with_client(client);
        app.campaign_input = "replacement-campaign".to_string();
        app.query_script = "::relations".to_string();
        app.query_result = Some(test_query_snapshot());
        app.selected_row = Some(0);

        app.run_query(None);

        assert!(app.query_result.is_none());
        assert!(app.selected_row.is_none());
        wait_for_events(&mut app);
        assert!(app.query_result.is_none());
        assert!(
            app.notice
                .as_deref()
                .is_some_and(|notice| !notice.contains("1 row"))
        );
        let _ = fs::remove_dir_all(root);
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
            trace_index: None,
            selected_trace: None,
            trace_snapshot: None,
            notice: None,
            client_generation: 0,
            walk_pending: None,
            query_pending: None,
            trace_index_pending: None,
            trace_pending: None,
            trace_serial: 0,
            center_view: CenterView::default(),
            debug_panel: false,
            debug_hover: false,
            buttons: UiButtonState::default(),
        }
    }

    fn wait_for_events(app: &mut WalkUiApp) {
        let deadline = Instant::now() + Duration::from_secs(4);
        while Instant::now() < deadline {
            app.poll_events();
            if app.walk_pending.is_none()
                && app.query_pending.is_none()
                && app.trace_index_pending.is_none()
                && app.trace_pending.is_none()
            {
                return;
            }
            thread::sleep(Duration::from_millis(10));
        }
        app.poll_events();
    }

    fn finish_walk_request(app: &mut WalkUiApp, kind: WalkRequestKind, result: WalkRequestResult) {
        let token = WalkRequestToken {
            generation: app.client_generation,
            kind,
        };
        app.walk_pending = Some(token);
        app.finish_walk_request(token, result);
    }

    fn finish_query(app: &mut WalkUiApp, result: Result<WalkQuerySnapshot, String>) {
        let generation = app.client_generation;
        app.query_pending = Some(generation);
        app.finish_query(generation, result);
    }

    fn test_run(campaign: &str, repo_root: &std::path::Path) -> WalkRunEntry {
        let campaign_dir = repo_root.join("target").join("walk-ui-test").join(campaign);
        WalkRunEntry {
            campaign_id: campaign.to_string(),
            prototype1_root: campaign_dir.join("prototype1"),
            owner_db_path: campaign_dir.join("prototype1/owner.cozo.sqlite"),
            campaign_dir,
            worktree_root: Some(repo_root.to_path_buf()),
            modified_unix_ms: None,
            has_manifest: true,
            has_closure_state: false,
            has_owner_db: false,
            has_parent_identity: true,
        }
    }

    fn test_query_snapshot() -> WalkQuerySnapshot {
        serde_json::from_value(serde_json::json!({
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
        .expect("typed query envelope")
    }

    fn test_coordinate(campaign: &str, run: &str) -> EvaluationRunCoordinate {
        serde_json::from_value(serde_json::json!({
            "campaign": campaign,
            "instance": "org__repo-1",
            "run_id": run
        }))
        .expect("evaluation run coordinate")
    }

    fn unique_temp_dir(prefix: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time")
            .as_nanos();
        std::env::temp_dir().join(format!("{prefix}-{}-{nanos}", std::process::id()))
    }
}
