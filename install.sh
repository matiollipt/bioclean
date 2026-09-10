#!/usr/bin/env bash
# -----------------------------------------------------------------------------
# AidBio AI - bioclean Agentic System Orchestrator Installer
# Authors: Cleverson Matiolli, PhD and Gemini
# Version: 2.0.0
# -----------------------------------------------------------------------------
set -euo pipefail

BOLD='\033[1m'
NC='\033[0m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
RED='\033[0;31m'

banner() {
    echo -e "${CYAN}${BOLD}"
    echo "================================================================="
    echo "            AidBio AI - bioclean System Orchestrator            "
    echo "      High-Performance Linux Agentic Environment Installer      "
    echo "      Authors: Cleverson Matiolli, PhD and Gemini               "
    echo "================================================================="
    echo -e "${NC}"
}

BIN_DIR="${HOME}/.local/bin"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

check_rust() {
    echo -e "${BOLD}Checking Rust Toolchain...${NC}"
    if [ -f "$HOME/.cargo/env" ]; then
        # shellcheck disable=SC1091
        source "$HOME/.cargo/env"
    fi

    if ! command -v cargo &>/dev/null; then
        echo -e "${YELLOW}Rust toolchain not found. Installing via rustup...${NC}"
        curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --profile default --default-toolchain stable
        # shellcheck disable=SC1091
        source "$HOME/.cargo/env"
    fi

    echo -e "  ✔ Found: ${GREEN}$(cargo --version)${NC}"
    echo -e "  ✔ Found: ${GREEN}$(rustc --version)${NC}"
}

configure_path() {
    echo -e "\n${BOLD}Configuring Shell PATH...${NC}"
    mkdir -p "$BIN_DIR"

    local path_line='export PATH="$HOME/.local/bin:$HOME/.cargo/bin:$PATH"'
    local added=0

    for rc_file in "$HOME/.bashrc" "$HOME/.zshrc" "$HOME/.profile"; do
        if [[ -f "$rc_file" ]]; then
            if ! grep -qs 'local/bin' "$rc_file"; then
                echo -e "\n# Added by AidBio bioclean" >> "$rc_file"
                echo "$path_line" >> "$rc_file"
                echo -e "  ✔ Added PATH to ${GREEN}$rc_file${NC}"
                added=1
            else
                echo -e "  ✔ PATH already configured in ${GREEN}$rc_file${NC}"
            fi
        fi
    done

    if [[ $added -eq 1 ]]; then
        echo -e "${YELLOW}Note: Restart your terminal or run 'source ~/.bashrc' to apply PATH changes.${NC}"
    fi
}

install_app() {
    banner
    check_rust
    configure_path

    echo -e "\n${BOLD}Compiling bioclean in Release Mode...${NC}"
    cd "$SCRIPT_DIR"
    cargo build --release

    local release_bin="$SCRIPT_DIR/target/release/bioclean"
    if [[ ! -f "$release_bin" ]]; then
        echo -e "${RED}ERROR: Build failed; binary not found at $release_bin${NC}"
        exit 1
    fi

    echo -e "\n${BOLD}Installing Binary...${NC}"
    cp "$release_bin" "$BIN_DIR/bioclean"
    chmod +x "$BIN_DIR/bioclean"
    echo -e "  ✔ Installed: ${GREEN}$BIN_DIR/bioclean${NC}"

    echo -e "\n${GREEN}${BOLD}=================================================================${NC}"
    echo -e "${GREEN}${BOLD}       bioclean v2.0.0 Installed Successfully!                   ${NC}"
    echo -e "${GREEN}${BOLD}=================================================================${NC}"
    echo -e "\nYou can now run ${CYAN}${BOLD}bioclean${NC} from any directory."
    echo -e "  • Interactive TUI    : ${YELLOW}bioclean${NC}"
    echo -e "  • System Health Diag : ${YELLOW}bioclean diagnose${NC}"
    echo -e "  • Cache Space Cleanup: ${YELLOW}bioclean free cache${NC}"
    echo -e "  • Prepare Crunch Work: ${YELLOW}bioclean workflow prepare-crunch${NC}"
    echo -e "  • Complete CLI Help  : ${YELLOW}bioclean --help${NC}\n"
}

uninstall_app() {
    banner
    echo -e "${YELLOW}${BOLD}Uninstalling bioclean...${NC}"
    rm -f "$BIN_DIR/bioclean"
    echo -e "${GREEN}bioclean uninstalled successfully.${NC}"
}

if [[ "${1:-}" == "--uninstall" ]]; then
    uninstall_app
else
    install_app
fi
