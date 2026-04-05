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
    pub external_id: u64,
    pub cpu_usage: f32,
    pub memory_usage: u64,
}

#[derive(Debug, Clone)]
struct ProcessSeed {
    external_id: u64,
    name: String,
}

impl From<(u64, String)> for ProcessSeed {
    fn from((external_id, name): (u64, String)) -> Self {
        Self { external_id, name }
    }
}

#[derive(Debug, Clone)]
struct MetricsRow {
    external_id: u64,
    ts: i64,
    cpu_x10: u16,
    mem_kib: u32,
}

#[derive(Debug)]
enum DbMsg {
    Batch(Vec<MetricsRow>),
    SyncNewProcess((u64, String)),
    Shutdown,
    CleanDb,
}

static GLOBAL_TX: OnceLock<mpsc::Sender<MetricsLog>> = OnceLock::new();
static GLOBAL_DB_PATH: OnceLock<PathBuf> = OnceLock::new();
static GLOBAL_DB_TX: OnceLock<mpsc::Sender<DbMsg>> = OnceLock::new();

impl MetricsService {
    pub fn init(processes: Vec<(u64, String)>) -> (mpsc::Receiver<MetricsLog>, PathBuf) {
        if GLOBAL_TX.get().is_some() {
            panic!("MetricsService::init() called more than once");
        }

        let (tx, rx) = mpsc::channel(256);
        GLOBAL_TX.set(tx).expect("Failed to set GLOBAL_TX");

        let db_path = pm3_home_dir_safe().join("metrics.db");
        GLOBAL_DB_PATH
            .set(db_path.clone())
            .expect("Failed to set GLOBAL_DB_PATH");

        let seeded = processes
            .into_iter()
            .map(|(external_id, name)| ProcessSeed { external_id, name })
            .collect::<Vec<_>>();

        let db_tx = spawn_db_worker(&db_path, seeded);
        GLOBAL_DB_TX.set(db_tx).expect("Failed to set GLOBAL_DB_TX");

        (rx, db_path)
    }

    pub fn get_metrics_handle() -> mpsc::Sender<MetricsLog> {
        GLOBAL_TX
            .get()
            .expect("MetricsService::init() not called")
            .clone()
    }

    pub async fn sync_new_process(process: (u64, String)) -> anyhow::Result<(), anyhow::Error> {
        let db_tx = GLOBAL_DB_TX
            .get()
            .expect("MetricsService::init() not called");

        db_tx
            .send(DbMsg::SyncNewProcess(process))
            .await
            .map_err(|e| anyhow::anyhow!("failed to send SyncProcesses to db worker: {}", e))
    }

