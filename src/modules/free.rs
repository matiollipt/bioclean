use anyhow::Result;
use colored::*;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use crate::ai::client::OllamaClient;
use crate::ai::safety_interlock::verify_safety_interlock;
use crate::utils::formatting::format_bytes;
use crate::utils::system::{command_exists, is_root, run_cmd_status, run_cmd_stdout, ActionPreview, RiskLevel};

#[derive(Debug, Default, Clone)]
pub struct ReclaimableItem {
    pub name: String,
    pub description: String,
    pub estimated_bytes: u64,
}

#[derive(Debug, Default, Clone)]
pub struct ReclaimableSummary {
    pub items: Vec<ReclaimableItem>,
    pub total_bytes: u64,
}

fn dir_size(path: &Path) -> u64 {
    if !path.exists() {
        return 0;
    }
    walkdir::WalkDir::new(path)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter_map(|e| e.metadata().ok())
        .filter(|m| m.is_file())
        .map(|m| m.len())
        .sum()
}

pub fn estimate_reclaimable() -> ReclaimableSummary {
    let mut items = Vec::new();
    let home = std::env::var("HOME").unwrap_or_else(|_| "/home/clever".to_string());

    // 1. APT Cache
    let apt_path = Path::new("/var/cache/apt/archives");
    let apt_bytes = dir_size(apt_path);
    if apt_bytes > 0 {
        items.push(ReclaimableItem {
            name: "APT Package Cache".to_string(),
            description: "/var/cache/apt/archives .deb package archives".to_string(),
            estimated_bytes: apt_bytes,
        });
    }

    // 2. Pip Cache
    let pip_path = PathBuf::from(&home).join(".cache/pip");
    let pip_bytes = dir_size(&pip_path);
    if pip_bytes > 0 {
        items.push(ReclaimableItem {
            name: "Python Pip Cache".to_string(),
            description: "~/.cache/pip downloaded wheels and tarballs".to_string(),
            estimated_bytes: pip_bytes,
        });
    }

    // 3. UV Cache
    let uv_path = PathBuf::from(&home).join(".cache/uv");
    let uv_bytes = dir_size(&uv_path);
    if uv_bytes > 0 {
        items.push(ReclaimableItem {
            name: "UV Package Cache".to_string(),
            description: "~/.cache/uv package and wheel cache".to_string(),
            estimated_bytes: uv_bytes,
        });
    }

    // 4. Conda Package Archives
    let conda_path = PathBuf::from(&home).join(".conda/pkgs");
    let miniconda_path = PathBuf::from(&home).join("miniconda3/pkgs");
    let conda_bytes = dir_size(&conda_path) + dir_size(&miniconda_path);
    if conda_bytes > 0 {
        items.push(ReclaimableItem {
            name: "Conda / Mamba Package Cache".to_string(),
            description: "Conda package tarballs (.conda, .tar.bz2)".to_string(),
            estimated_bytes: conda_bytes,
        });
    }

    // 5. User Build Caches (Cypress, node-gyp, thumbnails)
    let misc_caches = [
        PathBuf::from(&home).join(".cache/thumbnails"),
        PathBuf::from(&home).join(".cache/Cypress"),
        PathBuf::from(&home).join(".cache/node-gyp"),
        PathBuf::from(&home).join(".cache/ms-playwright-go"),
    ];
    let misc_bytes: u64 = misc_caches.iter().map(|p| dir_size(p)).sum();
    if misc_bytes > 0 {
        items.push(ReclaimableItem {
            name: "Browser & Build Caches".to_string(),
            description: "Thumbnails, Cypress, node-gyp build caches".to_string(),
            estimated_bytes: misc_bytes,
        });
    }

    // 6. Journalctl Logs
    if let Some(bytes) = crate::utils::procfs::read_journal_size() {
        if bytes > 50 * 1024 * 1024 {
            // Only suggest if > 50MB
            items.push(ReclaimableItem {
                name: "Systemd Journal Logs".to_string(),
                description: "Archived system journal logs".to_string(),
                estimated_bytes: bytes,
            });
        }
    }

    // 7. Docker reclaimable
    if command_exists("docker") {
        if let Ok(out) = run_cmd_stdout("docker", &["system", "df"]) {
            // Rough estimation
            let mut total_docker_reclaimable = 0u64;
            for line in out.lines().skip(1) {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if let Some(pos) = parts.iter().position(|&p| p.ends_with('%')) {
                    if pos >= 2 {
                        let size_cand = format!("{} {}", parts[pos - 2], parts[pos - 1]);
                        let bytes = crate::utils::formatting::parse_size_to_bytes(&size_cand).unwrap_or(0);
                        total_docker_reclaimable += bytes;
                    }
                }
            }
            if total_docker_reclaimable > 0 {
                items.push(ReclaimableItem {
                    name: "Docker Containers & Build Layers".to_string(),
                    description: "Unused Docker images, containers, and build caches".to_string(),
                    estimated_bytes: total_docker_reclaimable,
                });
            }
        }
    }

    let total_bytes = items.iter().map(|i| i.estimated_bytes).sum();
    ReclaimableSummary { items, total_bytes }
}

