# AGENTS.md — AidBio Bioclean

This document provides operational context, architectural contracts, and safety invariants for AI agents and automated assistants developing or extending **`bioclean`**.

---

## 1. Purpose and Mission

`bioclean` is a native, high-performance Linux workstation and cluster node orchestrator engineered in **Rust** for computational biology, bioinformatics, and scientific ML engineering at AidBio.

It follows the strict **Observe $\rightarrow$ Analyze $\rightarrow$ Propose $\rightarrow$ Execute** paradigm:
Every destructive action or significant hardware transition is preceded by verified diagnostic telemetry, local AI synthesis (with deterministic heuristic fallbacks), and a mandatory safety interlock.

### Core Architecture Principles
- **Rust Native**: High performance, zero runtime dependencies, static memory safety.
- **Safety First for Bio Data**: Biological sequencing data (`.fastq.gz`, `.bam`, `.cram`, `.sra`, `.vcf`, `.h5ad`, model checkpoints) must never be accidentally deleted or truncated.
- **Strict Data Integrity & Rollback Pairing**: Every diagnostic health report is cryptographically paired with a structured JSON telemetry snapshot, an execution audit trail, and a rollback manifest under a unified session directory (`~/.local/share/bioclean/history/<run_id>/`).
- **High-Fidelity Output**: Terminal reports render via native tools (`glow` or `batcat`) with clean ANSI fallbacks.
- **Configurability**: Users can inspect and set parameters via CLI (`bioclean config --param <KEY> <VALUE>`) or launch the interactive configuration wizard (`bioclean config`).

---

## 2. Environment & Toolchain Constraints

- **Language & Edition:** Rust 2021 edition (`rustc`, `cargo` via `rustup` in `~/.cargo/bin`).
- **OS & Kernel Target:** Ubuntu / Debian Linux on x86_64, Linux kernel 6.x+.
- **Installed External Integrations:**
  - `/usr/local/bin/ollama`: Local LLM server running on `http://localhost:11434`.
  - `/snap/bin/glow`: Markdown CLI pager and renderer.
  - `/usr/bin/batcat`: Syntax-highlighting pager.
  - `/usr/bin/docker`: Container runtime for optional sandboxing / testing.
- **Binary Install Target:** `~/.local/bin/bioclean`.

---

## 3. Validation Commands

Always run the full validation suite before claiming any task or refactoring is complete:

```bash
# 1. Compilation and type-checking
cargo check

# 2. Run all unit and integration tests
cargo test

# 3. Build optimized release binary
cargo build --release

# 4. Install updated release binary
cp target/release/bioclean ~/.local/bin/bioclean

# 5. Smoke test key CLI subcommands
bioclean config --list
bioclean diagnose --json
bioclean free cache --dry-run
bioclean power thermal --json
bioclean history
```

---

## 4. Repository Map & Module Boundaries

```text
bioclean/
├── Cargo.toml                  # Package manifest and dependencies (clap, ratatui, reqwest, sysinfo)
├── README.md                   # User-facing documentation
├── AGENTS.md                   # AI agent operational guidelines and architectural contracts
├── install.sh                  # Installer script: builds release binary and configures symlinks
├── src/
│   ├── main.rs                 # CLI routing, legacy command interception, and TUI entrypoint
│   ├── lib.rs                  # Library crate root exposing all submodules
│   ├── config.rs               # Config struct, TOML persistence, validation, and interactive wizard
│   ├── cli/
│   │   └── mod.rs              # Clap Parser hierarchy (commands: config, free, power, scan, etc.)
│   ├── ai/
│   │   ├── client.rs           # OllamaClient (generate API with OllamaOptions, model tags)
│   │   ├── prompts.rs          # System prompts with HPC context and biological safety rules
│   │   ├── fallback.rs         # Deterministic heuristic fallback engine for offline Ollama
│   │   └── safety_interlock.rs # AI confirmation prompt before destructive actions
│   ├── modules/
│   │   ├── diagnose.rs         # Procfs/sysfs telemetry snapshot, JSON context, paired report generator
│   │   ├── free.rs             # Cache cleanup (APT, pip, UV, conda, docker layers), logs, tmp, orphans
│   │   ├── power.rs            # CPU governors (performance/powersave), thermal monitoring, and the dGPU/GNOME-polling/TLP/powertop `optimize` pass
│   │   ├── scan.rs             # Heavy file inspection (rayon parallel) and active socket auditing
│   │   ├── workflows.rs        # Multi-step agentic workflows (prepare-crunch, maintenance)
│   │   ├── hdd.rs              # External drive auto-discovery, bio dataset migration, symlink engine
│   │   └── history.rs          # Session records, paired directories, audit logs, and rollback engine
│   ├── tui/
│   │   ├── mod.rs              # TUI module declaration
│   │   └── app.rs              # Ratatui full-screen interactive dashboard
│   └── utils/
│       ├── formatting.rs       # Byte size and duration formatting utilities
│       ├── procfs.rs           # Direct /proc and /sys parser (loadavg, meminfo, thermal, sockets)
│       ├── report.rs           # High-fidelity report renderer (Glow, Batcat, ANSI) and storage
│       └── system.rs           # Process execution, root detection, fstrim, prompt confirmations
└── tests/
    ├── test_ai_fallback.rs     # Tests for heuristic fallback scoring and safety warnings
    ├── test_config.rs          # Tests for configuration defaults, parameter setter/getter, options
    ├── test_formatting.rs      # Tests for byte size formatting and parsing
    ├── test_history.rs         # Tests for history session lifecycle
    └── test_report_pairing.rs  # Tests for paired session directories, audit logging, JSON snapshot
```

