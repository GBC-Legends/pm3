#!/bin/sh
set -eu

REPO="GBC-Legends/pm3"
API_URL="https://api.github.com/repos/$REPO/releases/latest"

INSTALL_DIR="/usr/bin/pm3"
CLI_PATH="$INSTALL_DIR/pm3"
DAEMON_PATH="$INSTALL_DIR/pm3-daemon"
DASHBOARD_DIR="$INSTALL_DIR/dashboard"

PROFILED_DIR="/etc/profile.d"
PROFILED_FILE="$PROFILED_DIR/pm3_path.sh"

SYSTEMD_USER_UNIT_DIR="/etc/systemd/user"
SERVICE_NAME="pm3.service"
SERVICE_PATH="$SYSTEMD_USER_UNIT_DIR/$SERVICE_NAME"

TMP_DIR=""

info() {
    printf '%s\n' "$*" >&2
}

warn() {
    printf 'WARN: %s\n' "$*" >&2
}

success() {
    printf '%s\n' "$*" >&2
}

error() {
    printf 'Error: %s\n' "$*" >&2
    exit 1
}

cleanup() {
    if [ -n "${TMP_DIR:-}" ] && [ -d "$TMP_DIR" ]; then
        rm -rf "$TMP_DIR"
    fi
}
trap cleanup EXIT INT TERM

require_root() {
    if [ "$(id -u)" -ne 0 ]; then
        error "run as root"
    fi
}

require_cmd() {
    command -v "$1" >/dev/null 2>&1 || error "missing command: $1"
}

check_requirements() {
    require_root
    require_cmd curl
    require_cmd tar
    require_cmd grep
    require_cmd cut
    require_cmd head
    require_cmd mktemp
    require_cmd install
    require_cmd cp
    require_cmd rm
    require_cmd mkdir
    require_cmd find
    require_cmd systemctl
    require_cmd loginctl
    require_cmd getent
    require_cmd su
}

confirm_replace_if_installed() {
    if [ -e "$CLI_PATH" ] || [ -e "$DAEMON_PATH" ] || [ -d "$DASHBOARD_DIR" ]; then
        printf 'pm3 is already installed in %s\n' "$INSTALL_DIR" >&2
        printf 'Delete current binaries/dashboard and install the latest release over them? [y/N]: ' >&2
        read ans || true

        case "${ans:-}" in
            y|Y|yes|YES)
                info "updating existing installation"
                ;;
            *)
                error "installation cancelled"
                ;;
        esac
    fi
}

get_latest_release_url() {
    curl -fsSL "$API_URL" \
        | grep '"browser_download_url"' \
        | grep 'pm3-linux-x86_64\.tar\.gz' \
        | head -n 1 \
        | cut -d '"' -f 4
}

download_and_extract() {
    TMP_DIR="$(mktemp -d)"
    ARCHIVE="$TMP_DIR/pm3.tar.gz"
    EXTRACT_DIR="$TMP_DIR/extracted"

    mkdir -p "$EXTRACT_DIR"

    info "fetching latest release url..."
    URL="$(get_latest_release_url)"
    [ -n "$URL" ] || error "failed to resolve latest release url"

    info "downloading $URL"
    curl -fL "$URL" -o "$ARCHIVE"

    info "extracting archive"
    tar -xzf "$ARCHIVE" -C "$EXTRACT_DIR"

    printf '%s\n' "$EXTRACT_DIR"
}

find_binary() {
    BASE="$1"
    NAME="$2"

    FOUND="$(find "$BASE" -type f -name "$NAME" 2>/dev/null | head -n 1 || true)"
    [ -n "$FOUND" ] || error "$NAME not found inside archive"

    printf '%s\n' "$FOUND"
}

find_dashboard_dir() {
    BASE="$1"
    FOUND="$(find "$BASE" -type d -name dashboard 2>/dev/null | head -n 1 || true)"
    if [ -n "$FOUND" ]; then
        printf '%s\n' "$FOUND"
    fi
}

