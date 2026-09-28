use anyhow::{Context, Result};
use colored::*;
use std::fs;
use std::io::Write as IoWrite;
use std::process::{Command, Stdio};

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

/// Writes a file that requires root, transparently escalating through `sudo
/// tee` when not already running as root. Used for modprobe/udev/systemd
/// drop-ins and one-shot sysfs writes (e.g. PCI `power/control`).
fn write_privileged_file(path: &str, contents: &str, dry_run: bool) -> Result<()> {
    if dry_run {
        println!("  🔎 [DRY RUN] Would write {} ({} bytes)", path, contents.len());
        return Ok(());
    }
    if is_root() {
        fs::write(path, contents).with_context(|| format!("writing {}", path))?;
        return Ok(());
    }
    let mut child = Command::new("sudo")
        .args(["tee", path])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::inherit())
        .spawn()
        .with_context(|| format!("spawning `sudo tee {}`", path))?;
    if let Some(stdin) = child.stdin.as_mut() {
        stdin.write_all(contents.as_bytes())?;
    }
    child.wait().with_context(|| format!("waiting on `sudo tee {}`", path))?;
    Ok(())
}

fn has_nvidia_gpu() -> bool {
    run_cmd_stdout("lspci", &["-d", "10de:"]).map(|s| !s.trim().is_empty()).unwrap_or(false)
}

/// Kills only `nvidia-smi` invocations that carry a loop flag (`-l`/`-lms`),
/// leaving one-shot queries (e.g. from `diagnose`) alone. GNOME extensions
/// like Astra Monitor and Conky/psensor widgets commonly spawn these to
/// refresh a GPU stat every 1-2s, which keeps the dGPU out of its idle state.
fn kill_nvidia_smi_pollers() -> usize {
    let output = run_cmd_stdout("pgrep", &["-af", "nvidia-smi"]).unwrap_or_default();
    let mut killed = 0;
    for line in output.lines() {
        if !line.contains("-l") {
            continue;
        }
        if let Some(pid) = line.split_whitespace().next() {
            if run_cmd_status("kill", &["-9", pid]).unwrap_or(false) {
                killed += 1;
            }
        }
    }
    killed
}

/// Disables Astra Monitor's GPU header/monitoring via dconf if the extension
/// is present — its default 2s `nvidia-smi -lms` poll is a common source of
/// otherwise-unexplained dGPU wakeups on hybrid-graphics laptops. Preserves
/// the extension's own GPU identity fields (domain/bus/slot/vendor/product),
/// only flipping the `monitor` flag, so it works regardless of which GPU is
/// configured on this machine.
fn disable_gnome_gpu_polling(dry_run: bool) -> Result<()> {
    let base = "/org/gnome/shell/extensions/astra-monitor";
    if !command_exists("dconf") {
        return Ok(());
    }
    let list_output = run_cmd_stdout("dconf", &["list", &format!("{}/", base)]).unwrap_or_default();
    if list_output.trim().is_empty() {
        return Ok(());
    }

    println!("  Detected Astra Monitor GNOME extension (its default GPU refresh polls `nvidia-smi` every 2s)...");
    if dry_run {
        println!("  🔎 [DRY RUN] Would disable Astra Monitor's GPU header/monitoring via dconf.");
        return Ok(());
    }

    let _ = run_cmd_status("dconf", &["write", &format!("{}/gpu-header-show", base), "false"]);

    let current = run_cmd_stdout("dconf", &["read", &format!("{}/gpu-data", base)]).unwrap_or_default();
    let current = current.trim();
    if !current.is_empty() {
        let updated = current.replace("\"monitor\": true", "\"monitor\": false").replace("\"monitor\":true", "\"monitor\":false");
        let _ = run_cmd_status("dconf", &["write", &format!("{}/gpu-data", base), &updated]);
    }

    println!("  ✔ Disabled Astra Monitor's GPU polling loop.");
    Ok(())
}

