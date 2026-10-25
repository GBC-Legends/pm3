#!/bin/sh
# Exercise the real installer inside a disposable chroot, with a simulated
# systemd manager. No host units, users or installation paths are modified.
set -eu
[ "$(id -u)" = 0 ] || { echo 'run this test as root' >&2; exit 1; }
REPO=$(CDPATH= cd -- "$(dirname "$0")/.." && pwd)
ROOT=$(mktemp -d)
trap 'rm -rf -- "$ROOT"' EXIT
mkdir -p "$ROOT/bin" "$ROOT/etc" "$ROOT/dev" "$ROOT/tmp" "$ROOT/run/systemd/system" "$ROOT/bundle/dashboard"
for cmd in sh install mktemp flock getent readlink tar cp mv ln awk sed grep find id uname chmod chown mkdir rm cat cut head env sleep; do
    binary=$(command -v "$cmd")
    cp -L "$binary" "$ROOT/bin/$cmd"
    ldd "$binary" 2>/dev/null | awk '/=> \// {print $3} /^\// {print $1} /^[[:space:]]*\// {print $1}' | while read -r lib; do
        mkdir -p "$ROOT$(dirname "$lib")"
        cp -L "$lib" "$ROOT$lib"
    done
done
printf 'root:x:0:0:root:/root:/bin/sh\napp:x:1000:1000:app:/home/app:/bin/sh\n' > "$ROOT/etc/passwd"
printf 'root:x:0:\napp:x:1000:\n' > "$ROOT/etc/group"
printf 'passwd: files\ngroup: files\n' > "$ROOT/etc/nsswitch.conf"
# Regular files suffice for redirect-only /dev/null and the installer lock.
touch "$ROOT/dev/null"
cp "$REPO/install.sh" "$ROOT/install.sh"
cat > "$ROOT/bin/systemctl" <<'MOCK'
#!/bin/sh
set -eu
echo "$*" >> /run/systemctl.log
case "$1" in
    list-units) for f in /run/active-*; do [ -f "$f" ] || continue; printf '%s loaded active running\n' "${f#/run/active-}"; done;;
    restart) touchfile=/run/active-$2; : > "$touchfile";;
    disable) rm -f "/run/active-$3";;
esac
MOCK
cat > "$ROOT/bundle/pm3" <<'CLI'
#!/bin/sh
set -eu
case "$*" in
    --help) echo 'pm3 --instance';;
    *dump) echo '1 configs saved to /var/lib/pm3/test/configs';;
    *ping) [ ! -f /opt/pm3/current/broken ] || exit 1; echo pong;;
