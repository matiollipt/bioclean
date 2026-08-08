#!/usr/bin/env bash
# -----------------------------------------------------------------------------
# AidBio AI - UI & Terminal Helper Module
# Authors: Cleverson Matiolli, PhD and Gemini
# -----------------------------------------------------------------------------

# Colors
BOLD='\033[1m'
NC='\033[0m' # No Color
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
GRAY='\033[0;90m'

ui_banner() {
    echo -e "${CYAN}${BOLD}"
    echo "================================================================="
    echo "                      AidBio AI - Disk Manager                   "
    echo "       Smart Storage Optimization & Bioinformatic Management     "
    echo "       Authors: Cleverson Matiolli, PhD and Gemini               "
    echo "================================================================="
    echo -e "${NC}"
}

ui_section() {
    echo -e "\n${BLUE}${BOLD}===> $1${NC}"
}

ui_info() {
    echo -e "${CYAN}ℹ $1${NC}"
}

ui_success() {
    echo -e "${GREEN}✔ $1${NC}"
}

ui_warn() {
    echo -e "${YELLOW}⚠️ $1${NC}"
}

ui_error() {
    echo -e "${RED}✖ $1${NC}"
}

# Explicit user confirmation prompt (defaults to NO)
ui_confirm() {
    local prompt_msg="$1"
    local default_ans="${2:-N}"
    local ans

    if [[ "$default_ans" == "Y" ]]; then
        prompt_msg="$prompt_msg [Y/n]: "
    else
        prompt_msg="$prompt_msg [y/N]: "
    fi

    echo -ne "${YELLOW}${BOLD}${prompt_msg}${NC}"
    read -r ans
    ans=$(echo "$ans" | tr '[:upper:]' '[:lower:]' | xargs)

    if [[ -z "$ans" ]]; then
        ans=$(echo "$default_ans" | tr '[:upper:]' '[:lower:]')
    fi

    if [[ "$ans" == "y" || "$ans" == "yes" ]]; then
        return 0
    else
        return 1
    fi
}
