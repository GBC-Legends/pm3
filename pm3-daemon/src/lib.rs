mod command_handler;
mod models;
mod process_runner;
mod utils;

use crate::command_handler::commands::RunnerCommand;
use command_handler::tcp_listener::TcpCommandHandler;
use process_runner::runner;
use tokio::sync::mpsc;

pub async fn start_application() -> anyhow::Result<()> {
    let mut pm3_runner = runner::ProcessRunner::init();

    let (tx, mut rx) = mpsc::channel::<RunnerCommand>(5);

    match pm3_runner.run().await {
        Ok(()) => println!("PM3 daemon initialized successfully"),
        Err(e) => eprintln!("PM3 daemon failed to initialize: {e}"),
    }

    tokio::spawn(async move {
        pm3_runner.dispatch(&mut rx).await;
    });

    let tcp = TcpCommandHandler::new("127.0.0.1:8046", tx).await?;
    tcp.run().await?;

    Ok(())
}
