mod tcp_connector;

use clap::{Parser, Subcommand};
use crate::tcp_connector::ping::ping_server;

#[derive(Parser)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    Ping,
    Start,
}

pub fn process_commands(cmd: Commands) {
    match cmd {
        Commands::Ping => ping_server(),
        Commands::Start => println!("Starting..."),
    }
}