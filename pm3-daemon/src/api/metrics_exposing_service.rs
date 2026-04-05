use std::convert::Infallible;
use std::io::{self, Write};
use std::net::SocketAddr;
use std::path::PathBuf;

use axum::Router;
use axum::body::{Body, Bytes};
use axum::extract::{Path, State};
use axum::http::{HeaderValue, Method, StatusCode, Uri, header};
use axum::response::Response;
use axum::routing::get;
use rand::{Rng, distributions::Alphanumeric};
use rusqlite::{Connection, OpenFlags, OptionalExtension};
use tokio::sync::mpsc;
use tokio::time::{Duration, interval};
use tokio_stream::wrappers::UnboundedReceiverStream;
use tower_http::cors::CorsLayer;
use tower_http::services::ServeDir;

use crate::logging::LogChunk;
use crate::logging::logging_subscription::LoggingSubscriptionAction;
use crate::utils::pm3_safe_dir::pm3_home_dir_safe;

#[derive(Clone)]
pub struct ExposingService {
    address: String,
    port: u16,
    db_path: PathBuf,
    shared_logging: mpsc::Sender<LoggingSubscriptionAction>,
}

impl ExposingService {
    pub fn init(
        address: impl Into<String>,
        port: u16,
        db_path: PathBuf,
        shared_logging: mpsc::Sender<LoggingSubscriptionAction>,
    ) -> Self {
        Self {
            address: address.into(),
            port,
            db_path,
            shared_logging,
        }
    }

    pub async fn dispatch(&self) {
        let cors = CorsLayer::new()
            .allow_origin(tower_http::cors::Any)
            .allow_methods([Method::GET, Method::POST, Method::OPTIONS])
            .allow_headers(tower_http::cors::Any);

        let dashboard_path = pm3_home_dir_safe().join("dashboard/");

        let app = Router::new()
            .route("/api/v1/healthz", get(Self::healthz))
            .route("/api/v1/list", get(Self::list))
            .route("/api/v1/get_metrics/{external_id}", get(Self::get_metrics))
            .route(
                "/api/v1/get_metrics_compressed/{external_id}",
                get(Self::get_metrics_compressed),
            )
            .route("/api/v1/subscribe_logs", get(Self::subscribe_logs))
            // Serve the dashboard from the dashboard_path directory
            .nest_service("/dash", ServeDir::new(dashboard_path))
            .layer(cors)
            .with_state(self.clone());

        let addr: SocketAddr = format!("{}:{}", self.address, self.port)
            .parse()
            .expect("invalid bind address");

        let listener = tokio::net::TcpListener::bind(addr).await.unwrap();

        println!("ExposingService is listening on {}", addr);

        let _ = axum::serve(listener, app).await;
    }

