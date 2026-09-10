use anyhow::Result;
use chrono::Utc;
use colored::*;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

fn default_run_type() -> String {
    "MIGRATION".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OperationRecord {
    pub op_type: String,
    pub src: String,
    pub dest: String,
    #[serde(default)]
    pub bytes_moved: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionRecord {
    pub session_id: String,
    pub timestamp: String,
    pub status: String,
    #[serde(default = "default_run_type")]
    pub run_type: String,
    #[serde(default)]
    pub report_path: Option<String>,
    #[serde(default)]
    pub snapshot_path: Option<String>,
    #[serde(default)]
    pub rollback_manifest_path: Option<String>,
    #[serde(default)]
    pub operations: Vec<OperationRecord>,
}

pub struct HistoryManager {
    file_path: PathBuf,
}

impl HistoryManager {
    pub fn new(path_str: &str) -> Self {
        Self {
            file_path: PathBuf::from(path_str),
        }
    }

    pub fn load_sessions(&self) -> Result<Vec<SessionRecord>> {
        if !self.file_path.exists() {
            return Ok(Vec::new());
        }
        let content = fs::read_to_string(&self.file_path)?;
        if content.trim().is_empty() {
            return Ok(Vec::new());
        }
        let list: Vec<SessionRecord> = serde_json::from_str(&content).unwrap_or_default();
        Ok(list)
    }

    pub fn save_sessions(&self, sessions: &[SessionRecord]) -> Result<()> {
        if let Some(parent) = self.file_path.parent() {
            fs::create_dir_all(parent)?;
        }
        let json_str = serde_json::to_string_pretty(sessions)?;
        fs::write(&self.file_path, json_str)?;
        Ok(())
    }

    pub fn generate_run_id(prefix: &str) -> String {
        let now = Utc::now();
        let pid = std::process::id();
        let nanos = now.timestamp_subsec_nanos();
        let suffix = ((pid as u64 ^ nanos as u64) & 0xFFFF) as u16;
        format!("{}_{}_{:04x}", prefix, now.format("%Y%m%d_%H%M%S"), suffix)
    }

    pub fn get_session_dir(base_dir: &str, run_id: &str) -> PathBuf {
        PathBuf::from(base_dir).join(run_id)
    }

    pub fn create_paired_session_dir(base_dir: &str, run_id: &str) -> Result<PathBuf> {
        let path = Self::get_session_dir(base_dir, run_id);
        fs::create_dir_all(&path)?;
        Ok(path)
    }

    pub fn log_audit_trail(session_dir: &Path, message: &str) -> Result<()> {
        let audit_file = session_dir.join("audit_trail.log");
        let line = format!("[{}] {}\n", Utc::now().to_rfc3339(), message);
        let mut file = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(audit_file)?;
        use std::io::Write;
        file.write_all(line.as_bytes())?;
        Ok(())
    }

    pub fn start_session(&self) -> Result<String> {
        let session_id = Self::generate_run_id("session");
        let record = SessionRecord {
            session_id: session_id.clone(),
            timestamp: Utc::now().to_rfc3339(),
            status: "COMPLETED".to_string(),
            run_type: "MIGRATION".to_string(),
            report_path: None,
            snapshot_path: None,
            rollback_manifest_path: None,
            operations: Vec::new(),
        };
        self.record_session(record)?;
        Ok(session_id)
    }

    pub fn record_session(&self, record: SessionRecord) -> Result<()> {
        let mut sessions = self.load_sessions()?;
        if let Some(idx) = sessions.iter().position(|s| s.session_id == record.session_id) {
            sessions[idx] = record;
        } else {
            sessions.push(record);
        }
        self.save_sessions(&sessions)
    }

    pub fn record_op(&self, session_id: &str, op_type: &str, src: &str, dest: &str, bytes: u64) -> Result<()> {
        let mut sessions = self.load_sessions()?;
        if let Some(s) = sessions.iter_mut().find(|s| s.session_id == session_id) {
            s.operations.push(OperationRecord {
                op_type: op_type.to_string(),
                src: src.to_string(),
                dest: dest.to_string(),
                bytes_moved: bytes,
            });
        } else {
            let record = SessionRecord {
                session_id: session_id.to_string(),
                timestamp: Utc::now().to_rfc3339(),
                status: "COMPLETED".to_string(),
                run_type: "MIGRATION".to_string(),
                report_path: None,
                snapshot_path: None,
                rollback_manifest_path: None,
                operations: vec![OperationRecord {
                    op_type: op_type.to_string(),
                    src: src.to_string(),
                    dest: dest.to_string(),
                    bytes_moved: bytes,
                }],
            };
            sessions.push(record);
        }
        self.save_sessions(&sessions)
    }

    pub fn list_history(&self) -> Result<()> {
        let sessions = self.load_sessions()?;
        if sessions.is_empty() {
            println!("{}", "No prior migration or cleanup history found.".yellow());
            return Ok(());
        }

        println!("{}", "\n📜 Transaction & Diagnostic Session History:".bright_cyan().bold());
        println!("{}", "=".repeat(80).dimmed());

        for s in &sessions {
            let status_badge = if s.status == "COMPLETED" {
                s.status.green().bold()
            } else if s.status == "REVERTED" {
                s.status.yellow().bold()
            } else {
                s.status.red().bold()
            };
            println!("• Session ID : {}", s.session_id.bright_yellow());
            println!("  Type       : {}", s.run_type.cyan().bold());
            println!("  Status     : {}", status_badge);
            println!("  Timestamp  : {}", s.timestamp.dimmed());
            if let Some(ref r) = s.report_path {
                println!("  Report     : {}", r.bright_green());
            }
            if let Some(ref sn) = s.snapshot_path {
                println!("  Snapshot   : {}", sn.dimmed());
            }
            if let Some(ref rb) = s.rollback_manifest_path {
                println!("  Rollback   : {}", rb.bright_purple());
            }
            if !s.operations.is_empty() {
                println!("  Operations : {}", s.operations.len());
                for op in &s.operations {
                    println!("    └─ [{}] {}  =>  {}", op.op_type.cyan(), op.src, op.dest.dimmed());
                }
            }
            println!("{}", "-".repeat(80).dimmed());
        }
        Ok(())
    }

    pub fn undo_last_session(&self, auto_yes: bool) -> Result<()> {
        let mut sessions = self.load_sessions()?;
        let last_active = sessions.iter_mut().rev().find(|s| s.status == "COMPLETED" && !s.operations.is_empty());

        let session = match last_active {
            Some(s) => s,
            None => {
                println!("{}", "No active migration sessions available to revert.".yellow());
                return Ok(());
            }
        };

        let session_id = session.session_id.clone();
        println!("\n{}", format!("↩️ Reverting Migration Session: {}", session_id).bright_yellow().bold());
        println!("{}", "Operations to rollback:".bold());
        for op in &session.operations {
            println!("  • Remove symlink: {}  <== Restore from: {}", op.src.bright_cyan(), op.dest.bright_green());
        }

        if !auto_yes {
            use crate::utils::system::confirm_prompt;
            if !confirm_prompt(&format!("Are you sure you want to revert session {}?", session_id), false) {
                println!("{}", "Undo operation cancelled.".yellow());
                return Ok(());
            }
        }

        for op in &session.operations {
            let src_path = Path::new(&op.src);
            let dest_path = Path::new(&op.dest);

            if src_path.is_symlink() || src_path.exists() {
                println!("  Removing symlink: {}", op.src.cyan());
                let _ = fs::remove_file(src_path);
            }

            if dest_path.exists() {
                println!("  Restoring data: {} -> {}", op.dest.green(), op.src.cyan());
                if let Some(parent) = src_path.parent() {
                    let _ = fs::create_dir_all(parent);
                }
                // Run rsync or rename
                let _ = crate::utils::system::run_cmd_stdout("rsync", &["-av", "--remove-source-files", &format!("{}/", op.dest), &format!("{}/", op.src)]);
                let _ = fs::remove_dir_all(dest_path);
            }
        }

        session.status = "REVERTED".to_string();
        self.save_sessions(&sessions)?;
        println!("{}", format!("✔ Session {} successfully reverted!", session_id).bright_green().bold());
        Ok(())
    }
}
