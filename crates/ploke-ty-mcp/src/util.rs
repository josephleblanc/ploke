/**
Utilities for examples and tests.

Install a basic tracing subscriber if none is set yet. Honors RUST_LOG-like env filters.
Returns true if a subscriber was installed by this call, false if one already existed.
*/
pub fn init_tracing_once() -> bool {
    use tracing_subscriber::{fmt, EnvFilter};
    fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .try_init()
        .is_ok()
}
