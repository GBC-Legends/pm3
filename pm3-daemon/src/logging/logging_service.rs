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

const MAX_BUF_PER_PROC: usize = 8 * 1024 * 1024; // 8MB
const FLUSH_SECS: u64 = 15;

type FileCache = HashMap<String, (Option<tokio::fs::File>, Option<tokio::fs::File>)>;
type BufCache = HashMap<String, (Vec<u8>, Vec<u8>)>;
type Activity15s = HashMap<String, (u64, u64)>;

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

    async fn ensure_exists_or_reopen(
        proc_name: &str,
        stdout_path: &PathBuf,
        stderr_path: &PathBuf,
        file_cache: &mut FileCache,
    ) {
        let out_exists = tokio::fs::try_exists(stdout_path).await.unwrap_or(false);
        let err_exists = tokio::fs::try_exists(stderr_path).await.unwrap_or(false);

        if !out_exists || !err_exists {
            eprintln!(
                "[pm3][{}][warn] log file missing (stdout_exists={}, stderr_exists={}) -> reopening",
                proc_name, out_exists, err_exists
            );
            if let Some((out_f, err_f)) = file_cache.get_mut(proc_name) {
                if !out_exists {
                    *out_f = None;
                }
                if !err_exists {
                    *err_f = None;
                }
            }
        }
    }

    async fn ensure_file_handles(
        proc_name: &str,
        stdout_path: &PathBuf,
        stderr_path: &PathBuf,
        file_cache: &mut FileCache,
    ) -> bool {
        let entry = file_cache
            .entry(proc_name.to_string())
            .or_insert((None, None));

        if let Some(parent) = stdout_path.parent() {
            if let Err(e) = fs::create_dir_all(parent).await {
                eprintln!("create_dir_all error ({}): {}", proc_name, e);
                return false;
            }
        }

        if entry.0.is_none() {
            match OpenOptions::new()
                .create(true)
                .append(true)
                .open(stdout_path)
                .await
            {
                Ok(f) => entry.0 = Some(f),
                Err(e) => {
                    eprintln!("open stdout log error ({}): {}", proc_name, e);
                    return false;
                }
            }
        }

        if entry.1.is_none() {
            match OpenOptions::new()
                .create(true)
                .append(true)
                .open(stderr_path)
                .await
            {
                Ok(f) => entry.1 = Some(f),
                Err(e) => {
                    eprintln!("open stderr log error ({}): {}", proc_name, e);
                    return false;
                }
            }
        }

        true
    }

    async fn flush_proc(proc_name: &str, buf_cache: &mut BufCache, file_cache: &mut FileCache) {
        let Some((out_buf, err_buf)) = buf_cache.get_mut(proc_name) else {
            return;
        };

        if out_buf.is_empty() && err_buf.is_empty() {
            return;
        }

        let (stdout_path, stderr_path) = Self::ensure_paths(proc_name);

        Self::ensure_exists_or_reopen(proc_name, &stdout_path, &stderr_path, file_cache).await;

        if !Self::ensure_file_handles(proc_name, &stdout_path, &stderr_path, file_cache).await {
            return;
        }

        let Some((out_f_opt, err_f_opt)) = file_cache.get_mut(proc_name) else {
            return;
        };

        if !out_buf.is_empty() {
            if let Some(out_f) = out_f_opt.as_mut() {
                if let Err(e) = out_f.write_all(out_buf).await {
                    eprintln!("stdout write error ({}): {} -> will reopen", proc_name, e);
                    *out_f_opt = None;
                    return;
                }
                out_buf.clear();
            } else {
                return;
            }
        }

        if !err_buf.is_empty() {
            if let Some(err_f) = err_f_opt.as_mut() {
                if let Err(e) = err_f.write_all(err_buf).await {
                    eprintln!("stderr write error ({}): {} -> will reopen", proc_name, e);
                    *err_f_opt = None;
                    return;
                }
                err_buf.clear();
            } else {
                return;
            }
        }
    }

    async fn flush_all(buf_cache: &mut BufCache, file_cache: &mut FileCache) {
        let procs: Vec<String> = buf_cache.keys().cloned().collect();
        for proc in procs {
            Self::flush_proc(&proc, buf_cache, file_cache).await;
        }
    }

    fn close_idle_fds(file_cache: &mut FileCache, activity_15s: &mut Activity15s) {
        let procs: Vec<String> = activity_15s.keys().cloned().collect();

        for proc in procs {
            let (out_b, err_b) = activity_15s.get(&proc).copied().unwrap_or((0, 0));

            if let Some((out_f, err_f)) = file_cache.get_mut(&proc) {
                if out_b == 0 {
                    *out_f = None;
                }
                if err_b == 0 {
                    *err_f = None;
                }
            }

            activity_15s.insert(proc, (0, 0));
        }
    }

    pub async fn dispatch(mut rx: mpsc::UnboundedReceiver<LogMsg>) {
        let mut tick = tokio::time::interval(Duration::from_secs(FLUSH_SECS));
        tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

        let mut bytes_last_15s: u64 = 0;

        let mut file_cache: FileCache = HashMap::new();
        let mut buf_cache: BufCache = HashMap::new();
        let mut activity_15s: Activity15s = HashMap::new();

        loop {
            tokio::select! {
                msg = rx.recv() => {
                    match msg {
                        Some(msg) => {
                            let n = msg.bytes.len() as u64;
                            bytes_last_15s += n;

                            let act = activity_15s.entry(msg.proc_name.clone()).or_insert((0,0));
                            match msg.stream {
                                StreamKind::Stdout => act.0 += n,
                                StreamKind::Stderr => act.1 += n,
                            }

                            let entry = buf_cache
                                .entry(msg.proc_name.clone())
                                .or_insert_with(|| (Vec::with_capacity(4096), Vec::with_capacity(4096)));

                            match msg.stream {
                                StreamKind::Stdout => entry.0.extend_from_slice(&msg.bytes),
                                StreamKind::Stderr => entry.1.extend_from_slice(&msg.bytes),
                            }

                            let total = entry.0.len() + entry.1.len();
                            if total >= MAX_BUF_PER_PROC {
                                Self::flush_proc(&msg.proc_name, &mut buf_cache, &mut file_cache).await;
                            }
                        }
                        None => {
                            Self::flush_all(&mut buf_cache, &mut file_cache).await;
                            break;
                        }
                    }
                }

                _ = tick.tick() => {
                    println!(
                        "[pm3][stats] received {} bytes in last 15s (~{} B/s)",
                        bytes_last_15s,
                        bytes_last_15s / FLUSH_SECS
                    );
                    bytes_last_15s = 0;

                    Self::flush_all(&mut buf_cache, &mut file_cache).await;

                    Self::close_idle_fds(&mut file_cache, &mut activity_15s);
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
