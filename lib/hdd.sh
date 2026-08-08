#!/usr/bin/env bash
# -----------------------------------------------------------------------------
# AidBio AI - External HDD Manager & Migration Module
# Authors: Cleverson Matiolli, PhD and Gemini
# -----------------------------------------------------------------------------

hdd_list_mounts() {
    ui_section "Scanning Mounted External Hard Drives"
    
    # Python helper to scan mounts under /media, /mnt, /run/media
    python3 - <<EOF
import os, psutil

mounts = []
for p in psutil.disk_partitions(all=False):
    # Exclude root NVMe/boot and loop devices
    if p.mountpoint.startswith(('/media', '/mnt', '/run/media')) and not p.mountpoint.startswith('/snap'):
        try:
            usage = psutil.disk_usage(p.mountpoint)
            total_gb = usage.total / (1024**3)
            free_gb = usage.free / (1024**3)
            used_pct = usage.percent
            mounts.append((p.device, p.fstype, p.mountpoint, total_gb, free_gb, used_pct))
        except Exception:
            pass

if not mounts:
    print("  ⚠️  No external HDDs currently detected under /media, /mnt, or /run/media.")
    print("     Please connect your external HDD and ensure it is mounted.")
else:
    print(f"  {'DEVICE':<15} {'FSTYPE':<8} {'TOTAL':<10} {'FREE':<10} {'USAGE':<8} {'MOUNT POINT'}")
    print("  " + "-" * 75)
    for dev, fstype, mp, total, free, pct in mounts:
        print(f"  {dev:<15} {fstype:<8} {total:>7.1f} GB {free:>7.1f} GB {pct:>6.1f}%  {mp}")
EOF
}

# Returns selected mount point
hdd_select_mount() {
    local candidate_mounts=()
    while IFS= read -r line; do
        if [[ -n "$line" && -d "$line" ]]; then
            candidate_mounts+=("$line")
        fi
    done < <(python3 -c "import psutil; [print(p.mountpoint) for p in psutil.disk_partitions() if p.mountpoint.startswith(('/media', '/mnt', '/run/media'))]" 2>/dev/null)

    if [[ ${#candidate_mounts[@]} -eq 0 ]]; then
        echo ""
        return 0
    fi

    if [[ ${#candidate_mounts[@]} -eq 1 ]]; then
        echo "${candidate_mounts[0]}"
        return 0
    fi

    # Multiple mounts found - prompt choice
    echo -e "\n${BOLD}Select Target External HDD:${NC}" >&2
    local idx=1
    for m in "${candidate_mounts[@]}"; do
        echo "  [$idx] $m" >&2
        ((idx++))
    done
    
    echo -ne "${YELLOW}Enter number (1-${#candidate_mounts[@]}): ${NC}" >&2
    local choice
    read -r choice
    if [[ "$choice" =~ ^[0-9]+$ ]] && (( choice >= 1 && choice <= ${#candidate_mounts[@]} )); then
        echo "${candidate_mounts[$((choice-1))]}"
    else
        echo ""
    fi
}

hdd_migrate_workflow() {
    hdd_list_mounts
    local target_hdd
    target_hdd=$(hdd_select_mount)

    if [[ -z "$target_hdd" ]]; then
        ui_warn "No external HDD target selected. You can also manually specify a path."
        echo -ne "${YELLOW}Enter target external HDD mount directory: ${NC}"
        read -r target_hdd
    fi

    if [[ -z "$target_hdd" || ! -d "$target_hdd" ]]; then
        ui_error "Invalid or missing external HDD mount point."
        return 1
    fi

    ui_success "Target External HDD: $target_hdd"

    local session_id
    session_id=$(history_start_session)
    ui_info "Created session ID: $session_id"

    local base_src="/home/clever/aidbio/ds/transcriptome/data"
    local base_dest="$target_hdd/aidbio_storage/transcriptome"

    ui_section "Discovered Local Bioinformatic Datasets"

    local targets=(
        "$base_src/raw/sra"
        "$base_src/processed/fastq"
        "$base_src/processed/trimmed"
    )

    for src in "${targets[@]}"; do
        if [[ -d "$src" && ! -L "$src" ]]; then
            local size
            size=$(du -sh "$src" 2>/dev/null | cut -f1)
            echo -e "  • ${CYAN}$src${NC}  (Size: ${YELLOW}$size${NC})"
            
            local cat_name
            cat_name=$(basename "$src")
            local dest_path="$base_dest/$cat_name"

            if ui_confirm "Move $src ($size) to HDD ($dest_path) and replace with symlink?"; then
                ui_info "Moving data to $dest_path..."
                mkdir -p "$dest_path"
                
                rsync -av --progress --remove-source-files "$src/" "$dest_path/"
                rm -rf "$src"
                ln -s "$dest_path" "$src"

                history_record_op "$session_id" "MOVE_AND_SYMLINK" "$src" "$dest_path"
                ui_success "Moved and symlinked: $src -> $dest_path"
            else
                ui_info "Skipped migration for $src"
            fi
        elif [[ -L "$src" ]]; then
            ui_info "Already symlinked: $src -> $(readlink -f "$src")"
        else
            ui_info "Path not found: $src"
        fi
    done

    # Configure NCBI SRA Toolkit cache on HDD
    echo -e "\n${BOLD}Reconfigure NCBI SRA Toolkit Cache${NC}"
    local ncbi_cache_dir="$target_hdd/aidbio_storage/ncbi_cache"
    if ui_confirm "Set NCBI SRA Toolkit temporary cache directory to HDD ($ncbi_cache_dir)?"; then
        mkdir -p "$ncbi_cache_dir"
        mkdir -p "$HOME/.ncbi"
        cat <<EOF > "$HOME/.ncbi/user-settings.mkfg"
/LIBS/GUID = "d906eff4-d782-4e40-b228-1189d36c2937"
/config/default = "false"
/libs/cache_amount = "8"
/libs/temp_cache = "$ncbi_cache_dir"
/libs/vdb/quality = "RZ"
/repository/user/main/public/root = "$ncbi_cache_dir"
EOF
        ui_success "NCBI SRA Toolkit reconfigured to store caches on HDD!"
    fi

    ui_section "Migration Step Finished"
    df -h / | awk 'NR==1 {printf "%-20s %-10s %-10s %-10s %-10s\n", $1, $2, $3, $4, $5} NR==2 {printf "%-20s %-10s %-10s %-10s %-10s\n", $1, $2, $3, $4, $5}'
}
