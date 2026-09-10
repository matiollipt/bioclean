use anyhow::Result;
use colored::*;
use rayon::prelude::*;
use std::fs;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

use crate::ai::client::OllamaClient;
use crate::ai::fallback::heavy_scan_fallback;
use crate::ai::prompts::{self, HEAVY_SCAN_ROLE};
use crate::utils::formatting::format_bytes;
use crate::utils::procfs::read_active_sockets;

#[derive(Debug, Clone, serde::Serialize)]
pub struct HeavyEntry {
    pub path: String,
    pub size_bytes: u64,
    pub is_dir: bool,
    pub category: String,
}

fn classify_file_type(path: &Path) -> String {
    let name = path.file_name().unwrap_or_default().to_string_lossy().to_lowercase();
    if name.ends_with(".fastq") || name.ends_with(".fastq.gz") || name.ends_with(".fq") || name.ends_with(".fq.gz") {
        "FASTQ Raw Reads".to_string()
    } else if name.ends_with(".bam") || name.ends_with(".cram") || name.ends_with(".sam") {
        "BAM/CRAM Alignments".to_string()
    } else if name.ends_with(".sra") {
        "NCBI SRA Archive".to_string()
    } else if name.ends_with(".vcf") || name.ends_with(".vcf.gz") || name.ends_with(".bcf") {
        "VCF Variant Calls".to_string()
    } else if name.ends_with(".h5ad") || name.ends_with(".loom") || name.ends_with(".rds") || name.ends_with(".rdata") {
        "Single-Cell / Expression Matrix".to_string()
    } else if name.ends_with(".pth") || name.ends_with(".pt") || name.ends_with(".ckpt") || name.ends_with(".onnx") || name.ends_with(".safetensors") {
        "ML Model Checkpoint".to_string()
    } else if name.ends_with(".tar.gz") || name.ends_with(".tgz") || name.ends_with(".zip") {
        "Compressed Archive".to_string()
    } else if name.ends_with(".iso") || name.ends_with(".img") {
        "Disk Image".to_string()
    } else if path.to_string_lossy().contains("/docker/") {
        "Docker Layer / Storage".to_string()
    } else if path.to_string_lossy().contains("/.conda/") || path.to_string_lossy().contains("/envs/") {
        "Conda Environment".to_string()
    } else {
        "Data Artifact".to_string()
    }
}

