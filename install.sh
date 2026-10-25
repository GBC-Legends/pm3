#!/bin/sh
# One daemon per Unix account, supervised by the system systemd manager.
set -eu
umask 022
REPO=GBC-Legends/pm3
PREFIX=/opt/pm3
TARGET_USER=${SUDO_USER:-root}
SOURCE=
VERSION=
YES=0
REMOVE=0
TMP=
OLD_RELEASE=
ACTIVE_UNITS=
CHANGED=0
UNIT_CHANGED=0
TARGET_EXISTS=0
TARGET_ENABLED=0
LEGACY_ACTIVE=0
LEGACY_ENABLED=0
LEGACY_MASKED=0
LEGACY_MIGRATED=0

info() { printf '%s\n' "$*" >&2; }
fail() { info "Error: $*"; exit 1; }
as_user() { runuser -u "$TARGET_USER" -- env -u PM3_HOME -u PM3_INSTANCE HOME="$TARGET_HOME" USER="$TARGET_USER" LOGNAME="$TARGET_USER" "$@"; }
legacy_ctl() { as_user env XDG_RUNTIME_DIR="/run/user/$TARGET_UID" DBUS_SESSION_BUS_ADDRESS="unix:path=/run/user/$TARGET_UID/bus" systemctl --user "$@"; }
cleanup() {
    status=$?
    trap - EXIT
    if [ "$status" -ne 0 ]; then
        # Restore the selected unit before restarting the old release.
        if [ "$UNIT_CHANGED" -eq 1 ]; then
            systemctl stop "$UNIT" || true
            if [ "$TARGET_ENABLED" -eq 0 ]; then systemctl disable "$UNIT" || true; fi
            if [ "$TARGET_EXISTS" -eq 1 ]; then
                cp -p "$TMP/previous.service" "$UNIT_PATH"
            else
                rm -f "$UNIT_PATH"
            fi
            systemctl daemon-reload || true
        fi
        if [ "$CHANGED" -eq 1 ] && [ -n "$OLD_RELEASE" ]; then
            info 'Restoring previous PM3 binaries and previously running system services.'
            ln -sfn "$OLD_RELEASE" "$PREFIX/current.rollback"
            mv -Tf "$PREFIX/current.rollback" "$PREFIX/current"
        fi
        if [ "$CHANGED" -eq 1 ]; then
            for service in $ACTIVE_UNITS; do systemctl restart "$service" || true; done
        fi
        if [ "$LEGACY_MASKED" -eq 1 ]; then
            as_user rm -f "$TARGET_HOME/.config/systemd/user/pm3.service"
            if [ -S "/run/user/$TARGET_UID/bus" ]; then
                legacy_ctl daemon-reload || true
                if [ "$LEGACY_ENABLED" -eq 1 ]; then legacy_ctl enable pm3.service || true; fi
                if [ "$LEGACY_ACTIVE" -eq 1 ]; then legacy_ctl start pm3.service || true; fi
            fi
            info 'The legacy per-user service configuration has been restored.'
        fi
    fi
    [ -z "$TMP" ] || rm -rf -- "$TMP"
    exit "$status"
}
trap cleanup EXIT
trap 'exit 130' INT
trap 'exit 143' TERM
usage() {
    cat <<'HELP'
Usage: sudo sh install.sh [options]
  --user USER       Account to install (default: SUDO_USER, otherwise root)
  --from-dir DIR    Local bundle containing pm3, pm3-daemon and dashboard/
  --version TAG     GitHub release tag (default: latest)
  --uninstall       Stop and disable this user's system service; preserve ~/.pm3
  -y, --yes         Run without confirmation prompts
  -h, --help        Show help

Examples:
  sudo sh install.sh --yes                  # pm3-<sudo caller>.service
  sudo sh install.sh --yes --user root      # pm3-root.service
  sudo sh install.sh --yes --user ubuntu    # pm3-ubuntu.service

Each service runs as its named user and keeps that user's ~/.pm3.
Use pm3 as that user, or sudo -H -u USER pm3 list.
Shared binary updates save/restart all running PM3 system services.
HELP
}
while [ "$#" -gt 0 ]; do
    case "$1" in
        --user|--from-dir|--version)
            [ "$#" -ge 2 ] && [ -n "$2" ] || fail "$1 requires a value"
            case "$1" in --user) TARGET_USER=$2;; --from-dir) SOURCE=$2;; --version) VERSION=$2;; esac
            shift;;
        --uninstall) REMOVE=1;;
        -y|--yes) YES=1;;
        -h|--help) usage; exit 0;;
        *) fail "unknown option: $1";;
    esac
    shift
