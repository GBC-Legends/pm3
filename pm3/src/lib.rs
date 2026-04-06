mod models;
mod tcp_connector;
mod utils;

use crate::tcp_connector::delete::delete_program;
use crate::tcp_connector::dump::dump_program;
use crate::tcp_connector::flush::flush_program;
use crate::tcp_connector::ping::ping_server;
use crate::tcp_connector::restart::restart_program;
use crate::tcp_connector::revive::revive_program;
use crate::tcp_connector::start::start_program;
use crate::tcp_connector::status::request_status;
use crate::tcp_connector::stop::stop_program;
use crate::tcp_connector::{logs::request_logs, monitor::open_monitor};

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
    Stop(StopArgs),
    Restart(RestartArgs),
    Reload(RestartArgs),
    Revive,
    Resurrect,
    Delete(DeleteArgs),
    Remove(DeleteArgs),
    Rm(DeleteArgs),
    Status,
    List,
    Ls,
    Logs(LogsArgs),
    Flush(FlushArgs),
    Clear(FlushArgs),
    Dump,
    Save,
    Monit,
    Monitor,
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

#[derive(Args, Debug)]
pub struct StopArgs {
    programs: Vec<String>,
}

#[derive(Args, Debug)]
pub struct RestartArgs {
    programs: Vec<String>,
}

#[derive(Args, Debug)]
pub struct DeleteArgs {
    programs: Vec<String>,
}

#[derive(Args, Debug)]
pub struct FlushArgs {
    programs: Vec<String>,
}

#[derive(Args, Debug)]
pub struct LogsArgs {
    #[arg(long)]
    pub lines: Option<u64>,
    pub programs: Vec<String>,
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
                Err(err) => println!("pm3: {}", err),
            }
        }
        Commands::Stop(args) => match stop_program(args.programs) {
            Ok(response) => println!("{}", response),
            Err(err) => println!("pm3: {}", err),
        },
        Commands::Restart(args) | Commands::Reload(args) => match restart_program(args.programs) {
            Ok(response) => println!("{}", response),
            Err(err) => println!("pm3: {}", err),
        },
        Commands::Delete(args) | Commands::Remove(args) | Commands::Rm(args) => {
            match delete_program(args.programs) {
                Ok(response) => println!("{}", response),
                Err(err) => println!("pm3: {}", err),
            }
        }
        Commands::Revive | Commands::Resurrect => match revive_program() {
            Ok(response) => println!("{}", response),
            Err(err) => println!("pm3: {}", err),
        },
        Commands::Dump | Commands::Save => match dump_program() {
            Ok(response) => println!("{}", response),
            Err(err) => println!("pm3: {}", err),
        },
        Commands::Flush(args) | Commands::Clear(args) => match flush_program(args.programs) {
            Ok(response) => println!("{}", response),
            Err(err) => println!("pm3: {}", err),
        },
        Commands::Status | Commands::List | Commands::Ls => match request_status() {
            Ok(_) => {}
            Err(err) => eprintln!("pm3: {}", err),
        },
        Commands::Logs(args) => match request_logs(args.lines, args.programs) {
            Ok(_) => {}
            Err(err) => eprintln!("pm3: {}", err),
        },
        Commands::Monit | Commands::Monitor => match open_monitor() {
            Ok(_) => {}
            Err(err) => eprintln!("pm3: {}", err),
        },
    };
}
