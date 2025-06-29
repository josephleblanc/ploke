use chrono::Local;
use std::{
    fs,
    hash::{Hash, Hasher},
    io,
    path::{Path, PathBuf},
};
use tracing_subscriber::{
    fmt::{self, writer::MakeWriterExt},
    EnvFilter,
};

const MAX_LOG_FILES: usize = 5;
const LOG_DIR: &str = ".tracing";

/// Initializes the tracing subscriber with file rotation.
pub fn init_tracing() -> Result<(), io::Error> {
    let log_dir = Path::new(LOG_DIR);
    fs::create_dir_all(log_dir)?;

    manage_log_files(log_dir)?;

    let file_appender = tracing_appender::rolling::never(log_dir, generate_log_filename());
    let (non_blocking_appender, _guard) = tracing_appender::non_blocking(file_appender);

    let subscriber = fmt::Subscriber::builder()
        .with_writer(non_blocking_appender.with_max_level(tracing::Level::TRACE))
        .with_env_filter(EnvFilter::from_default_env().add_directive("ploke=trace".parse().unwrap()))
        .json()
        .finish();

    tracing::subscriber::set_global_default(subscriber)
        .expect("Failed to set global default subscriber");

    Ok(())
}

/// Generates a unique log filename based on the current timestamp and a short hash.
fn generate_log_filename() -> String {
    let timestamp = Local::now().format("%Y-%m-%d_%H-%M-%S").to_string();
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    timestamp.hash(&mut hasher);
    let hash = hasher.finish();
    format!("{}-{:x}.log", timestamp, hash & 0xFFFFFF)
}

/// Manages log files in the specified directory, ensuring only `MAX_LOG_FILES` are kept.
fn manage_log_files(log_dir: &Path) -> Result<(), io::Error> {
    let entries = fs::read_dir(log_dir)?
        .filter_map(Result::ok)
        .map(|res| res.path())
        .collect::<Vec<PathBuf>>();

    if entries.len() < MAX_LOG_FILES {
        return Ok(());
    }

    let oldest_file = entries
        .into_iter()
        .filter_map(|path| {
            let metadata = fs::metadata(&path).ok()?;
            let created = metadata.created().ok()?;
            Some((path, created))
        })
        .min_by_key(|&(_, created)| created)
        .map(|(path, _)| path);

    if let Some(oldest) = oldest_file {
        fs::remove_file(oldest)?;
    }

    Ok(())
}
