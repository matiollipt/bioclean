use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use ratatui::{
    backend::Backend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Tabs, Wrap},
    Frame, Terminal,
};
use std::time::{Duration, Instant};

use crate::ai::client::OllamaClient;
use crate::config::Config;
use crate::modules::free::estimate_reclaimable;
use crate::utils::formatting::format_bytes;
use crate::utils::procfs::{read_cpu_governors, read_meminfo, read_thermal_zones};

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

pub struct App {
    pub current_tab: TabItem,
    pub menu_state: ListState,
    pub logs: Vec<String>,
    pub should_quit: bool,
    pub config: Config,
    pub ollama: OllamaClient,
    pub model: String,
    pub last_tick: Instant,
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
        }
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

    pub fn execute_selected(&mut self) {
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
                        let _ = crate::modules::free::clean_cache(false, false, &self.ollama, &self.model);
                        self.logs.push("✔ Cache cleanup finished.".to_string());
                    }
                    1 => {
                        self.logs.push("Vacuuming journalctl logs...".to_string());
                        let _ = crate::modules::free::clean_logs(7, false, false);
                        self.logs.push("✔ Logs vacuum finished.".to_string());
                    }
                    2 => {
                        self.logs.push("Safely cleaning /tmp files...".to_string());
                        let _ = crate::modules::free::clean_tmp(48, false, false);
                        self.logs.push("✔ Temp cleanup finished.".to_string());
                    }
                    3 => {
                        self.logs.push("Purging unneeded package orphans...".to_string());
                        let _ = crate::modules::free::clean_orphans(false, false);
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
                        let _ = crate::modules::power::set_performance_profile(false);
                    }
                    1 => {
                        self.logs.push("Switching to Battery / Power-save profile...".to_string());
                        let _ = crate::modules::power::set_battery_profile(false);
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
                            false,
                            false,
                            &self.ollama,
                            &self.model,
                        );
                    }
                    1 => {
                        self.logs.push("Executing 'maintenance' weekly upkeep workflow...".to_string());
                        let _ = crate::modules::workflows::workflow_maintenance(
                            Some("bioclean_maintenance_report.md"),
                            false,
                            false,
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

pub fn run_tui<B: Backend>(terminal: &mut Terminal<B>, mut app: App) -> Result<()> {
    let tick_rate = Duration::from_millis(250);

    loop {
        terminal.draw(|f| ui(f, &mut app))?;

        let timeout = tick_rate
            .checked_sub(app.last_tick.elapsed())
            .unwrap_or_else(|| Duration::from_secs(0));

        if crossterm::event::poll(timeout)? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
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
                                TabItem::PowerThermal => 2,
                                TabItem::Observability => 2,
                                TabItem::Workflows => 2,
                            };
                            app.next_item(max_items);
                        }
                        KeyCode::Up | KeyCode::Char('k') => {
                            let max_items = match app.current_tab {
                                TabItem::Dashboard => 1,
                                TabItem::SpaceRecovery => 4,
                                TabItem::PowerThermal => 2,
                                TabItem::Observability => 2,
                                TabItem::Workflows => 2,
                            };
                            app.prev_item(max_items);
                        }
                        KeyCode::Enter => {
                            // Temporarily suspend TUI to allow interactive prompts/output if needed
                            crossterm::terminal::disable_raw_mode()?;
                            crossterm::execute!(std::io::stdout(), crossterm::terminal::LeaveAlternateScreen)?;
                            
                            app.execute_selected();

                            println!("\nPress Enter to return to TUI dashboard...");
                            let mut buf = String::new();
                            let _ = std::io::stdin().read_line(&mut buf);

                            crossterm::terminal::enable_raw_mode()?;
                            crossterm::execute!(std::io::stdout(), crossterm::terminal::EnterAlternateScreen)?;
                            terminal.clear()?;
                        }
                        _ => {}
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
    let reclaimable = estimate_reclaimable();

    let mut tele_lines = vec![
        Line::from(vec![
            Span::styled("RAM Total / Available: ", Style::default().add_modifier(Modifier::BOLD)),
            Span::styled(
                format!("{} / {}", format_bytes(mem.total_bytes), format_bytes(mem.available_bytes)),
                Style::default().fg(Color::Yellow),
            ),
        ]),
        Line::from(vec![
            Span::styled("CPU Scaling Governor : ", Style::default().add_modifier(Modifier::BOLD)),
            Span::styled(gov_str, Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
        ]),
        Line::from(vec![
            Span::styled("Peak Thermal Sensor  : ", Style::default().add_modifier(Modifier::BOLD)),
            Span::styled(format!("{:.1}°C", max_temp), Style::default().fg(if max_temp > 75.0 { Color::Red } else { Color::Green })),
        ]),
        Line::from(vec![
            Span::styled("Estimated Reclaimable: ", Style::default().add_modifier(Modifier::BOLD)),
            Span::styled(format_bytes(reclaimable.total_bytes), Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
        ]),
        Line::from(""),
        Line::from(Span::styled("📋 Execution Logs:", Style::default().add_modifier(Modifier::UNDERLINED))),
    ];

    for log in app.logs.iter().rev().take(6) {
        tele_lines.push(Line::from(Span::styled(format!("• {}", log), Style::default().fg(Color::DarkGray))));
    }

    let right_block = Paragraph::new(tele_lines)
        .block(Block::default().borders(Borders::ALL).title(" 🖥️ Live Telemetry & Log "))
        .wrap(Wrap { trim: true });
    f.render_widget(right_block, main_chunks[1]);

    // 3. Footer
    let footer_text = Line::from(vec![
        Span::styled("[Tab] ", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
        Span::raw("Next Section  "),
        Span::styled("[↑/↓] ", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
        Span::raw("Select Target  "),
        Span::styled("[Enter] ", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
        Span::raw("Execute Action  "),
        Span::styled("[q/Esc] ", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
        Span::raw("Exit TUI"),
    ]);
    let footer = Paragraph::new(footer_text)
        .block(Block::default().borders(Borders::ALL).title(" Controls "))
        .style(Style::default().fg(Color::White));
    f.render_widget(footer, chunks[2]);
}
