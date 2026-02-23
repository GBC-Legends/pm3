use std::process::Stdio;
use std::sync::OnceLock;
use std::time::Duration;
use tokio::fs::OpenOptions;
use tokio::io::AsyncWriteExt;
use tokio::runtime::Handle;
use tokio::sync::mpsc;

use crate::logging::logging_instance::{LogMsg, LoggingInstance, StreamKind};

pub struct LoggingService;

static GLOBAL_TX: OnceLock<mpsc::UnboundedSender<LogMsg>> = OnceLock::new();

impl LoggingService {
    pub fn init() -> mpsc::UnboundedReceiver<LogMsg> {
        if GLOBAL_TX.get().is_some() {
            panic!("LoggingService::init() called more than once");
        }

        let (tx, rx) = mpsc::unbounded_channel::<LogMsg>();
        let _ = GLOBAL_TX.set(tx);

        rx
    }

    pub async fn dispatch(mut rx: mpsc::UnboundedReceiver<LogMsg>) {
        let mut stdout_file = OpenOptions::new()
            .create(true)
            .append(true)
            .open("../stdout.log")
            .await
            .expect("failed to open stdout.log");

        let mut stderr_file = OpenOptions::new()
            .create(true)
            .append(true)
            .open("../stderr.log")
            .await
            .expect("failed to open stderr.log");

        let mut tick = tokio::time::interval(Duration::from_secs(15));
        tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

        let mut bytes_last_15s: u64 = 0;

        loop {
            tokio::select! {
                msg = rx.recv() => {
                    match msg {
                        Some(msg) => {
                            let len = msg.bytes.len() as u64;
                            bytes_last_15s += len;

                            match msg.stream {
                                StreamKind::Stdout => {
                                    if let Err(e) = stdout_file.write_all(&msg.bytes).await {
                                        eprintln!("stdout write error: {}", e);
                                    }
                                }
                                StreamKind::Stderr => {
                                    if let Err(e) = stderr_file.write_all(&msg.bytes).await {
                                        eprintln!("stderr write error: {}", e);
                                    }
                                }
                            }
                        }
                        None => break,
                    }
                }

                _ = tick.tick() => {
                    println!(
                        "[pm3][stats] received {} bytes in last 15s (~{} B/s)",
                        bytes_last_15s,
                        bytes_last_15s / 15
                    );

                    bytes_last_15s = 0;
                }
            }
        }
    }

    pub fn get_logging_pair(proc_name: &str) -> (Stdio, Stdio) {
        let handle = Handle::try_current()
            .expect("LoggingService::get_logging_pair requires a running tokio runtime");

        let tx = GLOBAL_TX
            .get()
            .expect("LoggingService::init() must be called before get_logging_pair()")
            .clone();

        let out = LoggingInstance::new(
            proc_name.to_string(),
            StreamKind::Stdout,
            tx.clone(),
            handle.clone(),
        );

        let err = LoggingInstance::new(proc_name.to_string(), StreamKind::Stderr, tx, handle);

        (out.into(), err.into())
    }
}