pub fn clean_cache(dry_run: bool, auto_yes: bool, ollama: &OllamaClient, model: &str) -> Result<()> {
    println!("{}", "\n🧹 [bioclean free cache] Cleaning Package & Layer Caches".bright_cyan().bold());
    let home = std::env::var("HOME").unwrap_or_else(|_| "/home/clever".to_string());
    let summary = estimate_reclaimable();

    // clean_cache never touches the journal (that's clean_logs' job) — exclude
    // it from what this command's own summary/preview promises to free.
    let cache_items: Vec<_> = summary.items.iter().filter(|i| i.name != "Systemd Journal Logs").collect();
    let cache_bytes: u64 = cache_items.iter().map(|i| i.estimated_bytes).sum();

    println!("Identified reclaimable package/layer caches: {}", format_bytes(cache_bytes).bright_yellow());
    for item in &cache_items {
        println!(
            "  • {:<35} : {}  {}",
            item.name.bold(),
            format_bytes(item.estimated_bytes).yellow(),
            item.description.dimmed()
        );
    }

    if dry_run {
        println!("\n{}", "🔎 [DRY RUN] Simulation complete. No files were deleted.".bright_green().bold());
        return Ok(());
    }

    let preview = ActionPreview {
        action: "Purge package & layer caches (APT, Pip, UV, Conda, Docker, misc)".to_string(),
        current_state: format!("{} reclaimable across {} cache locations", format_bytes(cache_bytes), cache_items.len()),
        future_state: format!("~{} freed; caches repopulate automatically on next build/install", format_bytes(cache_bytes)),
        risk: RiskLevel::Sensitive,
    };
    if !verify_safety_interlock(&preview, "System & User Package Caches", "purge and vacuum", ollama, model, auto_yes) {
        println!("{}", "Cleanup aborted by user.".yellow());
        return Ok(());
    }

    // Execute APT Clean
    if command_exists("apt-get") {
        println!("  🧹 Cleaning APT package cache...");
        if is_root() {
            let _ = run_cmd_status("apt-get", &["clean"]);
        } else {
            let _ = run_cmd_status("sudo", &["apt-get", "clean"]);
        }
    }

    // Execute Pip Cache Purge
    if command_exists("pip") || command_exists("pip3") {
        let pip_bin = if command_exists("pip") { "pip" } else { "pip3" };
        println!("  🧹 Purging Pip cache...");
        let _ = run_cmd_status(pip_bin, &["cache", "purge"]);
    }

    // Execute UV Cache Clean
    if command_exists("uv") {
        println!("  🧹 Cleaning UV cache...");
        let _ = run_cmd_status("uv", &["cache", "clean"]);
    }

    // Conda / Mamba
    if command_exists("conda") {
        println!("  🧹 Purging Conda package archives...");
        let _ = run_cmd_status("conda", &["clean", "--tarballs", "-y"]);
    }

    // Docker prune
    if command_exists("docker") {
        println!("  🧹 Pruning unused Docker build layers and stopped containers...");
        let _ = run_cmd_status("docker", &["system", "prune", "-f"]);
    }

    // Misc user caches
    println!("  🧹 Purging temporary browser and build artifacts...");
    let caches_to_delete = [
        format!("{}/.cache/thumbnails", home),
        format!("{}/.cache/Cypress", home),
        format!("{}/.cache/node-gyp", home),
        format!("{}/.cache/ms-playwright-go", home),
    ];
    for p in &caches_to_delete {
        let path = Path::new(p);
        if path.exists() {
            let _ = fs::remove_dir_all(path);
        }
    }

    println!("{}", "\n✔ Cache cleanup completed successfully!".bright_green().bold());
    Ok(())
}

