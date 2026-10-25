#!/bin/sh
# Linux system-wide installation. Run as root; see --help.
set -eu
umask 022
REPO=GBC-Legends/pm3
PREFIX=/opt/pm3
INSTANCE=default
RUN_AS=
FROM_DIR=
VERSION=
MIGRATE_USER=
ASSUME_YES=0
UNINSTALL=0
TMP_DIR=
RESTART_UNITS=
OLD_RELEASE=
SWITCHED=0

info() { printf '%s\n' "$*" >&2; }
error() { info "Error: $*"; exit 1; }
cleanup() {
    status=$?
    trap - EXIT
    if [ "$status" -ne 0 ] && [ "$SWITCHED" -eq 1 ] && [ -n "$OLD_RELEASE" ]; then
        info 'Update failed; restoring the previous release.'
        ln -sfn "$OLD_RELEASE" "$PREFIX/current.rollback"
        mv -Tf "$PREFIX/current.rollback" "$PREFIX/current"
        for unit in $RESTART_UNITS; do systemctl restart "$unit" || true; done
    fi
    [ -z "$TMP_DIR" ] || rm -rf -- "$TMP_DIR"
    exit "$status"
}
trap cleanup EXIT
trap 'exit 130' INT
trap 'exit 143' TERM

usage() {
    cat <<'HELP'
Usage: sudo sh install.sh [options]
  --instance NAME       System instance (default: default)
  --user USER           Account for managed processes (new instances: root)
  --migrate-user USER   Move one old user service/state into this new instance
  --from-dir DIR        Install a locally built release (pm3, pm3-daemon, dashboard/)
  --version TAG         GitHub release tag (default: latest)
  --uninstall           Stop/remove this instance's service; retain its data
  -y, --yes             Accept update/restart without an interactive prompt
  -h, --help            Show help
All users select the same instance with pm3 --instance NAME.
Control a root instance with sudo pm3. Data: /var/lib/pm3/NAME.
Updates replace the shared binaries and restart all active PM3 system instances.
HELP
}
while [ "$#" -gt 0 ]; do
    case "$1" in
        --instance|--user|--migrate-user|--from-dir|--version)
            [ "$#" -ge 2 ] && [ -n "$2" ] || error "$1 requires a value"
            case "$1" in
                --instance) INSTANCE=$2;;
                --user) RUN_AS=$2;;
                --migrate-user) MIGRATE_USER=$2;;
                --from-dir) FROM_DIR=$2;;
                --version) VERSION=$2;;
            esac
            shift;;
        -y|--yes) ASSUME_YES=1;;
        --uninstall) UNINSTALL=1;;
        -h|--help) usage; exit 0;;
        *) error "unknown argument: $1";;
    esac
    shift
done
case "$INSTANCE" in ''|*[!a-z0-9_-]*|[-_]*) error 'invalid instance name';; esac
[ "${#INSTANCE}" -le 32 ] || error 'instance name is longer than 32 characters'
[ "$(uname -s)" = Linux ] || error 'this installer requires Linux'
[ "$(id -u)" = 0 ] || error 'run as root (sudo sh install.sh)'
for cmd in systemctl install mktemp flock getent readlink tar cp mv ln awk sed grep find; do
    command -v "$cmd" >/dev/null 2>&1 || error "missing command: $cmd"
done
[ -d /run/systemd/system ] || error 'a running systemd system manager is required'
# Serialize installers across instances, including migrations and port allocation.
exec 9>/run/pm3-install.lock
flock -n 9 || error 'another PM3 installer is running'
UNIT="pm3@$INSTANCE.service"
STATE="/var/lib/pm3/$INSTANCE"
CONFIG="/etc/pm3/$INSTANCE.env"
DROPIN="/etc/systemd/system/$UNIT.d"

confirm() {
    [ "$ASSUME_YES" -eq 0 ] || return 0
    printf '%s [y/N]: ' "$1" >&2
    answer=
    read -r answer < /dev/tty || true
    case "$answer" in y|Y|yes) return 0;; *) error 'cancelled; rerun with --yes for unattended installation';; esac
}

if [ "$UNINSTALL" -eq 1 ]; then
    confirm "Remove $UNIT (data and shared binaries will be retained)?"
    systemctl disable --now "$UNIT"
    systemctl mask "$UNIT"
    systemctl daemon-reload
    info "Disabled and masked $UNIT. Retained $STATE, $CONFIG, execution account settings and shared binaries."
    exit 0
fi

# Preserve the configured execution account on repeat installs.
if [ -f "$DROPIN/pm3.conf" ]; then
    EXISTING_USER=$(sed -n 's/^User=//p' "$DROPIN/pm3.conf")
    [ -z "$RUN_AS" ] || [ "$RUN_AS" = "$EXISTING_USER" ] || error 'changing an existing execution account requires a manual data/permissions migration'
    RUN_AS=$EXISTING_USER
