//! Web entry point for the operator graph UI.

use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::*;

use crate::demo::sample_graph;
use crate::ui::app::OperatorApp;

/// Canvas element id in `crates/ploke-egui/index.html` (Trunk shell).
pub const WEB_CANVAS_ID: &str = "ploke-operator-canvas";

#[derive(Clone)]
#[wasm_bindgen]
pub struct WebHandle {
    runner: eframe::WebRunner,
}

#[wasm_bindgen]
impl WebHandle {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self {
            runner: eframe::WebRunner::new(),
        }
    }

    #[wasm_bindgen]
    pub async fn start(&self, canvas: web_sys::HtmlCanvasElement) -> Result<(), JsValue> {
        self.runner
            .start(
                canvas,
                eframe::WebOptions::default(),
                Box::new(|cc| {
                    let mut app = OperatorApp::new(sample_graph());
                    app.graph_catalog_mut().enqueue_startup_graph_load();
                    if let Some(storage) = cc.storage {
                        app.load(storage);
                    }
                    if let Some(theme_id) = crate::bootstrap::startup_theme_id_from_location() {
                        app.apply_startup_theme_id(&cc.egui_ctx, &theme_id);
                    } else {
                        app.apply_theme_to_context(&cc.egui_ctx);
                    }
                    Ok(Box::new(app))
                }),
            )
            .await
    }

    #[wasm_bindgen]
    pub fn destroy(&self) {
        self.runner.destroy();
    }

    #[wasm_bindgen]
    pub fn has_panicked(&self) -> bool {
        self.runner.has_panicked()
    }
}

impl Default for WebHandle {
    fn default() -> Self {
        Self::new()
    }
}

/// Start the operator UI from the Trunk `index.html` canvas (used by `wasm_bindgen(start)`).
pub async fn start_from_document() -> Result<(), JsValue> {
    let window = web_sys::window().ok_or_else(|| JsValue::from_str("missing window"))?;
    let document = window
        .document()
        .ok_or_else(|| JsValue::from_str("missing document"))?;
    let canvas = document
        .get_element_by_id(WEB_CANVAS_ID)
        .ok_or_else(|| JsValue::from_str("missing ploke-operator-canvas"))?
        .dyn_into::<web_sys::HtmlCanvasElement>()?;
    WebHandle::new().start(canvas).await
}
