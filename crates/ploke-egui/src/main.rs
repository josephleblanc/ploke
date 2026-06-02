#[cfg(not(target_arch = "wasm32"))]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    ploke_egui::native::run()
}

#[cfg(target_arch = "wasm32")]
fn main() {}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen(start)]
pub fn wasm_start() -> Result<(), wasm_bindgen::JsValue> {
    wasm_bindgen_futures::spawn_local(async {
        if let Err(err) = ploke_egui::web::start_from_document().await {
            web_sys::console::error_1(&format!("ploke-egui failed to start: {err:?}").into());
        }
    });
    Ok(())
}
