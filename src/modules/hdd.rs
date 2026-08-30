use anyhow::Result;
use colored::*;
use std::fs;
use std::path::{Path, PathBuf};
use sysinfo::Disks;

use crate::modules::history::HistoryManager;
use crate::utils::formatting::format_bytes;
use crate::utils::system::{confirm_prompt, run_cmd_status};

#[derive(Debug, Clone)]
pub struct HddMount {
    pub device: String,
    pub fstype: String,
    pub mount_point: String,
    pub total_bytes: u64,
    pub free_bytes: u64,
}

pub fn scan_external_mounts() -> Vec<HddMount> {
    let mut mounts = Vec::new();
    let disks = Disks::new_with_refreshed_list();

    for d in disks.list() {
        let mp = d.mount_point().to_string_lossy().to_string();
        if mp.starts_with("/media") || mp.starts_with("/mnt") || mp.starts_with("/run/media") {
            if !mp.starts_with("/snap") {
                mounts.push(HddMount {
                    device: d.name().to_string_lossy().to_string(),
                    fstype: d.file_system().to_string_lossy().to_string(),
                    mount_point: mp,
                    total_bytes: d.total_space(),
                    free_bytes: d.available_space(),
                });
            }
        }
    }
    mounts
}

pub fn list_external_hdds(json_output: bool) -> Result<Vec<HddMount>> {
    let mounts = scan_external_mounts();

    if json_output {
        let json_arr: Vec<_> = mounts.iter().map(|m| {
            serde_json::json!({
                "device": m.device,
                "fstype": m.fstype,
                "mount_point": m.mount_point,
                "total_bytes": m.total_bytes,
                "free_bytes": m.free_bytes,
            })
        }).collect();
        println!("{}", serde_json::to_string_pretty(&json_arr)?);
        return Ok(mounts);
    }

    println!("{}", "\n💾 [bioclean hdd scan] Scanning Attached External Hard Drives".bright_cyan().bold());
    println!("{}", "=".repeat(75).dimmed());

    if mounts.is_empty() {
        println!("{}", "  ⚠️ No external HDDs currently detected under /media, /mnt, or /run/media.".yellow());
        println!("{}", "     Please connect and mount your external HDD to offload datasets.".dimmed());
    } else {
        println!("  {:<16} {:<10} {:<12} {:<12} {:<25}", "DEVICE", "FSTYPE", "TOTAL", "FREE", "MOUNT POINT");
        println!("  {}", "-".repeat(75).dimmed());
        for m in &mounts {
            println!(
                "  {:<16} {:<10} {:<12} {:<12} {:<25}",
                m.device.bright_cyan(),
                m.fstype,
                format_bytes(m.total_bytes),
                format_bytes(m.free_bytes).bright_green(),
                m.mount_point.bold()
            );
        }
    }
    Ok(mounts)
}

pub fn migrate_bio_datasets(
    target_hdd: Option<&str>,
    history_mgr: &HistoryManager,
    auto_yes: bool,
) -> Result<()> {
    let mounts = scan_external_mounts();
    let selected_mount: String = if let Some(t) = target_hdd {
        t.to_string()
    } else if let Some(first) = mounts.first() {
        first.mount_point.clone()
    } else {
        println!("{}", "No external HDD found. Please specify target path with --target-hdd <path>.".red());
        return Ok(());
    };

    println!("{}", format!("\n💾 Target External HDD: {}", selected_mount.bright_green().bold()));

    let home = std::env::var("HOME").unwrap_or_else(|_| "/home/clever".to_string());
    let base_src = format!("{}/aidbio/ds/transcriptome/data", home);
    let base_dest = format!("{}/aidbio_storage/transcriptome", selected_mount);

    let candidates = [
        format!("{}/raw/sra", base_src),
        format!("{}/processed/fastq", base_src),
        format!("{}/processed/trimmed", base_src),
    ];

    let session_id = history_mgr.start_session()?;
    println!("Created transaction session: {}", session_id.bright_yellow());

    for src_str in &candidates {
        let src_path = Path::new(src_str);
        if src_path.exists() && !src_path.is_symlink() {
            let cat_name = src_path.file_name().unwrap().to_string_lossy();
            let dest_str = format!("{}/{}", base_dest, cat_name);
            let dest_path = Path::new(&dest_str);

            println!("\n  • Found local dataset: {}", src_str.bright_cyan());

            if auto_yes || confirm_prompt(&format!("Move {} to external HDD ({}) and replace with symlink?", src_str, dest_str), false) {
                println!("    Copying data via rsync...");
                let _ = fs::create_dir_all(dest_path);
                let _ = run_cmd_status("rsync", &["-av", "--progress", "--remove-source-files", &format!("{}/", src_str), &format!("{}/", dest_str)]);
                let _ = fs::remove_dir_all(src_path);
                
                // Create symlink
                #[cfg(unix)]
                std::os::unix::fs::symlink(dest_path, src_path)?;

                history_mgr.record_op(&session_id, "MOVE_AND_SYMLINK", src_str, &dest_str, 0)?;
                println!("    {}", format!("✔ Symlinked: {} -> {}", src_str, dest_str).bright_green());
            }
        } else if src_path.is_symlink() {
            println!("  • Dataset already symlinked: {}", src_str.dimmed());
        }
    }

    // Configure NCBI SRA Toolkit cache
    let ncbi_cache = format!("{}/aidbio_storage/ncbi_cache", selected_mount);
    if auto_yes || confirm_prompt(&format!("Reconfigure NCBI SRA Toolkit cache to HDD ({})?", ncbi_cache), true) {
        reconfigure_ncbi_sra(&selected_mount)?;
    }

    println!("{}", "\n✔ Migration workflow completed!".bright_green().bold());
    Ok(())
}

pub fn reconfigure_ncbi_sra(target_hdd: &str) -> Result<()> {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/home/clever".to_string());
    let ncbi_dir = PathBuf::from(&home).join(".ncbi");
    let cache_dir = format!("{}/aidbio_storage/ncbi_cache", target_hdd);
    fs::create_dir_all(&ncbi_dir)?;
    fs::create_dir_all(&cache_dir)?;

    let config_content = format!(
r#"/LIBS/GUID = "d906eff4-d782-4e40-b228-1189d36c2937"
/config/default = "false"
/libs/cache_amount = "8"
/libs/temp_cache = "{cache_dir}"
/libs/vdb/quality = "RZ"
/repository/user/main/public/root = "{cache_dir}"
"#
    );

    fs::write(ncbi_dir.join("user-settings.mkfg"), config_content)?;
    println!("{}", "  ✔ Reconfigured ~/.ncbi/user-settings.mkfg to use external HDD cache.".bright_green());
    Ok(())
}
