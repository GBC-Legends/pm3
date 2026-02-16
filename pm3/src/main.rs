use clap::Parser;
use pm3::{process_commands, Cli};

fn main() {
    let cli = Cli::parse();

    process_commands(cli.command)
}