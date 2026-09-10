use anyhow::{bail, Context, Result};
use colored::*;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::{self, Write};
use std::path::PathBuf;

use crate::ai::client::{OllamaClient, OllamaOptions};

fn default_ollama_url() -> String {
    "http://localhost:11434".to_string()
}

fn default_model_str() -> String {
    "".to_string()
}

fn default_temperature() -> f32 {
    0.2
}

fn default_top_k() -> u32 {
    40
}

fn default_context_size() -> u32 {
    4096
}

fn default_report_output_dir() -> String {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/home/clever".to_string());
    format!("{}/.local/share/bioclean/reports", home)
}

fn default_report_formatter() -> String {
    "auto".to_string()
}

fn default_rollback_log_dir() -> String {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/home/clever".to_string());
    format!("{}/.local/share/bioclean/history", home)
}

fn default_history_file() -> String {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/home/clever".to_string());
    format!("{}/.aidbio/history/bioclean_sessions.json", home)
}

fn default_scratch_dirs() -> Vec<String> {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/home/clever".to_string());
    vec![
        "/tmp".to_string(),
        "/var/tmp".to_string(),
        format!("{}/aidbio/ds", home),
    ]
}

fn default_safety_interlock() -> bool {
    true
}

fn default_log_vacuum_days() -> u32 {
    7
}

fn default_tmp_min_age_hours() -> u32 {
    48
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    #[serde(default = "default_ollama_url")]
    pub ollama_url: String,

    #[serde(default = "default_model_str")]
    pub default_model: String,

    #[serde(default = "default_temperature")]
    pub temperature: f32,

    #[serde(default = "default_top_k")]
    pub top_k: u32,

    #[serde(default = "default_context_size")]
    pub context_size: u32,

    #[serde(default = "default_report_output_dir")]
    pub report_output_dir: String,

    #[serde(default = "default_report_formatter")]
    pub report_formatter: String,

    #[serde(default = "default_rollback_log_dir")]
    pub rollback_log_dir: String,

    #[serde(default = "default_history_file")]
    pub history_file: String,

    #[serde(default = "default_scratch_dirs")]
    pub scratch_dirs: Vec<String>,

    #[serde(default = "default_safety_interlock")]
    pub safety_interlock: bool,

    #[serde(default = "default_log_vacuum_days")]
    pub log_vacuum_days: u32,

    #[serde(default = "default_tmp_min_age_hours")]
    pub tmp_min_age_hours: u32,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            ollama_url: default_ollama_url(),
            default_model: default_model_str(),
            temperature: default_temperature(),
            top_k: default_top_k(),
            context_size: default_context_size(),
            report_output_dir: default_report_output_dir(),
            report_formatter: default_report_formatter(),
            rollback_log_dir: default_rollback_log_dir(),
            history_file: default_history_file(),
            scratch_dirs: default_scratch_dirs(),
            safety_interlock: default_safety_interlock(),
            log_vacuum_days: default_log_vacuum_days(),
            tmp_min_age_hours: default_tmp_min_age_hours(),
        }
    }
}

pub struct ConfigParamMeta {
    pub key: &'static str,
    pub description: &'static str,
    pub example: &'static str,
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

    pub fn ollama_options(&self) -> OllamaOptions {
        OllamaOptions {
            temperature: Some(self.temperature),
            top_k: Some(self.top_k),
            num_ctx: Some(self.context_size),
        }
    }

