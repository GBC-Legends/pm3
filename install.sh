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
    require_cmd getent
    require_cmd id
    require_cmd pkill
    require_cmd systemctl
    require_cmd loginctl
    require_cmd su
    require_cmd tr
}

trim_spaces() {
    printf '%s' "$1" | sed 's/^ *//;s/ *$//'
}

normalize_csv_to_lines() {
    printf '%s' "$1" | tr ',' '\n' | while IFS= read -r item; do
        item="$(trim_spaces "$item")"
        [ -n "$item" ] && printf '%s\n' "$item"
    done
}

user_exists() {
    id "$1" >/dev/null 2>&1
}

user_home() {
    getent passwd "$1" | cut -d: -f6
}

user_uid() {
    id -u "$1"
}

default_target_users() {
    if [ -n "${PM3_ENABLE_USERS:-}" ]; then
        normalize_csv_to_lines "$PM3_ENABLE_USERS"
        return
    fi

    found_any=0

    if [ -f "$SERVICE_PATH" ]; then
        getent passwd | while IFS=: read -r name _ uid _ _ home _; do
            [ -n "$name" ] || continue
            if [ -e "$home/.config/systemd/user/default.target.wants/$SERVICE_NAME" ]; then
                printf '%s\n' "$name"
                found_any=1
            fi
        done
    fi

    # shellcheck can't see subshell state; use a fallback re-scan outside
    enabled_users_tmp="${TMP_DIR:-}/enabled_users.txt"
    : > "$enabled_users_tmp" 2>/dev/null || true
    getent passwd | while IFS=: read -r name _ uid _ _ home _; do
        [ -n "$name" ] || continue
        if [ -e "$home/.config/systemd/user/default.target.wants/$SERVICE_NAME" ]; then
            printf '%s\n' "$name" >> "$enabled_users_tmp"
        fi
    done

    if [ -s "$enabled_users_tmp" ]; then
        cat "$enabled_users_tmp"
        return
    fi

    if [ -n "${SUDO_USER:-}" ] && [ "${SUDO_USER:-}" != "root" ]; then
        printf 'root\n%s\n' "$SUDO_USER"
    else
        printf 'root\n'
    fi
}

confirm_replace_if_installed() {
    if [ -e "$CLI_PATH" ] || [ -e "$DAEMON_PATH" ] || [ -d "$DASHBOARD_DIR" ]; then
        if [ "${PM3_YES:-0}" = "1" ]; then
            info "existing installation found, updating automatically"
            return
        fi

        printf 'pm3 is already installed in %s\n' "$INSTALL_DIR" >&2
        printf 'Delete current binaries/dashboard and install latest release over them? [y/N]: ' >&2
        read ans || true

        case "${ans:-}" in
            y|Y|yes|YES) info "updating existing installation" ;;
            *) error "installation cancelled" ;;
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
    [ -n "$FOUND" ] && printf '%s\n' "$FOUND"
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

    cat > "$SERVICE_PATH" <<'EOF'
[Unit]
Description=pm3 daemon
After=network.target

[Service]
Type=simple
EnvironmentFile=-%h/.config/pm3/daemon.env
ExecStart=/bin/sh -lc 'exec /usr/bin/pm3/pm3-daemon ${PM3_DAEMON_ARGS:-}'
Restart=always
RestartSec=3
WorkingDirectory=/usr/bin/pm3
LimitNOFILE=65536

[Install]
WantedBy=default.target
EOF
}

remove_old_global_enable() {
    info "removing old global pm3 auto-enable if present"
    systemctl --global disable "$SERVICE_NAME" >/dev/null 2>&1 || true
    rm -f "/etc/systemd/user/default.target.wants/$SERVICE_NAME" || true
}

write_user_env() {
    user="$1"
    mode="$2"

    home="$(user_home "$user")"
    [ -n "$home" ] || error "cannot resolve home for user $user"

    mkdir -p "$home/.config/pm3"

    if [ "$mode" = "public" ]; then
        cat > "$home/.config/pm3/daemon.env" <<EOF
PM3_DAEMON_ARGS=public
EOF
    else
        cat > "$home/.config/pm3/daemon.env" <<EOF
PM3_DAEMON_ARGS=
EOF
    fi

    chown -R "$user:$user" "$home/.config/pm3"
}

