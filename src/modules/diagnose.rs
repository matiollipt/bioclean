use anyhow::Result;
use colored::*;
use std::fs;

use crate::ai::client::OllamaClient;
use crate::ai::fallback::diagnose_fallback;
use crate::ai::prompts::DIAGNOSE_SYSTEM_PROMPT;
use crate::utils::formatting::format_bytes;
use crate::utils::procfs::{read_loadavg, read_meminfo, read_thermal_zones};
use crate::utils::system::run_cmd_stdout;

pub fn run_diagnose(
    ollama: &OllamaClient,
    model: &str,
    output_file: Option<&str>,
    json_output: bool,
) -> Result<String> {
    println!("{}", "\n🩺 [bioclean diagnose] Harvesting System Metrics & Running AI Health Diagnostic".bright_cyan().bold());

    // 1. Gather dmesg errors
    let dmesg_raw = run_cmd_stdout("dmesg", &["-T", "-l", "err,warn"]).unwrap_or_default();
    let dmesg_lines: Vec<&str> = dmesg_raw.lines().collect();
    let dmesg_err_count = dmesg_lines.len();
    let dmesg_sample = dmesg_lines.iter().rev().take(15).cloned().collect::<Vec<_>>().join("\n");

    // 2. Gather journalctl errors
    let journal_raw = run_cmd_stdout("journalctl", &["-p", "3", "-n", "20", "--no-pager"]).unwrap_or_default();
    let journal_lines: Vec<&str> = journal_raw.lines().collect();
    let journal_err_count = journal_lines.len();
    let journal_sample = journal_lines.iter().rev().take(15).cloned().collect::<Vec<_>>().join("\n");

    // 3. Gather df usage
    let df_out = run_cmd_stdout("df", &["-h", "/"]).unwrap_or_default();
    let mut root_used_pct = 0u32;
    if let Some(line) = df_out.lines().nth(1) {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() >= 5 {
            let pct_str = parts[4].trim_end_matches('%');
            root_used_pct = pct_str.parse::<u32>().unwrap_or(0);
        }
    }

    // 4. Memory & Swap
    let mem = read_meminfo();

    // 5. Load averages & thermals
    let (l1, l5, l15) = read_loadavg();
    let zones = read_thermal_zones();
    let max_temp = zones.iter().map(|z| z.temp_celsius).fold(0.0f32, f32::max);

    if json_output {
        let metrics_json = serde_json::json!({
            "root_disk_used_percent": root_used_pct,
            "memory": mem,
            "load_avg": [l1, l5, l15],
            "max_thermal_celsius": max_temp,
            "dmesg_warnings_count": dmesg_err_count,
            "journalctl_errors_count": journal_err_count,
        });
        println!("{}", serde_json::to_string_pretty(&metrics_json)?);
        return Ok(metrics_json.to_string());
    }

    let report_content = if ollama.is_online() {
        println!("  🧠 Calling local Ollama AI model ({}) for synthesis...", model.bright_yellow());
        let prompt = format!(
            r#"### System Diagnostic Snapshot:
- Root Disk (/) Usage: {}%
- RAM Total: {}, Free/Available: {}
- Swap Total: {}, Free: {}
- Load Averages (1m, 5m, 15m): {:.2}, {:.2}, {:.2}
- Peak Thermal Sensor: {:.1}°C
- Recent Kernel Warnings (dmesg):
{}
- Recent Systemd Journal Errors (priority <= 3):
{}
"#,
            root_used_pct,
            format_bytes(mem.total_bytes),
            format_bytes(mem.available_bytes),
            format_bytes(mem.swap_total_bytes),
            format_bytes(mem.swap_free_bytes),
            l1, l5, l15,
            max_temp,
            if dmesg_sample.is_empty() { "(none)" } else { &dmesg_sample },
            if journal_sample.is_empty() { "(none)" } else { &journal_sample }
        );

        match ollama.generate(model, &prompt, Some(DIAGNOSE_SYSTEM_PROMPT)) {
            Ok(report) if !report.is_empty() => report,
            _ => diagnose_fallback(dmesg_err_count, journal_err_count, root_used_pct, &mem, max_temp),
        }
    } else {
        println!("  ℹ Ollama offline. Generating heuristic diagnostic report...");
        diagnose_fallback(dmesg_err_count, journal_err_count, root_used_pct, &mem, max_temp)
    };

    println!("\n{}", report_content);

    if let Some(out_path) = output_file {
        if let Err(e) = fs::write(out_path, &report_content) {
            println!("{} Failed to write report to {}: {}", "✖".red(), out_path, e);
        } else {
            println!("{} Health report saved to: {}", "✔".bright_green(), out_path.bright_cyan());
        }
    }

    Ok(report_content)
}