done
case "$TARGET_USER" in ''|*[!a-zA-Z0-9_.-]*|[-.]*|[0-9]*) fail 'unsupported Unix account name';; esac
[ "$(uname -s)" = Linux ] || fail 'Linux is required'
[ "$(id -u)" = 0 ] || fail 'run this installer with sudo or as root'
for cmd in systemctl runuser getent id install mktemp flock readlink cp mv ln awk sed grep tar gzip env cut head chmod chown mkdir rm; do
    command -v "$cmd" >/dev/null 2>&1 || fail "missing command: $cmd"
done
[ -d /run/systemd/system ] || fail 'a running systemd system manager is required'
getent passwd "$TARGET_USER" > /dev/null || fail "unknown account: $TARGET_USER"
TARGET_UID=$(id -u "$TARGET_USER")
TARGET_GROUP=$(id -gn "$TARGET_USER")
TARGET_HOME=$(getent passwd "$TARGET_USER" | cut -d: -f6)
case "$TARGET_HOME" in /*) ;; *) fail 'the account needs an absolute home directory';; esac
# Paths with newlines cannot be represented safely in the generated unit.
case "$TARGET_HOME" in *'
'*) fail 'home directory contains a newline';; esac
[ -d "$TARGET_HOME" ] || fail "home directory does not exist: $TARGET_HOME"
UNIT="pm3-$TARGET_USER.service"
UNIT_PATH="/etc/systemd/system/$UNIT"
STATE="$TARGET_HOME/.pm3"
exec 9>/run/pm3-install.lock
flock -n 9 || fail 'another PM3 installation is in progress'
confirm() {
    [ "$YES" -eq 0 ] || return 0
    printf '%s [y/N]: ' "$1" >&2
    answer=
    read -r answer < /dev/tty || true
    case "$answer" in y|Y|yes) ;; *) fail 'cancelled (use --yes for unattended installation)';; esac
}
if [ -e "$UNIT_PATH" ]; then
    grep -q '^# Managed by PM3 per-user installer$' "$UNIT_PATH" || fail "refusing to overwrite an unmanaged unit: $UNIT_PATH"
    TARGET_EXISTS=1
fi
if [ "$REMOVE" -eq 1 ]; then
    [ "$TARGET_EXISTS" -eq 1 ] || fail "no managed service installed for $TARGET_USER"
    confirm "Stop and disable $UNIT, preserving $STATE?"
    systemctl disable --now "$UNIT"
    rm -f "$UNIT_PATH"
    systemctl daemon-reload
    info "Removed $UNIT; retained $STATE and shared binaries."
    exit 0
fi

# Recognize the original installer. Never start a second daemon for this account.
LEGACY=0
if [ -f /etc/systemd/user/pm3.service ] && [ "$TARGET_EXISTS" -eq 0 ]; then
    local_unit="$TARGET_HOME/.config/systemd/user/pm3.service"
    if [ "$(readlink "$local_unit" 2>/dev/null || true)" != /dev/null ]; then
        [ ! -e "$local_unit" ] && [ ! -L "$local_unit" ] || fail 'custom user unit found; stop/disable and back it up before installing the system service'
        LEGACY=1
        if [ -S "/run/user/$TARGET_UID/bus" ]; then
            if legacy_ctl is-active --quiet pm3.service; then LEGACY_ACTIVE=1; fi
            if legacy_ctl is-enabled --quiet pm3.service; then LEGACY_ENABLED=1; fi
        elif as_user /usr/bin/pm3/pm3 ping 2>/dev/null | grep -q pong; then
            fail 'the old daemon is running but its user manager is unavailable; stop it before migrating'
        fi
    fi
fi
confirm "Install $UNIT as $TARGET_USER with state in $STATE (and update shared PM3 binaries)?"
TMP=$(mktemp -d)
if [ -n "$SOURCE" ]; then
    SOURCE=$(cd "$SOURCE" && pwd)
    cp -R "$SOURCE/." "$TMP/bundle"
    VERSION=${VERSION:-local}
else
    command -v curl >/dev/null || fail 'missing command: curl'
    case "$(uname -m)" in x86_64|amd64) ARCH=x86_64;; aarch64|arm64) ARCH=aarch64;; *) fail 'unsupported CPU architecture';; esac
    case "$VERSION" in *[!a-zA-Z0-9._-]*) fail 'invalid release tag';; esac
    api="https://api.github.com/repos/$REPO/releases/latest"
    [ -z "$VERSION" ] || api="https://api.github.com/repos/$REPO/releases/tags/$VERSION"
    curl -fsSL "$api" -o "$TMP/release.json"
    VERSION=$(sed -n 's/^[[:space:]]*"tag_name":[[:space:]]*"\([^"]*\)".*/\1/p' "$TMP/release.json" | head -n 1)
    url=$(sed -n "s/^[[:space:]]*\"browser_download_url\":[[:space:]]*\"\([^\"]*pm3-linux-$ARCH.tar.gz\)\".*/\1/p" "$TMP/release.json" | head -n 1)
    [ -n "$VERSION" ] && [ -n "$url" ] || fail "no Linux $ARCH archive in this release"
    curl -fL "$url" -o "$TMP/bundle.tar.gz"
    mkdir "$TMP/bundle"
    tar -xzf "$TMP/bundle.tar.gz" --no-same-owner -C "$TMP/bundle"
