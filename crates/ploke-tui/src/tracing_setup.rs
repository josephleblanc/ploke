use tracing_subscriber::{EnvFilter, fmt, prelude::*};

pub fn init_tracing() {
    // log to stdout and a rolling file in the logs directory
    let file_appender = tracing_appender::rolling::daily("logs", "ploke.log");

    let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);

    tracing_subscriber::registry()
        .with(EnvFilter::from_default_env())
        .with(fmt::layer().with_writer(std::io::stdout))
        .with(fmt::layer().with_writer(non_blocking))
        .init();
}
