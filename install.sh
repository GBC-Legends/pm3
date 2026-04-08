#!/bin/sh
set -eu

REPO="GBC-Legends/pm3"
API_URL="https://api.github.com/repos/$REPO/releases/latest"

INSTALL_DIR="/usr/bin/pm3"
CLI_PATH="$INSTALL_DIR/pm3"
DAEMON_PATH="$INSTALL_DIR/pm3-daemon"
DASHBOARD_DIR="$INSTALL_DIR/dashboard"
VERSION_PATH="$INSTALL_DIR/pm3.version"
INSTALLED_SCRIPT_PATH="$INSTALL_DIR/install.sh"

SYSTEMD_USER_UNIT_DIR="/etc/systemd/user"
SERVICE_NAME="pm3"
SERVICE_PATH="$SYSTEMD_USER_UNIT_DIR/$SERVICE_NAME.service"

TMP_DIR=""
TARGET_USER=""
TARGET_HOME=""
LATEST_VERSION=""
LATEST_URL=""
EXTRACT_DIR=""
ARCHIVE=""
SCRIPT_URL=""

info()    { printf '%s\n' "$*" >&2; }
warn()    { printf 'WARN: %s\n' "$*" >&2; }
success() { printf '%s\n' "$*" >&2; }
error()   { printf 'Error: %s\n' "$*" >&2; exit 1; }

cleanup() {
    if [ -n "${TMP_DIR:-}" ] && [ -d "$TMP_DIR" ]; then
        rm -rf "$TMP_DIR"
    fi
}
trap cleanup EXIT INT TERM

require_root() {
    if [ "$(id -u)" -ne 0 ]; then
        error "run this script as root"
    fi
}

require_cmd() {
    command -v "$1" >/dev/null 2>&1 || error "missing command: $1"
}

check_requirements() {
    require_root

    require_cmd curl
    require_cmd tar
    require_cmd mktemp
    require_cmd rm
    require_cmd mkdir
    require_cmd install
    require_cmd cp
    require_cmd sed
    require_cmd grep
    require_cmd head
    require_cmd getent
    require_cmd cut
    require_cmd id
    require_cmd chown
    require_cmd chmod
    require_cmd systemctl
    require_cmd su
    require_cmd loginctl
    require_cmd cat
}

resolve_target_user() {
    if [ -n "${SUDO_USER:-}" ] && [ "$SUDO_USER" != "root" ]; then
        TARGET_USER="$SUDO_USER"
    else
        TARGET_USER="root"
    fi

    TARGET_HOME="$(getent passwd "$TARGET_USER" | cut -d: -f6)"
    [ -n "$TARGET_HOME" ] || error "cannot resolve home directory for $TARGET_USER"
}

prompt_yes_no() {
    question="$1"

    printf '%s [y/N]: ' "$question" >&2
    read ans || true

    case "${ans:-}" in
        y|Y|yes|YES) return 0 ;;
        *) return 1 ;;
    esac
}

fetch_release_metadata() {
    curl -fsSL "$API_URL"
}

extract_version() {
    printf '%s\n' "$1" \
        | sed -n 's/^[[:space:]]*"tag_name":[[:space:]]*"\([^"]*\)".*/\1/p' \
        | head -n 1
}

extract_download_url() {
    printf '%s\n' "$1" \
        | sed -n 's/^[[:space:]]*"browser_download_url":[[:space:]]*"\([^"]*pm3-linux-x86_64\.tar\.gz\)".*/\1/p' \
        | head -n 1
}

check_existing_install() {
    if [ ! -d "$INSTALL_DIR" ]; then
        return
    fi

    if [ -f "$VERSION_PATH" ]; then
        INSTALLED_VERSION="$(sed -n '1p' "$VERSION_PATH")"

        if [ "$INSTALLED_VERSION" = "$LATEST_VERSION" ]; then
            success "pm3 $LATEST_VERSION is already installed in $INSTALL_DIR"
            success "Nothing to do."
            exit 0
        fi

        info "installed version: $INSTALLED_VERSION"
        info "latest version: $LATEST_VERSION"

        if ! prompt_yes_no "A different pm3 version is already installed. Update it?"; then
            success "Update skipped."
            exit 0
        fi
    else
        warn "pm3 is already installed in $INSTALL_DIR, but $VERSION_PATH is missing"

        if ! prompt_yes_no "Update the existing pm3 installation?"; then
            success "Update skipped."
            exit 0
        fi
    fi

    info "removing old installation"
    rm -rf "$INSTALL_DIR" "$SERVICE_PATH"
}

download_and_extract() {
    TMP_DIR="$(mktemp -d)"
    ARCHIVE="$TMP_DIR/pm3-linux-x86_64.tar.gz"
    EXTRACT_DIR="$TMP_DIR/extracted"

    mkdir -p "$EXTRACT_DIR"

    info "downloading $LATEST_URL"
    curl -fL "$LATEST_URL" -o "$ARCHIVE"

    info "extracting archive"
    tar -xzf "$ARCHIVE" -C "$EXTRACT_DIR"

    [ -f "$EXTRACT_DIR/pm3" ] || error "pm3 not found in release archive"
    [ -f "$EXTRACT_DIR/pm3-daemon" ] || error "pm3-daemon not found in release archive"

    if [ ! -d "$EXTRACT_DIR/dashboard" ]; then
        warn "dashboard directory not found in release archive"
    fi

}

