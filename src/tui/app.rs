use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use ratatui::{
    backend::Backend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph, Tabs, Wrap},
    Frame, Terminal,
};
use std::time::{Duration, Instant};

use crate::ai::client::OllamaClient;
use crate::config::Config;
use crate::modules::free::{self, ReclaimableSummary};
use crate::utils::disks::{read_internal_disks, DiskUsageInfo};
use crate::utils::formatting::format_bytes;
use crate::utils::procfs::{
    power_reading_permission_denied, read_cpu_governors, read_journal_size, read_meminfo, read_power_reading, read_thermal_zones,
    PowerReading, PowerSource,
};
use crate::utils::system::{ActionPreview, RiskLevel};

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum TabItem {
    Dashboard = 0,
    SpaceRecovery = 1,
    PowerThermal = 2,
    Observability = 3,
    Workflows = 4,
}

impl TabItem {
    pub fn titles() -> Vec<&'static str> {
        vec![
            " 1. Dashboard ",
            " 2. Free Space ",
            " 3. Power & Thermals ",
            " 4. Observability ",
            " 5. Workflows ",
        ]
    }
}

/// Power reading display state, distinguishing "no reading yet" (RAPL needs a
/// second sample for a delta) from "not available on this system" (no
/// battery, no RAPL) — otherwise both look identical to the user.
#[derive(Debug, Clone)]
pub enum PowerDisplay {
    Sampling,
    Unavailable,
    PermissionDenied,
    Reading(PowerReading),
}

/// Telemetry data cached and refreshed on a slow interval, never recomputed
/// per-frame — `estimate_reclaimable()` in particular does a full filesystem
/// walk and must not run at the TUI's 250ms tick rate.
pub struct TelemetryCache {
    pub reclaimable: ReclaimableSummary,
    pub disks: Vec<DiskUsageInfo>,
    pub power: PowerDisplay,
    power_prev_sample: Option<(Instant, u64)>,
    pub journal_bytes: Option<u64>,
    last_refresh: Instant,
}

impl TelemetryCache {
    fn empty() -> Self {
        Self {
            reclaimable: ReclaimableSummary::default(),
            disks: Vec::new(),
            power: PowerDisplay::Sampling,
            power_prev_sample: None,
            journal_bytes: None,
            // Far enough in the past that the first loop iteration refreshes immediately.
            last_refresh: Instant::now() - Duration::from_secs(3600),
        }
    }
}

/// A confirmation the user must resolve (y/n/d/Esc) before the underlying
/// action runs. Rendered as a popup over the current tab.
pub struct PendingConfirmation {
    pub preview: ActionPreview,
}

pub struct App {
    pub current_tab: TabItem,
    pub menu_state: ListState,
    pub logs: Vec<String>,
    pub should_quit: bool,
    pub config: Config,
    pub ollama: OllamaClient,
    pub model: String,
    pub last_tick: Instant,
    pub telemetry: TelemetryCache,
    pub telemetry_refresh_interval: Duration,
    pub pending_confirmation: Option<PendingConfirmation>,
}

impl App {
    pub fn new(config: Config, ollama: OllamaClient, model: String) -> Self {
        let mut menu_state = ListState::default();
        menu_state.select(Some(0));

        Self {
            current_tab: TabItem::Dashboard,
            menu_state,
            logs: vec![
                "Welcome to AidBio bioclean Interactive Orchestrator.".to_string(),
                "Press Tab to switch sections, Arrow keys to navigate, Enter to trigger, 'q' to quit.".to_string(),
            ],
            should_quit: false,
            config,
            ollama,
            model,
            last_tick: Instant::now(),
            telemetry: TelemetryCache::empty(),
            telemetry_refresh_interval: Duration::from_secs(10),
            pending_confirmation: None,
        }
    }

