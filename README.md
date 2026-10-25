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
For automated installation on Linux, run the `install.sh` script. It will download the latest release and set up the binaries and systemd service for the daemon.