pub fn clean_logs(days: u32, dry_run: bool, auto_yes: bool) -> Result<()> {
    println!("{}", format!("\n📜 [bioclean free logs] Vacuuming System Logs Older than {} Days", days).bright_cyan().bold());

    if !command_exists("journalctl") {
        println!("{}", "journalctl is not available on this system.".yellow());
        return Ok(());
    }

    let before_usage = run_cmd_stdout("journalctl", &["--disk-usage"]).unwrap_or_default();
    println!("Current journal disk usage: {}", before_usage.trim().bright_yellow());

    if dry_run {
        println!("{}", format!("🔎 [DRY RUN] Would vacuum journalctl logs older than {} days.", days).bright_green());
        return Ok(());
    }

    let preview = ActionPreview {
        action: format!("Vacuum journalctl logs older than {} days", days),
        current_state: format!("Current journal disk usage: {}", before_usage.trim()),
        future_state: "Journal shrinks to retain only recent entries; new logs continue accumulating normally".to_string(),
        risk: RiskLevel::Safe,
    };
    if !auto_yes && !preview.confirm() {
        println!("{}", "Journal cleanup skipped.".yellow());
        return Ok(());
    }

    let days_arg = format!("--vacuum-time={}d", days);
    let ok = if is_root() {
        run_cmd_status("journalctl", &[&days_arg])
    } else {
        run_cmd_status("sudo", &["journalctl", &days_arg])
    };

    if ok.unwrap_or(false) {
        let after_usage = run_cmd_stdout("journalctl", &["--disk-usage"]).unwrap_or_default();
        println!("New journal disk usage: {}", after_usage.trim().bright_green());
        println!("{}", "✔ Journal logs successfully vacuumed!".bright_green().bold());
    } else {
        println!("{}", "Failed to vacuum journalctl logs (may require sudo privileges).".red());
    }
    Ok(())
}

/// Scans /tmp and /var/tmp for items older than `min_age_hours`, returning
/// (path, size, is_dir) for each. Shared by `clean_tmp` and the TUI's
/// confirmation preview so both report the same real numbers.
pub fn scan_stale_tmp(min_age_hours: u32) -> Vec<(PathBuf, u64, bool)> {
    let target_dirs = ["/tmp", "/var/tmp"];
    let now = SystemTime::now();
    let min_age = Duration::from_secs(min_age_hours as u64 * 3600);

    let mut eligible_files = Vec::new();

    for dir in &target_dirs {
        let p = Path::new(dir);
        if let Ok(entries) = fs::read_dir(p) {
            for entry in entries.flatten() {
                let file_path = entry.path();
                // Skip system sockets or locked files
                if file_path.to_string_lossy().contains("systemd") || file_path.to_string_lossy().contains("snap") {
                    continue;
                }
                if let Ok(meta) = entry.metadata() {
                    let accessed = meta.accessed().unwrap_or(meta.modified().unwrap_or(now));
                    if let Ok(age) = now.duration_since(accessed) {
                        if age >= min_age {
                            let size = if meta.is_dir() { dir_size(&file_path) } else { meta.len() };
                            eligible_files.push((file_path, size, meta.is_dir()));
                        }
                    }
                }
            }
        }
    }
    eligible_files
}

