use crate::types::{McpError, ServerId};
use serde::Deserialize;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

/// Specification for starting and managing a single MCP server.
#[derive(Debug, Clone)]
pub struct ServerSpec {
    pub id: ServerId,
    pub command: String,
    pub args: Vec<String>,
    pub env: BTreeMap<String, String>,
    pub autostart: bool,
    pub restart_on_exit: bool,
    /// Optional default timeout for tool calls on this server (milliseconds).
    pub default_timeout_ms: Option<u64>,
    /// Lower value = higher priority
    pub priority: u8,
}

/** Raw TOML mapping for [servers.<id>] tables. */
#[derive(Debug, Deserialize)]
struct RawServer {
    #[serde(default)]
    id: Option<String>,
    command: String,
    #[serde(default)]
    args: Option<Vec<String>>,
    #[serde(default)]
    env: Option<BTreeMap<String, String>>,
    #[serde(default)]
    autostart: Option<bool>,
    #[serde(default)]
    restart_on_exit: Option<bool>,
    #[serde(default)]
    default_timeout_ms: Option<u64>,
    #[serde(default)]
    priority: Option<u8>,
}

#[derive(Debug, Deserialize)]
struct RawConfig {
    servers: BTreeMap<String, RawServer>,
}

/// Top-level configuration listing all known servers.
#[derive(Debug, Clone, Default)]
pub struct McpConfig {
    pub servers: Vec<ServerSpec>,
}

impl McpConfig {
    /// Get a server spec by id.
    pub fn get(&self, id: &ServerId) -> Option<&ServerSpec> {
        self.servers.iter().find(|s| &s.id == id)
    }

    /// Parse a configuration from a TOML string using the documented schema.
    ///
    /// Example TOML:
    /// [servers.git]
    /// id = "git"
    /// command = "uvx"
    /// args = ["mcp-server-git"]
    /// autostart = true
    /// priority = 0
    pub fn from_toml_str(input: &str) -> Result<Self, McpError> {
        let raw: RawConfig = toml::from_str(input)
            .map_err(|e| McpError::Config(format!("TOML parse error: {}", e)))?;

        let mut servers = Vec::new();
        for (key, raw_srv) in raw.servers {
            let id_str = raw_srv.id.unwrap_or_else(|| key.clone());
            let spec = ServerSpec {
                id: ServerId(id_str),
                command: raw_srv.command,
                args: raw_srv.args.unwrap_or_default().into_iter().collect(),
                env: raw_srv.env.unwrap_or_default().into_iter().collect(),
                autostart: raw_srv.autostart.unwrap_or(false),
                restart_on_exit: raw_srv.restart_on_exit.unwrap_or(false),
                default_timeout_ms: raw_srv.default_timeout_ms,
                priority: raw_srv.priority.unwrap_or(100),
            };
            servers.push(spec);
        }

        // Sort by priority ascending (lower = more important)
        servers.sort_by_key(|s| s.priority);

        Ok(McpConfig { servers })
    }

    /// Load configuration from a specific file path.
    pub fn load_from_path(path: impl AsRef<Path>) -> Result<Self, McpError> {
        let path = path.as_ref();
        let content = fs::read_to_string(path)?;
        Self::from_toml_str(&content)
    }

    /// Compute the default config file path.
    /// Uses $XDG_CONFIG_HOME/ploke/mcp.toml or ~/.config/ploke/mcp.toml.
    pub fn default_config_path() -> Result<PathBuf, McpError> {
        let base = std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .or_else(|| dirs::home_dir().map(|p| p.join(".config")))
            .ok_or_else(|| McpError::Config("Could not determine config directory".into()))?;
        Ok(base.join("ploke").join("mcp.toml"))
    }

    /// Load configuration from the default path.
    /// Returns both the parsed config and the resolved path for diagnostics.
    pub fn load_default_file() -> Result<(Self, PathBuf), McpError> {
        let path = Self::default_config_path()?;
        let cfg = Self::load_from_path(&path)?;
        Ok((cfg, path))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_parsing_sorting_and_defaults() {
        let toml = r#"
[servers.git]
id = "git"
command = "uvx"
args = ["mcp-server-git"]
autostart = true
priority = 0

[servers.context7]
command = "npx"
args = ["-y", "@upstash/context7-mcp"]
restart_on_exit = false
default_timeout_ms = 15000
priority = 1

[servers.foo]
command = "/opt/custom/foo"
priority = 10
"#;

        let cfg = McpConfig::from_toml_str(toml).expect("parse ok");
        assert_eq!(cfg.servers.len(), 3, "should parse three servers");

        // Sorted by ascending priority: 0, 1, 10
        let ids: Vec<String> = cfg.servers.iter().map(|s| s.id.0.clone()).collect();
        assert_eq!(ids, vec!["git", "context7", "foo"]);

        // Defaults and explicit fields
        let git = &cfg.servers[0];
        assert_eq!(git.id.0, "git");
        assert_eq!(git.command, "uvx");
        assert_eq!(git.args, vec!["mcp-server-git"]);
        assert!(git.autostart, "git autostart should be true");
        assert_eq!(git.priority, 0);
        assert_eq!(git.env.len(), 0);
        assert_eq!(git.default_timeout_ms, None);

        let ctx = &cfg.servers[1];
        assert_eq!(ctx.id.0, "context7", "id should default from table key");
        assert_eq!(ctx.command, "npx");
        assert_eq!(ctx.args, vec!["-y", "@upstash/context7-mcp"]);
        assert!(
            !ctx.autostart,
            "autostart defaults to false when not provided"
        );
        assert_eq!(ctx.priority, 1);
        assert_eq!(ctx.default_timeout_ms, Some(15_000));

        let foo = &cfg.servers[2];
        assert_eq!(foo.id.0, "foo");
        assert_eq!(foo.command, "/opt/custom/foo");
        assert_eq!(foo.args.len(), 0, "args defaults to empty");
        assert!(!foo.autostart);
        assert_eq!(foo.priority, 10);
    }

    #[test]
    fn default_config_path_shape() {
        let path = McpConfig::default_config_path().expect("path");
        assert_eq!(path.file_name().and_then(|s| s.to_str()), Some("mcp.toml"));
        assert_eq!(path.extension().and_then(|s| s.to_str()), Some("toml"));
    }
}
