#!/usr/bin/env bash
# -----------------------------------------------------------------------------
# AidBio AI - System Audit Module
# Authors: Cleverson Matiolli, PhD and Gemini
# -----------------------------------------------------------------------------

audit_run_all() {
    ui_section "1. Root SSD Partition Usage"
    df -h / | awk 'NR==1 {printf "%-20s %-10s %-10s %-10s %-10s\n", $1, $2, $3, $4, $5} NR==2 {printf "%-20s %-10s %-10s %-10s %-10s\n", $1, $2, $3, $4, $5}' || true

    ui_section "2. Large Bioinformatic Datasets (>200 MiB)"
    echo -e "${GRAY}Scanning /home/clever for .sra, .fastq, .fq, .bam, .vcf files...${NC}"
    find /home/clever -type f \( -name "*.fastq*" -o -name "*.fq*" -o -name "*.sra" -o -name "*.bam" -o -name "*.vcf*" \) \
        -size +200M -exec du -h {} + 2>/dev/null | sort -rh | head -n 15 | awk '{printf "  • %-10s %s\n", $1, $2}' || true

    ui_section "3. Key Data & Directory Sizes"
    if [[ -d "/home/clever/aidbio/ds" ]]; then
        du -sh /home/clever/aidbio/ds/* 2>/dev/null | awk '{printf "  • %-10s %s\n", $1, $2}' || true
    fi

    ui_section "4. System Caches & Reclaimable Space"
    # Docker
    if command -v docker &>/dev/null; then
        echo -e "  • Docker Reclaimable Space:"
        docker system df 2>/dev/null | awk '{print "    " $0}' || echo "    Docker state unavailable"
    fi

    # Journalctl
    if command -v journalctl &>/dev/null; then
        local jsize
        jsize=$(journalctl --disk-usage 2>/dev/null | grep -oE '[0-9.]+[MGB]' || echo "N/A")
        echo -e "  • System Journal Logs Size : ${YELLOW}$jsize${NC}"
    fi

    # APT Cache
    if [[ -d "/var/cache/apt" ]]; then
        local apt_size
        apt_size=$(du -sh /var/cache/apt 2>/dev/null | cut -f1)
        echo -e "  • APT Package Cache Size   : ${YELLOW}${apt_size:-N/A}${NC}"
    fi

    # User Caches
    echo -e "  • User Cache Directory Sizes (~/.cache):"
    du -sh $HOME/.cache/* 2>/dev/null | sort -rh | head -n 8 | awk '{printf "    - %-10s %s\n", $1, $2}' || true
}
