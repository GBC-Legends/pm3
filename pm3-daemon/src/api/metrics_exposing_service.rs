use std::net::SocketAddr;
use std::path::PathBuf;

use axum::Router;
use axum::extract::State;
use axum::http::{StatusCode, header};
use axum::response::Response;
use axum::routing::get;
use rusqlite::{Connection, OpenFlags};

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
        let app = Router::new()
            .route("/api/v1/healthz", get(Self::healthz))
            .route("/api/v1/list", get(Self::list))
            .with_state(self.clone());

        let addr: SocketAddr = format!("{}:{}", self.address, self.port)
            .parse()
            .expect("invalid bind address");

        let listener = tokio::net::TcpListener::bind(addr)
            .await
            .unwrap_or_else(|e| panic!("failed to bind with address {}: {}", addr, e));

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
}
