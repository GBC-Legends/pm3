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
use std::time::Duration;
use tokio::sync::{mpsc, oneshot};

#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

pub async fn start_application() -> anyhow::Result<()> {
    let shutdown_signal = shutdown_signal()?;
    let home = utils::pm3_safe_dir::pm3_home_dir_safe();
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
            "a daemon already owns {} or it cannot be locked: {e}",
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

    let (tx, mut rx) = mpsc::channel::<RunnerCommand>(5);
    let tcp = TcpCommandHandler::new(&pm3_cfg, tx, cli_listener).await?;
    let api_listener = tokio::net::TcpListener::from_std(api_listener)?;
    let (stop_logs_tx, stop_logs_rx) = oneshot::channel();
    let mut logging_task = tokio::spawn(LoggingService::dispatch(
        logging_rx,
        logging_subs_rx,
        stop_logs_rx,
    ));
    let metrics_task = tokio::spawn(MetricsService::dispatch(metrics_rx));

    let restore_result = if std::env::var("PM3_RESTORE").as_deref() == Ok("1") {
        pm3_runner.restore().await
    } else {
        Ok(())
    };

    let (stop_runner_tx, stop_runner_rx) = oneshot::channel();
    let mut runner_task = tokio::spawn(async move {
        pm3_runner.dispatch(&mut rx, stop_runner_rx).await;
        pm3_runner.shutdown().await
    });

    let serving_result: anyhow::Result<()> = async {
        restore_result?;
        pm3_cfg.publish()?;
        println!("PM3 daemon ready; user state: {}", home.display());
        tokio::select! {
            biased;
            result = shutdown_signal => result.map_err(Into::into),
            result = tcp.run() => result,
            result = exposing_service.dispatch(api_listener) => result,
        }
    }
    .await;
    drop(tcp);

    // Stop accepting commands, terminate applications, then drain their pipes.
    // These bounds leave headroom within systemd's 30-second stop timeout.
    let _ = stop_runner_tx.send(());
    let runner_result = match tokio::time::timeout(Duration::from_secs(15), &mut runner_task).await
    {
        Ok(result) => result.map_err(anyhow::Error::from).and_then(|r| r),
        Err(error) => {
            runner_task.abort();
            Err(anyhow::anyhow!("process shutdown timed out: {error}"))
        }
    };
    let readers_result =
        match tokio::time::timeout(Duration::from_secs(5), LoggingService::drain_readers()).await {
            Ok(result) => result,
            Err(error) => Err(anyhow::anyhow!("log pipe drain timed out: {error}")),
        };
    LoggingService::close_input();
    let _ = stop_logs_tx.send(());
    let logs_result = match tokio::time::timeout(Duration::from_secs(5), &mut logging_task).await {
        Ok(result) => result.map_err(anyhow::Error::from).and_then(|r| r),
        Err(error) => {
            logging_task.abort();
            Err(anyhow::anyhow!("final log flush timed out: {error}"))
        }
    };
    metrics_task.abort();
    let _ = metrics_task.await;

    // Report failures only after every cleanup stage has been attempted.
    let mut failure = None;
    for result in [serving_result, runner_result, readers_result, logs_result] {
        if let Err(error) = result {
            eprintln!("[pm3] shutdown error: {error:#}");
            failure = Some(error);
        }
    }
    if let Some(error) = failure {
        return Err(error);
    }
    println!("PM3 shutdown complete; log pipes drained and files flushed");
    Ok(())
}

fn shutdown_signal() -> std::io::Result<impl std::future::Future<Output = std::io::Result<()>>> {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{SignalKind, signal};
        // Register before restoring applications, so early TERM/INT is queued.
        let mut terminate = signal(SignalKind::terminate())?;
        let mut interrupt = signal(SignalKind::interrupt())?;
        Ok(async move {
            tokio::select! {
                _ = terminate.recv() => {},
                _ = interrupt.recv() => {},
            }
            Ok(())
        })
    }
    #[cfg(not(unix))]
    {
        Ok(async { tokio::signal::ctrl_c().await })
    }
}