    pub fn list_param_meta() -> Vec<ConfigParamMeta> {
        vec![
            ConfigParamMeta {
                key: "ollama_url",
                description: "Base URL for the local or remote Ollama API endpoint",
                example: "http://localhost:11434",
            },
            ConfigParamMeta {
                key: "default_model",
                description: "Default Ollama model name (auto-detected if blank)",
                example: "qwen2.5-coder:7b",
            },
            ConfigParamMeta {
                key: "temperature",
                description: "Sampling temperature for AI model generation (0.0 - 2.0)",
                example: "0.2",
            },
            ConfigParamMeta {
                key: "top_k",
                description: "Top-K sampling limit for AI token generation (1 - 100)",
                example: "40",
            },
            ConfigParamMeta {
                key: "context_size",
                description: "Context window token limit (num_ctx) for Ollama (>= 512)",
                example: "4096",
            },
            ConfigParamMeta {
                key: "report_output_dir",
                description: "Directory where Markdown system health reports are saved",
                example: "~/.local/share/bioclean/reports",
            },
            ConfigParamMeta {
                key: "report_formatter",
                description: "Preferred terminal report viewer: auto, glow, batcat, or terminal",
                example: "glow",
            },
            ConfigParamMeta {
                key: "rollback_log_dir",
                description: "Directory for paired session records and rollback manifests",
                example: "~/.local/share/bioclean/history",
            },
            ConfigParamMeta {
                key: "history_file",
                description: "Path to global history session registry JSON",
                example: "~/.aidbio/history/bioclean_sessions.json",
            },
            ConfigParamMeta {
                key: "scratch_dirs",
                description: "Comma-separated directories eligible for heavy scratch audits",
                example: "/tmp,/var/tmp,/home/clever/aidbio/ds",
            },
            ConfigParamMeta {
                key: "safety_interlock",
                description: "Enable AI safety verification prompt before destructive actions",
                example: "true",
            },
            ConfigParamMeta {
                key: "log_vacuum_days",
                description: "Number of days of systemd journal logs to retain during vacuum",
                example: "7",
            },
            ConfigParamMeta {
                key: "tmp_min_age_hours",
                description: "Minimum age in hours for temporary files to be eligible for deletion",
                example: "48",
            },
        ]
    }

    pub fn get_param(&self, key: &str) -> Result<String> {
        let val = match key.to_lowercase().as_str() {
            "ollama_url" => self.ollama_url.clone(),
            "default_model" | "model" => self.default_model.clone(),
            "temperature" => self.temperature.to_string(),
            "top_k" | "topk" => self.top_k.to_string(),
            "context_size" | "num_ctx" => self.context_size.to_string(),
            "report_output_dir" | "report_dir" => self.report_output_dir.clone(),
            "report_formatter" | "formatter" => self.report_formatter.clone(),
            "rollback_log_dir" | "rollback_dir" => self.rollback_log_dir.clone(),
            "history_file" => self.history_file.clone(),
            "scratch_dirs" => self.scratch_dirs.join(","),
            "safety_interlock" => self.safety_interlock.to_string(),
            "log_vacuum_days" => self.log_vacuum_days.to_string(),
            "tmp_min_age_hours" => self.tmp_min_age_hours.to_string(),
            _ => bail!(
                "Unknown configuration key: '{}'. Run 'bioclean config --list' to view all settings.",
                key
            ),
        };
        Ok(val)
    }

