use crate::utils::pm3_safe_dir::pm3_home_dir_safe;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::OnceLock;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use tokio::select;
use tokio::sync::mpsc;

pub struct MetricsService;

#[derive(Debug, Clone)]
pub struct MetricsLog {
    pub proc_name: String,
    pub cpu_usage: f32,
    pub memory_usage: u64,
}

static GLOBAL_TX: OnceLock<mpsc::Sender<MetricsLog>> = OnceLock::new();

impl MetricsService {
    pub fn init() -> mpsc::Receiver<MetricsLog> {
        if GLOBAL_TX.get().is_some() {
            panic!("MetricsService::init() called more than once");
        }
        let (tx, rx) = mpsc::channel(256);
        GLOBAL_TX.set(tx).expect("Failed to set GLOBAL_TX");
        rx
    }

    pub fn get_metrics_handle() -> mpsc::Sender<MetricsLog> {
        GLOBAL_TX
            .get()
            .expect("MetricsService::init() not called")
            .clone()
    }

    pub async fn dispatch(mut rx: mpsc::Receiver<MetricsLog>) {
        let db_path = pm3_home_dir_safe().join("metrics.db");
        let db_tx = spawn_db_worker(db_path);

        const FLUSH_SECS: u64 = 1;
        const MAX_BATCH: usize = 256;

        let mut tick = tokio::time::interval(Duration::from_secs(FLUSH_SECS));
        tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

        let mut batch: Vec<MetricsRow> = Vec::with_capacity(MAX_BATCH);

        loop {
            select! {
                _ = tick.tick() => {
                    if !batch.is_empty() {
                        let _ = db_tx.send(DbMsg::Batch(std::mem::take(&mut batch))).await;
                    }
                }

                msg = rx.recv() => match msg {
                    Some(log) => {
                        let ts = now_unix_secs();

                        let cpu_x10: u16 = {
                            let v = (log.cpu_usage * 10.0).round();
                            let v = v.clamp(0.0, 65535.0);
                            v as u16
                        };

                        let mem_kib: u32 = {
                            let kib = (log.memory_usage + 512) / 1024;
                            if kib > u32::MAX as u64 { u32::MAX } else { kib as u32 }
                        };

                        batch.push(MetricsRow {
                            proc_name: log.proc_name,
                            ts,
                            cpu_x10,
                            mem_kib,
                        });

                        if batch.len() >= MAX_BATCH {
                            let _ = db_tx.send(DbMsg::Batch(std::mem::take(&mut batch))).await;
                        }
                    }
                    None => {
                        if !batch.is_empty() {
                            let _ = db_tx.send(DbMsg::Batch(std::mem::take(&mut batch))).await;
                        }
                        let _ = db_tx.send(DbMsg::Shutdown).await;
                        break;
                    }
                }
            }
        }
    }
}

#[derive(Debug, Clone)]
struct MetricsRow {
    proc_name: String,
    ts: i64,
    cpu_x10: u16,
    mem_kib: u32,
}

#[derive(Debug)]
enum DbMsg {
    Batch(Vec<MetricsRow>),
    Shutdown,
}