    /// Recomputes cached telemetry (cache sizes, disk usage, power, journal
    /// size) only if the refresh interval has elapsed. Called once per loop
    /// iteration, never from inside `ui()`.
    pub fn maybe_refresh_telemetry(&mut self) {
        if self.telemetry.last_refresh.elapsed() < self.telemetry_refresh_interval {
            return;
        }
        self.telemetry.reclaimable = free::estimate_reclaimable();
        self.telemetry.disks = read_internal_disks();
        self.telemetry.journal_bytes = read_journal_size();

        let (reading, new_prev) = read_power_reading(self.telemetry.power_prev_sample);
        self.telemetry.power = match (reading, new_prev) {
            (Some(r), _) => PowerDisplay::Reading(r),
            (None, Some(sample)) => {
                self.telemetry.power_prev_sample = Some(sample);
                PowerDisplay::Sampling
            }
            (None, None) if power_reading_permission_denied() => PowerDisplay::PermissionDenied,
            (None, None) => PowerDisplay::Unavailable,
        };
        if let PowerDisplay::Reading(_) = &self.telemetry.power {
            self.telemetry.power_prev_sample = new_prev.or(self.telemetry.power_prev_sample);
        }

        self.telemetry.last_refresh = Instant::now();
    }

    pub fn next_tab(&mut self) {
        self.current_tab = match self.current_tab {
            TabItem::Dashboard => TabItem::SpaceRecovery,
            TabItem::SpaceRecovery => TabItem::PowerThermal,
            TabItem::PowerThermal => TabItem::Observability,
            TabItem::Observability => TabItem::Workflows,
            TabItem::Workflows => TabItem::Dashboard,
        };
    }

    pub fn prev_tab(&mut self) {
        self.current_tab = match self.current_tab {
            TabItem::Dashboard => TabItem::Workflows,
            TabItem::SpaceRecovery => TabItem::Dashboard,
            TabItem::PowerThermal => TabItem::SpaceRecovery,
            TabItem::Observability => TabItem::PowerThermal,
            TabItem::Workflows => TabItem::Observability,
        };
    }

    pub fn next_item(&mut self, max: usize) {
        if max == 0 {
            return;
        }
        let i = match self.menu_state.selected() {
            Some(i) => {
                if i >= max - 1 {
                    0
                } else {
                    i + 1
                }
            }
            None => 0,
        };
        self.menu_state.select(Some(i));
    }

    pub fn prev_item(&mut self, max: usize) {
        if max == 0 {
            return;
        }
        let i = match self.menu_state.selected() {
            Some(i) => {
                if i == 0 {
                    max - 1
                } else {
                    i - 1
                }
            }
            None => 0,
        };
        self.menu_state.select(Some(i));
    }

