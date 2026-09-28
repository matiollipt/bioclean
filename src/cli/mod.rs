use clap::{Args, Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(
    name = "bioclean",
    author = "Cleverson Matiolli, PhD and Gemini",
    version = "2.0.0",
    about = "An Agentic System Orchestrator for High-Performance Linux Environments",
    long_about = "bioclean is a high-performance Linux workstation orchestrator tailored for computational biology, bioinformatics, and ML engineering. It reclaims disk space, manages CPU and thermal governors, discovers storage bloat with AI summaries, generates system health reports via local Ollama LLMs, and executes automated agentic workflows."
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,

    /// Override the Ollama model for AI diagnostics and summaries
    #[arg(long, global = true)]
    pub model: Option<String>,

    /// Launch full-screen interactive Terminal User Interface (TUI)
    #[arg(long)]
    pub tui: bool,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Inspect and modify configuration parameters or launch configuration wizard
    Config(ConfigArgs),

    /// Space recovery and garbage collection
    Free(FreeArgs),

    /// Energy, CPU governors, and thermal management
    Power(PowerArgs),

    /// Deep observability for heavy files and active network sockets
    Scan(ScanArgs),

    /// Run AI-powered system health diagnostics via local Ollama
    Diagnose(DiagnoseArgs),

    /// Multi-step agentic workflows tailored for bioinformatics
    Workflow(WorkflowArgs),

    /// External HDD discovery and dataset migration
    Hdd(HddArgs),

    /// Transaction session logging and rollback engine
    History(HistoryArgs),
}

#[derive(Args, Debug)]
pub struct ConfigArgs {
    #[command(subcommand)]
    pub action: Option<ConfigAction>,

    /// Set a configuration parameter directly: --param <SETTING> <VALUE>
    #[arg(long, num_args = 2, value_names = ["SETTING", "VALUE"])]
    pub param: Option<Vec<String>>,

    /// List all current configuration settings
    #[arg(short, long)]
    pub list: bool,

    /// Get the value of a single configuration setting
    #[arg(short, long)]
    pub get: Option<String>,

    /// Reset configuration to factory defaults
    #[arg(long)]
    pub reset: bool,
}

#[derive(Subcommand, Debug)]
pub enum ConfigAction {
    /// Launch interactive configuration wizard
    Wizard,
    /// List all configuration parameters and their descriptions
    List,
    /// Get a specific configuration parameter
    Get { key: String },
    /// Set a specific configuration parameter
    Set { key: String, value: String },
    /// Reset configuration to factory defaults
    Reset,
}

#[derive(Args, Debug)]
pub struct FreeArgs {
    #[command(subcommand)]
    pub action: Option<FreeAction>,

    /// Simulate actions without deleting any files
    #[arg(long, global = true)]
    pub dry_run: bool,

    /// Auto-confirm all prompts
    #[arg(short = 'y', long, global = true)]
    pub yes: bool,
}

#[derive(Subcommand, Debug)]
pub enum FreeAction {
    /// Clean APT, Pip, UV, Conda, and Docker layer caches
    Cache {
        #[arg(long)]
        dry_run: bool,
        #[arg(short = 'y', long)]
        yes: bool,
    },
    /// Vacuum journalctl system logs older than threshold
    Logs {
        /// Number of days of logs to retain
        #[arg(long, default_value_t = 7)]
        days: u32,
        #[arg(long)]
        dry_run: bool,
        #[arg(short = 'y', long)]
        yes: bool,
    },
    /// Safely clean /tmp and /var/tmp files respecting atime/mtime
    Tmp {
        /// Minimum file age in hours to be eligible for deletion
        #[arg(long, default_value_t = 48)]
        min_age_hours: u32,
        #[arg(long)]
        dry_run: bool,
        #[arg(short = 'y', long)]
        yes: bool,
    },
    /// Identify and remove unneeded package dependencies (autoremove)
    Orphans {
        #[arg(long)]
        dry_run: bool,
        #[arg(short = 'y', long)]
        yes: bool,
    },
}

#[derive(Args, Debug)]
pub struct PowerArgs {
    #[command(subcommand)]
    pub action: Option<PowerAction>,

    /// Output metrics in JSON format
    #[arg(long)]
    pub json: bool,
}

#[derive(Subcommand, Debug)]
pub enum PowerAction {
    /// Adjust settings for maximum battery longevity (powersave governor)
    Battery {
        #[arg(long)]
        dry_run: bool,
    },
    /// Prepare system for heavy computation (performance governor, max I/O priority)
    Performance {
        #[arg(long)]
        dry_run: bool,
    },
    /// Monitor thermal zones, thermald status, and throttling limits
    Thermal {
        #[arg(long)]
        json: bool,
    },
    /// Diagnose and fix idle battery drain: stop background GPU-polling
    /// loops, enable dGPU D3cold, retire nvidia-persistenced, bring up TLP
    /// and powertop auto-tune
    Optimize {
        #[arg(long)]
        dry_run: bool,
    },
}

#[derive(Args, Debug)]
pub struct ScanArgs {
    #[command(subcommand)]
    pub action: Option<ScanAction>,
}

#[derive(Subcommand, Debug)]
pub enum ScanAction {
    /// Parallel high-speed inspection of large directories with AI summaries
    Heavy {
        /// Path to scan (defaults to current directory)
        #[arg(short, long, default_value = ".")]
        path: String,

        /// Minimum size threshold (e.g. 50M, 1G, 500K)
        #[arg(short, long, default_value = "50M")]
        min_size: String,

        /// Maximum number of top heavy items to display
        #[arg(short, long, default_value_t = 15)]
        limit: usize,

        /// Disable AI summary generation
        #[arg(long)]
        no_ai: bool,
    },
    /// Audit active network sockets, open ports, and zombie processes
    Sockets {
        /// Show listening ports only
        #[arg(short, long)]
        listen: bool,

        /// Output socket table as JSON
        #[arg(long)]
        json: bool,
    },
}

#[derive(Args, Debug)]
pub struct DiagnoseArgs {
    /// Save markdown health report to specified file
    #[arg(short, long)]
    pub output: Option<String>,

    /// Output raw harvested diagnostic metrics as JSON
    #[arg(long)]
    pub json: bool,
}

#[derive(Args, Debug)]
pub struct WorkflowArgs {
    #[command(subcommand)]
    pub action: WorkflowAction,
}

#[derive(Subcommand, Debug)]
pub enum WorkflowAction {
    /// 5-step preparation for multi-day compute simulation or alignment
    PrepareCrunch {
        /// Path to scratch / working data directory
        #[arg(short, long, default_value = "/home/clever/aidbio/ds")]
        scratch: String,

        /// Simulate workflow steps without executing changes
        #[arg(long)]
        dry_run: bool,

        /// Auto-confirm all prompts
        #[arg(short = 'y', long)]
        yes: bool,
    },
    /// 4-step weekly maintenance (diagnose, clean caches/logs/orphans, fstrim)
    Maintenance {
        /// Output file path for maintenance markdown report
        #[arg(short, long, default_value = "bioclean_maintenance_report.md")]
        output: Option<String>,

        /// Simulate workflow steps without executing changes
        #[arg(long)]
        dry_run: bool,

        /// Auto-confirm all prompts
        #[arg(short = 'y', long)]
        yes: bool,
    },
}

#[derive(Args, Debug)]
pub struct HddArgs {
    #[command(subcommand)]
    pub action: Option<HddAction>,
}

#[derive(Subcommand, Debug)]
pub enum HddAction {
    /// Scan attached external drives under /media, /mnt, /run/media
    Scan {
        #[arg(long)]
        json: bool,
    },
    /// Migrate bio datasets to external HDD and replace with symlinks
    Migrate {
        /// Specific external HDD mount path
        #[arg(short, long)]
        target_hdd: Option<String>,

        /// Simulate the migration plan without moving any files
        #[arg(long)]
        dry_run: bool,

        /// Auto-confirm all prompts
        #[arg(short = 'y', long)]
        yes: bool,
    },
    /// Reconfigure NCBI SRA Toolkit cache directory to external HDD
    ReconfigureSra {
        #[arg(short, long)]
        target_hdd: String,
    },
}

#[derive(Args, Debug)]
pub struct HistoryArgs {
    #[command(subcommand)]
    pub action: Option<HistoryAction>,
}

#[derive(Subcommand, Debug)]
pub enum HistoryAction {
    /// Display session transaction history log
    List,

    /// Revert / undo the latest migration session
    Undo {
        #[arg(short = 'y', long)]
        yes: bool,
    },
}
