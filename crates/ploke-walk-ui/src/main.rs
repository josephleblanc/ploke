use std::{collections::BTreeMap, path::PathBuf, sync::mpsc, thread, time::Duration};

use eframe::egui::{
    self, Color32, ComboBox, Grid, RichText, ScrollArea, TextEdit, TextStyle, Ui, ViewportBuilder,
};
use ploke_eval::walk_client::{
    DbQueryResult, PhaseInfo, PhaseInventory, WalkClient, WalkPhase, WalkReplyStatus, WalkRunEntry,
    WalkSnapshot,
};
const DEFAULT_QUERY: &str = "::relations";
const MAX_TABLE_ROWS: usize = 200;
const RUN_LABEL_MAX_CHARS: usize = 48;
const WALK_REQUEST_TIMEOUT: Duration = Duration::from_secs(2);

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: ViewportBuilder::default().with_inner_size([1280.0, 820.0]),
        ..Default::default()
    };
    eframe::run_native(
        "ploke-walk-ui",
        options,
        Box::new(|_cc| Ok(Box::new(WalkUiApp::new()))),
    )
}

struct WalkUiApp {
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
    snapshot: Option<WalkSnapshot>,
    query_result: Option<DbQueryResult>,
    selected_row: Option<usize>,
    notice: Option<String>,
    walk_pending: Option<WalkRequestKind>,
    query_pending: bool,
    debug_panel: bool,
    debug_hover: bool,
    buttons: UiButtonState,
}

#[derive(Debug, Clone, Copy, Default)]
struct UiButtonState {
    run_details: bool,
}

#[derive(Debug, Clone)]
enum ServiceStatus {
    Unresolved,
    Offline,
    Online,
    Busy(String),
    Error(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WalkRequestKind {
    Health,
    Show,
}

enum UiEvent {
    Walk {
        kind: WalkRequestKind,
        result: WalkRequestResult,
    },
    Query(Result<DbQueryResult, String>),
}

enum WalkRequestResult {
    Snapshot(WalkSnapshot),
    Offline(PathBuf),
    TimedOut,
    Error(String),
}

impl WalkRequestKind {
    fn label(self) -> &'static str {
        match self {
            Self::Health => "health",
            Self::Show => "show",
        }
    }
}

impl WalkUiApp {
    fn new() -> Self {
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
            snapshot: None,
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
            self.snapshot = None;
            self.status = ServiceStatus::Unresolved;
            self.notice = Some("no Prototype 1 run selected".to_string());
            return;
        };