enable_user_unit() {
    user="$1"

    home="$(user_home "$user")"
    uid="$(user_uid "$user")"
    wants_dir="$home/.config/systemd/user/default.target.wants"
    user_unit_dir="$home/.config/systemd/user"

    mkdir -p "$wants_dir"
    mkdir -p "$user_unit_dir"

    ln -snf "$SERVICE_PATH" "$wants_dir/$SERVICE_NAME"
    chown -R "$user:$user" "$home/.config/systemd"

    loginctl enable-linger "$user" >/dev/null 2>&1 || true
    systemctl start "user@$uid.service" >/dev/null 2>&1 || true

    if [ -S "/run/user/$uid/bus" ]; then
        su - "$user" -s /bin/sh -c "
            export XDG_RUNTIME_DIR=/run/user/$uid
            export DBUS_SESSION_BUS_ADDRESS=unix:path=/run/user/$uid/bus
            systemctl --user daemon-reload || true
            systemctl --user enable $SERVICE_NAME >/dev/null 2>&1 || true
            systemctl --user restart $SERVICE_NAME || true
        " || true
    fi
}

disable_user_unit() {
    user="$1"

    if ! user_exists "$user"; then
        return
    fi

    home="$(user_home "$user")"
    uid="$(user_uid "$user")"

    rm -f "$home/.config/systemd/user/default.target.wants/$SERVICE_NAME" || true

    if [ -S "/run/user/$uid/bus" ]; then
        su - "$user" -s /bin/sh -c "
            export XDG_RUNTIME_DIR=/run/user/$uid
            export DBUS_SESSION_BUS_ADDRESS=unix:path=/run/user/$uid/bus
            systemctl --user disable --now $SERVICE_NAME >/dev/null 2>&1 || true
            systemctl --user daemon-reload || true
        " || true
    fi

    pkill -u "$user" -f "$DAEMON_PATH" >/dev/null 2>&1 || true
}

user_is_public() {
    user="$1"

    if [ -z "${PM3_PUBLIC_USERS:-}" ]; then
        return 1
    fi

    normalize_csv_to_lines "$PM3_PUBLIC_USERS" | while IFS= read -r line; do
        [ "$line" = "$user" ] && exit 0
    done

    return 1
}

sync_users() {
    desired_tmp="$TMP_DIR/desired_users.txt"
    current_tmp="$TMP_DIR/current_users.txt"

    : > "$desired_tmp"
    : > "$current_tmp"

    default_target_users | while IFS= read -r u; do
        [ -n "$u" ] || continue
        if user_exists "$u"; then
            printf '%s\n' "$u"
        else
            warn "skipping unknown user: $u"
        fi
    done | sort -u > "$desired_tmp"

    getent passwd | while IFS=: read -r name _ uid _ _ home _; do
        [ -n "$name" ] || continue
        if [ -e "$home/.config/systemd/user/default.target.wants/$SERVICE_NAME" ]; then
            printf '%s\n' "$name"
        fi
    done | sort -u > "$current_tmp"

    info "configuring pm3 users"

    while IFS= read -r user; do
        [ -n "$user" ] || continue

        mode="private"
        if user_is_public "$user"; then
            mode="public"
        fi

        info "enable pm3 for $user ($mode)"
        write_user_env "$user" "$mode"
        enable_user_unit "$user"
    done < "$desired_tmp"

    while IFS= read -r user; do
        [ -n "$user" ] || continue
        if ! grep -qx "$user" "$desired_tmp"; then
            info "disable pm3 for no-longer-selected user $user"
            disable_user_unit "$user"
        fi
    done < "$current_tmp"
}

print_summary() {
    success "pm3 installed/updated successfully"
    success "files dir: $INSTALL_DIR"
    success "cli: $CLI_PATH"
    success "daemon: $DAEMON_PATH"
    success "dashboard: $DASHBOARD_DIR"
    success "user service: $SERVICE_PATH"
    success "PATH is added via $PROFILED_FILE"

    if [ -n "${PM3_ENABLE_USERS:-}" ]; then
        success "selected users: $PM3_ENABLE_USERS"
    else
        success "selected users: auto-detected/preserved"
    fi

    if [ -n "${PM3_PUBLIC_USERS:-}" ]; then
        success "public users: $PM3_PUBLIC_USERS"
    else
        success "public users: none (default is private)"
    fi

    success "relogin or run: . $PROFILED_FILE"
}

main() {
    check_requirements
    confirm_replace_if_installed

    EXTRACTED="$(download_and_extract)"

    install_files "$EXTRACTED"
    install_path_env
    write_systemd_user_unit
    remove_old_global_enable
    sync_users
    print_summary

    info "start pm3-daemon: systemctl --user start pm3"
    info "autostart pm3-daemon: systemctl --user enable pm3"
}

main "$@"