    pub fn set_param(&mut self, key: &str, value: &str) -> Result<()> {
        match key.to_lowercase().as_str() {
            "ollama_url" => {
                let trimmed = value.trim().trim_end_matches('/');
                if !trimmed.starts_with("http://") && !trimmed.starts_with("https://") {
                    bail!("Invalid ollama_url: must start with http:// or https://");
                }
                self.ollama_url = trimmed.to_string();
            }
            "default_model" | "model" => {
                self.default_model = value.trim().to_string();
            }
            "temperature" => {
                let temp: f32 = value.trim().parse().context("Temperature must be a valid float (e.g. 0.2)")?;
                if !(0.0..=2.0).contains(&temp) {
                    bail!("Temperature must be between 0.0 and 2.0");
                }
                self.temperature = temp;
            }
            "top_k" | "topk" => {
                let topk: u32 = value.trim().parse().context("Top-K must be a positive integer (e.g. 40)")?;
                if topk == 0 || topk > 200 {
                    bail!("Top-K must be between 1 and 200");
                }
                self.top_k = topk;
            }
            "context_size" | "num_ctx" => {
                let ctx: u32 = value.trim().parse().context("Context size must be an integer >= 512 (e.g. 4096)")?;
                if ctx < 512 {
                    bail!("Context size must be at least 512 tokens");
                }
                self.context_size = ctx;
            }
            "report_output_dir" | "report_dir" => {
                let path_str = shellexpand_tilde(value.trim());
                self.report_output_dir = path_str;
            }
            "report_formatter" | "formatter" => {
                let fmt = value.trim().to_lowercase();
                if !["auto", "glow", "bat", "batcat", "terminal"].contains(&fmt.as_str()) {
                    bail!("Invalid formatter: '{}'. Supported: auto, glow, batcat, terminal", fmt);
                }
                self.report_formatter = fmt;
            }
            "rollback_log_dir" | "rollback_dir" => {
                let path_str = shellexpand_tilde(value.trim());
                self.rollback_log_dir = path_str;
            }
            "history_file" => {
                let path_str = shellexpand_tilde(value.trim());
                self.history_file = path_str;
            }
            "scratch_dirs" => {
                self.scratch_dirs = value
                    .split(',')
                    .map(|s| shellexpand_tilde(s.trim()))
                    .filter(|s| !s.is_empty())
                    .collect();
            }
            "safety_interlock" => {
                let lower = value.trim().to_lowercase();
                self.safety_interlock = matches!(lower.as_str(), "true" | "1" | "yes" | "on");
            }
            "log_vacuum_days" => {
                let days: u32 = value.trim().parse().context("log_vacuum_days must be a positive integer")?;
                if days == 0 {
                    bail!("log_vacuum_days must be at least 1 day");
                }
                self.log_vacuum_days = days;
            }
            "tmp_min_age_hours" => {
                let hours: u32 = value.trim().parse().context("tmp_min_age_hours must be a positive integer")?;
                self.tmp_min_age_hours = hours;
            }
            _ => bail!(
                "Unknown configuration key: '{}'. Run 'bioclean config --list' to see valid keys.",
                key
            ),
        }
        self.save()
    }

    pub fn print_list(&self) {
        println!("{}", "\n⚙️  bioclean System Configuration:".bright_cyan().bold());
        println!("{}", "=".repeat(85).dimmed());

        for meta in Self::list_param_meta() {
            let current_val = self.get_param(meta.key).unwrap_or_else(|_| "n/a".to_string());
            println!(
                "  • {:<20} : {}",
                meta.key.bright_yellow(),
                if current_val.is_empty() { "(auto-detected)".dimmed().to_string() } else { current_val.bright_green().bold().to_string() }
            );
            println!("    {:<20}   {} (e.g. {})", "".dimmed(), meta.description.dimmed(), meta.example.cyan());
        }
        println!("{}", "=".repeat(85).dimmed());
        println!("Configuration file: {}\n", Self::config_path().display().to_string().bright_cyan());
    }

