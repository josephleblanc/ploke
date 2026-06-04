//! Browser graph bootstrap: snapshot catalog and wasm file loading.

use std::fmt;

use egui;
use ploke_tree::Graph;

use crate::import::{ImportError, graph_from_snapshot_bytes};

/// Export-graph JSON served beside the Trunk `dist/` bundle (see `index.html` copy-dir).
pub const STANDARD_GRAPH_SNAPSHOT_FIXTURE_PATH: &str = "benchmark-fixtures/protocol-graph.json";

/// Same-origin URL for [`STANDARD_GRAPH_SNAPSHOT_FIXTURE_PATH`] when using `trunk serve`.
pub const STANDARD_GRAPH_SNAPSHOT_FIXTURE_URL: &str = "/benchmark-fixtures/protocol-graph.json";

/// Basename of the default WASM dogfood graph (also accepted via `?graph=` alias).
pub const STANDARD_GRAPH_SNAPSHOT_BASENAME: &str = "protocol-graph.json";

/// Multi-generation trajectory fixture for trend/table QA; load via `?graph=trajectory-multi-gen.json`.
pub const TRAJECTORY_MULTI_GEN_SNAPSHOT_FIXTURE_URL: &str =
    "/benchmark-fixtures/trajectory-multi-gen.json";

pub const TRAJECTORY_MULTI_GEN_SNAPSHOT_BASENAME: &str = "trajectory-multi-gen.json";

#[derive(Debug, Clone)]
pub struct LoadedGraph {
    pub label: String,
    pub graph: Graph,
}

pub struct GraphCatalog {
    entries: Vec<LoadedGraph>,
    selected: Option<usize>,
    last_error: Option<String>,
    #[cfg(target_arch = "wasm32")]
    wasm: WasmGraphLoad,
}

impl fmt::Debug for GraphCatalog {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GraphCatalog")
            .field("entries", &self.entries.len())
            .field("selected", &self.selected)
            .field("last_error", &self.last_error)
            .finish()
    }
}

impl Default for GraphCatalog {
    fn default() -> Self {
        Self::new()
    }
}