install_files() {
    BASE="$1"

    PM3_BIN="$(find_binary "$BASE" pm3)"
    PM3_DAEMON_BIN="$(find_binary "$BASE" pm3-daemon)"
    DASH_SRC="$(find_dashboard_dir "$BASE" || true)"

    info "preparing install dir $INSTALL_DIR"
    mkdir -p "$INSTALL_DIR"

    info "removing old binaries"
    rm -f "$CLI_PATH" "$DAEMON_PATH"
    rm -rf "$DASHBOARD_DIR"

    info "installing binaries"
    install -m 0755 "$PM3_BIN" "$CLI_PATH"
    install -m 0755 "$PM3_DAEMON_BIN" "$DAEMON_PATH"

    if [ -n "${DASH_SRC:-}" ] && [ -d "$DASH_SRC" ]; then
        info "installing dashboard"
        cp -R "$DASH_SRC" "$DASHBOARD_DIR"
    else
        warn "dashboard directory not found in archive"
    fi
}

install_path_env() {
    info "adding $INSTALL_DIR to PATH"

    mkdir -p "$PROFILED_DIR"

    cat > "$PROFILED_FILE" <<EOF
# added by pm3 installer
case ":\$PATH:" in
    *:$INSTALL_DIR:*) ;;
    *) export PATH="$INSTALL_DIR:\$PATH" ;;
esac
EOF

    chmod 0644 "$PROFILED_FILE"
}

write_systemd_user_unit() {
    info "writing user unit to $SERVICE_PATH"
    mkdir -p "$SYSTEMD_USER_UNIT_DIR"

    cat > "$SERVICE_PATH" <<EOF
[Unit]
Description=pm3 daemon
After=network.target

[Service]
Type=simple
ExecStart=$DAEMON_PATH
Restart=always
RestartSec=3
WorkingDirectory=$INSTALL_DIR
LimitNOFILE=65536

[Install]
WantedBy=default.target
EOF
}

enable_global_user_unit() {
    info "enabling global user unit"
    systemctl --global enable "$SERVICE_NAME" >/dev/null 2>&1 || true
}

enable_linger_for_real_users() {
    info "enabling linger for regular users"

    getent passwd | while IFS=: read -r name _ uid _ _ home _; do
        case "$uid" in
            ''|*[!0-9]*) continue ;;
        esac

        if [ "$uid" -ge 1000 ] && [ "$uid" -lt 60000 ] && [ -d "$home" ]; then
            loginctl enable-linger "$name" >/dev/null 2>&1 || true
        fi
    done
}

restart_running_user_services() {
    info "restarting pm3 for users with active user manager"

    getent passwd | while IFS=: read -r name _ uid _ _ home _; do
        case "$uid" in
            ''|*[!0-9]*) continue ;;
        esac

        if [ "$uid" -ge 1000 ] && [ "$uid" -lt 60000 ] && [ -d "$home" ]; then
            XDG_RUNTIME_DIR="/run/user/$uid"

            if [ -S "$XDG_RUNTIME_DIR/bus" ]; then
                su - "$name" -s /bin/sh -c "
                    export XDG_RUNTIME_DIR='$XDG_RUNTIME_DIR'
                    systemctl --user daemon-reload || true
                    systemctl --user enable $SERVICE_NAME >/dev/null 2>&1 || true
                    systemctl --user restart $SERVICE_NAME || true
                " || true
            fi
        fi
    done
}

main() {
    check_requirements
    confirm_replace_if_installed

    EXTRACTED="$(download_and_extract)"

    install_files "$EXTRACTED"
    install_path_env
    write_systemd_user_unit
    enable_global_user_unit
    enable_linger_for_real_users
    restart_running_user_services

    success "pm3 installed/updated successfully"
    success "files dir: $INSTALL_DIR"
    success "cli: $CLI_PATH"
    success "daemon: $DAEMON_PATH"
    success "dashboard: $DASHBOARD_DIR"
    success "user service: $SERVICE_PATH"
    success "PATH is added via $PROFILED_FILE"
    success "relogin or run: . $PROFILED_FILE"
}

main "$@"
