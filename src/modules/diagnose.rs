use anyhow::Result;
use chrono::Utc;
use colored::*;
use serde::{Deserialize, Serialize};
use std::fs;

use crate::ai::client::OllamaClient;
use crate::ai::fallback::diagnose_fallback;
use crate::ai::prompts::{self, DIAGNOSE_ROLE};
use crate::config::Config;
use crate::modules::history::{HistoryManager, SessionRecord};
use crate::utils::disks::read_internal_disks;
use crate::utils::procfs::{read_active_sockets, read_cpu_governors, read_journal_size, read_loadavg, read_meminfo, read_thermal_zones, MemInfo};
use crate::utils::report::{display_report, save_report};
use crate::utils::system::run_cmd_stdout;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HostInfo {
    pub hostname: String,
    pub os_release: String,
    pub uptime_secs: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CpuTelemetry {
    pub load_1m: f64,
    pub load_5m: f64,
    pub load_15m: f64,
    pub core_count: usize,
    pub governor: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryTelemetry {
    pub total_bytes: u64,
    pub free_bytes: u64,
    pub available_bytes: u64,
    pub used_percent: u32,
    pub swap_total_bytes: u64,
    pub swap_free_bytes: u64,
    pub swap_used_percent: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiskTelemetry {
    pub mount: String,
    pub used_percent: u32,
    pub total_human: String,
    pub avail_human: String,
    /// Per-mount internal disk breakdown (excludes external/removable media).
    #[serde(default)]
    pub per_mount: Vec<InternalDiskTelemetry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InternalDiskTelemetry {
    pub mount_point: String,
    pub kind: String,
    pub total_bytes: u64,
    pub used_bytes: u64,
    pub available_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThermalTelemetry {
    pub max_temp_celsius: f32,
    pub zone_count: usize,
    pub critical_throttle_detected: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SocketsTelemetry {
    pub total_active: usize,
    pub established: usize,
    pub listening: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogAnomaliesTelemetry {
    pub dmesg_warnings_count: usize,
    pub dmesg_recent_sample: Vec<String>,
    pub journal_errors_count: usize,
    pub journal_recent_sample: Vec<String>,
    #[serde(default)]
    pub journal_size_bytes: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemDiagnosticSnapshot {
    pub run_id: String,
    pub timestamp: String,
    pub host: HostInfo,
    pub cpu: CpuTelemetry,
    pub memory: MemoryTelemetry,
    pub disk: DiskTelemetry,
    pub thermals: ThermalTelemetry,
    pub sockets: SocketsTelemetry,
    pub logs: LogAnomaliesTelemetry,
    pub active_bio_workloads: Vec<String>,
}

fn detect_active_bio_workloads() -> Vec<String> {
    let mut detected = Vec::new();
    let bio_bins = [
        "nextflow", "snakemake", "bowtie2", "bwa", "bwa-mem2",
        "gatk", "samtools", "star", "cellranger", "hisat2",
    ];

    if let Ok(entries) = fs::read_dir("/proc") {
        for entry in entries.flatten() {
            if let Ok(comm) = fs::read_to_string(entry.path().join("comm")) {
                let name = comm.trim().to_lowercase();
                for bin in &bio_bins {
                    if name.contains(bin) && !detected.contains(&name) {
                        detected.push(name.clone());
                    }
                }
            }
        }
    }
    detected
}

fn read_host_info() -> HostInfo {
    let hostname = fs::read_to_string("/proc/sys/kernel/hostname")
        .unwrap_or_else(|_| "aidbio-workstation".to_string())
        .trim()
        .to_string();

    let os_release = fs::read_to_string("/proc/sys/kernel/osrelease")
        .unwrap_or_else(|_| "Linux".to_string())
        .trim()
        .to_string();

    let uptime_secs = fs::read_to_string("/proc/uptime")
        .ok()
        .and_then(|u| u.split_whitespace().next().and_then(|s| s.parse::<f64>().ok()))
        .map(|s| s as u64)
        .unwrap_or(0);

    HostInfo {
        hostname,
        os_release,
        uptime_secs,
    }
}

pub fn collect_diagnostic_snapshot(run_id: &str) -> SystemDiagnosticSnapshot {
    // 1. Gather dmesg errors
    let dmesg_raw = run_cmd_stdout("dmesg", &["-T", "-l", "err,warn"]).unwrap_or_default();
    let dmesg_lines: Vec<String> = dmesg_raw.lines().map(|s| s.to_string()).collect();
    let dmesg_err_count = dmesg_lines.len();
    let dmesg_sample = dmesg_lines.iter().rev().take(15).cloned().collect();

    // 2. Gather journalctl errors
    let journal_raw = run_cmd_stdout("journalctl", &["-p", "3", "-n", "20", "--no-pager"]).unwrap_or_default();
    let journal_lines: Vec<String> = journal_raw.lines().map(|s| s.to_string()).collect();
    let journal_err_count = journal_lines.len();
    let journal_sample = journal_lines.iter().rev().take(15).cloned().collect();

    // 3. Gather df usage
    let df_out = run_cmd_stdout("df", &["-h", "/"]).unwrap_or_default();
    let mut root_used_pct = 0u32;
    let mut total_h = "unknown".to_string();
    let mut avail_h = "unknown".to_string();
    if let Some(line) = df_out.lines().nth(1) {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() >= 5 {
            total_h = parts[1].to_string();
            avail_h = parts[3].to_string();
            let pct_str = parts[4].trim_end_matches('%');
            root_used_pct = pct_str.parse::<u32>().unwrap_or(0);
        }
    }

    // 4. Memory & Swap
    let mem: MemInfo = read_meminfo();
    let mem_used_pct = if mem.total_bytes > 0 {
        ((mem.total_bytes.saturating_sub(mem.available_bytes)) as f64 / mem.total_bytes as f64 * 100.0) as u32
    } else {
        0
    };
    let swap_used_pct = if mem.swap_total_bytes > 0 {
        ((mem.swap_total_bytes.saturating_sub(mem.swap_free_bytes)) as f64 / mem.swap_total_bytes as f64 * 100.0) as u32
    } else {
        0
    };

    // 5. Load averages & thermals
    let (l1, l5, l15) = read_loadavg();
    let zones = read_thermal_zones();
    let max_temp = zones.iter().map(|z| z.temp_celsius).fold(0.0f32, f32::max);

    // 6. CPU Governors & Cores
    let governors = read_cpu_governors();
    let core_count = if governors.is_empty() { 1 } else { governors.len() };
    let primary_gov = governors.first().map(|g| g.current_governor.clone()).unwrap_or_else(|| "unknown".to_string());

    // 7. Sockets
    let sockets = read_active_sockets();
    let total_active_sockets = sockets.len();
    let established_sockets = sockets.iter().filter(|s| s.state == "ESTABLISHED").count();
    let listening_sockets = sockets.iter().filter(|s| s.state == "LISTEN").count();

    // 8. Host & Workloads
    let host = read_host_info();
    let active_bio_workloads = detect_active_bio_workloads();

    SystemDiagnosticSnapshot {
        run_id: run_id.to_string(),
        timestamp: Utc::now().to_rfc3339(),
        host,
        cpu: CpuTelemetry {
            load_1m: l1,
            load_5m: l5,
            load_15m: l15,
            core_count,
            governor: primary_gov,
        },
        memory: MemoryTelemetry {
            total_bytes: mem.total_bytes,
            free_bytes: mem.free_bytes,
            available_bytes: mem.available_bytes,
            used_percent: mem_used_pct,
            swap_total_bytes: mem.swap_total_bytes,
            swap_free_bytes: mem.swap_free_bytes,
            swap_used_percent: swap_used_pct,
        },
        disk: DiskTelemetry {
            mount: "/".to_string(),
            used_percent: root_used_pct,
            total_human: total_h,
            avail_human: avail_h,
            per_mount: read_internal_disks()
                .into_iter()
                .map(|d| InternalDiskTelemetry {
                    mount_point: d.mount_point,
                    kind: d.kind,
                    total_bytes: d.total_bytes,
                    used_bytes: d.used_bytes,
                    available_bytes: d.available_bytes,
                })
                .collect(),
        },
        thermals: ThermalTelemetry {
            max_temp_celsius: max_temp,
            zone_count: zones.len(),
            critical_throttle_detected: max_temp > 85.0,
        },
        sockets: SocketsTelemetry {
            total_active: total_active_sockets,
            established: established_sockets,
            listening: listening_sockets,
        },
        logs: LogAnomaliesTelemetry {
            dmesg_warnings_count: dmesg_err_count,
            dmesg_recent_sample: dmesg_sample,
            journal_errors_count: journal_err_count,
            journal_recent_sample: journal_sample,
            journal_size_bytes: read_journal_size(),
        },
        active_bio_workloads,
    }
}

pub fn run_diagnose(
    ollama: &OllamaClient,
    model: &str,
    config: &Config,
    output_file: Option<&str>,
    json_output: bool,
) -> Result<String> {
    println!("{}", "\n🩺 [bioclean diagnose] Harvesting System Telemetry & Running AI Health Diagnostic".bright_cyan().bold());

    let run_id = HistoryManager::generate_run_id("diag");
    let snapshot = collect_diagnostic_snapshot(&run_id);
    let snapshot_json = serde_json::to_string_pretty(&snapshot)?;

    if json_output {
        println!("{}", snapshot_json);
        return Ok(snapshot_json);
    }

    let report_body = if ollama.is_online() {
        println!("  🧠 Calling local Ollama AI model ({}) with structured telemetry...", model.bright_yellow());
        let prompt = format!(
            r#"### SYSTEM OPERATIONAL CONTEXT:
- Host: {} ({}, Uptime: {}s)
- Primary CPU Governor: {} ({} Cores, Load: {:.2}, {:.2}, {:.2})
- Active Bioinformatics Pipeline Processes: {}

### STRUCTURED SYSTEM DIAGNOSTIC SNAPSHOT (JSON):
```json
{}
```

### INSTRUCTIONS:
Analyze the verified JSON diagnostic snapshot above according to your system prompt instructions. Formulate an objective health score, analyze anomalies, and prescribe safe, non-destructive actions."#,
            snapshot.host.hostname,
            snapshot.host.os_release,
            snapshot.host.uptime_secs,
            snapshot.cpu.governor,
            snapshot.cpu.core_count,
            snapshot.cpu.load_1m,
            snapshot.cpu.load_5m,
            snapshot.cpu.load_15m,
            if snapshot.active_bio_workloads.is_empty() { "(none detected)".to_string() } else { snapshot.active_bio_workloads.join(", ") },
            snapshot_json
        );

        let system_prompt = prompts::compose(DIAGNOSE_ROLE);
        match ollama.generate(model, &prompt, Some(&system_prompt), Some(&config.ollama_options())) {
            Ok(report) if !report.is_empty() => report,
            _ => {
                println!("  ⚠️ AI synthesis failed or returned empty response. Falling back to heuristic diagnostic engine.");
                let mem = MemInfo {
                    total_bytes: snapshot.memory.total_bytes,
                    free_bytes: snapshot.memory.free_bytes,
                    available_bytes: snapshot.memory.available_bytes,
                    buffers_bytes: 0,
                    cached_bytes: 0,
                    swap_total_bytes: snapshot.memory.swap_total_bytes,
                    swap_free_bytes: snapshot.memory.swap_free_bytes,
                };
                diagnose_fallback(
                    snapshot.logs.dmesg_warnings_count,
                    snapshot.logs.journal_errors_count,
                    snapshot.disk.used_percent,
                    &mem,
                    snapshot.thermals.max_temp_celsius,
                )
            }
        }
    } else {
        println!("  ℹ Ollama offline at {}. Generating heuristic diagnostic report...", config.ollama_url.dimmed());
        let mem = MemInfo {
            total_bytes: snapshot.memory.total_bytes,
            free_bytes: snapshot.memory.free_bytes,
            available_bytes: snapshot.memory.available_bytes,
            buffers_bytes: 0,
            cached_bytes: 0,
            swap_total_bytes: snapshot.memory.swap_total_bytes,
            swap_free_bytes: snapshot.memory.swap_free_bytes,
        };
        diagnose_fallback(
            snapshot.logs.dmesg_warnings_count,
            snapshot.logs.journal_errors_count,
            snapshot.disk.used_percent,
            &mem,
            snapshot.thermals.max_temp_celsius,
        )
    };

    // 1. Setup paired session directory: ~/.local/share/bioclean/history/<run_id>/
    let session_dir = HistoryManager::create_paired_session_dir(&config.rollback_log_dir, &run_id)?;
    let session_snapshot_path = session_dir.join("diagnostic_snapshot.json");
    fs::write(&session_snapshot_path, &snapshot_json)?;

    // 2. Format complete report with paired metadata header
    let full_report = format!(
        "{}\n\n---\n> **Session Correlation ID:** `{}`  \n> **Timestamp:** `{}`  \n> **Telemetry Snapshot:** `{}`  \n> **Rollback Registry:** `{}`\n",
        report_body.trim(),
        run_id,
        snapshot.timestamp,
        session_snapshot_path.display(),
        config.history_file
    );

    let session_report_path = session_dir.join("report.md");
    fs::write(&session_report_path, &full_report)?;

    // 3. Save report in user-configured report output directory
    let saved_report_path = save_report(&full_report, &config.report_output_dir, &run_id)?;

    // 4. If explicit output_file was requested, save there as well
    if let Some(custom_out) = output_file {
        if let Err(e) = fs::write(custom_out, &full_report) {
            println!("  ⚠️ Failed to save report to custom path '{}': {}", custom_out, e);
        } else {
            println!("  ✔ Custom report copy saved to: {}", custom_out.bright_cyan());
        }
    }

    // 5. Record paired session entry in history registry
    let history_mgr = HistoryManager::new(&config.history_file);
    let session_record = SessionRecord {
        session_id: run_id.clone(),
        timestamp: snapshot.timestamp.clone(),
        status: "COMPLETED".to_string(),
        run_type: "DIAGNOSTIC".to_string(),
        report_path: Some(saved_report_path.to_string_lossy().to_string()),
        snapshot_path: Some(session_snapshot_path.to_string_lossy().to_string()),
        rollback_manifest_path: None,
        operations: Vec::new(),
    };
    let _ = history_mgr.record_session(session_record);
    let _ = HistoryManager::log_audit_trail(&session_dir, &format!("Completed diagnostic run {} (Health report generated)", run_id));

    // 6. High-fidelity terminal display
    println!("\n{}", "=".repeat(80).dimmed());
    let _ = display_report(&full_report, &config.report_formatter);
    println!("{}", "=".repeat(80).dimmed());

    println!("{} Health report saved to : {}", "✔".bright_green(), saved_report_path.display().to_string().bright_cyan());
    println!("{} Telemetry snapshot     : {}", "✔".bright_green(), session_snapshot_path.display().to_string().dimmed());
    println!("{} Session ID             : {}\n", "✔".bright_green(), run_id.bright_yellow());

    Ok(full_report)
}

