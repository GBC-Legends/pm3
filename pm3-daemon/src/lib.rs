mod api;
mod command_handler;
mod daemon_config;
mod logging;
mod metrics;
mod models;
mod process_runner;
mod utils;

use crate::api::metrics_exposing_service::ExposingService;
use crate::command_handler::commands::RunnerCommand;
use crate::logging::logging_service::LoggingService;
use crate::metrics::metrics_service::MetricsService;

use command_handler::tcp_listener::TcpCommandHandler;
use process_runner::runner;
use tokio::sync::mpsc;

#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

pub async fn start_application() -> anyhow::Result<()> {
    let home = pm3_runtime::state_dir()?;
    std::fs::create_dir_all(&home)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&home, std::fs::Permissions::from_mode(0o700))?;
    }
    let lock = std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(home.join("daemon.lock"))?;
    lock.try_lock().map_err(|e| {
        anyhow::anyhow!(
            "PM3 instance at {} is already running or cannot be locked: {e}",
            home.display()
        )
    })?;
    let (pm3_cfg, cli_listener, api_listener) = daemon_config::DaemonConfig::new()?;

    let (logging_rx, logging_subs_tx, logging_subs_rx) = LoggingService::init();

    let (mut pm3_runner, processes) = runner::ProcessRunner::init(logging_subs_tx.clone());

    let (metrics_rx, db_path) = MetricsService::init(processes);
    let exposing_service = ExposingService::init(
        &pm3_cfg.metrics_api_addr,
        pm3_cfg.metrics_api_port,
        db_path,
        logging_subs_tx.clone(),
    );

    tokio::spawn(async move {
        LoggingService::dispatch(logging_rx, logging_subs_rx).await;
    });
    tokio::spawn(async move {
        MetricsService::dispatch(metrics_rx).await;
    });

    let (tx, mut rx) = mpsc::channel::<RunnerCommand>(5);

    pm3_runner.restore().await?;

    tokio::spawn(async move {
        pm3_runner.dispatch(&mut rx).await;
    });

    let tcp = TcpCommandHandler::new(&pm3_cfg, tx, cli_listener).await?;
    let api_listener = tokio::net::TcpListener::from_std(api_listener)?;
    pm3_cfg.dump()?;
    println!(
        "PM3 instance {} ready at {}",
        pm3_runtime::instance()?,
        home.display()
    );
    tokio::select! {
        result = tcp.run() => result?,
        result = exposing_service.dispatch(api_listener) => result?,
    }

    Ok(())
}