    /// Builds a confirmation preview for the currently selected action, or
    /// `None` for read-only actions (Dashboard's diagnose, Observability's
    /// scans) that should dispatch immediately without a prompt.
    pub fn build_pending_confirmation(&self) -> Option<PendingConfirmation> {
        let sel = self.menu_state.selected().unwrap_or(0);
        let preview = match self.current_tab {
            TabItem::Dashboard | TabItem::Observability => return None,
            TabItem::SpaceRecovery => match sel {
                0 => {
                    // clean_cache never touches the journal (clean_logs does) — exclude it here too.
                    let cache_items: Vec<_> = self.telemetry.reclaimable.items.iter().filter(|i| i.name != "Systemd Journal Logs").collect();
                    let cache_bytes: u64 = cache_items.iter().map(|i| i.estimated_bytes).sum();
                    ActionPreview {
                        action: "Purge package & layer caches (APT, Pip, UV, Conda, Docker, misc)".to_string(),
                        current_state: format!("{} reclaimable across {} cache locations", format_bytes(cache_bytes), cache_items.len()),
                        future_state: format!("~{} freed; caches repopulate automatically", format_bytes(cache_bytes)),
                        risk: RiskLevel::Sensitive,
                    }
                }
                1 => ActionPreview {
                    action: "Vacuum journalctl logs older than 7 days".to_string(),
                    current_state: format!("Journal at {}", self.telemetry.journal_bytes.map(format_bytes).unwrap_or_else(|| "unknown size".to_string())),
                    future_state: "Journal shrinks to retain only recent entries".to_string(),
                    risk: RiskLevel::Safe,
                },
                2 => {
                    let stale = free::scan_stale_tmp(48);
                    let stale_bytes: u64 = stale.iter().map(|(_, size, _)| size).sum();
                    ActionPreview {
                        action: "Safely clean /tmp and /var/tmp files (>= 48h old)".to_string(),
                        current_state: format!("{} stale items totaling {}", stale.len(), format_bytes(stale_bytes)),
                        future_state: format!("{} items removed; {} freed", stale.len(), format_bytes(stale_bytes)),
                        risk: RiskLevel::Sensitive,
                    }
                }
                3 => {
                    let pkg_count = free::scan_autoremove_count();
                    ActionPreview {
                        action: "Remove unneeded package dependencies (autoremove)".to_string(),
                        current_state: format!("{} package(s) marked as no longer needed", pkg_count),
                        future_state: format!("{} package(s) removed; their disk space freed", pkg_count),
                        risk: RiskLevel::Sensitive,
                    }
                }
                _ => return None,
            },
            TabItem::PowerThermal => match sel {
                0 => ActionPreview {
                    action: "Switch to Performance profile".to_string(),
                    current_state: "CPU governor may be set to powersave or balanced".to_string(),
                    future_state: "CPU governor set to performance; higher power draw and thermal load".to_string(),
                    risk: RiskLevel::Safe,
                },
                1 => ActionPreview {
                    action: "Switch to Battery / Power-save profile".to_string(),
                    current_state: "CPU governor may be set to performance".to_string(),
                    future_state: "CPU governor set to powersave; reduced peripheral drain".to_string(),
                    risk: RiskLevel::Safe,
                },
                2 => ActionPreview {
                    action: "Optimize dGPU, GNOME GPU-polling & Peripheral Power".to_string(),
                    current_state: "dGPU may be kept awake by polling loops or persistence mode; TLP/powertop may be unconfigured".to_string(),
                    future_state: "GPU polling stopped, dGPU D3cold enabled, persistence daemon masked, TLP + powertop enabled".to_string(),
                    risk: RiskLevel::Safe,
                },
                _ => return None,
            },
            TabItem::Workflows => match sel {
                0 => ActionPreview {
                    action: "Run 'prepare-crunch' workflow".to_string(),
                    current_state: format!("{} reclaimable across caches; governor not yet switched", format_bytes(self.telemetry.reclaimable.total_bytes)),
                    future_state: "Caches/logs cleared; CPU governor set to performance".to_string(),
                    risk: RiskLevel::Safe,
                },
                1 => ActionPreview {
                    action: "Run 'maintenance' workflow".to_string(),
                    current_state: format!("{} reclaimable across caches; SSDs not yet trimmed", format_bytes(self.telemetry.reclaimable.total_bytes)),
                    future_state: "Caches/logs/orphans cleared; SSDs trimmed; report written".to_string(),
                    risk: RiskLevel::Safe,
                },
                _ => return None,
            },
        };
        Some(PendingConfirmation { preview })
    }