esac
CLI
printf '#!/bin/sh\nexit 0\n' > "$ROOT/bundle/pm3-daemon"
printf '<html>dashboard</html>\n' > "$ROOT/bundle/dashboard/index.html"
chmod +x "$ROOT/bin/systemctl" "$ROOT/bundle/pm3" "$ROOT/bundle/pm3-daemon"
run() { chroot "$ROOT" /bin/env PATH=/bin /bin/sh /install.sh --from-dir /bundle --yes "$@"; }
run
test -L "$ROOT/usr/local/bin/pm3"
grep -q 'WantedBy=multi-user.target' "$ROOT/etc/systemd/system/pm3@.service"
grep -q 'User=root' "$ROOT/etc/systemd/system/pm3@default.service.d/pm3.conf"
test "$(stat -c %a "$ROOT/var/lib/pm3/default")" = 700
test ! -e "$ROOT/root/.pm3"
echo preserved > "$ROOT/var/lib/pm3/default/sentinel"
printf 'PM3_API_PORT = "8097" # reserved\n' > "$ROOT/etc/pm3/reserved.env"
run --instance api --user app
grep -q 'PM3_API_PORT=8098' "$ROOT/etc/pm3/api.env"
test "$(stat -c %a "$ROOT/etc/pm3/api.env")" = 640
test "$(stat -c %g "$ROOT/etc/pm3/api.env")" = 1000
test "$(stat -c %u "$ROOT/var/lib/pm3/api")" = 1000
run --instance api
grep -q 'User=app' "$ROOT/etc/systemd/system/pm3@api.service.d/pm3.conf"
grep -q preserved "$ROOT/var/lib/pm3/default/sentinel"
grep -q 'restart pm3@default.service' "$ROOT/run/systemctl.log"
grep -q 'restart pm3@api.service' "$ROOT/run/systemctl.log"
before=$(readlink "$ROOT/opt/pm3/current")
if run --instance '../escape'; then echo 'accepted invalid name' >&2; exit 1; fi
if run --instance api --user root; then echo 'silently changed execution account' >&2; exit 1; fi
mv "$ROOT/bundle/pm3-daemon" "$ROOT/daemon.backup"
if run; then echo 'accepted incomplete bundle' >&2; exit 1; fi
test "$(readlink "$ROOT/opt/pm3/current")" = "$before"
mv "$ROOT/daemon.backup" "$ROOT/bundle/pm3-daemon"
# A new CLI that cannot reach the daemon triggers binary rollback.
cp "$ROOT/bundle/pm3" "$ROOT/cli.backup"
sed -i 's/echo pong/exit 1/' "$ROOT/bundle/pm3"
if run; then echo 'accepted unhealthy release' >&2; exit 1; fi
test "$(readlink "$ROOT/opt/pm3/current")" = "$before"
mv "$ROOT/cli.backup" "$ROOT/bundle/pm3"
run --instance api --uninstall
grep -q 'mask pm3@api.service' "$ROOT/run/systemctl.log"
test -d "$ROOT/var/lib/pm3/api"
test -f "$ROOT/etc/pm3/api.env"
mkdir -p "$ROOT/etc/systemd/user"
echo legacy > "$ROOT/etc/systemd/user/pm3.service"
if run --instance migrated; then echo 'silently mixed legacy and system state' >&2; exit 1; fi
# Model a reachable legacy user manager with a UNIX socket and runuser shim.
mkdir -p "$ROOT/run/user/1000" "$ROOT/home/app/.pm3/configs" "$ROOT/usr/bin/pm3"
cat > "$ROOT/socket.c" <<'C'
#include <sys/socket.h>
#include <sys/un.h>
#include <string.h>
#include <unistd.h>
int main(int argc, char **argv) {
    struct sockaddr_un addr = { .sun_family = AF_UNIX };
    if (argc != 2 || strlen(argv[1]) >= sizeof(addr.sun_path)) return 1;
    strcpy(addr.sun_path, argv[1]);
    int fd = socket(AF_UNIX, SOCK_STREAM, 0);
    if (fd < 0 || bind(fd, (struct sockaddr *)&addr, sizeof(addr))) return 1;
    close(fd);
    return 0;
}
C
cc "$ROOT/socket.c" -o "$ROOT/create-socket"
"$ROOT/create-socket" "$ROOT/run/user/1000/bus"
cat > "$ROOT/bin/runuser" <<'RUNUSER'
#!/bin/sh
shift 2
shift
exec "$@"
RUNUSER
chmod +x "$ROOT/bin/runuser"
cp "$ROOT/bundle/pm3" "$ROOT/usr/bin/pm3/pm3"
echo saved > "$ROOT/home/app/.pm3/configs/old.proc"
printf 'PM3_IP=127.0.0.1\nPM3_API_PORT=9123\n' > "$ROOT/home/app/.pm3/pm3.env"
echo stale > "$ROOT/home/app/.pm3/config.proc"
run --instance migrated --migrate-user app
grep -q 'User=app' "$ROOT/etc/systemd/system/pm3@migrated.service.d/pm3.conf"
grep -q 'PM3_API_PORT=9123' "$ROOT/etc/pm3/migrated.env"
grep -q saved "$ROOT/var/lib/pm3/migrated/configs/old.proc"
grep -q saved "$ROOT/home/app/.pm3/configs/old.proc"
test ! -e "$ROOT/var/lib/pm3/migrated/config.proc"
test "$(stat -c %u "$ROOT/var/lib/pm3/migrated/configs/old.proc")" = 1000
grep -q -- '--user disable --now pm3.service' "$ROOT/run/systemctl.log"
grep -q -- '--user mask pm3.service' "$ROOT/run/systemctl.log"
if run --instance migrated --migrate-user app; then echo 'migration overwrote existing data' >&2; exit 1; fi
echo 'Installer lifecycle tests passed'
