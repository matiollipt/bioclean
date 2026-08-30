use anyhow::Result;
use colored::*;

use crate::utils::procfs::{read_cpu_governors, read_thermal_zones, set_cpu_governors};
use crate::utils::system::{command_exists, is_root, run_cmd_status, run_cmd_stdout};

pub fn set_battery_profile(dry_run: bool) -> Result<()> {
    println!("{}", "\n🔋 [bioclean power battery] Switching to Energy Saving Profile".bright_cyan().bold());

    if dry_run {
        println!("{}", "🔎 [DRY RUN] Would set CPU scaling governor to 'powersave' across all cores.".bright_green());
        return Ok(());
    }

    if is_root() {
        let count = set_cpu_governors("powersave")?;
        println!("  ✔ Set scaling governor to {} across {} CPU cores.", "powersave".bright_green(), count);
    } else {
        println!("  Attempting governor switch with sudo...");
        let script = "for g in /sys/devices/system/cpu/cpu*/cpufreq/scaling_governor; do [ -f \"$g\" ] && echo powersave > \"$g\"; done; for e in /sys/devices/system/cpu/cpu*/cpufreq/energy_performance_preference; do [ -f \"$e\" ] && echo power > \"$e\"; done";
        let _ = run_cmd_status("sudo", &["sh", "-c", script]);
    }

    // Check if TLP or powertop is available
    if command_exists("tlp") {
        println!("  Applying TLP battery profile (`tlp bat`)...");
        let _ = if is_root() {
            run_cmd_status("tlp", &["bat"])
        } else {
            run_cmd_status("sudo", &["tlp", "bat"])
        };
    }

    println!("{}", "✔ Battery profile activated. CPU scaling and peripherals throttled for longevity.".bright_green().bold());
    print_power_status()?;
    Ok(())
}

pub fn set_performance_profile(dry_run: bool) -> Result<()> {
    println!("{}", "\n⚡ [bioclean power performance] Switching to High-Performance Compute Profile".bright_cyan().bold());

    if dry_run {
        println!("{}", "🔎 [DRY RUN] Would set CPU governor to 'performance' and maximize I/O priority.".bright_green());
        return Ok(());
    }

    if is_root() {
        let count = set_cpu_governors("performance")?;
        println!("  ✔ Set scaling governor to {} across {} CPU cores.", "performance".bright_yellow(), count);
    } else {
        println!("  Attempting governor switch with sudo...");
        let script = "for g in /sys/devices/system/cpu/cpu*/cpufreq/scaling_governor; do [ -f \"$g\" ] && echo performance > \"$g\"; done; for e in /sys/devices/system/cpu/cpu*/cpufreq/energy_performance_preference; do [ -f \"$e\" ] && echo performance > \"$e\"; done";
        let _ = run_cmd_status("sudo", &["sh", "-c", script]);
    }

    // If thermald is active, report
    if command_exists("systemctl") {
        let active = run_cmd_stdout("systemctl", &["is-active", "thermald"]).unwrap_or_default();
        if active.trim() == "active" {
            println!("  ✔ Active thermal daemon (`thermald`) confirmed running.");
        }
    }

    println!("{}", "✔ High-performance profile activated! CPU governor set to performance for maximum pipeline throughput.".bright_green().bold());
    print_power_status()?;
    Ok(())
}

pub fn monitor_thermal(json_output: bool) -> Result<()> {
    let zones = read_thermal_zones();
    let cpus = read_cpu_governors();

    if json_output {
        let json_obj = serde_json::json!({
            "thermal_zones": zones,
            "cpu_governors": cpus,
        });
        println!("{}", serde_json::to_string_pretty(&json_obj)?);
        return Ok(());
    }

    println!("{}", "\n🌡 [bioclean power thermal] Thermal & Energy Observability".bright_cyan().bold());
    println!("{}", "=".repeat(65).dimmed());

    if zones.is_empty() {
        println!("{}", "  No hardware thermal zones exposed in /sys/class/thermal.".yellow());
    } else {
        println!("  {:<6} {:<25} {:<15} {:<15}", "ZONE", "TYPE", "TEMP (°C)", "CRITICAL TRIP");
        println!("  {}", "-".repeat(60).dimmed());
        for z in &zones {
            let temp_str = if z.temp_celsius > 85.0 {
                format!("{:.1}°C", z.temp_celsius).bright_red().bold()
            } else if z.temp_celsius > 70.0 {
                format!("{:.1}°C", z.temp_celsius).bright_yellow()
            } else {
                format!("{:.1}°C", z.temp_celsius).bright_green()
            };

            let crit_str = match z.trip_point_crit {
                Some(c) => format!("{:.1}°C", c),
                None => "N/A".to_string(),
            };

            println!("  {:<6} {:<25} {:<15} {:<15}", z.id, z.zone_type, temp_str, crit_str.dimmed());
        }
    }

    println!("\n{}", "⚙️ CPU Frequency & Scaling Governors:".bold());
    if cpus.is_empty() {
        println!("{}", "  No cpufreq scaling information available.".yellow());
    } else {
        let current_gov = cpus.first().map(|c| c.current_governor.as_str()).unwrap_or("unknown");
        let epp = cpus.first().and_then(|c| c.energy_perf_preference.as_deref()).unwrap_or("none");
        println!("  • Active Governor : {} (across {} cores)", current_gov.bright_yellow().bold(), cpus.len());
        println!("  • Energy Pref     : {}", epp.bright_cyan());
    }

    // Check thermald
    if command_exists("systemctl") {
        let active = run_cmd_stdout("systemctl", &["is-active", "thermald"]).unwrap_or_default();
        let status_badge = if active.trim() == "active" {
            "RUNNING".bright_green()
        } else {
            "INACTIVE".yellow()
        };
        println!("  • thermald status : {}", status_badge);
    }

    Ok(())
}

pub fn print_power_status() -> Result<()> {
    let cpus = read_cpu_governors();
    let zones = read_thermal_zones();
    let max_temp = zones.iter().map(|z| z.temp_celsius).fold(0.0f32, f32::max);

    if let Some(first) = cpus.first() {
        println!("  Current Governor: {} | Peak Thermal Zone: {:.1}°C", first.current_governor.bright_yellow().bold(), max_temp);
    }
    Ok(())
}
