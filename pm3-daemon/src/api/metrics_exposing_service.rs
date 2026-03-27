use std::net::SocketAddr;

use axum::{Router, routing::get};

pub struct ExposingService {
    address: String,
    port: u16,
}

impl ExposingService {
    pub fn init(address: impl Into<String>, port: u16) -> Self {
        Self {
            address: address.into(),
            port,
        }
    }

    pub async fn dispatch(&self) {
        let app = Router::new().route("/healthz", get(Self::healthz));

        let addr: SocketAddr = format!("{}:{}", self.address, self.port)
            .parse()
            .expect("invalid bind address");

        println!("ExposingService is listening on {}", addr);

        let listener = tokio::net::TcpListener::bind(addr)
            .await
            .expect(format!("failed to bind with address: {}", addr).as_str());
        let _ = axum::serve(listener, app).await;
    }

    async fn healthz() -> &'static str {
        "ok"
    }
}
