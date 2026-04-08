mod tracing_setup;

use clap::{Parser, error::ErrorKind};
use ploke_eval::Cli;
use std::process::ExitCode;

#[tokio::main]
async fn main() -> ExitCode {
    let cli = match Cli::try_parse() {
        Ok(cli) => cli,
        Err(err) => {
            let kind = err.kind();
            let _ = err.print();
            if kind == ErrorKind::DisplayHelp || kind == ErrorKind::DisplayVersion {
                return ExitCode::SUCCESS;
            }
            return ExitCode::FAILURE;
        }
    };

    let _log_guard = tracing_setup::init_tracing(cli.debug_tools);
    cli.run().await
}