impl GraphCatalog {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            selected: None,
            last_error: None,
            #[cfg(target_arch = "wasm32")]
            wasm: WasmGraphLoad::new(),
        }
    }

    pub fn entries(&self) -> &[LoadedGraph] {
        &self.entries
    }

    pub fn selected_label(&self) -> Option<&str> {
        self.selected
            .and_then(|index| self.entries.get(index))
            .map(|entry| entry.label.as_str())
    }

    pub fn last_error(&self) -> Option<&str> {
        self.last_error.as_deref()
    }

    pub fn ingest_snapshot_bytes(
        &mut self,
        label: String,
        bytes: &[u8],
    ) -> Result<(), ImportError> {
        let graph = graph_from_snapshot_bytes(bytes)?;
        self.push_loaded(label, graph);
        Ok(())
    }

    pub fn push_loaded(&mut self, label: String, graph: Graph) {
        self.entries.push(LoadedGraph { label, graph });
        self.selected = Some(self.entries.len() - 1);
        self.last_error = None;
    }

    /// Poll async wasm loads (file picker / `?graph=` fetch). Call from the app frame
    /// before central tiles so graph replacement is not tied to the left nav panel.
    #[cfg(target_arch = "wasm32")]
    pub fn apply_pending_graph(&mut self) -> Option<Graph> {
        let Some(pending) = self.wasm.take_pending_load() else {
            return None;
        };
        match pending {
            wasm::PendingLoad::Bytes { label, bytes } => {
                match self.ingest_snapshot_bytes(label, &bytes) {
                    Ok(()) => {
                        self.wasm.discard_pending_url_load();
                        self.selected_graph_clone()
                    }
                    Err(error) => {
                        self.last_error = Some(error.to_string());
                        None
                    }
                }
            }
            wasm::PendingLoad::Error(message) => {
                self.last_error = Some(message);
                None
            }
        }
    }

    /// Keep the latest egui context for async file-read callbacks (request_repaint).
    #[cfg(target_arch = "wasm32")]
    pub fn set_repaint_context(&mut self, ctx: egui::Context) {
        self.wasm.set_repaint_context(ctx);
    }

    /// Render catalog UI. Returns a graph when the user picks another loaded entry.
    ///
    /// `compact` keeps the benchmark placeholder footprint (labels only, no controls).
    pub fn show(&mut self, ui: &mut egui::Ui, compact: bool) -> Option<Graph> {
        ui.separator();
        if compact {
            ui.label("Graph catalog");
            ui.label(format!("entries: {}", self.entries.len()));
            return None;
        }

        ui.label("Graph snapshots");
        ui.label("Load an export-graph JSON snapshot (not a live run directory).");

        #[cfg(target_arch = "wasm32")]
        {
            ui.label(format!(
                "Dev URL: ?graph={STANDARD_GRAPH_SNAPSHOT_FIXTURE_URL} (or use the file picker; local filesystem paths are not readable in the browser)."
            ));
            if self.wasm.fetch_in_progress() {
                ui.label("Loading graph snapshot…");
            }
            if ui.button("Load graph snapshot (.json)…").clicked() {
                self.wasm.open_file_picker();
            }
        }

        if let Some(error) = &self.last_error {
            ui.colored_label(egui::Color32::LIGHT_RED, error);
        }

        let mut activated = None;
        if self.entries.is_empty() {
            ui.label("No snapshots loaded.");
        } else {
            let selected = self.selected;
            for (index, entry) in self.entries.iter().enumerate() {
                let selected_now = selected == Some(index);
                if ui
                    .selectable_label(selected_now, entry.label.as_str())
                    .clicked()
                    && !selected_now
                {
                    self.selected = Some(index);
                    activated = self.selected_graph_clone();
                }
            }
        }

        activated
    }

    /// Fetch `?graph=` when present, otherwise the Trunk-shipped default snapshot.
    #[cfg(target_arch = "wasm32")]
    pub fn enqueue_startup_graph_load(&mut self) {
        self.wasm.enqueue_startup_graph_load();
    }

    fn selected_graph_clone(&self) -> Option<Graph> {
        self.selected
            .and_then(|index| self.entries.get(index))
            .map(|entry| entry.graph.clone())
    }
}

#[cfg(target_arch = "wasm32")]
mod wasm {
    use std::cell::RefCell;
    use std::rc::Rc;

    use wasm_bindgen::JsCast;
    use wasm_bindgen::closure::Closure;
    use wasm_bindgen_futures::spawn_local;
    use web_sys::{FileReader, HtmlInputElement, Request, RequestInit, RequestMode, Response};

    pub enum PendingLoad {
        Bytes { label: String, bytes: Vec<u8> },
        Error(String),
    }

    pub struct WasmGraphLoad {
        file_input: HtmlInputElement,
        pending_file: Rc<RefCell<Option<(String, Vec<u8>)>>>,
        pending_error: Rc<RefCell<Option<String>>>,
        pending_url: Rc<RefCell<Option<Result<(String, Vec<u8>), String>>>>,
        ignore_pending_url: Rc<RefCell<bool>>,
        repaint_context: Rc<RefCell<Option<egui::Context>>>,
        url_fetch_started: bool,
        fetch_in_progress: Rc<RefCell<bool>>,
    }