        self.campaign_input = run.campaign_id.clone();
        if run.worktree_root.is_none() {
            self.client = None;
            self.snapshot = None;
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
            let result = run_walk_request(kind, client);
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

        self.query_pending = true;
        self.notice = Some("query pending".to_string());
        let script = self.query_script.clone();
        let tx = self.event_tx.clone();
        thread::spawn(move || {
            let result = WalkClient::query_campaign_db(&campaign, &script)
                .map_err(|error| error.to_string());
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
            WalkRequestResult::Snapshot(snapshot) => {
                self.status = if snapshot.status == WalkReplyStatus::Error {
                    ServiceStatus::Error(snapshot.message.clone())
                } else {
                    ServiceStatus::Online
                };
                self.notice = Some(format!("{} response received", kind.label()));
                self.snapshot = Some(snapshot);
            }
            WalkRequestResult::Offline(socket) => {
                self.status = ServiceStatus::Offline;
                self.snapshot = None;
                self.notice = Some(format!("walk server is offline at {}", socket.display()));
            }
            WalkRequestResult::TimedOut => {
                self.status = ServiceStatus::Busy("walk server did not respond".to_string());
                self.notice = Some(format!(
                    "{} timed out after {}s; the walk server may be busy with a live step",
                    kind.label(),
                    WALK_REQUEST_TIMEOUT.as_secs()
                ));
            }
            WalkRequestResult::Error(error) => {
                self.status = ServiceStatus::Error(error);
            }
        }
    }

    fn finish_query(&mut self, result: Result<DbQueryResult, String>) {
        self.query_pending = false;
        match result {
            Ok(result) => {
                self.selected_row = None;
                self.notice = Some(format!("{} row(s)", result.row_count));
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

    fn current_phase(&self) -> Option<WalkPhase> {
        self.snapshot.as_ref().and_then(|snapshot| snapshot.phase)
    }
}

impl eframe::App for WalkUiApp {
    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        self.poll_events();
        egui::Panel::top("top_bar").show_inside(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.label(status_text(&self.status));
                if let ServiceStatus::Error(error) = &self.status {
                    ui.label(RichText::new(error).color(Color32::LIGHT_RED));
                }
                ui.separator();
                ui.label("run");
                let mut selected = self.selected_run;
                ComboBox::from_id_salt("ploke-walk-ui.run-picker")
                    .width(440.0)
                    .selected_text(selected_run_label(self.selected_run()))
                    .show_ui(ui, |ui| {
                        for (index, run) in self.runs.iter().enumerate() {
                            ui.selectable_value(&mut selected, Some(index), run_menu_label(run));
                        }
                    });
                if selected != self.selected_run
                    && let Some(index) = selected
                {
                    self.select_run(index);
                }
                if ui.button("Refresh Runs").clicked() {
                    self.refresh_runs();
                    self.refresh_health(Some(ui.ctx().clone()));
                }
                ui.separator();
                ui.label("socket");
                ui.add_sized([260.0, 22.0], TextEdit::singleline(&mut self.socket_input));
                ui.toggle_value(&mut self.debug_panel, "Debug");
            });
        });

        egui::Panel::left("phase_rail")
            .resizable(true)
            .default_size(280.0)
            .show_inside(ui, |ui| self.phase_rail(ui));

        egui::Panel::right("details")
            .resizable(true)
            .default_size(360.0)
            .show_inside(ui, |ui| self.details_panel(ui));

        egui::CentralPanel::default().show_inside(ui, |ui| self.query_panel(ui));

        if self.debug_panel {
            self.debug_window(ui.ctx());
        }
    }
}

impl WalkUiApp {
    fn phase_rail(&mut self, ui: &mut Ui) {
        ui.heading("Phases");
        ui.add_space(6.0);
        let current = self.current_phase();
        ScrollArea::vertical().show(ui, |ui| {
            for phase in &self.phases.phases {
                phase_row(ui, phase, current);
            }
        });
    }