fn spawn_db_worker(db_path: PathBuf) -> mpsc::Sender<DbMsg> {
    let (tx, mut rx) = mpsc::channel::<DbMsg>(64);

    std::thread::Builder::new()
        .name("pm3-metrics-sqlite".into())
        .spawn(move || {
            use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};

            let mut conn = match Connection::open(&db_path) {
                Ok(c) => c,
                Err(e) => {
                    eprintln!("metrics sqlite open error ({}): {}", db_path.display(), e);
                    return;
                }
            };

            let _ = conn.busy_timeout(std::time::Duration::from_secs(5));

            if let Err(e) = conn.execute_batch(
                r#"
                PRAGMA journal_mode = DELETE;
                PRAGMA synchronous = NORMAL;
                PRAGMA temp_store = MEMORY;
                PRAGMA busy_timeout = 5000;
                PRAGMA foreign_keys = ON;

                CREATE TABLE IF NOT EXISTS processes (
                    id   INTEGER PRIMARY KEY,
                    name TEXT NOT NULL UNIQUE
                );

                CREATE TABLE IF NOT EXISTS metrics (
                    process_id INTEGER NOT NULL,
                    ts         INTEGER NOT NULL,
                    cpu_x10    INTEGER NOT NULL,
                    mem_kib    INTEGER NOT NULL,
                    PRIMARY KEY (process_id, ts),
                    FOREIGN KEY (process_id) REFERENCES processes(id) ON DELETE CASCADE
                ) WITHOUT ROWID;
                "#,
            ) {
                eprintln!("metrics sqlite init error: {}", e);
                return;
            }

            let mut proc_cache: HashMap<String, i64> = HashMap::new();

            while let Some(msg) = rx.blocking_recv() {
                match msg {
                    DbMsg::Batch(rows) => {
                        if rows.is_empty() {
                            continue;
                        }

                        let tx = match conn.transaction_with_behavior(TransactionBehavior::Immediate) {
                            Ok(t) => t,
                            Err(e) => {
                                eprintln!("metrics sqlite begin tx error: {}", e);
                                continue;
                            }
                        };

                        {
                            let mut get_id_stmt = match tx.prepare_cached(
                                "SELECT id FROM processes WHERE name = ?1"
                            ) {
                                Ok(s) => s,
                                Err(e) => {
                                    eprintln!("metrics sqlite prepare get_id error: {}", e);
                                    continue;
                                }
                            };

                            let mut ins_proc_stmt = match tx.prepare_cached(
                                "INSERT OR IGNORE INTO processes(name) VALUES (?1)"
                            ) {
                                Ok(s) => s,
                                Err(e) => {
                                    eprintln!("metrics sqlite prepare ins_proc error: {}", e);
                                    continue;
                                }
                            };

                            let mut ins_metrics_stmt = match tx.prepare_cached(
                                "INSERT OR REPLACE INTO metrics(process_id, ts, cpu_x10, mem_kib) VALUES (?1, ?2, ?3, ?4)"
                            ) {
                                Ok(s) => s,
                                Err(e) => {
                                    eprintln!("metrics sqlite prepare ins_metrics error: {}", e);
                                    continue;
                                }
                            };

                            for r in rows {
                                let pid = if let Some(&id) = proc_cache.get(&r.proc_name) {
                                    id
                                } else {
                                    if let Err(e) = ins_proc_stmt.execute(params![&r.proc_name]) {
                                        eprintln!("metrics sqlite insert process error: {}", e);
                                        continue;
                                    }

                                    let id: Option<i64> = match get_id_stmt
                                        .query_row(params![&r.proc_name], |row| row.get(0))
                                        .optional()
                                    {
                                        Ok(v) => v,
                                        Err(e) => {
                                            eprintln!("metrics sqlite select process id error: {}", e);
                                            continue;
                                        }
                                    };

                                    let Some(id) = id else {
                                        eprintln!("metrics sqlite process id missing after insert: {}", r.proc_name);
                                        continue;
                                    };

                                    proc_cache.insert(r.proc_name.clone(), id);
                                    id
                                };

                                if let Err(e) = ins_metrics_stmt.execute(params![
                                    pid,
                                    r.ts,
                                    r.cpu_x10 as i64,
                                    r.mem_kib as i64
                                ]) {
                                    eprintln!("metrics sqlite insert metrics error: {}", e);
                                }
                            }
                        }

                        if let Err(e) = tx.commit() {
                            eprintln!("metrics sqlite commit error: {}", e);
                        }
                    }
                    DbMsg::Shutdown => break,
                }
            }
        })
        .expect("failed to spawn pm3-metrics-sqlite thread");

    tx
}

fn now_unix_secs() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_else(|_| Duration::from_secs(0))
        .as_secs() as i64
}