    /// Dispatches the currently selected action with real dry_run/auto_yes
    /// values (previously hardcoded to `false, false` for every TUI action).
    pub fn execute_selected(&mut self, dry_run: bool, auto_yes: bool) {
        match self.current_tab {
            TabItem::Dashboard => {
                self.logs.push("Triggering full AI System Health Diagnostics...".to_string());
                match crate::modules::diagnose::run_diagnose(&self.ollama, &self.model, &self.config, None, false) {
                    Ok(_) => self.logs.push("✔ Diagnostics completed successfully!".to_string()),
                    Err(e) => self.logs.push(format!("✖ Diagnostics error: {}", e)),
                }
            }
            TabItem::SpaceRecovery => {
                let sel = self.menu_state.selected().unwrap_or(0);
                match sel {
                    0 => {
                        self.logs.push("Cleaning package & layer caches...".to_string());
                        let _ = crate::modules::free::clean_cache(dry_run, auto_yes, &self.ollama, &self.model);
                        self.logs.push("✔ Cache cleanup finished.".to_string());
                    }
                    1 => {
                        self.logs.push("Vacuuming journalctl logs...".to_string());
                        let _ = crate::modules::free::clean_logs(7, dry_run, auto_yes);
                        self.logs.push("✔ Logs vacuum finished.".to_string());
                    }
                    2 => {
                        self.logs.push("Safely cleaning /tmp files...".to_string());
                        let _ = crate::modules::free::clean_tmp(48, dry_run, auto_yes);
                        self.logs.push("✔ Temp cleanup finished.".to_string());
                    }
                    3 => {
                        self.logs.push("Purging unneeded package orphans...".to_string());
                        let _ = crate::modules::free::clean_orphans(dry_run, auto_yes);
                        self.logs.push("✔ Orphans purged.".to_string());
                    }
                    _ => {}
                }
            }
            TabItem::PowerThermal => {
                let sel = self.menu_state.selected().unwrap_or(0);
                match sel {
                    0 => {
                        self.logs.push("Switching to Performance profile...".to_string());
                        let _ = crate::modules::power::set_performance_profile(dry_run);
                    }
                    1 => {
                        self.logs.push("Switching to Battery / Power-save profile...".to_string());
                        let _ = crate::modules::power::set_battery_profile(dry_run);
                    }
                    2 => {
                        self.logs.push("Optimizing dGPU, GNOME GPU-polling & peripheral power...".to_string());
                        let _ = crate::modules::power::optimize_gpu_and_peripherals(dry_run);
                    }
                    _ => {}
                }
            }
            TabItem::Observability => {
                let sel = self.menu_state.selected().unwrap_or(0);
                match sel {
                    0 => {
                        self.logs.push("Scanning heavy files (min 50MB)...".to_string());
                        let _ = crate::modules::scan::scan_heavy_files(
                            std::path::Path::new("."),
                            50 * 1024 * 1024,
                            10,
                            true,
                            &self.ollama,
                            &self.model,
                        );
                    }
                    1 => {
                        self.logs.push("Auditing active network sockets...".to_string());
                        let _ = crate::modules::scan::scan_active_sockets(false, false);
                    }
                    _ => {}
                }
            }
            TabItem::Workflows => {
                let sel = self.menu_state.selected().unwrap_or(0);
                match sel {
                    0 => {
                        self.logs.push("Executing 'prepare-crunch' agentic workflow...".to_string());
                        let _ = crate::modules::workflows::workflow_prepare_crunch(
                            "/home/clever/aidbio/ds",
                            dry_run,
                            auto_yes,
                            &self.ollama,
                            &self.model,
                        );
                    }
                    1 => {
                        self.logs.push("Executing 'maintenance' weekly upkeep workflow...".to_string());
                        let _ = crate::modules::workflows::workflow_maintenance(
                            Some("bioclean_maintenance_report.md"),
                            dry_run,
                            auto_yes,
                            &self.ollama,
                            &self.model,
                            &self.config,
                        );
                    }
                    _ => {}
                }
            }
        }
    }
}

/// Runs the given action outside the TUI's alternate screen/raw mode, since
/// module functions may need an interactive sudo prompt (e.g. `apt-get`) or
/// print output the ratatui diff-renderer doesn't know about. The popup
/// confirmation itself stays fully in-TUI; only actual execution drops out.
fn run_outside_tui<B: Backend>(terminal: &mut Terminal<B>, app: &mut App, dry_run: bool, auto_yes: bool) -> Result<()> {
    crossterm::terminal::disable_raw_mode()?;
    crossterm::execute!(std::io::stdout(), crossterm::terminal::LeaveAlternateScreen)?;

    app.execute_selected(dry_run, auto_yes);

    println!("\nPress Enter to return to TUI dashboard...");
    let mut buf = String::new();
    let _ = std::io::stdin().read_line(&mut buf);

    crossterm::terminal::enable_raw_mode()?;
    crossterm::execute!(std::io::stdout(), crossterm::terminal::EnterAlternateScreen)?;
    terminal.clear()?;

    // Force an immediate telemetry refresh so the panel reflects the just-run
    // action instead of showing stale pre-action numbers for up to 10s.
    app.telemetry.last_refresh = Instant::now() - app.telemetry_refresh_interval;

    Ok(())
}

