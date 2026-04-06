#!/usr/bin/env bash
set -euo pipefail

REPO="GBC-Legends/pm3"

INSTALL_ROOT="/usr/bin/pm3"
BIN_LINK="/usr/bin/pm3"

SYSTEMD_USER_DIR=".config/systemd/user"
SERVICE_NAME="pm3.service"

TMP_DIR=""

info()    { printf '\033[1;34m%s\033[0m\n' "$*"; }
success() { printf '\033[1;32m%s\033[0m\n' "$*"; }
warn()    { printf '\033[1;33m%s\033[0m\n' "$*"; }
error()   { printf '\033[1;31mError: %s\033[0m\n' "$*" >&2; exit 1; }

cleanup() {
    [ -n "${TMP_DIR:-}" ] && rm -rf "$TMP_DIR"
}
trap cleanup EXIT

require_root() {
    [ "$EUID" -ne 0 ] && error "Run as root (sudo)"
}

require_cmd() {
    command -v "$1" >/dev/null 2>&1 || error "Missing: $1"
}

check_requirements() {
    require_cmd curl
    require_cmd tar
    require_cmd systemctl
    require_cmd grep
}

get_latest_url() {
    info "Fetching latest release..."

    local url
    url=$(curl -s https://api.github.com/repos/$REPO/releases/latest \
        | grep "browser_download_url" \
        | grep "linux-x86_64.tar.gz" \
        | cut -d '"' -f 4)

    [ -z "$url" ] && error "Failed to get latest release URL"

    echo "$url"
}

download_and_extract() {
    TMP_DIR="$(mktemp -d)"
    local archive="$TMP_DIR/pm3.tar.gz"
    local extract="$TMP_DIR/extracted"

    mkdir -p "$extract"

    local url
    url="$(get_latest_url)"

    info "Downloading: $url"
    curl -fL "$url" -o "$archive"

    info "Extracting..."
    tar -xzf "$archive" -C "$extract"

    echo "$extract"
}

find_root() {
    local dir="$1"

    if [ -f "$dir/pm3" ]; then
        echo "$dir"
        return
    fi

    local found
    found="$(find "$dir" -type f -name pm3 | head -n 1)"

    [ -z "$found" ] && error "pm3 binary not found"

    dirname "$found"
}

install_files() {
    local src="$1"

    info "Installing to $INSTALL_ROOT"

    rm -rf "$INSTALL_ROOT"
    mkdir -p "$INSTALL_ROOT"

    cp "$src/pm3" "$INSTALL_ROOT/"
    cp "$src/pm3-daemon" "$INSTALL_ROOT/"
    chmod +x "$INSTALL_ROOT/"*

    if [ -d "$src/dashboard" ]; then
        cp -r "$src/dashboard" "$INSTALL_ROOT/"
    fi

    ln -sf "$INSTALL_ROOT/pm3" "$BIN_LINK"
}

create_user_service() {
    local user="$1"
    local home_dir
    home_dir="$(eval echo "~$user")"

    local dir="$home_dir/$SYSTEMD_USER_DIR"
    mkdir -p "$dir"

    cat > "$dir/$SERVICE_NAME" <<EOF
[Unit]
Description=pm3 daemon
After=network.target

[Service]
Type=simple
ExecStart=$INSTALL_ROOT/pm3-daemon
Restart=always
RestartSec=3
WorkingDirectory=$INSTALL_ROOT
LimitNOFILE=65536

[Install]
WantedBy=default.target
EOF

    chown -R "$user:$user" "$home_dir/.config"
}

enable_for_user() {
    local user="$1"

    info "Enable for $user"

    sudo -u "$user" systemctl --user daemon-reload || true
    sudo -u "$user" systemctl --user enable pm3 || true
    sudo -u "$user" systemctl --user restart pm3 || true

    loginctl enable-linger "$user" >/dev/null 2>&1 || true
}

install_for_all_users() {
    for user in $(ls /home); do
        create_user_service "$user"
        enable_for_user "$user"
    done
}

main() {
    require_root
    check_requirements

    local extracted
    extracted="$(download_and_extract)"

    local root
    root="$(find_root "$extracted")"

    install_files "$root"
    install_for_all_users

    success "Installed latest pm3"
    success "Command: pm3"
    success "Daemon isolated per user"
}

main "$@"