    impl WasmGraphLoad {
        pub fn new() -> Self {
            let document = web_sys::window()
                .expect("browser window")
                .document()
                .expect("browser document");
            let file_input = document
                .create_element("input")
                .expect("create input")
                .dyn_into::<HtmlInputElement>()
                .expect("input element");
            file_input.set_type("file");
            file_input.set_accept(".json,application/json");
            let _ = file_input.style().set_property("display", "none");
            document
                .body()
                .expect("document body")
                .append_child(&file_input)
                .expect("attach file input");

            let pending_file: Rc<RefCell<Option<(String, Vec<u8>)>>> = Rc::new(RefCell::new(None));
            let pending_error: Rc<RefCell<Option<String>>> = Rc::new(RefCell::new(None));
            let repaint_context: Rc<RefCell<Option<egui::Context>>> = Rc::new(RefCell::new(None));
            let pending_for_change = pending_file.clone();
            let pending_error_for_change = pending_error.clone();
            let repaint_context_for_input = repaint_context.clone();
            let on_change = Closure::wrap(Box::new(move |event: web_sys::Event| {
                let Ok(input) = event
                    .target()
                    .expect("change event target")
                    .dyn_into::<HtmlInputElement>()
                else {
                    return;
                };
                let Some(files) = input.files() else {
                    return;
                };
                if files.length() == 0 {
                    return;
                }
                let Some(file) = files.get(0) else {
                    return;
                };
                let label = file.name();
                let label_for_error = label.clone();
                let reader = Rc::new(FileReader::new().expect("FileReader"));
                let pending = pending_for_change.clone();
                let pending_error = pending_error_for_change.clone();
                let pending_error_for_error = pending_error_for_change.clone();
                let repaint_context = repaint_context_for_input.clone();
                let reader_weak = Rc::downgrade(&reader);
                let on_load = Closure::wrap(Box::new(move |_event: web_sys::Event| {
                    let Some(reader) = reader_weak.upgrade() else {
                        return;
                    };
                    let Ok(result) = reader.result() else {
                        *pending_error.borrow_mut() = Some(format!("file read failed for {label}"));
                        return;
                    };
                    let Ok(array_buffer) = result.dyn_into::<js_sys::ArrayBuffer>() else {
                        *pending_error.borrow_mut() =
                            Some(format!("file read returned unexpected type for {label}"));
                        return;
                    };
                    let bytes = js_sys::Uint8Array::new(&array_buffer).to_vec();
                    *pending.borrow_mut() = Some((label.clone(), bytes));
                    if let Some(ctx) = repaint_context.borrow().as_ref() {
                        ctx.request_repaint();
                    }
                    wake_animation_frame();
                }) as Box<dyn FnMut(_)>);
                reader.set_onload(Some(on_load.as_ref().unchecked_ref()));
                on_load.forget();
                let on_error = Closure::wrap(Box::new(move |_event: web_sys::Event| {
                    *pending_error_for_error.borrow_mut() =
                        Some(format!("file read error for {label_for_error}"));
                }) as Box<dyn FnMut(_)>);
                reader.set_onerror(Some(on_error.as_ref().unchecked_ref()));
                on_error.forget();
                let _ = reader.read_as_array_buffer(&file);
            }) as Box<dyn FnMut(_)>);
            file_input.set_onchange(Some(on_change.as_ref().unchecked_ref()));
            on_change.forget();

            Self {
                file_input,
                pending_file,
                pending_error,
                pending_url: Rc::new(RefCell::new(None)),
                ignore_pending_url: Rc::new(RefCell::new(false)),
                repaint_context,
                url_fetch_started: false,
                fetch_in_progress: Rc::new(RefCell::new(false)),
            }
        }

        pub fn set_repaint_context(&self, ctx: egui::Context) {
            *self.repaint_context.borrow_mut() = Some(ctx);
        }

        pub fn fetch_in_progress(&self) -> bool {
            *self.fetch_in_progress.borrow()
        }

        pub fn discard_pending_url_load(&self) {
            *self.ignore_pending_url.borrow_mut() = true;
            self.pending_url.borrow_mut().take();
        }

        pub fn open_file_picker(&self) {
            *self.ignore_pending_url.borrow_mut() = true;
            self.pending_url.borrow_mut().take();
            self.file_input.set_value("");
            let _ = self.file_input.click();
        }