---

## 5. Critical Invariants & Rules for Future Iterations

### 5.1 Biological Data Preservation Invariants
- **NEVER** recommend or execute deletion of files matching bioinformatics extensions:
  `.fastq`, `.fastq.gz`, `.fq`, `.fq.gz`, `.bam`, `.cram`, `.sam`, `.sra`, `.vcf`, `.vcf.gz`, `.bcf`, `.h5ad`, `.loom`, `.rds`, `.pth`, `.pt`, `.ckpt`.
- When disk space is low, the orchestrator must recommend **offloading to external HDDs via `bioclean hdd migrate`** rather than deletion.
- Directories inside `~/aidbio/` are production code/data repositories and must never be treated as expendable cache.

### 5.2 Concurrency & Pipeline Safety
- Before proposing any process termination or service restarts, inspect running processes and active sockets.
- Active bioinformatics workflows (Nextflow, Snakemake, CellRanger, Bowtie, BWA, Python simulations) must never be interrupted.

### 5.3 Deterministic Fallbacks
- `bioclean` must remain fully operational when Ollama is offline or times out.
- Any new AI prompt or evaluation added to the codebase **must** be accompanied by a deterministic rule-based fallback in `src/ai/fallback.rs`.

### 5.4 High-Fidelity Report Formatting
- Reports intended for terminal viewing must pass through `crate::utils::report::display_report()`.
- It checks `io::stdout().is_terminal()`: if output is piped or redirected (e.g. `bioclean diagnose > out.txt`), raw markdown is printed without pager control sequences.
- If connected to a TTY, it honors the user's `report_formatter` setting (`glow`, `batcat`, `auto`, `terminal`).

### 5.5 Strict Data Integrity & Pairing
- Every diagnostic or cleanup run generates a unique session correlation ID: `HistoryManager::generate_run_id(...)`.
- Diagnostic reports, raw JSON telemetry snapshots, and rollback manifests must be saved together in `~/.local/share/bioclean/history/<run_id>/`.
- The session record must be registered in `HistoryManager` with cross-references to all paired files.

### 5.6 CLI Command Stream Rationalization
- Do **NOT** add top-level legacy flags back to `Cli` in `src/cli/mod.rs`.
- Root CLI subcommands must remain clean, modular, and domain-focused:
  `config`, `diagnose`, `free`, `power`, `scan`, `workflow`, `hdd`, `history`.
- Legacy invocation support for the old `clean-disk` name and its short flags (`-a`, `-c`, `-m`, `-u`, `-l`) has been removed; `bioclean` is the sole supported binary name and subcommands are the only supported interface.

### 5.7 Configuration Module Discipline
- New persistent settings must be added to `Config` in `src/config.rs` with `#[serde(default = "...")]` to guarantee backward compatibility with existing `config.toml` files.
- Every setting must be exposed in `set_param()`, `get_param()`, `list_param_meta()`, and the interactive `run_wizard()`.

---

## 6. Roadmap & Next Iterations

1. **Synthetic Staging Sandbox (`bioclean sandbox`)**:
   - Implement an optional containerized mock environment (`bioclean sandbox test-clean`) using Docker to simulate destructive workflows on dummy files before running on multi-terabyte production arrays.
2. **Snakemake & Nextflow Lockfile Auditing**:
   - Add explicit lockfile and `.nextflow.log` detection to `scan` and `diagnose` to provide automated pipeline run-state detection.
3. **Automated Cron / Systemd Timer Generator**:
   - Provide `bioclean config install-timer` to schedule weekly `bioclean workflow maintenance` with desktop notifications on degraded health scores.
4. **HTML / PDF Health Report Export**:
   - Add optional HTML report rendering with embedded SVG charts for executive distribution.
