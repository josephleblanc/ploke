use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Config {
    pub openai_api_key: Option<String>,
    pub model: String,
    pub temperature: f32,
    pub max_history: usize,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            openai_api_key: None,
            model: "gpt-3.5-turbo".to_string(),
            temperature: 0.7,
            max_history: 100,
        }
    }
}

impl Config {
    pub fn load() -> Self {
        let config_path = Self::config_path();
        if config_path.exists() {
            match std::fs::read_to_string(config_path) {
                Ok(contents) => match toml::from_str(&contents) {
                    Ok(config) => config,
                    Err(e) => {
                        eprintln!("Error parsing config: {e}, using defaults");
                        Self::default()
                    }
                },
                Err(e) => {
                    eprintln!("Error reading config: {e}, using defaults");
                    Self::default()
                }
            }
        } else {
            Self::default()
        }
    }

    pub fn save(&self) -> Result<(), anyhow::Error> {
        let config_path = Self::config_path();
        let contents = toml::to_string_pretty(self)?;
        std::fs::write(config_path, contents)?;
        Ok(())
    }

    fn config_path() -> PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("ploke")
            .join("config.toml")
    }
}