    pub async fn dispatch(mut rx: mpsc::Receiver<MetricsLog>) {
        let db_tx = GLOBAL_DB_TX
            .get()
            .expect("MetricsService::init() not called")
            .clone();

        const FLUSH_SECS: u64 = 1;
        const MAX_BATCH: usize = 256;

        let mut tick = tokio::time::interval(Duration::from_secs(FLUSH_SECS));
        tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

        let mut tick_clean_db = tokio::time::interval(Duration::from_secs(30 * 60));
        tick_clean_db.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

        let mut batch: Vec<MetricsRow> = Vec::with_capacity(MAX_BATCH);

        loop {
            select! {
                _ = tick.tick() => {
                    if !batch.is_empty() {
                        let _ = db_tx.send(DbMsg::Batch(std::mem::take(&mut batch))).await;
                    }
                }

                _ = tick_clean_db.tick() => {
                    let _ = db_tx.send(DbMsg::CleanDb).await;
                }

                msg = rx.recv() => match msg {
                    Some(log) => {
                        let ts = now_unix_secs();

                        let cpu_x10 = {
                            let v = (log.cpu_usage * 10.0).round().clamp(0.0, 65535.0);
                            v as u16
                        };

                        let mem_kib = {
                            let kib = (log.memory_usage + 512) / 1024;
                            if kib > u32::MAX as u64 {
                                u32::MAX
                            } else {
                                kib as u32
                            }
                        };

                        batch.push(MetricsRow {
                            external_id: log.external_id,
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

fn spawn_db_worker(db_path: &PathBuf, initial_processes: Vec<ProcessSeed>) -> mpsc::Sender<DbMsg> {
    let (tx, mut rx) = mpsc::channel::<DbMsg>(64);
    let db_path = db_path.clone();

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

            let init_db = |conn: &Connection| -> Result<(), String> {
                conn.execute_batch(
                    r#"
                    PRAGMA journal_mode = DELETE;
                    PRAGMA synchronous = NORMAL;
                    PRAGMA temp_store = MEMORY;
                    PRAGMA busy_timeout = 5000;
                    PRAGMA foreign_keys = ON;

                    CREATE TABLE IF NOT EXISTS processes (
                        id           INTEGER PRIMARY KEY AUTOINCREMENT,
                        external_id  INTEGER NOT NULL UNIQUE,
                        name         TEXT NOT NULL UNIQUE
                    );

                    CREATE UNIQUE INDEX IF NOT EXISTS idx_processes_external_id
                        ON processes(external_id);

                    CREATE UNIQUE INDEX IF NOT EXISTS idx_processes_name
                        ON processes(name);

                    CREATE TABLE IF NOT EXISTS metrics (
                        process_id INTEGER NOT NULL,
                        ts         INTEGER NOT NULL,
                        cpu_x10    INTEGER NOT NULL,
                        mem_kib    INTEGER NOT NULL,
                        PRIMARY KEY (process_id, ts),
                        FOREIGN KEY (process_id) REFERENCES processes(id) ON DELETE CASCADE
                    ) WITHOUT ROWID;
                    "#,
                )
                .map_err(|e| format!("metrics sqlite init error: {}", e))
            };

            let sync_processes = |conn: &mut Connection, processes: &[ProcessSeed]| -> Result<(), String> {
                let tx = conn
                    .transaction_with_behavior(TransactionBehavior::Immediate)
                    .map_err(|e| format!("metrics sqlite begin sync tx error: {}", e))?;

                (|| -> Result<(), String> {
                    let mut upsert_proc_stmt = tx
                        .prepare_cached(
                            r#"
                            INSERT INTO processes(external_id, name)
                            VALUES (?1, ?2)
                            ON CONFLICT(name) DO UPDATE SET
                                external_id = excluded.external_id
                            "#,
                        )
                        .map_err(|e| format!("metrics sqlite prepare sync stmt error: {}", e))?;

                    for proc in processes {
                        upsert_proc_stmt
                            .execute(params![proc.external_id as i64, &proc.name])
                            .map_err(|e| {
                                format!(
                                    "metrics sqlite sync process error name={} external_id={}: {}",
                                    proc.name, proc.external_id, e
                                )
                            })?;
                    }

                    Ok(())
                })()?;

                tx.commit()
                    .map_err(|e| format!("metrics sqlite sync commit error: {}", e))
            };

            let warm_proc_cache = |conn: &Connection| -> Result<HashMap<u64, i64>, String> {
                let mut stmt = conn
                    .prepare("SELECT id, external_id FROM processes")
                    .map_err(|e| format!("metrics sqlite prepare cache warmup error: {}", e))?;

                let rows = stmt
                    .query_map([], |row| {
                        let id: i64 = row.get(0)?;
                        let external_id_i64: i64 = row.get(1)?;
                        Ok((id, external_id_i64))
                    })
                    .map_err(|e| format!("metrics sqlite query cache warmup error: {}", e))?;

                let mut proc_cache = HashMap::new();

                for row in rows {
                    let (id, external_id_i64) =
                        row.map_err(|e| format!("metrics sqlite cache warmup row error: {}", e))?;

                    if external_id_i64 >= 0 {
                        proc_cache.insert(external_id_i64 as u64, id);
                    }
                }

                Ok(proc_cache)
            };

            let flush_batch = |conn: &mut Connection,
                               proc_cache: &mut HashMap<u64, i64>,
                               rows: Vec<MetricsRow>|
             -> Result<(), String> {
                if rows.is_empty() {
                    return Ok(());
                }

                let tx = conn
                    .transaction_with_behavior(TransactionBehavior::Immediate)
                    .map_err(|e| format!("metrics sqlite begin tx error: {}", e))?;

                (|| -> Result<(), String> {
                    let mut get_id_stmt = tx
                        .prepare_cached("SELECT id FROM processes WHERE external_id = ?1")
                        .map_err(|e| format!("metrics sqlite prepare get_id error: {}", e))?;

                    let mut ins_metrics_stmt = tx
                        .prepare_cached(
                            "INSERT OR REPLACE INTO metrics(process_id, ts, cpu_x10, mem_kib) VALUES (?1, ?2, ?3, ?4)"
                        )
                        .map_err(|e| format!("metrics sqlite prepare ins_metrics error: {}", e))?;

                    for r in rows {
                        let pid = if let Some(&id) = proc_cache.get(&r.external_id) {
                            id
                        } else {
                            let id = get_id_stmt
                                .query_row(params![r.external_id as i64], |row| row.get(0))
                                .optional()
                                .map_err(|e| {
                                    format!(
                                        "metrics sqlite select process id error external_id={}: {}",
                                        r.external_id, e
                                    )
                                })?;

                            let id = match id {
                                Some(v) => v,
                                None => {
                                    return Err(format!(
                                        "metrics sqlite missing process for external_id={}",
                                        r.external_id
                                    ));
                                }
                            };

                            proc_cache.insert(r.external_id, id);
                            id
                        };

                        ins_metrics_stmt
                            .execute(params![pid, r.ts, r.cpu_x10 as i64, r.mem_kib as i64])
                            .map_err(|e| {
                                format!(
                                    "metrics sqlite insert metrics error external_id={}: {}",
                                    r.external_id, e
                                )
                            })?;
                    }

                    Ok(())
                })()?;

                tx.commit()
                    .map_err(|e| format!("metrics sqlite commit error: {}", e))
            };

            let clean_db = |conn: &mut Connection| -> Result<Option<(usize, i64)>, String> {
                let now = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .map_err(|e| format!("metrics sqlite time error: {}", e))?
                    .as_secs() as i64;

                let cutoff = now - 86_400;

                let tx = conn
                    .transaction_with_behavior(TransactionBehavior::Immediate)
                    .map_err(|e| format!("metrics sqlite begin clean tx error: {}", e))?;

                let deleted = (|| -> Result<usize, String> {
                    tx.execute("DELETE FROM metrics WHERE ts < ?1", params![cutoff])
                        .map_err(|e| format!("metrics sqlite clean delete error: {}", e))
                })()?;

                tx.commit()
                    .map_err(|e| format!("metrics sqlite clean commit error: {}", e))?;

                if deleted > 0 {
                    Ok(Some((deleted, cutoff)))
                } else {
                    Ok(None)
                }
            };

            if let Err(e) = init_db(&conn) {
                eprintln!("{e}");
                return;
            }

            if let Err(e) = sync_processes(&mut conn, &initial_processes) {
                eprintln!("{e}");
                return;
            }

            let mut proc_cache = match warm_proc_cache(&conn) {
                Ok(v) => v,
                Err(e) => {
                    eprintln!("{e}");
                    return;
                }
            };

            while let Some(msg) = rx.blocking_recv() {
                match msg {
                    DbMsg::Batch(rows) => {
                        if let Err(e) = flush_batch(&mut conn, &mut proc_cache, rows) {
                            eprintln!("{e}");
                        }
                    }

                    DbMsg::SyncNewProcess(process) => {
                        if let Err(e) = sync_processes(&mut conn, &[process.into()]) {
                            eprintln!("{e}");
                            continue;
                        }

                        proc_cache = match warm_proc_cache(&conn) {
                            Ok(v) => v,
                            Err(e) => {
                                eprintln!("{e}");
                                HashMap::new()
                            }
                        };
                    }

                    DbMsg::CleanDb => match clean_db(&mut conn) {
                        Ok(Some((deleted, cutoff))) => {
                            println!(
                                "metrics sqlite cleanup: deleted {} rows older than {} (cutoff_ts={})",
                                deleted, 86_400, cutoff
                            );
                        }
                        Ok(None) => {}
                        Err(e) => eprintln!("{e}"),
                    },

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
