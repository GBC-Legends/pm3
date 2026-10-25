#!/bin/sh
# Optional real-systemd integration: only uniquely named transient test units.
set -eu
[ "$(id -u)" = 0 ] || { echo 'run as root' >&2; exit 1; }
REPO=$(CDPATH= cd -- "$(dirname "$0")/.." && pwd)
CLI=${PM3_TEST_CLI:-$REPO/pm3/target/debug/pm3}
DAEMON=${PM3_TEST_DAEMON:-$REPO/pm3-daemon/target/debug/pm3-daemon}
WORK=$(mktemp -d)
UNIT="pm3-test-$$"
LIMITED="$UNIT-limited"
cleanup() {
    systemctl stop "$LIMITED.service" 2>/dev/null || true
    systemctl reset-failed "$LIMITED.service" 2>/dev/null || true
    systemctl stop "$UNIT.service" 2>/dev/null || true
    systemctl reset-failed "$UNIT.service" 2>/dev/null || true
    rm -rf "$WORK"
}
trap cleanup EXIT
mkdir -p "$WORK/dashboard"
echo '<html>PM3 test dashboard</html>' > "$WORK/dashboard/index.html"
systemd-run --unit="$UNIT" --property="WorkingDirectory=$WORK" \
    --property=KillMode=control-group --property=UMask=0077 \
    --setenv="PM3_HOME=$WORK/state" --setenv=PM3_API_PORT=0 "$DAEMON"
ready() {
    for attempt in 1 2 3 4 5 6 7 8 9 10; do
        if PM3_HOME="$WORK/state" "$CLI" ping 2>/dev/null | grep -q pong; then return; fi
        sleep 1
    done
    journalctl -u "$UNIT" --no-pager >&2
    exit 1
}
ready
PM3_HOME="$WORK/state" "$CLI" start /bin/sleep 600 --name lifecycle
PM3_HOME="$WORK/state" "$CLI" save
parent=$(systemctl show -p MainPID --value "$UNIT")
child=$(pgrep -P "$parent" sleep)
test -n "$child"
port=$(sed -n 's/^api_port=//p' "$WORK/state/config.proc")
curl -fsS "http://127.0.0.1:$port/" | grep -q 'PM3 test dashboard'
systemctl restart "$UNIT"
ready
! kill -0 "$child" 2>/dev/null
PM3_HOME="$WORK/state" "$CLI" list | grep -q lifecycle
parent=$(systemctl show -p MainPID --value "$UNIT")
child=$(pgrep -P "$parent" sleep)
test -n "$child"
PM3_HOME="$WORK/state" "$CLI" stop lifecycle
PM3_HOME="$WORK/state" "$CLI" save
systemctl restart "$UNIT"
ready
PM3_HOME="$WORK/state" "$CLI" list | grep -q lifecycle
parent=$(systemctl show -p MainPID --value "$UNIT")
if pgrep -P "$parent" sleep; then echo 'saved stopped process restarted' >&2; exit 1; fi
systemctl stop "$UNIT"
! kill -0 "$child" 2>/dev/null
# The execution account can control its system instance without a login/session
# manager, but cannot read another account's private discovery credentials.
chmod 0755 "$WORK"
group=$(id -gn nobody)
install -d -m 0700 -o nobody -g "$group" "$WORK/limited"
printf 'PM3_IP=127.0.0.1\nPM3_API_PORT=0\n' > "$WORK/limited.env"
chown "root:$group" "$WORK/limited.env"
chmod 0640 "$WORK/limited.env"
systemd-run --unit="$LIMITED" --property="User=nobody" --property="Group=$group" \
    --property="WorkingDirectory=$WORK" --property=KillMode=control-group \
    --setenv="PM3_HOME=$WORK/limited" --setenv="PM3_CONFIG=$WORK/limited.env" "$DAEMON"
connected=0
for attempt in 1 2 3 4 5 6 7 8 9 10; do
    if runuser -u nobody -- env PM3_HOME="$WORK/limited" "$CLI" ping 2>/dev/null | grep -q pong; then connected=1; break; fi
    sleep 1
done
test "$connected" = 1
if runuser -u nobody -- env PM3_HOME="$WORK/state" "$CLI" ping >/dev/null 2>&1; then exit 1; fi
echo 'Real systemd restart/restoration/process cleanup passed'