install_files() {
    EXTRACT_DIR="$1"

    info "installing files to $INSTALL_DIR"
    mkdir -p "$INSTALL_DIR"

    install -m 0755 "$EXTRACT_DIR/pm3" "$CLI_PATH"
    install -m 0755 "$EXTRACT_DIR/pm3-daemon" "$DAEMON_PATH"

    if [ -d "$EXTRACT_DIR/dashboard" ]; then
        cp -R "$EXTRACT_DIR/dashboard" "$DASHBOARD_DIR"
    fi

    printf '%s\n' "$LATEST_VERSION" > "$VERSION_PATH"
    chmod 0644 "$VERSION_PATH"

    if curl -fsSL "$SCRIPT_URL" -o "$INSTALLED_SCRIPT_PATH"; then
        chmod 0755 "$INSTALLED_SCRIPT_PATH"
    elif [ -f "$0" ]; then
        install -m 0755 "$0" "$INSTALLED_SCRIPT_PATH"
    else
        warn "cannot copy install.sh to $INSTALLED_SCRIPT_PATH"
    fi
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
WorkingDirectory=$INSTALL_DIR
ExecStart=$DAEMON_PATH
Restart=always
RestartSec=3

[Install]
WantedBy=default.target
EOF

    chmod 0644 "$SERVICE_PATH"
}

update_path() {
    BASHRC_PATH="$TARGET_HOME/.bashrc"
    PATH_LINE='export PATH="/usr/bin/pm3:$PATH"'

    if grep -Fqx "$PATH_LINE" "$BASHRC_PATH" 2>/dev/null; then
        success "PATH is already configured in $BASHRC_PATH"
        success "Refresh the shell with: source ~/.bashrc"
        return
    fi

    info "updating $BASHRC_PATH"
    {
        printf '\n# pm3\n'
        printf '%s\n' "$PATH_LINE"
    } >> "$BASHRC_PATH"

    chown "$TARGET_USER" "$BASHRC_PATH"
    success "PATH updated. Refresh the shell with: source ~/.bashrc"
}

start_pm3_now() {
    uid="$(id -u "$TARGET_USER")"

    loginctl enable-linger "$TARGET_USER" >/dev/null 2>&1 || true
    systemctl start "user@$uid.service" >/dev/null 2>&1 || true

    if [ ! -S "/run/user/$uid/bus" ]; then
        warn "cannot access /run/user/$uid/bus"
        warn "start pm3 manually after login: systemctl --user start pm3"
        return
    fi

    info "starting pm3 for $TARGET_USER"
    su - "$TARGET_USER" -s /bin/sh -c "
        export XDG_RUNTIME_DIR=/run/user/$uid
        export DBUS_SESSION_BUS_ADDRESS=unix:path=/run/user/$uid/bus
        systemctl --user daemon-reload
        systemctl --user start pm3
    "

    success "pm3 started for $TARGET_USER"
    success "restart command: systemctl --user restart pm3"
}

print_summary() {
    success "pm3 $LATEST_VERSION installed successfully"
    success "cli: $CLI_PATH"
    success "daemon: $DAEMON_PATH"
    success "dashboard: $DASHBOARD_DIR"
    success "version file: $VERSION_PATH"
    success "install script copy: $INSTALLED_SCRIPT_PATH"
    success "systemd user unit: $SERVICE_PATH"
    success "pm3 settings, ports and other options: ~/.pm3/pm3.env"
}

main() {
    check_requirements
    resolve_target_user

    info "fetching latest release metadata"
    RELEASE_JSON="$(fetch_release_metadata)"
    LATEST_VERSION="$(extract_version "$RELEASE_JSON")"
    LATEST_URL="$(extract_download_url "$RELEASE_JSON")"
    SCRIPT_URL="https://raw.githubusercontent.com/$REPO/$LATEST_VERSION/install.sh"

    [ -n "$LATEST_VERSION" ] || error "failed to resolve latest version"
    [ -n "$LATEST_URL" ] || error "failed to resolve linux release archive url"

    check_existing_install

    download_and_extract
    install_files "$EXTRACT_DIR"
    write_systemd_user_unit

    if prompt_yes_no "Add /usr/bin/pm3 to PATH in ~/.bashrc for $TARGET_USER?"; then
        update_path
    else
        success "pm3 binary is available at $CLI_PATH"
    fi

    if prompt_yes_no "Start pm3 now for user $TARGET_USER?"; then
        start_pm3_now
    else
        success "you can start pm3 later with: systemctl --user start pm3"
    fi

    print_summary
}

main "$@"