pub fn run_tui<B: Backend>(terminal: &mut Terminal<B>, mut app: App) -> Result<()> {
    let tick_rate = Duration::from_millis(250);

    loop {
        terminal.draw(|f| ui(f, &mut app))?;
        app.maybe_refresh_telemetry();

        let timeout = tick_rate
            .checked_sub(app.last_tick.elapsed())
            .unwrap_or_else(|| Duration::from_secs(0));

        if crossterm::event::poll(timeout)? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    if app.pending_confirmation.is_some() {
                        // While a popup is open, only y/n/d/Esc are handled — navigation is frozen.
                        match key.code {
                            KeyCode::Char('y') | KeyCode::Char('Y') | KeyCode::Enter => {
                                app.pending_confirmation = None;
                                run_outside_tui(terminal, &mut app, false, true)?;
                            }
                            KeyCode::Char('d') | KeyCode::Char('D') => {
                                app.pending_confirmation = None;
                                run_outside_tui(terminal, &mut app, true, true)?;
                            }
                            KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                                app.pending_confirmation = None;
                            }
                            _ => {}
                        }
                    } else {
                        match key.code {
                            KeyCode::Char('q') | KeyCode::Esc => {
                                app.should_quit = true;
                            }
                            KeyCode::Tab => {
                                app.next_tab();
                                app.menu_state.select(Some(0));
                            }
                            KeyCode::BackTab => {
                                app.prev_tab();
                                app.menu_state.select(Some(0));
                            }
                            KeyCode::Down | KeyCode::Char('j') => {
                                let max_items = match app.current_tab {
                                    TabItem::Dashboard => 1,
                                    TabItem::SpaceRecovery => 4,
                                    TabItem::PowerThermal => 3,
                                    TabItem::Observability => 2,
                                    TabItem::Workflows => 2,
                                };
                                app.next_item(max_items);
                            }
                            KeyCode::Up | KeyCode::Char('k') => {
                                let max_items = match app.current_tab {
                                    TabItem::Dashboard => 1,
                                    TabItem::SpaceRecovery => 4,
                                    TabItem::PowerThermal => 3,
                                    TabItem::Observability => 2,
                                    TabItem::Workflows => 2,
                                };
                                app.prev_item(max_items);
                            }
                            KeyCode::Enter => {
                                if let Some(pending) = app.build_pending_confirmation() {
                                    app.pending_confirmation = Some(pending);
                                } else {
                                    // Read-only action (diagnose, observability scans): no confirmation needed.
                                    run_outside_tui(terminal, &mut app, false, true)?;
                                }
                            }
                            _ => {}
                        }
                    }
                }
            }
        }

        if app.last_tick.elapsed() >= tick_rate {
            app.last_tick = Instant::now();
        }

        if app.should_quit {
            break;
        }
    }

    Ok(())
}

/// Computes a centered rectangle of `percent_x` x `percent_y` within `area`,
/// for popup placement.
fn centered_rect(percent_x: u16, percent_y: u16, area: Rect) -> Rect {
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(area);
    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(vertical[1])[1]
}