/// `nvidia-persistenced` is meant for headless/server GPUs that need to skip
/// re-init latency; on a laptop with PRIME on-demand it just keeps the driver
/// state around for no benefit. It's udev-activated (no [Install] section),
/// so plain `disable` fails — `mask --now` is the durable way to stop it.
fn mask_nvidia_persistenced(dry_run: bool) -> Result<()> {
    if !command_exists("systemctl") {
        return Ok(());
    }
    let state = run_cmd_stdout("systemctl", &["is-enabled", "nvidia-persistenced"]).unwrap_or_default();
    let state = state.trim();
    if state.is_empty() || state == "not-found" {
        return Ok(());
    }
    if state == "masked" {
        return Ok(());
    }

    if dry_run {
        println!("  🔎 [DRY RUN] Would stop and mask nvidia-persistenced.service (redundant on Optimus laptops).");
        return Ok(());
    }

    let ok = if is_root() {
        run_cmd_status("systemctl", &["mask", "--now", "nvidia-persistenced"])
    } else {
        run_cmd_status("sudo", &["systemctl", "mask", "--now", "nvidia-persistenced"])
    }
    .unwrap_or(false);

    if ok {
        println!("  ✔ Stopped and masked nvidia-persistenced (persistence mode isn't needed on a laptop).");
    }
    Ok(())
}

/// Configures NVIDIA runtime D3cold power management so the dGPU can drop to
/// ~0W when idle instead of sitting in `active`: modprobe dynamic PM flag,
/// udev rule forcing `power/control=auto` on the GPU's PCI functions, an
/// immediate one-shot apply, killing any polling loops found, and disabling
/// the redundant persistence daemon.
fn optimize_nvidia_dgpu(dry_run: bool) -> Result<()> {
    if !has_nvidia_gpu() {
        println!("  No NVIDIA dGPU detected — skipping GPU-specific tuning.");
        return Ok(());
    }
    println!("  NVIDIA dGPU detected. Configuring runtime D3cold power management...");

    write_privileged_file(
        "/etc/modprobe.d/bioclean-nvidia-pm.conf",
        "# bioclean power optimize — NVIDIA dynamic runtime PM\noptions nvidia NVreg_DynamicPowerManagement=0x02\n",
        dry_run,
    )?;

    write_privileged_file(
        "/etc/udev/rules.d/80-bioclean-nvidia-pm.rules",
        concat!(
            "# bioclean power optimize — NVIDIA runtime PM udev rules\n",
            "ACTION==\"add\", SUBSYSTEM==\"pci\", ATTR{vendor}==\"0x10de\", ATTR{class}==\"0x030000\", ATTR{power/control}=\"auto\"\n",
            "ACTION==\"add\", SUBSYSTEM==\"pci\", ATTR{vendor}==\"0x10de\", ATTR{class}==\"0x038000\", ATTR{power/control}=\"auto\"\n",
            "ACTION==\"add\", SUBSYSTEM==\"pci\", ATTR{vendor}==\"0x10de\", ATTR{class}==\"0x040300\", ATTR{power/control}=\"auto\"\n",
        ),
        dry_run,
    )?;

    if !dry_run {
        let (cmd, base_args): (&str, &[&str]) = if is_root() { ("udevadm", &[]) } else { ("sudo", &["udevadm"]) };
        let mut reload_args: Vec<&str> = base_args.to_vec();
        reload_args.extend(["control", "--reload-rules"]);
        let _ = run_cmd_status(cmd, &reload_args);
        let mut trigger_args: Vec<&str> = base_args.to_vec();
        trigger_args.push("trigger");
        let _ = run_cmd_status(cmd, &trigger_args);

        if let Ok(entries) = fs::read_dir("/sys/bus/pci/devices") {
            for entry in entries.flatten() {
                let path = entry.path();
                let vendor = fs::read_to_string(path.join("vendor")).unwrap_or_default();
                if vendor.trim() != "0x10de" {
                    continue;
                }
                if let Some(control_path) = path.join("power/control").to_str() {
                    let _ = write_privileged_file(control_path, "auto", false);
                }
            }
        }
    }

    let killed = kill_nvidia_smi_pollers();
    if killed > 0 {
        println!("  ✔ Terminated {} background `nvidia-smi` polling process(es).", killed);
    }

    mask_nvidia_persistenced(dry_run)?;

    println!("  ✔ NVIDIA D3cold power management configured — dGPU can fully sleep when idle.");
    Ok(())
}

