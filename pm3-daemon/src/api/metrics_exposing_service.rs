use std::convert::Infallible;
use std::io::{self, Write};
use std::net::SocketAddr;
use std::path::PathBuf;

use axum::Router;
use axum::body::{Body, Bytes};
use axum::extract::{Path, State};
use axum::http::{Method, StatusCode, Uri, header};
use axum::response::Response;
use axum::routing::get;
use rusqlite::{Connection, OpenFlags, OptionalExtension};
use tokio::sync::mpsc;
use tokio_stream::wrappers::UnboundedReceiverStream;
use tower_http::cors::CorsLayer;

#[derive(Clone)]
pub struct ExposingService {
    address: String,
    port: u16,
    db_path: PathBuf,
}

impl ExposingService {
    pub fn init(address: impl Into<String>, port: u16, db_path: PathBuf) -> Self {
        Self {
            address: address.into(),
            port,
            db_path,
        }
    }

    pub async fn dispatch(&self) {
        let cors = CorsLayer::new()
            .allow_origin(tower_http::cors::Any)
            .allow_methods([Method::GET, Method::POST, Method::OPTIONS])
            .allow_headers(tower_http::cors::Any);

        let app = Router::new()
            .route("/api/v1/healthz", get(Self::healthz))
            .route("/api/v1/list", get(Self::list))
            .route("/api/v1/get_metrics/{external_id}", get(Self::get_metrics))
            .route(
                "/api/v1/get_metrics_compressed/{external_id}",
                get(Self::get_metrics_compressed),
            )
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

            let ts_u64 = if ts < 0 { 0 } else { ts as u64 };
            let cpu_x10 = if cpu_x10_i64 < 0 {
                0
            } else if cpu_x10_i64 > u16::MAX as i64 {
                u16::MAX
            } else {
                cpu_x10_i64 as u16
            };
            let mem_kib = if mem_kib_i64 < 0 {
                0
            } else if mem_kib_i64 > u32::MAX as i64 {
                u32::MAX
            } else {
                mem_kib_i64 as u32
            };

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