    pub fn run_wizard(&mut self, ollama: &OllamaClient) -> Result<()> {
        println!("{}", "\n🧙 bioclean Interactive Configuration Wizard".bright_purple().bold());
        println!("{}", "Press [Enter] to keep the current value shown in brackets.\n".dimmed());

        // 1. Ollama URL
        let url_prompt = format!("1. Ollama Base URL [{}]", self.ollama_url);
        let new_url = prompt_line(&url_prompt)?;
        if !new_url.is_empty() {
            self.set_param("ollama_url", &new_url)?;
        }

        // 2. Model Selection
        println!("\n2. AI Model Selection:");
        if ollama.is_online() {
            match ollama.list_models() {
                Ok(models) if !models.is_empty() => {
                    println!("   Found installed Ollama models:");
                    for (i, m) in models.iter().enumerate() {
                        let tag = if self.default_model == *m { " (current)" } else { "" };
                        println!("     [{}] {}{}", (i + 1).to_string().cyan(), m.bold(), tag.green());
                    }
                    println!("     [c] Enter custom model name");
                    let sel = prompt_line(&format!("   Select model [1-{}] or keep current [{}]", models.len(), self.default_model))?;
                    if let Ok(idx) = sel.parse::<usize>() {
                        if idx >= 1 && idx <= models.len() {
                            self.set_param("default_model", &models[idx - 1])?;
                        }
                    } else if sel.to_lowercase() == "c" {
                        let custom = prompt_line("   Custom model name")?;
                        if !custom.is_empty() {
                            self.set_param("default_model", &custom)?;
                        }
                    } else if !sel.is_empty() {
                        self.set_param("default_model", &sel)?;
                    }
                }
                _ => {
                    let m = prompt_line(&format!("   AI Model [{}]", self.default_model))?;
                    if !m.is_empty() {
                        self.set_param("default_model", &m)?;
                    }
                }
            }
        } else {
            println!("   {} Ollama is currently offline at {}", "ℹ".yellow(), self.ollama_url.dimmed());
            let m = prompt_line(&format!("   AI Model [{}]", self.default_model))?;
            if !m.is_empty() {
                self.set_param("default_model", &m)?;
            }
        }

        // 3. AI Temperature
        let temp_prompt = format!("\n3. AI Temperature (0.0 - 2.0) [{}]", self.temperature);
        let new_temp = prompt_line(&temp_prompt)?;
        if !new_temp.is_empty() {
            self.set_param("temperature", &new_temp)?;
        }

        // 4. AI Top-K
        let topk_prompt = format!("4. AI Top-K (1 - 200) [{}]", self.top_k);
        let new_topk = prompt_line(&topk_prompt)?;
        if !new_topk.is_empty() {
            self.set_param("top_k", &new_topk)?;
        }

        // 5. Context Size (num_ctx)
        let ctx_prompt = format!("5. Context Size in tokens [{}]", self.context_size);
        let new_ctx = prompt_line(&ctx_prompt)?;
        if !new_ctx.is_empty() {
            self.set_param("context_size", &new_ctx)?;
        }

        // 6. Report Formatter
        let fmt_prompt = format!("\n6. Report Formatter (auto, glow, batcat, terminal) [{}]", self.report_formatter);
        let new_fmt = prompt_line(&fmt_prompt)?;
        if !new_fmt.is_empty() {
            self.set_param("report_formatter", &new_fmt)?;
        }

        // 7. Report Output Directory
        let rep_prompt = format!("7. Report Output Directory [{}]", self.report_output_dir);
        let new_rep = prompt_line(&rep_prompt)?;
        if !new_rep.is_empty() {
            self.set_param("report_output_dir", &new_rep)?;
        }

        // 8. Rollback Log Directory
        let roll_prompt = format!("8. Rollback Log Directory [{}]", self.rollback_log_dir);
        let new_roll = prompt_line(&roll_prompt)?;
        if !new_roll.is_empty() {
            self.set_param("rollback_log_dir", &new_roll)?;
        }

        // 9. Safety Interlock
        let safe_prompt = format!("\n9. Safety Interlock Confirmation (true/false) [{}]", self.safety_interlock);
        let new_safe = prompt_line(&safe_prompt)?;
        if !new_safe.is_empty() {
            self.set_param("safety_interlock", &new_safe)?;
        }

        // 10. Log Retention Days
        let log_prompt = format!("10. Journalctl Log Retention Days [{}]", self.log_vacuum_days);
        let new_log = prompt_line(&log_prompt)?;
        if !new_log.is_empty() {
            self.set_param("log_vacuum_days", &new_log)?;
        }

        // 11. Temp File Age Hours
        let tmp_prompt = format!("11. Stale Temp File Age Threshold (hours) [{}]", self.tmp_min_age_hours);
        let new_tmp = prompt_line(&tmp_prompt)?;
        if !new_tmp.is_empty() {
            self.set_param("tmp_min_age_hours", &new_tmp)?;
        }

        self.save()?;
        println!("\n{}", "✔ Configuration successfully saved!".bright_green().bold());
        println!("File: {}", Self::config_path().display().to_string().bright_cyan());
        Ok(())
    }
}

fn shellexpand_tilde(path_str: &str) -> String {
    if path_str.starts_with("~/") {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/home/clever".to_string());
        format!("{}{}", home, &path_str[1..])
    } else {
        path_str.to_string()
    }
}

fn prompt_line(msg: &str) -> Result<String> {
    print!("{}: ", msg);
    io::stdout().flush()?;
    let mut buf = String::new();
    io::stdin().read_line(&mut buf)?;
    Ok(buf.trim().to_string())
}

