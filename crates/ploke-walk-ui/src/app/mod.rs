mod panels;

use std::{
    path::Path,
    sync::mpsc,
    thread,
    time::{Duration, Instant},
};

use eframe::egui;
use ploke_eval::setup_client::{CampaignId, RunSetupReceipt, RunSetupRequest};
use ploke_eval::walk_client::{
    AdvertisedStep, EvaluationRunCoordinate, EvaluationTraceIndex, EvaluationTraceSnapshot,
    EvaluationTraceState, LlmTraceCoordinate, LlmTraceIndex, LlmTraceSnapshot, OperationId,
    PhaseInventory, RunMode, WalkClient, WalkConfigSnapshot, WalkEventProjection, WalkJobSnapshot,
    WalkJobStatus, WalkPhase, WalkQuerySnapshot, WalkResponse, WalkRunEntry, WalkSessionSnapshot,
    WalkTransitionReceipt,
};

use crate::client;
use crate::model::{
    AutoMode, CenterView, DEFAULT_QUERY, HANDOFF_WAIT_TIMEOUT, HandoffContinuation, HandoffWait,
    OperationIntent, OperationStage, OperationToken, PendingOperation, ServiceStatus, SetupKind,
    SetupPending, SetupToken, TraceRequestToken, UiButtonState, UiEvent, WalkRequestKind,
    WalkRequestResult, WalkRequestToken, nonempty_path, optional_text,
};
use panels::{
    config::{ConfigAction, ConfigPanel},
    debug::DebugWindow,
    details::{DetailsAction, DetailsPanel, strict_step},
    llm_trace::{LlmTraceAction, LlmTracePanel},
    phase_rail::PhaseRail,
    query::{QueryAction, QueryPanel},
    setup::{ReviewedSetup, SetupAction, SetupDraft, SetupPanel},
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
    config_snapshot: Option<WalkConfigSnapshot>,
    query_result: Option<WalkQuerySnapshot>,
    selected_row: Option<usize>,
    trace_index: Option<EvaluationTraceIndex>,
    selected_trace: Option<usize>,
    trace_snapshot: Option<EvaluationTraceSnapshot>,
    llm_index: Option<LlmTraceIndex>,
    selected_llm: Option<String>,
    llm_snapshot: Option<LlmTraceSnapshot>,
    notice: Option<String>,
    client_generation: u64,
    walk_pending: Option<WalkRequestToken>,
    config_pending: Option<u64>,
    query_pending: Option<u64>,
    trace_index_pending: Option<TraceRequestToken>,
    trace_pending: Option<(TraceRequestToken, EvaluationRunCoordinate)>,
    llm_index_pending: Option<TraceRequestToken>,
    llm_pending: Option<(TraceRequestToken, LlmTraceCoordinate)>,
    trace_serial: u64,
    center_view: CenterView,
    debug_panel: bool,
    debug_hover: bool,
    buttons: UiButtonState,
    setup: SetupDraft,
    reviewed: Option<ReviewedSetup>,
    setup_receipt: Option<RunSetupReceipt>,
    setup_pending: Option<SetupPending>,
    setup_serial: u64,
    allow_live: bool,
    allow_git: bool,
    operation: Option<PendingOperation>,
    operation_serial: u64,
    updates: Vec<WalkResponse>,
    auto: AutoMode,
    status_serial: u64,
}