pub fn clean_tmp(min_age_hours: u32, dry_run: bool, auto_yes: bool) -> Result<()> {
    println!("{}", format!("\n🗂 [bioclean free tmp] Safely Cleaning /tmp and /var/tmp (Age >= {}h)", min_age_hours).bright_cyan().bold());

    let eligible_files = scan_stale_tmp(min_age_hours);
    let total_bytes: u64 = eligible_files.iter().map(|(_, size, _)| size).sum();

    println!("Discovered {} stale temp items totaling {}", eligible_files.len().to_string().bright_yellow(), format_bytes(total_bytes).bright_yellow());

    if dry_run {
        println!("{}", "🔎 [DRY RUN] Simulation complete. No temp files were removed.".bright_green().bold());
        return Ok(());
    }

    if eligible_files.is_empty() {
        println!("{}", "No temporary files older than the specified age threshold found.".green());
        return Ok(());
    }

    let preview = ActionPreview {
        action: format!("Delete stale /tmp and /var/tmp items (age >= {}h)", min_age_hours),
        current_state: format!("{} stale items totaling {}", eligible_files.len(), format_bytes(total_bytes)),
        future_state: format!("{} items removed; {} freed on this filesystem", eligible_files.len(), format_bytes(total_bytes)),
        risk: RiskLevel::Sensitive,
    };
    if !auto_yes && !preview.confirm() {
        println!("{}", "Temp cleanup skipped.".yellow());
        return Ok(());
    }

    let mut removed_count = 0;
    for (path, _, is_dir) in eligible_files {
        let res = if is_dir {
            fs::remove_dir_all(&path)
        } else {
            fs::remove_file(&path)
        };
        if res.is_ok() {
            removed_count += 1;
        }
    }

    println!("{}", format!("✔ Safely removed {} stale temporary items!", removed_count).bright_green().bold());
    Ok(())
}

/// Counts packages `apt-get autoremove` would remove, via its `-s` (simulate)
/// flag. Cheap enough to call for a confirmation preview. Shared by
/// `clean_orphans` and the TUI so both report the same real count.
pub fn scan_autoremove_count() -> usize {
    if !command_exists("apt-get") {
        return 0;
    }
    let sim_out = run_cmd_stdout("apt-get", &["autoremove", "-s"]).unwrap_or_default();
    sim_out.lines().filter(|l| l.starts_with("Remv ")).count()
}

pub fn clean_orphans(dry_run: bool, auto_yes: bool) -> Result<()> {
    println!("{}", "\n📦 [bioclean free orphans] Removing Unneeded Package Dependencies".bright_cyan().bold());

    if command_exists("apt-get") {
        if dry_run {
            println!("{}", "🔎 [DRY RUN] Simulating `apt-get autoremove -s`...".bright_green());
            let out = run_cmd_stdout("apt-get", &["autoremove", "-s"]).unwrap_or_default();
            for line in out.lines().filter(|l| l.starts_with("Remv ") || l.starts_with("Conf ")) {
                println!("  {}", line.dimmed());
            }
            return Ok(());
        }

        let pkg_count = scan_autoremove_count();

        let preview = ActionPreview {
            action: "Autoremove unneeded package dependencies with apt".to_string(),
            current_state: format!("{} package(s) marked as no longer needed", pkg_count),
            future_state: format!("{} package(s) removed; their disk space freed", pkg_count),
            risk: RiskLevel::Sensitive,
        };
        if !auto_yes && !preview.confirm() {
            println!("{}", "Autoremove skipped.".yellow());
            return Ok(());
        }

        let ok = if is_root() {
            run_cmd_status("apt-get", &["autoremove", "-y"])
        } else {
            run_cmd_status("sudo", &["apt-get", "autoremove", "-y"])
        };

        if ok.unwrap_or(false) {
            println!("{}", "✔ Unneeded package dependencies removed.".bright_green().bold());
        }
    } else {
        println!("{}", "No supported package manager orphan tool found on this system.".yellow());
    }
    Ok(())
}
