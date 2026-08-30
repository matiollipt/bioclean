# Implementation Plan: AidBio AI Interactive Disk Manager (`clean-disk.sh`)

**Artifact:** `ssd_optimization_plan.md`  
**Branding:** AidBio AI  
**Authors:** Cleverson Matiolli, PhD and Gemini  
**Target File:** `/home/clever/aidbio/clean-disk/clean-disk.sh` (and modular library `/home/clever/aidbio/clean-disk/lib/`)  

---

## Goal Description

Build a production-grade, interactive, modular CLI tool `clean-disk.sh` for Ubuntu systems tailored for bioinformatic workflows.

### Key Capabilities:

1. **Interactive TUI & CLI Options**: Interactive main menu + command-line flags (`--help`, `--audit`, `--clean`, `--migrate`, `--undo`, `--version`).
2. **SSD Audit & Scanning**: Deep inspection of NVMe SSD usage, bioinformatic datasets (`.sra`, `.fastq`, `.bam`, `.vcf`), Docker containers/volumes, Snap revisions, system logs, and user caches.
3. **Interactive Cache Cleaning**: Safe, step-by-step cleaning with mandatory user confirmation before deleting any caches or Docker/Snap artifacts.
4. **External HDD Auto-Discovery & Data Migration**:
   
   - Auto-scans all attached, mounted external drives (`/media/`, `/mnt/`, `/run/media/`).
   
   - Displays filesystem, total capacity, and available free space.
   
   - Offloads raw/processed bioinformatic data (`sra/`, `fastq/`, `trimmed/`) to the selected HDD and replaces original locations with transparent symbolic links.
   
   - Automatically updates NCBI SRA Toolkit configuration (`~/.ncbi/user-settings.mkfg`).
5. **Session Transaction History & Rollback (`--undo`)**:
   
   - Logs all migration actions (moved paths, target locations, created symlinks) in structured JSON format (`~/.aidbio/history/clean_disk_sessions.json`).
   
   - Provides an **Undo Last Session** command to seamlessly restore files from external HDD back to SSD and remove created symlinks.
6. **Confirmation & Safety First**: Every operation modifying files (delete, move, copy, symlink) prompts for explicit user confirmation (`[y/N]`).

---

## Architecture & Modular Layout

The tool repository is located at `/home/clever/aidbio/clean-disk/` and installs to `~/.local/share/aidbio/disk-manager/` (linked to `~/.local/bin/clean-disk` for global execution):

```
/home/clever/aidbio/clean-disk/
├── clean-disk.sh              # Main entrypoint script & menu driver
├── install.sh                 # Deployment & installer script
└── lib/
    ├── ui.sh                  # Color formatting, headers, banners, confirmation prompts
    ├── audit.sh               # Disk usage scanner & bioinformatic file discovery
    ├── clean.sh               # System cache, Docker, Snap, journalctl cleaner
    ├── hdd.sh                 # External HDD scanner, data migrator, SRA config, symlink manager
    └── history.sh             # JSON session logger & undo/rollback engine
```

---

## Detailed Component Specifications

### 1. Main Entrypoint & Branding (`clean-disk.sh`)

#### Features:

- Displays AidBio AI banner with authors: **Cleverson Matiolli, PhD and Gemini**.
- Parses `--help`, `-h`, `--audit`, `--clean`, `--migrate`, `--undo`, `--version`.
- Provides an interactive ASCII menu when run without flags.

```bash
AidBio AI - Smart Data Storage & Disk Optimization Suite v1.0.0
Authors: Cleverson Matiolli, PhD and Gemini
Usage: clean-disk.sh [OPTION]

Options:
  -a, --audit        Scan SSD, large bioinformatic files, and system caches
  -c, --clean        Interactive cleaning of caches, Docker, Snap, and logs
  -m, --migrate      Auto-discover external HDDs, migrate bio data & setup symlinks
  -u, --undo         Revert last session (restore moved files & remove symlinks)
  -h, --help         Display this help message and exit
  -v, --version      Show version information
```

---

### 2. User Interface & Safety Module (`lib/ui.sh`)

#### Functions:

- `ui_banner()`: Standardized header.
- `ui_confirm(prompt)`: Prompts `[y/N]` with default `N` to prevent accidental execution.
- `ui_print_table()`: Formatted color output for disk space and scanner results.

---

### 3. Audit Module (`lib/audit.sh`)

