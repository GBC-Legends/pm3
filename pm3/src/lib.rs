mod tcp_connector;

use clap::{Parser, Subcommand};
use crate::tcp_connector::ping::ping_server;
use crate::tcp_connector::start::start_program;

#[derive(Parser)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    Ping,
    Start {
        program: String,
        args: Vec<String>,
    },
}

pub fn process_commands(cmd: Commands) {
    match cmd {
        Commands::Ping => ping_server(),
        Commands::Start { program, args} => start_program(program, args),
    }
}