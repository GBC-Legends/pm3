#!/bin/sh
# Real systemd and Unix account isolation, using disposable HOME directories and
# uniquely named transient units. Does not touch anyone's installed PM3 daemon.
set -eu
[ "$(id -u)" = 0 ] || { echo 'run as root' >&2; exit 1; }
REPO=$(CDPATH= cd -- "$(dirname "$0")/.." && pwd)
CLI=${PM3_TEST_CLI:-$REPO/pm3/target/debug/pm3}
DAEMON=${PM3_TEST_DAEMON:-$REPO/pm3-daemon/target/debug/pm3-daemon}
WORK=$(mktemp -d)
ROOT_UNIT="pm3-validation-root-$$"
USER_UNIT="pm3-validation-user-$$"
cleanup() {
    systemctl stop "$ROOT_UNIT" "$USER_UNIT" 2>/dev/null || true
    systemctl reset-failed "$ROOT_UNIT" "$USER_UNIT" 2>/dev/null || true
    rm -rf "$WORK"
}
trap cleanup EXIT
chmod 0755 "$WORK"
# CI checkouts may live below a private runner HOME. Stage executable copies so
# nobody can run them without changing permissions on the checkout or its parents.
install -m 0755 "$CLI" "$WORK/pm3"
install -m 0755 "$DAEMON" "$WORK/pm3-daemon"
CLI="$WORK/pm3"
DAEMON="$WORK/pm3-daemon"
# Application starts inherit the CLI's working directory, which must also be
# accessible to the account running them (the caller may be inside /root).
cd "$WORK"
mkdir -p "$WORK/root/.pm3" "$WORK/app/.pm3" "$WORK/dashboard"
chmod 0700 "$WORK/root/.pm3" "$WORK/app/.pm3"
printf 'APP_SETTING=per-user-test\n' > "$WORK/app/.pm3/pm3.env"
chown -R "nobody:$(id -gn nobody)" "$WORK/app"
echo '<html>per-user dashboard</html>' > "$WORK/dashboard/index.html"
systemd-run --unit="$ROOT_UNIT" --property=User=root --property=KillMode=mixed \
    --property="WorkingDirectory=$WORK" --setenv="HOME=$WORK/root" --setenv=PM3_RESTORE=1 "$DAEMON"
systemd-run --unit="$USER_UNIT" --property=User=nobody --property=KillMode=mixed \
    --property="WorkingDirectory=$WORK" --setenv="HOME=$WORK/app" --setenv=PM3_RESTORE=1 "$DAEMON"
root_cli() { env HOME="$WORK/root" "$CLI" "$@"; }
app_cli() { runuser -u nobody -- env HOME="$WORK/app" "$CLI" "$@"; }
ready() {
    for attempt in 1 2 3 4 5 6 7 8 9 10; do
        if root_cli ping 2>/dev/null | grep -q pong && app_cli ping 2>/dev/null | grep -q pong; then return; fi
        sleep 1
    done
    journalctl -u "$ROOT_UNIT" -u "$USER_UNIT" --no-pager >&2
    exit 1
}
ready
root_port=$(sed -n 's/^port=//p' "$WORK/root/.pm3/config.proc")
app_port=$(sed -n 's/^port=//p' "$WORK/app/.pm3/config.proc")
test "$root_port" != "$app_port"
root_api=$(sed -n 's/^api_addr=//p' "$WORK/root/.pm3/config.proc")
app_api=$(sed -n 's/^api_addr=//p' "$WORK/app/.pm3/config.proc")
test "$root_api" != "$app_api"
curl -fsS "http://$root_api/" | grep -q 'per-user dashboard'
curl -fsS "http://$app_api/" | grep -q 'per-user dashboard'
test "$(stat -c %u "$WORK/app/.pm3/config.proc")" = "$(id -u nobody)"
test "$(stat -c %a "$WORK/root/.pm3/config.proc")" = 600
test "$(stat -c %a "$WORK/app/.pm3")" = 700
if runuser -u nobody -- env HOME="$WORK/root" "$CLI" ping >/dev/null 2>&1; then echo 'cross-user control allowed' >&2; exit 1; fi
cp "$WORK/root/.pm3/config.proc" "$WORK/before.proc"
if HOME="$WORK/root" "$DAEMON" > "$WORK/duplicate.log" 2>&1; then exit 1; fi
cmp "$WORK/before.proc" "$WORK/root/.pm3/config.proc"
grep -q 'already owns' "$WORK/duplicate.log"
root_cli start /bin/sleep 600 --name root-only
if app_cli list | grep -q root-only; then echo 'users share process lists' >&2; exit 1; fi
cat > "$WORK/app-check.sh" <<'APP'
#!/bin/sh
printf '%s:%s\n' "$(id -u)" "$APP_SETTING" > "$HOME/observed"
exec sleep 600
APP
chmod 0755 "$WORK/app-check.sh"
app_cli start "$WORK/app-check.sh" --interpreter /bin/sh --name app-only
for attempt in 1 2 3 4 5; do [ ! -f "$WORK/app/observed" ] || break; sleep 1; done
grep -q "^$(id -u nobody):per-user-test$" "$WORK/app/observed"
app_parent=$(systemctl show -p MainPID --value "$USER_UNIT")
app_child=$(pgrep -P "$app_parent" sleep)
root_cli save
parent=$(systemctl show -p MainPID --value "$ROOT_UNIT")
child=$(pgrep -P "$parent" sleep)
systemctl restart "$ROOT_UNIT"
ready
! kill -0 "$child" 2>/dev/null
root_cli list | grep -q root-only
test "$(systemctl show -p MainPID --value "$USER_UNIT")" = "$app_parent"
kill -0 "$app_child"
parent=$(systemctl show -p MainPID --value "$ROOT_UNIT")
child=$(pgrep -P "$parent" sleep)
root_cli stop root-only
root_cli save
systemctl restart "$ROOT_UNIT"
ready
parent=$(systemctl show -p MainPID --value "$ROOT_UNIT")
if pgrep -P "$parent" sleep; then echo 'saved stopped process was started' >&2; exit 1; fi
if HOME="$WORK/missing" "$CLI" ping > "$WORK/error" 2>&1; then exit 1; fi
test ! -e "$WORK/missing"
if "$CLI" --instance default list >/dev/null 2>&1; then exit 1; fi
mkdir -p "$WORK/bad/.pm3"
echo 'PM3_API_PORT=invalid' > "$WORK/bad/.pm3/pm3.env"
if HOME="$WORK/bad" "$DAEMON" > "$WORK/bad.log" 2>&1; then exit 1; fi
grep -q 'invalid PM3_API_PORT' "$WORK/bad.log"
systemctl stop "$ROOT_UNIT" "$USER_UNIT"
! kill -0 "$app_child" 2>/dev/null
echo 'Real per-user systemd isolation, restore, environment and cleanup tests passed'