fi
BUNDLE="$TMP/bundle"
[ -f "$BUNDLE/pm3" ] && [ -f "$BUNDLE/pm3-daemon" ] && [ -f "$BUNDLE/dashboard/index.html" ] || fail 'incomplete release bundle'
chmod 0755 "$BUNDLE/pm3" "$BUNDLE/pm3-daemon"
[ "$("$BUNDLE/pm3" --installation-model)" = per-user-systemd-v1 ] || fail 'incompatible release; build this branch or choose a release with per-user system services'
"$BUNDLE/pm3-daemon" --version >/dev/null || fail 'daemon cannot run on this machine'
if [ -L "$PREFIX/current" ]; then OLD_RELEASE=$(readlink -f "$PREFIX/current"); fi
if [ "$TARGET_EXISTS" -eq 1 ]; then cp -p "$UNIT_PATH" "$TMP/previous.service"; fi
if systemctl is-enabled --quiet "$UNIT"; then TARGET_ENABLED=1; fi
# Include only services created by this installer, not unrelated pm3-* units.
systemctl list-units --type=service --state=active --no-legend 'pm3-*.service' > "$TMP/active"
for service in $(awk '{print $1}' "$TMP/active"); do
    if grep -q '^# Managed by PM3 per-user installer$' "/etc/systemd/system/$service" 2>/dev/null; then
        ACTIVE_UNITS="$ACTIVE_UNITS $service"
    fi
done
save_user() {
    account=$1
    binary=$2
    home=$(getent passwd "$account" | cut -d: -f6)
    runuser -u "$account" -- env HOME="$home" "$binary" dump > "$TMP/dump"
    grep -q 'configs saved to' "$TMP/dump" || fail "failed to save processes for $account"
}
for service in $ACTIVE_UNITS; do
    account=${service#pm3-}; account=${account%.service}
    save_user "$account" "$PREFIX/current/pm3"
done
if [ "$LEGACY_ACTIVE" -eq 1 ]; then save_user "$TARGET_USER" /usr/bin/pm3/pm3; fi

# Stage all release files before stopping or changing any service.
install -d -m 0755 "$PREFIX/releases" /usr/local/bin /etc/systemd/system
RELEASE=$(mktemp -d "$PREFIX/releases/release.XXXXXXXX")
install -m 0755 "$BUNDLE/pm3" "$BUNDLE/pm3-daemon" "$RELEASE/"
cp -R "$BUNDLE/dashboard" "$RELEASE/dashboard"
chmod -R a+rX "$RELEASE"
printf '%s\n' "$VERSION" > "$RELEASE/pm3.version"
if [ -f "$BUNDLE/install.sh" ]; then install -m 0755 "$BUNDLE/install.sh" "$RELEASE/install.sh";
elif [ -f "$0" ]; then install -m 0755 "$0" "$RELEASE/install.sh"; fi
# Older manually launched daemons do not have our lock. Do not start a second
# writer merely because no system/user unit claims the running process.
case " $ACTIVE_UNITS " in
    *" $UNIT "*) ;;
    *) if [ "$LEGACY_ACTIVE" -eq 0 ] && as_user "$RELEASE/pm3" ping 2>/dev/null | grep -q pong; then
           fail "a daemon for $TARGET_USER is already running outside the managed service; save and stop it before installing"
       fi;;
