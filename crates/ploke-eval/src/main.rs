use clap::{Parser, error::ErrorKind};
use ploke_eval::Cli;
use std::process::ExitCode;

// Debug Prototype 1 walk jobs exceed Tokio's 2 MiB default worker stack.
const RUNTIME_STACK_BYTES: usize = 8 * 1024 * 1024;

fn main() -> ExitCode {
    let runtime = match build_runtime() {
        Ok(runtime) => runtime,
        Err(error) => {
            eprintln!("failed to build the ploke-eval async runtime: {error}");
            return ExitCode::FAILURE;
        }
    };
    runtime.block_on(run_cli())
}

fn build_runtime() -> std::io::Result<tokio::runtime::Runtime> {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .thread_stack_size(RUNTIME_STACK_BYTES)
        .build()
}

async fn run_cli() -> ExitCode {
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

    let _log_guard = ploke_eval::tracing_setup::init_tracing(cli.debug_tools);
    cli.run().await
}

#[cfg(test)]
mod tests {
    const STACK_PROBE_CHILD: &str = "PLOKE_EVAL_STACK_PROBE_CHILD";
    const PROBE_FRAME_BYTES: usize = 64 * 1024;
    const PROBE_DEPTH: usize = 40;

    #[test]
    fn runtime_worker_stack_supports_walk_debug_frames() {
        if std::env::var_os(STACK_PROBE_CHILD).is_none() {
            let status = std::process::Command::new(
                std::env::current_exe().expect("resolve ploke-eval test executable"),
            )
            .args([
                "--exact",
                "tests::runtime_worker_stack_supports_walk_debug_frames",
                "--nocapture",
            ])
            .env(STACK_PROBE_CHILD, "1")
            .status()
            .expect("run isolated runtime stack probe");
            assert!(status.success(), "runtime stack probe exited with {status}");
            return;
        }

        let runtime = super::build_runtime().expect("build ploke-eval runtime");
        let total = runtime
            .block_on(async { tokio::spawn(async { consume_stack(PROBE_DEPTH) }).await })
            .expect("stack probe task completes");

        assert_eq!(total, PROBE_DEPTH * (PROBE_DEPTH + 1) / 2);
    }

    #[inline(never)]
    fn consume_stack(depth: usize) -> usize {
        let mut frame = [0_u8; PROBE_FRAME_BYTES];
        frame[0] = depth as u8;
        std::hint::black_box(&mut frame);
        let nested = if depth == 0 {
            0
        } else {
            consume_stack(depth - 1)
        };
        std::hint::black_box(&frame);
        nested + usize::from(frame[0])
    }
}