    async fn healthz() -> &'static str {
        "ok"
    }

    async fn list(State(state): State<ExposingService>) -> Response {
        let db_path = state.db_path.clone();

        let result = tokio::task::spawn_blocking(move || {
            let conn = Connection::open_with_flags(
                db_path,
                OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
            )
            .map_err(|e| format!("db open error: {}", e))?;

            let mut stmt = conn
                .prepare("SELECT external_id, name FROM processes ORDER BY id ASC")
                .map_err(|e| format!("prepare error: {}", e))?;

            let mut rows = stmt.query([]).map_err(|e| format!("query error: {}", e))?;

            let mut body = String::with_capacity(1024);

            while let Some(row) = rows.next().map_err(|e| format!("rows next error: {}", e))? {
                let id: i64 = row.get(0).map_err(|e| format!("row id error: {}", e))?;
                let name: String = row.get(1).map_err(|e| format!("row name error: {}", e))?;

                body.push_str(&id.to_string());
                body.push_str(": ");
                body.push_str(&name);
                body.push('\n');
            }

            Ok::<String, String>(body)
        })
        .await;

        match result {
            Ok(Ok(body)) => Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, "text/plain; charset=utf-8")
                .body(body.into())
                .unwrap(),

            Ok(Err(err)) => Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .header(header::CONTENT_TYPE, "text/plain; charset=utf-8")
                .body(err.into())
                .unwrap(),

            Err(err) => Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .header(header::CONTENT_TYPE, "text/plain; charset=utf-8")
                .body(format!("join error: {}", err).into())
                .unwrap(),
        }
    }

    async fn subscribe_logs(State(state): State<ExposingService>, uri: Uri) -> Response {
        let params = match Self::parse_programs_and_line_from_query(uri.query().unwrap_or("")) {
            Some(params) => params,
            None => {
                return Response::builder()
                    .status(StatusCode::BAD_REQUEST)
                    .header(header::CONTENT_TYPE, "text/plain; charset=utf-8")
                    .body(Body::from("invalid query"))
                    .unwrap();
            }
        };

        let shared_logging = state.shared_logging.clone();

        let (tx_stream, rx_stream) = mpsc::unbounded_channel::<Result<Bytes, Infallible>>();

        tokio::spawn(async move {
            let (tx_logs, mut rx_logs) = mpsc::unbounded_channel::<LogChunk>();

            let sub_id: String = rand::thread_rng()
                .sample_iter(&Alphanumeric)
                .take(5)
                .map(char::from)
                .collect();

            let subscription = LoggingSubscriptionAction::Subscribe {
                id: sub_id.clone(),
                tx: tx_logs,
                programs: params.0,
                lines: params.1,
            };

            if shared_logging.send(subscription).await.is_err() {
                let _ = tx_stream.send(Ok(Bytes::from_static(b"eof\n")));
                return;
            }

            let mut ping = interval(Duration::from_secs(15));
            ping.tick().await;

            loop {
                tokio::select! {
                    _ = ping.tick() => {
                        if tx_stream.send(Ok(Bytes::from_static(b"ping\n"))).is_err() {
                            println!("client disconnected by ping, unsubscribing: {sub_id}");
                            let _ = shared_logging
                                .send(LoggingSubscriptionAction::Unsubscribe {
                                    id: sub_id.clone(),
                                })
                                .await;
                            return;
                        }
                    }

                    maybe_chunk = rx_logs.recv() => {
                        match maybe_chunk {
                            Some(LogChunk::Line(line)) => {
                                let mut out = line;
                                if !out.ends_with('\n') {
                                    out.push('\n');
                                }

                                if tx_stream.send(Ok(Bytes::from(out))).is_err() {
                                    println!("client disconnected, unsubscribing: {sub_id}");
                                    let _ = shared_logging
                                        .send(LoggingSubscriptionAction::Unsubscribe {
                                            id: sub_id.clone(),
                                        })
                                        .await;
                                    return;
                                }
                            }
                            Some(LogChunk::Eof) => {
                                let _ = tx_stream.send(Ok(Bytes::from_static(b"eof\n")));
                                let _ = shared_logging
                                    .send(LoggingSubscriptionAction::Unsubscribe {
                                        id: sub_id.clone(),
                                    })
                                    .await;
                                return;
                            }
                            Some(LogChunk::Ping) => {}
                            None => {
                                println!("subscription source closed, unsubscribing: {sub_id}");
                                let _ = shared_logging
                                    .send(LoggingSubscriptionAction::Unsubscribe {
                                        id: sub_id.clone(),
                                    })
                                    .await;

                                let _ = tx_stream.send(Ok(Bytes::from_static(b"eof\n")));
                                return;
                            }
                        }
                    }
                }
            }
        });

        let stream = UnboundedReceiverStream::new(rx_stream);
        let body = Body::from_stream(stream);

        Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, "text/plain; charset=utf-8")
            .header(header::CACHE_CONTROL, HeaderValue::from_static("no-cache"))
            .body(body)
            .unwrap()
    }

    fn parse_programs_and_line_from_query(query: &str) -> Option<(Vec<String>, u64)> {
        let mut programs: Vec<String> = Vec::new();
        let mut lines: u64 = 15;

        if query.is_empty() {
            return Some((programs, lines));
        }

        for pair in query.split('&') {
            let Some((key, value)) = pair.split_once('=') else {
                continue;
            };

            if key == "p" {
                if value.parse::<u64>().is_ok() {
                    programs.push(value.to_string());
                }
            } else if key == "lines" {
                if let Ok(line) = value.parse::<u64>() {
                    lines = line;
                }
            }
        }

        Some((programs, lines))
    }

    async fn get_metrics(
        State(state): State<ExposingService>,
        Path(external_id): Path<u64>,
        uri: Uri,
    ) -> Response {
        let db_path = state.db_path.clone();

        let since = uri
            .query()
            .and_then(Self::parse_since_from_query)
            .unwrap_or(0);

        let result = tokio::task::spawn_blocking(move || {
            let conn = Connection::open_with_flags(
                db_path,
                OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
            )
            .map_err(|e| format!("db open error: {}", e))?;

            let process_id: Option<i64> = conn
                .query_row(
                    "SELECT id FROM processes WHERE external_id = ?1 LIMIT 1",
                    [external_id as i64],
                    |row| row.get(0),
                )
                .optional()
                .map_err(|e| format!("failed to resolve process id: {}", e))?;

            let process_id = match process_id {
                Some(v) => v,
                None => {
                    return Err(format!(
                        "process with external_id={} not found",
                        external_id
                    ));
                }
            };

            let mut stmt = conn
                .prepare(
                    r#"
                    SELECT ts, cpu_x10, mem_kib
                    FROM metrics
                    WHERE process_id = ?1 AND ts >= ?2
                    ORDER BY ts ASC
                    "#,
                )
                .map_err(|e| format!("prepare error: {}", e))?;

            let mut rows = stmt
                .query([process_id, since])
                .map_err(|e| format!("query error: {}", e))?;

            let mut body = String::with_capacity(4096);

            while let Some(row) = rows.next().map_err(|e| format!("rows next error: {}", e))? {
                let ts: i64 = row.get(0).map_err(|e| format!("row ts error: {}", e))?;
                let cpu_x10: i64 = row
                    .get(1)
                    .map_err(|e| format!("row cpu_x10 error: {}", e))?;
                let mem_kib: i64 = row
                    .get(2)
                    .map_err(|e| format!("row mem_kib error: {}", e))?;

                let cpu = cpu_x10 / 10;

                body.push_str(&ts.to_string());
                body.push_str(": ");
                body.push_str(&format!("{cpu}"));
                body.push_str(", ");
                body.push_str(&mem_kib.to_string());
                body.push('\n');
            }

            Ok::<String, String>(body)
        })
        .await;

        match result {
            Ok(Ok(body)) => Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, "text/plain; charset=utf-8")
                .body(body.into())
                .unwrap(),

            Ok(Err(err)) => {
                let status = if err.contains("not found") {
                    StatusCode::NOT_FOUND
                } else {
                    StatusCode::INTERNAL_SERVER_ERROR
                };

                Response::builder()
                    .status(status)
                    .header(header::CONTENT_TYPE, "text/plain; charset=utf-8")
                    .body(err.into())
                    .unwrap()
            }

            Err(err) => Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .header(header::CONTENT_TYPE, "text/plain; charset=utf-8")
                .body(format!("join error: {}", err).into())
                .unwrap(),
        }
    }

    fn parse_since_from_query(query: &str) -> Option<i64> {
        for pair in query.split('&') {
            let (key, value) = pair.split_once('=')?;
            if key == "since" {
                if let Ok(v) = value.parse::<i64>() {
                    return Some(v);
                }
            }
        }

        None
    }

    async fn get_metrics_compressed(
        State(state): State<ExposingService>,
        Path(external_id): Path<u64>,
        uri: Uri,
    ) -> Response {
        let since = uri
            .query()
            .and_then(Self::parse_since_from_query)
            .unwrap_or(0);

        let db_path = state.db_path.clone();

        let (tx, rx) = mpsc::unbounded_channel::<Result<Bytes, Infallible>>();

        tokio::task::spawn_blocking(move || {
            if let Err(err) = Self::stream_metrics_zstd(db_path, external_id, since, tx) {
                eprintln!("get_metrics_bin error: {}", err);
            }
        });

        let stream = UnboundedReceiverStream::new(rx);
        let body = Body::from_stream(stream);

        Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, "application/octet-stream")
            .header("x-pm3-format", "pm3-metrics-v1")
            .header("x-pm3-compression", "zstd")
            .body(body)
            .unwrap()
    }

    fn stream_metrics_zstd(
        db_path: PathBuf,
        external_id: u64,
        since: i64,
        tx: mpsc::UnboundedSender<Result<Bytes, Infallible>>,
    ) -> Result<(), String> {
        let conn = Connection::open_with_flags(
            db_path,
            OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )
        .map_err(|e| format!("db open error: {}", e))?;

        let process_id: Option<i64> = conn
            .query_row(
                "SELECT id FROM processes WHERE external_id = ?1 LIMIT 1",
                [external_id as i64],
                |row| row.get(0),
            )
            .optional()
            .map_err(|e| format!("failed to resolve process id: {}", e))?;

        let process_id = match process_id {
            Some(v) => v,
            None => {
                return Err(format!(
                    "process with external_id={} not found",
                    external_id
                ));
            }
        };

        let mut stmt = conn
            .prepare(
                r#"
                    SELECT ts, cpu_x10, mem_kib
                    FROM metrics
                    WHERE process_id = ?1 AND ts >= ?2
                    ORDER BY ts ASC
                    "#,
            )
            .map_err(|e| format!("prepare error: {}", e))?;

        let mut rows = stmt
            .query([process_id, since])
            .map_err(|e| format!("query error: {}", e))?;

        let writer = ChunkedBytesWriter::new(tx, 32 * 1024);
        let mut encoder = zstd::stream::write::Encoder::new(writer, 3)
            .map_err(|e| format!("zstd encoder init error: {}", e))?;

        // stream header:
        // magic[4] = PM3M
        // version[1] = 1
        // record_size[1] = 14
        encoder
            .write_all(b"PM3M\x01\x0E")
            .map_err(|e| format!("zstd write header error: {}", e))?;

        let mut rec = [0u8; 14];

        while let Some(row) = rows.next().map_err(|e| format!("rows next error: {}", e))? {
            let ts: i64 = row.get(0).map_err(|e| format!("row ts error: {}", e))?;
            let cpu_x10_i64: i64 = row
                .get(1)
                .map_err(|e| format!("row cpu_x10 error: {}", e))?;
            let mem_kib_i64: i64 = row
                .get(2)
                .map_err(|e| format!("row mem_kib error: {}", e))?;

            let ts_u64 = ts.max(0) as u64;
            let cpu_x10 = cpu_x10_i64.clamp(0, u16::MAX as i64) as u16;
            let mem_kib = mem_kib_i64.clamp(0, u32::MAX as i64) as u32;

            Self::encode_metric_record(ts_u64, cpu_x10, mem_kib, &mut rec);

            encoder
                .write_all(&rec)
                .map_err(|e| format!("zstd write record error: {}", e))?;
        }

        let mut writer = encoder
            .finish()
            .map_err(|e| format!("zstd finish error: {}", e))?;

        writer
            .finish()
            .map_err(|e| format!("chunk writer finish error: {}", e))?;

        Ok(())
    }

    #[inline]
    fn encode_metric_record(ts: u64, cpu_x10: u16, mem_kib: u32, out: &mut [u8; 14]) {
        out[0..8].copy_from_slice(&ts.to_le_bytes());
        out[8..10].copy_from_slice(&cpu_x10.to_le_bytes());
        out[10..14].copy_from_slice(&mem_kib.to_le_bytes());
    }
}

