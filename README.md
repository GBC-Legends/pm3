# PM3

PM3 is a process manager consisting of a command-line interface (CLI), a daemon for background monitoring, and a web dashboard for visualization.

## Project Structure

- `pm3/` - Main CLI application built with Rust. Provides a terminal user interface for managing processes.
- `pm3-daemon/` - Background daemon service built with Rust. Runs as a server to monitor and control processes, using SQLite for storage and Axum for HTTP API.
- `pm3-runtime/` - Shared instance naming and state directory resolution for the CLI and daemon.
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
4. On Linux, run `PM3_HOME=/absolute/path/to/dev-state cargo run -- list` to connect to a development daemon using the same state directory.

### Building and Running pm3-daemon
1. Navigate to the `pm3-daemon/` directory.
2. Ensure Rust and Cargo are installed.
3. Run `cargo build --release` to build the daemon.
4. On Linux, run `PM3_HOME=/absolute/path/to/dev-state cargo run` to start a development daemon. Without this explicit override, Linux uses the system instance directory.

### Building and Running pm3-dashboard
1. Navigate to the `pm3-dashboard/` directory.
2. Ensure Node.js and npm are installed.
3. Run `npm install` to install dependencies.
4. Run `npm run dev` to start the development server (uses Vite).
5. Open your browser to the provided local URL to access the dashboard.

For production builds of the dashboard, run `npm run build` to generate static files in the `dist/` directory.

### Installation

Linux installations use **system-wide named instances**, managed by the system
systemd manager. `pm3` always selects `default`, regardless of the login user,
`HOME`, or whether it was invoked through sudo. There are no per-login daemons,
user systemd units, shell profile edits, or linger requirements for new installs.

Download `install.sh` from a release containing this installation model, then run:

```sh
sudo sh install.sh --yes
sudo pm3 start /absolute/path/to/app --name app
sudo pm3 save
sudo pm3 list
```

The installer downloads the latest compatible release, installs the CLI in
`/usr/local/bin`, and enables and starts `pm3@default.service` at boot. Linux with
systemd and glibc is required. Release builds target x86-64 (Ubuntu 22.04 or newer
glibc) and ARM64 (Ubuntu 24.04 or newer glibc, including suitable EC2 Graviton
images). Alpine/musl and systems without systemd are not supported by this installer.
Older release archives are rejected before modifying the running installation.

By default, the daemon and managed applications run as **root**; control them with
`sudo pm3`. To run applications as an existing administrator/application account:

```sh
sudo sh install.sh --yes --user ubuntu
pm3 list                         # as ubuntu
sudo pm3 list                    # the same instance
```

The execution account controls process permissions, not instance identity. Other
accounts do not receive access to its control token. Administrators use sudo;
the configured account can use the CLI directly. Settings belong to root, while
the execution account owns private state (0700) and discovery credentials (0600).
Changing an existing instance's execution account is rejected to prevent silently
changing the privileges of saved applications.

#### Additional instances

```sh
sudo sh install.sh --yes --instance api --user ubuntu
sudo pm3 --instance api start /srv/api/server --name server
sudo pm3 --instance api save
sudo pm3 --instance api list
systemctl list-units --all 'pm3@*.service'
sudo systemctl restart pm3@api
sudo journalctl -u pm3@api -f
```

Names contain 1–32 lowercase letters, digits, underscores or hyphens and start
with a letter or digit. `PM3_INSTANCE=api pm3 list` also selects an instance;
`--instance` takes precedence. With sudo, prefer the explicit CLI option because
sudo may discard environment variables. Creating another instance is explicit;
running the CLI never creates a daemon or a state directory. There is no imposed
five-instance limit.

| Purpose | Location |
| --- | --- |
| Shared binaries and dashboard | `/opt/pm3/current` (points to a complete release) |
| CLI | `/usr/local/bin/pm3` |
| System service template | `/etc/systemd/system/pm3@.service` |
| Instance execution account | `/etc/systemd/system/pm3@NAME.service.d/pm3.conf` |
| Network settings | `/etc/pm3/NAME.env` |
| State, saved processes, logs, SQLite and private discovery token | `/var/lib/pm3/NAME/` |

New dashboard ports are allocated from 8096 upward and saved in each instance's
settings. `PM3_IP=127.0.0.1` keeps the dashboard local. The dashboard/API currently
has no authentication: use an SSH tunnel for remote access. Set `PM3_API_PORT` to
a free port if another application uses the allocated one, then restart the unit.
An occupied explicitly configured port fails startup with an error in the journal.
The encrypted CLI listener binds only to loopback, reserves an available port and
publishes it in `config.proc`; CLI and monitor discover the correct instance's
endpoints automatically. Optional `PM3_PORT` pins the CLI port.

