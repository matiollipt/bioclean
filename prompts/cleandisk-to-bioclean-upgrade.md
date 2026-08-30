# Project Blueprint: `bioclean`
**Subtitle:** *An Agentic System Orchestrator for High-Performance Linux Environments.*

## 1. Core Philosophy
`bioclean` does not act blindly. It follows the **Observe $\rightarrow$ Analyze $\rightarrow$ Propose $\rightarrow$ Execute** pattern. Every destructive action must be preceded by an AI-verified diagnostic to ensure that "cleaning" a directory doesn't break a critical dependency or delete a vital dataset from a long-running bioinformatics pipeline.

## 2. Command Hierarchy (CLI Specification)
The CLI is designed using a `module -> action` syntax, allowing for both rapid one-liners and deep-module interaction.

### **Module: `free` (Garbage Collection & Space Recovery)**
*Focus: Reclaiming disk space by targeting non-essential artifacts.*
*   **`bioclean free cache`**: Targets `apt/pacman` caches, `pip` wheels, `conda` package archives, and Docker layer orphans.
*   **`bioclean free logs`**: Interfaces with `journalctl` to vacuum logs older than a specified threshold (e.g., `--days 7`).
*   **`bioclean free tmp`**: Safely clears `/tmp` and `/var/tmp`, respecting file access timestamps to avoid breaking active processes.
*   **`bioclean free orphans`**: Identifies and suggests removal of unneeded dependencies (e.g., `deborphan` or `apt autoremove` logic).

### **Module: `power` (Energy & Thermal Management)**
*Focus: Optimizing the power profile for the current workload.*
*   **`bioclean power battery`**: Adjusts `tlp` or `powertop` settings for maximum longevity; lowers CPU frequency scaling, disables high-performance peripherals.

*   **`bioclean power performance`**: Prepares the system for heavy computation (e.g., genomic sequencing); sets CPU governor to `performance`, maximizes I/O priority.
*   **`bioclean power thermal`**: Monitors `thermald`; suggests frequency caps if the system is hitting T-junction limits.

### **Module: `scan` (Deep Observability)**
*Focus: Identifying bloat and bottlenecks.*
*   **`bioclean scan heavy`**: A high-speed implementation of `du -ah`, using an AI summary to tell you *why* these folders are large (e.g., "This folder contains 50GB of intermediate `.sam` files from a finished alignment task").
*   **`bioclean scan sockets`**: Audits active network connections and identifies "zombie" or high-bandwidth processes.

### **Module: `diagnose` (The AI Engine)**
*Focus: The Ollama-powered intelligence layer.*
*   **`bioclean diagnose`**: The flagship command. It aggregates output from `dmesg`, `journalctl -p 3`, `df`, and `free -m`, feeds it to a local **Ollama (Llama-3/Mistral)** instance, and returns a human-readable "System Health Report."

---

/
## 3. Intelligent Workflows (The Agentic Layer)
Workflows are pre-configured sequences of the commands above, tailored for specific user personas.

### **Workflow A: `bioclean workflow prepare-crunch`**
*Target User: Bioinformatician / Data Scientist starting a multi-day simulation.*
1.  **Scan:** Check available disk space in the target scratch directory.
2.  **Free:** Clean package caches and old logs to maximize available space.
3.  **Power:** Switch to `performance` profile; ensure cooling is optimized.
4.  **Verify:** Ensure no high-priority I/O processes are competing for bandwidth.
5.  **Result:** A "Green Light" report confirming the system is stabilized for the workload.

### **Workflow B: `bioclean workflow maintenance`**
*Target User: DevOps / SysAdmin performing weekly upkeep.*
1.  **Audit:** Run `diagnose` to find any recent kernel errors or hardware warnings.
2.  **Clean:** Execute `free cache`, `free logs`, and `free orphans`.
3.  **Optimize:** Run `fstrim` on SSDs and optimize `sysctl` parameters.
4.  **Report:** Generate a Markdown summary of what was cleaned and what requires manual attention.

---

## 4. Interface & UX Requirements
*   **The "Selection" Mechanism:** While the CLI supports direct commands, running a module without arguments (e.g., just `bioclean free`) should trigger an **Interactive TUI (Terminal User Interface)** using `gum` or `ratatui`. This allows users to use arrow keys to select specific sub-targets for cleaning.
*   **The "Dry Run" Standard:** Every command supports a `--dry-run` flag. The AI will simulate the impact, showing exactly which files would be deleted and the predicted space recovered.
*   **Safety Interlock:** Any command involving `rm` or `systemctl stop` triggers an AI-generated prompt: *"I see you are about to delete /var/lib/conda/envs. This may break your current project 'Project_Alpha'. Do you wish to proceed? (y/N)"*

## 5. Technical Implementation Stack
*   **Language:** **Rust** (for speed, safety, and easy distribution as a single binary).
*   **Intelligence:** **Ollama API** (local-first, privacy-preserving).
*   **Data Sources:** `systemd` (journal/services), `udev` (hardware), `procfs`/`sysfs` (kernel/power), `du`/`df` (filesystem).
*   **Interaction:** `clap` (CLI parsing) and `ratatui` (Interactive TUI elements).
