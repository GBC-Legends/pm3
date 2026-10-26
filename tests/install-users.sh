#!/bin/sh
# Run the real installer in a disposable chroot with a simulated systemd manager.
set -eu
[ "$(id -u)" = 0 ] || { echo 'run as root' >&2; exit 1; }
REPO=$(CDPATH= cd -- "$(dirname "$0")/.." && pwd)
ROOT=$(mktemp -d)
trap 'rm -rf -- "$ROOT"' EXIT
mkdir -p "$ROOT/bin" "$ROOT/etc" "$ROOT/dev" "$ROOT/tmp" "$ROOT/run/systemd/system" "$ROOT/bundle/dashboard" "$ROOT/root" "$ROOT/home/ubuntu" "$ROOT/home/legacy"
for cmd in sh systemctl runuser getent id install mktemp flock readlink cp mv ln awk sed grep tar gzip env cut head chmod chown mkdir rm cat uname sleep; do
    case "$cmd" in systemctl|runuser) continue;; esac
    binary=$(command -v "$cmd")
    cp -L "$binary" "$ROOT/bin/$cmd"
    ldd "$binary" 2>/dev/null | awk '/=> \// {print $3} /^[[:space:]]*\// {print $1}' | while read -r lib; do
        mkdir -p "$ROOT$(dirname "$lib")"
        cp -L "$lib" "$ROOT$lib"
    done
done
printf 'root:x:0:0:root:/root:/bin/sh\nubuntu:x:1000:1000::/home/ubuntu:/bin/sh\nlegacy:x:1001:1001::/home/legacy:/bin/sh\n' > "$ROOT/etc/passwd"
printf 'root:x:0:\nubuntu:x:1000:\nlegacy:x:1001:\n' > "$ROOT/etc/group"
printf 'passwd: files\ngroup: files\n' > "$ROOT/etc/nsswitch.conf"
touch "$ROOT/dev/null"
cp "$REPO/install.sh" "$ROOT/install.sh"
cat > "$ROOT/bin/runuser" <<'RUNUSER'
#!/bin/sh
# OS permission isolation is tested with real runuser in linux-users.sh.
TEST_USER=$2
export TEST_USER
shift 3
exec "$@"
RUNUSER
cat > "$ROOT/bin/systemctl" <<'SYSTEMCTL'
#!/bin/sh
set -eu
printf '%s %s\n' "${TEST_USER:-root}" "$*" >> /run/systemctl.log
scope=system
if [ "$1" = --user ]; then scope=legacy; shift; fi
action=$1
shift
case "$action" in
    list-units)
        for f in /run/system-active-*; do [ -f "$f" ] || continue; printf '%s loaded active running\n' "${f#/run/system-active-}"; done
        exit 0;;
    daemon-reload) exit 0;;
esac
case "${1:-}" in --quiet|--now) shift;; esac
unit=${1:-}
if [ "$scope" = legacy ]; then unit=${TEST_USER:?}; fi
case "$action" in
    is-active) test -f "/run/$scope-active-$unit";;
    is-enabled) test -f "/run/$scope-enabled-$unit";;
    restart|start) : > "/run/$scope-active-$unit";;
    enable) : > "/run/$scope-enabled-$unit";;
    disable) rm -f "/run/$scope-enabled-$unit" "/run/$scope-active-$unit";;
    stop) rm -f "/run/$scope-active-$unit";;
    *) echo "unimplemented systemctl $action" >&2; exit 1;;
esac
SYSTEMCTL
cat > "$ROOT/bundle/pm3" <<'CLI'
#!/bin/sh
set -eu
printf '%s %s HOME=%s\n' "${TEST_USER:-root}" "$*" "${HOME:-}" >> /run/cli.log
case "${1:-}" in
    --installation-model) echo per-user-systemd-v1;;
    dump) echo 'OK 1 configs saved to ~/.pm3/configs';;
    ping)
        account=${TEST_USER:-root}
        test -f "/run/system-active-pm3-$account.service" || test -f "/run/legacy-active-$account" || exit 1
        echo pong;;
    *) exit 2;;
