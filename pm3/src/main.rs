mod tcp_connector;

use clap::{Parser, Subcommand};

#[derive(Parser)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Ping,
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Ping => tcp_connector::ping_server(),
    }
}