        pub fn enqueue_startup_graph_load(&mut self) {
            if self.url_fetch_started {
                return;
            }
            if let Some(raw) = query_graph_url_from_location() {
                if let Some(message) = super::graph_fetch_rejection_message(&raw) {
                    self.url_fetch_started = true;
                    *self.pending_url.borrow_mut() = Some(Err(message));
                    return;
                }
                self.start_url_fetch(super::resolve_graph_fetch_url(&raw));
                return;
            }
            self.start_url_fetch(super::STANDARD_GRAPH_SNAPSHOT_FIXTURE_URL.to_owned());
        }

        fn start_url_fetch(&mut self, url: String) {
            self.url_fetch_started = true;
            *self.fetch_in_progress.borrow_mut() = true;
            let pending_url = self.pending_url.clone();
            let fetch_in_progress = self.fetch_in_progress.clone();
            let repaint_context = self.repaint_context.clone();
            spawn_local(async move {
                let result = fetch_graph_bytes(&url).await;
                *fetch_in_progress.borrow_mut() = false;
                *pending_url.borrow_mut() = Some(result);
                if let Some(ctx) = repaint_context.borrow().as_ref() {
                    ctx.request_repaint();
                }
            });
        }

        pub fn take_pending_load(&self) -> Option<PendingLoad> {
            if let Some(message) = self.pending_error.borrow_mut().take() {
                return Some(PendingLoad::Error(message));
            }
            if let Some(pending) = self.pending_file.borrow_mut().take() {
                return Some(PendingLoad::Bytes {
                    label: pending.0,
                    bytes: pending.1,
                });
            }
            if *self.ignore_pending_url.borrow() {
                self.pending_url.borrow_mut().take();
                return None;
            }
            self.pending_url
                .borrow_mut()
                .take()
                .map(|result| match result {
                    Ok((label, bytes)) => PendingLoad::Bytes { label, bytes },
                    Err(message) => PendingLoad::Error(message),
                })
        }
    }

    fn wake_animation_frame() {
        let Some(window) = web_sys::window() else {
            return;
        };
        let wake = Closure::once(Box::new(move || {}) as Box<dyn FnMut()>);
        let _ = window.request_animation_frame(wake.as_ref().unchecked_ref());
        wake.forget();
    }

    fn query_graph_url_from_location() -> Option<String> {
        let window = web_sys::window()?;
        let search = window.location().search().ok()?;
        super::graph_query_url_from_search(&search)
    }

    async fn fetch_graph_bytes(url: &str) -> Result<(String, Vec<u8>), String> {
        let mut init = RequestInit::new();
        init.method("GET");
        init.set_mode(RequestMode::Cors);
        let request =
            Request::new_with_str_and_init(url, &init).map_err(|error| format!("{error:?}"))?;
        let response_value = wasm_bindgen_futures::JsFuture::from(
            web_sys::window()
                .expect("window")
                .fetch_with_request(&request),
        )
        .await
        .map_err(|error| format!("fetch failed: {error:?}"))?;
        let response: Response = response_value
            .dyn_into()
            .map_err(|_| "fetch response was not a Response".to_owned())?;
        if !response.ok() {
            return Err(format!("fetch returned HTTP {}", response.status()));
        }
        if response_is_html(&response) {
            return Err(format!(
                "fetch of {url} returned HTML (fixture missing from dist?); expected JSON. \
                 Rebuild with Trunk copy-dir for benchmark-fixtures, or use \
                 ?graph={}",
                super::STANDARD_GRAPH_SNAPSHOT_FIXTURE_URL
            ));
        }
        let buffer = wasm_bindgen_futures::JsFuture::from(
            response
                .array_buffer()
                .map_err(|error| format!("{error:?}"))?,
        )
        .await
        .map_err(|error| format!("read body failed: {error:?}"))?;
        let array_buffer = buffer
            .dyn_into::<js_sys::ArrayBuffer>()
            .map_err(|_| "body was not ArrayBuffer".to_owned())?;
        let bytes = js_sys::Uint8Array::new(&array_buffer).to_vec();
        if !super::looks_like_json_snapshot(&bytes) {
            return Err(format!(
                "fetch of {url} did not return a JSON object (got {} bytes); \
                 check the URL and Trunk static assets",
                bytes.len()
            ));
        }
        let label = url
            .rsplit('/')
            .next()
            .filter(|segment| !segment.is_empty())
            .unwrap_or(url)
            .to_owned();
        Ok((label, bytes))
    }