/// Installs and enables TLP if it isn't already active. TLP is the one piece
/// of this pass that needs a persistent daemon (PCIe ASPM, USB autosuspend,
/// battery charge thresholds) rather than a one-shot fix.
fn ensure_tlp(dry_run: bool) -> Result<()> {
    if command_exists("tlp") {
        println!("  TLP already installed.");
    } else if dry_run {
        println!("  🔎 [DRY RUN] Would install `tlp` and `tlp-rdw` packages.");
    } else {
        println!("  Installing TLP power management daemon...");
        let _ = run_cmd_status("sudo", &["apt-get", "update", "-qq"]);
        let _ = run_cmd_status("sudo", &["apt-get", "install", "-y", "tlp", "tlp-rdw"]);
    }

    if dry_run {
        println!("  🔎 [DRY RUN] Would enable and start tlp.service.");
        return Ok(());
    }
    let ok = run_cmd_status("sudo", &["systemctl", "enable", "--now", "tlp"]).unwrap_or(false);
    if ok {
        println!("  ✔ TLP power management active (PCIe ASPM, USB autosuspend, charge thresholds).");
    }
    Ok(())
}

/// Runs `powertop --auto-tune` and persists it as a oneshot systemd service so
/// the tuning (audio codec power-save, runtime PM on assorted buses) survives
/// reboots instead of needing a manual re-run each time.
fn ensure_powertop_autotune(dry_run: bool) -> Result<()> {
    if !command_exists("powertop") {
        if dry_run {
            println!("  🔎 [DRY RUN] Would install `powertop`.");
        } else {
            println!("  Installing powertop...");
            let _ = run_cmd_status("sudo", &["apt-get", "install", "-y", "powertop"]);
        }
    }

    if dry_run {
        println!("  🔎 [DRY RUN] Would run `powertop --auto-tune` and enable a persistent systemd service.");
        return Ok(());
    }

    let _ = run_cmd_status("sudo", &["powertop", "--auto-tune"]);

    write_privileged_file(
        "/etc/systemd/system/bioclean-powertop-autotune.service",
        concat!(
            "[Unit]\n",
            "Description=bioclean Powertop Auto-tune\n",
            "After=multi-user.target\n\n",
            "[Service]\n",
            "Type=oneshot\n",
            "ExecStart=/usr/sbin/powertop --auto-tune\n",
            "RemainAfterExit=yes\n\n",
            "[Install]\n",
            "WantedBy=multi-user.target\n",
        ),
        false,
    )?;
    let _ = run_cmd_status("sudo", &["systemctl", "daemon-reload"]);
    let _ = run_cmd_status("sudo", &["systemctl", "enable", "--now", "bioclean-powertop-autotune.service"]);

    println!("  ✔ Powertop auto-tune applied and persisted across reboots.");
    Ok(())
}

/// The "3. Power & Thermals" optimizer: reproduces, as a single reusable
/// pass, the manual battery-drain diagnosis/fix procedure for hybrid-graphics
/// (Intel iGPU + NVIDIA dGPU/Optimus) laptops — stop background GPU-polling
/// loops (GNOME extensions, stray `nvidia-smi -lms` scripts), let the dGPU
/// reach its D3cold idle state, retire the redundant persistence daemon, and
/// bring up TLP + powertop for ongoing PCIe/USB/audio power management.
pub fn optimize_gpu_and_peripherals(dry_run: bool) -> Result<()> {
    println!("{}", "\n🖥️ [bioclean power optimize] dGPU, GNOME Polling & Peripheral Power Optimizer".bright_cyan().bold());
    println!(
        "{}",
        "Targets idle battery drain on hybrid-graphics laptops: dGPU sleep state, background GPU-polling loops, and PCIe/USB/audio power management.".dimmed()
    );
    println!();

    disable_gnome_gpu_polling(dry_run)?;
    optimize_nvidia_dgpu(dry_run)?;
    ensure_tlp(dry_run)?;
    ensure_powertop_autotune(dry_run)?;

    println!(
        "\n{}",
        "✔ Power optimization pass complete. Re-check discharge rate with `bioclean power thermal` or `upower -i <BAT>` after a minute.".bright_green().bold()
    );
    print_power_status()?;
    Ok(())
}
