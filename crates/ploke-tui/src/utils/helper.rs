pub fn truncate_string(s: &str, max: usize) -> String {
    if s.len() > max {
        format!("{}...", &s[..std::cmp::min(s.len(), max)])
    } else {
        s.to_string()
    }
}
use tokio::fs;

pub async fn find_file_by_prefix(
    dir: impl AsRef<std::path::Path>,
    prefix: &str,
) -> std::io::Result<Option<std::path::PathBuf>> {
    let mut entries = fs::read_dir(dir).await?;
    while let Some(entry) = entries.next_entry().await? {
        let name = entry.file_name();
        if let Some(name_str) = name.to_str() {
            tracing::debug!(
                "
checking file to load:                  | {name_str}
name_str.starts_with(prefix):           | {}
name_str.len() == prefix.len() + 1 + 36 | {}
",
                name_str.starts_with(prefix),
                name_str.len() == prefix.len() + 1 + 36
            );
            if name_str.starts_with(prefix) && name_str.len() == prefix.len() + 1 + 36 {
                tracing::debug!("passes checks: {}", name_str);
                // fixture_tracking_hash_aa1d3812-abb4-5d05-a69f-fe80aa856e3d
                // prefix + '_' + 36-char UUID
                return Ok(Some(entry.path()));
            }
        }
    }
    Ok(None)
}