esac
if [ "$LEGACY" -eq 1 ]; then
    # Mask only this account. Other users can be migrated independently.
    LEGACY_MASKED=1
    if [ -S "/run/user/$TARGET_UID/bus" ]; then
        legacy_ctl disable --now pm3.service
    fi
    as_user mkdir -p "$TARGET_HOME/.config/systemd/user"
    as_user ln -s /dev/null "$TARGET_HOME/.config/systemd/user/pm3.service"
    if [ -S "/run/user/$TARGET_UID/bus" ]; then
        legacy_ctl daemon-reload
    fi
    LEGACY_MIGRATED=1
fi
# Do filesystem work in the target account, never chown another user's entire home.
as_user /bin/sh -c 'umask 077; mkdir -p "$HOME/.pm3"; chmod 700 "$HOME/.pm3"'
# Quote systemd values (including literal % specifiers and whitespace in HOME).
unit_home=$(printf '%s' "$TARGET_HOME" | sed 's/\\/\\\\/g; s/"/\\"/g; s/%/%%/g')
cat > "$TMP/new.service" <<UNIT_FILE
# Managed by PM3 per-user installer
[Unit]
Description=PM3 daemon for $TARGET_USER
After=network.target
RequiresMountsFor="$unit_home"

[Service]
Type=simple
User=$TARGET_USER
Group=$TARGET_GROUP
Environment="HOME=$unit_home" "USER=$TARGET_USER" "LOGNAME=$TARGET_USER" "PM3_RESTORE=1"
WorkingDirectory=$PREFIX/current
ExecStart=$PREFIX/current/pm3-daemon
Restart=on-failure
RestartSec=3
KillMode=mixed
TimeoutStopSec=30
UMask=0077

[Install]
WantedBy=multi-user.target
UNIT_FILE
UNIT_CHANGED=1
install -m 0644 "$TMP/new.service" "$UNIT_PATH"
ln -sfn "$RELEASE" "$PREFIX/current.next"
mv -Tf "$PREFIX/current.next" "$PREFIX/current"
CHANGED=1
ln -sfn "$PREFIX/current/pm3" /usr/local/bin/pm3
systemctl daemon-reload
systemctl enable "$UNIT"
RESTART_UNITS=$ACTIVE_UNITS
case " $RESTART_UNITS " in *" $UNIT "*) ;; *) RESTART_UNITS="$RESTART_UNITS $UNIT";; esac
for service in $RESTART_UNITS; do
    systemctl restart "$service"
    account=${service#pm3-}; account=${account%.service}
    home=$(getent passwd "$account" | cut -d: -f6)
    ready=0
    for attempt in 1 2 3 4 5 6 7 8 9 10; do
        if runuser -u "$account" -- env HOME="$home" "$PREFIX/current/pm3" ping 2>/dev/null | grep -q pong; then ready=1; break; fi
        sleep 1
    done
    [ "$ready" -eq 1 ] || fail "$service did not become ready; inspect journalctl -u $service"
done
# Old installs prepended /usr/bin/pm3 to each shell's PATH. Keep that exact CLI
# path working, without editing profiles or touching other users' old daemons.
if [ -f /etc/systemd/user/pm3.service ] && [ -f /usr/bin/pm3/pm3 ]; then
    if [ ! -e /usr/bin/pm3/pm3.legacy ]; then cp -p /usr/bin/pm3/pm3 /usr/bin/pm3/pm3.legacy; fi
    ln -sfn /usr/local/bin/pm3 /usr/bin/pm3/pm3
    if [ -f /usr/bin/pm3/install.sh ] && [ -f "$PREFIX/current/install.sh" ]; then
        if [ ! -e /usr/bin/pm3/install.sh.legacy ]; then cp -p /usr/bin/pm3/install.sh /usr/bin/pm3/install.sh.legacy; fi
        ln -sfn "$PREFIX/current/install.sh" /usr/bin/pm3/install.sh
    fi
fi
CHANGED=0
UNIT_CHANGED=0
LEGACY_MASKED=0
info "Installed $UNIT. Daemon and applications run as $TARGET_USER."
info "State and settings: $STATE (settings: pm3.env)."
info "CLI: sudo -H -u $TARGET_USER /usr/local/bin/pm3 list"
info "Service: sudo systemctl status $UNIT; logs: sudo journalctl -u $UNIT"
info 'Use pm3 save after changing processes to restore them at boot.'
if [ "$LEGACY_MIGRATED" -eq 1 ]; then
    info 'Legacy user service disabled for this account; ~/.pm3 was retained in place.'
    info 'The old /usr/bin/pm3/pm3 CLI path now forwards to the current CLI; the original binary is preserved as pm3.legacy.'
fi
