#!/usr/bin/env bash
# -----------------------------------------------------------------------------
# AidBio AI - Cache & System Cleanup Module
# Authors: Cleverson Matiolli, PhD and Gemini
# -----------------------------------------------------------------------------

clean_interactive() {
    ui_section "Interactive System & Cache Cleanup"
    ui_info "All deletion operations require explicit confirmation [y/N]."

    # 1. Docker Cleanup
    if command -v docker &>/dev/null; then
        echo -e "\n${BOLD}1. Docker Prune (Unused Containers, Volumes & Build Caches)${NC}"
        docker system df 2>/dev/null | awk '{print "   " $0}' || true
        if ui_confirm "Prune unused Docker volumes, stopped containers, and build cache?"; then
            ui_info "Running docker system prune..."
            docker system prune -f --volumes
            ui_success "Docker system pruned."
        else
            ui_info "Skipped Docker cleanup."
        fi
    fi

    # 2. Snap Purge
    if command -v snap &>/dev/null; then
        echo -e "\n${BOLD}2. Old Snap Package Revisions${NC}"
        local disabled_snaps
        disabled_snaps=$(LANG=C snap list --all 2>/dev/null | awk '/disabled/{print $1, $3}' || true)
        if [[ -n "$disabled_snaps" ]]; then
            echo "$disabled_snaps" | awk '{print "   Disabled revision: " $1 " (rev " $2 ")"}'
            if ui_confirm "Purge old disabled Snap package revisions?"; then
                sudo snap set system refresh.retain=2 2>/dev/null || true
                echo "$disabled_snaps" | while read -r snapname revision; do
                    ui_info "Removing $snapname (revision $revision)..."
                    sudo snap remove "$snapname" --revision="$revision"
                done
                ui_success "Old snap revisions purged."
            else
                ui_info "Skipped Snap cleanup."
            fi
        else
            ui_info "No disabled snap revisions found."
        fi
    fi

    # 3. System Journal Vacuum
    echo -e "\n${BOLD}3. System Journal Logs${NC}"
    local journal_size
    journal_size=$(journalctl --disk-usage 2>/dev/null || echo "")
    ui_info "$journal_size"
    if ui_confirm "Vacuum system journal logs to 100 MB?"; then
        sudo journalctl --vacuum-size=100M
        ui_success "Journal logs vacuumed."
    else
        ui_info "Skipped journal cleanup."
    fi

    # 4. APT Package Cache
    echo -e "\n${BOLD}4. APT Package Cache${NC}"
    local apt_size
    apt_size=$(du -sh /var/cache/apt 2>/dev/null | cut -f1 || echo "0B")
    ui_info "APT Cache size: $apt_size"
    if ui_confirm "Clean APT cache and autoremove unneeded packages?"; then
        sudo apt-get clean
        sudo apt-get autoremove -y
        ui_success "APT cache cleared."
    else
        ui_info "Skipped APT cleanup."
    fi

    # 5. User Build & Package Caches
    echo -e "\n${BOLD}5. User Caches (pip, uv, Cypress, thumbnails)${NC}"
    if ui_confirm "Purge pip/uv package caches and temporary build artifacts?"; then
        pip cache purge 2>/dev/null || true
        uv cache clean 2>/dev/null || true
        rm -rf ~/.cache/Cypress ~/.cache/node-gyp ~/.cache/thumbnails ~/.cache/ms-playwright-go 2>/dev/null || true
        ui_success "User build caches purged."
    else
        ui_info "Skipped user cache cleanup."
    fi

    ui_section "Cleanup Completed!"
    df -h / | awk 'NR==1 {printf "%-20s %-10s %-10s %-10s %-10s\n", $1, $2, $3, $4, $5} NR==2 {printf "%-20s %-10s %-10s %-10s %-10s\n", $1, $2, $3, $4, $5}'
}
