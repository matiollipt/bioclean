#!/usr/bin/env bash
# -----------------------------------------------------------------------------
# AidBio AI - History & Rollback Module
# Authors: Cleverson Matiolli, PhD and Gemini
# -----------------------------------------------------------------------------

HISTORY_DIR="$HOME/.aidbio/history"
HISTORY_FILE="$HISTORY_DIR/clean_disk_sessions.json"

history_init() {
    mkdir -p "$HISTORY_DIR"
    if [[ ! -f "$HISTORY_FILE" ]]; then
        echo '[]' > "$HISTORY_FILE"
    fi
}

# Add a session entry
history_start_session() {
    history_init
    local session_id="session_$(date +%Y%m%d_%H%M%S)"
    echo "$session_id"
}

# Record a migration operation
history_record_op() {
    local session_id="$1"
    local op_type="$2" # e.g. MOVE_AND_SYMLINK
    local src_path="$3"
    local dest_path="$4"

    history_init

    # Use python/jq to safely update JSON file
    python3 - <<EOF
import json, os

history_file = "$HISTORY_FILE"
session_id = "$session_id"
op_type = "$op_type"
src_path = "$src_path"
dest_path = "$dest_path"

with open(history_file, 'r') as f:
    data = json.load(f)

# Find or create session
session = None
for s in data:
    if s.get('session_id') == session_id:
        session = s
        break

if not session:
    session = {
        'session_id': session_id,
        'timestamp': "$(date -Iseconds)",
        'status': 'COMPLETED',
        'operations': []
    }
    data.append(session)

session['operations'].append({
    'type': op_type,
    'src': src_path,
    'dest': dest_path
})

with open(history_file, 'w') as f:
    json.dump(data, f, indent=2)
EOF
}

# Get latest active session
history_get_latest_active() {
    history_init
    python3 - <<EOF
import json
history_file = "$HISTORY_FILE"
try:
    with open(history_file, 'r') as f:
        data = json.load(f)
    active = [s for s in data if s.get('status') == 'COMPLETED' and s.get('operations')]
    if active:
        latest = active[-1]
        print(latest['session_id'])
    else:
        print("")
except Exception:
    print("")
EOF
}

# Display history summary
history_list() {
    history_init
    ui_section "Transaction Session History"
    python3 - <<EOF
import json
history_file = "$HISTORY_FILE"
try:
    with open(history_file, 'r') as f:
        data = json.load(f)
    if not data:
        print("No prior session history found.")
    for s in data:
        print(f"Session: {s.get('session_id')} | Status: {s.get('status')} | Date: {s.get('timestamp')}")
        for op in s.get('operations', []):
            print(f"   └─ [{op.get('type')}] {op.get('src')}  =>  {op.get('dest')}")
except Exception as e:
    print("Error reading history:", e)
EOF
}

# Revert last session
history_undo_last() {
    history_init
    local latest_id
    latest_id=$(history_get_latest_active)

    if [[ -z "$latest_id" ]]; then
        ui_warn "No active migration sessions available to revert."
        return 0
    fi

    ui_section "Undo / Revert Migration Session ($latest_id)"
    
    # Read operations for this session using python
    local ops_info
    ops_info=$(python3 - <<EOF
import json
with open("$HISTORY_FILE", 'r') as f:
    data = json.load(f)
for s in data:
    if s.get('session_id') == "$latest_id":
        for op in s.get('operations', []):
            print(f"{op.get('src')}|||{op.get('dest')}")
EOF
    )

    if [[ -z "$ops_info" ]]; then
        ui_warn "Session $latest_id has no recorded file operations."
        return 0
    fi

    echo -e "${YELLOW}Operations to revert:${NC}"
    while IFS='|||' read -r src dest; do
        echo -e "  • Symlink: ${CYAN}$src${NC}  <==  HDD Data: ${CYAN}$dest${NC}"
    done <<< "$ops_info"

    if ! ui_confirm "\nAre you sure you want to revert session $latest_id and restore data to SSD?"; then
        ui_info "Undo operation cancelled."
        return 0
    fi

    ui_section "Executing Rollback..."

    while IFS='|||' read -r src dest; do
        if [[ -L "$src" ]]; then
            ui_info "Removing symlink: $src"
            rm -f "$src"
        fi

        if [[ -d "$dest" ]]; then
            ui_info "Moving data back from $dest to $src..."
            mkdir -p "$(dirname "$src")"
            rsync -av --remove-source-files "$dest/" "$src/"
            rm -rf "$dest"
            ui_success "Restored: $src"
        elif [[ -f "$dest" ]]; then
            ui_info "Moving file back from $dest to $src..."
            mkdir -p "$(dirname "$src")"
            mv "$dest" "$src"
            ui_success "Restored file: $src"
        else
            ui_warn "Target data $dest not found on HDD; symlink removed."
        fi
    done <<< "$ops_info"

    # Mark session as REVERTED in JSON
    python3 - <<EOF
import json
with open("$HISTORY_FILE", 'r') as f:
    data = json.load(f)
for s in data:
    if s.get('session_id') == "$latest_id":
        s['status'] = 'REVERTED'
with open("$HISTORY_FILE", 'w') as f:
    json.dump(data, f, indent=2)
EOF

    ui_success "Session $latest_id successfully reverted!"
}
