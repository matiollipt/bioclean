# `bioclean` — Agentic System Orchestrator for High-Performance Linux

**Branding:** AidBio AI  
**Authors:** Cleverson Matiolli, PhD and Gemini  
**Version:** 2.0.0  
**Stack:** Rust, Ratatui, Clap, Ollama  

---

## 1. Overview & Philosophy

`bioclean` is a native, high-performance Linux workstation and cluster node orchestrator engineered for computational biology, bioinformatics, and scientific ML engineering.

It follows the **Observe $\rightarrow$ Analyze $\rightarrow$ Propose $\rightarrow$ Execute** pattern:
Every destructive action is preceded by an AI-verified diagnostic and safety interlock to ensure that cleaning a cache or removing an environment doesn't break a running pipeline or delete vital datasets.

```
                  ┌───────────────────────────────┐
                  │       bioclean (Rust)         │
                  └───────────────┬───────────────┘
                                  │
      ┌──────────────┬────────────┼────────────┬──────────────┐
      ▼              ▼            ▼            ▼              ▼
┌───────────┐  ┌───────────┐ ┌───────────┐ ┌───────────┐ ┌───────────┐
│   free    │  │   power   │ │   scan    │ │ diagnose  │ │ workflow  │
│  (caches, │  │ (governor,│ │  (heavy,  │ │  (Ollama  │ │ (prepare-  │
│ logs, tmp,│  │ thermals, │ │  sockets, │ │  Health   │ │  crunch,  │
│  orphans) │  │  profiles)│ │  zombies) │ │  Reports) │ │  upkeep)  │
└───────────┘  └───────────┘ └───────────┘ └───────────┘ └───────────┘
```

---

## 2. Key Capabilities

### 🧹 `bioclean free` (Garbage Collection & Space Recovery)
* **`bioclean free cache`**: Purges APT caches, Python Pip wheels, UV caches, Conda/Mamba package tarballs, and Docker layer build orphans.
* **`bioclean free logs --days 7`**: Vacuums `journalctl` logs older than the specified retention window.
* **`bioclean free tmp --min-age-hours 48`**: Safely clears stale temporary files in `/tmp` and `/var/tmp`, checking access timestamps (`atime`) so active jobs are uninterrupted.
* **`bioclean free orphans`**: Finds and removes unneeded package dependencies (`apt-get autoremove`).
* **`bioclean free`**: Running without subcommands launches an interactive space recovery selection menu.

### ⚡ `bioclean power` (Energy & Thermal Management)
* **`bioclean power battery`**: Sets CPU governor to `powersave`, lowers frequency scaling, and minimizes peripheral power draw.
* **`bioclean power performance`**: Configures CPU governor to `performance`, sets energy performance preference to maximum throughput, and boosts I/O priority for genomic sequence alignment or model training.
* **`bioclean power thermal`**: Real-time observability of `/sys/class/thermal` zones, CPU package temperatures, critical trip points, and `thermald` daemon status.

### 🔍 `bioclean scan` (Deep Observability)
* **`bioclean scan heavy --min-size 100M`**: Parallel high-speed directory inspection using Rayon with built-in bioinformatics dataset recognition (`.bam`, `.cram`, `.fastq`, `.fq.gz`, `.sra`, `.vcf`, `.h5ad`, checkpoints). Includes an AI storage summary explaining *why* folders are large.
* **`bioclean scan sockets`**: Audits active TCP/UDP network connections, resolving PIDs, process names, listening daemons, and zombie/hung processes.

### 🩺 `bioclean diagnose` (AI System Health Engine)
* Gathers `dmesg` kernel warnings, `journalctl -p 3` system errors, `df` disk usage, `/proc/meminfo` RAM/Swap, CPU load, and thermal status.
* Synthesizes data using a local **Ollama** LLM (e.g. `qwen2.5-coder:7b`, `qwen3.5:4b`, `gemma4:e2b`) into a human-readable **System Health Report** with health score, detected anomalies, and actionable remediation steps.
* Falls back to a deterministic rule-based heuristic engine when Ollama is offline.

### 🤖 `bioclean workflow` (Intelligent Agentic Layer)
* **`bioclean workflow prepare-crunch`**: 5-step automated preparation for heavy simulations or Nextflow/Snakemake pipelines:
  1. Scans target scratch directory.
  2. Cleans package caches and vacuum logs.
  3. Switches CPU governor to `performance` and verifies cooling margins.
  4. Audits active processes for competing high I/O consumers.
  5. Produces a "Green Light" readiness report.
* **`bioclean workflow maintenance`**: 4-step weekly upkeep:
  1. Runs `diagnose` for kernel and disk errors.
  2. Cleans caches, logs, and package orphans.
  3. Executes `fstrim -av` on SSD mounts.
  4. Generates a Markdown summary report.

### 💾 `bioclean hdd` & `history` (Data Migration & Rollback)
* Auto-discovers attached external HDDs (`/media`, `/mnt`, `/run/media`).
* Offloads raw/processed bioinformatic data (`sra`, `fastq`, `trimmed`) to external storage and creates transparent symbolic links.
* Automatically configures NCBI SRA Toolkit (`~/.ncbi/user-settings.mkfg`).
* Logs all transactions in `~/.aidbio/history/bioclean_sessions.json` and supports single-command rollback with `bioclean history undo`.

---

## 3. Installation & Setup

To install or upgrade `bioclean` on any Linux workstation:

```bash
git clone git@github.com:matiollipt/bioclean.git
cd bioclean
./install.sh
```

The installer will:
1. Auto-detect or install the Rust toolchain via `rustup`.
2. Compile `bioclean` in release mode (`cargo build --release`).
3. Install the optimized binary to `~/.local/bin/bioclean`.
4. Configure shell `PATH` in `~/.bashrc`, `~/.zshrc`, and `~/.profile`.

---

## 4. Usage Examples

```bash
# Launch full Interactive TUI Dashboard
bioclean

# Garbage Collection
bioclean free cache --dry-run
bioclean free cache -y
bioclean free logs --days 7
bioclean free tmp --min-age-hours 48
bioclean free orphans

# Power & Thermal
bioclean power performance
bioclean power battery
bioclean power thermal --json

# Deep Observability & AI Scan
bioclean scan heavy --path /home/clever/aidbio/ds --min-size 100M
bioclean scan sockets --listen

# AI Health Diagnosis
bioclean diagnose
bioclean diagnose --output health_report.md

# Workflows
bioclean workflow prepare-crunch --scratch /home/clever/aidbio/ds
bioclean workflow maintenance --output weekly_report.md

# External HDD Data Offload & Symlinking
bioclean hdd scan
bioclean hdd migrate
bioclean history list
bioclean history undo
```
