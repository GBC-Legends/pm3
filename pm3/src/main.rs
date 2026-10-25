use clap::Parser;
use pm3::{Cli, process_commands};

fn main() {
    let args: Vec<String> = std::env::args().collect();

    if args.len() == 2 && args[1] == "--installation-model" {
        println!("per-user-systemd-v1");
        return;
    }

    if args.len() == 2 && (args[1] == "-v" || args[1] == "-V" || args[1] == "--version") {
        println!("pm3 {}", env!("CARGO_PKG_VERSION"));
        return;
    }

    let cli = Cli::parse();

    process_commands(cli.command)
}
