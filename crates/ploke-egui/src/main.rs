#[cfg(not(target_arch = "wasm32"))]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    ploke_egui::native::run()
}

#[cfg(target_arch = "wasm32")]
fn main() {}