    fn response_is_html(response: &Response) -> bool {
        response
            .headers()
            .get("content-type")
            .ok()
            .flatten()
            .map(|value| value.to_lowercase().contains("text/html"))
            .unwrap_or(false)
    }
}

/// Map `?graph=` values to same-origin fetch URLs under `trunk serve`.
pub fn resolve_graph_fetch_url(query_value: &str) -> String {
    if query_value.starts_with("http://") || query_value.starts_with("https://") {
        return query_value.to_owned();
    }
    let path = if query_value.starts_with('/') {
        query_value.to_owned()
    } else {
        format!("/{query_value}")
    };
    if path == format!("/{STANDARD_GRAPH_SNAPSHOT_BASENAME}") {
        return STANDARD_GRAPH_SNAPSHOT_FIXTURE_URL.to_owned();
    }
    if path == format!("/{TRAJECTORY_MULTI_GEN_SNAPSHOT_BASENAME}") {
        return TRAJECTORY_MULTI_GEN_SNAPSHOT_FIXTURE_URL.to_owned();
    }
    path
}

fn looks_like_local_filesystem_path(value: &str) -> bool {
    value.starts_with("/home/")
        || value.starts_with("/Users/")
        || value.starts_with("/tmp/")
        || value.contains("/.ploke")
        || (value.len() > 2 && value.as_bytes()[1] == b':' && value.as_bytes()[2] == b'\\')
}

fn looks_like_json_snapshot(bytes: &[u8]) -> bool {
    bytes
        .iter()
        .copied()
        .skip_while(|byte| byte.is_ascii_whitespace())
        .next()
        == Some(b'{')
}

/// When set, startup should surface this instead of fetching.
pub fn graph_fetch_rejection_message(query_value: &str) -> Option<String> {
    if looks_like_local_filesystem_path(query_value) {
        return Some(format!(
            "Browser cannot open local path {query_value:?}; use the file picker or \
             ?graph={STANDARD_GRAPH_SNAPSHOT_FIXTURE_URL}"
        ));
    }
    if query_value.contains("crates/ploke-egui/")
        || (!query_value.starts_with('/') && query_value.contains("benchmark-fixtures/"))
    {
        return Some(format!(
            "Browser cannot open repo path {query_value:?}; use the file picker or \
             ?graph={STANDARD_GRAPH_SNAPSHOT_FIXTURE_URL}"
        ));
    }
    None
}

#[cfg(target_arch = "wasm32")]
use wasm::WasmGraphLoad;

/// Parse `?theme=<id>` from a `location.search` string (leading `?` optional).
pub fn theme_query_id_from_search(search: &str) -> Option<String> {
    query_param_from_search(search, "theme")
}

#[cfg(target_arch = "wasm32")]
pub fn startup_theme_id_from_location() -> Option<String> {
    let window = web_sys::window()?;
    let search = window.location().search().ok()?;
    theme_query_id_from_search(&search)
}

/// Parse `?graph=<url>` from a `location.search` string (leading `?` optional).
pub fn graph_query_url_from_search(search: &str) -> Option<String> {
    query_param_from_search(search, "graph")
}

fn query_param_from_search(search: &str, key: &str) -> Option<String> {
    let trimmed = search.trim().trim_start_matches('?');
    for pair in trimmed.split('&') {
        let (param, value) = pair.split_once('=')?;
        if param == key {
            let decoded = decode_query_component(value);
            return (!decoded.is_empty()).then_some(decoded);
        }
    }
    None
}

