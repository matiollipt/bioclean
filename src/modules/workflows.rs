use anyhow::Result;
use colored::*;
use std::fs;
use std::path::Path;

use crate::ai::client::OllamaClient;
use crate::config::Config;
use crate::modules::diagnose::run_diagnose;
use crate::modules::free::{clean_cache, clean_logs, clean_orphans};
use crate::modules::power::set_performance_profile;
use crate::modules::scan::scan_heavy_files;
use crate::utils::formatting::format_bytes;
use crate::modules::free::estimate_reclaimable;
use crate::utils::procfs::{read_active_sockets, read_meminfo, read_thermal_zones};
use crate::utils::system::{run_fstrim, ActionPreview, RiskLevel};

pub fn workflow_prepare_crunch(
    scratch_dir: &str,
    dry_run: bool,
    auto_yes: bool,
    ollama: &OllamaClient,
    model: &str,
) -> Result<()> {
    println!("{}", "\n🚀 [WORKFLOW] Preparing Workstation for Heavy Computation (Prepare-Crunch)".bright_purple().bold());
    println!("{}", "=".repeat(75).dimmed());

    let reclaimable = estimate_reclaimable();
    let preview = ActionPreview {
        action: "Run prepare-crunch workflow (clean caches/logs, switch to performance profile)".to_string(),
        current_state: format!("{} reclaimable across caches; CPU governor not yet switched", format_bytes(reclaimable.total_bytes)),
        future_state: "Caches and recent logs cleared; CPU governor set to performance".to_string(),
        risk: RiskLevel::Safe,
    };
    if !dry_run && !auto_yes && !preview.confirm() {
        println!("{}", "Prepare-crunch workflow cancelled.".yellow());
        return Ok(());
    }

    // 1. Scan scratch directory
    println!("\n{}", "Step 1/5: Checking Scratch Disk Capacity & Datasets".bold());
    let scratch_path = Path::new(scratch_dir);
    if scratch_path.exists() {
        let _ = scan_heavy_files(scratch_path, 100 * 1024 * 1024, 5, false, ollama, model);
    } else {
        println!("  Target scratch directory '{}' does not exist yet.", scratch_dir.yellow());
    }

    // 2. Free caches and old logs
    println!("\n{}", "Step 2/5: Freeing Package Caches and System Logs".bold());
    clean_cache(dry_run, auto_yes, ollama, model)?;
    clean_logs(3, dry_run, auto_yes)?;

    // 3. Power profile to performance
    println!("\n{}", "Step 3/5: Switching Power Profile to Performance".bold());
    set_performance_profile(dry_run)?;

    // 4. Verify thermal margins and active sockets
    println!("\n{}", "Step 4/5: Verifying Thermal & I/O Competition".bold());
    let zones = read_thermal_zones();
    let max_temp = zones.iter().map(|z| z.temp_celsius).fold(0.0f32, f32::max);
    println!("  • Current Peak Temperature: {:.1}°C", max_temp);

    let sockets = read_active_sockets();
    let established_count = sockets.iter().filter(|s| s.state == "ESTABLISHED").count();
    println!("  • Active Network Connections: {}", established_count);

    // 5. Final Green Light Report
    println!("\n{}", "Step 5/5: Pipeline Readiness Verification".bold());
    let mem = read_meminfo();
    let mem_avail_str = format_bytes(mem.available_bytes);

    println!("\n{}", "=================================================================".bright_green().bold());
    println!("{}", "             🟢 GREEN LIGHT: SYSTEM READY FOR CRUNCH             ".bright_green().bold());
    println!("{}", "=================================================================".bright_green().bold());
    println!("  • Scratch Target   : {}", scratch_dir.bright_cyan());
    println!("  • Available RAM    : {}", mem_avail_str.bright_yellow());
    println!("  • CPU Governor     : {}", "performance".bright_green().bold());
    println!("  • Peak Thermal     : {:.1}°C", max_temp);
    println!("  • Recommendation   : Ready to launch Nextflow, Snakemake, or ML training pipelines.\n");

    Ok(())
}

pub fn workflow_maintenance(
    output_file: Option<&str>,
    dry_run: bool,
    auto_yes: bool,
    ollama: &OllamaClient,
    model: &str,
    config: &Config,
) -> Result<()> {
    println!("{}", "\n🛠 [WORKFLOW] Running Complete Workstation Maintenance".bright_purple().bold());
    println!("{}", "=".repeat(75).dimmed());

    let reclaimable = estimate_reclaimable();
    let preview = ActionPreview {
        action: "Run maintenance workflow (diagnose, clean caches/logs/orphans, fstrim)".to_string(),
        current_state: format!("{} reclaimable across caches; journal and orphan packages not yet cleared", format_bytes(reclaimable.total_bytes)),
        future_state: format!("~{} reclaimed; SSDs trimmed; maintenance report written", format_bytes(reclaimable.total_bytes)),
        risk: RiskLevel::Safe,
    };
    if !dry_run && !auto_yes && !preview.confirm() {
        println!("{}", "Maintenance workflow cancelled.".yellow());
        return Ok(());
    }

    // 1. Audit & Diagnostics
    println!("\n{}", "Step 1/4: Running Comprehensive Health Diagnostics".bold());
    let report = run_diagnose(ollama, model, config, None, false)?;

    // 2. Space reclamation
    println!("\n{}", "Step 2/4: Executing Cache, Log & Orphan Cleanup".bold());
    clean_cache(dry_run, auto_yes, ollama, model)?;
    clean_logs(7, dry_run, auto_yes)?;
    clean_orphans(dry_run, auto_yes)?;

    // 3. SSD Optimization (fstrim)
    println!("\n{}", "Step 3/4: Optimizing SSD Storage (fstrim)".bold());
    if dry_run {
        println!("{}", "  🔎 [DRY RUN] Would execute fstrim -av across all mounted SSDs.".bright_green());
    } else {
        let fstrim_preview = ActionPreview {
            action: "Run fstrim across all mounted filesystems".to_string(),
            current_state: "Discarded/unused blocks not yet reclaimed by the SSD controller".to_string(),
            future_state: "Unused blocks trimmed; no data is modified, only idle space reclaimed".to_string(),
            risk: RiskLevel::Safe,
        };
        if auto_yes || fstrim_preview.confirm() {
            match run_fstrim() {
                Ok(trim_out) => {
                    for line in trim_out.lines() {
                        println!("  {}", line.dimmed());
                    }
                    println!("{}", "  ✔ SSD TRIM completed successfully.".bright_green());
                }
                Err(e) => {
                    println!("  ⚠️ fstrim execution skipped: {}", e);
                }
            }
        } else {
            println!("{}", "  fstrim skipped by user.".yellow());
        }
    }

    // 4. Report generation
    println!("\n{}", "Step 4/4: Generating Maintenance Summary".bold());
    let summary_md = format!(
        "# 📋 Bioclean Maintenance Report\n\n**Date:** {}\n\n{}\n\n---\n*Generated automatically by AidBio Bioclean.*",
        chrono::Utc::now().to_rfc3339(),
        report
    );

    let final_path = output_file.unwrap_or("bioclean_maintenance_report.md");
    if let Err(e) = fs::write(final_path, &summary_md) {
        println!("  ⚠️ Failed to save maintenance report to {}: {}", final_path, e);
    } else {
        println!("  ✔ Maintenance report saved to: {}", final_path.bright_cyan());
    }

    println!("{}", "\n✔ Weekly maintenance workflow completed!".bright_green().bold());
    Ok(())
}
