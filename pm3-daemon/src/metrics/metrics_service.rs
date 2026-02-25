use crate::utils::pm3_safe_dir::pm3_home_dir_safe;
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

        let (tx, rx) = mpsc::channel(50);
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
        const MAX_BATCH: usize = 1;

        let mut tick = tokio::time::interval(Duration::from_secs(FLUSH_SECS));
        tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

        let mut batch: Vec<MetricsRow> = Vec::with_capacity(MAX_BATCH);

        loop {
            select! {
                _ = tick.tick() => {
                    if !batch.is_empty() {
                        if db_tx.try_send(DbMsg::Batch(std::mem::take(&mut batch))).is_err() {
                            println!("pm3::metrics_service error with sending batch");
                        }
                    }
                }

                msg = rx.recv() => match msg {
                    Some(log) => {
                        let ts = now_unix_secs();
                        batch.push(MetricsRow {
                            proc_name: log.proc_name,
                            ts,
                            cpu: log.cpu_usage as f64,
                            mem: log.memory_usage as i64,
                        });

                        if batch.len() >= MAX_BATCH {
                            let _ = db_tx.try_send(DbMsg::Batch(std::mem::take(&mut batch)));
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
    cpu: f64,
    mem: i64,
}

#[derive(Debug)]
enum DbMsg {
    Batch(Vec<MetricsRow>),
    Shutdown,
}

fn spawn_db_worker(db_path: PathBuf) -> mpsc::Sender<DbMsg> {
    let (tx, mut rx) = mpsc::channel::<DbMsg>(32);

    std::thread::Builder::new()
        .name("pm3-metrics-sqlite".into())
        .spawn(move || {
            use rusqlite::{params, Connection};

            let mut conn = match Connection::open(&db_path) {
                Ok(c) => c,
                Err(e) => {
                    eprintln!("metrics sqlite open error ({}): {}", db_path.display(), e);
                    return;
                }
            };

            let _ = conn.execute_batch(
                r#"
                PRAGMA journal_mode=WAL;
                PRAGMA synchronous=NORMAL;
                PRAGMA temp_store=MEMORY;
                PRAGMA foreign_keys=ON;

                CREATE TABLE IF NOT EXISTS metrics (
                    proc_name TEXT NOT NULL,
                    ts        INTEGER NOT NULL,
                    cpu       REAL NOT NULL,
                    mem       INTEGER NOT NULL,
                    PRIMARY KEY (proc_name, ts)
                );
                "#,
            );

            while let Some(msg) = rx.blocking_recv() {
                match msg {
                    DbMsg::Batch(rows) => {
                        if rows.is_empty() {
                            continue;
                        }

                        let tx = match conn.transaction() {
                            Ok(t) => t,
                            Err(e) => {
                                eprintln!("metrics sqlite begin tx error: {}", e);
                                continue;
                            }
                        };

                        {
                            let mut stmt = match tx.prepare_cached(
                                "INSERT OR REPLACE INTO metrics (proc_name, ts, cpu, mem) VALUES (?1, ?2, ?3, ?4)"
                            ) {
                                Ok(s) => s,
                                Err(e) => {
                                    eprintln!("metrics sqlite prepare error: {}", e);
                                    continue;
                                }
                            };

                            for r in rows {
                                if let Err(e) = stmt.execute(params![r.proc_name, r.ts, r.cpu, r.mem]) {
                                    eprintln!("metrics sqlite insert error: {}", e);
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
