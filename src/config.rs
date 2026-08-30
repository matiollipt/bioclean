use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub ollama_url: String,
    pub default_model: String,
    pub history_file: String,
    pub scratch_dirs: Vec<String>,
    pub safety_interlock: bool,
    pub log_vacuum_days: u32,
    pub tmp_min_age_hours: u32,
}

impl Default for Config {
    fn default() -> Self {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/home/clever".to_string());
        Self {
            ollama_url: "http://localhost:11434".to_string(),
            default_model: "".to_string(), // Auto-detected if empty
            history_file: format!("{}/.aidbio/history/bioclean_sessions.json", home),
            scratch_dirs: vec![
                "/tmp".to_string(),
                "/var/tmp".to_string(),
                format!("{}/aidbio/ds", home),
            ],
            safety_interlock: true,
            log_vacuum_days: 7,
            tmp_min_age_hours: 48,
        }
    }
}

impl Config {
    pub fn config_path() -> PathBuf {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/home/clever".to_string());
        PathBuf::from(home).join(".config/bioclean/config.toml")
    }

    pub fn load() -> Self {
        let path = Self::config_path();
        if path.exists() {
            if let Ok(content) = fs::read_to_string(&path) {
                if let Ok(cfg) = toml::from_str::<Config>(&content) {
                    return cfg;
                }
            }
        }
        let default_cfg = Self::default();
        let _ = default_cfg.save();
        default_cfg
    }

    pub fn save(&self) -> Result<()> {
        let path = Self::config_path();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let toml_str = toml::to_string_pretty(self)?;
        fs::write(path, toml_str)?;
        Ok(())
    }
}
