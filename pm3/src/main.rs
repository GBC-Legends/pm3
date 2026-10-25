use clap::Parser;
use pm3::{Cli, process_commands};

fn main() {
    let args: Vec<String> = std::env::args().collect();

    if args.len() == 2 && (args[1] == "-v" || args[1] == "-V" || args[1] == "--version") {
        println!("pm3 {}", env!("CARGO_PKG_VERSION"));
        return;
    }

    let cli = Cli::parse();

    if let Some(instance) = &cli.instance {
        pm3_runtime::select_instance(instance).expect("instance selection failed");
    }
    if let Err(e) = pm3_runtime::state_dir() {
        eprintln!("pm3: {e}");
        std::process::exit(2);
    }

    process_commands(cli.command)
}
