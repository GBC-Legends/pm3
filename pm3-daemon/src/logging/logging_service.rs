use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

use tokio::fs::{self, OpenOptions};
use tokio::io::AsyncWriteExt;
use tokio::runtime::Handle;
use tokio::sync::mpsc;

use crate::logging::logging_instance::{LogMsg, LoggingInstance, StreamKind};
use crate::utils::pm3_safe_dir;

pub struct LoggingService;

static GLOBAL_TX: OnceLock<mpsc::UnboundedSender<LogMsg>> = OnceLock::new();
static PATH_MAP: OnceLock<Mutex<HashMap<String, (PathBuf, PathBuf)>>> = OnceLock::new();

impl LoggingService {
    pub fn init() -> mpsc::UnboundedReceiver<LogMsg> {
        if GLOBAL_TX.get().is_some() {
            panic!("LoggingService::init() called more than once");
        }

        let (tx, rx) = mpsc::unbounded_channel::<LogMsg>();
        let _ = GLOBAL_TX.set(tx);

        let _ = PATH_MAP.set(Mutex::new(HashMap::new()));

        rx
    }

    pub fn register_proc(proc_name: &str, stdout_path: PathBuf, stderr_path: PathBuf) {
        let map = PATH_MAP.get().expect("PATH_MAP not initialized");
        let mut map = map.lock().expect("PATH_MAP poisoned");
        map.insert(proc_name.to_string(), (stdout_path, stderr_path));
    }

    fn ensure_paths(proc_name: &str) -> (PathBuf, PathBuf) {
        let map = PATH_MAP.get().expect("PATH_MAP not initialized");
        let mut map = map.lock().expect("PATH_MAP poisoned");

        if let Some((out, err)) = map.get(proc_name) {
            return (out.clone(), err.clone());
        }

        let pm3_home_dir = pm3_safe_dir::pm3_home_dir_safe();
        let logs_dir = pm3_home_dir.join("processes").join(proc_name);
        let stdout_path = logs_dir.join("stdout.log");
        let stderr_path = logs_dir.join("stderr.log");

        map.insert(
            proc_name.to_string(),
            (stdout_path.clone(), stderr_path.clone()),
        );
        (stdout_path, stderr_path)
    }

    async fn open_files(
        stdout_path: &PathBuf,
        stderr_path: &PathBuf,
    ) -> std::io::Result<(tokio::fs::File, tokio::fs::File)> {
        if let Some(parent) = stdout_path.parent() {
            fs::create_dir_all(parent).await?;
        }

        let out_f = OpenOptions::new()
            .create(true)
            .append(true)
            .open(stdout_path)
            .await?;

        let err_f = OpenOptions::new()
            .create(true)
            .append(true)
            .open(stderr_path)
            .await?;

        Ok((out_f, err_f))
    }

    async fn ensure_file_handles(
        proc_name: &str,
        stdout_path: &PathBuf,
        stderr_path: &PathBuf,
        file_cache: &mut HashMap<String, (tokio::fs::File, tokio::fs::File)>,
    ) -> bool {
        if file_cache.contains_key(proc_name) {
            return true;
        }

        match Self::open_files(stdout_path, stderr_path).await {
            Ok((out_f, err_f)) => {
                file_cache.insert(proc_name.to_string(), (out_f, err_f));
                true
            }
            Err(e) => {
                eprintln!("open log files error ({}): {}", proc_name, e);
                false
            }
        }
    }

    pub async fn dispatch(mut rx: mpsc::UnboundedReceiver<LogMsg>) {
        let mut tick = tokio::time::interval(Duration::from_secs(15));
        tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

        let mut bytes_last_15s: u64 = 0;
        let mut file_cache: HashMap<String, (tokio::fs::File, tokio::fs::File)> = HashMap::new();

        loop {
            tokio::select! {
                msg = rx.recv() => {
                    match msg {
                        Some(msg) => {
                            println!("Received log message: {:?}", msg);
                            bytes_last_15s += msg.bytes.len() as u64;

                            let (stdout_path, stderr_path) = Self::ensure_paths(&msg.proc_name);

                            if !Self::ensure_file_handles(&msg.proc_name, &stdout_path, &stderr_path, &mut file_cache).await {
                                println!("123 failed");
                                continue;
                            }

                            let write_res = if let Some((out_f, err_f)) = file_cache.get_mut(&msg.proc_name) {
                                match msg.stream {
                                    StreamKind::Stdout => out_f.write_all(&msg.bytes).await,
                                    StreamKind::Stderr => err_f.write_all(&msg.bytes).await,
                                }
                            } else {
                                println!("133 failed");
                                continue;
                            };

                            if let Err(e) = write_res {
                                eprintln!("write error ({}): {} -> will reopen", msg.proc_name, e);
                                file_cache.remove(&msg.proc_name);

                                println!("141 failed");
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

                    let proc_names: Vec<String> = file_cache.keys().cloned().collect();

                    for proc in proc_names {
                        let (stdout_path, stderr_path) = Self::ensure_paths(&proc);

                        let out_exists = tokio::fs::try_exists(&stdout_path).await.unwrap_or(false);
                        let err_exists = tokio::fs::try_exists(&stderr_path).await.unwrap_or(false);

                        if !out_exists || !err_exists {
                            eprintln!(
                                "[pm3][{}][warn] log file missing (stdout_exists={}, stderr_exists={}) -> reopening",
                                proc, out_exists, err_exists
                            );
                            file_cache.remove(&proc);
                        }
                    }
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

        let _ = Self::ensure_paths(proc_name);

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
