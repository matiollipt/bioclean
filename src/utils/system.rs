use anyhow::{Context, Result};
use colored::*;
use std::io::{self, Write};
use std::process::{Command, Stdio};

pub fn run_cmd_stdout(cmd: &str, args: &[&str]) -> Result<String> {
    let output = Command::new(cmd)
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .with_context(|| format!("Failed to execute command: {} {}", cmd, args.join(" ")))?;

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    Ok(stdout)
}

pub fn run_cmd_status(cmd: &str, args: &[&str]) -> Result<bool> {
    let status = Command::new(cmd)
        .args(args)
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()
        .with_context(|| format!("Failed to run: {} {}", cmd, args.join(" ")))?;
    Ok(status.success())
}

pub fn command_exists(cmd: &str) -> bool {
    Command::new("which")
        .arg(cmd)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

pub fn is_root() -> bool {
    let uid_output = run_cmd_stdout("id", &["-u"]).unwrap_or_default();
    uid_output.trim() == "0"
}

pub fn confirm_prompt(msg: &str, default_yes: bool) -> bool {
    let suffix = if default_yes { "[Y/n]" } else { "[y/N]" };
    print!("{} {} ", msg, suffix);
    let _ = io::stdout().flush();

    let mut input = String::new();
    if io::stdin().read_line(&mut input).is_err() {
        return default_yes;
    }
    let trimmed = input.trim().to_lowercase();
    if trimmed.is_empty() {
        return default_yes;
    }
    trimmed == "y" || trimmed == "yes"
}

/// Classifies an action's blast radius so confirmation prompts pick a sane default.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RiskLevel {
    /// Reversible or idempotent: journal vacuum, fstrim, config writes.
    Safe,
    /// Destructive or hard to undo: file/dataset deletion, dataset moves.
    Sensitive,
}

impl RiskLevel {
    pub fn default_yes(self) -> bool {
        matches!(self, RiskLevel::Safe)
    }
}

/// A structured "what will change" preview shown before a confirmation prompt,
/// so the user sees current state -> future state instead of a bare yes/no question.
pub struct ActionPreview {
    pub action: String,
    pub current_state: String,
    pub future_state: String,
    pub risk: RiskLevel,
}

impl ActionPreview {
    pub fn confirm(&self) -> bool {
        let warning = if self.risk == RiskLevel::Sensitive {
            format!("\n{}", "⚠ This action cannot be undone.".red())
        } else {
            String::new()
        };
        let msg = format!(
            "{}\n  Current: {}\n  After:   {}{}",
            self.action.bold(),
            self.current_state,
            self.future_state,
            warning
        );
        confirm_prompt(&msg, self.risk.default_yes())
    }
}

pub fn run_fstrim() -> Result<String> {
    if is_root() {
        run_cmd_stdout("fstrim", &["-av"])
    } else {
        run_cmd_stdout("sudo", &["fstrim", "-av"])
    }
}
