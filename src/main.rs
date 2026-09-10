use anyhow::Result;
use clap::Parser;
use colored::*;
use crossterm::{
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::io;
use std::path::Path;

mod ai;
mod cli;
mod config;
mod modules;
mod tui;
mod utils;

use ai::client::OllamaClient;
use cli::{Cli, Commands, FreeAction, HddAction, HistoryAction, PowerAction, ScanAction, WorkflowAction};
use config::Config;
use modules::history::HistoryManager;
use utils::formatting::parse_size_to_bytes;

fn main() -> Result<()> {
    let raw_args: Vec<String> = std::env::args().collect();
    let invoked_name = raw_args
        .first()
        .map(|s| Path::new(s).file_name().unwrap_or_default().to_string_lossy().to_string())
        .unwrap_or_default();

    let mut config = Config::load();
    let ollama = OllamaClient::new(&config.ollama_url);
    let history_mgr = HistoryManager::new(&config.history_file);

    // 1. Handle Legacy Invocation (backward compatibility with clean-disk and legacy flags)
    let has_legacy_flags = raw_args.iter().any(|a| {
        matches!(
            a.as_str(),
            "-a" | "--audit" | "-c" | "--clean" | "-m" | "--migrate" | "-u" | "--undo" | "-l" | "--history-log"
        )
    });

    if invoked_name == "clean-disk" || has_legacy_flags {
        let model = ollama.select_best_model(if config.default_model.is_empty() {
            None
        } else {
            Some(&config.default_model)
        });

        if raw_args.iter().any(|a| a == "-a" || a == "--audit") {
            eprintln!("{}", "⚠️  Notice: '-a / --audit' is legacy. Forwarding to 'bioclean diagnose'...".yellow());
            modules::diagnose::run_diagnose(&ollama, &model, &config, None, false)?;
            return Ok(());
        }
        if raw_args.iter().any(|a| a == "-c" || a == "--clean") {
            eprintln!("{}", "⚠️  Notice: '-c / --clean' is legacy. Forwarding to 'bioclean free cache'...".yellow());
            modules::free::clean_cache(false, false, &ollama, &model)?;
            return Ok(());
        }
        if raw_args.iter().any(|a| a == "-m" || a == "--migrate") {
            eprintln!("{}", "⚠️  Notice: '-m / --migrate' is legacy. Forwarding to 'bioclean hdd migrate'...".yellow());
            modules::hdd::migrate_bio_datasets(None, &history_mgr, false)?;
            return Ok(());
        }
        if raw_args.iter().any(|a| a == "-u" || a == "--undo") {
            eprintln!("{}", "⚠️  Notice: '-u / --undo' is legacy. Forwarding to 'bioclean history undo'...".yellow());
            history_mgr.undo_last_session(false)?;
            return Ok(());
        }
        if raw_args.iter().any(|a| a == "-l" || a == "--history-log") {
            eprintln!("{}", "⚠️  Notice: '-l / --history-log' is legacy. Forwarding to 'bioclean history list'...".yellow());
            history_mgr.list_history()?;
            return Ok(());
        }
    }

    let cli = Cli::parse();
    let model = ollama.select_best_model(cli.model.as_deref().or(if config.default_model.is_empty() {
        None
    } else {
        Some(&config.default_model)
    }));

    // 2. Handle TUI Launch
    if cli.tui || cli.command.is_none() {
        if crossterm::terminal::is_raw_mode_enabled().is_ok() || std::env::var("TERM").is_ok() {
            enable_raw_mode()?;
            let mut stdout = io::stdout();
            execute!(stdout, EnterAlternateScreen)?;
            let backend = CrosstermBackend::new(stdout);
            let mut terminal = Terminal::new(backend)?;

            let app = tui::App::new(config, ollama, model);
            let res = tui::run_tui(&mut terminal, app);

            disable_raw_mode()?;
            execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
            terminal.show_cursor()?;

            if let Err(err) = res {
                eprintln!("TUI Error: {:?}", err);
            }
            return Ok(());
        }
    }

    // 3. Handle Subcommands
    match cli.command {
        Some(Commands::Config(args)) => {
            if let Some(ref param_pair) = args.param {
                if param_pair.len() == 2 {
                    let key = &param_pair[0];
                    let val = &param_pair[1];
                    config.set_param(key, val)?;
                    println!(
                        "{} Set configuration parameter '{}' = '{}'",
                        "✔".bright_green(),
                        key.bright_yellow(),
                        val.bright_cyan()
                    );
                    return Ok(());
                } else {
                    anyhow::bail!("--param requires 2 arguments: <SETTING> <VALUE>");
                }
            }

            if let Some(ref key) = args.get {
                let val = config.get_param(key)?;
                println!("{}", val);
                return Ok(());
            }

            if args.list {
                config.print_list();
                return Ok(());
            }

            if args.reset {
                let def = Config::default();
                def.save()?;
                println!("{}", "✔ Configuration reset to factory defaults.".bright_green().bold());
                return Ok(());
            }

            match args.action {
                Some(cli::ConfigAction::Wizard) => {
                    config.run_wizard(&ollama)?;
                }
                Some(cli::ConfigAction::List) => {
                    config.print_list();
                }
                Some(cli::ConfigAction::Get { key }) => {
                    let val = config.get_param(&key)?;
                    println!("{}", val);
                }
                Some(cli::ConfigAction::Set { key, value }) => {
                    config.set_param(&key, &value)?;
                    println!(
                        "{} Set configuration parameter '{}' = '{}'",
                        "✔".bright_green(),
                        key.bright_yellow(),
                        value.bright_cyan()
                    );
                }
                Some(cli::ConfigAction::Reset) => {
                    let def = Config::default();
                    def.save()?;
                    println!("{}", "✔ Configuration reset to factory defaults.".bright_green().bold());
                }
                None => {
                    config.run_wizard(&ollama)?;
                }
            }
        }

        Some(Commands::Free(args)) => match args.action {
            Some(FreeAction::Cache { dry_run, yes }) => {
                modules::free::clean_cache(dry_run || args.dry_run, yes || args.yes, &ollama, &model)?;
            }
            Some(FreeAction::Logs { days, dry_run, yes }) => {
                modules::free::clean_logs(days, dry_run || args.dry_run, yes || args.yes)?;
            }
            Some(FreeAction::Tmp { min_age_hours, dry_run, yes }) => {
                modules::free::clean_tmp(min_age_hours, dry_run || args.dry_run, yes || args.yes)?;
            }
            Some(FreeAction::Orphans { dry_run, yes }) => {
                modules::free::clean_orphans(dry_run || args.dry_run, yes || args.yes)?;
            }
            None => {
                modules::free::clean_cache(args.dry_run, args.yes, &ollama, &model)?;
            }
        },

        Some(Commands::Power(args)) => match args.action {
            Some(PowerAction::Battery { dry_run }) => {
                modules::power::set_battery_profile(dry_run)?;
            }
            Some(PowerAction::Performance { dry_run }) => {
                modules::power::set_performance_profile(dry_run)?;
            }
            Some(PowerAction::Thermal { json }) => {
                modules::power::monitor_thermal(json || args.json)?;
            }
            None => {
                modules::power::monitor_thermal(args.json)?;
            }
        },

        Some(Commands::Scan(args)) => match args.action {
            Some(ScanAction::Heavy { path, min_size, limit, no_ai }) => {
                let min_bytes = parse_size_to_bytes(&min_size).unwrap_or(50 * 1024 * 1024);
                let scan_path = Path::new(&path);
                modules::scan::scan_heavy_files(scan_path, min_bytes, limit, !no_ai, &ollama, &model)?;
            }
            Some(ScanAction::Sockets { listen, json }) => {
                modules::scan::scan_active_sockets(listen, json)?;
            }
            None => {
                let default_path = Path::new(".");
                modules::scan::scan_heavy_files(default_path, 50 * 1024 * 1024, 15, true, &ollama, &model)?;
            }
        },

        Some(Commands::Diagnose(args)) => {
            modules::diagnose::run_diagnose(&ollama, &model, &config, args.output.as_deref(), args.json)?;
        }

        Some(Commands::Workflow(args)) => match args.action {
            WorkflowAction::PrepareCrunch { scratch, dry_run, yes } => {
                modules::workflows::workflow_prepare_crunch(&scratch, dry_run, yes, &ollama, &model)?;
            }
            WorkflowAction::Maintenance { output, dry_run, yes } => {
                modules::workflows::workflow_maintenance(output.as_deref(), dry_run, yes, &ollama, &model, &config)?;
            }
        },

        Some(Commands::Hdd(args)) => match args.action {
            Some(HddAction::Scan { json }) => {
                modules::hdd::list_external_hdds(json)?;
            }
            Some(HddAction::Migrate { target_hdd, yes }) => {
                modules::hdd::migrate_bio_datasets(target_hdd.as_deref(), &history_mgr, yes)?;
            }
            Some(HddAction::ReconfigureSra { target_hdd }) => {
                modules::hdd::reconfigure_ncbi_sra(&target_hdd)?;
            }
            None => {
                modules::hdd::list_external_hdds(false)?;
            }
        },

        Some(Commands::History(args)) => match args.action {
            Some(HistoryAction::List) | None => {
                history_mgr.list_history()?;
            }
            Some(HistoryAction::Undo { yes }) => {
                history_mgr.undo_last_session(yes)?;
            }
        },

        None => {}
    }

    Ok(())
}
