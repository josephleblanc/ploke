//! Current Ploke relative oracle score helpers.

/// Relative oracle score `resolved / configured`, preserving zero configured as absent.
pub fn oracle_rate(resolved: usize, configured: usize) -> Option<f64> {
    if configured == 0 {
        None
    } else {
        Some(resolved as f64 / configured as f64)
    }
}