    fn query_panel(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            ui.label("campaign");
            ui.add_sized(
                [320.0, 22.0],
                TextEdit::singleline(&mut self.campaign_input),
            );
            if ui.button("Use Selected").clicked()
                && let Some(campaign) = self.selected_campaign_id()
            {
                self.campaign_input = campaign.to_string();
            }
            if ui
                .add_enabled(!self.query_pending, egui::Button::new("Run Query"))
                .clicked()
            {
                self.run_query(Some(ui.ctx().clone()));
            }
            if ui.button("Relations").clicked() {
                self.query_script = DEFAULT_QUERY.to_string();
            }
            if self.query_pending {
                ui.label(RichText::new("query...").color(Color32::YELLOW));
            }
        });
        ui.add_space(8.0);
        ui.add_sized(
            [ui.available_width(), 150.0],
            TextEdit::multiline(&mut self.query_script)
                .font(TextStyle::Monospace)
                .desired_rows(7),
        );
        ui.add_space(8.0);
        ui.separator();
        ui.add_space(8.0);
        let result = self.query_result.as_ref();
        match result {
            Some(result) => query_result_table(ui, result, &mut self.selected_row),
            None => {
                ui.label(RichText::new("No query result").color(Color32::GRAY));
            }
        }
    }

    fn details_panel(&mut self, ui: &mut Ui) {
        ui.horizontal_top(|ui| {
            ui.heading("Run");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::TOP), |ui| {
                if ui.button("Details").clicked() {
                    self.buttons.run_details = !self.buttons.run_details;
                }
            });
        });

        if self.buttons.run_details {
            ui.add_space(6.0);
            if let Some(run) = self.selected_run() {
                ui.label(&run.campaign_id);
                ui.label(format!("campaign: {}", run.campaign_dir.display()));
                ui.label(format!("prototype1: {}", run.prototype1_root.display()));
                if let Some(worktree) = run.worktree_root.as_deref() {
                    ui.label(format!("worktree: {}", worktree.display()));
                } else {
                    ui.label(RichText::new("worktree: missing").color(Color32::YELLOW));
                }
                ui.label(format!(
                    "db: {}",
                    if run.has_owner_db {
                        "present"
                    } else {
                        "missing"
                    }
                ));
                ui.label(format!(
                    "parent identity: {}",
                    if run.has_parent_identity {
                        "present"
                    } else {
                        "missing"
                    }
                ));
            } else {
                ui.label("No run selected");
            }
            if self.runs.is_empty() {
                ui.label("No Prototype 1 runs found under the ploke-eval home");
            }
            if let Some(error) = self.run_error.as_deref() {
                ui.label(RichText::new(error).color(Color32::LIGHT_RED));
            }
        }
        ui.separator();

        ui.horizontal_wrapped(|ui| {
            ui.horizontal_top(|ui| {
                ui.heading("Walk");
            });
            ui.with_layout(egui::Layout::right_to_left(egui::Align::TOP), |ui| {
                let walk_idle = self.walk_pending.is_none();
                if ui
                    .add_enabled(walk_idle, egui::Button::new("Health"))
                    .clicked()
                {
                    self.refresh_health(Some(ui.ctx().clone()));
                }
                let show_response = ui.add_enabled(
                    self.client.is_some() && walk_idle,
                    egui::Button::new("Show"),
                );
                if show_response.clicked() {
                    self.show_state(Some(ui.ctx().clone()));
                }
                if let Some(kind) = self.walk_pending {
                    ui.label(RichText::new(format!("{}...", kind.label())).color(Color32::YELLOW));
                }
            });
        });

        ui.add_space(6.0);
        if let Some(snapshot) = &self.snapshot {
            ui.label(format!(
                "phase: {}",
                snapshot.phase_id.as_deref().unwrap_or("unknown")
            ));
            if let Some(label) = snapshot.phase_label.as_deref() {
                ui.label(label);
            }
            ui.label(format!("protocol: {}", snapshot.epoch.protocol_version));
            ui.label(format!(
                "graph: {}",
                snapshot.epoch.transition_graph_version
            ));
            if let Some(head) = snapshot.epoch.git_head.as_deref() {
                ui.label(format!("git: {}", short_hash(head)));
            }
            ui.add_space(8.0);
            ScrollArea::vertical()
                .id_salt("walk_message")
                .max_height(180.0)
                .show(ui, |ui| {
                    ui.monospace(&snapshot.message);
                });
        } else {
            ui.label("No walk snapshot");
        }
        ui.separator();
        if let Some(notice) = self.notice.as_deref() {
            ui.label(notice);
        }
        ui.separator();
        if let (Some(result), Some(row)) = (&self.query_result, self.selected_row)
            && let Some(row) = result.rows.get(row)
        {
            ui.label("row");
            let text = serde_json::to_string_pretty(&row.object)
                .unwrap_or_else(|_| row.object.to_string());
            ScrollArea::vertical()
                .id_salt("row_detail")
                .show(ui, |ui| ui.monospace(text));
        }
    }

    fn debug_window(&mut self, ctx: &egui::Context) {
        egui::Window::new("Debug")
            .id(egui::Id::new("ploke-walk-ui.debug-window"))
            .default_width(460.0)
            .show(ctx, |ui| {
                if ui
                    .checkbox(&mut self.debug_hover, "widget info on hover")
                    .changed()
                {
                    ctx.set_debug_on_hover(self.debug_hover);
                }
                ui.separator();
                Grid::new("debug_state_grid").striped(true).show(ui, |ui| {
                    ui.label("status");
                    ui.label(status_plain(&self.status));
                    ui.end_row();
                    ui.label("selected_run");
                    ui.label(self.selected_campaign_id().unwrap_or("none").to_string());
                    ui.end_row();
                    ui.label("client_socket");
                    ui.label(
                        self.client
                            .as_ref()
                            .map(|client| client.socket().display().to_string())
                            .unwrap_or_else(|| "none".to_string()),
                    );
                    ui.end_row();
                    ui.label("walk_pending");
                    ui.label(
                        self.walk_pending
                            .map(WalkRequestKind::label)
                            .unwrap_or("none"),
                    );
                    ui.end_row();
                    ui.label("query_pending");
                    ui.label(self.query_pending.to_string());
                    ui.end_row();
                    ui.label("runs");
                    ui.label(self.runs.len().to_string());
                    ui.end_row();
                    ui.label("rows");
                    ui.label(
                        self.query_result
                            .as_ref()
                            .map(|result| result.row_count.to_string())
                            .unwrap_or_else(|| "none".to_string()),
                    );
                    ui.end_row();
                });
                ui.separator();
                ScrollArea::vertical()
                    .id_salt("egui_debug_scroll")
                    .max_height(420.0)
                    .show(ui, |ui| {
                        ui.heading("Inspection");
                        ctx.inspection_ui(ui);
                        ui.separator();
                        ui.heading("Settings");
                        ctx.settings_ui(ui);
                        ui.separator();
                        ui.heading("Memory");
                        ctx.memory_ui(ui);
                    });
            });
    }
}

