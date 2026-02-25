use std::sync::OnceLock;
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

    pub async fn dispatch(mut rx: mpsc::Receiver<MetricsLog>) {
        loop {
            select! {
                msg = rx.recv() => match msg {
                    Some(log) => {
                        // Implement metrics logging logic here
                        println!("Received metrics log: {:?}", log);
                    }
                    None => break,
                }
            }
        }
    }

    pub fn get_metrics_handle() -> mpsc::Sender<MetricsLog> {
        GLOBAL_TX
            .get()
            .expect("MetricsService::init() not called")
            .clone()
    }
}
