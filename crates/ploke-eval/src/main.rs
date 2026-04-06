mod tracing_setup;

use clap::Parser;
use ploke_eval::Cli;
use std::process::ExitCode;

#[tokio::main]
async fn main() -> ExitCode {
    let _log_guard = tracing_setup::init_tracing();
    Cli::parse().run().await
}