fn phase_row(ui: &mut Ui, phase: &PhaseInfo, current: Option<WalkPhase>) {
    let is_current = current == Some(phase.phase);
    let fill = if is_current {
        Color32::from_rgb(35, 74, 92)
    } else {
        Color32::from_rgb(31, 31, 34)
    };
    let stroke = if is_current {
        Color32::from_rgb(113, 180, 166)
    } else {
        Color32::from_rgb(62, 62, 66)
    };
    egui::Frame::new()
        .fill(fill)
        .stroke(egui::Stroke::new(1.0, stroke))
        .inner_margin(egui::Margin::symmetric(8, 6))
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.label(RichText::new(&phase.id).monospace().strong());
                ui.label(&phase.label);
            });
            for next in &phase.next {
                ui.label(
                    RichText::new(format!("-> {} ({})", next.phase_id, next.edge))
                        .small()
                        .color(Color32::LIGHT_GRAY),
                );
            }
        });
    ui.add_space(5.0);
}

fn query_result_table(ui: &mut Ui, result: &DbQueryResult, selected_row: &mut Option<usize>) {
    ui.horizontal(|ui| {
        ui.label(format!("rows: {}", result.row_count));
        ui.separator();
        ui.label(result.db_path.display().to_string());
    });
    ui.add_space(6.0);
    if result.headers.is_empty() {
        ui.label("No headers");
        return;
    }
    ScrollArea::both()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            Grid::new("query_result_grid")
                .striped(true)
                .min_col_width(120.0)
                .show(ui, |ui| {
                    ui.label(RichText::new("#").strong());
                    for header in &result.headers {
                        ui.label(RichText::new(header).strong());
                    }
                    ui.end_row();
                    for (index, row) in result.rows.iter().take(MAX_TABLE_ROWS).enumerate() {
                        let selected = *selected_row == Some(index);
                        if ui.selectable_label(selected, index.to_string()).clicked() {
                            *selected_row = Some(index);
                        }
                        for cell in &row.cells {
                            ui.label(format_cell(cell));
                        }
                        ui.end_row();
                    }
                });
        });
}

