# PM3

PM3 is a process manager consisting of a command-line interface (CLI), a daemon for background monitoring, and a web dashboard for visualization.

## Project Structure

- `pm3/` - Main CLI application built with Rust. Provides a terminal user interface for managing processes.
- `pm3-daemon/` - Background daemon service built with Rust. Runs as a server to monitor and control processes, using SQLite for storage and Axum for HTTP API.
- `pm3-dashboard/` - Web dashboard built with React and Vite. Offers a graphical interface to view process statistics and manage tasks.
- `.github/workflows/` - CI/CD pipelines for building and releasing the project.
- `install.sh` - Installation script for deploying PM3 on Linux systems.

## Requirements

### For pm3 and pm3-daemon
- Rust (latest stable version) and Cargo (Rust's package manager).

### For pm3-dashboard
- Node.js (version 22 or later) and npm.
- Vite (included as a dev dependency).

## Setup and Running

### Building and Running pm3 (CLI)
1. Navigate to the `pm3/` directory.
2. Ensure Rust and Cargo are installed.
3. Run `cargo build --release` to build the project.
4. Run `cargo run` to start the CLI application.

### Building and Running pm3-daemon
1. Navigate to the `pm3-daemon/` directory.
2. Ensure Rust and Cargo are installed.
3. Run `cargo build --release` to build the daemon.
4. Run `cargo run` to start the daemon service.

### Building and Running pm3-dashboard
1. Navigate to the `pm3-dashboard/` directory.
2. Ensure Node.js and npm are installed.
3. Run `npm install` to install dependencies.
4. Run `npm run dev` to start the development server (uses Vite).
5. Open your browser to the provided local URL to access the dashboard.

For production builds of the dashboard, run `npm run build` to generate static files in the `dist/` directory.

### Installation

PM3 keeps **one daemon per Unix user**. Each daemon is supervised by a separate
**system** systemd service: `pm3-root.service`, `pm3-ubuntu.service`, etc. It runs
as that user and keeps the user's existing `~/.pm3` directory. Logging in or
starting a per-user systemd manager is not required.

Download `install.sh` from a compatible PM3 release and run:

```sh
sudo sh install.sh --yes              # installs for the sudo caller
pm3 start /absolute/path/to/app --name app
pm3 save
pm3 list
```

Running the installer directly as root installs `pm3-root`. Explicit account
selection takes precedence over `SUDO_USER`:

```sh
sudo sh install.sh --yes --user root
sudo sh install.sh --yes --user ubuntu

sudo systemctl status pm3-root pm3-ubuntu
sudo systemctl restart pm3-ubuntu
sudo journalctl -u pm3-ubuntu -f
systemctl list-units --all 'pm3-*.service'
```

The account must already exist and have a home directory. Normal Linux account
names containing letters, digits, underscores, dots and hyphens are supported;
names beginning with a digit, dot or hyphen are rejected.

| Account | System service | Process owner | State and settings |
| --- | --- | --- | --- |
| root | `pm3-root.service` | root | `/root/.pm3/` |
| ubuntu | `pm3-ubuntu.service` | ubuntu | `/home/ubuntu/.pm3/` |

The actual home comes from the system account database, not the installer's
`HOME`. Binaries and dashboard are shared under `/opt/pm3/current`, with the CLI
available as `/usr/local/bin/pm3`. No shell-profile edits or linger configuration
are needed. Service files are written to `/etc/systemd/system/pm3-USER.service`.

Run `pm3` as the account whose processes you want to manage. The CLI retains its
home-based discovery; it does not switch to a shared default daemon. When
administering another account, set both identity and home:

```sh
pm3 list                         # current user's daemon
sudo -H pm3 list                 # root's daemon
sudo -H -u ubuntu pm3 list       # ubuntu's daemon
```

Avoid preserving another user's `HOME` when switching accounts. Each `.pm3`
directory is private (0700), and its discovery credentials are private (0600).
The daemon refuses a second launch against the same directory.

#### Configuration and boot

Edit `~/.pm3/pm3.env` as the service's user and restart `pm3-USER`. It contains
`PM3_IP` and `PM3_API_PORT`. A new user's daemon reserves an available port and
writes it to this file. Existing settings are retained. An explicitly configured
port conflict fails startup with a journal error, rather than silently disabling
the dashboard. Optional `PM3_PORT` pins the encrypted CLI port; normally the CLI
discovers it automatically. The terminal monitor also discovers the selected
user's API address, so different users' metrics do not get mixed together.

The dashboard binds to loopback by default and currently has no authentication;
use an SSH tunnel for remote access. App environment defaults in `pm3.env` are
preserved, with the service environment taking precedence. systemd does not source
login shell files such as `.bashrc` or nvm setup. Use absolute interpreter paths
and `sudo systemctl edit pm3-USER` for service environment/PATH overrides.

Run `pm3 save` after changing processes. Installed services set `PM3_RESTORE=1`
and restore saved definitions at startup/boot. Saved stopped processes remain
stopped. A service stop/restart terminates that user's managed process group.
Manual daemon launches retain explicit `pm3 revive` behavior unless
`PM3_RESTORE=1` is set.

On Linux, SIGTERM/SIGINT and `systemctl stop`/`restart` stop the daemon's command
loop, send SIGTERM to each application's process group, and allow up to 10 seconds
for application shutdown before killing remaining group members. PM3 then drains
stdout/stderr to EOF, writes queued log buffers, and waits for file writes before
exiting. The system unit uses `KillMode=mixed` so systemd lets the daemon perform
this sequence first; its 30-second timeout remains the final cleanup bound.
Applications should finish their own buffered output in their SIGTERM handlers.
SIGKILL, power loss, failed storage, or applications that outlive the grace period
cannot provide the same guarantee. Shutdown/drain/write failures are reported as
a nonzero daemon exit instead of a successful shutdown.

#### Updates, migration and removal

```sh
sudo sh /opt/pm3/current/install.sh --yes --user ubuntu
sudo sh install.sh --yes --user ubuntu --version vX.Y.Z
sudo sh install.sh --yes --user ubuntu --uninstall
```

Updates stage and validate a complete release before switching binaries. Because
binaries are shared, the installer saves and restarts all active **managed PM3
system services**, plus the selected user's service. Expect a brief interruption
for their applications. Other users' identities, homes and settings are retained.
Repeat installation does not skip service setup just because versions match.
Concurrent installers are rejected. On a failed readiness check, the previous
binary release and selected service definition are restored and previously active
services are restarted. This is a binary rollback, not a database-schema rollback.
Old releases remain under `/opt/pm3/releases` for recovery.

The old official `/etc/systemd/user/pm3.service` installation is detected
automatically. For the selected account, the installer saves its live processes,
stops/disables the old user service, masks it for that account, and starts
`pm3-USER.service` using the **same `~/.pm3` directory**. It does not merge or move
users' data. Other users can be migrated separately. Failed migration restores
the selected account's former user service. Existing custom local user units are
reported for manual migration before any service is changed. A running legacy
daemon with an unreachable user manager must be stopped first.

The old `/usr/bin/pm3/pm3` path forwards to the current CLI, so old PATH entries
continue to work. The original CLI is saved as `/usr/bin/pm3/pm3.legacy`; old daemon
binaries remain available for users not yet migrated. Linger is left alone because
other services may depend on it. The old installed `install.sh` also forwards to
the new installer, with its original saved as `install.sh.legacy`.
Once all users have migrated, the administrator
can retire the shared legacy user unit and old binaries.

`--uninstall` stops/disables and removes only the selected user's managed system
service. It preserves that user's `.pm3` and the shared binaries. It does not
reenable the old user service.

#### Supported Linux systems and testing this branch

The official installer requires Linux with a running systemd system manager and
glibc. Release builds cover x86-64 (Ubuntu 22.04/glibc 2.35 baseline) and ARM64
(Ubuntu 24.04/glibc 2.39 baseline). Minimal hosts need `curl`, `tar`, standard
coreutils, `getent`, `runuser` and `flock`. Alpine/musl and hosts without systemd
are outside this installer. Incompatible older release archives are rejected
before changing services.

Before publishing a release, build and install this branch from a local bundle:

```sh
cargo build --release --manifest-path pm3/Cargo.toml
cargo build --release --manifest-path pm3-daemon/Cargo.toml
(cd pm3-dashboard && npm ci && VITE_API_URL=/api/v1/ npm run build)
mkdir -p bundle/dashboard
cp pm3/target/release/pm3 pm3-daemon/target/release/pm3-daemon bundle/
cp -R pm3-dashboard/dist/. bundle/dashboard/
cp install.sh bundle/
sudo sh install.sh --yes --from-dir "$PWD/bundle" --user ubuntu
```

`sudo sh tests/install-users.sh` exercises the installer in a disposable chroot.
After debug builds, `sudo sh tests/linux-users.sh` tests real systemd daemons
under root and an unprivileged account, using temporary homes and transient units.
Neither test installs PM3 into the host's normal service/data paths. Rust tests
run with `cargo test` in both crate directories. Windows/macOS retain their
existing home-based directory conventions.