pub fn scan_heavy_files(
    root_path: &Path,
    min_size_bytes: u64,
    limit: usize,
    use_ai: bool,
    ollama: &OllamaClient,
    model: &str,
) -> Result<Vec<HeavyEntry>> {
    println!("{}", format!("\n🔍 [bioclean scan heavy] Scanning '{}' (>= {})", root_path.display(), format_bytes(min_size_bytes)).bright_cyan().bold());

    let entries: Vec<PathBuf> = match fs::read_dir(root_path) {
        Ok(read_dir) => read_dir.flatten().map(|e| e.path()).collect(),
        Err(_) => vec![root_path.to_path_buf()],
    };

    let heavy_items: Vec<HeavyEntry> = entries
        .par_iter()
        .filter_map(|entry| {
            if let Ok(meta) = entry.metadata() {
                let (size, is_dir) = if meta.is_dir() {
                    let total: u64 = WalkDir::new(entry)
                        .max_depth(4)
                        .into_iter()
                        .filter_map(|e| e.ok())
                        .filter_map(|e| e.metadata().ok())
                        .filter(|m| m.is_file())
                        .map(|m| m.len())
                        .sum();
                    (total, true)
                } else {
                    (meta.len(), false)
                };

                if size >= min_size_bytes {
                    let cat = classify_file_type(entry);
                    Some(HeavyEntry {
                        path: entry.to_string_lossy().to_string(),
                        size_bytes: size,
                        is_dir,
                        category: cat,
                    })
                } else {
                    None
                }
            } else {
                None
            }
        })
        .collect();

    let mut sorted_items = heavy_items;
    sorted_items.sort_by(|a, b| b.size_bytes.cmp(&a.size_bytes));
    sorted_items.truncate(limit);

    if sorted_items.is_empty() {
        println!("{}", "  No files or folders matching the size threshold were found.".green());
        return Ok(sorted_items);
    }

    println!("\n  {:<12} {:<30} {:<40}", "SIZE", "CATEGORY", "PATH");
    println!("  {}", "-".repeat(85).dimmed());
    for item in &sorted_items {
        let size_str = format_bytes(item.size_bytes).bright_yellow().bold();
        let cat_str = item.category.cyan();
        let path_str = if item.is_dir {
            format!("{}/", item.path).bold()
        } else {
            item.path.normal()
        };
        println!("  {:<12} {:<30} {:<40}", size_str, cat_str, path_str);
    }

    if use_ai {
        println!("\n{}", "🤖 [AI Storage Summary & Optimization Recommendations]".bright_yellow().bold());
        let sample_data: Vec<(String, u64)> = sorted_items.iter().map(|i| (i.path.clone(), i.size_bytes)).collect();

        if ollama.is_online() {
            let summary_input = sorted_items
                .iter()
                .take(8)
                .map(|i| format!("- Path: {}, Size: {}, Category: {}", i.path, format_bytes(i.size_bytes), i.category))
                .collect::<Vec<_>>()
                .join("\n");

            let prompt = format!(
                "Here are the largest files and folders detected on this workstation:\n{}\n\nExplain why these folders are large and recommend concrete cleanup or offloading steps.",
                summary_input
            );

            let system_prompt = prompts::compose(HEAVY_SCAN_ROLE);
            match ollama.generate(model, &prompt, Some(&system_prompt), None) {
                Ok(ai_out) if !ai_out.is_empty() => {
                    println!("{}", ai_out);
                }
                _ => {
                    println!("{}", heavy_scan_fallback(&sample_data));
                }
            }
        } else {
            println!("{}", heavy_scan_fallback(&sample_data));
        }
    }

    Ok(sorted_items)
}

pub fn scan_active_sockets(listen_only: bool, json_output: bool) -> Result<()> {
    let mut sockets = read_active_sockets();

    if listen_only {
        sockets.retain(|s| s.state == "LISTEN");
    }

    if json_output {
        println!("{}", serde_json::to_string_pretty(&sockets)?);
        return Ok(());
    }

    println!("{}", "\n🌐 [bioclean scan sockets] Auditing Active Network Sockets & Processes".bright_cyan().bold());
    println!("{}", "=".repeat(85).dimmed());

    if sockets.is_empty() {
        println!("{}", "  No matching active network sockets found.".green());
        return Ok(());
    }

    println!("  {:<6} {:<22} {:<22} {:<12} {:<8} {:<15}", "PROTO", "LOCAL ADDR", "REMOTE ADDR", "STATE", "PID", "PROCESS");
    println!("  {}", "-".repeat(85).dimmed());

    for s in &sockets {
        let local = format!("{}:{}", s.local_addr, s.local_port);
        let remote = if s.remote_port == 0 {
            "*:*".to_string()
        } else {
            format!("{}:{}", s.remote_addr, s.remote_port)
        };

        let state_colored = match s.state.as_str() {
            "LISTEN" => s.state.bright_green(),
            "ESTABLISHED" => s.state.bright_cyan(),
            "TIME_WAIT" | "CLOSE_WAIT" => s.state.yellow(),
            _ => s.state.normal(),
        };

        let pid_str = s.pid.map(|p| p.to_string()).unwrap_or_else(|| "-".to_string());
        let proc_str = s.process_name.as_deref().unwrap_or("-");

        println!(
            "  {:<6} {:<22} {:<22} {:<12} {:<8} {:<15}",
            s.protocol.bold(),
            local,
            remote,
            state_colored,
            pid_str.dimmed(),
            proc_str.bright_yellow()
        );
    }

    Ok(())
}