Run `pm3 save` after process changes: saved definitions are restored on service
startup, including boot. Restarting a unit stops its entire process group before
restoring the saved definitions. Applications inherit the service environment;
systemd does not source `.bashrc`, nvm, or a login shell. Use absolute interpreter
paths and `systemctl edit pm3@NAME` for application environment/PATH overrides.

#### Updates and removal

```sh
sudo sh /opt/pm3/current/install.sh --yes
sudo sh install.sh --yes --version vX.Y.Z
sudo sh install.sh --yes --instance api --uninstall
```

All instances share one binary release. An update stages and validates the full
archive first, saves active system instances, atomically changes the release
symlink, then restarts and checks every active instance plus the selected instance.
Expect a brief application interruption. Existing settings and state are retained,
including on repeat installs of the same version. Failed readiness checks restore
the previous binary release and attempt to restart the affected instances; inspect
the journal if the underlying configuration is invalid. Old release directories
remain under `/opt/pm3/releases` for manual recovery; this is a binary rollback,
not a database/schema rollback. Download failures leave the running installation
untouched. Concurrent installers are rejected.

`--uninstall` stops, disables and masks only the selected instance. It retains data,
settings, its execution account and shared binaries. Reinstalling that instance
unmasks it and reuses the retained settings. Remove retained data manually only
when no longer needed.

#### Migration from the old user installation

Migrate each old user daemon into a distinct named system instance. Migration is
explicit and preserves its execution account and absolute application paths:

```sh
# As the old service's user, first check/save the running processes:
/usr/bin/pm3/pm3 list
/usr/bin/pm3/pm3 dump

# From an administrator shell (use default for the main instance):
sudo sh install.sh --yes --instance default --migrate-user ubuntu
sudo /usr/local/bin/pm3 list
sudo systemctl status pm3@default
```

The old user's systemd manager must be reachable during migration. The installer
saves its live processes, stops/disables/masks that user's `pm3.service`, copies
the stopped `~/.pm3` state, and creates new discovery credentials. It keeps the old
directory and binaries as backups, preserves the dashboard settings, and refuses
to overwrite an existing target instance. Other users' services remain running
until migrated separately; review their API ports for conflicts. Linger is not
disabled because unrelated user services may need it.

Remove the old `export PATH="/usr/bin/pm3:$PATH"` line from the affected shell
profiles and run `hash -r`; `command -v pm3` should resolve to `/usr/local/bin/pm3`.
Once all users are migrated, retire the shared legacy unit so new logins cannot
start it: replace `/etc/systemd/user/pm3.service` with a `/dev/null` symlink and
reload any remaining user managers. Keep the original unit in your migration
backup. If migration fails after stopping the old unit, stop the new system unit
before unmasking and restarting the old user unit; never run both against the
same applications. No implicit fallback to `~/.pm3` happens on Linux.

#### Trying this branch before a release

Build both Rust binaries and the dashboard, then assemble a directory containing
`pm3`, `pm3-daemon`, `dashboard/index.html` and the remaining dashboard assets:

```sh
cargo build --release --manifest-path pm3/Cargo.toml
cargo build --release --manifest-path pm3-daemon/Cargo.toml
(cd pm3-dashboard && npm ci && VITE_API_URL=/api/v1/ npm run build)
mkdir -p bundle/dashboard
cp pm3/target/release/pm3 pm3-daemon/target/release/pm3-daemon bundle/
cp -R pm3-dashboard/dist/. bundle/dashboard/
cp install.sh bundle/
sudo sh install.sh --yes --from-dir "$PWD/bundle"
```

For development/testing, `PM3_HOME=/absolute/path` explicitly overrides the state
directory for both binaries. `PM3_CONFIG=/absolute/path/file.env` overrides the
daemon settings file; otherwise a development home reads `pm3.env` inside it.
Windows and macOS retain their existing default home-based locations. Run shared
unit tests with `cargo test --manifest-path pm3-runtime/Cargo.toml`, installer
lifecycle tests with `sudo sh tests/install-system.sh`, and real Linux daemon
tests (after debug builds) with `sh tests/linux-instances.sh`.
`sudo sh tests/systemd-lifecycle.sh` additionally checks real systemd restart,
saved process restoration and child cleanup using a temporary test unit.