fn decode_query_component(value: &str) -> String {
    #[cfg(target_arch = "wasm32")]
    {
        return js_sys::decode_uri_component(value)
            .ok()
            .and_then(|decoded| decoded.as_string())
            .unwrap_or_else(|| value.replace('+', " "));
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        percent_decode_query_value(value)
    }
}

pub fn percent_decode_query_value(input: &str) -> String {
    let mut out = Vec::with_capacity(input.len());
    let bytes = input.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        match bytes[index] {
            b'+' => {
                out.push(b' ');
                index += 1;
            }
            b'%' if index + 2 < bytes.len() => {
                let hex = &input[index + 1..index + 3];
                if let Ok(byte) = u8::from_str_radix(hex, 16) {
                    out.push(byte);
                    index += 3;
                    continue;
                }
                out.push(bytes[index]);
                index += 1;
            }
            byte => {
                out.push(byte);
                index += 1;
            }
        }
    }
    String::from_utf8(out).unwrap_or_else(|_| input.to_owned())
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    #[test]
    fn resolve_graph_fetch_url_maps_bare_snapshot_basename() {
        assert_eq!(
            resolve_graph_fetch_url(STANDARD_GRAPH_SNAPSHOT_BASENAME),
            STANDARD_GRAPH_SNAPSHOT_FIXTURE_URL
        );
        assert_eq!(
            resolve_graph_fetch_url(&format!("/{STANDARD_GRAPH_SNAPSHOT_BASENAME}")),
            STANDARD_GRAPH_SNAPSHOT_FIXTURE_URL
        );
        assert_eq!(
            resolve_graph_fetch_url(STANDARD_GRAPH_SNAPSHOT_FIXTURE_URL),
            STANDARD_GRAPH_SNAPSHOT_FIXTURE_URL
        );
        assert_eq!(
            resolve_graph_fetch_url(TRAJECTORY_MULTI_GEN_SNAPSHOT_BASENAME),
            TRAJECTORY_MULTI_GEN_SNAPSHOT_FIXTURE_URL
        );
    }

    #[test]
    fn graph_fetch_rejection_message_rejects_repo_paths() {
        let message = graph_fetch_rejection_message(
            "crates/ploke-egui/benchmark-fixtures/protocol-graph.json",
        )
        .expect("repo path");
        assert!(message.contains("Browser cannot open"));
        assert!(message.contains("file picker"));
    }

    #[test]
    fn graph_query_url_from_search_decodes_graph_param() {
        assert_eq!(
            graph_query_url_from_search("?graph=%2Fbenchmark-fixtures%2Fx.json"),
            Some("/benchmark-fixtures/x.json".to_owned())
        );
        assert_eq!(
            graph_query_url_from_search("graph=fixture.json"),
            Some("fixture.json".to_owned())
        );
        assert_eq!(graph_query_url_from_search("?other=1"), None);
    }

    #[test]
    fn theme_query_id_from_search_decodes_theme_param() {
        assert_eq!(
            theme_query_id_from_search("?theme=gruvbox_light"),
            Some("gruvbox_light".to_owned())
        );
        assert_eq!(
            theme_query_id_from_search("?graph=x&theme=tokyo_night"),
            Some("tokyo_night".to_owned())
        );
        assert_eq!(theme_query_id_from_search("?graph=x"), None);
    }

    #[test]
    fn graph_from_snapshot_bytes_loads_standard_benchmark_fixture() {
        let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("benchmark-fixtures/standard-prototype1-graph-snapshot.json");
        let bytes = std::fs::read(&fixture).expect("read benchmark fixture");
        let graph = graph_from_snapshot_bytes(&bytes).expect("decode fixture snapshot");
        assert!(!graph.history.blocks.is_empty());
        assert!(!graph.candidates.candidates.is_empty());
    }
}