impl WalkUiApp {
    pub(crate) fn new() -> Self {
        let (event_tx, event_rx) = mpsc::channel();
        let repo_root = std::env::current_dir().unwrap_or_else(|_| ".".into());
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
            config_snapshot: None,
            query_result: None,
            selected_row: None,
            trace_index: None,
            selected_trace: None,
            trace_snapshot: None,
            llm_index: None,
            selected_llm: None,
            llm_snapshot: None,
            notice: None,
            client_generation: 0,
            walk_pending: None,
            config_pending: None,
            query_pending: None,
            trace_index_pending: None,
            trace_pending: None,
            llm_index_pending: None,
            llm_pending: None,
            trace_serial: 0,
            center_view: CenterView::default(),
            debug_panel: false,
            debug_hover: false,
            buttons: UiButtonState::default(),
            setup: SetupDraft::for_root(&repo_root),
            reviewed: None,
            setup_receipt: None,
            setup_pending: None,
            setup_serial: 0,
            allow_live: false,
            allow_git: false,
            operation: None,
            operation_serial: 0,
            updates: Vec::new(),
            auto: AutoMode::default(),
            status_serial: 0,
        };
        app.refresh_runs();
        if app.client.is_some() {
            app.refresh_health(None);
        }
        app
    }

    fn refresh_runs(&mut self) {
        if let Some(reason) = self.run_blocker() {
            self.notice = Some(reason);
            return;
        }
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
        if let Some(reason) = self.run_blocker() {
            self.notice = Some(reason);
            return;
        }
        self.selected_run = Some(index);
        self.resolve_selected_client();
        if self.client.is_some() {
            self.refresh_health(None);
        }
    }

    fn resolve_selected_client(&mut self) {
        if let Some(reason) = self.socket_blocker() {
            self.notice = Some(reason);
            return;
        }
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
                "selected run '{}' has no exact local checkout binding; this UI cannot query or control it",
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

    fn run_blocker(&self) -> Option<String> {
        self.socket_blocker()
    }

    fn socket_blocker(&self) -> Option<String> {
        if let Some(pending) = self.operation.as_ref() {
            return Some(match pending.token.operation {
                Some(operation) => format!(
                    "walk operation {operation} is unresolved; retain this run and socket until its terminal receipt is recovered"
                ),
                None => {
                    "idle-server stop is unresolved; wait for its response before changing the run or socket"
                        .to_string()
                }
            });
        }
        if matches!(
            &self.auto,
            AutoMode::Refreshing { .. }
                | AutoMode::LoadingConfig { .. }
                | AutoMode::CheckingSuccessor { .. }
                | AutoMode::Running
                | AutoMode::Stopping
                | AutoMode::Paused
                | AutoMode::Waiting(_)
        ) {
            return Some(
                "UI-local auto-advance is active or paused; use End local automation before changing the run or socket"
                    .to_string(),
            );
        }
        self.setup_pending
            .filter(|pending| pending.kind == SetupKind::Admit)
            .map(|_| {
                "setup admission is still running; wait for its receipt before changing the run or socket"
                    .to_string()
            })
    }

    fn setup_blocked(&self) -> bool {
        self.operation.is_some()
            || self.walk_pending.is_some()
            || !matches!(&self.auto, AutoMode::Idle)
            || matches!(&self.status, ServiceStatus::Online | ServiceStatus::Busy(_))
    }

    fn invalidate_client_state(&mut self) {
        self.client_generation = self.client_generation.wrapping_add(1);
        if self.operation.is_none() {
            self.auto = AutoMode::Idle;
        }
        self.updates.clear();
        self.allow_live = false;
        self.allow_git = false;
        self.walk_pending = None;
        self.config_pending = None;
        if !self
            .setup_pending
            .is_some_and(|pending| pending.kind == SetupKind::Admit)
        {
            self.setup_pending = None;
        }
        self.query_pending = None;
        self.trace_index_pending = None;
        self.trace_pending = None;
        self.llm_index_pending = None;
        self.llm_pending = None;
        self.response = None;
        self.config_snapshot = None;
        self.query_result = None;
        self.selected_row = None;
        self.trace_index = None;
        self.selected_trace = None;
        self.trace_snapshot = None;
        self.llm_index = None;
        self.selected_llm = None;
        self.llm_snapshot = None;
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

    fn refresh_config(&mut self, repaint: Option<egui::Context>) {
        let Some(client) = self.client.clone() else {
            self.notice = Some("select a run with an available walk service".to_string());
            return;
        };
        if self.config_pending.is_some() {
            self.notice = Some("configuration request already pending".to_string());
            return;
        }

        let generation = self.client_generation;
        self.config_pending = Some(generation);
        self.notice = Some("configuration request pending".to_string());
        let tx = self.event_tx.clone();
        thread::spawn(move || {
            let result = client::config(client);
            let _ = tx.send(UiEvent::Config {
                generation,
                result: result.map(Box::new),
            });
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

    fn run_evidence(
        &mut self,
        view: ploke_eval::walk_client::WalkEvidenceQuery,
        repaint: Option<egui::Context>,
    ) {
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
        self.notice = Some(format!("{view:?} evidence query pending"));
        let tx = self.event_tx.clone();
        thread::spawn(move || {
            let result = client::query_evidence(client, &campaign, view);
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
                result: result.map(Box::new),
            });
            if let Some(ctx) = repaint {
                ctx.request_repaint();
            }
        });
    }

    fn refresh_llm_index(&mut self, repaint: Option<egui::Context>) {
        let Some(client) = self.client.clone() else {
            self.notice = Some("select a run with an available walk service".to_string());
            return;
        };
        if self.llm_index_pending.is_some() {
            self.notice = Some("live LLM index request already pending".to_string());
            return;
        }

        let token = self.next_trace_token();
        self.llm_index_pending = Some(token);
        self.llm_pending = None;
        self.llm_index = None;
        self.selected_llm = None;
        self.llm_snapshot = None;
        self.notice = Some("live LLM index pending".to_string());
        let tx = self.event_tx.clone();
        thread::spawn(move || {
            let result = client::llm_index(client);
            let _ = tx.send(UiEvent::LlmIndex { token, result });
            if let Some(ctx) = repaint {
                ctx.request_repaint();
            }
        });
    }

    fn load_llm_trace(&mut self, coordinate: LlmTraceCoordinate, repaint: Option<egui::Context>) {
        let Some(client) = self.client.clone() else {
            self.notice = Some("select a run with an available walk service".to_string());
            return;
        };
        if self.llm_pending.is_some() {
            self.notice = Some("live LLM observation already pending".to_string());
            return;
        }

        let token = self.next_trace_token();
        self.llm_snapshot = None;
        self.llm_pending = Some((token, coordinate.clone()));
        self.notice = Some(format!("loading LLM session {}", coordinate.session_id));
        let tx = self.event_tx.clone();
        thread::spawn(move || {
            let result = client::llm_trace(client, coordinate.clone());
            let _ = tx.send(UiEvent::LlmTrace {
                token,
                coordinate,
                result: result.map(Box::new),
            });
            if let Some(ctx) = repaint {
                ctx.request_repaint();
            }
        });
    }

    fn preview_setup(&mut self, repaint: Option<egui::Context>) {
        if self.setup_pending.is_some() {
            self.notice = Some("setup request already pending".to_string());
            return;
        }
        if let Err(error) = self.setup.setup_profile_usable() {
            self.notice = Some(error);
            return;
        }
        let request = match self.setup.request() {
            Ok(request) => request,
            Err(error) => {
                self.notice = Some(error);
                return;
            }
        };
        let token = self.next_setup_token();
        self.setup_pending = Some(SetupPending {
            token,
            kind: SetupKind::Preview,
        });
        self.notice = Some("read-only setup preview pending".to_string());
        let tx = self.event_tx.clone();
        thread::spawn(move || {
            let result = client::preview_setup(&request);
            let _ = tx.send(UiEvent::SetupPreview {
                token,
                request: Box::new(request),
                result: result.map(Box::new),
            });
            if let Some(ctx) = repaint {
                ctx.request_repaint();
            }
        });
    }

    fn admit_setup(&mut self, repaint: Option<egui::Context>) {
        if self.setup_blocked() {
            self.notice = Some(
                "stop the live walk authority before admitting setup; read-only preview remains available"
                    .to_string(),
            );
            return;
        }
        if self.setup_pending.is_some() {
            self.notice = Some("setup request already pending".to_string());
            return;
        }
        if let Err(error) = self.setup.setup_profile_usable() {
            self.notice = Some(error);
            return;
        }
        let request = match self.setup.request() {
            Ok(request) => request,
            Err(error) => {
                self.notice = Some(error);
                return;
            }
        };
        let Some(reviewed) = self.reviewed.as_ref() else {
            self.notice = Some("preview the setup before admission".to_string());
            return;
        };
        if reviewed.request != request {
            self.notice = Some("setup inputs changed; preview the exact request again".to_string());
            return;
        }
        let expected = reviewed.preview.plan_hash.clone();
        let token = self.next_setup_token();
        self.setup_pending = Some(SetupPending {
            token,
            kind: SetupKind::Admit,
        });
        self.notice = Some(format!("admitting reviewed setup {expected}"));
        let tx = self.event_tx.clone();
        thread::spawn(move || {
            let result = client::admit_setup(&request, &expected);
            let _ = tx.send(UiEvent::SetupAdmit {
                token,
                request: Box::new(request),
                result: result.map(Box::new),
            });
            if let Some(ctx) = repaint {
                ctx.request_repaint();
            }
        });
    }

    fn finish_setup_preview(
        &mut self,
        token: SetupToken,
        request: RunSetupRequest,
        result: Result<ploke_eval::setup_client::RunSetupPreview, String>,
    ) {
        if token.generation != self.client_generation
            || self.setup_pending
                != Some(SetupPending {
                    token,
                    kind: SetupKind::Preview,
                })
        {
            return;
        }
        self.setup_pending = None;
        match result {
            Ok(preview) => {
                if let Err(error) = self.setup.matches_setup_profile(&preview.profile) {
                    self.reviewed = None;
                    self.setup_receipt = None;
                    self.notice = Some(format!(
                        "setup preview profile does not match the editor: {error}"
                    ));
                    return;
                }
                self.notice = Some(format!("reviewed setup plan {}", preview.plan_hash));
                self.setup_receipt = None;
                self.reviewed = Some(ReviewedSetup { request, preview });
            }
            Err(error) => {
                self.notice = Some(format!("setup preview failed: {error}"));
            }
        }
    }

    fn finish_setup_admit(
        &mut self,
        token: SetupToken,
        request: RunSetupRequest,
        result: Result<RunSetupReceipt, String>,
    ) {
        if self.setup_pending
            != Some(SetupPending {
                token,
                kind: SetupKind::Admit,
            })
        {
            return;
        }
        self.setup_pending = None;
        match result {
            Ok(receipt) => {
                let campaign = receipt.config.identity.record.campaign_id.clone();
                self.campaign_input = campaign.to_string();
                self.setup_receipt = Some(receipt.clone());
                self.selected_run = None;
                self.refresh_runs();
                let selected = self.bind_admitted_run(&campaign, &receipt.repo_root);
                self.config_snapshot = Some(receipt.config);
                self.center_view = CenterView::Config;
                self.notice = Some(if selected {
                    format!(
                        "admitted and selected {campaign} from reviewed plan {}",
                        receipt.plan_hash
                    )
                } else {
                    format!(
                        "admitted {campaign} from reviewed plan {}, but run discovery did not return it or the exact checkout binding failed",
                        receipt.plan_hash
                    )
                });
            }
            Err(error) => {
                self.notice = Some(format!(
                    "setup admission failed for {}: {error}",
                    request.campaign
                ));
            }
        }
    }

    fn bind_admitted_run(&mut self, campaign: &CampaignId, repo_root: &Path) -> bool {
        let Some(index) = self
            .runs
            .iter()
            .position(|run| run.campaign_id == campaign.as_str())
        else {
            return false;
        };
        self.runs[index].worktree_root = Some(repo_root.to_path_buf());
        self.selected_run = Some(index);
        self.resolve_selected_client();
        self.client
            .as_ref()
            .is_some_and(|client| client.repo_root() == repo_root)
    }

    fn next_setup_token(&mut self) -> SetupToken {
        self.setup_serial = self.setup_serial.wrapping_add(1);
        SetupToken {
            generation: self.client_generation,
            serial: self.setup_serial,
        }
    }

    fn submit_operation(&mut self, intent: OperationIntent, repaint: Option<egui::Context>) {
        if self.setup_pending.is_some() {
            self.notice = Some(
                "wait for setup preview or admission to finish before submitting a live walk operation"
                    .to_string(),
            );
            return;
        }
        let Some(client) = self.client.clone() else {
            self.notice = Some("select a run with an available walk service".to_string());
            return;
        };
        if self.operation.is_some() {
            self.notice = Some("one walk operation is already pending".to_string());
            return;
        }
        if let OperationIntent::Step { advertised, .. } = &intent
            && advertised.requires_endpoint_following()
            && !client.follows_endpoint()
        {
            self.notice = Some(
                "this Step can transfer to a successor; use an unpinned client that follows endpoint authority"
                    .to_string(),
            );
            return;
        }
        self.operation_serial = self.operation_serial.wrapping_add(1);
        let operation = match &intent {
            OperationIntent::Stop => None,
            OperationIntent::Start { .. } | OperationIntent::Step { .. } => {
                Some(OperationId::new())
            }
        };
        let token = OperationToken {
            generation: self.client_generation,
            serial: self.operation_serial,
            operation,
        };
        self.operation = Some(PendingOperation {
            token,
            intent: intent.clone(),
            stage: OperationStage::Awaiting,
        });
        self.notice = Some(match operation {
            Some(operation) => format!("submitted walk operation {operation}"),
            None => "idle-server stop pending".to_string(),
        });
        let campaign = self.selected_campaign_id().map(str::to_owned);
        let allow_live = self.allow_live;
        let tx = self.event_tx.clone();
        thread::spawn(move || {
            let result = match intent {
                OperationIntent::Start { target } => client::start(
                    client,
                    campaign,
                    target,
                    allow_live,
                    operation.expect("start operation id"),
                ),
                OperationIntent::Step { advertised, .. } => {
                    client::step(client, advertised, operation.expect("step operation id"))
                }
                OperationIntent::Stop => client::stop_idle(client),
            };
            let _ = tx.send(UiEvent::Operation {
                token,
                result: result.map(Box::new),
            });
            if let Some(ctx) = repaint {
                ctx.request_repaint();
            }
        });
    }

    fn drive_operation(&mut self, ctx: &egui::Context) {
        let Some(pending) = self.operation.clone() else {
            return;
        };
        let OperationStage::PollAt(next) = pending.stage else {
            return;
        };
        let now = Instant::now();
        if now < next {
            ctx.request_repaint_after(next.saturating_duration_since(now));
            return;
        }
        let Some(operation) = pending.token.operation else {
            self.operation = None;
            self.halt_auto("operation polling lost its operation identity");
            return;
        };
        let Some(client) = self.client.clone() else {
            if let Some(current) = self.operation.as_mut() {
                current.stage =
                    OperationStage::PollAt(Instant::now() + crate::model::OPERATION_POLL_INTERVAL);
            }
            self.notice = Some(format!(
                "retaining unresolved walk operation {operation}; no client is available for status polling"
            ));
            ctx.request_repaint_after(crate::model::OPERATION_POLL_INTERVAL);
            return;
        };
        if let Some(current) = self.operation.as_mut() {
            current.stage = OperationStage::Awaiting;
        }
        let token = pending.token;
        let tx = self.event_tx.clone();
        let repaint = ctx.clone();
        thread::spawn(move || {
            let result = client::operation(client, operation);
            let _ = tx.send(UiEvent::Operation {
                token,
                result: result.map(Box::new),
            });
            repaint.request_repaint();
        });
    }

    fn finish_operation(
        &mut self,
        token: OperationToken,
        result: Result<WalkResponse, String>,
        ctx: &egui::Context,
    ) {
        let Some(pending) = self.operation.clone() else {
            return;
        };
        if pending.token != token {
            return;
        }
        match result {
            Ok(response) => {
                self.push_update(response.clone());
                self.response = Some(response.clone());
                match response {
                    WalkResponse::Job { job, .. } => {
                        if Some(job.operation_id) != token.operation {
                            self.operation = None;
                            self.halt_auto("walk response changed the retained operation identity");
                            return;
                        }
                        self.finish_job(pending.intent, &job, ctx);
                    }
                    WalkResponse::Error { detail, .. } => {
                        self.operation = None;
                        if pending.intent.is_auto() {
                            self.halt_auto(format!("auto-advance halted: {detail}"));
                        }
                        self.notice = Some(format!("walk operation failed: {detail}"));
                    }
                    WalkResponse::Status { .. }
                        if matches!(&pending.intent, OperationIntent::Stop) =>
                    {
                        self.operation = None;
                        self.status = ServiceStatus::Offline;
                        self.notice = Some("walk server accepted idle shutdown".to_string());
                    }
                    response => {
                        self.operation = None;
                        self.halt_auto(format!(
                            "walk operation returned an unexpected {:?} response",
                            response.phase()
                        ));
                    }
                }
            }
            Err(error) => {
                if let Some(operation) = token.operation {
                    if let Some(current) = self.operation.as_mut() {
                        current.stage = OperationStage::PollAt(
                            Instant::now() + crate::model::OPERATION_POLL_INTERVAL,
                        );
                    }
                    self.notice = Some(format!(
                        "walk operation {operation} request was ambiguous: {error}; retaining its identity and retrying operation status"
                    ));
                    ctx.request_repaint_after(crate::model::OPERATION_POLL_INTERVAL);
                } else {
                    self.operation = None;
                    self.notice = Some(format!("walk operation request failed: {error}"));
                }
            }
        }
    }

    fn finish_job(&mut self, intent: OperationIntent, job: &WalkJobSnapshot, ctx: &egui::Context) {
        match job.status {
            WalkJobStatus::Running | WalkJobStatus::CancelRequested => {
                if let Some(pending) = self.operation.as_mut() {
                    pending.stage = OperationStage::PollAt(
                        Instant::now() + crate::model::OPERATION_POLL_INTERVAL,
                    );
                }
                ctx.request_repaint_after(crate::model::OPERATION_POLL_INTERVAL);
            }
            WalkJobStatus::Succeeded => {
                self.operation = None;
                let receipt = match &intent {
                    OperationIntent::Step { advertised, .. } => {
                        match advertised.validate_terminal(job) {
                            Ok(receipt) => Some(receipt),
                            Err(error) => {
                                let detail = format!(
                                    "walk Step {} returned an invalid terminal receipt: {error}",
                                    job.operation_id
                                );
                                if intent.is_auto() {
                                    self.halt_auto(format!("auto-advance halted: {detail}"));
                                } else {
                                    self.notice = Some(detail);
                                }
                                return;
                            }
                        }
                    }
                    OperationIntent::Start { .. } | OperationIntent::Stop => None,
                };
                if let Some(receipt) = receipt {
                    let projection = match &receipt.event_projection {
                        WalkEventProjection::Unknown => {
                            Some("transition projection is unknown".to_string())
                        }
                        WalkEventProjection::Failed { detail } => {
                            Some(format!("transition projection failed: {detail}"))
                        }
                        WalkEventProjection::Recorded
                        | WalkEventProjection::NotApplicable { .. } => None,
                    };
                    if let Some(detail) = projection {
                        let detail = format!(
                            "walk Step {} returned an invalid terminal receipt: {detail}",
                            job.operation_id
                        );
                        if intent.is_auto() {
                            self.halt_auto(format!("auto-advance halted: {detail}"));
                        } else {
                            self.notice = Some(detail);
                        }
                        return;
                    }
                    if !intent.is_auto()
                        && receipt.phase_before == WalkPhase::R12
                        && receipt.phase_after == WalkPhase::R13b
                    {
                        self.begin_handoff(receipt, HandoffContinuation::Manual, ctx);
                        return;
                    }
                }
                if intent.is_auto() {
                    self.finish_auto_step(receipt.expect("auto intent is an advertised Step"), ctx);
                } else {
                    self.notice = Some(format!(
                        "{} operation {} succeeded",
                        job.command, job.operation_id
                    ));
                    self.refresh_health(Some(ctx.clone()));
                }
            }
            WalkJobStatus::Failed
            | WalkJobStatus::Cancelled
            | WalkJobStatus::Indeterminate
            | WalkJobStatus::Abandoned => {
                self.operation = None;
                let detail = format!(
                    "{} operation {} ended {:?}",
                    job.command, job.operation_id, job.status
                );
                if intent.is_auto() {
                    self.halt_auto(format!("auto-advance halted: {detail}"));
                }
                self.notice = Some(detail);
            }
        }
    }

    fn finish_auto_step(&mut self, receipt: &WalkTransitionReceipt, ctx: &egui::Context) {
        if receipt.phase_before == WalkPhase::R12 && receipt.phase_after == WalkPhase::R13c {
            self.halt_auto("handoff incomplete; operator action required");
            return;
        }
        if receipt.phase_after == WalkPhase::R14a {
            self.auto = AutoMode::Complete;
            self.notice = Some("auto-advance completed cleanly at R14a".to_string());
            return;
        }
        if receipt.phase_before == WalkPhase::R12 && receipt.phase_after == WalkPhase::R13b {
            let continuation = if matches!(self.auto, AutoMode::Stopping) {
                HandoffContinuation::Paused
            } else {
                HandoffContinuation::Auto
            };
            self.begin_handoff(receipt, continuation, ctx);
            return;
        }
        if receipt.phase_after == WalkPhase::R13b {
            self.halt_auto("auto-advance refused to Step a predecessor at R13b");
            return;
        }
        if matches!(self.auto, AutoMode::Stopping) {
            self.auto = AutoMode::Paused;
            self.notice = Some("Auto-advance paused".to_string());
            self.refresh_health(Some(ctx.clone()));
            return;
        }
        self.auto = AutoMode::Refreshing {
            after: self.status_serial,
        };
        self.refresh_health(Some(ctx.clone()));
    }

    fn begin_handoff(
        &mut self,
        receipt: &WalkTransitionReceipt,
        continuation: HandoffContinuation,
        ctx: &egui::Context,
    ) {
        let Some(prior) = receipt.version.session_id() else {
            self.halt_auto("handoff receipt omitted the predecessor session");
            return;
        };
        if !self
            .client
            .as_ref()
            .is_some_and(WalkClient::follows_endpoint)
        {
            self.halt_auto(
                "successor handoff requires an unpinned client that follows endpoint authority",
            );
            return;
        }
        self.config_snapshot = None;
        let now = Instant::now();
        self.auto = AutoMode::Waiting(HandoffWait {
            prior,
            started: now,
            next: now,
            delay: Duration::from_millis(250),
            after: self.status_serial,
            continuation,
        });
        self.notice = Some(match continuation {
            HandoffContinuation::Manual => {
                "Manual Step committed; following successor controller".to_string()
            }
            HandoffContinuation::Auto => "Waiting for successor controller".to_string(),
            HandoffContinuation::Paused => {
                "Waiting for successor controller before pausing".to_string()
            }
        });
        ctx.request_repaint();
    }

    fn begin_auto(&mut self, ctx: &egui::Context) {
        let Some(snapshot) = self.status_snapshot().cloned() else {
            self.halt_auto("auto-advance requires a fresh typed Walk status");
            return;
        };
        match snapshot.phase() {
            WalkPhase::R13b => {
                self.halt_auto("auto-advance will not Step the predecessor at R13b");
                return;
            }
            WalkPhase::R13c => {
                self.halt_auto("handoff incomplete; operator action required");
                return;
            }
            WalkPhase::R14a => {
                self.auto = AutoMode::Complete;
                return;
            }
            _ => {}
        }
        if let Err(error) = self.check_auto(&snapshot) {
            self.halt_auto(error);
            return;
        }
        self.auto = AutoMode::Refreshing {
            after: self.status_serial,
        };
        self.refresh_health(Some(ctx.clone()));
    }

    fn stop_auto(&mut self) {
        if self
            .operation
            .as_ref()
            .is_some_and(|pending| pending.intent.is_auto())
        {
            self.auto = AutoMode::Stopping;
            self.notice = Some("Stopping after current transition…".to_string());
        } else if let AutoMode::Waiting(mut wait) = self.auto.clone() {
            wait.continuation = HandoffContinuation::Paused;
            self.auto = AutoMode::Waiting(wait);
            self.notice = Some("Stopping after current transition…".to_string());
        } else {
            match self.auto.clone() {
                AutoMode::LoadingConfig {
                    requested, session, ..
                } => {
                    self.auto = AutoMode::LoadingConfig {
                        requested,
                        session,
                        continuation: HandoffContinuation::Paused,
                    };
                    self.notice = Some("Stopping before the successor transition…".to_string());
                }
                AutoMode::CheckingSuccessor { after, session, .. } => {
                    self.auto = AutoMode::CheckingSuccessor {
                        after,
                        session,
                        continuation: HandoffContinuation::Paused,
                    };
                    self.notice = Some("Stopping before the successor transition…".to_string());
                }
                _ => {
                    self.auto = AutoMode::Paused;
                    self.notice = Some("Auto-advance paused".to_string());
                }
            }
        }
    }

    fn resume_auto(&mut self, ctx: &egui::Context) {
        if !matches!(self.auto, AutoMode::Paused) {
            self.halt_auto("auto-advance resume requires a paused scheduler");
            return;
        }
        self.begin_auto(ctx);
    }

    fn drive_auto(&mut self, ctx: &egui::Context) {
        match self.auto.clone() {
            AutoMode::Refreshing { after } => {
                if self.operation.is_some() {
                    return;
                }
                if self.status_serial <= after {
                    if self.walk_pending.is_none() {
                        self.refresh_health(Some(ctx.clone()));
                    }
                    ctx.request_repaint_after(Duration::from_millis(100));
                    return;
                }
                let Some(snapshot) = self.status_snapshot().cloned() else {
                    self.halt_auto("auto-advance status refresh failed");
                    return;
                };
                match snapshot.phase() {
                    WalkPhase::R13b => {
                        self.halt_auto("auto-advance will not Step the predecessor at R13b");
                    }
                    WalkPhase::R13c => {
                        self.halt_auto("handoff incomplete; operator action required");
                    }
                    WalkPhase::R14a => {
                        self.auto = AutoMode::Complete;
                        self.notice = Some("auto-advance completed cleanly at R14a".to_string());
                    }
                    _ => {
                        let advertised = match self.check_auto(&snapshot) {
                            Ok(advertised) => advertised,
                            Err(error) => {
                                self.halt_auto(error);
                                return;
                            }
                        };
                        self.auto = AutoMode::Running;
                        self.submit_operation(
                            OperationIntent::Step {
                                advertised,
                                auto: true,
                            },
                            Some(ctx.clone()),
                        );
                    }
                }
            }
            AutoMode::LoadingConfig {
                requested,
                session,
                continuation,
            } => {
                if self.config_pending.is_some() {
                    ctx.request_repaint_after(Duration::from_millis(100));
                    return;
                }
                if !requested {
                    self.config_snapshot = None;
                    self.auto = AutoMode::LoadingConfig {
                        requested: true,
                        session,
                        continuation,
                    };
                    self.refresh_config(Some(ctx.clone()));
                    return;
                }
                self.halt_auto(
                    "successor configuration request completed without a validation outcome",
                );
            }
            AutoMode::CheckingSuccessor {
                after,
                session,
                continuation,
            } => {
                if self.status_serial <= after {
                    if self.walk_pending.is_none() {
                        self.refresh_health(Some(ctx.clone()));
                    }
                    ctx.request_repaint_after(Duration::from_millis(100));
                    return;
                }
                let snapshot = match self.check_successor(session, continuation) {
                    Ok(snapshot) => snapshot.clone(),
                    Err(error) => {
                        self.halt_auto(format!(
                            "successor configuration/status validation failed: {error}"
                        ));
                        return;
                    }
                };
                self.finish_handoff(continuation, &snapshot, ctx);
            }
            AutoMode::Waiting(mut wait) => {
                let now = Instant::now();
                if now.duration_since(wait.started) >= HANDOFF_WAIT_TIMEOUT {
                    self.halt_auto(format!(
                        "successor controller did not become ready within {}s",
                        HANDOFF_WAIT_TIMEOUT.as_secs()
                    ));
                    return;
                }
                if self.status_serial > wait.after {
                    if let Some(snapshot) = self.status_snapshot().cloned()
                        && let Some(session) = self.successor_ready(&snapshot, wait.prior)
                    {
                        self.enter_successor(&wait, session, ctx);
                        return;
                    }
                    wait.after = self.status_serial;
                    wait.next = now + wait.delay;
                    wait.delay = (wait.delay * 2).min(Duration::from_secs(4));
                    self.auto = AutoMode::Waiting(wait.clone());
                }
                if now >= wait.next && self.walk_pending.is_none() {
                    wait.after = self.status_serial;
                    self.auto = AutoMode::Waiting(wait.clone());
                    self.refresh_health(Some(ctx.clone()));
                }
                let remaining = wait.next.saturating_duration_since(now);
                ctx.request_repaint_after(remaining.max(Duration::from_millis(50)));
            }
            AutoMode::Running if self.operation.is_none() => {
                self.auto = AutoMode::Refreshing {
                    after: self.status_serial,
                };
                self.refresh_health(Some(ctx.clone()));
            }
            AutoMode::Idle
            | AutoMode::Running
            | AutoMode::Stopping
            | AutoMode::Paused
            | AutoMode::Halted(_)
            | AutoMode::Complete => {}
        }
    }

    fn check_auto(&self, snapshot: &WalkSessionSnapshot) -> Result<AdvertisedStep, String> {
        let config = self
            .config_snapshot
            .as_ref()
            .ok_or_else(|| "auto-advance requires the admitted configuration".to_string())?;
        if config.control.mode != RunMode::Step {
            return Err("auto-advance requires an admitted Step-mode run".to_string());
        }
        if !self
            .client
            .as_ref()
            .is_some_and(WalkClient::follows_endpoint)
        {
            return Err(
                "auto-advance requires an unpinned client that follows successor endpoints"
                    .to_string(),
            );
        }
        strict_step(snapshot, self.allow_live, self.allow_git).map_err(|error| error.to_string())
    }

    fn successor_ready(
        &self,
        snapshot: &WalkSessionSnapshot,
        prior: ploke_eval::walk_client::SessionId,
    ) -> Option<ploke_eval::walk_client::SessionId> {
        let session = snapshot.version().session_id()?;
        (snapshot.phase() == WalkPhase::R4c
            && session != prior
            && AdvertisedStep::from_snapshot(snapshot, true, true).is_ok())
        .then_some(session)
    }

    fn enter_successor(
        &mut self,
        wait: &HandoffWait,
        session: ploke_eval::walk_client::SessionId,
        ctx: &egui::Context,
    ) {
        self.config_snapshot = None;
        self.auto = AutoMode::LoadingConfig {
            requested: false,
            session,
            continuation: wait.continuation,
        };
        self.notice = Some(match wait.continuation {
            HandoffContinuation::Manual => {
                "Successor ready; validating configuration before returning control".to_string()
            }
            HandoffContinuation::Auto => {
                "Successor ready; validating configuration before the next Step".to_string()
            }
            HandoffContinuation::Paused => {
                "Successor ready; validating configuration before pausing".to_string()
            }
        });
        ctx.request_repaint();
    }

    fn check_successor(
        &self,
        session: ploke_eval::walk_client::SessionId,
        continuation: HandoffContinuation,
    ) -> Result<&WalkSessionSnapshot, String> {
        let Some(client) = self.client.as_ref() else {
            return Err("successor validation lost its walk client".to_string());
        };
        if !client.follows_endpoint() {
            return Err("walk client no longer follows endpoint authority".to_string());
        }
        let Some(WalkResponse::Status {
            snapshot, epoch, ..
        }) = self.response.as_ref()
        else {
            return Err("successor validation requires a fresh typed status".to_string());
        };
        if snapshot.phase() != WalkPhase::R4c || snapshot.version().session_id() != Some(session) {
            return Err(format!(
                "successor status changed from session {session} at R4c"
            ));
        }
        let Some(config) = self.config_snapshot.as_ref() else {
            return Err("successor validation lost its reloaded configuration".to_string());
        };
        if continuation != HandoffContinuation::Manual && config.control.mode != RunMode::Step {
            return Err("successor configuration is not admitted for Step mode".to_string());
        }
        let Some(selected) = self.selected_campaign_id() else {
            return Err("successor validation lost the selected campaign".to_string());
        };
        if config.identity.record.campaign_id.as_str() != selected {
            return Err(format!(
                "successor configuration campaign '{}' does not match selected campaign '{selected}'",
                config.identity.record.campaign_id
            ));
        }
        if epoch.repo_root != config.campaign.admission.setup_root
            || epoch.repo_root != client.repo_root()
        {
            return Err(format!(
                "successor status root '{}' does not match configuration '{}' and client '{}'",
                epoch.repo_root.display(),
                config.campaign.admission.setup_root.display(),
                client.repo_root().display()
            ));
        }
        AdvertisedStep::from_snapshot(snapshot, true, true).map_err(|error| {
            format!("successor status does not advertise one coherent Step: {error}")
        })?;
        Ok(snapshot)
    }

    fn finish_handoff(
        &mut self,
        continuation: HandoffContinuation,
        snapshot: &WalkSessionSnapshot,
        ctx: &egui::Context,
    ) {
        match continuation {
            HandoffContinuation::Manual => {
                self.auto = AutoMode::Idle;
                self.notice = Some(
                    "Successor configuration and status validated at R4c; manual Step complete"
                        .to_string(),
                );
            }
            HandoffContinuation::Paused => {
                self.auto = AutoMode::Paused;
                self.notice = Some(
                    "Successor configuration and status validated; auto-advance paused".to_string(),
                );
            }
            HandoffContinuation::Auto => {
                let advertised = match self.check_auto(snapshot) {
                    Ok(advertised) => advertised,
                    Err(error) => {
                        self.halt_auto(format!(
                            "auto-advance halted after successor validation: {error}"
                        ));
                        return;
                    }
                };
                self.auto = AutoMode::Running;
                self.submit_operation(
                    OperationIntent::Step {
                        advertised,
                        auto: true,
                    },
                    Some(ctx.clone()),
                );
            }
        }
    }

    fn halt_auto(&mut self, reason: impl Into<String>) {
        let reason = reason.into();
        self.notice = Some(reason.clone());
        self.auto = AutoMode::Halted(reason);
    }

    fn push_update(&mut self, response: WalkResponse) {
        if self
            .updates
            .last()
            .is_some_and(|prior| same_update(prior, &response))
        {
            return;
        }
        if self.updates.len() == 64 {
            self.updates.remove(0);
        }
        self.updates.push(response);
    }

    fn poll_events(&mut self, ctx: &egui::Context) {
        while let Ok(event) = self.event_rx.try_recv() {
            match event {
                UiEvent::Walk { token, result } => self.finish_walk_request(token, result),
                UiEvent::Config { generation, result } => {
                    self.finish_config(generation, result.map(|snapshot| *snapshot))
                }
                UiEvent::SetupPreview {
                    token,
                    request,
                    result,
                } => self.finish_setup_preview(token, *request, result.map(|preview| *preview)),
                UiEvent::SetupAdmit {
                    token,
                    request,
                    result,
                } => self.finish_setup_admit(token, *request, result.map(|receipt| *receipt)),
                UiEvent::Operation { token, result } => {
                    self.finish_operation(token, result.map(|response| *response), ctx)
                }
                UiEvent::Query { generation, result } => self.finish_query(generation, result),
                UiEvent::TraceIndex { token, result } => self.finish_trace_index(token, result),
                UiEvent::Trace {
                    token,
                    coordinate,
                    result,
                } => self.finish_trace(token, coordinate, result.map(|snapshot| *snapshot)),
                UiEvent::LlmIndex { token, result } => self.finish_llm_index(token, result),
                UiEvent::LlmTrace {
                    token,
                    coordinate,
                    result,
                } => self.finish_llm_trace(token, coordinate, result.map(|snapshot| *snapshot)),
            }
        }
    }

    fn finish_config(&mut self, generation: u64, result: Result<WalkConfigSnapshot, String>) {
        if generation != self.client_generation || self.config_pending != Some(generation) {
            return;
        }
        self.config_pending = None;
        let handoff = match self.auto.clone() {
            AutoMode::LoadingConfig {
                requested: true,
                session,
                continuation,
            } => Some((session, continuation)),
            _ => None,
        };
        match result {
            Ok(snapshot) => {
                self.config_snapshot = Some(snapshot);
                if let Some((session, continuation)) = handoff {
                    self.auto = AutoMode::CheckingSuccessor {
                        after: self.status_serial,
                        session,
                        continuation,
                    };
                    self.notice = Some(
                        "Successor configuration loaded; refreshing typed status for final validation"
                            .to_string(),
                    );
                } else {
                    self.notice = Some(format!(
                        "admitted configuration loaded for {}",
                        self.config_snapshot
                            .as_ref()
                            .expect("just stored configuration")
                            .identity
                            .record
                            .campaign_id
                    ));
                }
            }
            Err(error) => {
                self.config_snapshot = None;
                if handoff.is_some() {
                    self.halt_auto(format!("successor configuration request failed: {error}"));
                } else {
                    self.notice = Some(format!("configuration request failed: {error}"));
                }
            }
        }
    }

    fn finish_walk_request(&mut self, token: WalkRequestToken, result: WalkRequestResult) {
        if token.generation != self.client_generation || self.walk_pending != Some(token) {
            return;
        }
        self.walk_pending = None;
        let kind = token.kind;
        if kind == WalkRequestKind::Health {
            self.status_serial = self.status_serial.wrapping_add(1);
        }
        match result {
            WalkRequestResult::Response(response) => {
                self.status = match response.as_ref() {
                    WalkResponse::Error { detail, .. } => ServiceStatus::Error(detail.clone()),
                    _ => ServiceStatus::Online,
                };
                self.notice = Some(format!("{} response received", kind.label()));
                self.response = Some(*response);
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

    fn finish_llm_index(
        &mut self,
        token: TraceRequestToken,
        result: Result<LlmTraceIndex, String>,
    ) {
        if token.generation != self.client_generation || self.llm_index_pending != Some(token) {
            return;
        }
        self.llm_index_pending = None;
        match result {
            Ok(index) => {
                let selected = self.selected_campaign_id();
                if selected != Some(index.campaign.as_str()) {
                    self.notice = Some(format!(
                        "LLM trace campaign '{}' disagrees with selected campaign '{}'",
                        index.campaign,
                        selected.unwrap_or("-")
                    ));
                    return;
                }
                let sessions = index
                    .lanes
                    .iter()
                    .map(|lane| lane.sessions.len())
                    .sum::<usize>();
                self.selected_llm = None;
                self.llm_snapshot = None;
                self.notice = Some(format!(
                    "{} LLM session(s), {} issue(s)",
                    sessions,
                    index.issues.len()
                ));
                self.llm_index = Some(index);
            }
            Err(error) => {
                self.llm_index = None;
                self.selected_llm = None;
                self.llm_snapshot = None;
                self.notice = Some(error);
            }
        }
    }

    fn finish_llm_trace(
        &mut self,
        token: TraceRequestToken,
        coordinate: LlmTraceCoordinate,
        result: Result<LlmTraceSnapshot, String>,
    ) {
        if token.generation != self.client_generation
            || !self
                .llm_pending
                .as_ref()
                .is_some_and(|pending| pending.0 == token && pending.1 == coordinate)
        {
            return;
        }
        self.llm_pending = None;
        match result {
            Ok(snapshot)
                if snapshot.coordinate == coordinate
                    && snapshot.session.value.session_id == coordinate.session_id =>
            {
                self.selected_llm = Some(coordinate.session_id.clone());
                self.notice = Some(format!(
                    "LLM session {} observation loaded",
                    coordinate.session_id
                ));
                self.llm_snapshot = Some(snapshot);
            }
            Ok(snapshot) => {
                self.llm_snapshot = None;
                self.notice = Some(format!(
                    "LLM trace response '{}' disagrees with requested session '{}'",
                    snapshot.coordinate.session_id, coordinate.session_id
                ));
            }
            Err(error) => {
                self.llm_snapshot = None;
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
        if (action.selected_run.is_some() || action.refresh_runs)
            && let Some(reason) = self.run_blocker()
        {
            self.notice = Some(reason);
            return;
        }
        if action.socket_changed
            && let Some(reason) = self.socket_blocker()
        {
            if let Some(client) = self.client.as_ref() {
                self.socket_input = client.socket().display().to_string();
            }
            self.notice = Some(reason);
            return;
        }
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
        if action.run_raw {
            self.run_query(Some(ctx.clone()));
        }
        if let Some(view) = action.view {
            self.run_evidence(view, Some(ctx.clone()));
        }
    }

    fn handle_config_action(&mut self, action: ConfigAction, ctx: &egui::Context) {
        if action.refresh {
            self.refresh_config(Some(ctx.clone()));
        }
    }

    fn handle_setup_action(&mut self, action: SetupAction, ctx: &egui::Context) {
        if action.profile_changed || action.profile_defaults || action.profile_load {
            self.reviewed = None;
            self.setup_receipt = None;
            if action.profile_changed {
                self.notice =
                    Some("Profile draft changed; the stale setup preview was cleared".to_string());
            }
        }
        if action.profile_defaults {
            self.setup.restore_profile();
        }
        if action.profile_load {
            self.setup.load_profile();
        }
        if action.profile_review {
            self.setup.review_profile();
        }
        if action.profile_save && self.setup.save_profile() {
            self.reviewed = None;
            self.setup_receipt = None;
            self.notice = Some(
                "Saved the reviewed run profile; the stale setup preview was cleared".to_string(),
            );
        }
        if action.preview {
            self.preview_setup(Some(ctx.clone()));
        }
        if action.admit {
            self.admit_setup(Some(ctx.clone()));
        }
    }

    fn handle_details_action(&mut self, action: DetailsAction, ctx: &egui::Context) {
        if action.refresh_health {
            self.refresh_health(Some(ctx.clone()));
        }
        if action.show_state {
            self.show_state(Some(ctx.clone()));
        }
        if let Some(target) = action.start {
            self.submit_operation(OperationIntent::Start { target }, Some(ctx.clone()));
        }
        if let Some(advertised) = action.step {
            self.submit_operation(
                OperationIntent::Step {
                    advertised,
                    auto: false,
                },
                Some(ctx.clone()),
            );
        }
        if action.stop {
            self.submit_operation(OperationIntent::Stop, Some(ctx.clone()));
        }
        if action.start_auto {
            self.begin_auto(ctx);
        }
        if action.resume_auto {
            self.resume_auto(ctx);
        }
        if action.stop_auto {
            self.stop_auto();
        }
        if action.end_auto {
            self.auto = AutoMode::Idle;
            self.notice = Some("Local auto-advance intent ended".to_string());
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

    fn handle_llm_action(&mut self, action: LlmTraceAction, ctx: &egui::Context) {
        if action.refresh {
            self.refresh_llm_index(Some(ctx.clone()));
        }
        if let Some(coordinate) = action.load {
            self.load_llm_trace(coordinate, Some(ctx.clone()));
        }
    }
}

impl eframe::App for WalkUiApp {
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.poll_events(ctx);
        self.drive_operation(ctx);
        self.drive_auto(ctx);
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        let run_blocked = self.run_blocker().is_some();
        let socket_blocked = self.socket_blocker().is_some();
        egui::Panel::top("top_bar").show_inside(ui, |ui| {
            let action = TopBar {
                status: &self.status,
                runs: &self.runs,
                selected_run: self.selected_run,
                socket_input: &mut self.socket_input,
                debug_panel: &mut self.debug_panel,
                run_blocked,
                socket_blocked,
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
                let config_step = self
                    .config_snapshot
                    .as_ref()
                    .is_some_and(|config| config.control.mode == RunMode::Step);
                let follows_endpoint = self
                    .client
                    .as_ref()
                    .is_some_and(WalkClient::follows_endpoint);
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
                    allow_live: &mut self.allow_live,
                    allow_git: &mut self.allow_git,
                    operation_pending: self.operation.is_some()
                        || self.walk_pending.is_some()
                        || self.setup_pending.is_some(),
                    config_step,
                    follows_endpoint,
                    auto: &self.auto,
                    updates: &self.updates,
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
                ui.selectable_value(&mut self.center_view, CenterView::LiveLlm, "Live LLM");
                ui.selectable_value(&mut self.center_view, CenterView::Config, "Run Config");
                ui.selectable_value(&mut self.center_view, CenterView::Setup, "New Run");
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
                CenterView::LiveLlm => {
                    let action = LlmTracePanel {
                        index: self.llm_index.as_ref(),
                        selected: &mut self.selected_llm,
                        snapshot: self.llm_snapshot.as_ref(),
                        index_pending: self.llm_index_pending.is_some(),
                        trace_pending: self.llm_pending.is_some(),
                    }
                    .show(ui);
                    self.handle_llm_action(action, &ctx);
                }
                CenterView::Config => {
                    let action = ConfigPanel {
                        snapshot: self.config_snapshot.as_ref(),
                        pending: self.config_pending.is_some(),
                        client_available: self.client.is_some(),
                    }
                    .show(ui);
                    self.handle_config_action(action, &ctx);
                }
                CenterView::Setup => {
                    let operation_pending = self.setup_blocked();
                    let action = SetupPanel {
                        draft: &mut self.setup,
                        reviewed: self.reviewed.as_ref(),
                        receipt: self.setup_receipt.as_ref(),
                        pending: self.setup_pending.is_some(),
                        operation_pending,
                    }
                    .show(ui);
                    self.handle_setup_action(action, &ctx);
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
                        client_available: self.client.is_some(),
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

fn same_update(prior: &WalkResponse, current: &WalkResponse) -> bool {
    match (prior, current) {
        (
            WalkResponse::Job {
                job: prior,
                message: prior_message,
                ..
            },
            WalkResponse::Job {
                job: current,
                message: current_message,
                ..
            },
        ) => prior == current && prior_message == current_message,
        (
            WalkResponse::Error {
                code: prior_code,
                detail: prior_detail,
                phase: prior_phase,
                ..
            },
            WalkResponse::Error {
                code: current_code,
                detail: current_detail,
                phase: current_phase,
                ..
            },
        ) => {
            prior_code == current_code
                && prior_detail == current_detail
                && prior_phase == current_phase
        }
        _ => false,
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
    use ploke_eval::{
        setup_client::{
            RunSetupBatch, RunSetupEmbedding, RunSetupModel, RunSetupProfile, RunSetupProtocol,
        },
        walk_client::ControlEdge,
    };

    #[test]
    fn kittest_disables_query_without_exact_client_binding() {
        use egui_kittest::{
            Harness,
            kittest::{NodeT, Queryable},
        };

        let mut harness = Harness::builder()
            .with_size(egui::Vec2::new(1280.0, 820.0))
            .build_eframe(|_cc| test_app());

        harness.get_by_label("Database Query").click();
        harness.run();
        assert!(
            harness
                .get_by_label("Run raw query")
                .accesskit_node()
                .is_disabled()
        );
        harness.get_by_label(
            "Select a run with an exact local checkout binding to query its walk service.",
        );
    }

    #[test]
    fn kittest_exposes_admitted_run_configuration_surface() {
        use egui_kittest::{Harness, kittest::Queryable};

        let mut harness = Harness::builder()
            .with_size(egui::Vec2::new(1280.0, 820.0))
            .build_eframe(|_cc| test_app());

        harness.get_by_label("Run Config").click();
        harness.run();

        harness.get_by_label("Admitted run configuration");
        harness.get_by_label("Refresh Config");
        harness.get_by_label("No admitted configuration loaded");
    }

    #[test]
    fn manual_step_retains_server_advertised_branch_offer() {
        let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        let socket = unique_temp_dir("ploke-walk-ui-step-target").join("walk.sock");
        let client =
            WalkClient::resolve(Some(&repo_root), Some(&socket)).expect("resolve test client");
        let mut app = test_app_with_client(client);
        let advertised = test_advertised(
            WalkPhase::R4a,
            &[ControlEdge::R4aToR4b, ControlEdge::R4aToR4c],
            false,
            false,
        );
        let expected = advertised.clone();

        app.handle_details_action(
            DetailsAction {
                step: Some(advertised),
                ..DetailsAction::default()
            },
            &egui::Context::default(),
        );

        let pending = app.operation.as_ref().expect("manual Step retained");
        let OperationIntent::Step { advertised, auto } = &pending.intent else {
            panic!("manual action must retain a Step intent");
        };
        assert!(!auto);
        assert_eq!(advertised, &expected);
    }

    #[test]
    fn handoff_step_requires_a_following_client_before_submission() {
        let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        let socket = unique_temp_dir("ploke-walk-ui-step-handoff").join("walk.sock");
        let client =
            WalkClient::resolve(Some(&repo_root), Some(&socket)).expect("resolve pinned client");
        let mut app = test_app_with_client(client);
        let outcomes = [
            ControlEdge::R12ToR13a,
            ControlEdge::R12ToR13b,
            ControlEdge::R12ToR13c,
        ];

        app.handle_details_action(
            DetailsAction {
                step: Some(test_advertised(WalkPhase::R12, &outcomes, false, true)),
                ..DetailsAction::default()
            },
            &egui::Context::default(),
        );

        assert!(
            app.operation.is_none(),
            "handoff-capable Step must fail before operation admission"
        );
        assert!(
            app.notice
                .as_deref()
                .is_some_and(|notice| notice.contains("unpinned client")),
            "operator feedback should explain the endpoint requirement: {:?}",
            app.notice
        );

        app.handle_details_action(
            DetailsAction {
                step: Some(test_advertised(WalkPhase::R12, &outcomes, false, false)),
                ..DetailsAction::default()
            },
            &egui::Context::default(),
        );
        assert!(
            app.operation.is_some(),
            "the same pinned client may submit the retained stop-only R12 offer"
        );
    }

    #[test]
    fn admitted_run_binds_the_exact_canonical_setup_root() {
        let temp = unique_temp_dir("ploke-walk-ui-setup-root");
        let setup_root = temp.join("setup-seeds").join("campaign-a");
        fs::create_dir_all(&setup_root).expect("create arbitrary setup checkout");
        let setup_root = fs::canonicalize(&setup_root).expect("canonical setup checkout");
        assert!(
            setup_root.ends_with("setup-seeds/campaign-a")
                && !setup_root.ends_with("worktrees/campaign-a"),
            "regression root must not exercise the legacy worktree convention"
        );

        let mut run = test_run("campaign-a", &setup_root);
        run.worktree_root = None;
        let mut app = test_app();
        app.runs = vec![run];

        assert!(app.bind_admitted_run(&CampaignId::from("campaign-a"), &setup_root));
        assert_eq!(app.selected_run, Some(0));
        assert_eq!(
            app.runs[0].worktree_root.as_deref(),
            Some(setup_root.as_path())
        );
        assert_eq!(
            app.client.as_ref().map(WalkClient::repo_root),
            Some(setup_root.as_path())
        );

        fs::remove_dir_all(temp).expect("remove setup-root fixture");
    }

    #[test]
    fn start_retains_the_server_advertised_target() {
        let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        let socket = unique_temp_dir("ploke-walk-ui-start-target").join("walk.sock");
        let client =
            WalkClient::resolve(Some(&repo_root), Some(&socket)).expect("resolve test client");
        let mut app = test_app_with_client(client);

        app.handle_details_action(
            DetailsAction {
                start: Some(WalkPhase::R7),
                ..DetailsAction::default()
            },
            &egui::Context::default(),
        );

        assert!(matches!(
            app.operation.as_ref().map(|pending| &pending.intent),
            Some(OperationIntent::Start {
                target: WalkPhase::R7
            })
        ));
    }

    #[test]
    fn advertised_step_success_requires_one_matching_step_receipt() {
        let advertised = test_advertised(WalkPhase::R5, &[ControlEdge::R5ToR6], true, true);
        let missing = test_job(WalkPhase::R5, WalkPhase::R6, &[], false);
        assert!(
            advertised
                .validate_terminal(&missing)
                .expect_err("missing receipt must halt")
                .to_string()
                .contains("omitted its typed receipt")
        );

        let multiple = test_job(
            WalkPhase::R5,
            WalkPhase::R7,
            &[ControlEdge::R5ToR6, ControlEdge::R6ToR7],
            true,
        );
        assert!(
            advertised
                .validate_terminal(&multiple)
                .expect_err("multi-edge receipt must halt")
                .to_string()
                .contains("expected exactly one")
        );

        let mismatch = test_job(WalkPhase::R5, WalkPhase::R7, &[ControlEdge::R5ToR6], true);
        assert!(
            advertised
                .validate_terminal(&mismatch)
                .expect_err("mismatched receipt must halt")
                .to_string()
                .contains("does not match")
        );

        let valid = test_job(WalkPhase::R5, WalkPhase::R6, &[ControlEdge::R5ToR6], true);
        assert!(advertised.validate_terminal(&valid).is_ok());
    }

    #[test]
    fn manual_and_auto_step_reject_outcomes_outside_the_retained_offer() {
        let advertised = test_advertised(
            WalkPhase::R4a,
            &[ControlEdge::R4aToR4b, ControlEdge::R4aToR4c],
            true,
            true,
        );
        let outside = test_job(WalkPhase::R4a, WalkPhase::R6, &[ControlEdge::R5ToR6], true);

        let mut manual = test_app();
        manual.finish_job(
            OperationIntent::Step {
                advertised: advertised.clone(),
                auto: false,
            },
            &outside,
            &egui::Context::default(),
        );
        assert!(
            manual
                .notice
                .as_deref()
                .is_some_and(|notice| notice.contains("outside retained outcomes"))
        );

        let mut auto = test_app();
        auto.auto = AutoMode::Running;
        auto.finish_job(
            OperationIntent::Step {
                advertised,
                auto: true,
            },
            &outside,
            &egui::Context::default(),
        );
        let AutoMode::Halted(reason) = &auto.auto else {
            panic!("invalid auto outcome must halt automation");
        };
        assert!(reason.contains("outside retained outcomes"), "{reason}");
    }

    #[test]
    fn auto_step_accepts_branches_and_requires_complete_r12_grants() {
        let branch = test_snapshot(
            WalkPhase::R4a,
            &[ControlEdge::R4aToR4b, ControlEdge::R4aToR4c],
        );
        let advertised = strict_step(&branch, false, false).expect("branch-bearing auto Step");
        assert_eq!(advertised.outcomes().len(), 2);

        let r12 = test_snapshot(
            WalkPhase::R12,
            &[
                ControlEdge::R12ToR13a,
                ControlEdge::R12ToR13b,
                ControlEdge::R12ToR13c,
            ],
        );
        let error = strict_step(&r12, false, false)
            .expect_err("auto R12 requires complete checkout coverage")
            .to_string();
        assert!(error.contains("every advertised outcome"), "{error}");
        assert_eq!(
            strict_step(&r12, false, true)
                .expect("checkout-authorized R12 auto Step")
                .outcomes()
                .len(),
            3
        );
    }

    #[test]
    fn pause_during_handoff_survives_successor_wait() {
        let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        let client = WalkClient::resolve(Some(&repo_root), None).expect("resolve test client");
        let mut app = test_app_with_client(client);
        app.auto = AutoMode::Stopping;
        let job = test_job(
            WalkPhase::R12,
            WalkPhase::R13b,
            &[ControlEdge::R12ToR13b],
            true,
        );

        app.finish_auto_step(
            job.receipt.as_ref().expect("handoff receipt"),
            &egui::Context::default(),
        );

        let AutoMode::Waiting(wait) = &app.auto else {
            panic!("handoff should wait for successor controller");
        };
        assert_eq!(
            wait.continuation,
            HandoffContinuation::Paused,
            "stop intent must survive successor handoff"
        );
        assert!(app.operation.is_none(), "no successor Step may be queued");
        let wait = wait.clone();
        let session = test_session(2);

        app.enter_successor(&wait, session, &egui::Context::default());

        assert!(matches!(
            app.auto,
            AutoMode::LoadingConfig {
                requested: false,
                continuation: HandoffContinuation::Paused,
                ..
            }
        ));
        assert!(app.config_snapshot.is_none());
        assert!(app.operation.is_none(), "paused successor must not Step");

        app.drive_auto(&egui::Context::default());
        assert!(matches!(
            app.auto,
            AutoMode::LoadingConfig {
                requested: true,
                continuation: HandoffContinuation::Paused,
                ..
            }
        ));
        assert!(app.config_pending.is_some());
        assert!(
            app.operation.is_none(),
            "pause must validate successor configuration without a Step"
        );
    }

    #[test]
    fn successor_config_refresh_precedes_next_step() {
        let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        let client = WalkClient::resolve(Some(&repo_root), None).expect("resolve test client");
        let mut app = test_app_with_client(client);
        let wait = HandoffWait {
            prior: test_session(1),
            started: Instant::now(),
            next: Instant::now(),
            delay: Duration::from_millis(250),
            after: 0,
            continuation: HandoffContinuation::Auto,
        };
        let session = test_session(2);

        app.enter_successor(&wait, session, &egui::Context::default());

        assert!(matches!(
            app.auto,
            AutoMode::LoadingConfig {
                requested: false,
                continuation: HandoffContinuation::Auto,
                ..
            }
        ));
        assert!(app.config_snapshot.is_none());
        assert!(app.operation.is_none(), "successor Step must wait");

        app.drive_auto(&egui::Context::default());

        assert!(matches!(
            app.auto,
            AutoMode::LoadingConfig {
                requested: true,
                continuation: HandoffContinuation::Auto,
                ..
            }
        ));
        assert!(app.config_pending.is_some());
        assert!(app.operation.is_none(), "configuration must finish first");
    }

    #[test]
    fn successor_config_failure_halts_before_step() {
        let mut app = test_app();
        let generation = app.client_generation;
        app.auto = AutoMode::LoadingConfig {
            requested: true,
            session: test_session(2),
            continuation: HandoffContinuation::Auto,
        };
        app.config_pending = Some(generation);

        app.finish_config(
            generation,
            Err("successor identity could not be validated".to_string()),
        );

        let AutoMode::Halted(reason) = &app.auto else {
            panic!("configuration failure must halt auto-advance");
        };
        assert!(reason.contains("successor identity could not be validated"));
        assert!(app.operation.is_none());
    }

    #[test]
    fn successor_status_mismatch_halts_before_manual_completion() {
        let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        let client = WalkClient::resolve(Some(&repo_root), None).expect("resolve test client");
        let epoch_root = client.repo_root().to_path_buf();
        let mut app = test_app_with_client(client);
        let expected = test_session(2);
        app.auto = AutoMode::CheckingSuccessor {
            after: 0,
            session: expected,
            continuation: HandoffContinuation::Manual,
        };
        app.status_serial = 1;
        app.response = Some(WalkResponse::Status {
            message: "successor moved".to_string(),
            snapshot: test_snapshot_for_session(
                WalkPhase::R5,
                &[ControlEdge::R5ToR6],
                test_session(3),
            ),
            epoch: ploke_eval::walk_client::ServerEpoch {
                protocol_version: 12,
                transition_graph_version: "walk-r0-r14a-v2".to_string(),
                repo_root: epoch_root,
                exe_path: PathBuf::from("/tmp/ploke-eval"),
                exe_modified_unix_ms: Some(17),
                git_head: Some("abc123".to_string()),
                active_branch: Some("successor/runtime-2".to_string()),
                source_status_hash: Some("def456".to_string()),
                build_fingerprint: "test-build".to_string(),
            },
        });

        app.drive_auto(&egui::Context::default());

        let AutoMode::Halted(reason) = &app.auto else {
            panic!("status/config mismatch must fail closed");
        };
        assert!(reason.contains("successor status changed"), "{reason}");
        assert!(app.operation.is_none());
    }

    #[test]
    fn manual_handoff_validates_successor_without_submitting_another_step() {
        let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        let client = WalkClient::resolve(Some(&repo_root), None).expect("resolve test client");
        let mut app = test_app_with_client(client);
        let outcomes = [
            ControlEdge::R12ToR13a,
            ControlEdge::R12ToR13b,
            ControlEdge::R12ToR13c,
        ];
        let job = test_job(
            WalkPhase::R12,
            WalkPhase::R13b,
            &[ControlEdge::R12ToR13b],
            true,
        );

        app.finish_job(
            OperationIntent::Step {
                advertised: test_advertised(WalkPhase::R12, &outcomes, true, true),
                auto: false,
            },
            &job,
            &egui::Context::default(),
        );

        let AutoMode::Waiting(wait) = app.auto.clone() else {
            panic!("manual handoff must wait for the successor endpoint");
        };
        assert_eq!(wait.continuation, HandoffContinuation::Manual);
        assert!(app.config_snapshot.is_none());
        assert!(app.operation.is_none());

        let session = test_session(2);
        app.enter_successor(&wait, session, &egui::Context::default());
        assert!(matches!(
            app.auto,
            AutoMode::LoadingConfig {
                continuation: HandoffContinuation::Manual,
                ..
            }
        ));

        // `drive_auto` calls this seam only after the successor config reply and
        // a fresh, matching typed status have both been validated.
        let successor = test_snapshot_for_session(WalkPhase::R4c, &[ControlEdge::R4cToR5], session);
        app.auto = AutoMode::CheckingSuccessor {
            after: app.status_serial,
            session,
            continuation: HandoffContinuation::Manual,
        };
        app.finish_handoff(
            HandoffContinuation::Manual,
            &successor,
            &egui::Context::default(),
        );

        assert!(matches!(app.auto, AutoMode::Idle));
        assert!(
            app.operation.is_none(),
            "manual handoff must not queue R4c -> R5"
        );
        assert!(
            app.notice
                .as_deref()
                .is_some_and(|notice| notice.contains("manual Step complete"))
        );
    }

    #[test]
    fn unresolved_operation_survives_invalidation_and_ambiguous_failure() {
        let mut app = test_app();
        let token = OperationToken {
            generation: app.client_generation,
            serial: 7,
            operation: Some(OperationId::new()),
        };
        app.operation = Some(PendingOperation {
            token,
            intent: OperationIntent::Step {
                advertised: test_advertised(WalkPhase::R5, &[ControlEdge::R5ToR6], true, false),
                auto: true,
            },
            stage: OperationStage::Awaiting,
        });
        app.auto = AutoMode::Running;
        let prior_generation = app.client_generation;

        app.invalidate_client_state();

        assert_ne!(app.client_generation, prior_generation);
        assert_eq!(
            app.operation.as_ref().map(|pending| pending.token),
            Some(token)
        );
        assert!(matches!(app.auto, AutoMode::Running));
        let retained_generation = app.client_generation;
        app.handle_top_bar_action(
            TopBarAction {
                refresh_runs: true,
                ..TopBarAction::default()
            },
            &egui::Context::default(),
        );
        assert_eq!(app.client_generation, retained_generation);
        app.handle_top_bar_action(
            TopBarAction {
                socket_changed: true,
                ..TopBarAction::default()
            },
            &egui::Context::default(),
        );
        assert_eq!(app.client_generation, retained_generation);
        assert_eq!(
            app.operation.as_ref().map(|pending| pending.token),
            Some(token)
        );

        app.finish_operation(
            token,
            Err("transport timed out after admission".to_string()),
            &egui::Context::default(),
        );

        let retained = app.operation.as_ref().expect("operation identity retained");
        assert_eq!(retained.token, token);
        assert!(matches!(retained.stage, OperationStage::PollAt(_)));
        assert!(
            app.notice
                .as_deref()
                .is_some_and(|notice| notice.contains(&token.operation.unwrap().to_string()))
        );

        app.submit_operation(
            OperationIntent::Step {
                advertised: test_advertised(WalkPhase::R5, &[ControlEdge::R5ToR6], true, false),
                auto: false,
            },
            None,
        );
        assert_eq!(
            app.operation.as_ref().map(|pending| pending.token),
            Some(token)
        );
    }

    #[test]
    fn setup_admission_survives_invalidation_and_reports_completion() {
        let mut app = test_app();
        let token = SetupToken {
            generation: app.client_generation,
            serial: 11,
        };
        let pending = SetupPending {
            token,
            kind: SetupKind::Admit,
        };
        app.setup_pending = Some(pending);
        let request = RunSetupRequest {
            repo_root: PathBuf::from("/tmp/worktree"),
            batch: RunSetupBatch::Id("batch-a".to_string()),
            campaign: CampaignId::from("campaign-a"),
            profile: RunSetupProfile::Name("profile-a".to_string()),
            primary_instance: None,
            model: RunSetupModel::default(),
            protocol: RunSetupProtocol::default(),
            embedding: RunSetupEmbedding::default(),
        };

        app.invalidate_client_state();

        assert_eq!(app.setup_pending, Some(pending));
        let retained_generation = app.client_generation;
        app.handle_top_bar_action(
            TopBarAction {
                refresh_runs: true,
                ..TopBarAction::default()
            },
            &egui::Context::default(),
        );
        assert_eq!(app.client_generation, retained_generation);
        app.finish_setup_admit(
            token,
            request,
            Err("admission failed after writes".to_string()),
        );

        assert!(app.setup_pending.is_none());
        assert!(
            app.notice
                .as_deref()
                .is_some_and(|notice| notice.contains("admission failed after writes"))
        );
    }

    #[test]
    fn setup_admission_and_live_mutations_reject_each_other_at_submission() {
        let mut app = test_app();
        let operation = OperationId::new();
        app.operation = Some(PendingOperation {
            token: OperationToken {
                generation: app.client_generation,
                serial: 1,
                operation: Some(operation),
            },
            intent: OperationIntent::Start {
                target: WalkPhase::R4c,
            },
            stage: OperationStage::Awaiting,
        });

        app.admit_setup(None);
        assert!(app.setup_pending.is_none());
        assert!(
            app.notice
                .as_deref()
                .is_some_and(|notice| notice.contains("stop the live walk authority"))
        );

        app.operation = None;
        app.setup_pending = Some(SetupPending {
            token: SetupToken {
                generation: app.client_generation,
                serial: 2,
            },
            kind: SetupKind::Preview,
        });
        app.submit_operation(
            OperationIntent::Start {
                target: WalkPhase::R4c,
            },
            None,
        );

        assert!(app.operation.is_none());
        assert!(
            app.notice
                .as_deref()
                .is_some_and(|notice| notice.contains("setup preview or admission"))
        );
    }

    #[test]
    fn setup_admission_uses_live_status_not_cached_response() {
        let mut app = test_app();
        app.response = Some(WalkResponse::Status {
            message: "last accepted health".to_string(),
            snapshot: test_snapshot(WalkPhase::R4c, &[]),
            epoch: ploke_eval::walk_client::ServerEpoch {
                protocol_version: 12,
                transition_graph_version: "walk-r0-r14a-v2".to_string(),
                repo_root: PathBuf::from("/tmp/ploke-parent"),
                exe_path: PathBuf::from("/tmp/ploke-eval"),
                exe_modified_unix_ms: Some(17),
                git_head: Some("abc123".to_string()),
                active_branch: Some("parent/runtime-1".to_string()),
                source_status_hash: Some("def456".to_string()),
                build_fingerprint: "test-build".to_string(),
            },
        });

        app.status = ServiceStatus::Offline;
        assert!(
            !app.setup_blocked(),
            "an accepted idle Stop must permit admission even when the last typed response is cached"
        );

        app.status = ServiceStatus::Online;
        assert!(
            app.setup_blocked(),
            "typed Online status must keep admission disabled"
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
            WalkRequestResult::Response(Box::new(response)),
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
            build_fingerprint: String::new(),
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
            WalkRequestResult::Response(Box::new(status.clone())),
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
            WalkRequestResult::Response(Box::new(WalkResponse::Ok {
                phase: ploke_eval::walk_client::WalkPhase::R4c,
                result: ploke_eval::walk_client::WalkOkPayload::Show {
                    report: "same reconstructed phase".to_string(),
                },
                epoch: epoch.clone(),
            })),
        );
        assert!(app.status_snapshot().is_none());

        finish_walk_request(
            &mut app,
            WalkRequestKind::Show,
            WalkRequestResult::Response(Box::new(WalkResponse::Ok {
                phase: ploke_eval::walk_client::WalkPhase::R5,
                result: ploke_eval::walk_client::WalkOkPayload::Show {
                    report: "new phase".to_string(),
                },
                epoch: epoch.clone(),
            })),
        );
        assert!(app.status_snapshot().is_none());

        finish_walk_request(
            &mut app,
            WalkRequestKind::Health,
            WalkRequestResult::Response(Box::new(status)),
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
            WalkRequestResult::Response(Box::new(status.clone())),
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
            WalkRequestResult::Response(Box::new(status)),
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
    fn llm_reply_from_previous_run_is_discarded_after_selection() {
        let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        let mut app = test_app();
        app.socket_input = unique_temp_dir("ploke-walk-ui-llm-generation")
            .join("walk.sock")
            .display()
            .to_string();
        app.runs = vec![test_run("run-a", &repo_root), test_run("run-b", &repo_root)];
        app.selected_run = Some(0);
        app.resolve_selected_client();
        let coordinate = test_llm_coordinate("session-a", None);
        let index_token = app.next_trace_token();
        let trace_token = app.next_trace_token();
        app.llm_index_pending = Some(index_token);
        app.llm_pending = Some((trace_token, coordinate.clone()));

        app.select_run(1);

        app.finish_llm_index(index_token, Err("LLM index from run A".to_string()));
        app.finish_llm_trace(
            trace_token,
            coordinate,
            Err("LLM response from run A".to_string()),
        );
        assert_eq!(app.selected_campaign_id(), Some("run-b"));
        assert!(app.llm_index.is_none());
        assert!(app.llm_snapshot.is_none());
        assert!(app.llm_index_pending.is_none());
        assert!(app.llm_pending.is_none());
        assert!(
            app.notice
                .as_deref()
                .is_none_or(|notice| !notice.contains("run A"))
        );
    }

    #[test]
    fn same_run_llm_reply_requires_current_serial_and_coordinate() {
        let mut app = test_app();
        let coordinate = test_llm_coordinate("session-a", Some(2));
        let old = app.next_trace_token();
        app.llm_pending = Some((old, coordinate.clone()));

        app.llm_pending = None;
        let current = app.next_trace_token();
        app.llm_pending = Some((current, coordinate.clone()));
        app.notice = Some("new LLM request pending".to_string());

        app.finish_llm_trace(
            old,
            coordinate.clone(),
            Err("stale same-run response".to_string()),
        );
        app.finish_llm_trace(
            current,
            test_llm_coordinate("session-b", Some(2)),
            Err("wrong session response".to_string()),
        );

        assert_eq!(
            app.llm_pending.as_ref().map(|pending| pending.0),
            Some(current)
        );
        assert_eq!(app.notice.as_deref(), Some("new LLM request pending"));
        assert!(app.llm_snapshot.is_none());
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
    fn unresolved_operation_blocks_socket_rebinding() {
        let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        let mut app = test_app();
        app.runs = vec![test_run("run-a", &repo_root)];
        app.selected_run = Some(0);
        let first = unique_temp_dir("ploke-walk-ui-operation-socket-a").join("walk.sock");
        app.socket_input = first.display().to_string();
        app.resolve_selected_client();
        let generation = app.client_generation;
        let operation = OperationId::new();
        app.operation = Some(PendingOperation {
            token: OperationToken {
                generation,
                serial: 1,
                operation: Some(operation),
            },
            intent: OperationIntent::Start {
                target: WalkPhase::R4c,
            },
            stage: OperationStage::Awaiting,
        });
        let second = unique_temp_dir("ploke-walk-ui-operation-socket-b").join("walk.sock");
        app.socket_input = second.display().to_string();

        app.handle_top_bar_action(
            TopBarAction {
                socket_changed: true,
                ..TopBarAction::default()
            },
            &egui::Context::default(),
        );

        assert_eq!(app.client_generation, generation);
        assert_eq!(
            app.client.as_ref().map(WalkClient::socket),
            Some(first.as_path())
        );
        assert_eq!(app.socket_input, first.display().to_string());
        assert!(app.operation.is_some());
        assert!(
            app.notice
                .as_deref()
                .is_some_and(|notice| notice.contains(&operation.to_string()))
        );
    }

    #[test]
    fn active_auto_advance_blocks_run_and_socket_rebinding() {
        let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        let mut app = test_app();
        app.runs = vec![test_run("run-a", &repo_root), test_run("run-b", &repo_root)];
        app.selected_run = Some(0);
        let first = unique_temp_dir("ploke-walk-ui-auto-socket-a").join("walk.sock");
        app.socket_input = first.display().to_string();
        app.resolve_selected_client();
        let generation = app.client_generation;
        app.auto = AutoMode::Running;

        app.select_run(1);
        assert_eq!(app.selected_run, Some(0));
        assert_eq!(app.client_generation, generation);
        assert!(
            app.notice
                .as_deref()
                .is_some_and(|notice| notice.contains("auto-advance is active or paused"))
        );

        let second = unique_temp_dir("ploke-walk-ui-auto-socket-b").join("walk.sock");
        app.socket_input = second.display().to_string();
        app.handle_top_bar_action(
            TopBarAction {
                socket_changed: true,
                ..TopBarAction::default()
            },
            &egui::Context::default(),
        );

        assert_eq!(app.client_generation, generation);
        assert_eq!(
            app.client.as_ref().map(WalkClient::socket),
            Some(first.as_path())
        );
        assert!(matches!(app.auto, AutoMode::Running));
    }

    #[test]
    fn paused_auto_advance_retains_binding() {
        let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        let mut app = test_app();
        app.runs = vec![test_run("run-a", &repo_root), test_run("run-b", &repo_root)];
        app.selected_run = Some(0);
        let first = unique_temp_dir("ploke-walk-ui-paused-socket-a").join("walk.sock");
        app.socket_input = first.display().to_string();
        app.resolve_selected_client();
        let generation = app.client_generation;
        app.auto = AutoMode::Paused;

        app.select_run(1);

        assert_eq!(app.selected_run, Some(0));
        assert_eq!(app.client_generation, generation);
        assert_eq!(
            app.client.as_ref().map(WalkClient::socket),
            Some(first.as_path())
        );
        assert!(matches!(app.auto, AutoMode::Paused));

        app.handle_details_action(
            DetailsAction {
                end_auto: true,
                ..DetailsAction::default()
            },
            &egui::Context::default(),
        );

        assert!(matches!(app.auto, AutoMode::Idle));
        assert!(app.run_blocker().is_none());
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
            config_snapshot: None,
            query_result: None,
            selected_row: None,
            trace_index: None,
            selected_trace: None,
            trace_snapshot: None,
            llm_index: None,
            selected_llm: None,
            llm_snapshot: None,
            notice: None,
            client_generation: 0,
            walk_pending: None,
            config_pending: None,
            query_pending: None,
            trace_index_pending: None,
            trace_pending: None,
            llm_index_pending: None,
            llm_pending: None,
            trace_serial: 0,
            center_view: CenterView::default(),
            debug_panel: false,
            debug_hover: false,
            buttons: UiButtonState::default(),
            setup: SetupDraft::for_root(PathBuf::from("/tmp").as_path()),
            reviewed: None,
            setup_receipt: None,
            setup_pending: None,
            setup_serial: 0,
            allow_live: false,
            allow_git: false,
            operation: None,
            operation_serial: 0,
            updates: Vec::new(),
            auto: AutoMode::default(),
            status_serial: 0,
        }
    }

    fn wait_for_events(app: &mut WalkUiApp) {
        let deadline = Instant::now() + Duration::from_secs(4);
        let ctx = egui::Context::default();
        while Instant::now() < deadline {
            app.poll_events(&ctx);
            if app.walk_pending.is_none()
                && app.config_pending.is_none()
                && app.setup_pending.is_none()
                && app.query_pending.is_none()
                && app.operation.is_none()
                && app.trace_index_pending.is_none()
                && app.trace_pending.is_none()
                && app.llm_index_pending.is_none()
                && app.llm_pending.is_none()
            {
                return;
            }
            thread::sleep(Duration::from_millis(10));
        }
        app.poll_events(&ctx);
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

    fn test_llm_coordinate(session: &str, step: Option<usize>) -> LlmTraceCoordinate {
        LlmTraceCoordinate {
            session_id: session.to_string(),
            step,
        }
    }

    fn test_snapshot(phase: WalkPhase, edges: &[ControlEdge]) -> WalkSessionSnapshot {
        let actions = edges
            .iter()
            .map(|edge| {
                serde_json::json!({
                    "kind": "step",
                    "edge": edge,
                    "target": edge.to(),
                    "enabled": true,
                    "requires_live_api": edge.requires_live(),
                    "requires_git_changes": edge.requires_checkout(),
                    "blocker": null
                })
            })
            .collect::<Vec<_>>();
        serde_json::from_value(serde_json::json!({
            "phase": phase,
            "version": {
                "session_id": "00000000-0000-0000-0000-000000000001",
                "cursor": {
                    "phase": phase,
                    "evidence": "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"
                },
                "journal_revision": 8
            },
            "position": {
                "source": "session",
                "version": {
                    "session_id": "00000000-0000-0000-0000-000000000001",
                    "cursor": {
                        "phase": phase,
                        "evidence": "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"
                    },
                    "journal_revision": 8
                }
            },
            "controller_attached": true,
            "authority": "active",
            "job": null,
            "blocker": null,
            "actions": actions
        }))
        .expect("typed test status")
    }

    fn test_session(value: u128) -> ploke_eval::walk_client::SessionId {
        serde_json::from_value(serde_json::json!(format!(
            "00000000-0000-0000-0000-{value:012x}"
        )))
        .expect("typed test session")
    }

    fn test_snapshot_for_session(
        phase: WalkPhase,
        edges: &[ControlEdge],
        session: ploke_eval::walk_client::SessionId,
    ) -> WalkSessionSnapshot {
        let mut value =
            serde_json::to_value(test_snapshot(phase, edges)).expect("serialize typed test status");
        let session = serde_json::to_value(session).expect("serialize typed test session");
        value["version"]["session_id"] = session.clone();
        value["position"]["version"]["session_id"] = session;
        serde_json::from_value(value).expect("typed successor test status")
    }

    fn test_advertised(
        phase: WalkPhase,
        edges: &[ControlEdge],
        allow_live: bool,
        allow_git: bool,
    ) -> AdvertisedStep {
        AdvertisedStep::from_snapshot(&test_snapshot(phase, edges), allow_live, allow_git)
            .expect("valid advertised Step")
    }

    fn test_job(
        before: WalkPhase,
        after: WalkPhase,
        edges: &[ControlEdge],
        include_receipt: bool,
    ) -> WalkJobSnapshot {
        let operation = OperationId::new();
        let receipt = include_receipt.then(|| {
            serde_json::json!({
                "phase_before": before,
                "phase_after": after,
                "edges": edges,
                "version": {
                    "session_id": "00000000-0000-0000-0000-000000000001",
                    "cursor": {
                        "phase": after,
                        "evidence": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
                    },
                    "journal_revision": 9
                },
                "event_projection": {"status": "recorded"}
            })
        });
        serde_json::from_value(serde_json::json!({
            "job_id": 4,
            "operation_id": operation,
            "expected": {
                "session_id": "00000000-0000-0000-0000-000000000001",
                "cursor": {
                    "phase": before,
                    "evidence": "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"
                },
                "journal_revision": 8
            },
            "command": "step",
            "status": "succeeded",
            "phase_before": before,
            "phase_after": after,
            "target_phase": null,
            "watch": false,
            "allow_live_api": true,
            "allow_git_changes": true,
            "started_at": "2026-07-26T00:00:00Z",
            "updated_at": "2026-07-26T00:00:01Z",
            "finished_at": "2026-07-26T00:00:01Z",
            "message": "completed",
            "receipt": receipt,
            "resolution": null
        }))
        .expect("typed test job")
    }

    fn unique_temp_dir(prefix: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time")
            .as_nanos();
        std::env::temp_dir().join(format!("{prefix}-{}-{nanos}", std::process::id()))
    }
}