fn ui(f: &mut Frame, app: &mut App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Header Banner & Tabs
            Constraint::Min(12),   // Main Area (Sidebar + Detail)
            Constraint::Length(3), // Footer Keybindings
        ])
        .split(f.area());

    // 1. Header with Tabs
    let titles: Vec<Line> = TabItem::titles()
        .iter()
        .map(|t| Line::from(Span::styled(*t, Style::default().fg(Color::Yellow))))
        .collect();

    let tabs = Tabs::new(titles)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" 🧬 AidBio bioclean — Agentic Linux System Orchestrator ")
                .title_style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
        )
        .select(app.current_tab as usize)
        .style(Style::default().fg(Color::White))
        .highlight_style(
            Style::default()
                .fg(Color::Black)
                .bg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        );
    f.render_widget(tabs, chunks[0]);

    // 2. Main Area Split (Left: Actions/Menu, Right: Live Telemetry & Log)
    let main_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(45), Constraint::Percentage(55)])
        .split(chunks[1]);

    // Left Panel: Options according to active tab
    match app.current_tab {
        TabItem::Dashboard => {
            let info_text = vec![
                Line::from(vec![
                    Span::styled("🤖 AI Engine Model: ", Style::default().add_modifier(Modifier::BOLD)),
                    Span::styled(&app.model, Style::default().fg(Color::Yellow)),
                ]),
                Line::from(vec![
                    Span::styled("⚡ Ollama Status  : ", Style::default().add_modifier(Modifier::BOLD)),
                    if app.ollama.is_online() {
                        Span::styled("ONLINE (Local API Connected)", Style::default().fg(Color::Green))
                    } else {
                        Span::styled("OFFLINE (Using Rule-based Heuristics)", Style::default().fg(Color::Red))
                    },
                ]),
                Line::from(""),
                Line::from(Span::styled("Action Hub:", Style::default().add_modifier(Modifier::UNDERLINED))),
                Line::from("  [Enter] 🩺 Run Complete AI System Health Diagnostic"),
                Line::from(""),
                Line::from("Press Tab to explore space recovery, power, observability, and workflows."),
            ];
            let block = Paragraph::new(info_text)
                .block(Block::default().borders(Borders::ALL).title(" 📊 System Overview "))
                .wrap(Wrap { trim: true });
            f.render_widget(block, main_chunks[0]);
        }
        TabItem::SpaceRecovery => {
            let items = vec![
                ListItem::new("🧹 1. Clean Package & Layer Caches (APT, Pip, UV, Conda, Docker)"),
                ListItem::new("📜 2. Vacuum Systemd Journal Logs (older than 7 days)"),
                ListItem::new("🗂 3. Safely Clean Stale /tmp and /var/tmp Files (>= 48h)"),
                ListItem::new("📦 4. Remove Unneeded Package Dependencies (autoremove)"),
            ];
            let list = List::new(items)
                .block(Block::default().borders(Borders::ALL).title(" 🧹 Space Recovery Targets "))
                .highlight_style(
                    Style::default()
                        .fg(Color::Black)
                        .bg(Color::Green)
                        .add_modifier(Modifier::BOLD),
                )
                .highlight_symbol("▶ ");
            f.render_stateful_widget(list, main_chunks[0], &mut app.menu_state);
        }
        TabItem::PowerThermal => {
            let items = vec![
                ListItem::new("⚡ 1. Performance Profile (High CPU Governor, Max I/O Priority)"),
                ListItem::new("🔋 2. Battery Profile (Powersave Governor, Low Peripheral Drain)"),
                ListItem::new("🖥️ 3. Optimize dGPU, GNOME Polling & Peripherals (D3cold, TLP, Powertop)"),
            ];
            let list = List::new(items)
                .block(Block::default().borders(Borders::ALL).title(" ⚡ Power & Thermal Governor "))
                .highlight_style(
                    Style::default()
                        .fg(Color::Black)
                        .bg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                )
                .highlight_symbol("▶ ");
            f.render_stateful_widget(list, main_chunks[0], &mut app.menu_state);
        }
        TabItem::Observability => {
            let items = vec![
                ListItem::new("🔍 1. Scan Heavy Directories & Datasets with AI Explanations"),
                ListItem::new("🌐 2. Audit Active Network Sockets & Zombie Processes"),
            ];
            let list = List::new(items)
                .block(Block::default().borders(Borders::ALL).title(" 🔍 Deep Observability "))
                .highlight_style(
                    Style::default()
                        .fg(Color::Black)
                        .bg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                )
                .highlight_symbol("▶ ");
            f.render_stateful_widget(list, main_chunks[0], &mut app.menu_state);
        }
        TabItem::Workflows => {
            let items = vec![
                ListItem::new("🚀 1. prepare-crunch (Pre-compute Checklist, Performance, Free Space)"),
                ListItem::new("🛠 2. maintenance (Diagnose, Clean Caches, SSD TRIM, Markdown Report)"),
            ];
            let list = List::new(items)
                .block(Block::default().borders(Borders::ALL).title(" 🤖 Agentic Workflows "))
                .highlight_style(
                    Style::default()
                        .fg(Color::Black)
                        .bg(Color::Magenta)
                        .add_modifier(Modifier::BOLD),
                )
                .highlight_symbol("▶ ");
            f.render_stateful_widget(list, main_chunks[0], &mut app.menu_state);
        }
    }

    // Right Panel: Telemetry & Log Output
    let mem = read_meminfo();
    let zones = read_thermal_zones();
    let max_temp = zones.iter().map(|z| z.temp_celsius).fold(0.0f32, f32::max);
    let cpus = read_cpu_governors();
    let gov_str = cpus.first().map(|c| c.current_governor.as_str()).unwrap_or("powersave");

    let mut tele_lines = vec![
        Line::from(vec![
            Span::styled("RAM Total / Available : ", Style::default().add_modifier(Modifier::BOLD)),
            Span::styled(
                format!("{} / {}", format_bytes(mem.total_bytes), format_bytes(mem.available_bytes)),
                Style::default().fg(Color::Yellow),
            ),
        ]),
        Line::from(vec![
            Span::styled("CPU Scaling Governor  : ", Style::default().add_modifier(Modifier::BOLD)),
            Span::styled(gov_str, Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
        ]),
        Line::from(vec![
            Span::styled("Peak Thermal Sensor   : ", Style::default().add_modifier(Modifier::BOLD)),
            Span::styled(format!("{:.1}°C", max_temp), Style::default().fg(if max_temp > 75.0 { Color::Red } else { Color::Green })),
        ]),
        Line::from(vec![
            Span::styled("Power Draw            : ", Style::default().add_modifier(Modifier::BOLD)),
            match &app.telemetry.power {
                PowerDisplay::Reading(r) => {
                    let src = match r.source {
                        PowerSource::Battery => "Battery",
                        PowerSource::Rapl => "RAPL",
                    };
                    match r.voltage {
                        Some(v) => Span::styled(
                            format!("{:.1} W, {:.2} V ({})", r.watts, v, src),
                            Style::default().fg(Color::Yellow),
                        ),
                        None => Span::styled(format!("{:.1} W ({})", r.watts, src), Style::default().fg(Color::Yellow)),
                    }
                }
                PowerDisplay::Sampling => Span::styled("sampling…", Style::default().fg(Color::DarkGray)),
                PowerDisplay::Unavailable => Span::styled("unavailable on this system", Style::default().fg(Color::DarkGray)),
                PowerDisplay::PermissionDenied => Span::styled("sensor present, needs root (run as sudo to enable)", Style::default().fg(Color::DarkGray)),
            },
        ]),
    ];

    if app.telemetry.disks.is_empty() {
        tele_lines.push(Line::from(vec![
            Span::styled("Internal Disks        : ", Style::default().add_modifier(Modifier::BOLD)),
            Span::styled("none detected", Style::default().fg(Color::DarkGray)),
        ]));
    } else {
        let mut sorted_disks = app.telemetry.disks.clone();
        sorted_disks.sort_by(|a, b| b.used_percent().cmp(&a.used_percent()));
        for d in sorted_disks.iter().take(3) {
            tele_lines.push(Line::from(vec![
                Span::styled(format!("Disk {} ({}) : ", d.mount_point, d.kind), Style::default().add_modifier(Modifier::BOLD)),
                Span::styled(
                    format!("{} / {} used ({}%)", format_bytes(d.used_bytes), format_bytes(d.total_bytes), d.used_percent()),
                    Style::default().fg(if d.used_percent() > 90 { Color::Red } else { Color::Cyan }),
                ),
            ]));
        }
        if sorted_disks.len() > 3 {
            tele_lines.push(Line::from(Span::styled(
                format!("  +{} more (see `bioclean diagnose`)", sorted_disks.len() - 3),
                Style::default().fg(Color::DarkGray),
            )));
        }
    }

    tele_lines.push(Line::from(vec![
        Span::styled("Reclaimable Cache     : ", Style::default().add_modifier(Modifier::BOLD)),
        Span::styled(format_bytes(app.telemetry.reclaimable.total_bytes), Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
    ]));
    tele_lines.push(Line::from(vec![
        Span::styled("Journal Log Size      : ", Style::default().add_modifier(Modifier::BOLD)),
        Span::styled(
            app.telemetry.journal_bytes.map(format_bytes).unwrap_or_else(|| "unknown".to_string()),
            Style::default().fg(Color::Cyan),
        ),
    ]));

    tele_lines.push(Line::from(""));
    tele_lines.push(Line::from(Span::styled("📋 Execution Logs:", Style::default().add_modifier(Modifier::UNDERLINED))));

    for log in app.logs.iter().rev().take(6) {
        tele_lines.push(Line::from(Span::styled(format!("• {}", log), Style::default().fg(Color::DarkGray))));
    }

    let right_block = Paragraph::new(tele_lines)
        .block(Block::default().borders(Borders::ALL).title(" 🖥️ Live Telemetry & Log "))
        .wrap(Wrap { trim: true });
    f.render_widget(right_block, main_chunks[1]);

    // 3. Footer — legend changes while a confirmation popup is open.
    let footer_text = if app.pending_confirmation.is_some() {
        Line::from(vec![
            Span::styled("[y/Enter] ", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
            Span::raw("Confirm  "),
            Span::styled("[d] ", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
            Span::raw("Dry Run  "),
            Span::styled("[n/Esc] ", Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)),
            Span::raw("Cancel"),
        ])
    } else {
        Line::from(vec![
            Span::styled("[Tab] ", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
            Span::raw("Next Section  "),
            Span::styled("[↑/↓] ", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
            Span::raw("Select Target  "),
            Span::styled("[Enter] ", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
            Span::raw("Execute Action  "),
            Span::styled("[q/Esc] ", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
            Span::raw("Exit TUI"),
        ])
    };
    let footer = Paragraph::new(footer_text)
        .block(Block::default().borders(Borders::ALL).title(" Controls "))
        .style(Style::default().fg(Color::White));
    f.render_widget(footer, chunks[2]);

    // 4. Confirmation popup, rendered last so it overlays everything else.
    if let Some(pending) = &app.pending_confirmation {
        let area = centered_rect(60, 30, f.area());
        f.render_widget(Clear, area);
        let default_hint = if pending.preview.risk.default_yes() { "[Y/n]" } else { "[y/N]" };
        let text = vec![
            Line::from(Span::styled(&pending.preview.action, Style::default().add_modifier(Modifier::BOLD))),
            Line::from(""),
            Line::from(format!("Current: {}", pending.preview.current_state)),
            Line::from(format!("After:   {}", pending.preview.future_state)),
            Line::from(""),
            Line::from(Span::styled(
                format!("Confirm {}  ·  d = dry-run  ·  Esc = cancel", default_hint),
                Style::default().add_modifier(Modifier::BOLD),
            )),
        ];
        let border_color = if pending.preview.risk == RiskLevel::Sensitive { Color::Red } else { Color::Green };
        let popup = Paragraph::new(text)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(border_color))
                    .title(" Confirm Action "),
            )
            .wrap(Wrap { trim: true });
        f.render_widget(popup, area);
    }
}
