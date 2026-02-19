mod tcp_connector;
mod utils;

use crate::tcp_connector::ping::ping_server;
use crate::tcp_connector::start::start_program;
use clap::{Args, Parser, Subcommand};

#[derive(Parser)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    Ping,
    Start(StartArgs),
}

#[derive(Args, Debug)]
pub struct StartArgs {
    program: String,
    args: Vec<String>,

    #[arg(long)]
    interpreter: Option<String>,
    #[arg(long)]
    name: Option<String>,
}

pub fn process_commands(cmd: Commands) {
    match cmd {
        Commands::Ping => match ping_server() {
            Ok(response) => println!("{}", response),
            Err(err) => println!("Error: {}", err),
        },
        Commands::Start(args) => {
            match start_program(args.program, args.args, args.interpreter, args.name) {
                Ok(response) => println!("{}", response),
                Err(err) => println!("Error: {}", err),
            }
        }
    };
}