struct ChunkedBytesWriter {
    tx: mpsc::UnboundedSender<Result<Bytes, Infallible>>,
    buf: Vec<u8>,
    chunk_size: usize,
}

impl ChunkedBytesWriter {
    fn new(tx: mpsc::UnboundedSender<Result<Bytes, Infallible>>, chunk_size: usize) -> Self {
        Self {
            tx,
            buf: Vec::with_capacity(chunk_size * 2),
            chunk_size,
        }
    }

    fn flush_chunk(&mut self) -> io::Result<()> {
        if self.buf.is_empty() {
            return Ok(());
        }

        let bytes = Bytes::from(std::mem::take(&mut self.buf));

        self.tx
            .send(Ok(bytes))
            .map_err(|_| io::Error::new(io::ErrorKind::BrokenPipe, "receiver dropped"))?;

        self.buf = Vec::with_capacity(self.chunk_size * 2);
        Ok(())
    }

    fn finish(&mut self) -> io::Result<()> {
        self.flush_chunk()
    }
}

impl Write for ChunkedBytesWriter {
    fn write(&mut self, src: &[u8]) -> io::Result<usize> {
        self.buf.extend_from_slice(src);

        if self.buf.len() >= self.chunk_size {
            self.flush_chunk()?;
        }

        Ok(src.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        self.flush_chunk()
    }
}