fi
[ -z "$RUN_AS" ] && RUN_AS=${MIGRATE_USER:-root}
case "$RUN_AS" in ''|*[!a-zA-Z0-9_-]*|[-0-9]*) error 'invalid service account';; esac
getent passwd "$RUN_AS" >/dev/null || error "unknown account: $RUN_AS"
RUN_GROUP=$(id -gn "$RUN_AS")
RUN_HOME=$(getent passwd "$RUN_AS" | cut -d: -f6)
case "$RUN_HOME" in *[!a-zA-Z0-9_./-]*) error 'unsupported account home path';; esac

if [ -n "$MIGRATE_USER" ]; then
    [ ! -e "$STATE" ] && [ ! -L "$STATE" ] || error 'migration requires a new instance name/state directory'
    [ "$RUN_AS" = "$MIGRATE_USER" ] || error 'migration must preserve the old execution account; use --user with the same account'
    LEGACY_HOME=$(getent passwd "$MIGRATE_USER" | cut -d: -f6)
    [ -d "$LEGACY_HOME/.pm3" ] || error "no legacy state at $LEGACY_HOME/.pm3"
    command -v runuser >/dev/null || error 'migration requires runuser'
elif [ -f /etc/systemd/user/pm3.service ] && [ ! -f "$DROPIN/pm3.conf" ]; then
    error 'legacy user installation detected. Save its processes with the old pm3 dump, then use --migrate-user USER (one named instance per old daemon).'
fi

TMP_DIR=$(mktemp -d)
if [ -n "$FROM_DIR" ]; then
    FROM_DIR=$(cd "$FROM_DIR" && pwd)
    cp -R "$FROM_DIR/." "$TMP_DIR/bundle"
    VERSION=${VERSION:-local}
else
    command -v curl >/dev/null || error 'missing command: curl'
    case "$(uname -m)" in x86_64|amd64) ARCH=x86_64;; aarch64|arm64) ARCH=aarch64;; *) error 'unsupported CPU architecture';; esac
    case "$VERSION" in *[!a-zA-Z0-9._-]*) error 'invalid version tag';; esac
    API="https://api.github.com/repos/$REPO/releases/latest"
    [ -z "$VERSION" ] || API="https://api.github.com/repos/$REPO/releases/tags/$VERSION"
    curl -fsSL "$API" -o "$TMP_DIR/release.json"
    VERSION=$(sed -n 's/^[[:space:]]*"tag_name":[[:space:]]*"\([^"]*\)".*/\1/p' "$TMP_DIR/release.json" | head -n 1)
    URL=$(sed -n "s/^[[:space:]]*\"browser_download_url\":[[:space:]]*\"\([^\"]*pm3-linux-$ARCH.tar.gz\)\".*/\1/p" "$TMP_DIR/release.json" | head -n 1)
    [ -n "$VERSION" ] && [ -n "$URL" ] || error "release has no Linux $ARCH archive"
    curl -fL "$URL" -o "$TMP_DIR/release.tar.gz"
    mkdir "$TMP_DIR/bundle"
    tar -xzf "$TMP_DIR/release.tar.gz" --no-same-owner -C "$TMP_DIR/bundle"
fi
BUNDLE="$TMP_DIR/bundle"
[ -f "$BUNDLE/pm3" ] && [ -f "$BUNDLE/pm3-daemon" ] && [ -f "$BUNDLE/dashboard/index.html" ] || error 'incomplete release: expected pm3, pm3-daemon and dashboard/index.html'
chmod 0755 "$BUNDLE/pm3" "$BUNDLE/pm3-daemon"
"$BUNDLE/pm3" --help | grep -q -- --instance || error 'release predates system instances or cannot run on this system; build this branch or select a compatible release'
# Catch loader/libc/architecture errors before changing the running installation.
"$BUNDLE/pm3-daemon" --version >/dev/null || error 'daemon binary cannot run on this system'

RESTART_UNITS=$(systemctl list-units --type=service --state=active --no-legend 'pm3@*.service' | awk '{printf "%s ", $1}')
if [ -L "$PREFIX/current" ]; then
    OLD_RELEASE=$(readlink -f "$PREFIX/current")
    confirm 'Update shared binaries and restart active PM3 instances?'
fi

# Stage a complete release; retain older releases for rollback.
install -d -m 0755 "$PREFIX/releases" /usr/local/bin /etc/pm3 /var/lib/pm3
RELEASE=$(mktemp -d "$PREFIX/releases/release.XXXXXXXX")
install -m 0755 "$BUNDLE/pm3" "$BUNDLE/pm3-daemon" "$RELEASE/"
cp -R "$BUNDLE/dashboard" "$RELEASE/dashboard"
chmod -R a+rX "$RELEASE"
printf '%s\n' "$VERSION" > "$RELEASE/pm3.version"
if [ -f "$BUNDLE/install.sh" ]; then
    install -m 0755 "$BUNDLE/install.sh" "$RELEASE/install.sh"
elif [ -f "$0" ]; then
    install -m 0755 "$0" "$RELEASE/install.sh"
fi

