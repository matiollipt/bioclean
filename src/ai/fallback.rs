use crate::utils::formatting::format_bytes;
use crate::utils::procfs::MemInfo;

pub fn diagnose_fallback(
    dmesg_err_count: usize,
    journal_err_count: usize,
    root_used_pct: u32,
    mem: &MemInfo,
    max_temp: f32,
) -> String {
    let mut score: i32 = 100;
    let mut issues = Vec::new();
    let mut actions = Vec::new();

    if root_used_pct > 90 {
        score -= 30;
        issues.push(format!("CRITICAL: Root filesystem usage is dangerously high ({root_used_pct}%)."));
        actions.push("• Run `bioclean free cache` and `bioclean free logs` to reclaim disk space immediately.");
    } else if root_used_pct > 80 {
        score -= 15;
        issues.push(format!("WARNING: Root filesystem usage is elevated ({root_used_pct}%)."));
        actions.push("• Consider running `bioclean free cache` or offloading bio datasets with `bioclean hdd migrate`.");
    }

    let mem_used_pct = if mem.total_bytes > 0 {
        ((mem.total_bytes.saturating_sub(mem.available_bytes)) as f64 / mem.total_bytes as f64 * 100.0) as u32
    } else {
        0
    };

    if mem_used_pct > 90 {
        score -= 20;
        issues.push(format!("WARNING: System RAM utilization is high ({mem_used_pct}% used)."));
        actions.push("• Check for memory leaks or zombie processes using `bioclean scan sockets`.");
    }

    if max_temp > 85.0 {
        score -= 20;
        issues.push(format!("WARNING: Elevated CPU/Package temperature detected ({max_temp:.1}°C)."));
        actions.push("• Check cooling/fans and consider adjusting power profile with `bioclean power thermal`.");
    }

    if journal_err_count > 10 {
        score -= 10;
        issues.push(format!("INFO: Found {journal_err_count} recent systemd journal errors (priority <= err)."));
        actions.push("• Inspect recent journalctl logs (`journalctl -p 3 -n 20`).");
    }

    if dmesg_err_count > 5 {
        score -= 10;
        issues.push(format!("INFO: Found {dmesg_err_count} kernel hardware or driver warnings in dmesg."));
    }

    if score < 0 {
        score = 0;
    }

    let status = if score >= 85 {
        "OPTIMAL"
    } else if score >= 65 {
        "DEGRADED"
    } else {
        "CRITICAL"
    };

    if actions.is_empty() {
        actions.push("• System is in good working order. No immediate interventions required.");
    }

    let issues_md = if issues.is_empty() {
        "• No critical system anomalies detected.".to_string()
    } else {
        issues.iter().map(|i| format!("• {}", i)).collect::<Vec<_>>().join("\n")
    };

    format!(
r#"# 🩺 Bioclean System Health Report

### **Health Score & Status:** `{score}/100 — {status}`

### **Executive Summary:**
The system is operating with **{root_used_pct}%** disk usage on `/`, **{mem_used_pct}%** memory utilization ({mem_free_fmt} available of {mem_total_fmt}), and peak thermal reading at **{max_temp:.1}°C**.

### **Detected Issues & Observations:**
{issues_md}

### **Recommended Actions:**
{}
"#,
        actions.join("\n"),
        score = score,
        status = status,
        root_used_pct = root_used_pct,
        mem_used_pct = mem_used_pct,
        mem_free_fmt = format_bytes(mem.available_bytes),
        mem_total_fmt = format_bytes(mem.total_bytes),
        max_temp = max_temp,
        issues_md = issues_md,
    )
}

pub fn heavy_scan_fallback(samples: &[(String, u64)]) -> String {
    let mut bio_count = 0;
    let mut cache_count = 0;
    let mut total_bytes = 0;

    for (path, bytes) in samples {
        total_bytes += bytes;
        let lower = path.to_lowercase();
        if lower.ends_with(".bam") || lower.ends_with(".fastq") || lower.ends_with(".fq.gz") || lower.ends_with(".sra") || lower.ends_with(".vcf") {
            bio_count += 1;
        }
        if lower.contains(".cache") || lower.contains("docker") || lower.contains("conda/pkgs") {
            cache_count += 1;
        }
    }

    let mut notes = Vec::new();
    if bio_count > 0 {
        notes.push(format!("• Identified **{}** large bioinformatics files (.sra, .fastq, .bam, .vcf). Consider offloading raw/intermediate datasets to external HDD using `bioclean hdd migrate` to preserve SSD lifespan.", bio_count));
    }
    if cache_count > 0 {
        notes.push(format!("• Detected **{}** package/docker cache directories. These can be safely purged using `bioclean free cache`.", cache_count));
    }
    if notes.is_empty() {
        notes.push("• Large folders discovered across your workspace. Inspect files to determine if intermediate artifacts can be deleted.".to_string());
    }

    format!(
        "### 💡 Storage Analysis & Insights ({})\n{}\n",
        format_bytes(total_bytes),
        notes.join("\n")
    )
}

pub fn safety_warning_fallback(target_path: &str, action: &str) -> String {
    let lower = target_path.to_lowercase();
    if lower.contains("conda") || lower.contains("miniconda") || lower.contains("envs") {
        format!("I see you are about to {} '{}'. This may disrupt active Conda/Python environments and installed dependencies. Do you wish to proceed?", action, target_path)
    } else if lower.contains("aidbio") || lower.contains("transcriptome") || lower.contains(".bam") || lower.contains(".sra") {
        format!("I see you are about to {} '{}'. This targets computational biology datasets. Do you wish to proceed?", action, target_path)
    } else if lower.contains("docker") {
        format!("I see you are about to {} '{}'. This will remove Docker images, build caches, and unused volumes. Do you wish to proceed?", action, target_path)
    } else {
        format!("I see you are about to {} '{}'. Do you wish to proceed?", action, target_path)
    }
}
