# AidBio AI - Smart Storage & Disk Optimization Suite (`clean-disk`)

**Authors:** Cleverson Matiolli, PhD and Gemini  
**Branding:** AidBio AI  
**Version:** 1.0.0  

---

## Overview

`clean-disk` is a specialized, production-minded disk optimization and storage management tool built for Linux systems running computational biology, bioinformatics, and ML pipelines.

### Features
* **SSD & Dataset Audit (`clean-disk --audit`)**: Scans NVMe/SSD partitions, large bioinformatic files (`.sra`, `.fastq`, `.bam`, `.vcf`), and reclaimable caches.
* **Interactive Cache Cleanup (`clean-disk --clean`)**: Safely purges Docker volumes (~22 GB), old Snap revisions (~8.4 GB), journal logs (~1.5 GB), APT caches, and user build caches. **Requires explicit `[y/N]` confirmation before deleting anything!**
* **External HDD Auto-Discovery & Data Offloading (`clean-disk --migrate`)**:
  * Auto-discovers attached, mounted external HDDs.
  * Offloads raw/processed bioinformatic data to the external HDD.
  * Replaces local folders with **transparent symbolic links** so pipelines work without code changes.
  * Reconfigures NCBI SRA Toolkit (`~/.ncbi/user-settings.mkfg`) to use the external HDD.
* **Rollback & Transaction Undo (`clean-disk --undo`)**: Logs every operation in `~/.aidbio/history/clean_disk_sessions.json` and allows single-command reversal of migration sessions.

---

## Installation on Any AidBio Computer

To install `clean-disk` on any Ubuntu/Linux computer:

```bash
git clone git@github.com:matiollipt/clean-disk.git
cd clean-disk
./install.sh
```

The installer will:
1. Auto-detect required dependencies (`bash`, `python3`, `rsync`, `psutil`).
2. Copy app files into `~/.local/share/aidbio/disk-manager/`.
3. Create the launcher symlink at `~/.local/bin/clean-disk`.
4. Ensure `~/.local/bin` is configured in `~/.bashrc`, `~/.zshrc`, and `~/.profile`.

---

## Usage Summary

```bash
# Launch interactive menu driver
clean-disk

# Command-line options
clean-disk --audit       # Audit SSD, bio datasets, and caches
clean-disk --clean       # Interactive cache and system cleanup
clean-disk --migrate     # Scan external HDDs, migrate datasets & setup symlinks
clean-disk --history     # View transaction session history
clean-disk --undo        # Revert last migration session
clean-disk --help        # Display help message
clean-disk --version     # Display version information
```

To uninstall from a computer:
```bash
./install.sh --uninstall
```
