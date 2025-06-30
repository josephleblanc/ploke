pub fn truncate_string(s: &str, max: usize) -> String {
    if s.len() > max {
        format!("{}...", &s[..std::cmp::min(s.len(), max)])
    } else {
        s.to_string()
    }
}