# Dump the current process list before any system instance is restarted.
for unit in $RESTART_UNITS; do
    name=${unit#pm3@}; name=${name%.service}
    env -u PM3_HOME -u PM3_INSTANCE "$PREFIX/current/pm3" --instance "$name" dump > "$TMP_DIR/dump"
    grep -q 'configs saved to' "$TMP_DIR/dump" || error "cannot save $unit; no services have been stopped"
done

if [ -n "$MIGRATE_USER" ]; then
    uid=$(id -u "$MIGRATE_USER")
    legacy_ctl() { runuser -u "$MIGRATE_USER" -- env XDG_RUNTIME_DIR="/run/user/$uid" DBUS_SESSION_BUS_ADDRESS="unix:path=/run/user/$uid/bus" systemctl --user "$@"; }
    if [ -S "/run/user/$uid/bus" ]; then
        if legacy_ctl is-active --quiet pm3.service; then
            runuser -u "$MIGRATE_USER" -- env HOME="$LEGACY_HOME" /usr/bin/pm3/pm3 dump > "$TMP_DIR/dump"
            grep -q 'configs saved to' "$TMP_DIR/dump" || error 'cannot save legacy processes'
        fi
        legacy_ctl disable --now pm3.service
        legacy_ctl mask pm3.service
    else
        error 'legacy user manager unavailable; start it and retry migration so the old service can be stopped and disabled safely'
    fi
    # Copy only after stopping the writer, including SQLite WAL and logs. Leave backup intact.
    cp -a "$LEGACY_HOME/.pm3/." "$STATE"
    rm -f "$STATE/config.proc" "$STATE/daemon.lock"
    chown -R "$RUN_AS:$RUN_GROUP" "$STATE"
fi
install -d -m 0700 -o "$RUN_AS" -g "$RUN_GROUP" "$STATE"
if [ ! -f "$CONFIG" ]; then
    PORT=8096
    while grep -Eq "^[[:space:]]*PM3_API_PORT[[:space:]]*=[[:space:]]*['\"]?$PORT['\"]?[[:space:]]*(#.*)?$" /etc/pm3/*.env 2>/dev/null; do PORT=$((PORT + 1)); done
    if [ -f "$STATE/pm3.env" ]; then
        # Preserve migrated API settings, including the original address/port.
        install -m 0640 -o root -g "$RUN_GROUP" "$STATE/pm3.env" "$CONFIG"
    else
        printf 'PM3_IP=127.0.0.1\nPM3_API_PORT=%s\n' "$PORT" > "$CONFIG"
        chown "root:$RUN_GROUP" "$CONFIG"
        chmod 0640 "$CONFIG"
    fi
fi
install -d -m 0755 /etc/systemd/system "$DROPIN"
cat > /etc/systemd/system/pm3@.service <<'UNIT_FILE'
[Unit]
Description=PM3 process manager (%i)
After=network.target
ConditionPathExists=/etc/pm3/%i.env

[Service]
Type=simple
WorkingDirectory=/opt/pm3/current
Environment=PM3_INSTANCE=%i
Environment=PM3_CONFIG=/etc/pm3/%i.env
ExecStart=/opt/pm3/current/pm3-daemon
Restart=on-failure
RestartSec=3
KillMode=control-group
TimeoutStopSec=30
UMask=0077

[Install]
WantedBy=multi-user.target
UNIT_FILE
cat > "$DROPIN/pm3.conf" <<ACCOUNT
[Service]
User=$RUN_AS
Group=$RUN_GROUP
Environment=HOME=$RUN_HOME
ACCOUNT
chmod 0644 /etc/systemd/system/pm3@.service "$DROPIN/pm3.conf"
ln -sfn "$RELEASE" "$PREFIX/current.next"
mv -Tf "$PREFIX/current.next" "$PREFIX/current"
SWITCHED=1
ln -sfn "$PREFIX/current/pm3" /usr/local/bin/pm3
systemctl daemon-reload
systemctl unmask "$UNIT"
systemctl enable "$UNIT"
case " $RESTART_UNITS " in *" $UNIT "*) ;; *) RESTART_UNITS="$RESTART_UNITS $UNIT";; esac
for unit in $RESTART_UNITS; do
    systemctl restart "$unit"
    name=${unit#pm3@}; name=${name%.service}
    ready=0
    for attempt in 1 2 3 4 5 6 7 8 9 10; do
        if env -u PM3_HOME -u PM3_INSTANCE "$PREFIX/current/pm3" --instance "$name" ping 2>/dev/null | grep -q pong; then ready=1; break; fi
        sleep 1
    done
    [ "$ready" -eq 1 ] || error "$unit failed its readiness check; inspect journalctl -u $unit"
done
SWITCHED=0
info "PM3 $VERSION installed: $UNIT (process account: $RUN_AS)"
info "CLI: sudo pm3 --instance $INSTANCE list"
info "Settings: $CONFIG | State: $STATE"
info "Service: systemctl status $UNIT | Logs: journalctl -u $UNIT"
info 'Use pm3 save after process changes to restore them on boot. All instances: systemctl list-units --all pm3@*.service'
if [ -d /usr/bin/pm3 ]; then
    info 'Legacy binaries remain in /usr/bin/pm3 as a migration backup. Remove the old /usr/bin/pm3 PATH entry from shell profiles; use /usr/local/bin/pm3.'
fi
