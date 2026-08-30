use anyhow::Result;
use clap::Parser;
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
    let cli = Cli::parse();
    let config = Config::load();
    let ollama = OllamaClient::new(&config.ollama_url);
    let model = ollama.select_best_model(cli.model.as_deref().or(if config.default_model.is_empty() { None } else { Some(&config.default_model) }));
    let history_mgr = HistoryManager::new(&config.history_file);

    // 1. Handle Legacy Flags (backward compatibility with clean-disk)
    if cli.audit {
        modules::diagnose::run_diagnose(&ollama, &model, None, false)?;
        return Ok(());
    }
    if cli.clean {
        modules::free::clean_cache(false, false, &ollama, &model)?;
        return Ok(());
    }
    if cli.migrate {
        modules::hdd::migrate_bio_datasets(None, &history_mgr, false)?;
        return Ok(());
    }
    if cli.undo {
        history_mgr.undo_last_session(false)?;
        return Ok(());
    }
    if cli.history_log {
        history_mgr.list_history()?;
        return Ok(());
    }

    // 2. Handle TUI Launch
    if cli.tui || cli.command.is_none() {
        // If no subcommand is given and stdout is a terminal, launch TUI
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
            modules::diagnose::run_diagnose(&ollama, &model, args.output.as_deref(), args.json)?;
        }

        Some(Commands::Workflow(args)) => match args.action {
            WorkflowAction::PrepareCrunch { scratch, dry_run, yes } => {
                modules::workflows::workflow_prepare_crunch(&scratch, dry_run, yes, &ollama, &model)?;
            }
            WorkflowAction::Maintenance { output, dry_run, yes } => {
                modules::workflows::workflow_maintenance(output.as_deref(), dry_run, yes, &ollama, &model)?;
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
