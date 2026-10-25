#!/bin/sh
# Real daemon/CLI integration, entirely under a temporary PM3_HOME.
set -eu
REPO=$(CDPATH= cd -- "$(dirname "$0")/.." && pwd)
CLI=${PM3_TEST_CLI:-$REPO/pm3/target/debug/pm3}
DAEMON=${PM3_TEST_DAEMON:-$REPO/pm3-daemon/target/debug/pm3-daemon}
WORK=$(mktemp -d)
PIDS=
cleanup() { for pid in $PIDS; do kill "$pid" 2>/dev/null || true; wait "$pid" 2>/dev/null || true; done; rm -rf "$WORK"; }
trap cleanup EXIT
start() {
    PM3_HOME="$WORK/$1" PM3_API_PORT=0 "$DAEMON" > "$WORK/$1.log" 2>&1 &
    LAST_PID=$!
    PIDS="$PIDS $LAST_PID"
    for attempt in 1 2 3 4 5 6 7 8 9 10; do
        if PM3_HOME="$WORK/$1" "$CLI" ping 2>/dev/null | grep -q pong; then return; fi
        sleep 1
    done
    cat "$WORK/$1.log" >&2
    exit 1
}
start a
FIRST=$LAST_PID
start b
test "$(sed -n 's/^port=//p' "$WORK/a/config.proc")" != "$(sed -n 's/^port=//p' "$WORK/b/config.proc")"
cp "$WORK/a/config.proc" "$WORK/original.proc"
if PM3_HOME="$WORK/a" "$DAEMON" > "$WORK/duplicate.log" 2>&1; then echo 'duplicate daemon started' >&2; exit 1; fi
cmp "$WORK/a/config.proc" "$WORK/original.proc"
grep -q 'already running' "$WORK/duplicate.log"
test "$(stat -c %a "$WORK/a/config.proc")" = 600
test "$(stat -c %a "$WORK/a")" = 700
# Saved definitions are restored automatically on the next daemon startup.
mkdir -p "$WORK/a/configs"
cat > "$WORK/a/configs/example.proc" <<'CONFIG'
proc_name=saved-example
exec_dir=/tmp
exec_name=/bin/sleep
exec_args=600
active=0
CONFIG
kill "$FIRST"
wait "$FIRST" 2>/dev/null || true
start a
PM3_HOME="$WORK/a" "$CLI" list | grep -q saved-example
# Different login homes must discover the same explicitly selected state.
HOME="$WORK/unrelated-user" PM3_HOME="$WORK/a" "$CLI" ping | grep -q pong
PM3_INSTANCE='../invalid' PM3_HOME="$WORK/a" "$CLI" --instance default ping | grep -q pong
if PM3_INSTANCE='../invalid' PM3_HOME="$WORK/a" "$CLI" ping >/dev/null 2>&1; then exit 1; fi
if PM3_HOME="$WORK/missing" "$CLI" ping > "$WORK/error" 2>&1; then echo 'missing instance returned success' >&2; exit 1; fi
test ! -d "$WORK/missing"
if "$CLI" --instance '../escape' ping >/dev/null 2>&1; then exit 1; fi
if PM3_HOME=relative "$DAEMON" >/dev/null 2>&1; then exit 1; fi
# A bad/occupied API address fails the whole daemon, not just a detached task.
api_port=$(sed -n 's/^api_port=//p' "$WORK/a/config.proc")
if PM3_HOME="$WORK/conflict" PM3_API_PORT="$api_port" "$DAEMON" > "$WORK/conflict.log" 2>&1; then exit 1; fi
test ! -e "$WORK/conflict/config.proc"
mkdir -p "$WORK/corrupt/configs"
echo 'invalid config' > "$WORK/corrupt/configs/broken.proc"
if PM3_HOME="$WORK/corrupt" PM3_API_PORT=0 "$DAEMON" > "$WORK/corrupt.log" 2>&1; then exit 1; fi
grep -q 'invalid process config' "$WORK/corrupt.log"
echo 'Linux instance lifecycle tests passed'