esac
CLI
printf '#!/bin/sh\nexit 0\n' > "$ROOT/bundle/pm3-daemon"
echo '<html>dashboard</html>' > "$ROOT/bundle/dashboard/index.html"
chmod +x "$ROOT/bin/systemctl" "$ROOT/bin/runuser" "$ROOT/bundle/pm3" "$ROOT/bundle/pm3-daemon"
run() { chroot "$ROOT" /bin/env -u SUDO_USER PATH=/bin /bin/sh /install.sh --yes --from-dir /bundle "$@"; }
run
grep -q '^User=root$' "$ROOT/etc/systemd/system/pm3-root.service"
grep -q '^KillMode=mixed$' "$ROOT/etc/systemd/system/pm3-root.service"
grep -q 'HOME=/root' "$ROOT/etc/systemd/system/pm3-root.service"
test -L "$ROOT/usr/local/bin/pm3"
test -d "$ROOT/root/.pm3"
test ! -e "$ROOT/var/lib/pm3"
echo retained > "$ROOT/root/.pm3/sentinel"
# sudo's caller is selected by default, independently of root's HOME.
chroot "$ROOT" /bin/env PATH=/bin SUDO_USER=ubuntu HOME=/root /bin/sh /install.sh --yes --from-dir /bundle
grep -q '^User=ubuntu$' "$ROOT/etc/systemd/system/pm3-ubuntu.service"
grep -q 'HOME=/home/ubuntu' "$ROOT/etc/systemd/system/pm3-ubuntu.service"
test -d "$ROOT/home/ubuntu/.pm3"
grep -q 'ubuntu ping HOME=/home/ubuntu' "$ROOT/run/cli.log"
run --user ubuntu
grep -q 'root dump HOME=/root' "$ROOT/run/cli.log"
grep -q 'ubuntu dump HOME=/home/ubuntu' "$ROOT/run/cli.log"
grep -q retained "$ROOT/root/.pm3/sentinel"
before=$(readlink "$ROOT/opt/pm3/current")
if run --user '../root'; then exit 1; fi
if run --instance default; then echo 'shared instances still accepted' >&2; exit 1; fi
mv "$ROOT/bundle/pm3-daemon" "$ROOT/daemon.backup"
if run; then echo 'accepted incomplete bundle' >&2; exit 1; fi
test "$(readlink "$ROOT/opt/pm3/current")" = "$before"
mv "$ROOT/daemon.backup" "$ROOT/bundle/pm3-daemon"
cp "$ROOT/bundle/pm3" "$ROOT/cli.backup"
sed -i 's/echo pong/exit 1/' "$ROOT/bundle/pm3"
if run; then echo 'accepted unhealthy release' >&2; exit 1; fi
test "$(readlink "$ROOT/opt/pm3/current")" = "$before"
test -f "$ROOT/run/system-active-pm3-root.service"
test -f "$ROOT/run/system-active-pm3-ubuntu.service"
mv "$ROOT/cli.backup" "$ROOT/bundle/pm3"
run --user ubuntu --uninstall
test ! -e "$ROOT/etc/systemd/system/pm3-ubuntu.service"
test -d "$ROOT/home/ubuntu/.pm3"
test -f "$ROOT/run/system-active-pm3-root.service"
# Cover the release-download path as well as --from-dir, without network access.
tar -czf "$ROOT/release.tar.gz" -C "$ROOT/bundle" .
cat > "$ROOT/release.json" <<'JSON'
{
  "tag_name": "v-test",
  "browser_download_url": "https://example.invalid/pm3-linux-x86_64.tar.gz"
}
JSON
cat > "$ROOT/bin/curl" <<'CURL'
#!/bin/sh
set -eu
echo "$2" >> /run/download.log
case "$2" in
    https://api.github.com/*) cp /release.json "$4";;
    https://example.invalid/*) cp /release.tar.gz "$4";;
    *) exit 22;;
