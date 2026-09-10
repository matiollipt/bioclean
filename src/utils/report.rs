use anyhow::Result;
use chrono::Utc;
use colored::*;
use std::fs;
use std::io::{self, IsTerminal, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

pub fn find_tool_path(name: &str) -> Option<PathBuf> {
    if let Ok(path_out) = crate::utils::system::run_cmd_stdout("which", &[name]) {
        let p = path_out.trim();
        if !p.is_empty() && Path::new(p).exists() {
            return Some(PathBuf::from(p));
        }
    }
    let fallbacks = match name {
        "glow" => vec!["/snap/bin/glow", "/usr/local/bin/glow", "/usr/bin/glow"],
        "batcat" => vec!["/usr/bin/batcat", "/usr/local/bin/batcat"],
        "bat" => vec!["/usr/bin/bat", "/usr/local/bin/bat"],
        _ => vec![],
    };
    for fb in fallbacks {
        if Path::new(fb).exists() {
            return Some(PathBuf::from(fb));
        }
    }
    None
}

pub fn display_report(content: &str, preferred_formatter: &str) -> Result<()> {
    if !io::stdout().is_terminal() {
        println!("{}", content);
        return Ok(());
    }

    let pref = preferred_formatter.to_lowercase();

    if pref == "terminal" {
        render_terminal_colored(content);
        return Ok(());
    }

    let try_glow = pref == "glow" || pref == "auto";
    let try_bat = pref == "bat" || pref == "batcat" || pref == "auto";

    if try_glow {
        if let Some(glow_path) = find_tool_path("glow") {
            if render_with_pipe(&glow_path, &["-p"], content).is_ok() {
                return Ok(());
            }
            if render_with_pipe(&glow_path, &[], content).is_ok() {
                return Ok(());
            }
        }
    }

    if try_bat {
        if let Some(bat_path) = find_tool_path("batcat").or_else(|| find_tool_path("bat")) {
            if render_with_pipe(
                &bat_path,
                &["--language=markdown", "--style=plain,header", "--paging=auto"],
                content,
            )
            .is_ok()
            {
                return Ok(());
            }
        }
    }

    render_terminal_colored(content);
    Ok(())
}

fn render_with_pipe(cmd: &Path, args: &[&str], input: &str) -> Result<()> {
    let mut child = Command::new(cmd)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()?;

    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(input.as_bytes())?;
    }

    let status = child.wait()?;
    if status.success() {
        Ok(())
    } else {
        anyhow::bail!("Command {:?} exited with status {}", cmd, status);
    }
}

pub fn render_terminal_colored(content: &str) {
    for line in content.lines() {
        if line.starts_with("# ") {
            println!("\n{}", line.bright_cyan().bold());
        } else if line.starts_with("## ") {
            println!("\n{}", line.bright_blue().bold());
        } else if line.starts_with("### ") {
            println!("{}", line.yellow().bold());
        } else if line.starts_with("- ") || line.starts_with("• ") {
            println!("  {}", line.green());
        } else if line.starts_with("```") {
            println!("{}", line.dimmed());
        } else if line.contains("CRITICAL") {
            println!("{}", line.bright_red().bold());
        } else if line.contains("WARNING") || line.contains("DEGRADED") {
            println!("{}", line.bright_yellow().bold());
        } else if line.contains("OPTIMAL") || line.contains("HEALTHY") {
            println!("{}", line.bright_green().bold());
        } else {
            println!("{}", line);
        }
    }
}

pub fn save_report(
    report_content: &str,
    output_dir: &str,
    run_id: &str,
) -> Result<PathBuf> {
    let dir = Path::new(output_dir);
    fs::create_dir_all(dir)?;

    let timestamp = Utc::now().format("%Y%m%d_%H%M%S");
    let filename = format!("report_{}_{}.md", timestamp, run_id);
    let full_path = dir.join(filename);
    fs::write(&full_path, report_content)?;
    Ok(full_path)
}
