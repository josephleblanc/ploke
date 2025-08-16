#![allow(unused_variables, unused_imports, dead_code)]

// TODO:
//
// 1 Add serialization support for saving/loading conversations
// 2 Implement scrolling through long message histories
// 3 Add visual indicators for branch points
// 4 Implement sibling navigation (up/down between children of same parent)
// 5 Add color coding for different message types (user vs assistant)

use ploke_tui::{tracing_setup, try_main};

#[tokio::main]
async fn main() -> color_eyre::Result<()> {
    // let _guard = tracing_setup::init_tracing();
    //  TODO: Getting weird stuff writing to terminal, might be this.
    //  Look more into it later
    // color_eyre::config::HookBuilder::default()
    //     .display_location_section(false)
    //     .install()?;

    if let Err(e) = try_main().await {
        tracing::error!(error = %e, "Application error");
        return Err(e);
    }
    tracing::info!("Application exited normally");
    Ok(())
}