#### Functions:

- `audit_ssd_usage()`: Summarizes root partition `/dev/nvme0n1p2` usage.
- `audit_bio_files()`: Scans `/home/clever/aidbio` and system for SRA, FASTQ, BAM, VCF files > 500 MiB.
- `audit_caches()`: Checks size of Docker volumes/containers, Snap versions, journalctl logs, APT cache, `~/.cache` (Chrome, Whisper, uv, HuggingFace).

---

### 4. Clean Module (`lib/clean.sh`)

#### Functions (each requires `ui_confirm` before execution):

- `clean_docker()`: `docker system prune -f --volumes` (reclaims ~22 GB).
- `clean_snap()`: Purges old disabled snap revisions (reclaims ~8.4 GB).
- `clean_journal()`: `sudo journalctl --vacuum-size=100M` (reclaims ~1.4 GB).
- `clean_apt()`: `sudo apt-get clean && sudo apt-get autoremove -y`.
- `clean_user_caches()`: `pip cache purge`, `uv cache clean`, Cypress/browser caches.

---

### 5. External HDD Module (`lib/hdd.sh`)

#### Functions:

- `hdd_scan_mounts()`: Queries `lsblk -J` or `df -h` to list external mounted drives under `/media/`, `/mnt/`, `/run/media/`. Displays drive name, mount point, filesystem, total size, free space.
- `hdd_migrate_directory(src, target_hdd)`:
  
  1. Asks user confirmation for each directory/file.
  
  2. Copies/moves data using `rsync -av --progress` or `mv`.
  
  3. Replaces original path with `ln -s $target $src`.
  
  4. Records the exact transaction in `history.sh`.
- `hdd_configure_sra(target_hdd)`: Updates `~/.ncbi/user-settings.mkfg` to point NCBI `temp_cache` and `public/root` to the external HDD.

---

### 6. Transaction Logging & Rollback Module (`lib/history.sh`)

#### Session History Storage: `~/.aidbio/history/clean_disk_sessions.json`

Format:

```json
{
  "session_id": "20260808_063000",
  "timestamp": "2026-08-08T06:30:00-03:00",
  "hdd_mount": "/media/clever/MyExternalHDD",
  "operations": [
    {
      "type": "MIGRATE_SYMLINK",
      "original_path": "/home/clever/aidbio/ds/transcriptome/data/raw/sra",
      "hdd_path": "/media/clever/MyExternalHDD/aidbio_storage/transcriptome/sra",
      "bytes_moved": 48318382080
    }
  ]
}
```

#### Functions:

- `history_log_operation(session_id, type, src, dest)`: Appends transaction record.
- `history_undo_last_session()`:
  
  1. Reads the latest session from JSON history.
  
  2. Prompts user: *"Revert session 20260808_063000? This will move 45 GB back to SSD from /media/clever/MyExternalHDD."*
  
  3. Upon confirmation:
     - Deletes the symbolic link on the SSD.
     - Moves files back from HDD to SSD (`rsync -av` / `mv`).
     - Restores original NCBI SRA configuration if changed.
     - Marks session as `REVERTED`.

---

## Verification Plan

### Automated & Functionality Tests

| Test Case                   | Execution Command                 | Expected Behavior                                                 |
| --------------------------- | --------------------------------- | ----------------------------------------------------------------- |
| Help & Version output       | `./clean-disk.sh --help`          | Displays AidBio AI header, authors, options cleanly               |
| HDD Scanner                 | `./clean-disk.sh --audit`         | Correctly identifies SSD usage and lists mounted HDDs             |
| Dry-run Confirmation Safety | Select clean option & choose `N`  | Aborts without modifying any file or system state                 |
| Session Logging             | Run test migration on mock folder | Creates valid JSON entry in `~/.aidbio/history/`                  |
| Rollback / Undo Test        | `./clean-disk.sh --undo`          | Reverts symlink, restores mock files to SSD, updates history JSON |

---

## Next Steps

1. Create directory structure `/home/clever/aidbio/clean-disk/lib/`.
2. Implement modular library files (`ui.sh`, `audit.sh`, `clean.sh`, `hdd.sh`, `history.sh`).
3. Implement `clean-disk.sh` master script and link it to `~/.local/bin/clean-disk.sh`.
4. Verify executable permissions and test `--help`, `--audit`, and `--undo` functionality.