fn run_walk_request(kind: WalkRequestKind, client: WalkClient) -> WalkRequestResult {
    let socket = client.socket().to_path_buf();
    let runtime = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(error) => {
            return WalkRequestResult::Error(format!("failed to create tokio runtime: {error}"));
        }
    };

    runtime.block_on(async move {
        match kind {
            WalkRequestKind::Health => {
                match tokio::time::timeout(WALK_REQUEST_TIMEOUT, client.health()).await {
                    Ok(Ok(Some(snapshot))) => WalkRequestResult::Snapshot(snapshot),
                    Ok(Ok(None)) => WalkRequestResult::Offline(socket),
                    Ok(Err(error)) => WalkRequestResult::Error(error.to_string()),
                    Err(_) => WalkRequestResult::TimedOut,
                }
            }
            WalkRequestKind::Show => {
                match tokio::time::timeout(WALK_REQUEST_TIMEOUT, client.health()).await {
                    Ok(Ok(Some(_))) => {}
                    Ok(Ok(None)) => return WalkRequestResult::Offline(socket),
                    Ok(Err(error)) => return WalkRequestResult::Error(error.to_string()),
                    Err(_) => return WalkRequestResult::TimedOut,
                }
                match tokio::time::timeout(WALK_REQUEST_TIMEOUT, client.show()).await {
                    Ok(Ok(snapshot)) => WalkRequestResult::Snapshot(snapshot),
                    Ok(Err(error)) => WalkRequestResult::Error(error.to_string()),
                    Err(_) => WalkRequestResult::TimedOut,
                }
            }
        }
    })
}

fn status_text(status: &ServiceStatus) -> RichText {
    match status {
        ServiceStatus::Unresolved => RichText::new("unresolved").color(Color32::GRAY),
        ServiceStatus::Offline => RichText::new("offline").color(Color32::YELLOW),
        ServiceStatus::Online => RichText::new("online").color(Color32::GREEN),
        ServiceStatus::Busy(_) => RichText::new("busy").color(Color32::YELLOW),
        ServiceStatus::Error(_) => RichText::new("error").color(Color32::RED),
    }
}

fn status_plain(status: &ServiceStatus) -> String {
    match status {
        ServiceStatus::Unresolved => "unresolved".to_string(),
        ServiceStatus::Offline => "offline".to_string(),
        ServiceStatus::Online => "online".to_string(),
        ServiceStatus::Busy(detail) => format!("busy: {detail}"),
        ServiceStatus::Error(error) => format!("error: {error}"),
    }
}

fn selected_run_label(run: Option<&WalkRunEntry>) -> String {
    run.map(|run| elide_middle(&run.campaign_id, RUN_LABEL_MAX_CHARS))
        .unwrap_or_else(|| "Select run".to_string())
}

fn run_menu_label(run: &WalkRunEntry) -> String {
    let mut flags = Vec::new();
    flags.push(if run.has_owner_db { "db" } else { "no db" });
    flags.push(if run.worktree_root.is_some() {
        "worktree"
    } else {
        "no worktree"
    });
    flags.push(if run.has_parent_identity {
        "identity"
    } else {
        "no identity"
    });
    format!(
        "{}  |  {}",
        elide_middle(&run.campaign_id, RUN_LABEL_MAX_CHARS),
        flags.join(", ")
    )
}

fn nonempty_path(text: &str) -> Option<PathBuf> {
    let trimmed = text.trim();
    (!trimmed.is_empty()).then(|| PathBuf::from(trimmed))
}

fn optional_text(text: &str) -> Option<&str> {
    let trimmed = text.trim();
    (!trimmed.is_empty()).then_some(trimmed)
}

fn format_cell(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::Null => "null".to_string(),
        serde_json::Value::String(text) => text.clone(),
        other => other.to_string(),
    }
}

fn short_hash(text: &str) -> &str {
    text.get(..12).unwrap_or(text)
}

fn elide_middle(value: &str, max_chars: usize) -> String {
    let len = value.chars().count();
    if len <= max_chars || max_chars < 5 {
        return value.to_owned();
    }

    let prefix_len = (max_chars - 1) / 2;
    let suffix_len = max_chars - prefix_len - 1;
    let prefix: String = value.chars().take(prefix_len).collect();
    let suffix: String = value
        .chars()
        .rev()
        .take(suffix_len)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();
    format!("{prefix}.{suffix}")
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        time::{Instant, SystemTime, UNIX_EPOCH},
    };

    use super::*;

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
        assert!(app.snapshot.is_none());
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
            client: Some(client),
            phases: PhaseInventory::current(),
            status: ServiceStatus::Unresolved,
            snapshot: None,
            query_result: None,
            selected_row: None,
            notice: None,
            walk_pending: None,
            query_pending: false,
            debug_panel: false,
            debug_hover: false,
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
