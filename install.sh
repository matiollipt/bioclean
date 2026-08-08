#!/usr/bin/env bash
# -----------------------------------------------------------------------------
# AidBio AI Disk Manager - Installer & Deployer
# Authors: Cleverson Matiolli, PhD and Gemini
# Version: 1.0.0
# -----------------------------------------------------------------------------
set -euo pipefail

# ANSI Colors
BOLD='\033[1m'
NC='\033[0m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
RED='\033[0;31m'

banner() {
    echo -e "${CYAN}${BOLD}"
    echo "================================================================="
    echo "            AidBio AI Disk Manager - Installer                  "
    echo "      Authors: Cleverson Matiolli, PhD and Gemini               "
    echo "================================================================="
    echo -e "${NC}"
}

INSTALL_DIR="${HOME}/.local/share/aidbio/disk-manager"
BIN_DIR="${HOME}/.local/bin"

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

check_dependencies() {
    echo -e "${BOLD}Checking System Dependencies...${NC}"

    local missing=()
    for cmd in bash python3 rsync df du lsblk; do
        if ! command -v "$cmd" &>/dev/null; then
            missing+=("$cmd")
        else
            echo -e "  ✔ Found: ${GREEN}$cmd${NC}"
        fi
    done

    if [[ ${#missing[@]} -gt 0 ]]; then
        echo -e "${RED}ERROR: The following required dependencies are missing: ${missing[*]}${NC}"
        echo "Please install them via your system package manager (e.g. sudo apt install ${missing[*]})"
        exit 1
    fi

    # Check python psutil
    if ! python3 -c "import psutil" 2>/dev/null; then
        echo -e "${YELLOW}Notice: Python 'psutil' module is missing. Attempting to install via pip...${NC}"
        python3 -m pip install psutil --user 2>/dev/null || echo -e "${YELLOW}Warning: psutil auto-install skipped. Fallbacks will be used.${NC}"
    else
        echo -e "  ✔ Found: ${GREEN}python3 psutil${NC}"
    fi
}

configure_path() {
    echo -e "\n${BOLD}Configuring Shell PATH...${NC}"

    local path_line='export PATH="$HOME/.local/bin:$PATH"'
    local added=0

    # Add to PATH in current session if missing
    if [[ ":$PATH:" != *":$BIN_DIR:"* ]]; then
        export PATH="$BIN_DIR:$PATH"
    fi

    # Update shell configuration files
    for rc_file in "$HOME/.bashrc" "$HOME/.zshrc" "$HOME/.profile"; do
        if [[ -f "$rc_file" ]]; then
            if ! grep -qs 'local/bin' "$rc_file"; then
                echo -e "\n# Added by AidBio AI Disk Manager" >> "$rc_file"
                echo "$path_line" >> "$rc_file"
                echo -e "  ✔ Added PATH configuration to ${GREEN}$rc_file${NC}"
                added=1
            else
                echo -e "  ✔ PATH already configured in ${GREEN}$rc_file${NC}"
            fi
        fi
    done

    if [[ $added -eq 1 ]]; then
        echo -e "${YELLOW}Note: Restart your terminal session or run 'source ~/.bashrc' to apply PATH changes.${NC}"
    fi
}

install_app() {
    banner
    check_dependencies

    echo -e "\n${BOLD}Installing AidBio AI Disk Manager...${NC}"

    # Target directory structure
    mkdir -p "$INSTALL_DIR/lib" "$BIN_DIR"

    # Copy files
    if [[ -d "$SCRIPT_DIR/lib" ]]; then
        cp -r "$SCRIPT_DIR/lib"/* "$INSTALL_DIR/lib/"
        cp "$SCRIPT_DIR/clean-disk.sh" "$INSTALL_DIR/clean-disk.sh"
    elif [[ -f "$SCRIPT_DIR/clean-disk.sh" ]]; then
        cp "$SCRIPT_DIR/clean-disk.sh" "$INSTALL_DIR/clean-disk.sh"
        cp -r "$SCRIPT_DIR/lib" "$INSTALL_DIR/"
    else
        echo -e "${RED}ERROR: Source files not found in $SCRIPT_DIR${NC}"
        exit 1
    fi

    # Set executable permissions
    chmod +x "$INSTALL_DIR/clean-disk.sh" "$INSTALL_DIR/lib"/*.sh

    # Create launcher symlink
    ln -sf "$INSTALL_DIR/clean-disk.sh" "$BIN_DIR/clean-disk"
    echo -e "  ✔ Created symlink: ${GREEN}$BIN_DIR/clean-disk${NC} -> ${CYAN}$INSTALL_DIR/clean-disk.sh${NC}"

    configure_path

    echo -e "\n${GREEN}${BOLD}=================================================================${NC}"
    echo -e "${GREEN}${BOLD}   AidBio AI Disk Manager Installed Successfully!               ${NC}"
    echo -e "${GREEN}${BOLD}=================================================================${NC}"
    echo -e "\nYou can now run ${CYAN}${BOLD}clean-disk${NC} from any terminal directory."
    echo -e "Try running: ${YELLOW}clean-disk --help${NC}\n"
}

uninstall_app() {
    banner
    echo -e "${YELLOW}${BOLD}Uninstalling AidBio AI Disk Manager...${NC}"

    rm -rf "$INSTALL_DIR"
    rm -f "$BIN_DIR/clean-disk"

    echo -e "${GREEN}AidBio AI Disk Manager uninstalled successfully.${NC}"
}

if [[ "${1:-}" == "--uninstall" ]]; then
    uninstall_app
else
    install_app
fi
