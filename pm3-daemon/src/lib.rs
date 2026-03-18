mod command_handler;
mod daemon_config;
mod logging;
mod metrics;
mod models;
mod process_runner;
mod utils;

use crate::command_handler::commands::RunnerCommand;
use crate::logging::logging_service::LoggingService;
use crate::metrics::metrics_service::MetricsService;
use command_handler::tcp_listener::TcpCommandHandler;
use process_runner::runner;
use tokio::sync::mpsc;

pub async fn start_application() -> anyhow::Result<()> {
    let pm3_cfg = daemon_config::DaemonConfig::new()?;

    let (logging_rx, logging_subs_tx, logging_subs_rx) = LoggingService::init();
    let metrics_rx = MetricsService::init();

    tokio::spawn(async move {
        LoggingService::dispatch(logging_rx, logging_subs_rx).await;
    });
    tokio::spawn(async move {
        MetricsService::dispatch(metrics_rx).await;
    });

    let mut pm3_runner = runner::ProcessRunner::init(logging_subs_tx);

    let (tx, mut rx) = mpsc::channel::<RunnerCommand>(5);

    match pm3_runner.run().await {
        Ok(()) => println!("PM3 daemon initialized successfully"),
        Err(e) => eprintln!("PM3 daemon failed to initialize: {e}"),
    }

    tokio::spawn(async move {
        pm3_runner.dispatch(&mut rx).await;
    });

    let tcp = TcpCommandHandler::new(&pm3_cfg, tx).await?;
    tcp.run().await?;

    Ok(())
}
