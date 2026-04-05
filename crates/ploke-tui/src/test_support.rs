use std::sync::OnceLock;

use tokio::sync::Mutex as TokioMutex;

/// Shared lock for tests that mutate `XDG_CONFIG_HOME`.
///
/// This keeps the library test binary from racing when multiple modules need
/// to point the workspace registry at a temp config directory.
pub fn config_home_lock() -> &'static TokioMutex<()> {
    static LOCK: OnceLock<TokioMutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| TokioMutex::new(()))
}
