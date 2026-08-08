#!/usr/bin/env bash
# -----------------------------------------------------------------------------
# AidBio AI - Smart Data Storage & Disk Optimization Suite
# Authors: Cleverson Matiolli, PhD and Gemini
# Version: 1.0.0
# -----------------------------------------------------------------------------
set -euo pipefail

# Resolve real physical path of script, handling symlinks correctly
REAL_SOURCE="${BASH_SOURCE[0]}"
while [ -h "$REAL_SOURCE" ]; do
  SCRIPT_DIR="$( cd -P "$( dirname "$REAL_SOURCE" )" >/dev/null 2>&1 && pwd )"
  REAL_SOURCE="$(readlink "$REAL_SOURCE")"
  [[ $REAL_SOURCE != /* ]] && REAL_SOURCE="$SCRIPT_DIR/$REAL_SOURCE"
done
SCRIPT_DIR="$( cd -P "$( dirname "$REAL_SOURCE" )" >/dev/null 2>&1 && pwd )"
LIB_DIR="$SCRIPT_DIR/lib"

# Load modules
if [[ ! -d "$LIB_DIR" ]]; then
    echo "ERROR: Library directory not found at $LIB_DIR"
    exit 1
fi

source "$LIB_DIR/ui.sh"
source "$LIB_DIR/history.sh"
source "$LIB_DIR/audit.sh"
source "$LIB_DIR/clean.sh"
source "$LIB_DIR/hdd.sh"

show_version() {
    echo "AidBio AI Disk Manager v1.0.0"
    echo "Authors: Cleverson Matiolli, PhD and Gemini"
}

show_help() {
    ui_banner
    echo -e "${BOLD}USAGE:${NC}"
    echo -e "  clean-disk [OPTION]\n"
    echo -e "${BOLD}DESCRIPTION:${NC}"
    echo -e "  AidBio AI Smart Data Storage & Disk Optimization Suite."
    echo -e "  Keeps SSDs clean, audits bioinformatic datasets (SRA, FASTQ, BAM, VCF),"
    echo -e "  offloads raw datasets to external HDDs using transparent symlinks,"
    echo -e "  and provides complete transaction logging with undo capabilities.\n"
    echo -e "${BOLD}OPTIONS:${NC}"
    echo -e "  -a, --audit       Scan SSD partition, large bio datasets, and system caches"
    echo -e "  -c, --clean       Interactively clean Docker, Snap, journalctl, APT & user caches"
    echo -e "  -m, --migrate     Auto-discover external HDDs, migrate bio data & setup symlinks"
    echo -e "  -u, --undo        Revert last migration session (restores files & removes symlinks)"
    echo -e "  -l, --history     Display transaction session history log"
    echo -e "  -h, --help        Show this help message and exit"
    echo -e "  -v, --version     Show version information and exit\n"
    echo -e "${BOLD}AUTHORS:${NC}"
    echo -e "  Cleverson Matiolli, PhD and Gemini\n"
}

interactive_menu() {
    while true; do
        clear || true
        ui_banner
        echo -e "${BOLD}MAIN MENU:${NC}"
        echo -e "  [1] 🔍 Audit SSD Usage & Large Bio Datasets"
        echo -e "  [2] 🧹 Interactive Cache & System Cleanup"
        echo -e "  [3] 💾 Scan External HDDs & Migrate Bio Data (Symlink Setup)"
        echo -e "  [4] ↩️  Undo / Revert Last Migration Session"
        echo -e "  [5] 📜 View Transaction History Log"
        echo -e "  [0] 🚪 Exit\n"

        echo -ne "${YELLOW}${BOLD}Select an option [0-5]: ${NC}"
        read -r choice

        case "$choice" in
            1)
                audit_run_all
                echo -ne "\n${GRAY}Press Enter to return to menu...${NC}"
                read -r
                ;;
            2)
                clean_interactive
                echo -ne "\n${GRAY}Press Enter to return to menu...${NC}"
                read -r
                ;;
            3)
                hdd_migrate_workflow
                echo -ne "\n${GRAY}Press Enter to return to menu...${NC}"
                read -r
                ;;
            4)
                history_undo_last
                echo -ne "\n${GRAY}Press Enter to return to menu...${NC}"
                read -r
                ;;
            5)
                history_list
                echo -ne "\n${GRAY}Press Enter to return to menu...${NC}"
                read -r
                ;;
            0)
                ui_info "Exiting AidBio AI Disk Manager. Goodbye!"
                exit 0
                ;;
            *)
                ui_warn "Invalid selection. Please choose 0-5."
                sleep 1
                ;;
        esac
    done
}

main() {
    if [[ $# -eq 0 ]]; then
        interactive_menu
        exit 0
    fi

    case "$1" in
        -a|--audit)
            ui_banner
            audit_run_all
            ;;
        -c|--clean)
            ui_banner
            clean_interactive
            ;;
        -m|--migrate)
            ui_banner
            hdd_migrate_workflow
            ;;
        -u|--undo)
            ui_banner
            history_undo_last
            ;;
        -l|--history)
            ui_banner
            history_list
            ;;
        -v|--version)
            show_version
            ;;
        -h|--help)
            show_help
            ;;
        *)
            ui_error "Unknown option: $1"
            show_help
            exit 1
            ;;
    esac
}

main "$@"