esac
CURL
chmod +x "$ROOT/bin/curl"
chroot "$ROOT" /bin/env PATH=/bin /bin/sh /install.sh --yes --user ubuntu --version v-test
grep -q '/releases/tags/v-test' "$ROOT/run/download.log"
grep -q 'pm3-linux-x86_64.tar.gz' "$ROOT/run/download.log"
before=$(readlink "$ROOT/opt/pm3/current")
echo '{}' > "$ROOT/release.json"
if chroot "$ROOT" /bin/env PATH=/bin /bin/sh /install.sh --yes --user ubuntu; then exit 1; fi
test "$(readlink "$ROOT/opt/pm3/current")" = "$before"
# Migration keeps ~/.pm3 in place and retires only that user's old manager unit.
mkdir -p "$ROOT/etc/systemd/user" "$ROOT/run/user/1001" "$ROOT/usr/bin/pm3" "$ROOT/home/legacy/.pm3/configs"
echo original > "$ROOT/etc/systemd/user/pm3.service"
echo saved > "$ROOT/home/legacy/.pm3/configs/app.proc"
cp "$ROOT/bundle/pm3" "$ROOT/usr/bin/pm3/pm3"
echo 'legacy installer' > "$ROOT/usr/bin/pm3/install.sh"
cat > "$ROOT/socket.c" <<'C'
#include <sys/socket.h>
#include <sys/un.h>
#include <string.h>
#include <unistd.h>
int main(int argc, char **argv) {
    struct sockaddr_un a = { .sun_family = AF_UNIX };
    if (argc != 2 || strlen(argv[1]) >= sizeof a.sun_path) return 1;
    strcpy(a.sun_path, argv[1]);
    int fd = socket(AF_UNIX, SOCK_STREAM, 0);
    if (fd < 0 || bind(fd, (struct sockaddr *)&a, sizeof a)) return 1;
    close(fd);
    return 0;
}
C
cc "$ROOT/socket.c" -o "$ROOT/socket"
"$ROOT/socket" "$ROOT/run/user/1001/bus"
touch "$ROOT/run/legacy-active-legacy" "$ROOT/run/legacy-enabled-legacy"
# Failed migration must restore the old user unit and leave data untouched.
cp "$ROOT/bundle/pm3" "$ROOT/cli.backup"
sed -i 's/echo pong/[ "${TEST_USER:-}" != legacy ] || exit 1; echo pong/' "$ROOT/bundle/pm3"
if run --user legacy; then exit 1; fi
test -f "$ROOT/run/legacy-active-legacy"
test ! -e "$ROOT/etc/systemd/system/pm3-legacy.service"
test ! -L "$ROOT/home/legacy/.config/systemd/user/pm3.service"
mv "$ROOT/cli.backup" "$ROOT/bundle/pm3"
run --user legacy
test "$(readlink "$ROOT/home/legacy/.config/systemd/user/pm3.service")" = /dev/null
test ! -e "$ROOT/run/legacy-active-legacy"
grep -q saved "$ROOT/home/legacy/.pm3/configs/app.proc"
grep -q '^User=legacy$' "$ROOT/etc/systemd/system/pm3-legacy.service"
grep -q original "$ROOT/etc/systemd/user/pm3.service"
test "$(readlink "$ROOT/usr/bin/pm3/pm3")" = /usr/local/bin/pm3
test -f "$ROOT/usr/bin/pm3/pm3.legacy"
test "$(readlink "$ROOT/usr/bin/pm3/install.sh")" = /opt/pm3/current/install.sh
grep -q 'legacy installer' "$ROOT/usr/bin/pm3/install.sh.legacy"
run --user legacy
echo 'Per-user installer lifecycle tests passed'
